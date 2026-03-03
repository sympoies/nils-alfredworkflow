#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage:
  $(basename "$0") <version> [--remote <name>] [--dry-run] [--force-tag] [--poll-seconds <n>] [--max-wait-seconds <n>]

Examples:
  $(basename "$0") v0.1.0
  $(basename "$0") v0.1.1 --remote origin
  $(basename "$0") v0.1.1 --force-tag
  $(basename "$0") v0.1.1 --poll-seconds 15 --max-wait-seconds 1800
  $(basename "$0") v0.2.0 --dry-run
USAGE
}

fail() {
  local code="$1"
  shift
  echo "error: $*" >&2
  exit "$code"
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || fail 3 "missing required command: $cmd"
}

parse_positive_int() {
  local name="$1"
  local value="$2"
  [[ "$value" =~ ^[0-9]+$ ]] || fail 2 "$name must be a positive integer"
  (( value > 0 )) || fail 2 "$name must be a positive integer"
}

to_github_repo_path() {
  local remote_url="$1"
  local repo_path=""

  if [[ "$remote_url" =~ ^git@github\.com:(.+)$ ]]; then
    repo_path="${BASH_REMATCH[1]}"
  elif [[ "$remote_url" =~ ^https://github\.com/(.+)$ ]]; then
    repo_path="${BASH_REMATCH[1]}"
  elif [[ "$remote_url" =~ ^ssh://git@github\.com/(.+)$ ]]; then
    repo_path="${BASH_REMATCH[1]}"
  else
    return 1
  fi

  repo_path="${repo_path%.git}"
  printf '%s\n' "$repo_path"
}

to_release_url() {
  local remote_url="$1"
  local version="$2"
  local repo_path
  if ! repo_path="$(to_github_repo_path "$remote_url")"; then
    return 1
  fi
  echo "https://github.com/${repo_path}/releases/tag/${version}"
}

ensure_github_cli_ready() {
  require_cmd gh
  gh auth status >/dev/null 2>&1 \
    || fail 3 "gh is not authenticated; run 'gh auth login' before release waiting"
}

wait_for_release_workflow_success() {
  local repo_path="$1"
  local version="$2"
  local poll_seconds="$3"
  local max_wait_seconds="$4"
  local start now elapsed run_json run_status run_conclusion run_url run_id

  start="$(date +%s)"
  while true; do
    run_json="$(gh run list \
      --repo "$repo_path" \
      --workflow release.yml \
      --event push \
      --limit 50 \
      --json databaseId,headBranch,status,conclusion,url \
      --jq "map(select(.headBranch == \"$version\"))[0]")"

    if [[ -n "$run_json" && "$run_json" != "null" ]]; then
      run_id="$(gh run list \
        --repo "$repo_path" \
        --workflow release.yml \
        --event push \
        --limit 50 \
        --json databaseId,headBranch \
        --jq "map(select(.headBranch == \"$version\"))[0].databaseId")"
      run_status="$(gh run list \
        --repo "$repo_path" \
        --workflow release.yml \
        --event push \
        --limit 50 \
        --json status,headBranch \
        --jq "map(select(.headBranch == \"$version\"))[0].status")"
      run_conclusion="$(gh run list \
        --repo "$repo_path" \
        --workflow release.yml \
        --event push \
        --limit 50 \
        --json conclusion,headBranch \
        --jq "map(select(.headBranch == \"$version\"))[0].conclusion")"
      run_url="$(gh run list \
        --repo "$repo_path" \
        --workflow release.yml \
        --event push \
        --limit 50 \
        --json url,headBranch \
        --jq "map(select(.headBranch == \"$version\"))[0].url")"

      echo "release workflow run: id=${run_id:-unknown} status=${run_status:-unknown} conclusion=${run_conclusion:-pending}"

      if [[ "$run_status" == "completed" ]]; then
        if [[ "$run_conclusion" == "success" ]]; then
          echo "ok: release workflow succeeded for tag ${version}"
          return 0
        fi
        echo "error: release workflow failed for tag ${version} (conclusion=${run_conclusion:-unknown})" >&2
        if [[ -n "$run_url" && "$run_url" != "null" ]]; then
          echo "error: failed run url: $run_url" >&2
        fi
        return 1
      fi
    else
      echo "waiting: release workflow run for tag ${version} not found yet"
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if (( elapsed >= max_wait_seconds )); then
      echo "error: timed out waiting for release workflow (elapsed=${elapsed}s, limit=${max_wait_seconds}s)" >&2
      return 124
    fi

    sleep "$poll_seconds"
  done
}

wait_for_release_page() {
  local repo_path="$1"
  local version="$2"
  local poll_seconds="$3"
  local max_wait_seconds="$4"
  local start now elapsed release_url

  start="$(date +%s)"
  while true; do
    release_url="$(gh release view "$version" --repo "$repo_path" --json url --jq '.url' 2>/dev/null || true)"
    if [[ -n "$release_url" ]]; then
      echo "release page: $release_url"
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if (( elapsed >= max_wait_seconds )); then
      echo "error: timed out waiting for release page for ${version} (elapsed=${elapsed}s, limit=${max_wait_seconds}s)" >&2
      return 124
    fi

    echo "waiting: release page for ${version} not available yet"
    sleep "$poll_seconds"
  done
}

wait_for_ci_workflow_success_for_sha() {
  local repo_path="$1"
  local head_sha="$2"
  local poll_seconds="$3"
  local max_wait_seconds="$4"
  local start now elapsed run_json run_status run_conclusion run_url run_id
  local ci_workflow_file=".github/workflows/ci.yml"

  if [[ ! -f "$ci_workflow_file" ]]; then
    echo "note: ${ci_workflow_file} not found; skipping CI wait gate"
    return 0
  fi

  start="$(date +%s)"
  while true; do
    run_json="$(gh run list \
      --repo "$repo_path" \
      --workflow "$ci_workflow_file" \
      --event push \
      --commit "$head_sha" \
      --limit 20 \
      --json databaseId,status,conclusion,url \
      --jq '.[0]')"

    if [[ -n "$run_json" && "$run_json" != "null" ]]; then
      run_id="$(gh run list \
        --repo "$repo_path" \
        --workflow "$ci_workflow_file" \
        --event push \
        --commit "$head_sha" \
        --limit 20 \
        --json databaseId \
        --jq '.[0].databaseId')"
      run_status="$(gh run list \
        --repo "$repo_path" \
        --workflow "$ci_workflow_file" \
        --event push \
        --commit "$head_sha" \
        --limit 20 \
        --json status \
        --jq '.[0].status')"
      run_conclusion="$(gh run list \
        --repo "$repo_path" \
        --workflow "$ci_workflow_file" \
        --event push \
        --commit "$head_sha" \
        --limit 20 \
        --json conclusion \
        --jq '.[0].conclusion')"
      run_url="$(gh run list \
        --repo "$repo_path" \
        --workflow "$ci_workflow_file" \
        --event push \
        --commit "$head_sha" \
        --limit 20 \
        --json url \
        --jq '.[0].url')"

      echo "ci workflow run: id=${run_id:-unknown} status=${run_status:-unknown} conclusion=${run_conclusion:-pending}"

      if [[ "$run_status" == "completed" ]]; then
        if [[ "$run_conclusion" == "success" ]]; then
          echo "ok: CI workflow succeeded for commit ${head_sha}"
          return 0
        fi
        echo "error: CI workflow failed for commit ${head_sha} (conclusion=${run_conclusion:-unknown})" >&2
        if [[ -n "$run_url" && "$run_url" != "null" ]]; then
          echo "error: failed CI run url: $run_url" >&2
        fi
        return 1
      fi
    else
      echo "waiting: CI workflow run for commit ${head_sha} not found yet"
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if (( elapsed >= max_wait_seconds )); then
      echo "error: timed out waiting for CI workflow (elapsed=${elapsed}s, limit=${max_wait_seconds}s)" >&2
      return 124
    fi

    sleep "$poll_seconds"
  done
}

ensure_release_workflow_trigger() {
  local workflow_file=".github/workflows/release.yml"
  [[ -f "$workflow_file" ]] || fail 3 "missing release workflow: $workflow_file"

  if ! grep -Eq '^[[:space:]]*tags:[[:space:]]*$' "$workflow_file"; then
    fail 3 "release workflow missing 'tags' trigger: $workflow_file"
  fi

  if ! grep -Eq 'v\*' "$workflow_file"; then
    fail 3 "release workflow does not include v* tag pattern: $workflow_file"
  fi
}

extract_version_value() {
  local file="$1"
  awk -F'=' '
    /^[[:space:]]*version[[:space:]]*=/ {
      value=$2
      sub(/^[[:space:]]*/, "", value)
      sub(/[[:space:]]*$/, "", value)
      gsub(/^"|"$/, "", value)
      print value
      exit
    }
  ' "$file"
}

extract_json_version() {
  local file="$1"
  node - "$file" <<'NODE'
const fs = require("fs");
const file = process.argv[2];
const data = JSON.parse(fs.readFileSync(file, "utf8"));
if (typeof data.version === "string") {
  process.stdout.write(data.version);
}
NODE
}

extract_package_lock_versions() {
  local file="$1"
  node - "$file" <<'NODE'
const fs = require("fs");
const file = process.argv[2];
const data = JSON.parse(fs.readFileSync(file, "utf8"));
const topLevel = typeof data.version === "string" ? data.version : "";
const rootPackage =
  data &&
  data.packages &&
  typeof data.packages === "object" &&
  data.packages[""] &&
  typeof data.packages[""] === "object" &&
  typeof data.packages[""].version === "string"
    ? data.packages[""].version
    : "";
process.stdout.write(`${topLevel}|${rootPackage}`);
NODE
}

extract_bangumi_user_agent_placeholder_version() {
  local file="$1"
  sed -n 's#.*<string>nils-bangumi-cli/\([^<]*\)</string>.*#\1#p' "$file" | head -n1
}

set_explicit_version() {
  local file="$1"
  local target_version="$2"
  local tmp_file
  tmp_file="$(mktemp)"

  if ! awk -v target="$target_version" '
    BEGIN { replaced = 0 }
    {
      if (!replaced && $0 ~ /^[[:space:]]*version[[:space:]]*=[[:space:]]*"/) {
        print "version = \"" target "\""
        replaced = 1
      } else {
        print $0
      }
    }
    END {
      if (!replaced) {
        exit 2
      }
    }
  ' "$file" >"$tmp_file"; then
    rm -f "$tmp_file"
    fail 1 "failed to update explicit version field in $file"
  fi

  mv "$tmp_file" "$file"
}

set_package_json_version() {
  local file="$1"
  local target_version="$2"
  if ! node - "$file" "$target_version" <<'NODE'
const fs = require("fs");
const file = process.argv[2];
const target = process.argv[3];
const data = JSON.parse(fs.readFileSync(file, "utf8"));
if (typeof data.version !== "string") {
  process.exit(2);
}
if (data.version !== target) {
  data.version = target;
  fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}
NODE
  then
    fail 1 "failed to update package version field in $file"
  fi
}

set_package_lock_versions() {
  local file="$1"
  local target_version="$2"
  if ! node - "$file" "$target_version" <<'NODE'
const fs = require("fs");
const file = process.argv[2];
const target = process.argv[3];
const data = JSON.parse(fs.readFileSync(file, "utf8"));
let changed = false;

if (typeof data.version === "string" && data.version !== target) {
  data.version = target;
  changed = true;
}
if (data && data.packages && typeof data.packages === "object") {
  const rootPackage = data.packages[""];
  if (rootPackage && typeof rootPackage === "object" && typeof rootPackage.version === "string" && rootPackage.version !== target) {
    rootPackage.version = target;
    changed = true;
  }
}

if (changed) {
  fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}
NODE
  then
    fail 1 "failed to update package lock version fields in $file"
  fi
}

set_bangumi_user_agent_placeholder_version() {
  local file="$1"
  local target_version="$2"
  local tmp_file
  tmp_file="$(mktemp)"

  if ! awk -v target="$target_version" '
    BEGIN { replaced = 0 }
    {
      if (!replaced && $0 ~ /<string>nils-bangumi-cli\/[^<]+<\/string>/) {
        sub(/<string>nils-bangumi-cli\/[^<]+<\/string>/, "<string>nils-bangumi-cli/" target "</string>")
        replaced = 1
      }
      print $0
    }
    END {
      if (!replaced) {
        exit 2
      }
    }
  ' "$file" >"$tmp_file"; then
    rm -f "$tmp_file"
    fail 1 "failed to update Bangumi User-Agent placeholder in $file"
  fi

  mv "$tmp_file" "$file"
}

add_version_target() {
  local file="$1"
  local kind="$2"
  local description="$3"
  VERSION_TARGET_FILES+=("$file")
  VERSION_TARGET_KIND+=("$kind")
  VERSION_TARGET_DESC+=("$description")
}

collect_version_targets() {
  local semver="$1"
  local file current lock_versions lock_top_level lock_root
  VERSION_TARGET_FILES=()
  VERSION_TARGET_KIND=()
  VERSION_TARGET_DESC=()

  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    current="$(extract_version_value "$file")"
    [[ -n "$current" ]] || continue
    if [[ "$current" != "$semver" ]]; then
      add_version_target "$file" "toml" "$file: $current -> $semver"
    fi
  done < <(git ls-files '*Cargo.toml')

  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    current="$(extract_version_value "$file")"
    [[ -n "$current" ]] || continue
    if [[ "$current" != "$semver" ]]; then
      add_version_target "$file" "toml" "$file: $current -> $semver"
    fi
  done < <(
    git ls-files 'workflows/*/workflow.toml' \
      | awk '$0 != "workflows/_template/workflow.toml"'
  )

  if git ls-files --error-unmatch package.json >/dev/null 2>&1 \
    || git ls-files --error-unmatch package-lock.json >/dev/null 2>&1; then
    command -v node >/dev/null 2>&1 || fail 3 "node is required to sync package*.json versions"
  fi

  if git ls-files --error-unmatch package.json >/dev/null 2>&1; then
    current="$(extract_json_version package.json)"
    [[ -n "$current" ]] || fail 1 "package.json is missing a string version field"
    if [[ "$current" != "$semver" ]]; then
      add_version_target "package.json" "package-json" "package.json: $current -> $semver"
    fi
  fi

  if git ls-files --error-unmatch package-lock.json >/dev/null 2>&1; then
    lock_versions="$(extract_package_lock_versions package-lock.json)"
    lock_top_level="${lock_versions%%|*}"
    lock_root="${lock_versions#*|}"
    if [[ "$lock_top_level" != "$semver" || "$lock_root" != "$semver" ]]; then
      add_version_target \
        "package-lock.json" \
        "package-lock" \
        "package-lock.json: version=${lock_top_level:-<missing>}, packages[\"\"].version=${lock_root:-<missing>} -> $semver"
    fi
  fi

  local bangumi_info_file="workflows/bangumi-search/src/info.plist.template"
  if git ls-files --error-unmatch "$bangumi_info_file" >/dev/null 2>&1; then
    current="$(extract_bangumi_user_agent_placeholder_version "$bangumi_info_file")"
    [[ -n "$current" ]] || fail 1 "${bangumi_info_file} is missing nils-bangumi-cli/<version> placeholder"
    if [[ "$current" != "$semver" ]]; then
      add_version_target \
        "$bangumi_info_file" \
        "bangumi-user-agent" \
        "${bangumi_info_file}: BANGUMI_USER_AGENT placeholder nils-bangumi-cli/${current} -> nils-bangumi-cli/${semver}"
    fi
  fi
}

refresh_cargo_lock_if_present() {
  local semver="$1"
  local lock_file="Cargo.lock"

  if ! git ls-files --error-unmatch "$lock_file" >/dev/null 2>&1; then
    return 0
  fi

  command -v cargo >/dev/null 2>&1 || fail 3 "cargo is required to refresh Cargo.lock"
  cargo update --workspace >/dev/null

  if ! git diff --quiet -- "$lock_file"; then
    add_version_target "$lock_file" "cargo-lock" "$lock_file: sync workspace package versions to $semver"
  fi
}

refresh_third_party_licenses_if_present() {
  local generator_script="scripts/generate-third-party-licenses.sh"
  local artifact_file="THIRD_PARTY_LICENSES.md"

  if ! git ls-files --error-unmatch "$artifact_file" >/dev/null 2>&1; then
    return 0
  fi

  [[ -f "$generator_script" ]] \
    || fail 3 "tracked $artifact_file requires generator script: $generator_script"

  bash "$generator_script" --write >/dev/null

  if ! git diff --quiet -- "$artifact_file"; then
    add_version_target \
      "$artifact_file" \
      "third-party-licenses" \
      "$artifact_file: refresh generated artifact for release inputs"
  fi
}

ensure_upstream_ready() {
  local remote="$1"
  local upstream_ref counts behind_count ahead_count upstream_remote

  upstream_ref="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
  [[ -n "$upstream_ref" ]] || fail 3 "current branch has no upstream; set upstream before release"

  upstream_remote="${upstream_ref%%/*}"
  [[ "$upstream_remote" == "$remote" ]] \
    || fail 3 "current upstream remote is '$upstream_remote' (expected '$remote')"

  counts="$(git rev-list --left-right --count "${upstream_ref}...HEAD")"
  read -r behind_count ahead_count <<<"$counts"
  if [[ -z "$behind_count" || -z "$ahead_count" ]]; then
    fail 3 "failed to parse ahead/behind counts for ${upstream_ref}"
  fi

  if (( behind_count != 0 )); then
    fail 3 "local branch is behind ${upstream_ref}; pull/rebase before release"
  fi

  RELEASE_UPSTREAM_BRANCH="${upstream_ref#*/}"
}

remote="origin"
dry_run=0
version=""
poll_seconds=15
max_wait_seconds=1800
force_tag=0

while [[ $# -gt 0 ]]; do
  case "${1:-}" in
    --remote)
      remote="${2:-}"
      [[ -n "$remote" ]] || fail 2 "--remote requires a value"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --force-tag)
      force_tag=1
      shift
      ;;
    --poll-seconds)
      poll_seconds="${2:-}"
      [[ -n "$poll_seconds" ]] || fail 2 "--poll-seconds requires a value"
      shift 2
      ;;
    --max-wait-seconds)
      max_wait_seconds="${2:-}"
      [[ -n "$max_wait_seconds" ]] || fail 2 "--max-wait-seconds requires a value"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -z "$version" ]]; then
        version="$1"
        shift
      else
        fail 2 "unknown argument: ${1:-}"
      fi
      ;;
  esac
done

[[ -n "$version" ]] || {
  usage >&2
  exit 2
}

[[ "$version" =~ ^v[0-9]+(\.[0-9]+){2}([-.][0-9A-Za-z.-]+)?$ ]] \
  || fail 2 "invalid version '$version' (expected like v0.1.0)"
parse_positive_int "--poll-seconds" "$poll_seconds"
parse_positive_int "--max-wait-seconds" "$max_wait_seconds"

semver="${version#v}"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail 3 "not inside a git repository"

ensure_release_workflow_trigger

if [[ -n "$(git status --porcelain)" ]]; then
  fail 3 "working tree is not clean; commit or stash changes first"
fi

remote_url="$(git remote get-url "$remote" 2>/dev/null || true)"
[[ -n "$remote_url" ]] || fail 3 "remote '$remote' is not configured"
ensure_upstream_ready "$remote"

github_repo_path="$(to_github_repo_path "$remote_url" || true)"
if [[ -n "$github_repo_path" && "$dry_run" -eq 0 ]]; then
  ensure_github_cli_ready
fi

if [[ "$force_tag" -eq 0 ]]; then
  if git rev-parse -q --verify "refs/tags/${version}" >/dev/null; then
    fail 3 "tag already exists locally: $version"
  fi

  if git ls-remote --exit-code --tags "$remote" "refs/tags/${version}" >/dev/null 2>&1; then
    fail 3 "tag already exists on remote '$remote': $version"
  fi
else
  echo "note: force-tag enabled; existing local/remote tag '${version}' will be overwritten"
fi

collect_version_targets "$semver"

echo "release workflow: .github/workflows/release.yml"
echo "remote: $remote ($remote_url)"
echo "tag version: $version"
echo "package version: $semver"
if [[ "${#VERSION_TARGET_DESC[@]}" -gt 0 ]]; then
  echo "version sync targets:"
  printf '  - %s\n' "${VERSION_TARGET_DESC[@]}"
else
  echo "version sync targets: already up to date"
fi

if [[ "$dry_run" -eq 1 ]]; then
  dry_run_tag_mode_note=""
  if [[ "$force_tag" -eq 1 ]]; then
    dry_run_tag_mode_note=" (force update when existing)"
  fi
  if [[ -n "$github_repo_path" ]]; then
    echo "dry-run: would sync versions, commit/push if needed, create/push tag${dry_run_tag_mode_note}, wait release workflow success, and wait release page"
  else
    echo "dry-run: would sync versions, commit/push if needed, then create and push tag${dry_run_tag_mode_note}"
  fi
  exit 0
fi

if [[ "${#VERSION_TARGET_FILES[@]}" -gt 0 ]]; then
  for idx in "${!VERSION_TARGET_FILES[@]}"; do
    target_file="${VERSION_TARGET_FILES[$idx]}"
    target_kind="${VERSION_TARGET_KIND[$idx]}"
    case "$target_kind" in
      toml)
        set_explicit_version "$target_file" "$semver"
        ;;
      package-json)
        set_package_json_version "$target_file" "$semver"
        ;;
      package-lock)
        set_package_lock_versions "$target_file" "$semver"
        ;;
      bangumi-user-agent)
        set_bangumi_user_agent_placeholder_version "$target_file" "$semver"
        ;;
      cargo-lock|third-party-licenses)
        ;;
      *)
        fail 1 "unsupported version target kind '$target_kind' for $target_file"
        ;;
    esac
  done
fi

refresh_cargo_lock_if_present "$semver"
refresh_third_party_licenses_if_present

if [[ "${#VERSION_TARGET_FILES[@]}" -gt 0 ]]; then
  if ! command -v semantic-commit >/dev/null 2>&1; then
    fail 3 "semantic-commit is required to commit version bump changes"
  fi

  git add "${VERSION_TARGET_FILES[@]}"
  cat <<EOF | semantic-commit commit
chore(release): bump version to ${semver}

- Sync Cargo, workflow, package, and Bangumi UA placeholder versions to ${semver}.
- Refresh Cargo.lock workspace package versions when present.
- Regenerate THIRD_PARTY_LICENSES.md when tracked.
EOF

  git push "$remote" "HEAD:${RELEASE_UPSTREAM_BRANCH}"
  echo "ok: pushed version bump commit to $remote/${RELEASE_UPSTREAM_BRANCH}"
fi

if [[ -n "$github_repo_path" ]]; then
  release_head_sha="$(git rev-parse HEAD)"
  wait_for_ci_workflow_success_for_sha "$github_repo_path" "$release_head_sha" "$poll_seconds" "$max_wait_seconds"
fi

if [[ "$force_tag" -eq 1 ]]; then
  git tag -a -f "$version" -m "Release $version"
  git push --force "$remote" "refs/tags/${version}"
else
  git tag -a "$version" -m "Release $version"
  git push "$remote" "refs/tags/${version}"
fi

echo "ok: pushed tag $version to $remote"
if [[ -n "$github_repo_path" ]]; then
  wait_for_release_workflow_success "$github_repo_path" "$version" "$poll_seconds" "$max_wait_seconds"
  wait_for_release_page "$github_repo_path" "$version" "$poll_seconds" "$max_wait_seconds"
elif release_url="$(to_release_url "$remote_url" "$version")"; then
  echo "release page: $release_url"
fi

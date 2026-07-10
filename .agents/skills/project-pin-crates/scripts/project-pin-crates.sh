#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_root="$(cd "${script_dir}/.." && pwd)"
repo_root="$(cd "${skill_root}/../../.." && pwd)"

version=""
targets_raw=""
use_all=0
dry_run=0
update_lock=0
list_targets=0
auto_commit=0
auto_push=0
push_remote="origin"
push_remote_explicit=0
commit_status="not-requested"
push_status="not-requested"
version_check_status="not-run"

declare -a selected_targets=()
declare -a changed_files=()
declare -a lock_crates=()

usage() {
  cat <<'USAGE'
Usage:
  <ENTRYPOINT> --version <x.y.z> [--targets <target[,target...]>|--all] [--dry-run] [--update-lock] [--auto-commit] [--auto-push] [--push-remote <remote>]
  <ENTRYPOINT> --list-targets
  <ENTRYPOINT> --help

Options:
  --version <ver>         Exact version to pin (example: 0.3.7)
  --targets <list>        Comma-separated target aliases (example: codex-cli,memo)
  --all                   Pin all managed targets (default if --targets omitted)
  --dry-run               Print planned changes without writing files
  --update-lock           Run cargo update --precise for cargo-managed targets
  --auto-commit           Stage touched files and create a semantic commit after pinning
  --auto-push             Push current branch after auto-commit (implies --auto-commit)
  --push-remote <name>    Remote to use with --auto-push (default: origin)
  (always validates selected crate versions on crates.io before file edits)
  --list-targets          Print supported targets and aliases
  -h, --help              Show this help
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

usage_error() {
  echo "error: $*" >&2
  usage >&2
  exit 2
}

target_crate() {
  case "$1" in
    codex-cli) printf '%s\n' "nils-codex-cli" ;;
    memo) printf '%s\n' "nils-memo" ;;
    *) return 1 ;;
  esac
}

canonical_target() {
  case "$1" in
    codex-cli|codex|nils-codex-cli) printf '%s\n' "codex-cli" ;;
    memo|nils-memo) printf '%s\n' "memo" ;;
    *) return 1 ;;
  esac
}

print_targets() {
  cat <<'TARGETS'
codex-cli
  aliases: codex-cli, codex, nils-codex-cli
  release_source: sympoies/nils-cli GitHub release
  kind: workflow release-bundle runtime pin + docs
memo
  aliases: memo, nils-memo
  published_crate: nils-memo
  kind: cargo dependency pin + docs
TARGETS
}

assert_file() {
  local file="$1"
  [[ -f "$file" ]] || die "missing required file: $file"
}

record_changed_file() {
  local file="$1"
  local existing
  for existing in "${changed_files[@]:-}"; do
    [[ "$existing" == "$file" ]] && return 0
  done
  changed_files+=("$file")
}

record_lock_crate() {
  local crate="$1"
  local existing
  for existing in "${lock_crates[@]:-}"; do
    [[ "$existing" == "$crate" ]] && return 0
  done
  lock_crates+=("$crate")
}

replace_in_file() {
  local file="$1"
  local pattern="$2"
  local replacement="$3"
  local label="$4"
  assert_file "$file"

  if [[ "$dry_run" -eq 1 ]]; then
    if ! python3 - "$file" "$pattern" <<'PY'
import pathlib
import re
import sys

path, pattern = sys.argv[1], sys.argv[2]
text = pathlib.Path(path).read_text()
sys.exit(0 if re.search(pattern, text, flags=re.MULTILINE | re.DOTALL) else 1)
PY
    then
      die "pattern not found for ${label}: $file"
    fi
    echo "dry-run: ${label} -> $file"
    record_changed_file "$file"
    return 0
  fi

  if ! python3 - "$file" "$pattern" "$replacement" <<'PY'
import pathlib
import re
import sys

path, pattern, replacement = sys.argv[1], sys.argv[2], sys.argv[3]
file_path = pathlib.Path(path)
text = file_path.read_text()
if not re.search(pattern, text, flags=re.MULTILINE | re.DOTALL):
    sys.exit(1)
updated = re.sub(pattern, lambda _m: replacement, text, flags=re.MULTILINE | re.DOTALL)
file_path.write_text(updated)
PY
  then
    die "pattern not found for ${label}: $file"
  fi
  echo "updated: ${label} -> $file"
  record_changed_file "$file"
}

pin_codex_cli() {
  local runtime_file="$repo_root/workflows/codex-cli/scripts/lib/codex_cli_runtime.sh"
  local canonical_version_file="$repo_root/scripts/lib/codex_cli_version.sh"
  local readme_file="$repo_root/workflows/codex-cli/README.md"
  local plist_file="$repo_root/workflows/codex-cli/src/info.plist.template"

  replace_in_file \
    "$canonical_version_file" \
    'if \[\[ -z "\$\{CODEX_CLI_VERSION:-\}" \]\]; then\s+CODEX_CLI_VERSION="[^"]+"' \
    "if [[ -z \"\${CODEX_CLI_VERSION:-}\" ]]; then
  CODEX_CLI_VERSION=\"${version}\"" \
    "codex canonical version pin"

  replace_in_file \
    "$runtime_file" \
    'if \[\[ -z "\$\{CODEX_CLI_VERSION:-\}" \]\]; then\s+CODEX_CLI_VERSION="[^"]+"' \
    "if [[ -z \"\${CODEX_CLI_VERSION:-}\" ]]; then
    CODEX_CLI_VERSION=\"${version}\"" \
    "codex runtime fallback version pin"

  replace_in_file \
    "$readme_file" \
    'codex-cli[@.][0-9A-Za-z.+-]+' \
    "codex-cli@${version}" \
    "codex readme runtime pin"

  replace_in_file \
    "$plist_file" \
    'nils-cli v[0-9A-Za-z.+-]+ release binary' \
    "nils-cli v${version} release binary" \
    "codex plist release pin"
}

pin_memo() {
  local cargo_file="$repo_root/crates/memo-workflow-cli/Cargo.toml"
  local crate_readme_file="$repo_root/crates/memo-workflow-cli/README.md"
  local workflow_readme_file="$repo_root/workflows/memo-add/README.md"
  local workflow_guide_file="$repo_root/docs/WORKFLOW_GUIDE.md"
  local workflow_contract_file="$repo_root/crates/memo-workflow-cli/docs/workflow-contract.md"

  replace_in_file \
    "$cargo_file" \
    'nils-memo = "=[^"]+"' \
    "nils-memo = \"=${version}\"" \
    "memo cargo dependency pin"

  replace_in_file \
    "$crate_readme_file" \
    'nils-memo@[0-9A-Za-z.+-]+' \
    "nils-memo@${version}" \
    "memo crate readme pin"

  replace_in_file \
    "$workflow_readme_file" \
    'nils-memo@[0-9A-Za-z.+-]+' \
    "nils-memo@${version}" \
    "memo workflow readme pin"

  if [[ -f "$workflow_guide_file" ]]; then
    replace_in_file \
      "$workflow_guide_file" \
      'nils-memo@[0-9A-Za-z.+-]+' \
      "nils-memo@${version}" \
      "memo workflow guide pin"
  else
    echo "note: optional file missing, skipped memo workflow guide pin: $workflow_guide_file"
  fi

  replace_in_file \
    "$workflow_contract_file" \
    'nils-memo@[0-9A-Za-z.+-]+' \
    "nils-memo@${version}" \
    "memo workflow contract pin"

  record_lock_crate "nils-memo"
}

resolve_targets() {
  declare -A dedup=()
  local token canonical
  if [[ "$use_all" -eq 1 || -z "$targets_raw" ]]; then
    selected_targets=("codex-cli" "memo")
    return 0
  fi

  IFS=',' read -r -a raw_tokens <<<"$targets_raw"
  for token in "${raw_tokens[@]}"; do
    token="${token//[[:space:]]/}"
    [[ -n "$token" ]] || continue
    canonical="$(canonical_target "$token")" || usage_error "unknown target alias: $token"
    if [[ -z "${dedup[$canonical]:-}" ]]; then
      dedup["$canonical"]=1
      selected_targets+=("$canonical")
    fi
  done

  [[ "${#selected_targets[@]}" -gt 0 ]] || usage_error "no valid targets resolved"
}

check_crate_version_exists() {
  local crate="$1"
  local status=0
  python3 - "$crate" "$version" <<'PY' || status="$?"
import json
import sys
import urllib.error
import urllib.request

crate, version = sys.argv[1], sys.argv[2]
url = f"https://crates.io/api/v1/crates/{crate}/{version}"
request = urllib.request.Request(
    url,
    headers={"User-Agent": "project-pin-crates/1.0"},
)
try:
    with urllib.request.urlopen(request, timeout=20) as response:
        if response.status != 200:
            sys.exit(4)
        data = json.load(response)
except urllib.error.HTTPError as exc:
    if exc.code == 404:
        sys.exit(3)
    sys.exit(4)
except Exception:
    sys.exit(4)

remote_version = (data.get("version") or {}).get("num")
if remote_version != version:
    sys.exit(5)
PY
  if [[ "$status" -eq 0 ]]; then
    echo "verified: crates.io has ${crate}@${version}"
    return 0
  fi

  case "$status" in
    3) die "crate version not found on crates.io: ${crate}@${version}" ;;
    4) die "failed to query crates.io for ${crate}@${version}" ;;
    5) die "crates.io version mismatch for ${crate}@${version}" ;;
    *) die "version verification failed for ${crate}@${version}" ;;
  esac
}

check_nils_cli_release_exists() {
  local status=0
  python3 - "$version" <<'PY' || status="$?"
import json
import sys
import urllib.error
import urllib.request

version = sys.argv[1]
url = f"https://api.github.com/repos/sympoies/nils-cli/releases/tags/v{version}"
request = urllib.request.Request(
    url,
    headers={
        "Accept": "application/vnd.github+json",
        "User-Agent": "project-pin-crates/1.0",
    },
)
try:
    with urllib.request.urlopen(request, timeout=20) as response:
        if response.status != 200:
            sys.exit(4)
        data = json.load(response)
except urllib.error.HTTPError as exc:
    if exc.code == 404:
        sys.exit(3)
    sys.exit(4)
except Exception:
    sys.exit(4)

expected = f"nils-cli-v{version}-aarch64-apple-darwin.tar.gz"
assets = {asset.get("name") for asset in data.get("assets", [])}
if data.get("tag_name") != f"v{version}" or expected not in assets:
    sys.exit(5)
PY
  if [[ "$status" -eq 0 ]]; then
    echo "verified: GitHub release has sympoies/nils-cli@v${version} macOS arm64 asset"
    return 0
  fi

  case "$status" in
    3) die "nils-cli release not found on GitHub: v${version}" ;;
    4) die "failed to query GitHub release: sympoies/nils-cli@v${version}" ;;
    5) die "nils-cli release is missing the macOS arm64 bundle: v${version}" ;;
    *) die "release verification failed for sympoies/nils-cli@v${version}" ;;
  esac
}

verify_target_versions_available() {
  declare -A dedup=()
  local target crate
  for target in "${selected_targets[@]}"; do
    case "$target" in
      codex-cli)
        if [[ -z "${dedup[nils-cli-release]:-}" ]]; then
          dedup[nils-cli-release]=1
          check_nils_cli_release_exists
        fi
        ;;
      *)
        crate="$(target_crate "$target")" || die "failed to resolve crate for target: $target"
        if [[ -z "${dedup[$crate]:-}" ]]; then
          dedup["$crate"]=1
          check_crate_version_exists "$crate"
        fi
        ;;
    esac
  done
  version_check_status="verified"
}

run_lock_sync() {
  local crate
  [[ "$update_lock" -eq 1 ]] || return 0
  [[ "${#lock_crates[@]}" -gt 0 ]] || return 0

  if [[ "$dry_run" -eq 1 ]]; then
    for crate in "${lock_crates[@]}"; do
      echo "dry-run: cargo update -p ${crate} --precise ${version}"
    done
    return 0
  fi

  command -v cargo >/dev/null 2>&1 || die "cargo is required when --update-lock is enabled"
  for crate in "${lock_crates[@]}"; do
    echo "running: cargo update -p ${crate} --precise ${version}"
    cargo update -p "$crate" --precise "$version"
  done
}

build_commit_message() {
  printf 'chore(workflows): pin managed crates to %s\n' "$version"
}

run_auto_commit_and_push() {
  local current_branch commit_message before_head after_head
  [[ "$auto_commit" -eq 1 ]] || return 0

  if [[ "${#changed_files[@]}" -eq 0 ]]; then
    echo "note: no touched files were recorded; skipped auto-commit"
    commit_status="skipped-no-files"
    if [[ "$auto_push" -eq 1 ]]; then
      echo "note: skipped auto-push because no commit was created"
      push_status="skipped-no-commit"
    fi
    return 0
  fi

  if [[ "$dry_run" -eq 1 ]]; then
    echo "dry-run: would stage touched files for auto-commit"
    echo "dry-run: would run semantic-commit commit with message: $(build_commit_message)"
    commit_status="dry-run"
    if [[ "$auto_push" -eq 1 ]]; then
      current_branch="$(git -C "$repo_root" rev-parse --abbrev-ref HEAD)"
      [[ "$current_branch" != "HEAD" ]] || die "auto-push requires a branch checkout (detached HEAD is not supported)"
      echo "dry-run: would run git push ${push_remote} HEAD:${current_branch}"
      push_status="dry-run"
    fi
    return 0
  fi

  command -v semantic-commit >/dev/null 2>&1 || die "semantic-commit is required when --auto-commit is enabled"

  if ! git -C "$repo_root" diff --cached --quiet --ignore-submodules --; then
    die "auto-commit requires an empty staged index before running"
  fi

  if git -C "$repo_root" diff --quiet --ignore-submodules -- "${changed_files[@]}"; then
    echo "note: selected targets are already pinned to ${version}; skipped auto-commit"
    commit_status="skipped-no-diff"
    if [[ "$auto_push" -eq 1 ]]; then
      echo "note: skipped auto-push because no commit was created"
      push_status="skipped-no-commit"
    fi
    return 0
  fi

  git -C "$repo_root" add -- "${changed_files[@]}"
  if git -C "$repo_root" diff --cached --quiet --ignore-submodules --; then
    echo "note: no staged delta after pin updates; skipped auto-commit"
    commit_status="skipped-no-diff"
    if [[ "$auto_push" -eq 1 ]]; then
      echo "note: skipped auto-push because no commit was created"
      push_status="skipped-no-commit"
    fi
    return 0
  fi

  before_head="$(git -C "$repo_root" rev-parse --verify HEAD)"
  commit_message="$(build_commit_message)"
  printf '%s\n' "$commit_message" | semantic-commit commit
  after_head="$(git -C "$repo_root" rev-parse --verify HEAD)"
  [[ "$after_head" != "$before_head" ]] || die "auto-commit requested but no commit was created"
  commit_status="created:${after_head:0:7}"

  if [[ "$auto_push" -eq 1 ]]; then
    current_branch="$(git -C "$repo_root" rev-parse --abbrev-ref HEAD)"
    [[ "$current_branch" != "HEAD" ]] || die "auto-push requires a branch checkout (detached HEAD is not supported)"
    git -C "$repo_root" push "$push_remote" "HEAD:${current_branch}"
    push_status="pushed:${push_remote}/${current_branch}"
  fi
}

print_summary() {
  local target
  echo "summary:"
  echo "  version: ${version}"
  echo "  dry_run: ${dry_run}"
  echo "  update_lock: ${update_lock}"
  echo "  version_check: ${version_check_status}"
  echo "  auto_commit: ${auto_commit}"
  echo "  auto_push: ${auto_push}"
  if [[ "$auto_push" -eq 1 ]]; then
    echo "  push_remote: ${push_remote}"
  fi
  echo "  commit_status: ${commit_status}"
  echo "  push_status: ${push_status}"
  echo "  targets:"
  for target in "${selected_targets[@]}"; do
    if [[ "$target" == codex-cli ]]; then
      echo "    - ${target} (release: sympoies/nils-cli)"
    else
      echo "    - ${target} (crate: $(target_crate "$target"))"
    fi
  done
  echo "  files:"
  for target in "${changed_files[@]:-}"; do
    echo "    - ${target}"
  done
}

while [[ $# -gt 0 ]]; do
  case "${1:-}" in
    --version)
      [[ $# -ge 2 ]] || usage_error "--version requires a value"
      version="$2"
      shift 2
      ;;
    --targets)
      [[ $# -ge 2 ]] || usage_error "--targets requires a value"
      targets_raw="$2"
      shift 2
      ;;
    --all)
      use_all=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --update-lock)
      update_lock=1
      shift
      ;;
    --auto-commit)
      auto_commit=1
      shift
      ;;
    --auto-push)
      auto_push=1
      shift
      ;;
    --push-remote)
      [[ $# -ge 2 ]] || usage_error "--push-remote requires a value"
      push_remote="$2"
      push_remote_explicit=1
      shift 2
      ;;
    --list-targets)
      list_targets=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage_error "unknown argument: ${1:-}"
      ;;
  esac
done

if [[ "$list_targets" -eq 1 ]]; then
  print_targets
  exit 0
fi

[[ -n "$version" ]] || usage_error "--version is required unless --list-targets is used"
if [[ "$version" != *[![:space:]]* ]]; then
  usage_error "--version cannot be empty"
fi
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]]; then
  usage_error "invalid version format: $version"
fi
if [[ "$use_all" -eq 1 && -n "$targets_raw" ]]; then
  usage_error "--all and --targets cannot be used together"
fi
if [[ "$auto_push" -eq 1 ]]; then
  auto_commit=1
fi
if [[ "$push_remote_explicit" -eq 1 && "$auto_push" -ne 1 ]]; then
  usage_error "--push-remote requires --auto-push"
fi

git -C "$repo_root" rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "repo root is not a git work tree: $repo_root"
resolve_targets
verify_target_versions_available

for target in "${selected_targets[@]}"; do
  case "$target" in
    codex-cli) pin_codex_cli ;;
    memo) pin_memo ;;
    *) usage_error "unsupported target: $target" ;;
  esac
done

run_lock_sync
run_auto_commit_and_push
print_summary

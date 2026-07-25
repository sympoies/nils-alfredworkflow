#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/ci-workflow-audit.sh --check
USAGE
}

mode="check"
if [[ $# -eq 0 ]]; then
  mode="check"
else
  while [[ $# -gt 0 ]]; do
    case "$1" in
    --check)
      mode="check"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
    esac
  done
fi

if [[ "$mode" != "check" ]]; then
  echo "error: unsupported mode: $mode" >&2
  exit 2
fi

ci_workflow="$repo_root/.github/workflows/ci.yml"
release_workflow="$repo_root/.github/workflows/release.yml"
publish_workflow="$repo_root/.github/workflows/publish-crates.yml"
bootstrap_script="$repo_root/scripts/ci/ci-bootstrap.sh"
packaging_doc="$repo_root/docs/PACKAGING.md"

for workflow_file in "$ci_workflow" "$release_workflow" "$publish_workflow"; do
  [[ -f "$workflow_file" ]] || {
    echo "error: missing workflow file: $workflow_file" >&2
    exit 1
  }
done

[[ -f "$bootstrap_script" ]] || {
  echo "error: missing CI bootstrap: $bootstrap_script" >&2
  exit 1
}
[[ -f "$packaging_doc" ]] || {
  echo "error: missing packaging policy: $packaging_doc" >&2
  exit 1
}

failures=0

record_failure() {
  local message="$1"
  echo "error: $message" >&2
  failures=$((failures + 1))
}

require_fixed() {
  local file="$1"
  local needle="$2"
  local label="$3"
  local hint="$4"

  if ! rg -n --fixed-strings "$needle" "$file" >/dev/null 2>&1; then
    record_failure "$label not found in ${file#"$repo_root"/}"
    echo "hint: $hint" >&2
  fi
}

reject_fixed() {
  local file="$1"
  local needle="$2"
  local label="$3"
  local hint="$4"
  local matches=""

  matches="$(rg -n --fixed-strings "$needle" "$file" || true)"
  if [[ -n "$matches" ]]; then
    record_failure "$label found in ${file#"$repo_root"/}"
    while IFS= read -r line; do
      echo "  match: $line" >&2
    done <<<"$matches"
    echo "hint: $hint" >&2
  fi
}

reject_regex() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  local hint="$4"
  local matches=""

  matches="$(rg -n "$pattern" "$file" || true)"
  if [[ -n "$matches" ]]; then
    record_failure "$label found in ${file#"$repo_root"/}"
    while IFS= read -r line; do
      echo "  match: $line" >&2
    done <<<"$matches"
    echo "hint: $hint" >&2
  fi
}

require_step_github_token() {
  local file="$1"
  local step_name="$2"
  local label="$3"

  if ! python3 - "$file" "$step_name" <<'PY'; then
from pathlib import Path
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
needle = f"      - name: {sys.argv[2]}"
try:
    start = lines.index(needle)
except ValueError:
    raise SystemExit(1)
end = len(lines)
for index in range(start + 1, len(lines)):
    if lines[index].startswith("      - name: "):
        end = index
        break
block = lines[start:end]
for index, line in enumerate(block[:-1]):
    if line == "        env:" and block[index + 1] == "          GITHUB_TOKEN: ${{ github.token }}":
        raise SystemExit(0)
raise SystemExit(1)
PY
    record_failure "$label not found in ${file#"$repo_root"/}"
    echo "hint: Pass github.token only to every step that can invoke the runtime metadata generator." >&2
  fi
}

workflow_run_commands() {
  local file="$1"

  awk '
    function trim(value) {
      sub(/^[[:space:]]+/, "", value)
      sub(/[[:space:]]+$/, "", value)
      return value
    }

    {
      line = $0
      if (in_block) {
        if (line ~ /^[[:space:]]*$/) {
          next
        }
        match(line, /^[[:space:]]*/)
        current_indent = RLENGTH
        if (current_indent > block_indent) {
          command = trim(substr(line, current_indent + 1))
          if (command !~ /^#/) {
            print command
          }
          next
        }
        in_block = 0
      }

      if (line ~ /^[[:space:]]*#/) {
        next
      }
      match(line, /^[[:space:]]*/)
      run_indent = RLENGTH
      rest = substr(line, run_indent + 1)
      sub(/^-[[:space:]]+/, "", rest)
      if (rest !~ /^run:[[:space:]]*/) {
        next
      }
      sub(/^run:[[:space:]]*/, "", rest)
      rest = trim(rest)
      if (rest ~ /^[|>][-+0-9]*$/) {
        in_block = 1
        block_indent = run_indent
      } else if (rest != "") {
        print rest
      }
    }
  ' "$file"
}

require_run_exact() {
  local commands="$1"
  local needle="$2"
  local label="$3"
  local hint="$4"
  local count=0

  while IFS= read -r command; do
    if [[ "$command" == "$needle" ]]; then
      count=$((count + 1))
    fi
  done <<<"$commands"

  if [[ "$count" -ne 1 ]]; then
    record_failure "$label must appear exactly once as an executable run command (found $count)"
    echo "hint: $hint" >&2
  fi
}

reject_run_regex() {
  local commands="$1"
  local pattern="$2"
  local label="$3"
  local hint="$4"
  local matches=""

  matches="$(printf '%s\n' "$commands" | rg -n "$pattern" || true)"
  if [[ -n "$matches" ]]; then
    record_failure "$label found in executable workflow commands"
    while IFS= read -r line; do
      echo "  match: $line" >&2
    done <<<"$matches"
    echo "hint: $hint" >&2
  fi
}

ci_run_commands="$(workflow_run_commands "$ci_workflow")"

# Canonical gate routing must remain shared-script driven.
require_fixed \
  "$ci_workflow" \
  "run: bash scripts/ci/ci-bootstrap.sh --context ci --install-codex-cli" \
  "ci bootstrap entrypoint" \
  "Route CI bootstrap through scripts/ci/ci-bootstrap.sh."
require_step_github_token "$ci_workflow" \
  "Validate project" \
  "ci project validation runtime metadata GitHub token"
require_step_github_token "$release_workflow" \
  "Run release package gates" \
  "release package runtime metadata GitHub token"
require_step_github_token "$release_workflow" \
  "Regenerate third-party artifacts" \
  "release matrix runtime metadata GitHub token"
require_run_exact \
  "$ci_run_commands" \
  "bash scripts/local-pre-commit.sh --mode ci" \
  "ci project validation entrypoint" \
  "Route the ordinary CI gates through the repository-owned local-pre-commit wrapper."
require_fixed \
  "$ci_workflow" \
  "run: bash scripts/ci/ci-run-gates.sh package-smoke --skip-arch-check" \
  "ci package smoke gate entrypoint" \
  "Route package smoke checks through ci-run-gates package-smoke."

require_fixed \
  "$release_workflow" \
  "run: bash scripts/ci/ci-bootstrap.sh --context release --install-codex-cli" \
  "release bootstrap entrypoint" \
  "Route release bootstrap through scripts/ci/ci-bootstrap.sh."
require_fixed \
  "$release_workflow" \
  "run: bash scripts/ci/ci-run-gates.sh release-package --tag \"\${GITHUB_REF_NAME}\"" \
  "release gate entrypoint" \
  "Route release package flow through scripts/ci/ci-run-gates.sh release-package."

require_fixed \
  "$publish_workflow" \
  "run: bash scripts/ci/ci-bootstrap.sh --context publish-crates" \
  "publish bootstrap entrypoint" \
  "Route publish bootstrap through scripts/ci/ci-bootstrap.sh."

require_fixed \
  "$publish_workflow" \
  "bash scripts/ci/ci-run-gates.sh publish-crates" \
  "publish crates gate entrypoint" \
  "Publish workflow must invoke scripts/ci/ci-run-gates.sh publish-crates."

# Block workflow bypasses that skip canonical shared gates.
reject_fixed \
  "$ci_workflow" \
  "run: scripts/workflow-lint.sh" \
  "legacy direct lint workflow call" \
  "Use ci-run-gates lint instead of direct scripts/workflow-lint.sh."
reject_fixed \
  "$ci_workflow" \
  "run: scripts/workflow-test.sh" \
  "legacy direct test workflow call" \
  "Use ci-run-gates test instead of direct scripts/workflow-test.sh."
reject_fixed \
  "$ci_workflow" \
  "run: scripts/workflow-pack.sh --all" \
  "legacy direct package workflow call" \
  "Use ci-run-gates package-smoke instead of direct scripts/workflow-pack.sh."
reject_run_regex \
  "$ci_run_commands" \
  '(^|[;&|[:space:]])bash[[:space:]]+scripts/ci/ci-run-gates\.sh[[:space:]]+(lint|third-party-artifacts-audit|node-scraper-tests|script-tests|test)([;&|[:space:]]|$)' \
  "duplicate direct ordinary gate workflow call" \
  "Route the ordinary gate sequence through scripts/local-pre-commit.sh --mode ci."
reject_fixed \
  "$release_workflow" \
  "run: scripts/workflow-pack.sh --all" \
  "legacy direct release package workflow call" \
  "Use ci-run-gates release-package instead of direct scripts/workflow-pack.sh."
reject_fixed \
  "$publish_workflow" \
  "scripts/publish-crates.sh \"\${args[@]}\"" \
  "legacy direct publish script call" \
  "Use ci-run-gates publish-crates instead of direct scripts/publish-crates.sh."

# Removed stale CI setup/install paths must stay deleted.
reject_fixed \
  "$ci_workflow" \
  "sudo apt-get install -y git jq ripgrep shellcheck shfmt zip unzip" \
  "deprecated apt package install block" \
  "Do not reintroduce inline apt package installation in workflow YAML."
reject_fixed \
  "$ci_workflow" \
  "sudo apt-get update" \
  "deprecated apt update block" \
  "Do not reintroduce inline apt update in workflow YAML."
reject_fixed \
  "$ci_workflow" \
  "source scripts/lib/codex_cli_version.sh" \
  "deprecated ci codex-cli version source block" \
  "Use ci-bootstrap --install-codex-cli instead of inline source + cargo install."
reject_fixed \
  "$release_workflow" \
  "source scripts/lib/codex_cli_version.sh" \
  "deprecated release codex-cli version source block" \
  "Use ci-bootstrap --install-codex-cli instead of inline source + cargo install."

for workflow_file in "$ci_workflow" "$release_workflow" "$publish_workflow"; do
  reject_fixed \
    "$workflow_file" \
    "cargo install \"\${CODEX_CLI_CRATE}\" --version \"\${CODEX_CLI_VERSION}\" --locked" \
    "deprecated inline codex-cli cargo install" \
    "Inline codex-cli install is forbidden; use ci-bootstrap --install-codex-cli."
done

require_fixed \
  "$bootstrap_script" \
  "codex_cli_release_install \"\$target\" \"\$install_root\" \"\$destination\"" \
  "verified codex-cli release bootstrap" \
  "Install codex-cli through the repository-pinned release helper."
reject_regex \
  "$bootstrap_script" \
  'cargo[[:space:]]+install[[:space:]]+.*codex' \
  "retired codex-cli cargo bootstrap" \
  "codex-cli is release-only; do not restore crates.io installation."
require_fixed \
  "$packaging_doc" \
  'Production packaging always downloads the official pinned target archive' \
  "codex-cli official-download packaging policy" \
  "Document production release downloads and repository-pinned verification."
reject_fixed \
  "$packaging_doc" \
  'Its packaging order is explicit local override' \
  "retired codex-cli local/PATH packaging policy" \
  "Do not advertise test-only local or mirror inputs as production resolution steps."

for workflow_file in "$ci_workflow" "$release_workflow" "$publish_workflow"; do
  reject_regex \
    "$workflow_file" \
    '^[[:space:]]*run:[[:space:]]*cargo[[:space:]]+(install|fmt|clippy|test|nextest)\b' \
    "inline cargo gate command in workflow yaml" \
    "Route cargo gates through scripts/ci/ci-bootstrap.sh and scripts/ci/ci-run-gates.sh."
done

if [[ "$failures" -gt 0 ]]; then
  echo "error: ci workflow audit failed with $failures violation(s)." >&2
  exit 1
fi

echo "ok: ci workflow audit passed"

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

workflow_id=""

usage() {
  cat <<USAGE
Usage:
  scripts/workflow-lint.sh [--id <workflow-id>]
USAGE
}

manifest_check() {
  local manifest="$1"
  local required=(id name bundle_id version script_filter action)

  for key in "${required[@]}"; do
    if ! rg -n "^${key}[[:space:]]*=" "$manifest" >/dev/null; then
      echo "error: missing key '$key' in $manifest" >&2
      return 1
    fi
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --id)
    workflow_id="${2:-}"
    [[ -n "$workflow_id" ]] || {
      echo "error: --id requires a value" >&2
      exit 2
    }
    shift 2
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

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
"$repo_root/scripts/cli-standards-audit.sh"
"$repo_root/scripts/docs-placement-audit.sh" --strict
bash "$repo_root/scripts/ci/markdownlint-audit.sh" --strict
bash "$repo_root/scripts/workflow-shared-foundation-audit.sh" --check
bash "$repo_root/scripts/workflow-cli-resolver-audit.sh" --check
bash "$repo_root/scripts/ci/third-party-artifacts-audit.sh" --strict
bash "$repo_root/scripts/ci/ci-workflow-audit.sh" --check

if command -v shellcheck >/dev/null 2>&1; then
  mapfile -t sh_files < <(find "$repo_root/scripts" "$repo_root/workflows" -type f -name '*.sh' | sort)
  if [[ ${#sh_files[@]} -gt 0 ]]; then
    # setup-rust-tooling sources $HOME/.cargo/env dynamically.
    shellcheck -e SC1091 "${sh_files[@]}"
  fi
else
  echo "warn: shellcheck not found; skipping shell lint" >&2
fi

if command -v shfmt >/dev/null 2>&1; then
  mapfile -t sh_files < <(find "$repo_root/scripts" "$repo_root/workflows" -type f -name '*.sh' | sort)
  if [[ ${#sh_files[@]} -gt 0 ]]; then
    shfmt -d "${sh_files[@]}"
  fi
else
  echo "warn: shfmt not found; skipping shell format check" >&2
fi

if [[ -n "$workflow_id" ]]; then
  manifest_check "$repo_root/workflows/$workflow_id/workflow.toml"
else
  while IFS= read -r manifest; do
    manifest_check "$manifest"
  done < <(find "$repo_root/workflows" -mindepth 2 -maxdepth 2 -name workflow.toml | sort)
fi

echo "ok: lint checks passed"

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

workflow_id=""
skip_third_party_audit=0
skip_workspace_tests=0

usage() {
  cat <<USAGE
Usage:
  scripts/workflow-test.sh [--id <workflow-id>] [--skip-third-party-audit] [--skip-workspace-tests]

Notes:
  - By default runs strict third-party artifacts freshness audit before tests.
  - By default runs cargo workspace tests before workflow smoke tests.
  - Runs shellcheck for workflow-local shell scripts before smoke.
USAGE
}

run_workflow_shellcheck() {
  local id="$1"
  local workflow_root="$repo_root/workflows/$id"

  if ! command -v shellcheck >/dev/null 2>&1; then
    echo "error: missing required binary: shellcheck" >&2
    exit 1
  fi

  mapfile -t sh_files < <(find "$workflow_root" -type f -name '*.sh' | sort)
  if [[ ${#sh_files[@]} -eq 0 ]]; then
    return 0
  fi

  # setup-rust-tooling sources $HOME/.cargo/env dynamically in some environments.
  shellcheck -e SC1091 "${sh_files[@]}"
}

run_smoke() {
  local id="$1"
  run_workflow_shellcheck "$id"
  local smoke_script="$repo_root/workflows/$id/tests/smoke.sh"
  if [[ -x "$smoke_script" ]]; then
    "$smoke_script"
  elif [[ -f "$smoke_script" ]]; then
    bash "$smoke_script"
  fi
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
  --skip-third-party-audit)
    skip_third_party_audit=1
    shift
    ;;
  --skip-workspace-tests)
    skip_workspace_tests=1
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

if [[ "$skip_third_party_audit" -eq 0 ]]; then
  echo "== Third-party artifacts audit (strict) =="
  bash "$repo_root/scripts/ci/third-party-artifacts-audit.sh" --strict
else
  echo "== Third-party artifacts audit (strict) =="
  echo "skip: --skip-third-party-audit enabled"
fi

if [[ "$skip_workspace_tests" -eq 0 ]]; then
  cargo test --workspace
else
  echo "== Cargo workspace tests =="
  echo "skip: --skip-workspace-tests enabled"
fi

if [[ -n "$workflow_id" ]]; then
  run_smoke "$workflow_id"
else
  while IFS= read -r id; do
    [[ "$id" == "_template" ]] && continue
    run_smoke "$id"
  done < <(find "$repo_root/workflows" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)
fi

echo "ok: tests passed"

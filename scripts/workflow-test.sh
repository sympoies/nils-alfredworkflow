#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

workflow_id=""

usage() {
  cat <<USAGE
Usage:
  scripts/workflow-test.sh [--id <workflow-id>]

Notes:
  - Runs strict third-party artifacts freshness audit before tests.
USAGE
}

run_smoke() {
  local id="$1"
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

echo "== Third-party artifacts audit (strict) =="
bash "$repo_root/scripts/ci/third-party-artifacts-audit.sh" --strict

cargo test --workspace

if [[ -n "$workflow_id" ]]; then
  run_smoke "$workflow_id"
else
  while IFS= read -r id; do
    [[ "$id" == "_template" ]] && continue
    run_smoke "$id"
  done < <(find "$repo_root/workflows" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)
fi

echo "ok: tests passed"

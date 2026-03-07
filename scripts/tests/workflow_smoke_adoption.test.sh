#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

declare -a missing=()

while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  if ! rg -n 'workflow_smoke_helpers\.sh' "$path" >/dev/null 2>&1; then
    missing+=("$path")
  fi
done < <(find "$repo_root/workflows" -path '*/tests/smoke.sh' -type f | sort)

if [[ "${#missing[@]}" -ne 0 ]]; then
  printf 'FAIL: workflow smoke tests must source scripts/lib/workflow_smoke_helpers.sh\n' >&2
  printf '%s\n' "${missing[@]}" >&2
  exit 1
fi

printf 'PASS: all workflow smoke tests source workflow_smoke_helpers.sh\n'

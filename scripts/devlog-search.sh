#!/usr/bin/env bash
# Search the development log under docs/devlog/.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
devlog_dir="docs/devlog"

usage() {
  cat <<'USAGE'
Usage:
  scripts/devlog-search.sh <term> [YYYY-MM]

Arguments:
  <term>     Case-insensitive literal search string.
  YYYY-MM    Restrict the search to one month file.
USAGE
}

term="${1:-}"
month="${2:-}"

if [[ "$term" == "-h" || "$term" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$#" -gt 2 || -z "$term" ]]; then
  usage >&2
  exit 2
fi

if ! cd "$repo_root"; then
  echo "error: unable to enter the repository root" >&2
  exit 1
fi

declare -a files=()
if [[ -n "$month" ]]; then
  if [[ ! "$month" =~ ^[0-9]{4}-(0[1-9]|1[0-2])$ ]]; then
    usage >&2
    exit 2
  fi
  if [[ ! -e "$devlog_dir/$month.md" ]]; then
    echo "error: no devlog file for $month under $devlog_dir" >&2
    exit 1
  fi
  files=("$devlog_dir/$month.md")
else
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    files+=("$path")
  done < <(find "$devlog_dir" -maxdepth 1 -type f -name '????-??.md' | sort -r)
fi

if [[ "${#files[@]}" -eq 0 || ! -e "${files[0]}" ]]; then
  echo "error: no devlog month files found under $devlog_dir" >&2
  exit 1
fi

if ! grep -n -i -F -- "$term" "${files[@]}"; then
  echo "(no matches for '$term')" >&2
  exit 1
fi

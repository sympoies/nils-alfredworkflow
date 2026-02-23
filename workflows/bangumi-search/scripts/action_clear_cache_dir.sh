#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
helper_loader=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -z "$helper_loader" ]]; then
  echo "error: workflow helper missing: workflow_helper_loader.sh" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" off; then
  echo "error: workflow helper missing: workflow_cli_resolver.sh" >&2
  exit 1
fi

cache_dir_raw="${BANGUMI_CACHE_DIR:-}"
cache_dir="$(printf '%s' "$cache_dir_raw" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
cache_dir="$(wfcr_expand_home_path "$cache_dir")"

if [[ -z "$cache_dir" ]]; then
  exit 0
fi

case "$cache_dir" in
"/" | "." | "..")
  echo "refusing to clear unsafe BANGUMI_CACHE_DIR value: $cache_dir" >&2
  exit 1
  ;;
esac

if [[ -d "$cache_dir" ]]; then
  find "$cache_dir" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
fi

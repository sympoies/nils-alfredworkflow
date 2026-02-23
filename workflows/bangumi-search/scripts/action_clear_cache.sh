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
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  echo "workflow_helper_loader.sh helper not found" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "script_filter_async_coalesce.sh" auto; then
  echo "script_filter_async_coalesce.sh helper not found" >&2
  exit 1
fi

workflow_key="$(sfac_sanitize_component "bangumi-search")"
cache_dir="$(sfac_resolve_workflow_cache_dir "nils-bangumi-search-workflow")"
state_dir="$cache_dir/script-filter-async-coalesce/$workflow_key"

if [[ -d "$state_dir" ]]; then
  rm -rf "$state_dir"
fi

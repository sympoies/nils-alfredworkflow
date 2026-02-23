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
  echo "memo-workflow helper missing: workflow_helper_loader.sh" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "script_filter_query_policy.sh" off; then
  echo "memo-workflow helper missing: script_filter_query_policy.sh" >&2
  exit 1
fi

query="$(sfqp_resolve_query_input_memo_trimmed "$@")"

# Default behavior follows mmr for empty query (latest list only).
if [[ -z "$query" ]]; then
  exec "$script_dir/script_filter_recent.sh" "$query"
fi

MEMO_QUERY_PREFIX="copy" exec "$script_dir/script_filter.sh" "$query"

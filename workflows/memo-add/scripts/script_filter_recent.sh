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

first_token="${query%%[[:space:]]*}"
first_token_lower="$(printf '%s' "$first_token" | tr '[:upper:]' '[:lower:]')"

case "$first_token_lower" in
item | update | delete | copy | search)
  exec "$script_dir/script_filter.sh" "$query"
  ;;
esac

# mmr <number> => route to item-id action menu.
if [[ "$query" =~ ^[0-9]+$ ]]; then
  MEMO_QUERY_PREFIX="item" exec "$script_dir/script_filter.sh" "$query"
fi

# Default: always render latest list (newest first) via empty-query mode.
alfred_workflow_query="" ALFRED_WORKFLOW_QUERY="" exec "$script_dir/script_filter.sh" "" </dev/null

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

if [[ -n "$helper_loader" ]]; then
  # shellcheck disable=SC1090
  source "$helper_loader"
  wfhl_source_helper "$script_dir" "script_filter_error_json.sh" off || true
fi

if ! declare -F sfej_emit_error_item_json >/dev/null 2>&1; then
  sfej_fallback_json_escape() {
    local value="${1-}"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/ }"
    value="${value//$'\r'/ }"
    printf '%s' "$value"
  }

  sfej_emit_error_item_json() {
    local title="${1-Error}"
    local subtitle="${2-}"
    printf '{"items":[{"title":"%s","subtitle":"%s","valid":false}]}' \
      "$(sfej_fallback_json_escape "$title")" \
      "$(sfej_fallback_json_escape "$subtitle")"
    printf '\n'
  }
fi

if [[ -z "$helper_loader" ]]; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_helper_loader.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" off; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_cli_resolver.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_query_policy.sh" off; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate script_filter_query_policy.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_cli_driver.sh" off; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate script_filter_cli_driver.sh runtime helper."
  exit 0
fi

map_error_title() {
  local message
  message="$(printf '%s' "${1-}" | tr '[:upper:]' '[:lower:]')"

  if [[ "$message" == *"invalid memo_"* ]]; then
    printf '%s\n' "Invalid Memo workflow config"
    return
  fi

  if [[ "$message" == *"binary not found"* ]]; then
    printf '%s\n' "memo-workflow-cli binary not found"
    return
  fi

  printf '%s\n' "Memo workflow error"
}

print_error_item() {
  local raw_message="${1:-memo-workflow-cli script-filter failed}"
  local message="${raw_message}"
  [[ -n "$message" ]] || message="memo-workflow-cli script-filter failed"

  local title
  title="$(map_error_title "$message")"
  if [[ "$title" == "memo-workflow-cli binary not found" ]]; then
    sfej_emit_error_item_json "$title" "Re-import workflow package or set MEMO_WORKFLOW_CLI_BIN."
    return 0
  fi

  sfej_emit_error_item_json "$title" "$message"
}

execute_memo_script_filter() {
  local query="${1:-}"
  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local memo_workflow_cli
  memo_workflow_cli="$(
    wfcr_resolve_binary \
      "MEMO_WORKFLOW_CLI_BIN" \
      "$script_dir/../bin/memo-workflow-cli" \
      "$repo_root/target/release/memo-workflow-cli" \
      "$repo_root/target/debug/memo-workflow-cli" \
      "memo-workflow-cli binary not found (checked MEMO_WORKFLOW_CLI_BIN/package/release/debug paths)"
  )"

  "$memo_workflow_cli" script-filter --query "$query"
}

query="$(sfqp_resolve_query_input_memo "$@")"

query_prefix="${MEMO_QUERY_PREFIX:-}"
if [[ -n "$query_prefix" ]]; then
  if [[ -n "$query" ]]; then
    query="$query_prefix $query"
  else
    query="$query_prefix"
  fi
fi

sfcd_run_cli_flow \
  "execute_memo_script_filter" \
  "print_error_item" \
  "memo-workflow-cli returned empty response" \
  "memo-workflow-cli returned malformed Alfred JSON" \
  "$query"

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(
  CDPATH=
  cd -- "$(dirname -- "$0")" && pwd
)"

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
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "script_filter_error_json.sh" off; then
  wfhl_emit_missing_helper_item_json "script_filter_error_json.sh"
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" off; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_cli_resolver.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_cli_driver.sh" off; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate script_filter_cli_driver.sh runtime helper."
  exit 0
fi

resolve_workflow_cli() {
  local repo_root
  repo_root="$(
    CDPATH=
    cd -- "$script_dir/../../.." && pwd
  )"

  wfcr_resolve_binary \
    "WORKFLOW_CLI_BIN" \
    "$script_dir/../bin/workflow-cli" \
    "$repo_root/target/release/workflow-cli" \
    "$repo_root/target/debug/workflow-cli" \
    "error: workflow-cli binary not found (checked package/release/debug paths)"
}

print_error_item() {
  local raw_message="${1:-workflow-cli script-filter failed}"
  local message
  message="$(sfej_normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="workflow-cli script-filter failed"
  sfej_emit_error_item_json "Open Project error" "$message"
}

execute_open_project_script_filter() {
  local query="${1-}"
  local mode="${2-}"
  local workflow_cli
  workflow_cli="$(resolve_workflow_cli)"
  "$workflow_cli" script-filter --query "$query" --mode "$mode"
}

query="${1-}"
mode="${OPEN_PROJECT_MODE:-open}"

sfcd_run_cli_flow \
  "execute_open_project_script_filter" \
  "print_error_item" \
  "workflow-cli returned empty response" \
  "workflow-cli returned malformed Alfred JSON" \
  "$query" \
  "$mode"

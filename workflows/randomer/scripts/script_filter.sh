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

normalize_error_message() {
  sfej_normalize_error_message "${1-}"
}

emit_error_item() {
  local title="$1"
  local subtitle="$2"
  sfej_emit_error_item_json "$title" "$subtitle"
}

print_error_item() {
  local raw_message="${1:-randomer-cli list-formats failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="randomer-cli list-formats failed"

  local title="Randomer error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"binary not found"* ]]; then
    title="randomer-cli binary not found"
    subtitle="Package workflow or set RANDOMER_CLI_BIN to a randomer-cli executable."
  elif [[ "$lower" == *"malformed alfred json"* ]]; then
    title="Randomer output format error"
    subtitle="randomer-cli returned malformed Alfred JSON."
  fi

  emit_error_item "$title" "$subtitle"
}

resolve_randomer_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/randomer-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/randomer-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/randomer-cli"

  wfcr_resolve_binary \
    "RANDOMER_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "randomer-cli binary not found (checked RANDOMER_CLI_BIN/package/release/debug paths)"
}

execute_list_formats() {
  local query="${1:-}"
  local randomer_cli
  randomer_cli="$(resolve_randomer_cli)"
  "$randomer_cli" list-formats --query "$query" --mode alfred
}

query="${1:-}"
sfcd_run_cli_flow \
  "execute_list_formats" \
  "print_error_item" \
  "randomer-cli returned empty response" \
  "randomer-cli returned malformed Alfred JSON" \
  "$query"

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

helper_loader=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -z "$helper_loader" ]] && command -v git >/dev/null 2>&1; then
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$helper_loader"

load_helper_or_exit() {
  local helper_name="$1"
  if ! wfhl_source_helper "$script_dir" "$helper_name" auto; then
    wfhl_emit_missing_helper_item_json "$helper_name"
    exit 0
  fi
}

load_helper_or_exit "script_filter_error_json.sh"
load_helper_or_exit "workflow_cli_resolver.sh"
load_helper_or_exit "script_filter_cli_driver.sh"

print_error_item() {
  local raw_message="${1:-quote-cli feed failed}"
  local message
  message="$(sfej_normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="quote-cli feed failed"

  local title="Quote Feed error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"invalid quote_"* ]]; then
    title="Invalid Quote workflow config"
    subtitle="Check QUOTE_DISPLAY_COUNT / QUOTE_REFRESH_INTERVAL / QUOTE_FETCH_COUNT / QUOTE_MAX_ENTRIES."
  elif [[ "$lower" == *"binary not found"* ]]; then
    title="quote-cli binary not found"
    subtitle="Package workflow or set QUOTE_CLI_BIN to an executable quote-cli path."
  elif [[ "$lower" == *"zenquotes"* || "$lower" == *"request failed"* || "$lower" == *"timed out"* || "$lower" == *"timeout"* || "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* ]]; then
    title="Quote refresh unavailable"
    subtitle="Network/API refresh failed; cached quotes are still shown when available."
  fi

  sfej_emit_error_item_json "$title" "$subtitle"
}

resolve_quote_cli() {
  wfcr_resolve_binary \
    "QUOTE_CLI_BIN" \
    "$script_dir/../bin/quote-cli" \
    "$repo_root/target/release/quote-cli" \
    "$repo_root/target/debug/quote-cli" \
    "quote-cli binary not found (checked QUOTE_CLI_BIN/package/release/debug paths)"
}

execute_quote_feed() {
  local query="$1"
  local quote_cli=""

  if ! quote_cli="$(resolve_quote_cli)"; then
    return 1
  fi

  "$quote_cli" feed --query "$query" --mode alfred
}

query="${1:-}"

sfcd_run_cli_flow \
  "execute_quote_feed" \
  "print_error_item" \
  "quote-cli returned empty response" \
  "quote-cli returned malformed Alfred JSON" \
  "$query"

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
  local raw_message="${1:-market-cli expr failed}"
  local message
  message="$(sfej_normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="market-cli expr failed"

  local title="Market Expression error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"binary not found"* ]]; then
    title="market-cli binary not found"
    subtitle="Package workflow or set MARKET_CLI_BIN to an executable market-cli path."
  elif [[ "$lower" == *"unsupported operator"* || "$lower" == *"operator '*'"* || "$lower" == *"operator '/'"* || "$lower" == *"operator *"* || "$lower" == *"operator /"* || "$lower" == *"unsupported *"* || "$lower" == *"unsupported /"* ]]; then
    title="Unsupported operator"
    subtitle="Asset expressions support +/-. Numeric-only expressions support + - * /."
  elif [[ "$lower" == *"mixed asset and numeric"* || "$lower" == *"mixed numeric and asset"* || "$lower" == *"cannot mix asset and numeric"* || "$lower" == *"cannot mix numeric and asset"* || "$lower" == *"asset and numeric terms"* || "$lower" == *"numeric and asset terms"* ]]; then
    title="Invalid expression terms"
    subtitle="Do not mix asset symbols and raw numeric-only terms in the same side of expression."
  elif [[ "$lower" == *"invalid to clause"* || "$lower" == *"incomplete to clause"* || "$lower" == *"invalid to-clause"* || "$lower" == *"incomplete to-clause"* || "$lower" == *"missing target after to"* || "$lower" == *"expected target after to"* ]]; then
    title="Invalid to-clause"
    subtitle="Use a complete target clause, for example: 1 BTC + 2 ETH to USD."
  elif [[ "$lower" == *"invalid expression"* || "$lower" == *"parse error"* || "$lower" == *"syntax error"* || "$lower" == *"expected expression"* || "$lower" == *"unexpected token"* || "$lower" == *"invalid token"* ]]; then
    title="Invalid expression"
    subtitle="Use market terms with + or -, for example: 1 BTC + 2 ETH to USD."
  elif [[ "$lower" == *"provider"* || "$lower" == *"upstream"* || "$lower" == *"rate limit"* || "$lower" == *"429"* ]]; then
    title="Market Expression provider failure"
    subtitle="Failed to fetch market data from provider. Retry shortly."
  elif [[ "$lower" == *"timeout"* || "$lower" == *"timed out"* || "$lower" == *"io error"* || "$lower" == *"internal error"* || "$lower" == *"panic"* ]]; then
    title="Market Expression runtime failure"
    subtitle="market-cli failed while evaluating expression. Retry or inspect stderr details."
  fi

  sfej_emit_error_item_json "$title" "$subtitle"
}

resolve_market_cli() {
  wfcr_resolve_binary \
    "MARKET_CLI_BIN" \
    "$script_dir/../bin/market-cli" \
    "$repo_root/target/release/market-cli" \
    "$repo_root/target/debug/market-cli" \
    "market-cli binary not found (checked MARKET_CLI_BIN/package/release/debug paths)"
}

execute_market_expression() {
  local query="$1"
  local default_fiat="$2"
  local market_cli=""

  if ! market_cli="$(resolve_market_cli)"; then
    return 1
  fi

  "$market_cli" expr --query "$query" --default-fiat "$default_fiat"
}

query="${1:-}"
default_fiat="${MARKET_DEFAULT_FIAT:-USD}"

if [[ -z "$(printf '%s' "$query" | sed 's/[[:space:]]//g')" ]]; then
  sfej_emit_error_item_json \
    "Enter a market expression" \
    "Example: 1 BTC + 3 ETH to JPY (default fiat: ${default_fiat})"
  exit 0
fi

sfcd_run_cli_flow \
  "execute_market_expression" \
  "print_error_item" \
  "market-cli returned empty response" \
  "market-cli returned malformed Alfred JSON" \
  "$query" \
  "$default_fiat"

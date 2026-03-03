#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

workflow_helper_loader="$script_dir/lib/workflow_helper_loader.sh"
if [[ ! -f "$workflow_helper_loader" ]]; then
  workflow_helper_loader="$script_dir/../../../scripts/lib/workflow_helper_loader.sh"
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    workflow_helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$workflow_helper_loader"

load_helper_or_exit() {
  local helper_name="$1"
  local fallback="${2:-auto}"
  if ! wfhl_source_helper "$script_dir" "$helper_name" "$fallback"; then
    wfhl_emit_missing_helper_item_json "$helper_name"
    exit 0
  fi
}

load_helper_or_exit "script_filter_error_json.sh"
load_helper_or_exit "workflow_cli_resolver.sh"

normalize_error_message() {
  sfej_normalize_error_message "${1-}"
}

emit_error_item() {
  local title="$1"
  local subtitle="$2"
  sfej_emit_error_item_json "$title" "$subtitle"
}

print_error_item() {
  local raw_message="${1:-brave-cli query failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="brave-cli query failed"

  local title="Google Search error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"query must not be empty"* || "$lower" == *"query cannot be empty"* || "$lower" == *"empty query"* ]]; then
    title="Enter a search query"
    subtitle="Type keywords after gg to get Google suggestions."
  elif [[ "$lower" == *"google suggest request failed"* || "$lower" == *"invalid google suggest response"* || "$lower" == *"suggestions unavailable"* ]]; then
    title="Google suggestions unavailable"
    subtitle="Cannot fetch suggestions now. Retry or use gb for direct search."
  elif [[ "$lower" == *"missing brave_api_key"* || "$lower" == *"brave_api_key is missing"* || "$lower" == *"brave api key is missing"* || "$lower" == *"brave_api_key is required"* ]]; then
    title="Brave API key is missing"
    subtitle="Set BRAVE_API_KEY in workflow configuration to open selected suggestions."
  elif [[ "$lower" == *"quota"* || "$lower" == *"rate limit"* || "$lower" == *"rate-limit"* || "$lower" == *"too many requests"* || "$lower" == *"http 429"* || "$lower" == *"status 429"* ]]; then
    title="Brave API rate limited"
    subtitle="Too many requests in a short time. Retry shortly or lower BRAVE_MAX_RESULTS."
  elif [[ "$lower" == *"unavailable"* || "$lower" == *"transport"* || "$lower" == *"timed out"* || "$lower" == *"timeout"* || "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* || "$lower" == *"5xx"* || "$lower" == *"status 500"* || "$lower" == *"status 502"* || "$lower" == *"status 503"* || "$lower" == *"status 504"* ]]; then
    title="Brave API unavailable"
    subtitle="Cannot reach Brave API now. Check network and retry."
  elif [[ "$lower" == *"invalid brave_max_results"* || "$lower" == *"invalid brave_safesearch"* || "$lower" == *"invalid brave_country"* ]]; then
    title="Invalid Brave workflow config"
    subtitle="$message"
  fi

  emit_error_item "$title" "$subtitle"
}

resolve_brave_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/brave-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/brave-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/brave-cli"

  wfcr_resolve_binary \
    "BRAVE_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "brave-cli binary not found (checked package/release/debug paths)"
}

google_query_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/google-search-suggest-script-filter.err.$$.$RANDOM"

  local brave_cli
  if ! brave_cli="$(resolve_brave_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$brave_cli" query --input "$query" --mode alfred 2>"$err_file")"; then
    rm -f "$err_file"
    if [[ -z "$json_output" ]]; then
      echo "brave-cli returned empty response" >&2
      return 1
    fi

    printf '%s\n' "$json_output"
    return 0
  fi

  cat "$err_file" >&2
  rm -f "$err_file"
  return 1
}

load_helper_or_exit "script_filter_query_policy.sh"
load_helper_or_exit "script_filter_async_coalesce.sh"
load_helper_or_exit "script_filter_search_driver.sh"

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
query="$trimmed_query"

if [[ -z "$query" ]]; then
  emit_error_item "Enter a search query" "Type keywords after gg to get Google suggestions."
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before fetching suggestions."
  exit 0
fi

# Shared driver owns cache/coalesce orchestration only.
# Google-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$query" \
  "google-search" \
  "nils-google-search-workflow" \
  "BRAVE_QUERY_CACHE_TTL_SECONDS" \
  "BRAVE_QUERY_COALESCE_SETTLE_SECONDS" \
  "BRAVE_QUERY_COALESCE_RERUN_SECONDS" \
  "Fetching Google suggestions..." \
  "Waiting for final query before fetching Google suggestions." \
  "google_query_fetch_json" \
  "print_error_item"

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

if [[ -z "$helper_loader" ]] && command -v git >/dev/null 2>&1; then
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}'
  printf '\n'
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
load_helper_or_exit "script_filter_query_policy.sh"
load_helper_or_exit "script_filter_async_coalesce.sh"
load_helper_or_exit "script_filter_search_driver.sh"
load_helper_or_exit "workflow_cli_resolver.sh"

print_error_item() {
  local raw_message="${1:-steam-cli search failed}"
  local message
  message="$(sfej_normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="steam-cli search failed"

  local title="Steam Search error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"empty query"* || "$lower" == *"query must not be empty"* ]]; then
    title="Enter a search query"
    subtitle="Type keywords after st to search Steam."
  elif [[ "$lower" == *"invalid steam_region"* || "$lower" == *"invalid steam_region_options"* || "$lower" == *"invalid steam_max_results"* ]]; then
    title="Invalid Steam workflow config"
    subtitle="Check STEAM_REGION, STEAM_REGION_OPTIONS, and STEAM_MAX_RESULTS."
  elif [[ "$lower" == *"binary not found"* ]]; then
    title="steam-cli binary not found"
    subtitle="Package workflow or set STEAM_CLI_BIN to a steam-cli executable."
  elif [[ "$lower" == *"unavailable"* || "$lower" == *"timed out"* || "$lower" == *"timeout"* || "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* || "$lower" == *"status 500"* || "$lower" == *"status 502"* || "$lower" == *"status 503"* || "$lower" == *"status 504"* ]]; then
    title="Steam API unavailable"
    subtitle="Cannot reach Steam Store now. Check network and retry."
  fi

  sfej_emit_error_item_json "$title" "$subtitle"
}

resolve_steam_cli() {
  local packaged_cli
  packaged_cli="$script_dir/../bin/steam-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/steam-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/steam-cli"

  wfcr_resolve_binary \
    "STEAM_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "steam-cli binary not found (checked STEAM_CLI_BIN/package/release/debug paths)"
}

steam_search_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/steam-search-script-filter.err.$$.$RANDOM"

  if [[ -n "${STEAM_SCRIPT_FILTER_STUB_FILE:-}" && -f "${STEAM_SCRIPT_FILTER_STUB_FILE}" ]]; then
    cat "${STEAM_SCRIPT_FILTER_STUB_FILE}"
    return 0
  fi

  local steam_cli
  if ! steam_cli="$(resolve_steam_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$steam_cli" search --query "$query" --mode alfred 2>"$err_file")"; then
    rm -f "$err_file"

    if [[ -z "$json_output" ]]; then
      echo "steam-cli returned empty response" >&2
      return 1
    fi

    if command -v jq >/dev/null 2>&1; then
      if ! jq -e '.items | type == "array"' >/dev/null <<<"$json_output"; then
        echo "steam-cli returned malformed Alfred JSON" >&2
        return 1
      fi
    fi

    printf '%s\n' "$json_output"
    return 0
  fi

  cat "$err_file" >&2
  rm -f "$err_file"
  return 1
}

query="$(sfqp_resolve_query_input "${1:-}")"
query="$(sfqp_trim "$query")"

if [[ -z "$query" ]]; then
  sfej_emit_error_item_json "Enter a search query" "Type keywords after st to search Steam."
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before searching Steam."
  exit 0
fi

# Keep Steam scaffold immediate by default while preserving optional env overrides.
: "${STEAM_QUERY_CACHE_TTL_SECONDS:=0}"
: "${STEAM_QUERY_COALESCE_SETTLE_SECONDS:=0}"
: "${STEAM_QUERY_COALESCE_RERUN_SECONDS:=0.4}"

# Shared driver owns cache/coalesce orchestration only.
# Steam-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$query" \
  "steam-search" \
  "nils-steam-search-workflow" \
  "STEAM_QUERY_CACHE_TTL_SECONDS" \
  "STEAM_QUERY_COALESCE_SETTLE_SECONDS" \
  "STEAM_QUERY_COALESCE_RERUN_SECONDS" \
  "Searching Steam..." \
  "Waiting for final query before calling Steam Store." \
  "steam_search_fetch_json" \
  "print_error_item"

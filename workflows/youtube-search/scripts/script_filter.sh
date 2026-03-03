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
  local raw_message="${1:-youtube-cli search failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="youtube-cli search failed"

  local title="YouTube Search error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"query must not be empty"* ]]; then
    title="Enter a search query"
    subtitle="Type keywords after yt to search YouTube."
  elif [[ "$lower" == *"missing youtube_api_key"* ]]; then
    title="YouTube API key is missing"
    subtitle="Set YOUTUBE_API_KEY in workflow configuration and retry."
  elif [[ "$lower" == *"quota"* || "$lower" == *"dailylimitexceeded"* ]]; then
    title="YouTube quota exceeded"
    subtitle="Daily quota is exhausted. Retry later or lower YOUTUBE_MAX_RESULTS."
  elif [[ "$lower" == *"youtube api request failed"* || "$lower" == *"youtube api error (5"* || "$lower" == *"service unavailable"* || "$lower" == *"timed out"* || "$lower" == *"connection"* ]]; then
    title="YouTube API unavailable"
    subtitle="Cannot reach YouTube API now. Check network and retry."
  elif [[ "$lower" == *"invalid youtube_max_results"* || "$lower" == *"invalid youtube_region_code"* ]]; then
    title="Invalid YouTube workflow config"
    subtitle="$message"
  fi

  emit_error_item "$title" "$subtitle"
}

resolve_youtube_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/youtube-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/youtube-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/youtube-cli"

  wfcr_resolve_binary \
    "YOUTUBE_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "youtube-cli binary not found (checked package/release/debug paths)"
}

youtube_search_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/youtube-search-script-filter.err.$$.$RANDOM"

  local youtube_cli
  if ! youtube_cli="$(resolve_youtube_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$youtube_cli" search --query "$query" --mode alfred 2>"$err_file")"; then
    rm -f "$err_file"
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
  emit_error_item "Enter a search query" "Type keywords after yt to search YouTube."
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before searching YouTube."
  exit 0
fi

# Shared driver owns cache/coalesce orchestration only.
# YouTube-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$query" \
  "youtube-search" \
  "nils-youtube-search-workflow" \
  "YOUTUBE_QUERY_CACHE_TTL_SECONDS" \
  "YOUTUBE_QUERY_COALESCE_SETTLE_SECONDS" \
  "YOUTUBE_QUERY_COALESCE_RERUN_SECONDS" \
  "Searching YouTube..." \
  "Waiting for final query before calling YouTube API." \
  "youtube_search_fetch_json" \
  "print_error_item"

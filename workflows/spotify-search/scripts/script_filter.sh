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

if ! wfhl_source_helper "$script_dir" "script_filter_error_json.sh"; then
  wfhl_emit_missing_helper_item_json "script_filter_error_json.sh"
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh"; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_cli_resolver.sh runtime helper."
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
  local raw_message="${1:-spotify-cli search failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="spotify-cli search failed"

  local title="Spotify Search error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"query must not be empty"* || "$lower" == *"query cannot be empty"* || "$lower" == *"empty query"* ]]; then
    title="Enter a search query"
    subtitle="Type keywords after sp to search Spotify tracks."
  elif [[ "$lower" == *"missing spotify_client_id"* || "$lower" == *"missing spotify_client_secret"* || "$lower" == *"spotify_client_id is required"* || "$lower" == *"spotify_client_secret is required"* || "$lower" == *"missing spotify client id"* || "$lower" == *"missing spotify client secret"* || "$lower" == *"missing credentials"* ]]; then
    title="Spotify credentials are missing"
    subtitle="Set SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET in workflow configuration."
  elif [[ "$lower" == *"invalid_client"* || "$lower" == *"spotify auth error (401)"* || "$lower" == *"spotify auth error (403)"* || "$lower" == *"unauthorized"* || "$lower" == *"forbidden"* ]]; then
    title="Spotify credentials are invalid"
    subtitle="Verify SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET and retry."
  elif [[ "$lower" == *"quota"* || "$lower" == *"rate limit"* || "$lower" == *"rate-limit"* || "$lower" == *"too many requests"* || "$lower" == *"http 429"* || "$lower" == *"status 429"* ]]; then
    title="Spotify API rate limited"
    subtitle="Rate limit reached. Retry later or lower SPOTIFY_MAX_RESULTS."
  elif [[ "$lower" == *"unavailable"* || "$lower" == *"transport"* || "$lower" == *"timed out"* || "$lower" == *"timeout"* || "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* || "$lower" == *"5xx"* || "$lower" == *"status 500"* || "$lower" == *"status 502"* || "$lower" == *"status 503"* || "$lower" == *"status 504"* ]]; then
    title="Spotify API unavailable"
    subtitle="Cannot reach Spotify API now. Check network and retry."
  elif [[ "$lower" == *"invalid spotify_max_results"* || "$lower" == *"invalid spotify_market"* || "$lower" == *"invalid config"* || "$lower" == *"invalid configuration"* ]]; then
    title="Invalid Spotify workflow config"
    subtitle="$message"
  elif [[ "$lower" == *"binary not found"* ]]; then
    title="spotify-cli binary not found"
    subtitle="Package workflow or set SPOTIFY_CLI_BIN to a spotify-cli executable."
  fi

  emit_error_item "$title" "$subtitle"
}

resolve_spotify_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/spotify-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/spotify-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/spotify-cli"

  wfcr_resolve_binary \
    "SPOTIFY_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "spotify-cli binary not found (checked SPOTIFY_CLI_BIN/package/release/debug paths)"
}

spotify_search_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/spotify-search-script-filter.err.$$.$RANDOM"

  local spotify_cli
  if ! spotify_cli="$(resolve_spotify_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$spotify_cli" search --query "$query" --mode alfred 2>"$err_file")"; then
    rm -f "$err_file"
    if [[ -z "$json_output" ]]; then
      echo "spotify-cli returned empty response" >&2
      return 1
    fi

    if command -v jq >/dev/null 2>&1; then
      if ! jq -e '.items | type == "array"' >/dev/null <<<"$json_output"; then
        echo "spotify-cli returned malformed Alfred JSON" >&2
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

if ! wfhl_source_helper "$script_dir" "script_filter_query_policy.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_query_policy.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_async_coalesce.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_async_coalesce.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_search_driver.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_search_driver.sh runtime helper."
  exit 0
fi

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
query="$trimmed_query"
if [[ -z "$trimmed_query" ]]; then
  emit_error_item "Enter a search query" "Type keywords after sp to search Spotify tracks."
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before searching Spotify."
  exit 0
fi

# Keep Spotify search responsive while avoiding transient prefix queries.
: "${SPOTIFY_QUERY_CACHE_TTL_SECONDS:=0}"
: "${SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS:=1}"
: "${SPOTIFY_QUERY_COALESCE_RERUN_SECONDS:=0.4}"

# Shared driver owns cache/coalesce orchestration only.
# Spotify-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$query" \
  "spotify-search" \
  "nils-spotify-search-workflow" \
  "SPOTIFY_QUERY_CACHE_TTL_SECONDS" \
  "SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS" \
  "SPOTIFY_QUERY_COALESCE_RERUN_SECONDS" \
  "Searching Spotify..." \
  "Waiting for final query before calling Spotify API." \
  "spotify_search_fetch_json" \
  "print_error_item"

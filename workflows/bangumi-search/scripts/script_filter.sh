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

BANGUMI_CLEAR_CACHE_ACTION_ARG="__BANGUMI_CLEAR_CACHE__"
BANGUMI_CLEAR_CACHE_DIR_ACTION_ARG="__BANGUMI_CLEAR_CACHE_DIR__"

print_error_item() {
  local raw_message="${1:-bangumi-cli query failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="bangumi-cli query failed"

  local title="Bangumi Search error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"query must not be empty"* || "$lower" == *"empty query"* ]]; then
    title="Enter a search query"
    subtitle="Type keywords after bgm to search Bangumi."
  elif [[ "$lower" == *"missing bangumi_api_key"* || "$lower" == *"api key required"* ]]; then
    title="Bangumi API key is missing"
    subtitle="Set BANGUMI_API_KEY in workflow configuration and retry."
  elif [[ "$lower" == *"invalid bangumi_"* || "$lower" == *"invalid config"* ]]; then
    title="Invalid Bangumi workflow config"
    subtitle="Check BANGUMI_MAX_RESULTS/BANGUMI_TIMEOUT_MS/BANGUMI_API_FALLBACK and retry."
  elif [[ "$lower" == *"rate limit"* || "$lower" == *"status 429"* ]]; then
    title="Bangumi API rate-limited"
    subtitle="Bangumi API returned 429. Retry later or lower BANGUMI_MAX_RESULTS."
  elif [[ "$lower" == *"bangumi-cli binary not found"* || "$lower" == *"binary not found"* ]]; then
    title="bangumi-cli binary not found"
    subtitle="Package workflow or set BANGUMI_CLI_BIN to a bangumi-cli executable."
  elif [[ "$lower" == *"bangumi api request failed"* || "$lower" == *"bangumi api unavailable"* || "$lower" == *"invalid bangumi api response"* || "$lower" == *"timed out"* || "$lower" == *"timeout"* || "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* || "$lower" == *"status 500"* || "$lower" == *"status 502"* || "$lower" == *"status 503"* || "$lower" == *"status 504"* || "$lower" == *"api error (5"* ]]; then
    title="Bangumi API unavailable"
    subtitle="Cannot reach Bangumi API now. Check network and retry."
  fi

  emit_error_item "$title" "$subtitle"
}

normalize_query_command() {
  local value="$1"
  value="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
  value="${value//-/ }"
  value="${value//_/ }"
  value="$(printf '%s' "$value" | tr -s '[:space:]' ' ')"
  sfqp_trim "$value"
}

is_clear_cache_command() {
  local normalized
  normalized="$(normalize_query_command "$1")"
  case "$normalized" in
  "clear cache" | "cache clear")
    return 0
    ;;
  esac
  return 1
}

is_clear_cache_dir_command() {
  local normalized
  normalized="$(normalize_query_command "$1")"
  case "$normalized" in
  "clear cache dir" | "clear cachedir" | "cache dir clear" | "cachedir clear")
    return 0
    ;;
  esac
  return 1
}

emit_clear_cache_item_json() {
  printf '{"items":[{"title":"Clear Bangumi query cache","subtitle":"Press Enter to remove local bangumi-search cache files.","arg":"%s","valid":true}]}\n' \
    "$BANGUMI_CLEAR_CACHE_ACTION_ARG"
}

emit_clear_cache_dir_item_json() {
  printf '{"items":[{"title":"Clear Bangumi cache dir","subtitle":"Press Enter to remove files under BANGUMI_CACHE_DIR (if configured).","arg":"%s","valid":true}]}\n' \
    "$BANGUMI_CLEAR_CACHE_DIR_ACTION_ARG"
}

emit_empty_query_items_json() {
  printf '{"items":[{"title":"Enter a search query","subtitle":"Type keywords after bgm to search Bangumi.","valid":false},{"title":"Clear Bangumi query cache","subtitle":"Press Enter to remove local bangumi-search cache files.","arg":"%s","valid":true},{"title":"Clear Bangumi cache dir","subtitle":"Press Enter to remove files under BANGUMI_CACHE_DIR (if configured).","arg":"%s","valid":true},{"title":"Bangumi Search (Anime)","subtitle":"Type and search anime via bgm anime <query>.","autocomplete":"anime ","valid":false},{"title":"Bangumi Search (Game)","subtitle":"Type and search games via bgm game <query>.","autocomplete":"game ","valid":false},{"title":"Bangumi Search (Music)","subtitle":"Type and search music via bgm music <query>.","autocomplete":"music ","valid":false},{"title":"Bangumi Search (Book)","subtitle":"Type and search books via bgm book <query>.","autocomplete":"book ","valid":false},{"title":"Bangumi Search (Real)","subtitle":"Type and search real/live-action via bgm real <query>.","autocomplete":"real ","valid":false}]}\n' \
    "$BANGUMI_CLEAR_CACHE_ACTION_ARG" \
    "$BANGUMI_CLEAR_CACHE_DIR_ACTION_ARG"
}

resolve_keyword_default_subject_type() {
  local keyword="${alfred_workflow_keyword:-${ALFRED_WORKFLOW_KEYWORD:-}}"
  keyword="$(printf '%s' "$keyword" | tr '[:upper:]' '[:lower:]')"

  case "$keyword" in
  bgmb)
    printf '%s\n' "book"
    ;;
  bgma)
    printf '%s\n' "anime"
    ;;
  bgmm)
    printf '%s\n' "music"
    ;;
  bgmg)
    printf '%s\n' "game"
    ;;
  bgmr)
    printf '%s\n' "real"
    ;;
  *)
    printf '%s\n' ""
    ;;
  esac
}

query_has_explicit_subject_type() {
  local query_lower
  query_lower="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"

  if [[ "$query_lower" =~ ^\[[[:space:]]*(all|book|anime|music|game|real)[[:space:]]*\][[:space:]]+ ]]; then
    return 0
  fi

  if [[ "$query_lower" =~ ^(all|book|anime|music|game|real): ]]; then
    return 0
  fi

  if [[ "$query_lower" =~ ^(all|book|anime|music|game|real)([[:space:]]+|$) ]]; then
    return 0
  fi

  return 1
}

apply_default_subject_type() {
  local default_type="$1"
  local query="$2"

  case "$default_type" in
  all | book | anime | music | game | real) ;;
  *)
    printf '%s\n' "$query"
    return 0
    ;;
  esac

  if query_has_explicit_subject_type "$query"; then
    printf '%s\n' "$query"
  else
    printf '%s %s\n' "$default_type" "$query"
  fi
}

resolve_bangumi_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/bangumi-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/bangumi-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/bangumi-cli"

  wfcr_resolve_binary \
    "BANGUMI_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "bangumi-cli binary not found (checked BANGUMI_CLI_BIN/package/release/debug paths)"
}

bangumi_query_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/bangumi-search-script-filter.err.$$.$RANDOM"

  local bangumi_cli
  if ! bangumi_cli="$(resolve_bangumi_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$bangumi_cli" query --input "$query" --output alfred-json 2>"$err_file")"; then
    rm -f "$err_file"

    if [[ -z "$json_output" ]]; then
      echo "bangumi-cli returned empty response" >&2
      return 1
    fi

    if command -v jq >/dev/null 2>&1; then
      if ! jq -e '.items | type == "array"' >/dev/null <<<"$json_output"; then
        echo "bangumi-cli returned malformed Alfred JSON" >&2
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

if [[ -z "$query" ]]; then
  emit_empty_query_items_json
  exit 0
fi

if is_clear_cache_command "$query"; then
  emit_clear_cache_item_json
  exit 0
fi

if is_clear_cache_dir_command "$query"; then
  emit_clear_cache_dir_item_json
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before searching Bangumi."
  exit 0
fi

default_subject_type="$(sfqp_trim "${BANGUMI_DEFAULT_TYPE:-}")"
if [[ -z "$default_subject_type" ]]; then
  default_subject_type="$(resolve_keyword_default_subject_type)"
fi
if [[ -n "$default_subject_type" ]]; then
  default_subject_type="$(printf '%s' "$default_subject_type" | tr '[:upper:]' '[:lower:]')"
  query="$(apply_default_subject_type "$default_subject_type" "$query")"
fi

# Shared driver owns cache/coalesce orchestration only.
# Bangumi-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$query" \
  "bangumi-search" \
  "nils-bangumi-search-workflow" \
  "BANGUMI_QUERY_CACHE_TTL_SECONDS" \
  "BANGUMI_QUERY_COALESCE_SETTLE_SECONDS" \
  "BANGUMI_QUERY_COALESCE_RERUN_SECONDS" \
  "Searching Bangumi..." \
  "Waiting for final query before calling Bangumi API." \
  "bangumi_query_fetch_json" \
  "print_error_item"

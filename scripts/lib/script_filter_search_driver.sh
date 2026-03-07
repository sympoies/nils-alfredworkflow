#!/usr/bin/env bash
# Shared async-search Script Filter orchestration driver.
# This helper centralizes cache/coalesce/pending flow only.
# Each workflow keeps backend fetch details and error mapping locally.

sfsd_emit_fallback_error_row_json() {
  local raw_message="${1-script-filter search failed}"
  local message="$raw_message"
  message="${message//\\/\\\\}"
  message="${message//\"/\\\"}"
  message="${message//$'\n'/ }"
  message="${message//$'\r'/ }"
  printf '{"items":[{"title":"Workflow runtime error","subtitle":"%s","valid":false}]}\n' "$message"
}

sfsd_emit_error() {
  local error_fn="${1-}"
  local raw_message="${2-script-filter search failed}"

  if [[ -n "$error_fn" ]] && declare -F "$error_fn" >/dev/null 2>&1; then
    "$error_fn" "$raw_message"
    return 0
  fi

  sfsd_emit_fallback_error_row_json "$raw_message"
}

sfsd_make_temp_err_file() {
  local prefix="${1:-script-filter-search-driver.err}"
  if command -v mktemp >/dev/null 2>&1; then
    mktemp "${TMPDIR:-/tmp}/${prefix}.XXXXXX"
    return 0
  fi

  printf '%s/%s.%s.%s\n' "${TMPDIR:-/tmp}" "$prefix" "$$" "$RANDOM"
}

sfsd_fetch_and_emit() {
  local query="$1"
  local cache_ttl_seconds="$2"
  local fetch_fn="$3"
  local error_fn="$4"

  local json_output err_msg
  local err_file=""

  if [[ -z "$fetch_fn" ]] || ! declare -F "$fetch_fn" >/dev/null 2>&1; then
    sfsd_emit_error "$error_fn" "script-filter fetch callback is not defined"
    return 0
  fi

  err_file="$(sfsd_make_temp_err_file "script-filter-search-driver.err")"
  if json_output="$("$fetch_fn" "$query" 2>"$err_file")"; then
    if [[ "$cache_ttl_seconds" -gt 0 ]]; then
      sfac_store_cache_result "$query" "ok" "$json_output" || true
    fi
    rm -f "$err_file"
    printf '%s\n' "$json_output"
    return 0
  fi

  err_msg="$(cat "$err_file")"
  rm -f "$err_file"
  if [[ "$cache_ttl_seconds" -gt 0 ]]; then
    sfac_store_cache_result "$query" "err" "$err_msg" || true
  fi
  sfsd_emit_error "$error_fn" "$err_msg"
}

sfsd_run_search_flow() {
  local query="$1"
  local workflow_key="$2"
  local cache_fallback="$3"
  local cache_ttl_env="$4"
  local settle_env="$5"
  local rerun_env="$6"
  local pending_title="$7"
  local pending_subtitle="$8"
  local fetch_fn="$9"
  local error_fn="${10}"

  sfac_init_context "$workflow_key" "$cache_fallback"
  local cache_ttl_seconds settle_seconds rerun_seconds
  # Keep same-query cache disabled by default for live-typing Script Filters.
  # Current flow checks cache before settle-window coalescing; defaulting to 0
  # avoids stale prefix hits surfacing ahead of the final query.
  cache_ttl_seconds="$(sfac_resolve_positive_int_env "$cache_ttl_env" "0")"
  settle_seconds="$(sfac_resolve_non_negative_number_env "$settle_env" "1")"
  rerun_seconds="$(sfac_resolve_non_negative_number_env "$rerun_env" "0.4")"

  if sfac_load_cache_result "$query" "$cache_ttl_seconds"; then
    if [[ "$SFAC_CACHE_STATUS" == "ok" ]]; then
      printf '%s\n' "$SFAC_CACHE_PAYLOAD"
    else
      sfsd_emit_error "$error_fn" "$SFAC_CACHE_PAYLOAD"
    fi
    return 0
  fi

  if [[ "$settle_seconds" == "0" || "$settle_seconds" == "0.0" ]]; then
    sfsd_fetch_and_emit "$query" "$cache_ttl_seconds" "$fetch_fn" "$error_fn"
    return 0
  fi

  if ! sfac_wait_for_final_query "$query" "$settle_seconds"; then
    sfac_emit_pending_item_json "$pending_title" "$pending_subtitle" "$rerun_seconds"
    return 0
  fi

  sfsd_fetch_and_emit "$query" "$cache_ttl_seconds" "$fetch_fn" "$error_fn"
  return 0
}

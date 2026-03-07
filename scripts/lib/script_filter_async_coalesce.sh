#!/usr/bin/env bash
# Shared query coalescing helpers for Alfred Script Filter workflows.
# This helper keeps final-query priority by using a queue-safe settle window:
# latest query must remain unchanged for N seconds before backend dispatch.

sfac_json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

sfac_emit_pending_item_json() {
  local title="${1:-Searching...}"
  local subtitle="${2:-Waiting for query to stabilize.}"
  local rerun_seconds="${3:-0.4}"

  if [[ ! "$rerun_seconds" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    rerun_seconds="0.4"
  fi

  printf '{"rerun":%s,"items":[{"title":"%s","subtitle":"%s","valid":false}]}' \
    "$rerun_seconds" \
    "$(sfac_json_escape "$title")" \
    "$(sfac_json_escape "$subtitle")"
  printf '\n'
}

sfac_resolve_positive_int_env() {
  local env_name="$1"
  local default_value="$2"

  local raw_value=""
  if [[ -n "${!env_name:-}" ]]; then
    raw_value="${!env_name}"
  fi

  raw_value="$(printf '%s' "$raw_value" | tr -d '[:space:]')"
  if [[ -z "$raw_value" || ! "$raw_value" =~ ^[0-9]+$ ]]; then
    raw_value="$default_value"
  fi

  printf '%s\n' "$raw_value"
}

sfac_resolve_non_negative_number_env() {
  local env_name="$1"
  local default_value="$2"

  local raw_value=""
  if [[ -n "${!env_name:-}" ]]; then
    raw_value="${!env_name}"
  fi

  raw_value="$(printf '%s' "$raw_value" | tr -d '[:space:]')"
  if [[ -z "$raw_value" || ! "$raw_value" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    raw_value="$default_value"
  fi

  printf '%s\n' "$raw_value"
}

sfac_resolve_workflow_cache_dir() {
  local fallback_dir="${1:-nils-script-filter-workflow}"

  local candidate
  for candidate in \
    "${ALFRED_WORKFLOW_CACHE:-}" \
    "${ALFRED_WORKFLOW_DATA:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "${TMPDIR:-/tmp}/$fallback_dir"
}

sfac_sanitize_component() {
  local raw="${1:-workflow}"
  raw="$(printf '%s' "$raw" | tr -c 'A-Za-z0-9._-' '_')"
  raw="${raw#_}"
  raw="${raw%_}"
  [[ -n "$raw" ]] || raw="workflow"
  printf '%s\n' "$raw"
}

sfac_now_epoch_seconds() {
  date +%s
}

sfac_now_epoch_millis() {
  if command -v perl >/dev/null 2>&1; then
    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'
    return 0
  fi

  printf '%s000\n' "$(sfac_now_epoch_seconds)"
}

sfac_normalize_epoch_millis() {
  local raw="${1:-}"
  [[ "$raw" =~ ^[0-9]+$ ]] || return 1

  # Backward compatibility: older request files stored whole seconds.
  if [[ "${#raw}" -le 10 ]]; then
    printf '%s000\n' "$raw"
    return 0
  fi

  printf '%s\n' "$raw"
}

sfac_init_context() {
  SFAC_WORKFLOW_KEY="$(sfac_sanitize_component "$1")"
  SFAC_WORKFLOW_CACHE_FALLBACK="${2:-nils-script-filter-workflow}"
}

sfac_require_context() {
  [[ -n "${SFAC_WORKFLOW_KEY:-}" ]]
}

sfac_state_dir() {
  sfac_require_context || return 1
  local cache_dir
  cache_dir="$(sfac_resolve_workflow_cache_dir "${SFAC_WORKFLOW_CACHE_FALLBACK:-nils-script-filter-workflow}")"

  local state_dir="$cache_dir/script-filter-async-coalesce/${SFAC_WORKFLOW_KEY}"
  mkdir -p "$state_dir/cache"
  printf '%s\n' "$state_dir"
}

sfac_request_file_path() {
  local state_dir
  state_dir="$(sfac_state_dir)" || return 1
  printf '%s/request.latest\n' "$state_dir"
}

sfac_query_key() {
  local query="${1-}"
  printf '%s' "$query" | cksum | awk '{print $1 "-" $2}'
}

sfac_cache_meta_path() {
  local query="$1"
  local key
  key="$(sfac_query_key "$query")"

  local state_dir
  state_dir="$(sfac_state_dir)" || return 1
  printf '%s/cache/%s.meta\n' "$state_dir" "$key"
}

sfac_cache_payload_path() {
  local query="$1"
  local key
  key="$(sfac_query_key "$query")"

  local state_dir
  state_dir="$(sfac_state_dir)" || return 1
  printf '%s/cache/%s.payload\n' "$state_dir" "$key"
}

sfac_write_latest_request() {
  local query="${1-}"
  local request_file
  request_file="$(sfac_request_file_path)" || return 1

  local seq now_ms tmp_file
  now_ms="$(sfac_now_epoch_millis)"
  seq="${now_ms}.$$.$RANDOM"
  tmp_file="${request_file}.tmp.$$.$RANDOM"

  printf '%s\n%s\n%s\n' "$seq" "$now_ms" "$query" >"$tmp_file"
  mv "$tmp_file" "$request_file"
  printf '%s\n' "$seq"
}

sfac_read_latest_request() {
  local request_file
  request_file="$(sfac_request_file_path)" || return 1
  [[ -f "$request_file" ]] || return 1

  local seq updated query normalized_updated
  seq="$(sed -n '1p' "$request_file")"
  updated="$(sed -n '2p' "$request_file")"
  query="$(sed -n '3p' "$request_file")"

  [[ -n "$seq" && -n "$updated" ]] || return 1
  normalized_updated="$(sfac_normalize_epoch_millis "$updated")" || return 1

  # shellcheck disable=SC2034 # Exposed for callers that inspect the request snapshot fields.
  SFAC_REQUEST_SEQ="$seq"
  # shellcheck disable=SC2034 # Exposed for callers that inspect the request snapshot fields.
  SFAC_REQUEST_UPDATED="$normalized_updated"
  # shellcheck disable=SC2034 # Exposed for callers that inspect the request snapshot fields.
  SFAC_REQUEST_QUERY="$query"
  return 0
}

sfac_wait_for_final_query() {
  local query="$1"
  local settle_seconds="${2:-0}"

  if [[ ! "$settle_seconds" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    settle_seconds="0"
  fi

  if [[ "$settle_seconds" == "0" || "$settle_seconds" == "0.0" ]]; then
    sfac_write_latest_request "$query" >/dev/null || true
    return 0
  fi

  local now_ms
  now_ms="$(sfac_now_epoch_millis)"

  if ! sfac_read_latest_request; then
    sfac_write_latest_request "$query" >/dev/null || return 1
    return 1
  fi

  if [[ "$SFAC_REQUEST_QUERY" != "$query" ]]; then
    sfac_write_latest_request "$query" >/dev/null || return 1
    return 1
  fi

  local age_millis
  age_millis=$((now_ms - SFAC_REQUEST_UPDATED))
  if [[ "$age_millis" -lt 0 ]]; then
    age_millis=0
  fi

  if awk -v age_ms="$age_millis" -v settle="$settle_seconds" 'BEGIN { exit !(age_ms >= (settle * 1000)) }'; then
    return 0
  fi

  return 1
}

sfac_store_cache_result() {
  local query="$1"
  local status="$2"
  local payload="${3-}"

  if [[ "$status" != "ok" && "$status" != "err" ]]; then
    status="err"
  fi

  local meta_path payload_path
  meta_path="$(sfac_cache_meta_path "$query")" || return 1
  payload_path="$(sfac_cache_payload_path "$query")" || return 1

  local now tmp_meta tmp_payload
  now="$(sfac_now_epoch_seconds)"
  tmp_meta="${meta_path}.tmp.$$.$RANDOM"
  tmp_payload="${payload_path}.tmp.$$.$RANDOM"

  printf '%s\n' "$payload" >"$tmp_payload"
  printf '%s\t%s\n' "$now" "$status" >"$tmp_meta"

  mv "$tmp_payload" "$payload_path"
  mv "$tmp_meta" "$meta_path"
}

sfac_load_cache_result() {
  local query="$1"
  local ttl_seconds="${2:-0}"

  # shellcheck disable=SC2034 # API output variable consumed by caller scripts.
  SFAC_CACHE_STATUS=""
  # shellcheck disable=SC2034 # API output variable consumed by caller scripts.
  SFAC_CACHE_PAYLOAD=""

  [[ "$ttl_seconds" =~ ^[0-9]+$ ]] || return 1
  [[ "$ttl_seconds" -gt 0 ]] || return 1

  local meta_path payload_path
  meta_path="$(sfac_cache_meta_path "$query")" || return 1
  payload_path="$(sfac_cache_payload_path "$query")" || return 1

  [[ -f "$meta_path" && -f "$payload_path" ]] || return 1

  local cached_at status
  IFS=$'\t' read -r cached_at status <"$meta_path" || return 1
  [[ "$cached_at" =~ ^[0-9]+$ ]] || return 1
  [[ "$status" == "ok" || "$status" == "err" ]] || return 1

  local now age
  now="$(sfac_now_epoch_seconds)"
  age=$((now - cached_at))
  [[ "$age" -ge 0 && "$age" -le "$ttl_seconds" ]] || return 1

  local payload
  payload="$(cat "$payload_path" 2>/dev/null || true)"
  [[ -n "$payload" ]] || return 1

  # shellcheck disable=SC2034 # API output variable consumed by caller scripts.
  SFAC_CACHE_STATUS="$status"
  # shellcheck disable=SC2034 # API output variable consumed by caller scripts.
  SFAC_CACHE_PAYLOAD="$payload"
  return 0
}

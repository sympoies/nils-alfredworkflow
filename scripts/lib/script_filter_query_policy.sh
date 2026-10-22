#!/usr/bin/env bash
# Shared Script Filter query normalization and minimum-length helpers.

sfqp_trim() {
  local value="${1-}"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

sfqp_resolve_query_input() {
  local query="${1-}"
  if [[ -z "$query" && -n "${alfred_workflow_query:-}" ]]; then
    query="${alfred_workflow_query}"
  elif [[ -z "$query" && -n "${ALFRED_WORKFLOW_QUERY:-}" ]]; then
    query="${ALFRED_WORKFLOW_QUERY}"
  elif [[ -z "$query" && ! -t 0 ]]; then
    query="$(cat)"
  fi

  printf '%s' "$query"
}

sfqp_query_length() {
  local query
  query="$(sfqp_trim "${1-}")"
  printf '%s' "${#query}"
}

sfqp_is_short_query() {
  local query="${1-}"
  local min_chars="${2:-2}"
  if [[ ! "$min_chars" =~ ^[0-9]+$ ]]; then
    min_chars=2
  fi

  local length
  length="$(sfqp_query_length "$query")"
  [[ "$length" -lt "$min_chars" ]]
}

sfqp_json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

sfqp_emit_non_actionable_item_json() {
  local title="${1:-Info}"
  local subtitle="${2:-}"
  printf '{"items":[{"title":"%s","subtitle":"%s","valid":false}]}' \
    "$(sfqp_json_escape "$title")" \
    "$(sfqp_json_escape "$subtitle")"
  printf '\n'
}

sfqp_emit_short_query_item_json() {
  local min_chars="${1:-2}"
  local title="${2:-Keep typing}"
  local subtitle_template="${3:-Type at least %s characters before continuing.}"

  if [[ ! "$min_chars" =~ ^[0-9]+$ ]]; then
    min_chars=2
  fi

  local subtitle=""
  # shellcheck disable=SC2059
  printf -v subtitle "$subtitle_template" "$min_chars"
  sfqp_emit_non_actionable_item_json "$title" "$subtitle"
}

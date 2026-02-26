#!/usr/bin/env bash
# Shared action-side requery primitives for region/language switch workflows.

wfar_resolve_cache_dir() {
  local fallback_key="${1:-nils-workflow-action-requery}"
  local candidate=""

  for candidate in \
    "${ALFRED_WORKFLOW_CACHE:-}" \
    "${ALFRED_WORKFLOW_DATA:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s/%s\n' "${TMPDIR:-/tmp}" "$fallback_key"
}

wfar_state_file_path() {
  local cache_dir="$1"
  local file_name="$2"
  printf '%s/%s\n' "$cache_dir" "$file_name"
}

wfar_parse_requery_payload() {
  if [[ $# -lt 2 ]]; then
    echo "wfar_parse_requery_payload requires: <arg> <prefix>" >&2
    return 2
  fi

  local arg="$1"
  local prefix="$2"

  if [[ -z "$arg" || -z "$prefix" || "$arg" != "$prefix"* ]]; then
    echo "invalid requery payload prefix" >&2
    return 2
  fi

  local payload selector query
  payload="${arg#"$prefix"}"
  selector="${payload%%:*}"
  if [[ -z "$selector" || "$payload" == "$selector" ]]; then
    echo "invalid requery payload" >&2
    return 2
  fi

  query="${payload#*:}"
  # shellcheck disable=SC2034
  WFAR_REQUERY_SELECTOR="$selector"
  # shellcheck disable=SC2034
  WFAR_REQUERY_QUERY="$query"
  return 0
}

wfar_write_state_file() {
  local state_file="$1"
  local value="$2"

  mkdir -p "$(dirname "$state_file")"
  printf '%s\n' "$value" >"$state_file"
}

wfar_escape_applescript_string() {
  local input="${1:-}"
  input="${input//\\/\\\\}"
  input="${input//\"/\\\"}"
  input="${input//$'\n'/ }"
  input="${input//$'\r'/ }"
  printf '%s' "$input"
}

wfar_build_keyword_requery_text() {
  local keyword="$1"
  local query="${2:-}"
  local requery_text="$keyword"
  if [[ -n "${query//[[:space:]]/}" ]]; then
    requery_text="$keyword $query"
  fi
  printf '%s\n' "$requery_text"
}

wfar_trigger_requery() {
  local query="$1"
  local command_override="${2:-}"
  local app_name="${3:-Alfred 5}"

  if [[ -n "$command_override" ]]; then
    "$command_override" "$query"
    return 0
  fi

  if command -v osascript >/dev/null 2>&1; then
    local escaped_query escaped_app_name
    escaped_query="$(wfar_escape_applescript_string "$query")"
    escaped_app_name="$(wfar_escape_applescript_string "$app_name")"
    osascript -e "tell application \"${escaped_app_name}\" to search \"${escaped_query}\""
    return 0
  fi

  echo "cannot trigger Alfred requery: set command override or install osascript support" >&2
  return 1
}

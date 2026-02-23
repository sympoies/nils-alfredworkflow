#!/usr/bin/env bash
# Shared non-search Script Filter CLI execution driver.
#
# Callbacks:
#   execute_fn [args...]
#     - runs workflow-local CLI logic
#     - prints Alfred JSON to stdout on success
#     - returns non-zero on failure (stderr is captured by this driver)
#   map_error_fn <raw-message>
#     - maps an error message to an Alfred error-row JSON payload

sfcd_json_escape() {
  local value="${1-}"
  value="$(printf '%s' "$value" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  value="$(printf '%s' "$value" | tr '\000-\037' ' ')"
  printf '%s' "$value"
}

sfcd_emit_fallback_error_row_json() {
  local raw_message="${1-script-filter command failed}"
  local message="$raw_message"

  if [[ -z "$message" ]]; then
    message="script-filter command failed"
  fi

  printf '{"items":[{"title":"Workflow runtime error","subtitle":"%s","valid":false}]}\n' \
    "$(sfcd_json_escape "$message")"
}

sfcd_json_has_items_array() {
  local payload="${1-}"

  if command -v jq >/dev/null 2>&1; then
    jq -e '.items | type == "array"' >/dev/null <<<"$payload"
    return $?
  fi

  # Keep a minimal structural guard when jq is unavailable.
  printf '%s' "$payload" | grep -Eq '"items"[[:space:]]*:[[:space:]]*\['
}

sfcd_emit_mapped_error_json() {
  local map_error_fn="${1-}"
  local raw_message="${2-}"
  local mapped_json=""

  if [[ -z "$raw_message" ]]; then
    raw_message="script-filter command failed"
  fi

  if [[ -n "$map_error_fn" ]] && declare -F "$map_error_fn" >/dev/null 2>&1; then
    if mapped_json="$("$map_error_fn" "$raw_message")" && [[ -n "$mapped_json" ]]; then
      if sfcd_json_has_items_array "$mapped_json"; then
        printf '%s\n' "$mapped_json"
        return 0
      fi
    fi
  fi

  sfcd_emit_fallback_error_row_json "$raw_message"
}

sfcd_run_cli_flow() {
  local execute_fn="${1-}"
  local map_error_fn="${2-}"
  local empty_output_message="${3-script-filter command returned empty response}"
  local malformed_json_message="${4-script-filter command returned malformed Alfred JSON}"
  local -a execute_args=()
  local err_file=""
  local err_msg=""
  local json_output=""

  if [[ $# -ge 5 ]]; then
    execute_args=("${@:5}")
  fi

  if [[ -z "$execute_fn" ]] || ! declare -F "$execute_fn" >/dev/null 2>&1; then
    sfcd_emit_mapped_error_json "$map_error_fn" "script-filter execute callback is not defined"
    return 0
  fi

  err_file="${TMPDIR:-/tmp}/script-filter-cli-driver.err.$$.$RANDOM"

  if json_output="$("$execute_fn" "${execute_args[@]}" 2>"$err_file")"; then
    if [[ -z "$json_output" ]]; then
      rm -f "$err_file"
      sfcd_emit_mapped_error_json "$map_error_fn" "$empty_output_message"
      return 0
    fi

    if ! sfcd_json_has_items_array "$json_output"; then
      rm -f "$err_file"
      sfcd_emit_mapped_error_json "$map_error_fn" "$malformed_json_message"
      return 0
    fi

    rm -f "$err_file"
    printf '%s\n' "$json_output"
    return 0
  fi

  err_msg="$(cat "$err_file" 2>/dev/null || true)"
  rm -f "$err_file"

  if [[ -z "$err_msg" ]]; then
    err_msg="script-filter command failed"
  fi

  sfcd_emit_mapped_error_json "$map_error_fn" "$err_msg"
  return 0
}

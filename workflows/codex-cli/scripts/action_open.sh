#!/usr/bin/env bash
set -euo pipefail

workflow_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runtime_meta="$workflow_script_dir/lib/codex_cli_runtime.sh"
if [[ ! -f "$runtime_meta" ]]; then
  echo "error: missing runtime metadata: $runtime_meta" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$runtime_meta"
# shellcheck disable=SC2153
codex_cli_pinned_version="${CODEX_CLI_PINNED_VERSION}"
# shellcheck disable=SC2153
codex_cli_pinned_crate="${CODEX_CLI_PINNED_CRATE}"

helper_loader=""
for candidate in \
  "$workflow_script_dir/lib/workflow_helper_loader.sh" \
  "$workflow_script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -n "$helper_loader" ]]; then
  # shellcheck disable=SC1090
  source "$helper_loader"
  wfhl_source_helper "$workflow_script_dir" "workflow_cli_resolver.sh" off || true
fi

if [[ "$#" -lt 1 || -z "${1:-}" ]]; then
  echo "usage: action_open.sh <action-token>" >&2
  exit 2
fi

clear_quarantine_if_needed() {
  local cli_path="$1"

  if declare -F wfcr_clear_workflow_quarantine_once_if_needed >/dev/null 2>&1; then
    wfcr_clear_workflow_quarantine_once_if_needed "$cli_path"
  fi

  if [[ "$(uname -s 2>/dev/null || printf '')" != "Darwin" ]]; then
    return 0
  fi

  if ! command -v xattr >/dev/null 2>&1; then
    return 0
  fi

  if xattr -p com.apple.quarantine "$cli_path" >/dev/null 2>&1; then
    xattr -d com.apple.quarantine "$cli_path" >/dev/null 2>&1 || true
  fi
}

trim() {
  local value="${1-}"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

expand_home_path() {
  local value="${1-}"

  case "$value" in
  "~")
    if [[ -n "${HOME:-}" ]]; then
      printf '%s\n' "${HOME%/}"
      return 0
    fi
    ;;
  \~/*)
    if [[ -n "${HOME:-}" ]]; then
      printf '%s/%s\n' "${HOME%/}" "${value#\~/}"
      return 0
    fi
    ;;
  esac

  printf '%s\n' "$value"
}

resolve_codex_cli_override() {
  local configured="${CODEX_CLI_BIN:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"
  [[ -n "$configured" ]] || return 1
  printf '%s\n' "$configured"
}

resolve_codex_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/codex-cli"
  local release_cli
  release_cli="$repo_root/target/release/codex-cli"
  local debug_cli
  debug_cli="$repo_root/target/debug/codex-cli"

  if declare -F wfcr_resolve_binary >/dev/null 2>&1; then
    wfcr_resolve_binary \
      "CODEX_CLI_BIN" \
      "$packaged_cli" \
      "$release_cli" \
      "$debug_cli" \
      "codex-cli binary not found (re-import workflow bundle, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version})"
    return $?
  fi

  local configured_cli=""
  configured_cli="$(resolve_codex_cli_override || true)"
  if [[ -n "$configured_cli" && -x "$configured_cli" ]]; then
    clear_quarantine_if_needed "$configured_cli"
    printf '%s\n' "$configured_cli"
    return 0
  fi

  if [[ -x "$packaged_cli" ]]; then
    clear_quarantine_if_needed "$packaged_cli"
    printf '%s\n' "$packaged_cli"
    return 0
  fi

  local resolved
  resolved="$(command -v codex-cli 2>/dev/null || true)"
  if [[ -n "$resolved" && -x "$resolved" ]]; then
    clear_quarantine_if_needed "$resolved"
    printf '%s\n' "$resolved"
    return 0
  fi

  echo "codex-cli binary not found (re-import workflow bundle, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version})" >&2
  return 1
}

notify() {
  local message="$1"
  local escaped
  escaped="$(printf '%s' "$message" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if command -v osascript >/dev/null 2>&1; then
    osascript -e "display notification \"$escaped\" with title \"Codex CLI Workflow\"" >/dev/null 2>&1 || true
  fi
}

save_confirmation_enabled() {
  local raw="${CODEX_SAVE_CONFIRM:-1}"
  raw="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]')"
  case "$raw" in
  0 | false | no | off)
    return 1
    ;;
  *)
    return 0
    ;;
  esac
}

remove_confirmation_enabled() {
  local raw="${CODEX_REMOVE_CONFIRM:-1}"
  raw="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]')"
  case "$raw" in
  0 | false | no | off)
    return 1
    ;;
  *)
    return 0
    ;;
  esac
}

confirm_save_if_needed() {
  local secret="$1"
  local yes_flag="$2"

  if [[ "$yes_flag" == "1" ]]; then
    return 0
  fi

  if ! save_confirmation_enabled; then
    return 0
  fi

  if ! command -v osascript >/dev/null 2>&1; then
    return 0
  fi

  local escaped_secret
  escaped_secret="$(printf '%s' "$secret" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if osascript >/dev/null 2>&1 <<EOF; then
tell application "System Events"
  activate
  display dialog "Save current auth to ${escaped_secret}?" buttons {"Cancel", "Save"} default button "Save" with icon caution
end tell
EOF
    return 0
  fi

  notify "Cancelled: auth save ${secret}"
  echo "auth save cancelled by user." >&2
  return 130
}

confirm_remove_if_needed() {
  local secret="$1"
  local yes_flag="$2"

  if [[ "$yes_flag" == "1" ]]; then
    return 0
  fi

  if ! remove_confirmation_enabled; then
    return 0
  fi

  if ! command -v osascript >/dev/null 2>&1; then
    return 0
  fi

  local escaped_secret
  escaped_secret="$(printf '%s' "$secret" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if osascript >/dev/null 2>&1 <<EOF; then
tell application "System Events"
  activate
  display dialog "Remove ${escaped_secret} from CODEX_SECRET_DIR?" buttons {"Cancel", "Remove"} default button "Remove" with icon caution
end tell
EOF
    return 0
  fi

  notify "Cancelled: auth remove ${secret}"
  echo "auth remove cancelled by user." >&2
  return 130
}

resolve_default_codex_secret_dir() {
  if [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
    printf '%s/codex_secrets\n' "${XDG_CONFIG_HOME%/}"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    printf '%s/.config/codex_secrets\n' "${HOME%/}"
    return 0
  fi

  return 1
}

resolve_codex_auth_file_env_value() {
  local configured="${CODEX_AUTH_FILE:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"

  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    printf '%s/.codex/auth.json\n' "${HOME%/}"
    return 0
  fi

  return 1
}

ensure_codex_auth_file_env() {
  local configured=""
  configured="$(resolve_codex_auth_file_env_value || true)"
  [[ -n "$configured" ]] || return 1
  export CODEX_AUTH_FILE="$configured"
  return 0
}

ensure_codex_secret_dir_env() {
  local configured="${CODEX_SECRET_DIR:-}"
  configured="$(trim "$configured")"

  if [[ -z "$configured" ]]; then
    configured="$(resolve_default_codex_secret_dir || true)"
  fi

  if [[ -z "$configured" ]]; then
    return 1
  fi

  configured="$(expand_home_path "$configured")"
  export CODEX_SECRET_DIR="$configured"
  printf '%s\n' "$configured"
}

ensure_codex_secret_dir_exists() {
  local resolved
  if ! resolved="$(ensure_codex_secret_dir_env)"; then
    echo "CODEX_SECRET_DIR is not configured and no default path could be derived." >&2
    return 1
  fi

  if mkdir -p "$resolved"; then
    return 0
  fi

  echo "CODEX_SECRET_DIR could not be created: $resolved" >&2
  return 1
}

ensure_remove_secret_exists() {
  local secret="$1"
  local secret_dir
  if ! secret_dir="$(ensure_codex_secret_dir_env 2>/dev/null)"; then
    echo "CODEX_SECRET_DIR is not configured; cannot remove ${secret}." >&2
    return 66
  fi

  local secret_path="${secret_dir%/}/${secret}"
  if [[ ! -f "$secret_path" ]]; then
    echo "secret file does not exist: ${secret_path}" >&2
    return 66
  fi

  return 0
}

secret_dir_has_saved_json() {
  local secret_dir="${1:-}"
  [[ -n "$secret_dir" ]] || return 1
  [[ -d "$secret_dir" ]] || return 1

  local any_file
  any_file="$(find "$secret_dir" -maxdepth 1 -type f -name '*.json' -print -quit 2>/dev/null || true)"
  [[ -n "$any_file" ]]
}

resolve_diag_scope_for_all() {
  local secret_dir
  secret_dir="$(ensure_codex_secret_dir_env || true)"
  if secret_dir_has_saved_json "$secret_dir"; then
    printf 'all\n'
  else
    printf 'current\n'
  fi
}

validate_use_secret_name() {
  local secret="$1"
  [[ -n "$secret" ]] || return 1
  [[ "$secret" =~ ^[A-Za-z0-9._@-]+$ ]]
}

resolve_workflow_cache_dir() {
  local candidate
  for candidate in \
    "${ALFRED_WORKFLOW_CACHE:-}" \
    "${ALFRED_WORKFLOW_DATA:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "${TMPDIR:-/tmp}/nils-codex-cli-workflow"
}

sanitize_diag_mode() {
  local mode="${1:-default}"
  mode="${mode//[^A-Za-z0-9._-]/_}"
  [[ -n "$mode" ]] || mode="default"
  printf '%s\n' "$mode"
}

diag_result_meta_path() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.last.meta\n' "$cache_dir"
}

diag_result_output_path() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.last.out\n' "$cache_dir"
}

diag_result_meta_path_all_json() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.all-json.meta\n' "$cache_dir"
}

diag_result_output_path_all_json() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.all-json.out\n' "$cache_dir"
}

store_diag_result() {
  local mode="$1"
  local summary="$2"
  local command="$3"
  local rc="$4"
  local output="$5"
  local normalized_mode
  normalized_mode="$(sanitize_diag_mode "$mode")"
  local timestamp
  timestamp="$(date +%s)"

  local last_meta_path last_output_path
  last_meta_path="$(diag_result_meta_path)"
  last_output_path="$(diag_result_output_path)"

  mkdir -p "$(dirname "$last_output_path")"
  {
    printf 'mode=%s\n' "$normalized_mode"
    printf 'summary=%s\n' "$summary"
    printf 'command=%s\n' "$command"
    printf 'exit_code=%s\n' "$rc"
    printf 'timestamp=%s\n' "$timestamp"
  } >"$last_meta_path"
  printf '%s\n' "$output" >"$last_output_path"

  if [[ "$normalized_mode" == "all-json" ]]; then
    local all_meta_path all_output_path
    all_meta_path="$(diag_result_meta_path_all_json)"
    all_output_path="$(diag_result_output_path_all_json)"
    mkdir -p "$(dirname "$all_output_path")"
    {
      printf 'mode=%s\n' "$normalized_mode"
      printf 'summary=%s\n' "$summary"
      printf 'command=%s\n' "$command"
      printf 'exit_code=%s\n' "$rc"
      printf 'timestamp=%s\n' "$timestamp"
    } >"$all_meta_path"
    printf '%s\n' "$output" >"$all_output_path"
  fi
}

open_alfred_search_best_effort() {
  local query="$1"
  local escaped
  escaped="$(printf '%s' "$query" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if ! command -v osascript >/dev/null 2>&1; then
    return 1
  fi

  run_with_timeout 2 osascript -e "tell application \"Alfred 5\" to search \"$escaped\"" >/dev/null 2>&1 ||
    run_with_timeout 2 osascript -e "tell application \"Alfred\" to search \"$escaped\"" >/dev/null 2>&1 ||
    true
  return 0
}

resolve_login_timeout_seconds() {
  local raw="${CODEX_LOGIN_TIMEOUT_SECONDS:-60}"
  if [[ "$raw" =~ ^[0-9]+$ ]] && [[ "$raw" -ge 1 ]] && [[ "$raw" -le 3600 ]]; then
    printf '%s\n' "$raw"
    return 0
  fi
  printf '60\n'
}

run_with_timeout() {
  local timeout_seconds="$1"
  shift

  if [[ "$timeout_seconds" -le 0 ]]; then
    "$@"
    return $?
  fi

  "$@" &
  local cmd_pid=$!
  local start_ts=$SECONDS

  while kill -0 "$cmd_pid" >/dev/null 2>&1; do
    if ((SECONDS - start_ts >= timeout_seconds)); then
      kill -TERM "$cmd_pid" >/dev/null 2>&1 || true
      sleep 2
      kill -KILL "$cmd_pid" >/dev/null 2>&1 || true
      wait "$cmd_pid" >/dev/null 2>&1 || true
      return 124
    fi
    sleep 0.2
  done

  wait "$cmd_pid"
  return $?
}

strip_ansi() {
  local line="${1:-}"
  printf '%s' "$line" | sed -E $'s/\\x1B\\[[0-9;]*[A-Za-z]//g'
}

extract_login_url() {
  local line="${1:-}"
  local clean
  clean="$(strip_ansi "$line")"
  local urls
  urls="$(printf '%s\n' "$clean" | grep -Eo 'https?://[^[:space:]<>()"]+' || true)"

  if [[ -z "$urls" ]]; then
    return 1
  fi

  local url
  while IFS= read -r url || [[ -n "$url" ]]; do
    url="${url%%[.,;:!?)]}"
    if [[ "$url" =~ ^https?://(localhost|127\.0\.0\.1)(:[0-9]+)?(/|$) ]]; then
      continue
    fi
    if [[ "$url" =~ ^https://(auth\.openai\.com|chatgpt\.com|openai\.com)(/|$) ]]; then
      printf '%s\n' "$url"
      return 0
    fi
  done <<<"$urls"

  while IFS= read -r url || [[ -n "$url" ]]; do
    url="${url%%[.,;:!?)]}"
    if [[ "$url" =~ ^https?://(localhost|127\.0\.0\.1)(:[0-9]+)?(/|$) ]]; then
      continue
    fi
    printf '%s\n' "$url"
    return 0
  done <<<"$urls"

  return 1
}

extract_device_code() {
  local line="${1:-}"
  local clean
  clean="$(strip_ansi "$line")"
  local code
  code="$(printf '%s\n' "$clean" | grep -Eo '[A-Z0-9]{3,8}-[A-Z0-9]{3,8}' | head -n1 || true)"
  if [[ -z "$code" ]]; then
    code="$(printf '%s\n' "$clean" | grep -Eo '[A-Z0-9]{6,12}' | head -n1 || true)"
  fi
  [[ -n "$code" ]] || return 1
  printf '%s\n' "$code"
}

open_url_best_effort() {
  local url="$1"
  if command -v open >/dev/null 2>&1; then
    open "$url" >/dev/null 2>&1 || true
    return 0
  fi
  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$url" >/dev/null 2>&1 || true
    return 0
  fi
  return 1
}

copy_to_clipboard_best_effort() {
  local value="$1"
  if command -v pbcopy >/dev/null 2>&1; then
    printf '%s' "$value" | pbcopy
    return 0
  fi
  if command -v wl-copy >/dev/null 2>&1; then
    printf '%s' "$value" | wl-copy
    return 0
  fi
  if command -v xclip >/dev/null 2>&1; then
    printf '%s' "$value" | xclip -selection clipboard
    return 0
  fi
  return 1
}

capture_command_output_with_stdout_priority() {
  local __out_var="$1"
  local __rc_var="$2"
  shift 2

  local stdout_file stderr_file
  stdout_file="$(mktemp "${TMPDIR:-/tmp}/codex-diag-stdout.XXXXXX")"
  stderr_file="$(mktemp "${TMPDIR:-/tmp}/codex-diag-stderr.XXXXXX")"

  local capture_rc=0
  set +e
  "$@" >"$stdout_file" 2>"$stderr_file"
  capture_rc=$?
  set -e

  local captured_stdout captured_stderr chosen_output
  captured_stdout="$(cat "$stdout_file" 2>/dev/null || true)"
  captured_stderr="$(cat "$stderr_file" 2>/dev/null || true)"
  rm -f "$stdout_file" "$stderr_file"

  chosen_output="$captured_stderr"
  if [[ -n "$captured_stdout" ]]; then
    chosen_output="$captured_stdout"
  fi

  printf -v "$__out_var" '%s' "$chosen_output"
  printf -v "$__rc_var" '%s' "$capture_rc"
}

run_codex_command() {
  local codex_cli="$1"
  local summary="$2"
  shift 2

  local output
  if output="$("$codex_cli" "$@" 2>&1)"; then
    notify "Success: ${summary}"
    if [[ -n "$output" ]]; then
      printf '%s\n' "$output"
    fi
    return 0
  else
    local rc=$?
    notify "Failed(${rc}): ${summary}"
    printf '%s\n' "$output" >&2
    return "$rc"
  fi
}

run_codex_diag_command() {
  local codex_cli="$1"
  local mode="$2"
  local summary="$3"
  local result_query="$4"
  shift 4

  local output=""
  local rc=0

  capture_command_output_with_stdout_priority output rc "$codex_cli" "$@"

  store_diag_result "$mode" "$summary" "$*" "$rc" "$output"
  open_alfred_search_best_effort "$result_query"

  if [[ "$rc" -eq 0 ]]; then
    notify "Diag ready: ${mode}"
    [[ -n "$output" ]] && printf '%s\n' "$output"
    return 0
  fi

  notify "Diag failed(${rc}): ${mode}"
  [[ -n "$output" ]] && printf '%s\n' "$output" >&2
  return "$rc"
}

run_codex_login_api_key() {
  local codex_cli="$1"
  local summary="auth login --api-key"
  local api_key="${CODEX_API_KEY:-}"
  local timeout_seconds
  timeout_seconds="$(resolve_login_timeout_seconds)"
  local output=""
  local rc=0

  if [[ -z "$api_key" ]]; then
    notify "Waiting: enter API key"
    if command -v osascript >/dev/null 2>&1; then
      if ! api_key="$(
        osascript <<'EOF'
tell application "System Events"
  activate
  display dialog "Enter OpenAI API key for codex-cli login" default answer "" with hidden answer buttons {"Cancel", "Login"} default button "Login"
  text returned of result
end tell
EOF
      )"; then
        notify "Cancelled: ${summary}"
        echo "codex-cli api-key login cancelled." >&2
        return 130
      fi
    fi
  fi

  api_key="$(printf '%s' "$api_key" | tr -d '\r' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
  if [[ -z "$api_key" ]]; then
    notify "Failed(64): ${summary}"
    echo "No API key provided. Set CODEX_API_KEY or enter key when prompted." >&2
    return 64
  fi

  set +e
  output="$(printf '%s\n' "$api_key" | run_with_timeout "$timeout_seconds" "$codex_cli" auth login --api-key 2>&1)"
  rc=$?
  set -e

  if [[ "$rc" -eq 0 ]]; then
    notify "Success: ${summary}"
    [[ -n "$output" ]] && printf '%s\n' "$output"
    return 0
  fi

  if [[ "$rc" -eq 124 ]]; then
    notify "Timed out(${timeout_seconds}s): ${summary}"
    echo "Login timed out after ${timeout_seconds}s. Set CODEX_LOGIN_TIMEOUT_SECONDS to adjust." >&2
    return 124
  fi

  notify "Failed(${rc}): ${summary}"
  [[ -n "$output" ]] && printf '%s\n' "$output" >&2
  return "$rc"
}

run_codex_login_browser() {
  local codex_cli="$1"
  local summary="auth login"
  local timeout_seconds
  timeout_seconds="$(resolve_login_timeout_seconds)"
  local browser_opened="0"

  notify "Starting: browser login (${timeout_seconds}s timeout)"
  set +e
  run_with_timeout "$timeout_seconds" "$codex_cli" auth login 2>&1 | while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "$line"
    if [[ "$browser_opened" == "0" ]]; then
      local login_url
      login_url="$(extract_login_url "$line" || true)"
      if [[ -n "$login_url" ]]; then
        open_url_best_effort "$login_url"
        browser_opened="1"
      fi
    fi
  done
  local rc=${PIPESTATUS[0]}
  set -e

  if [[ "$rc" -eq 0 ]]; then
    notify "Success: ${summary}"
    return 0
  fi

  if [[ "$rc" -eq 124 ]]; then
    notify "Timed out(${timeout_seconds}s): ${summary}"
    echo "Login timed out after ${timeout_seconds}s. Set CODEX_LOGIN_TIMEOUT_SECONDS to adjust." >&2
    return 124
  fi

  notify "Failed(${rc}): ${summary}"
  return "$rc"
}

run_codex_login_device_code() {
  local codex_cli="$1"
  local summary="auth login --device-code"
  local timeout_seconds
  timeout_seconds="$(resolve_login_timeout_seconds)"
  local browser_opened="0"
  local code_copied="0"

  notify "Starting: device-code login (${timeout_seconds}s timeout)"
  set +e
  run_with_timeout "$timeout_seconds" "$codex_cli" auth login --device-code 2>&1 | while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "$line"

    if [[ "$browser_opened" == "0" ]]; then
      local login_url
      login_url="$(extract_login_url "$line" || true)"
      if [[ -n "$login_url" ]]; then
        open_url_best_effort "$login_url"
        browser_opened="1"
      fi
    fi

    if [[ "$code_copied" == "0" ]]; then
      local device_code
      device_code="$(extract_device_code "$line" || true)"
      if [[ -n "$device_code" ]]; then
        copy_to_clipboard_best_effort "$device_code" || true
        notify "Device code copied: ${device_code}"
        code_copied="1"
      fi
    fi
  done
  local rc=${PIPESTATUS[0]}
  set -e

  if [[ "$rc" -eq 0 ]]; then
    notify "Success: ${summary}"
    return 0
  fi

  if [[ "$rc" -eq 124 ]]; then
    notify "Timed out(${timeout_seconds}s): ${summary}"
    echo "Login timed out after ${timeout_seconds}s. Set CODEX_LOGIN_TIMEOUT_SECONDS to adjust." >&2
    return 124
  fi

  notify "Failed(${rc}): ${summary}"
  return "$rc"
}

action_token="$1"
codex_cli=""
if ! codex_cli="$(resolve_codex_cli)"; then
  exit 1
fi
ensure_codex_auth_file_env >/dev/null 2>&1 || true
ensure_codex_secret_dir_env >/dev/null 2>&1 || true

case "$action_token" in
login::browser)
  run_codex_login_browser "$codex_cli"
  exit $?
  ;;
login::api-key)
  run_codex_login_api_key "$codex_cli"
  exit $?
  ;;
login::device-code)
  run_codex_login_device_code "$codex_cli"
  exit $?
  ;;
use::*)
  secret="${action_token#use::}"
  if ! validate_use_secret_name "$secret"; then
    echo "invalid use action token: $action_token" >&2
    exit 2
  fi
  run_codex_command "$codex_cli" "auth use $secret" auth use "$secret"
  exit $?
  ;;
save::*)
  payload="${action_token#save::}"
  secret="${payload%::*}"
  yes_flag="${payload##*::}"

  if [[ -z "$secret" || -z "$yes_flag" ]]; then
    echo "invalid save action token: $action_token" >&2
    exit 2
  fi

  if confirm_save_if_needed "$secret" "$yes_flag"; then
    :
  else
    exit $?
  fi

  if ! ensure_codex_secret_dir_exists; then
    notify "Failed: CODEX_SECRET_DIR missing"
    exit 1
  fi

  effective_yes_flag="$yes_flag"
  target_secret_path="${CODEX_SECRET_DIR%/}/${secret}"
  if [[ "$effective_yes_flag" != "1" && -f "$target_secret_path" ]]; then
    # A confirmed save on an existing file should behave as overwrite.
    if save_confirmation_enabled && command -v osascript >/dev/null 2>&1; then
      effective_yes_flag="1"
    fi
  fi

  if [[ "$effective_yes_flag" == "1" ]]; then
    run_codex_command "$codex_cli" "auth save --yes $secret" auth save --yes "$secret"
  else
    run_codex_command "$codex_cli" "auth save $secret" auth save "$secret"
  fi
  exit $?
  ;;
remove::*)
  payload="${action_token#remove::}"
  secret="${payload%::*}"
  yes_flag="${payload##*::}"

  if [[ -z "$secret" || -z "$yes_flag" ]]; then
    echo "invalid remove action token: $action_token" >&2
    exit 2
  fi

  if ensure_remove_secret_exists "$secret"; then
    :
  else
    remove_rc=$?
    notify "Failed(${remove_rc}): auth remove ${secret}"
    exit "$remove_rc"
  fi

  if confirm_remove_if_needed "$secret" "$yes_flag"; then
    :
  else
    exit $?
  fi

  effective_yes_flag="$yes_flag"
  if [[ "$effective_yes_flag" != "1" ]]; then
    # Alfred executes action_open non-interactively; after explicit dialog confirmation,
    # force --yes to bypass codex-cli's terminal confirmation prompt.
    if remove_confirmation_enabled && command -v osascript >/dev/null 2>&1; then
      effective_yes_flag="1"
    fi
  fi

  if [[ "$effective_yes_flag" == "1" ]]; then
    run_codex_command "$codex_cli" "auth remove --yes $secret" auth remove --yes "$secret"
  else
    run_codex_command "$codex_cli" "auth remove $secret" auth remove "$secret"
  fi
  exit $?
  ;;
diag::default)
  run_codex_diag_command "$codex_cli" "default" "diag rate-limits --json" "cxd result" diag rate-limits --json
  exit $?
  ;;
diag::cached)
  run_codex_diag_command "$codex_cli" "cached" "diag rate-limits --cached" "cxd result" diag rate-limits --cached
  exit $?
  ;;
diag::one-line)
  run_codex_diag_command "$codex_cli" "one-line" "diag rate-limits --one-line" "cxd result" diag rate-limits --one-line
  exit $?
  ;;
diag::all)
  if [[ "$(resolve_diag_scope_for_all)" == "all" ]]; then
    run_codex_diag_command "$codex_cli" "all" "diag rate-limits --all" "cxd result" diag rate-limits --all
  else
    run_codex_diag_command "$codex_cli" "default" "diag rate-limits (auth.json fallback)" "cxd result" diag rate-limits
  fi
  exit $?
  ;;
diag::all-json)
  if [[ "$(resolve_diag_scope_for_all)" == "all" ]]; then
    run_codex_diag_command "$codex_cli" "all-json" "diag rate-limits --all --json" "cxda result" diag rate-limits --all --json
  else
    run_codex_diag_command "$codex_cli" "all-json" "diag rate-limits --json (auth.json)" "cxda result" diag rate-limits --json
  fi
  exit $?
  ;;
diag::async)
  if [[ "$(resolve_diag_scope_for_all)" == "all" ]]; then
    run_codex_diag_command "$codex_cli" "async" "diag rate-limits --all --async --jobs 4" "cxd result" diag rate-limits --all --async --jobs 4
  else
    run_codex_diag_command "$codex_cli" "default" "diag rate-limits (auth.json fallback)" "cxd result" diag rate-limits
  fi
  exit $?
  ;;
*)
  echo "unknown action token: $action_token" >&2
  exit 2
  ;;
esac

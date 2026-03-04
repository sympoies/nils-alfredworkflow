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

if [[ -z "$helper_loader" ]]; then
  echo "Workflow helper missing: Cannot locate workflow_helper_loader.sh runtime helper." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$helper_loader"

wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" off || true

if [[ "$#" -lt 1 || -z "${1:-}" ]]; then
  echo "usage: action_open.sh <action-token>" >&2
  exit 2
fi

action_token="$1"

trim() {
  local value="${1-}"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

to_lower() {
  printf '%s' "${1-}" | tr '[:upper:]' '[:lower:]'
}

is_truthy() {
  local value
  value="$(to_lower "${1-}")"
  case "$value" in
  1 | true | yes | on)
    return 0
    ;;
  *)
    return 1
    ;;
  esac
}

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
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

resolve_google_cli_override() {
  local configured="${GOOGLE_CLI_BIN:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"
  [[ -n "$configured" ]] || return 1
  printf '%s\n' "$configured"
}

resolve_google_cli() {
  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local configured
  configured="$(resolve_google_cli_override || true)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/google-cli"

  local release_cli
  release_cli="$repo_root/target/release/google-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/google-cli"

  if declare -F wfcr_resolve_binary >/dev/null 2>&1; then
    wfcr_resolve_binary \
      "GOOGLE_CLI_BIN" \
      "$packaged_cli" \
      "$release_cli" \
      "$debug_cli" \
      "google-cli binary not found (set GOOGLE_CLI_BIN, install nils-google-cli, or build local target)"
    return $?
  fi

  if [[ -n "$configured" && -x "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  if [[ -x "$packaged_cli" ]]; then
    printf '%s\n' "$packaged_cli"
    return 0
  fi

  if [[ -x "$release_cli" ]]; then
    printf '%s\n' "$release_cli"
    return 0
  fi

  if [[ -x "$debug_cli" ]]; then
    printf '%s\n' "$debug_cli"
    return 0
  fi

  return 1
}

apply_google_cli_env_overrides() {
  local resolved_config_dir=""
  resolved_config_dir="$(resolve_google_cli_config_dir_env || true)"
  if [[ -n "$resolved_config_dir" ]]; then
    export GOOGLE_CLI_CONFIG_DIR="$resolved_config_dir"
  fi

  if [[ -n "${GOOGLE_CLI_KEYRING_MODE:-}" ]]; then
    export GOOGLE_CLI_KEYRING_MODE
  fi
}

resolve_google_cli_config_dir_env() {
  local configured="${GOOGLE_CLI_CONFIG_DIR:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"

  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    local legacy_config_dir
    legacy_config_dir="${HOME%/}/.config/google/credentials"
    if [[ -d "$legacy_config_dir" ]]; then
      printf '%s\n' "$legacy_config_dir"
      return 0
    fi
  fi

  return 1
}

resolve_workflow_data_dir() {
  local candidate
  for candidate in \
    "${ALFRED_WORKFLOW_DATA:-}" \
    "${ALFRED_WORKFLOW_CACHE:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "${TMPDIR:-/tmp}/nils-google-service-workflow"
}

resolve_active_account_file() {
  local data_dir
  data_dir="$(resolve_workflow_data_dir)"
  printf '%s/active-account.v1.json\n' "$data_dir"
}

write_active_account() {
  local account="$1"
  local active_file
  active_file="$(resolve_active_account_file)"
  mkdir -p "$(dirname "$active_file")"

  local now
  now="$(date +%s)"
  local temp_file
  temp_file="${active_file}.tmp.$RANDOM"

  cat >"$temp_file" <<JSON
{"version":1,"active_account":"$(json_escape "$account")","updated_at_epoch":${now}}
JSON

  mv "$temp_file" "$active_file"
}

clear_active_account() {
  local active_file
  active_file="$(resolve_active_account_file)"
  rm -f "$active_file"
}

resolve_drive_download_dir() {
  local configured="${GOOGLE_DRIVE_DOWNLOAD_DIR:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"
  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    printf '%s/Downloads\n' "${HOME%/}"
    return 0
  fi

  printf '%s/downloads\n' "$(resolve_workflow_data_dir)"
}

sanitize_download_file_name() {
  local name="${1-}"
  local fallback="${2-}"

  name="${name//\//_}"
  name="${name//\\/}"
  name="${name//$'\n'/ }"
  name="${name//$'\r'/ }"
  name="$(trim "$name")"

  if [[ -z "$name" ]]; then
    printf '%s\n' "$fallback"
    return 0
  fi

  printf '%s\n' "$name"
}

resolve_unique_download_path() {
  local dir="$1"
  local file_name="$2"

  local candidate="$dir/$file_name"
  if [[ ! -e "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi

  local stem="$file_name"
  local ext=""
  if [[ "$file_name" == *.* && "$file_name" != .* ]]; then
    stem="${file_name%.*}"
    ext=".${file_name##*.}"
  fi

  local index=1
  while true; do
    candidate="$dir/${stem} (${index})${ext}"
    if [[ ! -e "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
    index=$((index + 1))
  done
}

is_google_apps_mime() {
  local mime_type="${1-}"
  [[ "$mime_type" == application/vnd.google-apps.* ]]
}

resolve_drive_export_format() {
  local mime_type="${1-}"
  case "$mime_type" in
  application/vnd.google-apps.document)
    printf 'docx\n'
    ;;
  application/vnd.google-apps.spreadsheet)
    printf 'xlsx\n'
    ;;
  application/vnd.google-apps.presentation)
    printf 'pptx\n'
    ;;
  application/vnd.google-apps.folder | application/vnd.google-apps.shortcut)
    printf '\n'
    ;;
  application/vnd.google-apps.*)
    printf 'pdf\n'
    ;;
  *)
    printf '\n'
    ;;
  esac
}

append_export_extension_if_needed() {
  local file_name="${1-}"
  local export_format="${2-}"

  if [[ -z "$export_format" ]]; then
    printf '%s\n' "$file_name"
    return 0
  fi

  if [[ "$file_name" == *.* ]]; then
    printf '%s\n' "$file_name"
    return 0
  fi

  printf '%s.%s\n' "$file_name" "$export_format"
}

read_active_account() {
  local active_file
  active_file="$(resolve_active_account_file)"
  [[ -f "$active_file" ]] || return 1
  command -v jq >/dev/null 2>&1 || return 1

  local account
  account="$(jq -r '.active_account // empty' "$active_file" 2>/dev/null || true)"
  [[ -n "$account" ]] || return 1
  printf '%s\n' "$account"
}

notify() {
  local message="$1"
  local escaped
  escaped="$(printf '%s' "$message" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if command -v osascript >/dev/null 2>&1; then
    osascript -e "display notification \"$escaped\" with title \"Google Service Workflow\"" >/dev/null 2>&1 || true
  fi
}

notify_failure() {
  local message="$1"
  notify "Failed: ${message}"
}

fail_with_notify() {
  local message="$1"
  local rc="${2:-1}"
  notify_failure "$message"
  echo "$message" >&2
  return "$rc"
}

die_with_notify() {
  local message="$1"
  local rc="${2:-1}"
  notify_failure "$message"
  echo "$message" >&2
  exit "$rc"
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

open_alfred_search_best_effort() {
  local query="$1"
  local escaped
  escaped="$(printf '%s' "$query" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if ! command -v osascript >/dev/null 2>&1; then
    return 1
  fi

  osascript -e "tell application \"Alfred 5\" to search \"$escaped\"" >/dev/null 2>&1 ||
    osascript -e "tell application \"Alfred\" to search \"$escaped\"" >/dev/null 2>&1 || true
}

url_decode() {
  local encoded="${1-}"
  if [[ -z "$encoded" ]]; then
    printf '\n'
    return 0
  fi

  local decoded
  decoded="$(printf '%b' "${encoded//%/\\x}" 2>/dev/null || true)"
  if [[ -n "$decoded" ]]; then
    printf '%s\n' "$decoded"
    return 0
  fi

  printf '%s\n' "$encoded"
}

url_encode() {
  local raw="${1-}"
  if [[ -z "$raw" ]]; then
    printf '\n'
    return 0
  fi

  if command -v jq >/dev/null 2>&1; then
    jq -rn --arg value "$raw" '$value | @uri'
    return 0
  fi

  if command -v python3 >/dev/null 2>&1; then
    python3 - "$raw" <<'PY'
import sys
import urllib.parse

print(urllib.parse.quote(sys.argv[1], safe=""))
PY
    return 0
  fi

  printf '%s\n' "${raw// /%20}"
}

run_google_json_capture() {
  local __out_var="$1"
  local __rc_var="$2"
  local google_cli="$3"
  shift 3

  apply_google_cli_env_overrides

  local captured_output=""
  local captured_rc=0
  set +e
  captured_output="$("$google_cli" --json "$@" 2>&1)"
  captured_rc=$?
  set -e

  printf -v "$__out_var" '%s' "$captured_output"
  printf -v "$__rc_var" '%s' "$captured_rc"
}

extract_error_message() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    local message
    message="$(printf '%s\n' "$payload" | jq -r '.error.message // empty' 2>/dev/null || true)"
    if [[ -n "$message" ]]; then
      printf '%s\n' "$message"
      return 0
    fi
  fi

  printf '%s\n' "$payload" | tail -n1
}

confirm_remove_enabled() {
  local raw="${GOOGLE_AUTH_REMOVE_CONFIRM:-1}"
  if is_truthy "$raw"; then
    return 0
  fi
  return 1
}

confirm_remove_if_needed() {
  local account="$1"
  local yes_flag="$2"

  if [[ "$yes_flag" == "1" ]]; then
    return 0
  fi

  if ! confirm_remove_enabled; then
    return 0
  fi

  if ! command -v osascript >/dev/null 2>&1; then
    return 0
  fi

  local escaped_account
  escaped_account="$(printf '%s' "$account" | sed 's/\\/\\\\/g; s/"/\\"/g')"

  if osascript >/dev/null 2>&1 <<EOF2; then
tell application "System Events"
  activate
  display dialog "Remove Google account ${escaped_account}?" buttons {"Cancel", "Remove"} default button "Remove" with icon caution
end tell
EOF2
    return 0
  fi

  notify "Cancelled: remove ${account}"
  echo "auth remove cancelled by user." >&2
  return 130
}

load_accounts_from_json() {
  local payload="$1"
  local __accounts_var="$2"
  local __default_var="$3"

  local account_lines=""
  local default_account=""

  if command -v jq >/dev/null 2>&1; then
    account_lines="$(printf '%s\n' "$payload" | jq -r '.result.accounts[]?' 2>/dev/null || true)"
    default_account="$(printf '%s\n' "$payload" | jq -r '.result.default_account // empty' 2>/dev/null || true)"
  fi

  printf -v "$__accounts_var" '%s' "$account_lines"
  printf -v "$__default_var" '%s' "$default_account"
}

rebalance_active_account() {
  local google_cli="$1"

  local output rc
  run_google_json_capture output rc "$google_cli" auth list
  if [[ "$rc" -ne 0 ]]; then
    return 0
  fi

  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  if ! printf '%s\n' "$output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    return 0
  fi

  local default_account
  default_account="$(printf '%s\n' "$output" | jq -r '.result.default_account // empty')"

  local stored_active
  stored_active="$(read_active_account || true)"

  local selected=""
  if [[ -n "$stored_active" ]] && printf '%s\n' "$output" | jq -e --arg account "$stored_active" '.result.accounts | index($account) != null' >/dev/null 2>&1; then
    selected="$stored_active"
  elif [[ -n "$default_account" ]] && printf '%s\n' "$output" | jq -e --arg account "$default_account" '.result.accounts | index($account) != null' >/dev/null 2>&1; then
    selected="$default_account"
  else
    selected="$(printf '%s\n' "$output" | jq -r '.result.accounts[0] // empty')"
  fi

  if [[ -n "$selected" ]]; then
    write_active_account "$selected"
  else
    clear_active_account
  fi
}

handle_switch() {
  local google_cli="$1"
  local account="$2"

  if [[ -z "$account" ]]; then
    fail_with_notify "invalid switch action token" 2
    return
  fi

  local output rc
  run_google_json_capture output rc "$google_cli" auth list
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "switch ${account}: ${message}" "$rc"
    return
  fi

  if command -v jq >/dev/null 2>&1; then
    if ! printf '%s\n' "$output" | jq -e --arg account "$account" '.result.accounts | index($account) != null' >/dev/null 2>&1; then
      fail_with_notify "account not found in auth list: ${account}" 66
      return
    fi
  fi

  write_active_account "$account"
  notify "Active account: ${account}"
  open_alfred_search_best_effort "gsa"
  return 0
}

handle_login_remote_step1() {
  local google_cli="$1"
  local account="$2"

  local output rc
  run_google_json_capture output rc "$google_cli" auth add "$account" --remote --step 1
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "remote step 1 (${account}): ${message}" "$rc"
    return
  fi

  local auth_url=""
  local state=""
  if command -v jq >/dev/null 2>&1; then
    auth_url="$(printf '%s\n' "$output" | jq -r '.result.authorization_url // empty' 2>/dev/null || true)"
    state="$(printf '%s\n' "$output" | jq -r '.result.state // empty' 2>/dev/null || true)"
  fi

  if [[ -n "$auth_url" ]]; then
    open_url_best_effort "$auth_url" || true
  fi

  if [[ -n "$state" ]]; then
    copy_to_clipboard_best_effort "$state" || true
  fi
  notify "Remote step 1 ready (${account}) - paste callback URL in Alfred"
  open_alfred_search_best_effort "gsa "

  printf '%s\n' "$output"
  return 0
}

handle_login_remote_step2() {
  local google_cli="$1"
  local account="$2"
  local state="$3"
  local code="$4"

  state="$(url_decode "$state")"
  code="$(url_decode "$code")"

  local output rc
  run_google_json_capture output rc "$google_cli" auth add "$account" --remote --step 2 --state "$state" --code "$code"
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "remote step 2 (${account}): ${message}" "$rc"
    return
  fi

  write_active_account "$account"
  notify "Login success: ${account}"
  open_alfred_search_best_effort "gsa"
  printf '%s\n' "$output"
  return 0
}

handle_login_manual() {
  local google_cli="$1"
  local account="$2"
  local code="$3"

  code="$(url_decode "$code")"

  local output rc
  run_google_json_capture output rc "$google_cli" auth add "$account" --manual --code "$code"
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "manual login (${account}): ${message}" "$rc"
    return
  fi

  write_active_account "$account"
  notify "Login success: ${account}"
  open_alfred_search_best_effort "gsa"
  printf '%s\n' "$output"
  return 0
}

handle_remove() {
  local google_cli="$1"
  local account="$2"
  local yes_flag="$3"

  if ! confirm_remove_if_needed "$account" "$yes_flag"; then
    return $?
  fi

  local output rc
  run_google_json_capture output rc "$google_cli" auth remove "$account"
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "remove ${account}: ${message}" "$rc"
    return
  fi

  rebalance_active_account "$google_cli"
  notify "Removed: ${account}"
  open_alfred_search_best_effort "gsa"
  printf '%s\n' "$output"
  return 0
}

handle_drive_download() {
  local google_cli="$1"
  local file_id="$2"
  local search_count="${3-}"

  if [[ -z "$file_id" ]]; then
    fail_with_notify "invalid drive download action token" 2
    return
  fi

  local file_name="$file_id"
  local mime_type=""
  local metadata_output metadata_rc
  run_google_json_capture metadata_output metadata_rc "$google_cli" drive get "$file_id"
  if [[ "$metadata_rc" -eq 0 ]] && command -v jq >/dev/null 2>&1; then
    if printf '%s\n' "$metadata_output" | jq -e '.ok == true and (.result.file | type == "object")' >/dev/null 2>&1; then
      local extracted_name
      extracted_name="$(printf '%s\n' "$metadata_output" | jq -r '.result.file.name // empty' 2>/dev/null || true)"
      if [[ -n "$extracted_name" ]]; then
        file_name="$extracted_name"
      fi
      mime_type="$(printf '%s\n' "$metadata_output" | jq -r '.result.file.mime_type // empty' 2>/dev/null || true)"
    fi
  fi

  if [[ "$mime_type" == "application/vnd.google-apps.folder" || "$mime_type" == "application/vnd.google-apps.shortcut" ]]; then
    fail_with_notify "drive download ${file_id}: folders/shortcuts cannot be downloaded" 65
    return
  fi

  local export_format=""
  if is_google_apps_mime "$mime_type"; then
    export_format="$(resolve_drive_export_format "$mime_type")"
    if [[ -z "$export_format" ]]; then
      fail_with_notify "drive download ${file_id}: unsupported Google Docs export type (${mime_type})" 65
      return
    fi
  fi

  local download_dir
  download_dir="$(resolve_drive_download_dir)"
  if ! mkdir -p "$download_dir"; then
    fail_with_notify "failed to create download directory: ${download_dir}" 1
    return
  fi

  local safe_name
  safe_name="$(sanitize_download_file_name "$file_name" "$file_id")"
  safe_name="$(append_export_extension_if_needed "$safe_name" "$export_format")"
  local output_path
  output_path="$(resolve_unique_download_path "$download_dir" "$safe_name")"

  local -a command_args=(drive download "$file_id")
  if [[ -n "$export_format" ]]; then
    command_args+=(--format "$export_format")
  fi
  command_args+=(--out "$output_path")

  local output rc
  run_google_json_capture output rc "$google_cli" "${command_args[@]}"
  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$output")"
    fail_with_notify "drive download ${file_id}: ${message}" "$rc"
    return
  fi

  local count_suffix=""
  if [[ -n "$search_count" ]]; then
    count_suffix=" · result.count=${search_count}"
  fi
  notify "Downloaded: ${safe_name}${count_suffix}"
  printf '%s\n' "$output"
  return 0
}

handle_drive_open_home() {
  local url="https://drive.google.com/drive/home"
  open_url_best_effort "$url" || true
  notify "Opened Google Drive home"
  return 0
}

handle_drive_open_search() {
  local search_query="$1"
  search_query="$(trim "$search_query")"
  if [[ -z "$search_query" ]]; then
    fail_with_notify "drive search query is empty" 2
    return
  fi

  local encoded_query
  encoded_query="$(url_encode "$search_query")"
  local url="https://drive.google.com/drive/search?q=${encoded_query}"
  open_url_best_effort "$url" || true
  notify "Opened Drive search: ${search_query}"
  return 0
}

handle_gmail_open_home() {
  local url="https://mail.google.com/mail/u/0/#inbox"
  open_url_best_effort "$url" || true
  notify "Opened Gmail inbox"
  return 0
}

handle_gmail_open_search() {
  local search_query="$1"
  search_query="$(trim "$search_query")"
  if [[ -z "$search_query" ]]; then
    fail_with_notify "gmail search query is empty" 2
    return
  fi

  local encoded_query
  encoded_query="$(url_encode "$search_query")"
  local url="https://mail.google.com/mail/u/0/#search/${encoded_query}"
  open_url_best_effort "$url" || true
  notify "Opened Gmail search: ${search_query}"
  return 0
}

handle_gmail_open_message() {
  local message_id="$1"
  message_id="$(trim "$message_id")"
  if [[ -z "$message_id" ]]; then
    fail_with_notify "gmail message id is empty" 2
    return
  fi

  local encoded_id
  encoded_id="$(url_encode "$message_id")"
  local url="https://mail.google.com/mail/u/0/#all/${encoded_id}"
  open_url_best_effort "$url" || true
  notify "Opened Gmail message: ${message_id}"
  return 0
}

require_google_cli() {
  local resolved=""
  if ! resolved="$(resolve_google_cli)"; then
    die_with_notify "google-cli binary not found (set GOOGLE_CLI_BIN or install nils-google-cli)." 1
  fi
  printf '%s\n' "$resolved"
}

case "$action_token" in
prompt::auth)
  open_alfred_search_best_effort "gsa "
  ;;
prompt::login)
  open_alfred_search_best_effort "gsa login "
  ;;
prompt::switch)
  open_alfred_search_best_effort "gsa switch "
  ;;
prompt::remove)
  open_alfred_search_best_effort "gsa remove "
  ;;
prompt::mail-unread)
  open_alfred_search_best_effort "gsm unread"
  ;;
prompt::mail-unread-account::*)
  account="${action_token#prompt::mail-unread-account::}"
  account="$(trim "$account")"
  if [[ -z "$account" ]]; then
    die_with_notify "mail unread account token is missing account email" 2
  fi
  open_alfred_search_best_effort "gsm unread --account ${account}"
  ;;
switch::*)
  google_cli="$(require_google_cli)"
  account="${action_token#switch::}"
  handle_switch "$google_cli" "$account"
  ;;
login::remote::step1::*)
  google_cli="$(require_google_cli)"
  account="${action_token#login::remote::step1::}"
  handle_login_remote_step1 "$google_cli" "$account"
  ;;
login::remote::step2::*)
  google_cli="$(require_google_cli)"
  payload="${action_token#login::remote::step2::}"
  account="${payload%%::*}"
  payload="${payload#*::}"
  state="${payload%%::*}"
  code="${payload#*::}"
  if [[ -z "$account" || -z "$state" || -z "$code" ]]; then
    die_with_notify "invalid login remote step2 token" 2
  fi
  handle_login_remote_step2 "$google_cli" "$account" "$state" "$code"
  ;;
login::manual::*)
  google_cli="$(require_google_cli)"
  payload="${action_token#login::manual::}"
  account="${payload%%::*}"
  code="${payload#*::}"
  if [[ -z "$account" || -z "$code" ]]; then
    die_with_notify "invalid login manual token" 2
  fi
  handle_login_manual "$google_cli" "$account" "$code"
  ;;
remove::*)
  google_cli="$(require_google_cli)"
  payload="${action_token#remove::}"
  account="${payload%%::*}"
  yes_flag="${payload##*::}"
  if [[ -z "$account" || -z "$yes_flag" ]]; then
    die_with_notify "invalid remove token" 2
  fi
  handle_remove "$google_cli" "$account" "$yes_flag"
  ;;
drive-download::*)
  google_cli="$(require_google_cli)"
  payload="${action_token#drive-download::}"
  file_id="${payload%%::*}"
  search_count=""
  if [[ "$payload" == *"::"* ]]; then
    search_count="${payload#*::}"
  fi
  if [[ -z "$file_id" ]]; then
    die_with_notify "invalid drive download token" 2
  fi
  handle_drive_download "$google_cli" "$file_id" "$search_count"
  ;;
drive-open-home)
  handle_drive_open_home
  ;;
drive-open-search::*)
  search_query="${action_token#drive-open-search::}"
  handle_drive_open_search "$search_query"
  ;;
gmail-open-home)
  handle_gmail_open_home
  ;;
gmail-open-search::*)
  search_query="${action_token#gmail-open-search::}"
  handle_gmail_open_search "$search_query"
  ;;
gmail-open-message::*)
  message_id="${action_token#gmail-open-message::}"
  handle_gmail_open_message "$message_id"
  ;;
*)
  die_with_notify "unknown action token: $action_token" 2
  ;;
esac

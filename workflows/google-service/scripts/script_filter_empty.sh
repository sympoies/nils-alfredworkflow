#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

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
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}'
  exit 0
fi

# shellcheck disable=SC1090
source "$helper_loader"

load_helper_or_exit() {
  local helper_name="$1"
  if ! wfhl_source_helper "$script_dir" "$helper_name" auto; then
    wfhl_emit_missing_helper_item_json "$helper_name"
    exit 0
  fi
}

load_helper_or_exit "workflow_cli_resolver.sh"

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

begin_items() {
  _items_started=1
  _item_count=0
  printf '{"items":['
}

emit_action_item() {
  local title="$1"
  local subtitle="$2"
  local arg="$3"

  [[ "${_items_started:-0}" -eq 1 ]] || return 1

  if [[ "${_item_count:-0}" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":true,"arg":"%s"}' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$(json_escape "$arg")"

  _item_count=$((_item_count + 1))
}

end_items() {
  if [[ "${_items_started:-0}" -eq 1 ]]; then
    printf ']}'
  else
    printf '{"items":[]}'
  fi
}

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
  value="$(to_lower "$(trim "${1-}")")"
  case "$value" in
  1 | true | yes | on)
    return 0
    ;;
  *)
    return 1
    ;;
  esac
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
  local payload="${1-}"
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

read_active_account() {
  command -v jq >/dev/null 2>&1 || return 1

  local data_dir
  data_dir="$(resolve_workflow_data_dir)"
  local active_file="$data_dir/active-account.v1.json"
  [[ -f "$active_file" ]] || return 1

  local active
  active="$(jq -r '.active_account // empty' "$active_file" 2>/dev/null || true)"
  [[ -n "$active" ]] || return 1
  printf '%s\n' "$active"
}

resolve_google_cli() {
  wfcr_resolve_binary \
    "GOOGLE_CLI_BIN" \
    "$script_dir/../bin/google-cli" \
    "$repo_root/target/release/google-cli" \
    "$repo_root/target/debug/google-cli" \
    "google-cli binary not found (set GOOGLE_CLI_BIN or install nils-google-cli)"
}

read_auth_list_payload() {
  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local google_cli
  if ! google_cli="$(resolve_google_cli 2>/dev/null)"; then
    return 1
  fi

  local output rc
  run_google_json_capture output rc "$google_cli" auth list
  if [[ "$rc" -ne 0 ]]; then
    return 1
  fi

  if ! printf '%s\n' "$output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    return 1
  fi

  printf '%s\n' "$output"
}

read_default_account() {
  local auth_list_payload=""
  auth_list_payload="$(read_auth_list_payload || true)"
  [[ -n "$auth_list_payload" ]] || return 1

  local account
  account="$(printf '%s\n' "$auth_list_payload" | jq -r '.result.default_account // empty' 2>/dev/null || true)"
  [[ -n "$account" ]] || return 1
  printf '%s\n' "$account"
}

join_with_dot() {
  local IFS=' · '
  printf '%s' "$*"
}

emit_all_accounts_unread_summary_row() {
  if ! command -v jq >/dev/null 2>&1; then
    emit_action_item "Unread mail (all accounts): unavailable" "jq is required to parse google-cli output" "prompt::mail-unread"
    return
  fi

  local google_cli
  if ! google_cli="$(resolve_google_cli 2>/dev/null)"; then
    emit_action_item "Unread mail (all accounts): unavailable" "google-cli binary not found" "prompt::auth"
    return
  fi

  local auth_output auth_rc
  run_google_json_capture auth_output auth_rc "$google_cli" auth list
  if [[ "$auth_rc" -ne 0 ]]; then
    local message
    message="$(extract_error_message "$auth_output")"
    [[ -n "$message" ]] || message="failed to read account list"
    emit_action_item "Unread mail (all accounts): unavailable" "$message" "prompt::auth"
    return
  fi

  if ! printf '%s\n' "$auth_output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    local message
    message="$(extract_error_message "$auth_output")"
    [[ -n "$message" ]] || message="unexpected auth list response"
    emit_action_item "Unread mail (all accounts): unavailable" "$message" "prompt::auth"
    return
  fi

  local -a accounts=()
  mapfile -t accounts < <(printf '%s\n' "$auth_output" | jq -r '.result.accounts[]?' 2>/dev/null || true)

  if [[ "${#accounts[@]}" -eq 0 ]]; then
    emit_action_item "Unread mail (all accounts): 0" "No configured accounts" "prompt::login"
    return
  fi

  local total_unread=0
  local -a details=()
  local -a account_row_titles=()
  local -a account_row_subtitles=()
  local -a account_row_args=()
  local account
  for account in "${accounts[@]}"; do
    local search_output search_rc
    run_google_json_capture search_output search_rc "$google_cli" -a "$account" gmail search --max 500 --format minimal --query "in:inbox is:unread"

    if [[ "$search_rc" -ne 0 ]]; then
      details+=("${account}:err")
      account_row_titles+=("Unread ${account}: unavailable")
      account_row_subtitles+=("Count unavailable in summary · Open unread list for this account")
      account_row_args+=("prompt::mail-unread-account::${account}")
      continue
    fi

    if ! printf '%s\n' "$search_output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
      details+=("${account}:err")
      account_row_titles+=("Unread ${account}: unavailable")
      account_row_subtitles+=("Count unavailable in summary · Open unread list for this account")
      account_row_args+=("prompt::mail-unread-account::${account}")
      continue
    fi

    local count
    count="$(printf '%s\n' "$search_output" | jq -r '.result.count // 0' 2>/dev/null || true)"
    if ! [[ "$count" =~ ^[0-9]+$ ]]; then
      count="0"
    fi

    total_unread=$((total_unread + count))
    details+=("${account}:${count}")
    if ((count > 0)); then
      account_row_titles+=("Unread ${account}: ${count}")
      account_row_subtitles+=("Open unread list for this account (current account unchanged)")
      account_row_args+=("prompt::mail-unread-account::${account}")
    fi
  done

  local subtitle
  subtitle="$(join_with_dot "${details[@]}")"
  [[ -n "$subtitle" ]] || subtitle="No unread summary available"

  emit_action_item "Unread mail (all accounts): ${total_unread}" "$subtitle" "prompt::mail-unread"

  local index
  for index in "${!account_row_titles[@]}"; do
    emit_action_item \
      "${account_row_titles[$index]}" \
      "${account_row_subtitles[$index]}" \
      "${account_row_args[$index]}"
  done
}

begin_items

active_account="$(read_active_account || true)"
if [[ -n "$active_account" ]]; then
  emit_action_item "Current account: ${active_account}" "Workflow active account" "prompt::switch"
else
  default_account="$(read_default_account || true)"
  if [[ -n "$default_account" ]]; then
    emit_action_item "Current account: ${default_account}" "google-cli default account (active not set)" "prompt::switch"
  else
    emit_action_item "Current account: (none)" "Run gsa login or gsa switch to set account" "prompt::auth"
  fi
fi

if is_truthy "${GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD:-0}"; then
  emit_all_accounts_unread_summary_row
fi

end_items

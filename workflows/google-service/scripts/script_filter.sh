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

load_helper_or_exit "script_filter_error_json.sh"
load_helper_or_exit "workflow_cli_resolver.sh"
load_helper_or_exit "script_filter_query_policy.sh"

if ! declare -F sfqp_trim >/dev/null 2>&1; then
  sfqp_trim() {
    local value="${1-}"
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    printf '%s' "$value"
  }
fi

if ! declare -F sfqp_resolve_query_input >/dev/null 2>&1; then
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
fi

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

emit_item() {
  local title="$1"
  local subtitle="$2"
  local arg="${3-}"
  local valid="${4-true}"
  local autocomplete="${5-}"

  [[ "${_items_started:-0}" -eq 1 ]] || return 1

  if [[ "${_item_count:-0}" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":%s' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$valid"

  if [[ -n "$arg" ]]; then
    printf ',"arg":"%s"' "$(json_escape "$arg")"
  fi

  if [[ -n "$autocomplete" ]]; then
    printf ',"autocomplete":"%s"' "$(json_escape "$autocomplete")"
  fi

  printf '}'
  _item_count=$((_item_count + 1))
}

end_items() {
  if [[ "${_items_started:-0}" -eq 1 ]]; then
    printf ']}'
  else
    printf '{"items":[]}'
  fi
}

to_lower() {
  printf '%s' "${1-}" | tr '[:upper:]' '[:lower:]'
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
  configured="$(sfqp_trim "$configured")"
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
  configured="$(sfqp_trim "$configured")"
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

run_google_json() {
  local google_cli="$1"
  shift

  apply_google_cli_env_overrides
  "$google_cli" --json "$@"
}

array_contains() {
  local needle="$1"
  shift || true
  local value
  for value in "$@"; do
    if [[ "$value" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

declare -a accounts=()
default_account=""
active_account=""
accounts_fetch_error=""
accounts_loaded=0

load_accounts_once() {
  if [[ "$accounts_loaded" -eq 1 ]]; then
    return 0
  fi
  accounts_loaded=1

  accounts=()
  default_account=""
  active_account=""
  accounts_fetch_error=""

  if ! command -v jq >/dev/null 2>&1; then
    accounts_fetch_error="jq is required to parse google-cli JSON output"
    return 0
  fi

  local google_cli
  if ! google_cli="$(resolve_google_cli 2>/dev/null)"; then
    accounts_fetch_error="google-cli binary not found (set GOOGLE_CLI_BIN or install nils-google-cli)"
    return 0
  fi

  local output
  local rc=0
  set +e
  output="$(run_google_json "$google_cli" auth list 2>&1)"
  rc=$?
  set -e

  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(printf '%s\n' "$output" | jq -r '.error.message // empty' 2>/dev/null || true)"
    if [[ -z "$message" ]]; then
      message="$(sfej_normalize_error_message "$output")"
    fi
    [[ -n "$message" ]] || message="google-cli auth list failed"
    accounts_fetch_error="$message"
    return 0
  fi

  if ! printf '%s\n' "$output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    local message
    message="$(printf '%s\n' "$output" | jq -r '.error.message // empty' 2>/dev/null || true)"
    [[ -n "$message" ]] || message="unexpected auth list response format"
    accounts_fetch_error="$message"
    return 0
  fi

  mapfile -t accounts < <(printf '%s\n' "$output" | jq -r '.result.accounts[]?')
  default_account="$(printf '%s\n' "$output" | jq -r '.result.default_account // empty')"

  local stored_active
  stored_active="$(read_active_account || true)"

  if [[ -n "$stored_active" ]] && array_contains "$stored_active" "${accounts[@]}"; then
    active_account="$stored_active"
  elif [[ -n "$default_account" ]] && array_contains "$default_account" "${accounts[@]}"; then
    active_account="$default_account"
  elif [[ "${#accounts[@]}" -gt 0 ]]; then
    active_account="${accounts[0]}"
  fi
}

emit_login_help_items() {
  emit_item \
    "Login account (remote step 1)" \
    "Type: login <email>" \
    "" \
    false \
    "login "

  emit_item \
    "Finish remote login (step 2)" \
    "Type: login <callback-url> (or login <email> <callback-url>)" \
    "" \
    false \
    "login "

  emit_item \
    "Manual login" \
    "Type: login <email> --manual --code <authorization_code>" \
    "" \
    false \
    "login <email> --manual --code "
}

emit_switch_rows() {
  load_accounts_once

  if [[ -n "$accounts_fetch_error" ]]; then
    emit_item \
      "Account list unavailable" \
      "$accounts_fetch_error" \
      "" \
      false \
      ""
    return
  fi

  if [[ "${#accounts[@]}" -eq 0 ]]; then
    emit_item \
      "No accounts configured" \
      "Run login <email> first." \
      "" \
      false \
      "login "
    return
  fi

  local account
  for account in "${accounts[@]}"; do
    local marker=""
    if [[ "$account" == "$active_account" ]]; then
      marker="active"
    fi
    if [[ -n "$default_account" && "$account" == "$default_account" ]]; then
      if [[ -n "$marker" ]]; then
        marker="${marker}, default"
      else
        marker="default"
      fi
    fi

    local subtitle="Switch active account"
    if [[ -n "$marker" ]]; then
      subtitle="${subtitle} (${marker})"
    fi

    emit_item \
      "Use ${account}" \
      "$subtitle" \
      "switch::${account}" \
      true \
      "switch ${account}"
  done
}

emit_remove_rows() {
  load_accounts_once

  if [[ -n "$accounts_fetch_error" ]]; then
    emit_item \
      "Account list unavailable" \
      "$accounts_fetch_error" \
      "" \
      false \
      ""
    return
  fi

  if [[ "${#accounts[@]}" -eq 0 ]]; then
    emit_item \
      "No accounts to remove" \
      "Run login first, then remove by account email." \
      "" \
      false \
      "login "
    return
  fi

  local account
  for account in "${accounts[@]}"; do
    local subtitle="Remove account from google-cli auth store"
    if [[ "$account" == "$active_account" ]]; then
      subtitle="${subtitle} (currently active)"
    fi

    emit_item \
      "Remove ${account}" \
      "$subtitle" \
      "remove::${account}::0" \
      true \
      "remove ${account}"
  done
}

emit_auth_command_rows() {
  emit_item \
    "Google Service Auth Login" \
    "Expand to gsa login" \
    "prompt::login" \
    true \
    "login "

  emit_item \
    "Google Service Auth Switch" \
    "Expand to gsa switch" \
    "prompt::switch" \
    true \
    "switch "

  emit_item \
    "Google Service Auth Remove" \
    "Expand to gsa remove" \
    "prompt::remove" \
    true \
    "remove "
}

handle_switch_query() {
  local raw_query="$1"
  local remainder
  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?switch[[:space:]]*//I')"

  if [[ -z "$remainder" ]]; then
    emit_switch_rows
    return
  fi

  local target=""
  local token
  local seen_extra=0

  # shellcheck disable=SC2086
  for token in $remainder; do
    if [[ -z "$target" ]]; then
      target="$token"
    else
      seen_extra=1
    fi
  done

  if [[ "$seen_extra" -eq 1 ]]; then
    emit_item \
      "Invalid switch arguments" \
      "Usage: switch <email>" \
      "" \
      false \
      "switch "
    return
  fi

  load_accounts_once
  if [[ -n "$accounts_fetch_error" ]]; then
    emit_item "Account list unavailable" "$accounts_fetch_error" "" false ""
    return
  fi

  if ! array_contains "$target" "${accounts[@]}"; then
    emit_item \
      "Unknown account" \
      "${target} is not in auth list; run login first." \
      "" \
      false \
      "switch "
    return
  fi

  emit_item \
    "Use ${target}" \
    "Set workflow active account to ${target}" \
    "switch::${target}" \
    true \
    "switch ${target}"
}

handle_remove_query() {
  local raw_query="$1"
  local remainder
  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?remove[[:space:]]*//I')"

  if [[ -z "$remainder" ]]; then
    emit_remove_rows
    return
  fi

  local yes_flag=0
  local target=""
  local token
  local seen_extra=0

  # shellcheck disable=SC2206
  local parts=($remainder)
  local index=0
  while [[ "$index" -lt "${#parts[@]}" ]]; do
    token="${parts[$index]}"
    case "$token" in
    --yes | -y)
      yes_flag=1
      ;;
    *)
      if [[ -z "$target" ]]; then
        target="$token"
      else
        seen_extra=1
      fi
      ;;
    esac
    index=$((index + 1))
  done

  if [[ "$seen_extra" -eq 1 || -z "$target" ]]; then
    emit_item \
      "Invalid remove arguments" \
      "Usage: remove [--yes] <email>" \
      "" \
      false \
      "remove "
    return
  fi

  load_accounts_once
  if [[ -n "$accounts_fetch_error" ]]; then
    emit_item "Account list unavailable" "$accounts_fetch_error" "" false ""
    return
  fi

  if ! array_contains "$target" "${accounts[@]}"; then
    emit_item \
      "Unknown account" \
      "${target} is not in auth list." \
      "" \
      false \
      "remove "
    return
  fi

  emit_item \
    "Remove ${target}" \
    "Run google-cli auth remove ${target}" \
    "remove::${target}::${yes_flag}" \
    true \
    "remove ${target}"

  if [[ "$yes_flag" -eq 0 ]]; then
    emit_item \
      "Remove ${target} --yes" \
      "Skip workflow confirmation dialog." \
      "remove::${target}::1" \
      true \
      "remove --yes ${target}"
  fi
}

extract_callback_param() {
  local url="$1"
  local key="$2"
  printf '%s\n' "$url" | sed -nE "s|.*[?&]${key}=([^&#]+).*|\\1|p" | head -n1
}

looks_like_callback_url() {
  local value="${1-}"
  local is_localhost=1
  case "$value" in
  http://localhost/* | https://localhost/* | http://127.0.0.1/* | https://127.0.0.1/*)
    is_localhost=0
    ;;
  esac

  if [[ "$is_localhost" -eq 0 && "$value" == *"state="* && "$value" == *"code="* ]]; then
    return 0
  fi
  return 1
}

state_account_resolve_error=""

resolve_account_from_remote_state() {
  local state_value="${1-}"
  state_account_resolve_error=""

  if [[ -z "$state_value" ]]; then
    state_account_resolve_error="callback URL is missing state"
    return 1
  fi

  if ! command -v jq >/dev/null 2>&1; then
    state_account_resolve_error="jq is required to resolve callback account"
    return 1
  fi

  local config_dir=""
  config_dir="$(resolve_google_cli_config_dir_env || true)"
  if [[ -z "$config_dir" ]]; then
    state_account_resolve_error="cannot resolve google-cli config dir (set GOOGLE_CLI_CONFIG_DIR)"
    return 1
  fi

  local remote_state_path="$config_dir/remote-state.v1.json"
  if [[ ! -f "$remote_state_path" ]]; then
    state_account_resolve_error="remote state not found; run login <email> first"
    return 1
  fi

  local -a matched_accounts=()
  mapfile -t matched_accounts < <(
    jq -r --arg state "$state_value" \
      '.by_account // {} | to_entries[]? | select(.value.state == $state) | .key' \
      "$remote_state_path" 2>/dev/null || true
  )

  if [[ "${#matched_accounts[@]}" -eq 1 ]]; then
    printf '%s\n' "${matched_accounts[0]}"
    return 0
  fi

  if [[ "${#matched_accounts[@]}" -eq 0 ]]; then
    state_account_resolve_error="state does not match any pending remote login"
    return 1
  fi

  state_account_resolve_error="state matches multiple pending accounts; specify email explicitly"
  return 1
}

handle_login_query() {
  local raw_query="$1"
  local remainder
  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?login[[:space:]]*//I')"

  if [[ -z "$remainder" ]]; then
    emit_login_help_items
    return
  fi

  local account=""
  local mode="remote"
  local step="1"
  local code=""
  local state=""
  local callback_url=""
  local manual_flag=0
  local remote_flag=0

  # shellcheck disable=SC2206
  local parts=($remainder)
  local index=0
  while [[ "$index" -lt "${#parts[@]}" ]]; do
    local token="${parts[$index]}"
    case "$token" in
    --manual)
      manual_flag=1
      mode="manual"
      ;;
    --remote)
      remote_flag=1
      mode="remote"
      ;;
    --step)
      index=$((index + 1))
      if [[ "$index" -ge "${#parts[@]}" ]]; then
        emit_item "Invalid login arguments" "--step requires value 1 or 2" "" false "login "
        return
      fi
      step="${parts[$index]}"
      ;;
    --code)
      index=$((index + 1))
      if [[ "$index" -ge "${#parts[@]}" ]]; then
        emit_item "Invalid login arguments" "--code requires a value" "" false "login "
        return
      fi
      code="${parts[$index]}"
      ;;
    --state)
      index=$((index + 1))
      if [[ "$index" -ge "${#parts[@]}" ]]; then
        emit_item "Invalid login arguments" "--state requires a value" "" false "login "
        return
      fi
      state="${parts[$index]}"
      ;;
    --callback)
      index=$((index + 1))
      if [[ "$index" -ge "${#parts[@]}" ]]; then
        emit_item "Invalid login arguments" "--callback requires a URL" "" false "login "
        return
      fi
      callback_url="${parts[$index]}"
      ;;
    *)
      if [[ -z "$callback_url" ]] && looks_like_callback_url "$token"; then
        callback_url="$token"
      elif [[ -z "$account" ]]; then
        account="$token"
      else
        emit_item "Invalid login arguments" "Unexpected token: ${token}" "" false "login "
        return
      fi
      ;;
    esac

    index=$((index + 1))
  done

  if [[ "$manual_flag" -eq 1 && "$remote_flag" -eq 1 ]]; then
    emit_item \
      "Invalid login mode" \
      "Choose one mode: --remote or --manual." \
      "" \
      false \
      "login "
    return
  fi

  if [[ -n "$callback_url" ]]; then
    if [[ -z "$state" ]]; then
      state="$(extract_callback_param "$callback_url" "state")"
    fi
    if [[ -z "$code" ]]; then
      code="$(extract_callback_param "$callback_url" "code")"
    fi

    # Convenience mode: allow callback URL without explicit --step/state/code flags.
    if [[ "$mode" == "remote" && "$step" == "1" && -n "$state" && -n "$code" ]]; then
      step="2"
    fi
  fi

  # Convenience mode: explicit --state/--code implies remote step 2.
  if [[ "$mode" == "remote" && "$step" == "1" && -n "$state" && -n "$code" ]]; then
    step="2"
  fi

  if [[ "$mode" == "manual" ]]; then
    if [[ -z "$account" ]]; then
      emit_item \
        "Missing account email" \
        "Usage: login <email> --manual --code <authorization_code>" \
        "" \
        false \
        "login "
      return
    fi

    if [[ -z "$code" ]]; then
      emit_item \
        "Missing manual auth code" \
        "Usage: login ${account} --manual --code <authorization_code>" \
        "" \
        false \
        "login ${account} --manual --code "
      return
    fi

    emit_item \
      "Run login (manual) for ${account}" \
      "Execute google-cli auth add ${account} --manual --code <...>" \
      "login::manual::${account}::${code}" \
      true \
      "login ${account} --manual --code ${code}"
    return
  fi

  if [[ "$step" != "1" && "$step" != "2" ]]; then
    local account_for_hint="${account:-<email>}"
    emit_item \
      "Invalid remote step" \
      "--step must be 1 or 2" \
      "" \
      false \
      "login ${account_for_hint} --remote --step "
    return
  fi

  if [[ "$step" == "2" && -z "$account" && -n "$state" ]]; then
    account="$(resolve_account_from_remote_state "$state" || true)"
  fi

  if [[ -z "$account" ]]; then
    if [[ "$step" == "2" && -n "$state" ]]; then
      local resolve_detail="${state_account_resolve_error:-unable to resolve account from callback state}"
      emit_item \
        "Cannot resolve account for step 2" \
        "${resolve_detail}. Use: login <email> <callback-url>" \
        "" \
        false \
        "login "
      return
    fi

    emit_item \
      "Missing account email" \
      "Usage: login <email> [--remote|--manual ...]" \
      "" \
      false \
      "login "
    return
  fi

  if [[ "$step" == "1" ]]; then
    emit_item \
      "Run login (remote step 1) for ${account}" \
      "Generate authorization URL and state." \
      "login::remote::step1::${account}" \
      true \
      "login ${account} --remote --step 1"
    return
  fi

  if [[ -z "$state" || -z "$code" ]]; then
    emit_item \
      "Missing step 2 arguments" \
      "Usage: login <callback-url> or login ${account} <callback-url>" \
      "" \
      false \
      "login ${account} "
    return
  fi

  emit_item \
    "Run login (remote step 2) for ${account}" \
    "Exchange code using saved/entered state." \
    "login::remote::step2::${account}::${state}::${code}" \
    true \
    "login ${account} --remote --step 2 --state ${state} --code ${code}"
}

emit_default_items() {
  emit_auth_command_rows
  emit_switch_rows
}

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
lower_query="$(to_lower "$trimmed_query")"

begin_items

if [[ "$lower_query" == "" || "$lower_query" == "auth" || "$lower_query" == "help" || "$lower_query" == "?" ]]; then
  emit_default_items
  end_items
  exit 0
fi

if [[ "$lower_query" == login* || "$lower_query" == auth\ login* ]]; then
  handle_login_query "$trimmed_query"
  end_items
  exit 0
fi

if looks_like_callback_url "$trimmed_query"; then
  handle_login_query "login $trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == switch* || "$lower_query" == auth\ switch* ]]; then
  handle_switch_query "$trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == remove* || "$lower_query" == auth\ remove* ]]; then
  handle_remove_query "$trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == accounts* || "$lower_query" == list* || "$lower_query" == auth\ list* ]]; then
  emit_switch_rows
  end_items
  exit 0
fi

emit_item \
  "Unknown command: ${trimmed_query}" \
  "Try: login, switch, remove, accounts, or help" \
  "" \
  false \
  "help"
emit_default_items
end_items

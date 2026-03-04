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

emit_mail_item() {
  local title="$1"
  local subtitle="$2"
  local arg="$3"
  local search_query="$4"
  local result_count="$5"
  local message_id="$6"
  local thread_id="$7"
  local mode_label="$8"
  local resolved_account="$9"
  local modifier_arg="gmail-open-search::${search_query}"
  local modifier_subtitle="Open Gmail web search for ${search_query}"

  [[ "${_items_started:-0}" -eq 1 ]] || return 1

  if [[ "${_item_count:-0}" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":true,"arg":"%s","variables":{"GOOGLE_MAIL_SEARCH_RESULT_COUNT":"%s","GOOGLE_MAIL_MESSAGE_ID":"%s","GOOGLE_MAIL_MESSAGE_THREAD_ID":"%s","GOOGLE_MAIL_QUERY":"%s","GOOGLE_MAIL_QUERY_MODE":"%s","GOOGLE_MAIL_ACCOUNT":"%s"},"mods":{"cmd":{"valid":true,"arg":"%s","subtitle":"%s"}}}' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$(json_escape "$arg")" \
    "$(json_escape "$result_count")" \
    "$(json_escape "$message_id")" \
    "$(json_escape "$thread_id")" \
    "$(json_escape "$search_query")" \
    "$(json_escape "$mode_label")" \
    "$(json_escape "$resolved_account")" \
    "$(json_escape "$modifier_arg")" \
    "$(json_escape "$modifier_subtitle")"

  _item_count=$((_item_count + 1))
}

emit_unread_hint_item() {
  local title="$1"
  local subtitle="$2"
  local unread_count="$3"

  [[ "${_items_started:-0}" -eq 1 ]] || return 1

  if [[ "${_item_count:-0}" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":false,"autocomplete":"unread ","variables":{"GOOGLE_MAIL_UNREAD_COUNT":"%s","GOOGLE_MAIL_QUERY":"in:inbox is:unread","GOOGLE_MAIL_QUERY_MODE":"unread"}}' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$(json_escape "$unread_count")"

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

compact_whitespace() {
  local value="${1-}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  value="$(printf '%s' "$value" | tr -s '[:space:]' ' ')"
  value="$(sfqp_trim "$value")"
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

emit_help_items() {
  local unread_count=""
  unread_count="$(fetch_unread_count || true)"
  local unread_title="Unread Mail List"
  if [[ -n "$unread_count" ]]; then
    unread_title="Unread Mail List (${unread_count})"
  fi

  emit_item \
    "Open Gmail Inbox" \
    "Open https://mail.google.com/mail/u/0/#inbox" \
    "gmail-open-home" \
    true \
    "open"

  emit_unread_hint_item \
    "$unread_title" \
    "Type: unread [--account <email>] [optional query terms]" \
    "$unread_count"

  emit_item \
    "Latest Mail List" \
    "Type: latest [optional query terms]" \
    "" \
    false \
    "latest "

  emit_item \
    "Search Mail" \
    "Type: search <query> (or directly type query text)" \
    "" \
    false \
    "search "
}

fetch_unread_count() {
  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local google_cli
  if ! google_cli="$(resolve_google_cli 2>/dev/null)"; then
    return 1
  fi

  local active_account=""
  active_account="$(read_active_account || true)"

  local -a command_args=()
  if [[ -n "$active_account" ]]; then
    command_args+=(-a "$active_account")
  fi
  command_args+=(gmail search --max 500 --format minimal --query "in:inbox is:unread")

  local output rc
  run_google_json_capture output rc "$google_cli" "${command_args[@]}"
  if [[ "$rc" -ne 0 ]]; then
    return 1
  fi

  if ! printf '%s\n' "$output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    return 1
  fi

  local count
  count="$(printf '%s\n' "$output" | jq -r '.result.count // empty' 2>/dev/null || true)"
  if [[ "$count" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$count"
    return 0
  fi

  return 1
}

append_optional_terms() {
  local base_query="$1"
  local extra_terms
  extra_terms="$(sfqp_trim "${2-}")"
  if [[ -z "$extra_terms" ]]; then
    printf '%s\n' "$base_query"
  else
    printf '%s %s\n' "$base_query" "$extra_terms"
  fi
}

parse_unread_query_options() {
  local input="${1-}"
  unread_query_account=""
  unread_query_terms=""
  unread_query_error=""

  input="$(sfqp_trim "$input")"

  local -a tokens=()
  if [[ -n "$input" ]]; then
    # shellcheck disable=SC2206
    tokens=($input)
  fi

  local -a terms=()
  local index=0
  while ((index < ${#tokens[@]})); do
    local token="${tokens[$index]}"
    case "$token" in
    --account)
      index=$((index + 1))
      if ((index >= ${#tokens[@]})); then
        unread_query_error="missing value for --account"
        return 1
      fi
      unread_query_account="$(sfqp_trim "${tokens[$index]}")"
      if [[ -z "$unread_query_account" ]]; then
        unread_query_error="missing value for --account"
        return 1
      fi
      ;;
    --account=*)
      unread_query_account="$(sfqp_trim "${token#--account=}")"
      if [[ -z "$unread_query_account" ]]; then
        unread_query_error="missing value for --account"
        return 1
      fi
      ;;
    *)
      terms+=("$token")
      ;;
    esac
    index=$((index + 1))
  done

  local term
  for term in "${terms[@]}"; do
    if [[ -z "$unread_query_terms" ]]; then
      unread_query_terms="$term"
    else
      unread_query_terms="${unread_query_terms} ${term}"
    fi
  done
  unread_query_terms="$(sfqp_trim "$unread_query_terms")"
  return 0
}

resolve_mail_limit_env() {
  local raw_value="${1-}"
  local default_value="${2-25}"

  raw_value="$(sfqp_trim "$raw_value")"
  if [[ -z "$raw_value" ]]; then
    printf '%s\n' "$default_value"
    return 0
  fi

  if ! [[ "$raw_value" =~ ^[0-9]+$ ]]; then
    printf '%s\n' "$default_value"
    return 0
  fi

  if ((raw_value < 1 || raw_value > 500)); then
    printf '%s\n' "$default_value"
    return 0
  fi

  printf '%s\n' "$raw_value"
}

resolve_mail_result_max() {
  local mode_label="$1"
  case "$mode_label" in
  latest | unread)
    resolve_mail_limit_env "${GOOGLE_MAIL_LATEST_MAX:-}" "25"
    ;;
  *)
    resolve_mail_limit_env "${GOOGLE_MAIL_SEARCH_MAX:-}" "25"
    ;;
  esac
}

handle_mail_search() {
  local search_query="$1"
  local mode_label="$2"
  local account_override="${3-}"

  if ! command -v jq >/dev/null 2>&1; then
    emit_item \
      "Mail search unavailable" \
      "jq is required to parse google-cli JSON output" \
      "" \
      false \
      "search "
    return
  fi

  local google_cli
  if ! google_cli="$(resolve_google_cli 2>/dev/null)"; then
    emit_item \
      "Mail search unavailable" \
      "google-cli binary not found (set GOOGLE_CLI_BIN or install nils-google-cli)" \
      "" \
      false \
      "search "
    return
  fi

  local effective_account=""
  if [[ -n "$account_override" ]]; then
    effective_account="$account_override"
  else
    effective_account="$(read_active_account || true)"
  fi

  local search_max
  search_max="$(resolve_mail_result_max "$mode_label")"

  local -a command_args=()
  if [[ -n "$effective_account" ]]; then
    command_args+=(-a "$effective_account")
  fi
  command_args+=(gmail search --max "$search_max" --format metadata --headers "Subject,From,Date" --query "$search_query")

  local output rc
  run_google_json_capture output rc "$google_cli" "${command_args[@]}"

  if [[ "$rc" -ne 0 ]]; then
    local message
    message="$(printf '%s\n' "$output" | jq -r '.error.message // empty' 2>/dev/null || true)"
    if [[ -z "$message" ]]; then
      message="$(sfej_normalize_error_message "$output")"
    fi
    [[ -n "$message" ]] || message="google-cli gmail search failed"
    emit_item "Mail search failed" "$message" "" false "search "
    return
  fi

  if ! printf '%s\n' "$output" | jq -e '.ok == true and (.result | type == "object")' >/dev/null 2>&1; then
    local message
    message="$(printf '%s\n' "$output" | jq -r '.error.message // empty' 2>/dev/null || true)"
    [[ -n "$message" ]] || message="unexpected gmail search response format"
    emit_item "Mail search failed" "$message" "" false "search "
    return
  fi

  local result_count
  result_count="$(printf '%s\n' "$output" | jq -r '.result.count // ((.result.messages // []) | length) // 0' 2>/dev/null || true)"
  if ! [[ "$result_count" =~ ^[0-9]+$ ]]; then
    result_count="0"
  fi

  local resolved_account=""
  resolved_account="$(printf '%s\n' "$output" | jq -r '.result.account // empty' 2>/dev/null || true)"
  if [[ -z "$resolved_account" && -n "$effective_account" ]]; then
    resolved_account="$effective_account"
  fi
  if [[ -z "$resolved_account" ]]; then
    resolved_account="(auto)"
  fi

  local emitted=0
  while IFS=$'\t' read -r message_id thread_id subject from_header date_header snippet; do
    [[ -n "$message_id" ]] || continue

    subject="$(compact_whitespace "$subject")"
    from_header="$(compact_whitespace "$from_header")"
    date_header="$(compact_whitespace "$date_header")"
    snippet="$(compact_whitespace "$snippet")"

    local title="$subject"
    if [[ -z "$title" ]]; then
      title="$snippet"
    fi
    if [[ -z "$title" ]]; then
      title="(no subject)"
    fi

    local sender_label="$from_header"
    local date_label="$date_header"
    [[ -n "$sender_label" ]] || sender_label="unknown sender"
    [[ -n "$date_label" ]] || date_label="unknown date"

    local subtitle="${sender_label} · ${date_label}"
    if [[ -n "$snippet" ]]; then
      subtitle="${subtitle} · ${snippet}"
    fi
    subtitle="${subtitle} · mode=${mode_label} · account=${resolved_account} · result.count=${result_count}"

    emit_mail_item \
      "$title" \
      "$subtitle" \
      "gmail-open-message::${message_id}" \
      "$search_query" \
      "$result_count" \
      "$message_id" \
      "$thread_id" \
      "$mode_label" \
      "$resolved_account"

    emitted=1
  done < <(printf '%s\n' "$output" | jq -r '.result.messages[]? | [.id // "", .thread_id // "", .headers.Subject // .headers.subject // "", .headers.From // .headers.from // "", .headers.Date // .headers.date // "", .snippet // ""] | @tsv')

  if [[ "$emitted" -eq 0 ]]; then
    emit_item \
      "No mail found" \
      "query=${search_query} · mode=${mode_label} · account=${resolved_account} · result.count=${result_count}" \
      "" \
      false \
      "search ${search_query}"
  fi
}

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
lower_query="$(to_lower "$trimmed_query")"

begin_items

if [[ -z "$trimmed_query" || "$lower_query" == "help" || "$lower_query" == "?" ]]; then
  emit_help_items
  end_items
  exit 0
fi

if [[ "$lower_query" == "open" || "$lower_query" == "home" || "$lower_query" == "open inbox" || "$lower_query" == "inbox" || "$lower_query" == "open gmail" ]]; then
  emit_item \
    "Open Gmail Inbox" \
    "Open https://mail.google.com/mail/u/0/#inbox" \
    "gmail-open-home" \
    true \
    "open"
  end_items
  exit 0
fi

search_query=""
mode_label="search"
account_override=""

if [[ "$lower_query" == unread* ]]; then
  mode_label="unread"
  extra_terms="$(printf '%s' "$trimmed_query" | sed -E 's/^[[:space:]]*unread[[:space:]]*//I')"
  if ! parse_unread_query_options "$extra_terms"; then
    emit_item \
      "Unread query invalid" \
      "$unread_query_error" \
      "" \
      false \
      "unread "
    end_items
    exit 0
  fi
  account_override="$unread_query_account"
  search_query="$(append_optional_terms "in:inbox is:unread" "$unread_query_terms")"
elif [[ "$lower_query" == latest* ]]; then
  mode_label="latest"
  extra_terms="$(printf '%s' "$trimmed_query" | sed -E 's/^[[:space:]]*latest[[:space:]]*//I')"
  search_query="$(append_optional_terms "in:inbox" "$extra_terms")"
elif [[ "$lower_query" == search* ]]; then
  mode_label="search"
  search_query="$(printf '%s' "$trimmed_query" | sed -E 's/^[[:space:]]*search[[:space:]]*//I')"
else
  mode_label="search"
  search_query="$trimmed_query"
fi

search_query="$(sfqp_trim "$search_query")"

if [[ -z "$search_query" ]]; then
  emit_help_items
  end_items
  exit 0
fi

handle_mail_search "$search_query" "$mode_label" "$account_override"
end_items

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workflow_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$workflow_dir/../.." && pwd)"
smoke_helper="$repo_root/scripts/lib/workflow_smoke_helpers.sh"

if [[ ! -f "$smoke_helper" ]]; then
  echo "missing required helper: $smoke_helper" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$smoke_helper"

for required in \
  workflow.toml \
  README.md \
  TROUBLESHOOTING.md \
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter_empty.sh \
  scripts/script_filter.sh \
  scripts/script_filter_drive.sh \
  scripts/script_filter_mail.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter_empty.sh \
  scripts/script_filter.sh \
  scripts/script_filter_drive.sh \
  scripts/script_filter_mail.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "google-service" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"
if rg -n '^rust_binary[[:space:]]*=' "$manifest" >/dev/null; then
  [[ "$(toml_string "$manifest" rust_binary)" == "google-cli" ]] || fail "rust_binary must be google-cli"
else
  fail "rust_binary must be configured for google-service workflow"
fi

for env_key in \
  GOOGLE_CLI_BIN \
  GOOGLE_CLI_CONFIG_DIR \
  GOOGLE_CLI_KEYRING_MODE \
  GOOGLE_DRIVE_DOWNLOAD_DIR; do
  if ! rg -n "^${env_key}[[:space:]]*=[[:space:]]*\"\"" "$manifest" >/dev/null; then
    fail "${env_key} default must be empty"
  fi
done

if ! rg -n '^GOOGLE_MAIL_SEARCH_MAX[[:space:]]*=[[:space:]]*"25"' "$manifest" >/dev/null; then
  fail "GOOGLE_MAIL_SEARCH_MAX default must be 25"
fi

if ! rg -n '^GOOGLE_MAIL_LATEST_MAX[[:space:]]*=[[:space:]]*"25"' "$manifest" >/dev/null; then
  fail "GOOGLE_MAIL_LATEST_MAX default must be 25"
fi

if ! rg -n '^GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD[[:space:]]*=[[:space:]]*"0"' "$manifest" >/dev/null; then
  fail "GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD default must be 0"
fi

if ! rg -n '^GOOGLE_AUTH_REMOVE_CONFIRM[[:space:]]*=[[:space:]]*"1"' "$manifest" >/dev/null; then
  fail "GOOGLE_AUTH_REMOVE_CONFIRM default must be 1"
fi

plist_json="$(plist_to_json "$workflow_dir/src/info.plist.template")"
assert_jq_json "$plist_json" '.objects | type == "array" and length == 5' "plist should contain four script filters and one action"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter")] | length == 4' "script filter count mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gs" and .config.scriptfile == "./scripts/script_filter_empty.sh")] | length == 1' "gs keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gsa" and .config.scriptfile == "./scripts/script_filter.sh")] | length == 1' "gsa keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gsd" and .config.scriptfile == "./scripts/script_filter_drive.sh")] | length == 1' "gsd keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gsm" and .config.scriptfile == "./scripts/script_filter_mail.sh")] | length == 1' "gsm keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaycustom == 1] | all' "queue delay custom must be 1"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelayimmediatelyinitially == false] | all' "queue immediate initial must be false"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.alfredfiltersresults == false] | all' "alfredfiltersresults must be false"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.action.script") | .config.scriptfile == "./scripts/action_open.sh"' "action script path mismatch"
assert_jq_json "$plist_json" '[.userconfigurationconfig[] | select(.variable == "GOOGLE_DRIVE_DOWNLOAD_DIR")] | length == 1' "GOOGLE_DRIVE_DOWNLOAD_DIR user config entry missing"
assert_jq_json "$plist_json" '[.userconfigurationconfig[] | select(.variable == "GOOGLE_MAIL_SEARCH_MAX")] | length == 1' "GOOGLE_MAIL_SEARCH_MAX user config entry missing"
assert_jq_json "$plist_json" '[.userconfigurationconfig[] | select(.variable == "GOOGLE_MAIL_LATEST_MAX")] | length == 1' "GOOGLE_MAIL_LATEST_MAX user config entry missing"
assert_jq_json "$plist_json" '[.userconfigurationconfig[] | select(.variable == "GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD")] | length == 1' "GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD user config entry missing"

smoke_tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$smoke_tmp"
}
trap cleanup EXIT

mkdir -p "$smoke_tmp/bin" "$smoke_tmp/data"
mkdir -p "$smoke_tmp/config"
stub_accounts_file="$smoke_tmp/accounts.json"
stub_log="$smoke_tmp/google-cli.log"
remote_state_file="$smoke_tmp/config/remote-state.v1.json"

cat >"$stub_accounts_file" <<'JSON'
{"accounts":["a@example.com","b@example.com"],"default_account":"a@example.com"}
JSON

cat >"$remote_state_file" <<'JSON'
{
  "version": 1,
  "by_account": {
    "c@example.com": {
      "state": "state-c@example.com",
      "issued_at_epoch_secs": 1
    }
  }
}
JSON

cat >"$smoke_tmp/bin/google-cli" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

accounts_file="${STUB_ACCOUNTS_FILE:?missing STUB_ACCOUNTS_FILE}"
log_file="${STUB_LOG:-}"

log() {
  if [[ -n "$log_file" ]]; then
    printf '%s\n' "$*" >>"$log_file"
  fi
}

read_state() {
  if [[ -f "$accounts_file" ]]; then
    cat "$accounts_file"
  else
    printf '{"accounts":[],"default_account":null}'
  fi
}

write_state() {
  local payload="$1"
  printf '%s\n' "$payload" >"$accounts_file"
}

emit_ok() {
  local command_id="$1"
  local result_json="$2"
  jq -cn --arg cmd "$command_id" --argjson result "$result_json" '{schema_version:"v1",command:$cmd,ok:true,result:$result}'
}

emit_error() {
  local command_id="$1"
  local message="$2"
  jq -cn --arg cmd "$command_id" --arg msg "$message" '{schema_version:"v1",command:$cmd,ok:false,error:{code:"NILS_GOOGLE_005",message:$msg,details:{kind:"user",exit_code:2}}}'
}

if [[ "${1:-}" == "--json" ]]; then
  shift
fi

selected_account=""
while [[ "${1:-}" == "-a" || "${1:-}" == "--account" ]]; do
  shift
  selected_account="${1:-}"
  [[ -n "$selected_account" ]] || {
    emit_error "google.unknown" "missing value for --account"
    exit 2
  }
  shift
done

log "$*"

drive_fixture_json='[
  {"id":"file-1","name":"Keyboard_Configuration","mime_type":"application/vnd.google-apps.document","size_bytes":2097152,"parents":["folder-1"]},
  {"id":"file-2","name":"keyboard-notes.txt","mime_type":"text/plain","size_bytes":2048,"parents":["folder-1"]}
]'
gmail_fixture_json='[
  {"id":"msg-1","thread_id":"thread-1","snippet":"Keyboard shortcut guide for team","label_ids":["INBOX","UNREAD"],"headers":{"From":"Team <team@example.com>","Subject":"Keyboard shortcuts","Date":"Tue, 03 Mar 2026 08:00:00 +0800"}},
  {"id":"msg-2","thread_id":"thread-2","snippet":"Weekly summary for project status","label_ids":["INBOX"],"headers":{"From":"Manager <manager@example.com>","Subject":"Weekly summary","Date":"Mon, 02 Mar 2026 10:30:00 +0800"}},
  {"id":"msg-3","thread_id":"thread-3","snippet":"Keyboard firmware release notes","label_ids":["INBOX","UNREAD"],"headers":{"From":"Ops <ops@example.com>","Subject":"Firmware keyboard release","Date":"Sun, 01 Mar 2026 21:15:00 +0800"}}
]'

case "${1:-}" in
auth)
  case "${2:-}" in
  list)
    state="$(read_state)"
    accounts_json="$(jq -c '.accounts // []' <<<"$state")"
    default_json="$(jq -c '.default_account // null' <<<"$state")"
    result_json="$(jq -cn --argjson accounts "$accounts_json" --argjson default_account "$default_json" '{accounts:$accounts,default_account:$default_account,aliases:{}}')"
    emit_ok "google.auth.list" "$result_json"
    ;;
  add)
    account="${3:-}"
    [[ -n "$account" ]] || {
      emit_error "google.auth.add" "missing account"
      exit 2
    }

    mode="loopback"
    step=""
    state=""
    code=""

    shift 3
    while [[ "$#" -gt 0 ]]; do
      case "$1" in
      --remote)
        mode="remote"
        ;;
      --manual)
        mode="manual"
        ;;
      --step)
        step="${2:-}"
        shift
        ;;
      --state)
        state="${2:-}"
        shift
        ;;
      --code)
        code="${2:-}"
        shift
        ;;
      esac
      shift
    done

    if [[ "$mode" == "remote" && "$step" == "1" ]]; then
      step_state="state-${account}"
      auth_url="https://accounts.google.com/o/oauth2/v2/auth?state=${step_state}&login_hint=${account}"
      result_json="$(jq -cn --arg account "$account" --arg state "$step_state" --arg url "$auth_url" '{account:$account,mode:"remote",step:1,state:$state,authorization_url:$url}')"
      emit_ok "google.auth.add" "$result_json"
      exit 0
    fi

    if [[ "$mode" == "remote" && "$step" == "2" ]]; then
      [[ -n "$state" && -n "$code" ]] || {
        emit_error "google.auth.add" "remote step2 requires --state and --code"
        exit 2
      }
    fi

    if [[ "$mode" == "manual" ]]; then
      [[ -n "$code" ]] || {
        emit_error "google.auth.add" "manual mode requires --code"
        exit 2
      }
    fi

    next_state="$(jq -c --arg account "$account" '
      .accounts = (if ((.accounts // []) | index($account)) == null then ((.accounts // []) + [$account]) else (.accounts // []) end)
      | .default_account = (if (.default_account == null or .default_account == "") then $account else .default_account end)
    ' <<<"$(read_state)")"
    write_state "$next_state"

    if [[ "$mode" == "remote" && "$step" == "2" ]]; then
      result_json="$(jq -cn --arg account "$account" '{account:$account,mode:"remote",step:2,backend:"file",stored:true}')"
    elif [[ "$mode" == "manual" ]]; then
      result_json="$(jq -cn --arg account "$account" '{account:$account,mode:"manual",backend:"file",stored:true}')"
    else
      result_json="$(jq -cn --arg account "$account" '{account:$account,mode:"loopback",backend:"file",stored:true}')"
    fi

    emit_ok "google.auth.add" "$result_json"
    ;;
  remove)
    account="${3:-}"
    [[ -n "$account" ]] || {
      emit_error "google.auth.remove" "missing account"
      exit 2
    }

    removed_state="$(jq -c --arg account "$account" '
      .accounts = [(.accounts // [])[] | select(. != $account)]
      | .default_account = (if .default_account == $account then (.accounts[0] // null) else .default_account end)
    ' <<<"$(read_state)")"
    write_state "$removed_state"

    remaining="$(jq '.accounts | length' <<<"$removed_state")"
    result_json="$(jq -cn --arg account "$account" --argjson remaining "$remaining" '{account:$account,removed_token:true,remaining_accounts:$remaining}')"
    emit_ok "google.auth.remove" "$result_json"
    ;;
  *)
    emit_error "google.auth" "unsupported auth command: ${2:-}"
    exit 2
    ;;
  esac
  ;;
gmail)
  case "${2:-}" in
  search)
    max="25"
    query=""

    shift 2
    while [[ "$#" -gt 0 ]]; do
      case "$1" in
      --max)
        max="${2:-25}"
        shift
        ;;
      --page)
        shift
        ;;
      --query)
        query="${2:-}"
        shift
        ;;
      --format)
        shift
        ;;
      --headers)
        shift
        ;;
      *)
        if [[ -z "$query" ]]; then
          query="$1"
        else
          query="$query $1"
        fi
        ;;
      esac
      shift
    done

    if ! [[ "$max" =~ ^[0-9]+$ ]]; then
      max="25"
    fi

    query_lower="$(printf '%s' "$query" | tr '[:upper:]' '[:lower:]')"
    require_unread=0
    require_inbox=0
    if [[ "$query_lower" == *"is:unread"* ]]; then
      require_unread=1
    fi
    if [[ "$query_lower" == *"in:inbox"* ]]; then
      require_inbox=1
    fi
    text_query="${query_lower//is:unread/ }"
    text_query="${text_query//in:inbox/ }"
    text_query="$(printf '%s' "$text_query" | tr -s '[:space:]' ' ' | sed 's/^ //; s/ $//')"

    filtered="$(jq -c \
      --arg q "$text_query" \
      --argjson require_unread "$require_unread" \
      --argjson require_inbox "$require_inbox" '
      [ .[] | select(
        (($require_unread == 0) or ((.label_ids // []) | index("UNREAD") != null)) and
        (($require_inbox == 0) or ((.label_ids // []) | index("INBOX") != null)) and
        (($q == "") or (((.snippet + " " + (.headers.Subject // "") + " " + (.headers.From // "") + " " + .id) | ascii_downcase) | contains($q)))
      ) ]
    ' <<<"$gmail_fixture_json")"

    # Simulate latest ordering by Date descending in fixture sequence.
    limited="$(jq -c --argjson max "$max" '.[0:$max]' <<<"$filtered")"
    count="$(jq 'length' <<<"$limited")"

    if [[ -n "$selected_account" ]]; then
      account_for_result="$selected_account"
      account_source="explicit"
    else
      account_for_result="$(jq -r '.default_account // empty' <<<"$(read_state)")"
      account_source="default"
    fi

    result_json="$(jq -cn \
      --arg account "$account_for_result" \
      --arg account_source "$account_source" \
      --arg query "$query" \
      --argjson max "$max" \
      --argjson count "$count" \
      --argjson messages "$limited" \
      '{account:$account,account_source:$account_source,query:$query,format:"metadata",max:$max,page_token:null,count:$count,messages:$messages}')"
    emit_ok "google.gmail.search" "$result_json"
    ;;
  *)
    emit_error "google.gmail" "unsupported gmail command: ${2:-}"
    exit 2
    ;;
  esac
  ;;
drive)
  case "${2:-}" in
  search)
    max="25"
    query=""

    shift 2
    while [[ "$#" -gt 0 ]]; do
      case "$1" in
      --max)
        max="${2:-25}"
        shift
        ;;
      --page)
        shift
        ;;
      --query)
        query="${2:-}"
        shift
        ;;
      --raw-query)
        ;;
      *)
        if [[ -z "$query" ]]; then
          query="$1"
        else
          query="$query $1"
        fi
        ;;
      esac
      shift
    done

    if ! [[ "$max" =~ ^[0-9]+$ ]]; then
      max="25"
    fi

    query_lower="$(printf '%s' "$query" | tr '[:upper:]' '[:lower:]')"
    filtered="$(jq -c --arg q "$query_lower" '
      [ .[] | select(
        ($q == "") or
        (((.name + " " + .mime_type + " " + .id) | ascii_downcase) | contains($q))
      ) ]
    ' <<<"$drive_fixture_json")"
    limited="$(jq -c --argjson max "$max" '.[0:$max]' <<<"$filtered")"
    count="$(jq 'length' <<<"$limited")"

    if [[ -n "$selected_account" ]]; then
      account_for_result="$selected_account"
      account_source="explicit"
    else
      account_for_result="$(jq -r '.default_account // empty' <<<"$(read_state)")"
      account_source="default"
    fi

    result_json="$(jq -cn \
      --arg account "$account_for_result" \
      --arg account_source "$account_source" \
      --arg query "$query" \
      --argjson max "$max" \
      --argjson count "$count" \
      --argjson files "$limited" \
      '{account:$account,account_source:$account_source,query:$query,raw_query:false,max:$max,page_token:null,count:$count,files:$files}')"
    emit_ok "google.drive.search" "$result_json"
    ;;
  get)
    file_id="${3:-}"
    [[ -n "$file_id" ]] || {
      emit_error "google.drive.get" "missing file id"
      exit 2
    }

    file_json="$(jq -c --arg id "$file_id" '[.[] | select(.id == $id)][0] // empty' <<<"$drive_fixture_json")"
    if [[ -z "$file_json" ]]; then
      emit_error "google.drive.get" "file not found"
      exit 2
    fi

    if [[ -n "$selected_account" ]]; then
      account_for_result="$selected_account"
      account_source="explicit"
    else
      account_for_result="$(jq -r '.default_account // empty' <<<"$(read_state)")"
      account_source="default"
    fi

    result_json="$(jq -cn \
      --arg account "$account_for_result" \
      --arg account_source "$account_source" \
      --argjson file "$file_json" \
      '{account:$account,account_source:$account_source,file:$file}')"
    emit_ok "google.drive.get" "$result_json"
    ;;
  download)
    file_id="${3:-}"
    [[ -n "$file_id" ]] || {
      emit_error "google.drive.download" "missing file id"
      exit 2
    }

    out_path=""
    shift 3
    while [[ "$#" -gt 0 ]]; do
      case "$1" in
      --out)
        out_path="${2:-}"
        shift
        ;;
      --overwrite)
        ;;
      *)
        ;;
      esac
      shift
    done

    file_json="$(jq -c --arg id "$file_id" '[.[] | select(.id == $id)][0] // empty' <<<"$drive_fixture_json")"
    if [[ -z "$file_json" ]]; then
      emit_error "google.drive.download" "file not found"
      exit 2
    fi

    file_name="$(jq -r '.name // empty' <<<"$file_json")"
    mime_type="$(jq -r '.mime_type // "application/octet-stream"' <<<"$file_json")"
    [[ -n "$file_name" ]] || file_name="$file_id"
    [[ -n "$out_path" ]] || out_path="$file_name"
    mkdir -p "$(dirname "$out_path")"
    printf 'download-%s\n' "$file_id" >"$out_path"
    bytes_written="$(wc -c <"$out_path" | tr -d '[:space:]')"

    if [[ -n "$selected_account" ]]; then
      account_for_result="$selected_account"
      account_source="explicit"
    else
      account_for_result="$(jq -r '.default_account // empty' <<<"$(read_state)")"
      account_source="default"
    fi

    result_json="$(jq -cn \
      --arg account "$account_for_result" \
      --arg account_source "$account_source" \
      --arg file_id "$file_id" \
      --arg file_name "$file_name" \
      --arg mime_type "$mime_type" \
      --arg path "$out_path" \
      --argjson bytes_written "${bytes_written:-0}" \
      '{account:$account,account_source:$account_source,file_id:$file_id,file_name:$file_name,mime_type:$mime_type,source:"download",format:null,bytes_written:$bytes_written,path:$path}')"
    emit_ok "google.drive.download" "$result_json"
    ;;
  *)
    emit_error "google.drive" "unsupported drive command: ${2:-}"
    exit 2
    ;;
  esac
  ;;
*)
  emit_error "google.unknown" "unsupported command"
  exit 2
  ;;
esac
EOS
chmod +x "$smoke_tmp/bin/google-cli"

script_filter_empty="$workflow_dir/scripts/script_filter_empty.sh"
script_filter="$workflow_dir/scripts/script_filter.sh"
script_filter_drive="$workflow_dir/scripts/script_filter_drive.sh"
script_filter_mail="$workflow_dir/scripts/script_filter_mail.sh"
action_open="$workflow_dir/scripts/action_open.sh"

mkdir -p "$smoke_tmp/home/Downloads"

base_env=(
  "GOOGLE_CLI_BIN=$smoke_tmp/bin/google-cli"
  "STUB_ACCOUNTS_FILE=$stub_accounts_file"
  "STUB_LOG=$stub_log"
  "GOOGLE_CLI_CONFIG_DIR=$smoke_tmp/config"
  "ALFRED_WORKFLOW_DATA=$smoke_tmp/data"
  "GOOGLE_AUTH_REMOVE_CONFIRM=0"
  "HOME=$smoke_tmp/home"
)

run_with_env() {
  env "${base_env[@]}" "$@"
}

root_json="$(run_with_env bash "$script_filter_empty" "")"
assert_jq_json "$root_json" '.items | length == 1' "gs root query should emit one account status row"
assert_jq_json "$root_json" '.items[0].title == "Current account: a@example.com"' "gs should show default account when active account is not set"
assert_jq_json "$root_json" '.items[0].arg == "prompt::switch"' "gs current account row should route to switch prompt token"

root_with_unread_json="$(env "${base_env[@]}" GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD=1 bash "$script_filter_empty" "")"
assert_jq_json "$root_with_unread_json" '.items | length == 2' "gs root query should emit unread summary row when toggle enabled"
assert_jq_json "$root_with_unread_json" '.items[1].title == "Unread mail (all accounts): 4"' "gs unread summary total mismatch"
assert_jq_json "$root_with_unread_json" '.items[1].arg == "prompt::mail-unread"' "gs unread summary row should route to gsm unread prompt token"
assert_jq_json "$root_with_unread_json" '.items[1].subtitle | test("a@example.com:2")' "gs unread summary should include account a count"
assert_jq_json "$root_with_unread_json" '.items[1].subtitle | test("b@example.com:2")' "gs unread summary should include account b count"

auth_root_json="$(run_with_env bash "$script_filter" "")"
assert_jq_json "$auth_root_json" '.items | length >= 5' "gsa root query should emit command and account rows"
assert_jq_json "$auth_root_json" '.items[0].title == "Google Service Auth Login" and .items[0].arg == "prompt::login"' "gsa command row login mismatch"
assert_jq_json "$auth_root_json" '.items[1].title == "Google Service Auth Switch" and .items[1].arg == "prompt::switch"' "gsa command row switch mismatch"
assert_jq_json "$auth_root_json" '.items[2].title == "Google Service Auth Remove" and .items[2].arg == "prompt::remove"' "gsa command row remove mismatch"
assert_jq_json "$auth_root_json" '[.items[] | select(.arg == "switch::a@example.com")] | length == 1' "switch row for default account missing"
assert_jq_json "$auth_root_json" '[.items[] | select((.title // "") | test("google-cli ready"; "i"))] | length == 0' "gsa should not contain runtime row"

drive_help_json="$(run_with_env bash "$script_filter_drive" "")"
assert_jq_json "$drive_help_json" '.items | length >= 2' "gsd help query should emit home and usage rows"
assert_jq_json "$drive_help_json" '[.items[] | select(.arg == "drive-open-home")] | length == 1' "gsd home item missing"
assert_jq_json "$drive_help_json" '[.items[] | select(.autocomplete == "search ")] | length == 1' "gsd search hint item missing"

drive_search_json="$(run_with_env bash "$script_filter_drive" "search keyboard")"
assert_jq_json "$drive_search_json" '.items | length == 2' "gsd search should emit two rows from fixture"
assert_jq_json "$drive_search_json" '.items[0].arg == "drive-download::file-1::2"' "drive download token should include result.count"
assert_jq_json "$drive_search_json" '.items[0].mods.cmd.arg == "drive-open-search::keyboard"' "cmd modifier should open drive web search"
assert_jq_json "$drive_search_json" '.items[0].subtitle | test("2.00 MB")' "drive subtitle should format MB size"
assert_jq_json "$drive_search_json" '.items[1].subtitle | test("2.00 KB")' "drive subtitle should format KB size"
assert_jq_json "$drive_search_json" '.items[0].variables.GOOGLE_DRIVE_SEARCH_RESULT_COUNT == "2"' "workflow variable should include drive result.count"
assert_jq_json "$drive_search_json" '.items[0].variables.GOOGLE_DRIVE_FILE_ID == "file-1"' "workflow variable should include file id"

mail_help_json="$(run_with_env bash "$script_filter_mail" "")"
assert_jq_json "$mail_help_json" '.items | length >= 4' "gsm help query should emit inbox and usage rows"
assert_jq_json "$mail_help_json" '[.items[] | select(.arg == "gmail-open-home")] | length == 1' "gsm inbox open item missing"
assert_jq_json "$mail_help_json" '[.items[] | select(.title == "Unread Mail List (2)")] | length == 1' "gsm unread title should include unread count"
assert_jq_json "$mail_help_json" '[.items[] | select(.autocomplete == "unread ")] | length == 1' "gsm unread hint item missing"
assert_jq_json "$mail_help_json" '[.items[] | select(.autocomplete == "unread ")][0].variables.GOOGLE_MAIL_UNREAD_COUNT == "2"' "gsm unread hint should expose unread count variable"
assert_jq_json "$mail_help_json" '[.items[] | select(.autocomplete == "unread ")][0].variables.GOOGLE_MAIL_QUERY_MODE == "unread"' "gsm unread hint should expose query mode variable"
assert_jq_json "$mail_help_json" '[.items[] | select(.autocomplete == "latest ")] | length == 1' "gsm latest hint item missing"
assert_jq_json "$mail_help_json" '[.items[] | select(.autocomplete == "search ")] | length == 1' "gsm search hint item missing"

mail_unread_json="$(run_with_env bash "$script_filter_mail" "unread")"
assert_jq_json "$mail_unread_json" '.items | length == 2' "gsm unread should emit unread rows from fixture"
assert_jq_json "$mail_unread_json" '.items[0].arg == "gmail-open-message::msg-1"' "gsm unread first action token mismatch"
assert_jq_json "$mail_unread_json" '.items[0].mods.cmd.arg == "gmail-open-search::in:inbox is:unread"' "gsm unread cmd modifier should open gmail web search"
assert_jq_json "$mail_unread_json" '.items[0].variables.GOOGLE_MAIL_SEARCH_RESULT_COUNT == "2"' "gsm unread should set result.count variable"
assert_jq_json "$mail_unread_json" '.items[0].variables.GOOGLE_MAIL_MESSAGE_ID == "msg-1"' "gsm unread should set message id variable"
assert_jq_json "$mail_unread_json" '.items[0].variables.GOOGLE_MAIL_MESSAGE_THREAD_ID == "thread-1"' "gsm unread should set thread id variable"
assert_jq_json "$mail_unread_json" '.items[0].variables.GOOGLE_MAIL_QUERY_MODE == "unread"' "gsm unread should set query mode variable"

mail_latest_json="$(run_with_env bash "$script_filter_mail" "latest")"
assert_jq_json "$mail_latest_json" '.items | length == 3' "gsm latest should emit latest inbox rows from fixture"
assert_jq_json "$mail_latest_json" '.items[0].subtitle | test("mode=latest")' "gsm latest subtitle should include mode marker"

mail_search_json="$(run_with_env bash "$script_filter_mail" "search keyboard")"
assert_jq_json "$mail_search_json" '.items | length == 2' "gsm search should emit keyboard rows"
assert_jq_json "$mail_search_json" '.items[0].title | test("keyboard"; "i")' "gsm search should include keyboard subject"

mail_search_limited_json="$(env "${base_env[@]}" GOOGLE_MAIL_SEARCH_MAX=1 bash "$script_filter_mail" "search keyboard")"
assert_jq_json "$mail_search_limited_json" '.items | length == 1' "gsm search should respect GOOGLE_MAIL_SEARCH_MAX"

mail_latest_limited_json="$(env "${base_env[@]}" GOOGLE_MAIL_LATEST_MAX=2 bash "$script_filter_mail" "latest")"
assert_jq_json "$mail_latest_limited_json" '.items | length == 2' "gsm latest should respect GOOGLE_MAIL_LATEST_MAX"

login_step1_json="$(run_with_env bash "$script_filter" "login c@example.com")"
assert_jq_json "$login_step1_json" '[.items[] | select(.arg == "login::remote::step1::c@example.com")] | length == 1' "login step1 token mismatch"

login_step2_json="$(run_with_env bash "$script_filter" "login c@example.com --remote --step 2 --state state-c@example.com --code code-123")"
assert_jq_json "$login_step2_json" '[.items[] | select(.arg == "login::remote::step2::c@example.com::state-c@example.com::code-123")] | length == 1' "login step2 token mismatch"

login_step2_url_json="$(run_with_env bash "$script_filter" "login c@example.com http://localhost/?state=state-c@example.com&code=code-123&scope=x")"
assert_jq_json "$login_step2_url_json" '[.items[] | select(.arg == "login::remote::step2::c@example.com::state-c@example.com::code-123")] | length == 1' "login step2 callback-url shortcut token mismatch"

login_step2_url_without_email_json="$(run_with_env bash "$script_filter" "login http://localhost/?state=state-c@example.com&code=code-123&scope=x")"
assert_jq_json "$login_step2_url_without_email_json" '[.items[] | select(.arg == "login::remote::step2::c@example.com::state-c@example.com::code-123")] | length == 1' "login step2 callback-url (without email) token mismatch"

direct_callback_query_json="$(run_with_env bash "$script_filter" "http://localhost/?state=state-c@example.com&code=code-123&scope=x")"
assert_jq_json "$direct_callback_query_json" '[.items[] | select(.arg == "login::remote::step2::c@example.com::state-c@example.com::code-123")] | length == 1' "direct callback query token mismatch"

login_manual_json="$(run_with_env bash "$script_filter" "login c@example.com --manual --code manual-code")"
assert_jq_json "$login_manual_json" '[.items[] | select(.arg == "login::manual::c@example.com::manual-code")] | length == 1' "manual login token mismatch"

run_with_env bash "$action_open" "prompt::login" >/dev/null
run_with_env bash "$action_open" "prompt::auth" >/dev/null
run_with_env bash "$action_open" "prompt::switch" >/dev/null
run_with_env bash "$action_open" "prompt::remove" >/dev/null
run_with_env bash "$action_open" "prompt::mail-unread" >/dev/null

run_with_env bash "$action_open" "switch::b@example.com" >/dev/null
active_file="$smoke_tmp/data/active-account.v1.json"
assert_file "$active_file"
assert_jq_file "$active_file" '.active_account == "b@example.com"' "switch action should persist active account"
root_after_switch_json="$(run_with_env bash "$script_filter_empty" "")"
assert_jq_json "$root_after_switch_json" '.items[0].title == "Current account: b@example.com"' "gs should show workflow active account after switch"

step1_output="$(run_with_env bash "$action_open" "login::remote::step1::c@example.com")"
assert_jq_json "$step1_output" '.ok == true and .result.step == 1' "remote step1 output mismatch"

step2_output="$(run_with_env bash "$action_open" "login::remote::step2::c@example.com::state-c@example.com::code-123")"
assert_jq_json "$step2_output" '.ok == true and .result.step == 2' "remote step2 output mismatch"
assert_jq_file "$active_file" '.active_account == "c@example.com"' "step2 should set active account"

remove_output="$(run_with_env bash "$action_open" "remove::c@example.com::1")"
assert_jq_json "$remove_output" '.ok == true and .result.account == "c@example.com"' "remove output mismatch"
assert_jq_file "$active_file" '.active_account == "a@example.com"' "remove should rebalance active account"

drive_download_output="$(run_with_env bash "$action_open" "drive-download::file-1::2")"
assert_jq_json "$drive_download_output" '.ok == true and .command == "google.drive.download"' "drive download output mismatch"
assert_file "$smoke_tmp/home/Downloads/Keyboard_Configuration.docx"

run_with_env bash "$action_open" "drive-open-home" >/dev/null
run_with_env bash "$action_open" "drive-open-search::keyboard" >/dev/null
run_with_env bash "$action_open" "gmail-open-home" >/dev/null
run_with_env bash "$action_open" "gmail-open-search::keyboard" >/dev/null
run_with_env bash "$action_open" "gmail-open-message::msg-1" >/dev/null

if ! rg -n "auth add c@example.com --remote --step 1" "$stub_log" >/dev/null; then
  fail "stub log missing remote step1 invocation"
fi
if ! rg -n "auth add c@example.com --remote --step 2 --state state-c@example.com --code code-123" "$stub_log" >/dev/null; then
  fail "stub log missing remote step2 invocation"
fi
if ! rg -n "auth remove c@example.com" "$stub_log" >/dev/null; then
  fail "stub log missing remove invocation"
fi
if ! rg -n "drive get file-1" "$stub_log" >/dev/null; then
  fail "stub log missing drive get invocation"
fi
if ! rg -n "drive download file-1 --format docx --out .*Downloads/Keyboard_Configuration.docx" "$stub_log" >/dev/null; then
  fail "stub log missing drive download invocation"
fi
if ! rg -n "gmail search --max 25 --format metadata --headers Subject,From,Date --query in:inbox is:unread" "$stub_log" >/dev/null; then
  fail "stub log missing gmail unread search invocation"
fi
if ! rg -n "gmail search --max 25 --format metadata --headers Subject,From,Date --query in:inbox" "$stub_log" >/dev/null; then
  fail "stub log missing gmail latest search invocation"
fi
if ! rg -n "gmail search --max 25 --format metadata --headers Subject,From,Date --query keyboard" "$stub_log" >/dev/null; then
  fail "stub log missing gmail keyword search invocation"
fi

echo "ok: google-service workflow smoke test"

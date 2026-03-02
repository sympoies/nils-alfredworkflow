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
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter_empty.sh \
  scripts/script_filter.sh \
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
  GOOGLE_CLI_KEYRING_MODE; do
  if ! rg -n "^${env_key}[[:space:]]*=[[:space:]]*\"\"" "$manifest" >/dev/null; then
    fail "${env_key} default must be empty"
  fi
done

if ! rg -n '^GOOGLE_AUTH_REMOVE_CONFIRM[[:space:]]*=[[:space:]]*"1"' "$manifest" >/dev/null; then
  fail "GOOGLE_AUTH_REMOVE_CONFIRM default must be 1"
fi

plist_json="$(plist_to_json "$workflow_dir/src/info.plist.template")"
assert_jq_json "$plist_json" '.objects | type == "array" and length == 3' "plist should contain two script filters and one action"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter")] | length == 2' "script filter count mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gs" and .config.scriptfile == "./scripts/script_filter_empty.sh")] | length == 1' "gs keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "gsa" and .config.scriptfile == "./scripts/script_filter.sh")] | length == 1' "gsa keyword binding mismatch"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaycustom == 1] | all' "queue delay custom must be 1"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelayimmediatelyinitially == false] | all' "queue immediate initial must be false"
assert_jq_json "$plist_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.alfredfiltersresults == false] | all' "alfredfiltersresults must be false"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.action.script") | .config.scriptfile == "./scripts/action_open.sh"' "action script path mismatch"

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

log "$*"

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
*)
  emit_error "google.unknown" "unsupported command"
  exit 2
  ;;
esac
EOS
chmod +x "$smoke_tmp/bin/google-cli"

script_filter_empty="$workflow_dir/scripts/script_filter_empty.sh"
script_filter="$workflow_dir/scripts/script_filter.sh"
action_open="$workflow_dir/scripts/action_open.sh"

base_env=(
  "GOOGLE_CLI_BIN=$smoke_tmp/bin/google-cli"
  "STUB_ACCOUNTS_FILE=$stub_accounts_file"
  "STUB_LOG=$stub_log"
  "GOOGLE_CLI_CONFIG_DIR=$smoke_tmp/config"
  "ALFRED_WORKFLOW_DATA=$smoke_tmp/data"
  "GOOGLE_AUTH_REMOVE_CONFIRM=0"
)

run_with_env() {
  env "${base_env[@]}" "$@"
}

root_json="$(run_with_env bash "$script_filter_empty" "")"
assert_jq_json "$root_json" '.items | length == 1' "gs root query should emit one account status row"
assert_jq_json "$root_json" '.items[0].title == "Current account: a@example.com"' "gs should show default account when active account is not set"

auth_root_json="$(run_with_env bash "$script_filter" "")"
assert_jq_json "$auth_root_json" '.items | length >= 5' "gsa root query should emit command and account rows"
assert_jq_json "$auth_root_json" '.items[0].title == "Google Service Auth Login" and .items[0].arg == "prompt::login"' "gsa command row login mismatch"
assert_jq_json "$auth_root_json" '.items[1].title == "Google Service Auth Switch" and .items[1].arg == "prompt::switch"' "gsa command row switch mismatch"
assert_jq_json "$auth_root_json" '.items[2].title == "Google Service Auth Remove" and .items[2].arg == "prompt::remove"' "gsa command row remove mismatch"
assert_jq_json "$auth_root_json" '[.items[] | select(.arg == "switch::a@example.com")] | length == 1' "switch row for default account missing"
assert_jq_json "$auth_root_json" '[.items[] | select((.title // "") | test("google-cli ready"; "i"))] | length == 0' "gsa should not contain runtime row"

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
run_with_env bash "$action_open" "prompt::switch" >/dev/null
run_with_env bash "$action_open" "prompt::remove" >/dev/null

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

if ! rg -n "auth add c@example.com --remote --step 1" "$stub_log" >/dev/null; then
  fail "stub log missing remote step1 invocation"
fi
if ! rg -n "auth add c@example.com --remote --step 2 --state state-c@example.com --code code-123" "$stub_log" >/dev/null; then
  fail "stub log missing remote step2 invocation"
fi
if ! rg -n "auth remove c@example.com" "$stub_log" >/dev/null; then
  fail "stub log missing remove invocation"
fi

echo "ok: google-service workflow smoke test"

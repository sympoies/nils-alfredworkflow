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
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter_entry.sh \
  scripts/script_filter.sh \
  scripts/script_filter_add.sh \
  scripts/script_filter_update.sh \
  scripts/script_filter_delete.sh \
  scripts/script_filter_copy.sh \
  scripts/script_filter_search.sh \
  scripts/script_filter_recent.sh \
  scripts/action_run.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter_entry.sh \
  scripts/script_filter.sh \
  scripts/script_filter_add.sh \
  scripts/script_filter_update.sh \
  scripts/script_filter_delete.sh \
  scripts/script_filter_copy.sh \
  scripts/script_filter_search.sh \
  scripts/script_filter_recent.sh \
  scripts/action_run.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "memo-add" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "memo-workflow-cli" ]] || fail "rust_binary mismatch"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter_entry.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_run.sh" ]] || fail "action mismatch"

for variable in MEMO_DB_PATH MEMO_SOURCE MEMO_REQUIRE_CONFIRM MEMO_MAX_INPUT_BYTES MEMO_RECENT_LIMIT MEMO_SEARCH_MATCH MEMO_WORKFLOW_CLI_BIN; do
  rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null || fail "missing env var: $variable"
done

rg -n '^MEMO_SOURCE[[:space:]]*=[[:space:]]*"alfred"' "$manifest" >/dev/null || fail "MEMO_SOURCE default mismatch"
rg -n '^MEMO_REQUIRE_CONFIRM[[:space:]]*=[[:space:]]*"0"' "$manifest" >/dev/null || fail "MEMO_REQUIRE_CONFIRM default mismatch"
rg -n '^MEMO_MAX_INPUT_BYTES[[:space:]]*=[[:space:]]*"4096"' "$manifest" >/dev/null || fail "MEMO_MAX_INPUT_BYTES default mismatch"
rg -n '^MEMO_RECENT_LIMIT[[:space:]]*=[[:space:]]*"8"' "$manifest" >/dev/null || fail "MEMO_RECENT_LIMIT default mismatch"
rg -n '^MEMO_SEARCH_MATCH[[:space:]]*=[[:space:]]*"fts"' "$manifest" >/dev/null || fail "MEMO_SEARCH_MATCH default mismatch"

workflow_smoke_assert_action_requires_arg "$workflow_dir/scripts/action_run.sh"

tmp_dir="$(mktemp -d)"
crud_tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir" "$crud_tmp_dir"
}
trap cleanup EXIT
mkdir -p "$tmp_dir/stubs"

cat >"$tmp_dir/stubs/memo-workflow-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
db_path="${MEMO_DB_PATH:-${TMPDIR:-/tmp}/memo-smoke.db}"
state_file="${db_path}.state"

ensure_state_parent() {
  mkdir -p "$(dirname "$state_file")"
}

emit_item() {
  local title="$1"
  local token="$2"
  printf '{"items":[{"title":"%s","subtitle":"ok","arg":"%s","valid":true}]}\n' "$title" "$token"
}

item_route_id() {
  local item_id="$1"
  if [[ "$item_id" =~ ^itm_([0-9]+)$ ]]; then
    printf '%d' "$((10#${BASH_REMATCH[1]}))"
    return 0
  fi

  printf '%s' "$item_id"
}

item_display_id() {
  local item_id="$1"
  if [[ "$item_id" =~ ^itm_([0-9]+)$ ]]; then
    printf '#%d' "$((10#${BASH_REMATCH[1]}))"
    return 0
  fi

  printf '%s' "$item_id"
}

emit_copy_item() {
  local item_id="$1"
  local memo_text="${2:-seed memo}"
  local item_display
  item_display="$(item_display_id "$item_id")"
  printf '{"items":[{"title":"Copy memo: %s | %s","arg":"copy::%s","valid":true,"mods":{"cmd":{"subtitle":"raw json","arg":"copy-json::%s","valid":true}}}]}\n' "$item_display" "$memo_text" "$item_id" "$item_id"
}

emit_update_guidance() {
  local item_id="$1"
  local memo_text="${2:-seed memo}"
  local item_display item_route
  item_display="$(item_display_id "$item_id")"
  item_route="$(item_route_id "$item_id")"
  printf '{"items":[{"title":"Update memo: %s | %s","autocomplete":"update %s ","valid":false}]}\n' "$item_display" "$memo_text" "$item_route"
}

emit_item_menu() {
  local item_id="$1"
  local memo_text="${2:-seed memo}"
  local item_display item_route
  item_display="$(item_display_id "$item_id")"
  item_route="$(item_route_id "$item_id")"
  printf '{"items":[{"title":"Copy memo: %s | %s","arg":"copy::%s","valid":true,"mods":{"cmd":{"subtitle":"raw json","arg":"copy-json::%s","valid":true}}},{"title":"Update memo: %s | %s","autocomplete":"update %s ","valid":false},{"title":"Delete memo: %s | %s","arg":"delete::%s","valid":true}]}\n' "$item_display" "$memo_text" "$item_id" "$item_id" "$item_display" "$memo_text" "$item_route" "$item_display" "$memo_text" "$item_id"
}

if [[ "${1:-}" == "script-filter" && "${2:-}" == "--query" ]]; then
  query="${3:-}"
  case "$query" in
    "buy milk")
      emit_item "Add memo: buy milk" "add::buy milk"
      ;;
    "update itm_00000001 buy oat milk")
      emit_item "Update memo: #1" "update::itm_00000001::buy oat milk"
      ;;
    "update 1")
      emit_update_guidance "itm_00000001" "buy milk"
      ;;
    "delete itm_00000001")
      emit_item "Delete memo: #1" "delete::itm_00000001"
      ;;
    "delete 1")
      emit_item "Delete memo: #1" "delete::itm_00000001"
      ;;
    "copy itm_00000001")
      emit_copy_item "itm_00000001" "buy milk"
      ;;
    "copy 1")
      emit_copy_item "itm_00000001" "buy milk"
      ;;
    "search")
      printf '{"items":[{"title":"Type search text after keyword","subtitle":"Use: search <query>","valid":false}]}\n'
      ;;
    "search milk")
      printf '{"items":[{"title":"Search #1: buy milk","subtitle":"search","autocomplete":"item 1","valid":false}]}\n'
      ;;
    "item itm_00000001")
      emit_item_menu "itm_00000001" "buy milk"
      ;;
    "item 1")
      emit_item_menu "itm_00000001" "buy milk"
      ;;
    *)
      emit_item "Add memo: $query" "add::$query"
      ;;
  esac
  exit 0
fi

if [[ "${1:-}" == "action" && "${2:-}" == "--token" ]]; then
  token="${3:-}"
  ensure_state_parent
  case "$token" in
    db-init)
      : >"$db_path"
      : >"$state_file"
      printf 'initialized %s\n' "$db_path"
      exit 0
      ;;
    add::*)
      text="${token#add::}"
      row_count=0
      if [[ -f "$state_file" ]]; then
        row_count="$(wc -l <"$state_file")"
      fi
      item_num=$((row_count + 1))
      printf -v item_id 'itm_%08d' "$item_num"
      printf '%s\t%s\n' "$item_id" "$text" >>"$state_file"
      printf 'added %s at 2026-02-12T12:00:00Z\n' "$item_id"
      exit 0
      ;;
    update::*)
      payload="${token#update::}"
      item_id="${payload%%::*}"
      text="${payload#*::}"
      tmp_state="${state_file}.tmp"
      found=0
      : >"$tmp_state"
      if [[ -f "$state_file" ]]; then
        while IFS=$'\t' read -r row_id row_text; do
          if [[ "$row_id" == "$item_id" ]]; then
            printf '%s\t%s\n' "$row_id" "$text" >>"$tmp_state"
            found=1
          elif [[ -n "${row_id:-}" ]]; then
            printf '%s\t%s\n' "$row_id" "$row_text" >>"$tmp_state"
          fi
        done <"$state_file"
      fi
      if [[ "$found" -eq 0 ]]; then
        rm -f "$tmp_state"
        echo "item not found: $item_id" >&2
        exit 4
      fi
      mv "$tmp_state" "$state_file"
      printf 'updated %s at 2026-02-12T12:05:00Z\n' "$item_id"
      exit 0
      ;;
    delete::*)
      item_id="${token#delete::}"
      tmp_state="${state_file}.tmp"
      found=0
      : >"$tmp_state"
      if [[ -f "$state_file" ]]; then
        while IFS=$'\t' read -r row_id row_text; do
          if [[ "$row_id" == "$item_id" ]]; then
            found=1
            continue
          fi
          if [[ -n "${row_id:-}" ]]; then
            printf '%s\t%s\n' "$row_id" "$row_text" >>"$tmp_state"
          fi
        done <"$state_file"
      fi
      if [[ "$found" -eq 0 ]]; then
        rm -f "$tmp_state"
        echo "item not found: $item_id" >&2
        exit 4
      fi
      mv "$tmp_state" "$state_file"
      printf 'deleted %s at 2026-02-12T12:10:00Z\n' "$item_id"
      exit 0
      ;;
    copy::*)
      item_id="${token#copy::}"
      if [[ -f "$state_file" ]]; then
        while IFS=$'\t' read -r row_id row_text; do
          if [[ "$row_id" == "$item_id" ]]; then
            printf '%s' "$row_text"
            exit 0
          fi
        done <"$state_file"
      fi
      echo "item not found: $item_id" >&2
      exit 4
      ;;
    copy-json::*)
      item_id="${token#copy-json::}"
      if [[ -f "$state_file" ]]; then
        while IFS=$'\t' read -r row_id row_text; do
          if [[ "$row_id" == "$item_id" ]]; then
            printf '{"item_id":"%s","text":"%s"}' "$row_id" "$row_text"
            exit 0
          fi
        done <"$state_file"
      fi
      echo "item not found: $item_id" >&2
      exit 4
      ;;
  esac
fi

exit 9
EOS
chmod +x "$tmp_dir/stubs/memo-workflow-cli-ok"

cat >"$tmp_dir/stubs/memo-workflow-cli-invalid" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid MEMO_MAX_INPUT_BYTES" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/memo-workflow-cli-invalid"

cat >"$tmp_dir/stubs/osascript" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${MEMO_NOTIFY_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$MEMO_NOTIFY_LOG"
fi
EOS
chmod +x "$tmp_dir/stubs/osascript"

cat >"$tmp_dir/stubs/pbcopy" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${MEMO_CLIPBOARD_LOG:-}" ]]; then
  cat >"$MEMO_CLIPBOARD_LOG"
else
  cat >/dev/null
fi
EOS
chmod +x "$tmp_dir/stubs/pbcopy"

success_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "buy milk"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must return one item"
assert_jq_json "$success_json" '.items[0].arg == "add::buy milk"' "script_filter add arg mismatch"

entry_json="$("$workflow_dir/scripts/script_filter_entry.sh" "buy milk")"
assert_jq_json "$entry_json" '.items[0].title == "Memo Commands"' "entry menu title mismatch"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "r ")' "entry menu should include r suffix for mmr"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "a ")' "entry menu should include a suffix for mma"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "u ")' "entry menu should include u suffix for mmu"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "d ")' "entry menu should include d suffix for mmd"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "c ")' "entry menu should include c suffix for mmc"
assert_jq_json "$entry_json" '.items | any(.autocomplete == "q ")' "entry menu should include q suffix for mmq"
assert_jq_json "$entry_json" '.items | map(select((.arg // "") | startswith("add::"))) | length == 0' "entry menu should not return add action tokens"

keyword_add_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_add.sh" "buy milk"; })"
assert_jq_json "$keyword_add_json" '.items[0].arg == "add::buy milk"' "mma add arg mismatch"

keyword_update_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_update.sh" "itm_00000001 buy oat milk"; })"
assert_jq_json "$keyword_update_json" '.items[0].arg == "update::itm_00000001::buy oat milk"' "mmu update arg mismatch"

keyword_delete_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_delete.sh" "itm_00000001"; })"
assert_jq_json "$keyword_delete_json" '.items[0].arg == "delete::itm_00000001"' "mmd delete arg mismatch"

keyword_update_recent_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_update.sh" ""; })"
assert_jq_json "$keyword_update_recent_json" '.items[0].arg == "add::"' "mmu empty query should map to newest-first list"

keyword_update_id_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_update.sh" "1"; })"
assert_jq_json "$keyword_update_id_json" '.items | length == 1' "mmu numeric query should not show full item menu"
assert_jq_json "$keyword_update_id_json" '(.items[0].title | startswith("Update memo: #1 |"))' "mmu numeric query should include memo preview in update title"
assert_jq_json "$keyword_update_id_json" '.items[0].autocomplete == "update 1 "' "mmu numeric query should keep update autocomplete"

keyword_delete_recent_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_delete.sh" ""; })"
assert_jq_json "$keyword_delete_recent_json" '.items[0].arg == "add::"' "mmd empty query should map to newest-first list"

keyword_delete_id_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_delete.sh" "1"; })"
assert_jq_json "$keyword_delete_id_json" '.items | length == 1' "mmd numeric query should not show full item menu"
assert_jq_json "$keyword_delete_id_json" '.items[0].arg == "delete::itm_00000001"' "mmd numeric query should map to delete action"

keyword_copy_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_copy.sh" "itm_00000001"; })"
assert_jq_json "$keyword_copy_json" '.items[0].arg == "copy::itm_00000001"' "mmc copy menu mismatch"
assert_jq_json "$keyword_copy_json" '.items | length == 1' "mmc id query should not show full item menu"
assert_jq_json "$keyword_copy_json" '(.items[0].title | startswith("Copy memo: #1 |"))' "mmc copy title should include memo preview"

keyword_copy_recent_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_copy.sh" ""; })"
assert_jq_json "$keyword_copy_recent_json" '.items[0].arg == "add::"' "mmc empty query should map to newest-first list"

keyword_copy_id_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_copy.sh" "1"; })"
assert_jq_json "$keyword_copy_id_json" '.items | length == 1' "mmc numeric query should not show full item menu"
assert_jq_json "$keyword_copy_id_json" '.items[0].arg == "copy::itm_00000001"' "mmc numeric query should map to copy action"
assert_jq_json "$keyword_copy_id_json" '(.items[0].title | startswith("Copy memo: #1 |"))' "mmc numeric query should include memo preview"

keyword_search_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_search.sh" "milk"; })"
assert_jq_json "$keyword_search_json" '.items | length == 1' "mmq query should return search rows"
assert_jq_json "$keyword_search_json" '.items[0].autocomplete == "item 1"' "mmq should route to item autocomplete"

keyword_search_item_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_search.sh" "item 1"; })"
assert_jq_json "$keyword_search_item_json" '.items | length == 3' "mmq item intent should keep full item menu"
assert_jq_json "$keyword_search_item_json" '.items[0].arg == "copy::itm_00000001"' "mmq item intent should include copy action"

keyword_search_empty_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_search.sh" ""; })"
assert_jq_json "$keyword_search_empty_json" '.items[0].valid == false' "mmq empty query should show guidance row"
assert_jq_json "$keyword_search_empty_json" '([.items[].arg // ""] | all(startswith("add::") | not))' "mmq empty query should not return add token"

keyword_recent_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_recent.sh" "buy milk"; })"
assert_jq_json "$keyword_recent_json" '.items[0].arg == "add::"' "mmr should force empty query for newest-first view"

keyword_recent_id_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_recent.sh" "1"; })"
assert_jq_json "$keyword_recent_id_json" '.items[0].arg == "copy::itm_00000001"' "mmr numeric query should map to item lookup"
assert_jq_json "$keyword_recent_id_json" '.items | length == 3' "mmr numeric query should keep full item menu"
keyword_recent_item_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter_recent.sh" "item 1"; })"
assert_jq_json "$keyword_recent_item_json" '.items[0].arg == "copy::itm_00000001"' "mmr item intent should passthrough to item lookup"
assert_jq_json "$keyword_recent_item_json" '.items | length == 3' "mmr item intent should keep full item menu"

success_env_query_json="$({ alfred_workflow_query="buy milk" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$success_env_query_json" '.items[0].arg == "add::buy milk"' "script_filter alfred_workflow_query fallback mismatch"

success_null_placeholder_json="$({ alfred_workflow_query="buy milk" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "(null)"; })"
assert_jq_json "$success_null_placeholder_json" '.items[0].arg == "add::buy milk"' "script_filter (null) fallback mismatch"

success_stdin_query_json="$(printf 'buy milk' | MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$success_stdin_query_json" '.items[0].arg == "add::buy milk"' "script_filter stdin fallback mismatch"

update_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "update itm_00000001 buy oat milk"; })"
assert_jq_json "$update_json" '.items[0].arg == "update::itm_00000001::buy oat milk"' "script_filter update arg mismatch"

delete_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "delete itm_00000001"; })"
assert_jq_json "$delete_json" '.items[0].arg == "delete::itm_00000001"' "script_filter delete arg mismatch"

item_menu_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "item itm_00000001"; })"
assert_jq_json "$item_menu_json" '.items | type == "array" and length == 3' "script_filter item menu length mismatch"
assert_jq_json "$item_menu_json" '.items[0].arg == "copy::itm_00000001"' "script_filter copy arg mismatch"
assert_jq_json "$item_menu_json" '(.items[0].title | startswith("Copy memo: #1 |"))' "script_filter item copy title should include memo preview"
assert_jq_json "$item_menu_json" '.items[0].mods.cmd.arg == "copy-json::itm_00000001"' "script_filter copy-json cmd arg mismatch"
assert_jq_json "$item_menu_json" '(.items[1].title | startswith("Update memo: #1 |"))' "script_filter item update title should include memo preview"
assert_jq_json "$item_menu_json" '.items[1].autocomplete == "update 1 "' "script_filter item update autocomplete mismatch"
assert_jq_json "$item_menu_json" '.items[2].arg == "delete::itm_00000001"' "script_filter item delete arg mismatch"

invalid_json="$({ MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-invalid" "$workflow_dir/scripts/script_filter.sh" "buy milk"; })"
assert_jq_json "$invalid_json" '.items[0].title == "Invalid Memo workflow config"' "invalid config title mismatch"

action_output="$({ MEMO_DB_PATH="$tmp_dir/smoke.db" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/action_run.sh" "add::buy milk"; })"
[[ "$action_output" == *"added itm_00000001"* ]] || fail "action output mismatch"

crud_db_path="$crud_tmp_dir/memo.db"
crud_state_path="${crud_db_path}.state"
notify_log="$crud_tmp_dir/notify.log"
clipboard_log="$crud_tmp_dir/clipboard.log"

db_init_output="$({
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "db-init"
})"
[[ "$db_init_output" == *"initialized $crud_db_path"* ]] || fail "db-init output mismatch"
rg -n --fixed-strings 'Memo DB initialized' "$notify_log" >/dev/null || fail "db-init notification mismatch"

crud_add_json="$({ MEMO_DB_PATH="$crud_db_path" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "buy milk"; })"
crud_add_token="$(jq -r '.items[0].arg' <<<"$crud_add_json")"
[[ "$crud_add_token" == "add::buy milk" ]] || fail "crud add token mismatch"
: >"$notify_log"
crud_add_output="$({
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "$crud_add_token"
})"
[[ "$crud_add_output" == *"added itm_00000001"* ]] || fail "crud add output mismatch"
rg -n --fixed-strings 'Memo added' "$notify_log" >/dev/null || fail "add notification mismatch"
rg -n '^itm_00000001[[:space:]]+buy milk$' "$crud_state_path" >/dev/null || fail "crud add state mismatch"

crud_update_json="$({ MEMO_DB_PATH="$crud_db_path" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "update itm_00000001 buy oat milk"; })"
crud_update_token="$(jq -r '.items[0].arg' <<<"$crud_update_json")"
[[ "$crud_update_token" == "update::itm_00000001::buy oat milk" ]] || fail "crud update token mismatch"
: >"$notify_log"
crud_update_output="$({
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "$crud_update_token"
})"
[[ "$crud_update_output" == *"updated itm_00000001"* ]] || fail "crud update output mismatch"
rg -n --fixed-strings 'Memo updated' "$notify_log" >/dev/null || fail "update notification mismatch"
rg -n '^itm_00000001[[:space:]]+buy oat milk$' "$crud_state_path" >/dev/null || fail "crud update state mismatch"

crud_copy_json="$({ MEMO_DB_PATH="$crud_db_path" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "item itm_00000001"; })"
crud_copy_token="$(jq -r '.items[0].arg' <<<"$crud_copy_json")"
[[ "$crud_copy_token" == "copy::itm_00000001" ]] || fail "crud copy token mismatch"
: >"$notify_log"
: >"$clipboard_log"
{
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_CLIPBOARD_LOG="$clipboard_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "$crud_copy_token"
} >/dev/null
rg -n --fixed-strings 'Memo copied' "$notify_log" >/dev/null || fail "copy notification mismatch"
[[ "$(cat "$clipboard_log")" == "buy oat milk" ]] || fail "copy clipboard mismatch"

crud_copy_json_token="$(jq -r '.items[0].mods.cmd.arg' <<<"$crud_copy_json")"
[[ "$crud_copy_json_token" == "copy-json::itm_00000001" ]] || fail "crud copy-json token mismatch"
: >"$notify_log"
: >"$clipboard_log"
{
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_CLIPBOARD_LOG="$clipboard_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "$crud_copy_json_token"
} >/dev/null
rg -n --fixed-strings 'Memo JSON copied' "$notify_log" >/dev/null || fail "copy-json notification mismatch"
assert_jq_json "$(cat "$clipboard_log")" '.item_id == "itm_00000001" and .text == "buy oat milk"' "copy-json clipboard payload mismatch"

crud_delete_json="$({ MEMO_DB_PATH="$crud_db_path" MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" "$workflow_dir/scripts/script_filter.sh" "delete itm_00000001"; })"
crud_delete_token="$(jq -r '.items[0].arg' <<<"$crud_delete_json")"
[[ "$crud_delete_token" == "delete::itm_00000001" ]] || fail "crud delete token mismatch"
: >"$notify_log"
crud_delete_output="$({
  PATH="$tmp_dir/stubs:$PATH" \
    MEMO_NOTIFY_LOG="$notify_log" \
    MEMO_DB_PATH="$crud_db_path" \
    MEMO_WORKFLOW_CLI_BIN="$tmp_dir/stubs/memo-workflow-cli-ok" \
    "$workflow_dir/scripts/action_run.sh" "$crud_delete_token"
})"
[[ "$crud_delete_output" == *"deleted itm_00000001"* ]] || fail "crud delete output mismatch"
rg -n --fixed-strings 'Memo deleted' "$notify_log" >/dev/null || fail "delete notification mismatch"
[[ ! -s "$crud_state_path" ]] || fail "crud delete should clear state"

cat >"$tmp_dir/stubs/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-memo-workflow-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/memo-workflow-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/memo-workflow-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/stubs/cargo"

PATH="$tmp_dir/stubs:$PATH" "$repo_root/scripts/workflow-pack.sh" --id memo-add >/dev/null

packaged_plist="$repo_root/build/workflows/memo-add/pkg/info.plist"
assert_file "$packaged_plist"
assert_file "$repo_root/build/workflows/memo-add/pkg/bin/memo-workflow-cli"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
  packaged_json="$(plutil -convert json -o - "$packaged_plist")"
else
  packaged_json="$(
    python3 - "$packaged_plist" <<'PY'
import json
import plistlib
import sys
with open(sys.argv[1], 'rb') as f:
    print(json.dumps(plistlib.load(f)))
PY
  )"
fi

assert_jq_json "$packaged_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter")] | length == 7' "scriptfilter count mismatch"
assert_jq_json "$packaged_json" '[.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.keyword] | sort == ["mma","mmc","mmd","mmq","mmr","mmu","mm||memo"]' "keyword wiring mismatch"
assert_jq_json "$packaged_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "mm||memo") | .config.scriptfile == "./scripts/script_filter_entry.sh"' "mm keyword should use command-entry script"
assert_jq_json "$packaged_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "mm||memo") | .config.withspace == false' "mm keyword should keep no-space suffix routing"
assert_jq_json "$packaged_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter" and .config.keyword == "mmq") | .config.scriptfile == "./scripts/script_filter_search.sh"' "mmq keyword should use search script"
assert_jq_json "$packaged_json" '.connections | length == 7' "connection wiring mismatch"
assert_jq_json "$packaged_json" '[.userconfigurationconfig[].variable] | sort == ["MEMO_DB_PATH","MEMO_MAX_INPUT_BYTES","MEMO_RECENT_LIMIT","MEMO_REQUIRE_CONFIRM","MEMO_SEARCH_MATCH","MEMO_SOURCE","MEMO_WORKFLOW_CLI_BIN"]' "plist variable list mismatch"
assert_jq_json "$packaged_json" '.userconfigurationconfig[] | select(.variable == "MEMO_MAX_INPUT_BYTES") | .config.default == "4096"' "plist default mismatch"
assert_jq_json "$packaged_json" '.userconfigurationconfig[] | select(.variable == "MEMO_RECENT_LIMIT") | .config.default == "8"' "plist recent limit default mismatch"
assert_jq_json "$packaged_json" '.userconfigurationconfig[] | select(.variable == "MEMO_SEARCH_MATCH") | .config.default == "fts"' "plist search match default mismatch"

echo "ok: memo-add smoke test"

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
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "steam-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "steam-cli" ]] || fail "rust_binary must be steam-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in STEAM_REGION STEAM_REGION_OPTIONS STEAM_SHOW_REGION_OPTIONS STEAM_LANGUAGE STEAM_MAX_RESULTS STEAM_SEARCH_API; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

if ! rg -n '^STEAM_SHOW_REGION_OPTIONS[[:space:]]*=[[:space:]]*"0"' "$manifest" >/dev/null; then
  fail "STEAM_SHOW_REGION_OPTIONS default must be 0"
fi

if ! rg -n '^STEAM_LANGUAGE[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "STEAM_LANGUAGE default must be empty"
fi

if ! rg -n '^STEAM_SEARCH_API[[:space:]]*=[[:space:]]*"search-suggestions"' "$manifest" >/dev/null; then
  fail "STEAM_SEARCH_API default must be search-suggestions"
fi

plist_json="$(plist_to_json "$workflow_dir/src/info.plist.template")"
script_filter_uid="6B7F46DF-B4AB-4D24-89FD-C90A15469E65"
action_uid="14EF02C5-6A95-4E03-95F6-E062AB6CF067"

assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.keyword == "st||steam"' "plist keyword wiring mismatch"
assert_jq_json "$plist_json" ".objects[] | select(.uid == \"$script_filter_uid\") | .config.scriptfile == \"./scripts/script_filter.sh\"" "script_filter scriptfile wiring mismatch"
assert_jq_json "$plist_json" ".objects[] | select(.uid == \"$action_uid\") | .config.scriptfile == \"./scripts/action_open.sh\"" "action scriptfile wiring mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaycustom == 1' "queue delay custom mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaymode == 0' "queue delay mode mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelayimmediatelyinitially == false' "queue immediate policy mismatch"
assert_jq_json "$plist_json" ".connections[\"$script_filter_uid\"] | any(.destinationuid == \"$action_uid\" and .modifiers == 0)" "script_filter connection graph mismatch"
assert_jq_json "$plist_json" '[.userconfigurationconfig[] | .variable] | sort == ["STEAM_LANGUAGE","STEAM_MAX_RESULTS","STEAM_REGION","STEAM_REGION_OPTIONS","STEAM_SEARCH_API","STEAM_SHOW_REGION_OPTIONS"]' "user configuration variables mismatch"
assert_jq_json "$plist_json" '.userconfigurationconfig[] | select(.variable=="STEAM_SHOW_REGION_OPTIONS") | .config.default == "0"' "STEAM_SHOW_REGION_OPTIONS default mismatch"
assert_jq_json "$plist_json" '.userconfigurationconfig[] | select(.variable=="STEAM_LANGUAGE") | .config.default == ""' "STEAM_LANGUAGE default mismatch"
assert_jq_json "$plist_json" '.userconfigurationconfig[] | select(.variable=="STEAM_SEARCH_API") | .config.default == "search-suggestions"' "STEAM_SEARCH_API default mismatch"

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/cache"
export STEAM_QUERY_CACHE_TTL_SECONDS=0
export STEAM_QUERY_COALESCE_SETTLE_SECONDS=0
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

release_cli="$repo_root/target/release/steam-cli"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"
release_backup="$(artifact_backup_file "$release_cli" "$tmp_dir" "steam-cli.release")"

cleanup() {
  artifact_restore_file "$release_cli" "$release_backup"
  artifact_restore_file "$artifact_path" "$artifact_backup"
  artifact_restore_file "$artifact_sha_path" "$artifact_sha_backup"
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"
workflow_smoke_write_open_stub "$tmp_dir/bin/open"
workflow_smoke_assert_action_requires_arg "$workflow_dir/scripts/action_open.sh"

action_arg="https://store.steampowered.com/app/620/Portal_2/"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/requery" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$STEAM_REQUERY_OUT"
EOS
chmod +x "$tmp_dir/stubs/requery"

ALFRED_WORKFLOW_CACHE="$tmp_dir/cache" \
  STEAM_REQUERY_OUT="$tmp_dir/requery.txt" \
  STEAM_REQUERY_COMMAND="$tmp_dir/stubs/requery" \
  "$workflow_dir/scripts/action_open.sh" "steam-requery:us:helldivers"

[[ "$(cat "$tmp_dir/requery.txt")" == "st helldivers" ]] || fail "steam requery text mismatch"
[[ "$(sed -n '1p' "$tmp_dir/cache/steam-region-override.state")" == "US" ]] || fail "steam region override state mismatch"

cat >"$tmp_dir/stubs/steam-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${STEAM_STUB_LOG:-}" ]]; then
  printf '%s | region=%s\n' "$*" "${STEAM_REGION:-}" >>"$STEAM_STUB_LOG"
fi
[[ "${1:-}" == "search" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
printf '{"items":[{"title":"Steam stub","subtitle":"query=%s","arg":"https://store.steampowered.com/app/620/","valid":true}]}' "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/steam-cli-ok"

cat >"$tmp_dir/stubs/steam-cli-invalid-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid STEAM_REGION: USA" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/steam-cli-invalid-config"

result_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" "$workflow_dir/scripts/script_filter.sh" "portal"; })"
assert_jq_json "$result_json" '.items[0].title == "Steam stub"' "script_filter success pass-through mismatch"
assert_jq_json "$result_json" '.items[0].subtitle == "query=portal"' "script_filter query forwarding mismatch"

env_query_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" alfred_workflow_query="portal 2" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].subtitle == "query=portal 2"' "script_filter env query fallback mismatch"

stdin_query_json="$(printf 'half-life' | STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].subtitle == "query=half-life"' "script_filter stdin query fallback mismatch"

short_query_log="$tmp_dir/steam-short-query.log"
short_query_json="$({ STEAM_STUB_LOG="$short_query_log" STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" "$workflow_dir/scripts/script_filter.sh" "p"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guard mismatch"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke steam-cli backend"

invalid_config_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-invalid-config" "$workflow_dir/scripts/script_filter.sh" "portal"; })"
assert_jq_json "$invalid_config_json" '.items[0].title == "Invalid Steam workflow config"' "invalid config title mismatch"
assert_jq_json "$invalid_config_json" '.items[0].subtitle | contains("STEAM_SEARCH_API")' "invalid config subtitle mismatch"

override_region_log="$tmp_dir/steam-override-region.log"
{
  STEAM_REGION="US" STEAM_STUB_LOG="$override_region_log" STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "helldivers" >/dev/null
}
override_first_line="$(sed -n '1p' "$override_region_log")"
[[ "$override_first_line" == *"region=us"* ]] || fail "script_filter must consume requery-selected region override"

empty_query_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "empty query guidance title mismatch"
[[ ! -f "$tmp_dir/cache/steam-region-override.state" ]] || fail "empty query should clear region override state"

default_region_log="$tmp_dir/steam-default-region.log"
{
  STEAM_REGION="US" STEAM_STUB_LOG="$default_region_log" STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "portal" >/dev/null
}
default_first_line="$(sed -n '1p' "$default_region_log")"
[[ "$default_first_line" == *"region=US"* ]] || fail "script_filter should use configured STEAM_REGION after override is cleared"

default_cache_log="$tmp_dir/steam-default-cache.log"
{
  STEAM_STUB_LOG="$default_cache_log" STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    env -u STEAM_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "portal" >/dev/null
  STEAM_STUB_LOG="$default_cache_log" STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    env -u STEAM_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "portal" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for steam-search"

opt_in_cache_log="$tmp_dir/steam-opt-in-cache.log"
{
  STEAM_STUB_LOG="$opt_in_cache_log" STEAM_QUERY_CACHE_TTL_SECONDS=10 STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "portal" >/dev/null
  STEAM_STUB_LOG="$opt_in_cache_log" STEAM_QUERY_CACHE_TTL_SECONDS=10 STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "portal" >/dev/null
}
opt_in_cache_hits="$(wc -l <"$opt_in_cache_log" | tr -d '[:space:]')"
[[ "$opt_in_cache_hits" == "1" ]] || fail "query cache should work when STEAM_QUERY_CACHE_TTL_SECONDS is explicitly set"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"${marker}","subtitle":"ok","arg":"https://example.com","valid":true}]}'
printf '\\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/steam-search/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"
  mkdir -p "$layout/workflows/steam-search/scripts/lib"
  cp "$repo_root/scripts/lib/"*.sh "$layout/workflows/steam-search/scripts/lib/"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/steam-search/bin/steam-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/steam-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/steam-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(STEAM_QUERY_COALESCE_SETTLE_SECONDS=0 STEAM_QUERY_CACHE_TTL_SECONDS=0 "$copied_script" "demo")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode steam-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-steam-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/steam-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/steam-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id steam-search >/dev/null

packaged_dir="$repo_root/build/workflows/steam-search/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/steam-cli"
assert_file "$packaged_dir/scripts/lib/script_filter_query_policy.sh"
assert_file "$packaged_dir/scripts/lib/workflow_action_requery.sh"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"
assert_jq_file "$packaged_json_file" '.objects | length > 0' "packaged plist missing objects"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.scriptfile == \"./scripts/script_filter.sh\"" "packaged script_filter scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.keyword == \"st||steam\"" "packaged keyword mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaycustom == 1" "packaged queue delay custom mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaymode == 0" "packaged queue delay mode mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelayimmediatelyinitially == false" "packaged immediate queue policy mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$action_uid\") | .config.scriptfile == \"./scripts/action_open.sh\"" "packaged action scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".connections[\"$script_filter_uid\"] | any(.destinationuid == \"$action_uid\" and .modifiers == 0)" "packaged connection graph mismatch"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["STEAM_LANGUAGE","STEAM_MAX_RESULTS","STEAM_REGION","STEAM_REGION_OPTIONS","STEAM_SEARCH_API","STEAM_SHOW_REGION_OPTIONS"]' "packaged user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_REGION") | .config.default == "US"' "packaged STEAM_REGION default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_REGION_OPTIONS") | .config.default == "US,JP"' "packaged STEAM_REGION_OPTIONS default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_SHOW_REGION_OPTIONS") | .config.default == "0"' "packaged STEAM_SHOW_REGION_OPTIONS default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_LANGUAGE") | .config.default == ""' "packaged STEAM_LANGUAGE default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_MAX_RESULTS") | .config.default == "10"' "packaged STEAM_MAX_RESULTS default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="STEAM_SEARCH_API") | .config.default == "search-suggestions"' "packaged STEAM_SEARCH_API default mismatch"

echo "ok: steam-search smoke test passed"

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
  scripts/script_filter_direct.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/script_filter_direct.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "google-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "brave-cli" ]] || fail "rust_binary must be brave-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in BRAVE_API_KEY BRAVE_MAX_RESULTS BRAVE_SAFESEARCH BRAVE_COUNTRY; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/alfred-cache"
export BRAVE_QUERY_CACHE_TTL_SECONDS=0
export BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

release_cli="$repo_root/target/release/brave-cli"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"
release_backup="$(artifact_backup_file "$release_cli" "$tmp_dir" "brave-cli.release")"

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

action_arg="https://www.google.com/search?q=rust"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/brave-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${BRAVE_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$BRAVE_STUB_LOG"
fi
if [[ "${1:-}" == "search" ]]; then
  [[ "${2:-}" == "--query" ]] || exit 9
  query="${3:-}"
  printf '{"items":[{"title":"stub-result","subtitle":"query=%s","arg":"https://example.com","valid":true}]}' "$query"
  printf '\n'
  exit 0
fi
if [[ "${1:-}" == "query" ]]; then
  [[ "${2:-}" == "--input" ]] || exit 9
  input="${3:-}"
  if [[ "$input" == res::* ]]; then
    query="${input#res::}"
    printf '{"items":[{"title":"stub-result","subtitle":"query=%s","arg":"https://example.com","valid":true}]}' "$query"
    printf '\n'
    exit 0
  fi
  printf '{"items":[{"title":"%s","subtitle":"Search \\"%s\\" | Press Tab to load search results","autocomplete":"res::%s","valid":false},{"title":"%s guide","subtitle":"Search \\"%s guide\\" | Press Tab to load search results","autocomplete":"res::%s guide","valid":false}]}' "$input" "$input" "$input" "$input" "$input" "$input"
  printf '\n'
  exit 0
fi
exit 9
EOS
chmod +x "$tmp_dir/stubs/brave-cli-ok"

cat >"$tmp_dir/stubs/brave-cli-quota" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "rate limit exceeded" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/brave-cli-quota"

cat >"$tmp_dir/stubs/brave-cli-missing-key" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "error: missing BRAVE_API_KEY" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/brave-cli-missing-key"

cat >"$tmp_dir/stubs/brave-cli-unavailable" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "transport error: connection reset by peer" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/brave-cli-unavailable"

cat >"$tmp_dir/stubs/brave-cli-invalid-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid BRAVE_SAFESEARCH: badvalue" >&2
exit 4
EOS
chmod +x "$tmp_dir/stubs/brave-cli-invalid-config"

cat >"$tmp_dir/stubs/brave-cli-suggest-down" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "google suggest request failed" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/brave-cli-suggest-down"

suggest_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "rust"; })"
assert_jq_json "$suggest_json" '.items | type == "array" and length >= 1' "gg suggest stage must output items array"
assert_jq_json "$suggest_json" '.items[0].title == "rust"' "gg suggest first item should keep raw query"
assert_jq_json "$suggest_json" '.items[0].autocomplete == "res::rust"' "gg suggest item must expose res:: autocomplete token"
assert_jq_json "$suggest_json" '.items[0].valid == false' "gg suggest rows must be non-actionable"

detail_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "res::rust book"; })"
assert_jq_json "$detail_json" '.items[0].title == "stub-result"' "gg detail stage should forward search results"
assert_jq_json "$detail_json" '.items[0].subtitle == "query=rust book"' "gg detail stage must strip res:: prefix before search"

env_query_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" alfred_workflow_query="rust async" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].autocomplete == "res::rust async"' "gg script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'rustlang' | BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].autocomplete == "res::rustlang"' "gg script_filter must support query via stdin fallback"

suggest_down_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-suggest-down" "$workflow_dir/scripts/script_filter.sh" "rust"; })"
assert_jq_json "$suggest_down_json" '.items[0].title == "Google suggestions unavailable"' "gg suggest-down title mapping mismatch"
assert_jq_json "$suggest_down_json" '.items[0].valid == false' "gg suggest-down fallback item must be invalid"

missing_key_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-missing-key" "$workflow_dir/scripts/script_filter.sh" "res::rust"; })"
assert_jq_json "$missing_key_json" '.items[0].title == "Brave API key is missing"' "gg stage2 missing key title mismatch"
assert_jq_json "$missing_key_json" '.items[0].subtitle | contains("BRAVE_API_KEY")' "gg stage2 missing key subtitle should guide configuration"

empty_query_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "gg empty query guidance title mismatch"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "gg empty query item must be invalid"

short_query_log="$tmp_dir/brave-short-query.log"
short_query_json="$({ BRAVE_STUB_LOG="$short_query_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "r"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "gg short query guidance title mismatch"
assert_jq_json "$short_query_json" '.items[0].subtitle | contains("2")' "gg short query guidance subtitle must mention minimum length"
[[ ! -s "$short_query_log" ]] || fail "gg short query should not invoke brave-cli backend"

default_cache_log="$tmp_dir/brave-default-cache.log"
{
  BRAVE_STUB_LOG="$default_cache_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    env -u BRAVE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "cache-hit-default" >/dev/null
  BRAVE_STUB_LOG="$default_cache_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    env -u BRAVE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "cache-hit-default" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for google-search suggest flow"

cache_probe_log="$tmp_dir/brave-cache-probe.log"
cache_probe_first="$({ BRAVE_STUB_LOG="$cache_probe_log" BRAVE_QUERY_CACHE_TTL_SECONDS=30 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "cache-hit"; })"
assert_jq_json "$cache_probe_first" '.items[0].autocomplete == "res::cache-hit"' "gg cache probe first response mismatch"

cache_probe_second="$({ BRAVE_STUB_LOG="$cache_probe_log" BRAVE_QUERY_CACHE_TTL_SECONDS=30 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "cache-hit"; })"
assert_jq_json "$cache_probe_second" '.items[0].autocomplete == "res::cache-hit"' "gg cache probe second response mismatch"
[[ "$(wc -l <"$cache_probe_log")" -eq 1 ]] || fail "gg same-query cache should avoid duplicate brave-cli invocation"

coalesce_probe_log="$tmp_dir/brave-coalesce-probe.log"
coalesce_pending_a="$({ BRAVE_STUB_LOG="$coalesce_probe_log" BRAVE_QUERY_CACHE_TTL_SECONDS=0 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=1 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "mayda"; })"
coalesce_pending_b="$({ BRAVE_STUB_LOG="$coalesce_probe_log" BRAVE_QUERY_CACHE_TTL_SECONDS=0 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=1 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "mayday"; })"
sleep 1.1
coalesce_result="$({ BRAVE_STUB_LOG="$coalesce_probe_log" BRAVE_QUERY_CACHE_TTL_SECONDS=0 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=1 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter.sh" "mayday"; })"

assert_jq_json "$coalesce_pending_a" '.items[0].title == "Fetching Google suggestions..."' "gg coalesce first pending title mismatch"
assert_jq_json "$coalesce_pending_a" '.items[0].valid == false' "gg coalesce first pending item must be invalid"
assert_jq_json "$coalesce_pending_b" '.items[0].title == "Fetching Google suggestions..."' "gg coalesce second pending title mismatch"
assert_jq_json "$coalesce_pending_b" '.items[0].valid == false' "gg coalesce second pending item must be invalid"
assert_jq_json "$coalesce_result" '.items[0].autocomplete == "res::mayday"' "gg coalesce final result query mismatch"
[[ "$(grep -c -- '--input mayda --mode' "$coalesce_probe_log" || true)" -eq 0 ]] || fail "gg coalesce should avoid mayda backend invocation"
[[ "$(grep -c -- '--input mayday' "$coalesce_probe_log" || true)" -eq 1 ]] || fail "gg coalesce should invoke mayday exactly once"

direct_success_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter_direct.sh" "rust"; })"
assert_jq_json "$direct_success_json" '.items[0].title == "stub-result"' "gb direct success should output results"
assert_jq_json "$direct_success_json" '.items[0].subtitle == "query=rust"' "gb direct success query mismatch"

direct_env_query_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" alfred_workflow_query="rust book" "$workflow_dir/scripts/script_filter_direct.sh"; })"
assert_jq_json "$direct_env_query_json" '.items[0].subtitle == "query=rust book"' "gb script_filter_direct must support Alfred query via env fallback"

quota_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-quota" "$workflow_dir/scripts/script_filter_direct.sh" "rust"; })"
assert_jq_json "$quota_json" '.items | type == "array" and length == 1' "gb quota fallback must output single item"
assert_jq_json "$quota_json" '.items[0].valid == false' "gb quota fallback item must be invalid"
assert_jq_json "$quota_json" '.items[0].title == "Brave API rate limited"' "gb rate-limit title mapping mismatch"
assert_jq_json "$quota_json" '.items[0].subtitle | contains("Too many requests")' "gb rate-limit subtitle mapping mismatch"

unavailable_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-unavailable" "$workflow_dir/scripts/script_filter_direct.sh" "rust"; })"
assert_jq_json "$unavailable_json" '.items[0].title == "Brave API unavailable"' "gb unavailable title mapping mismatch"

invalid_config_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-invalid-config" "$workflow_dir/scripts/script_filter_direct.sh" "rust"; })"
assert_jq_json "$invalid_config_json" '.items[0].title == "Invalid Brave workflow config"' "gb invalid config title mapping mismatch"

direct_empty_query_json="$({ BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter_direct.sh" "   "; })"
assert_jq_json "$direct_empty_query_json" '.items[0].title == "Enter a search query"' "gb empty query guidance title mismatch"
assert_jq_json "$direct_empty_query_json" '.items[0].valid == false' "gb empty query item must be invalid"

direct_short_query_log="$tmp_dir/brave-direct-short-query.log"
direct_short_query_json="$({ BRAVE_STUB_LOG="$direct_short_query_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" "$workflow_dir/scripts/script_filter_direct.sh" "r"; })"
assert_jq_json "$direct_short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "gb short query guidance title mismatch"
assert_jq_json "$direct_short_query_json" '.items[0].subtitle | contains("2")' "gb short query guidance subtitle must mention minimum length"
[[ ! -s "$direct_short_query_log" ]] || fail "gb short query should not invoke brave-cli backend"

direct_default_cache_log="$tmp_dir/brave-direct-default-cache.log"
{
  BRAVE_STUB_LOG="$direct_default_cache_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    env -u BRAVE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter_direct.sh" "cache-hit-direct" >/dev/null
  BRAVE_STUB_LOG="$direct_default_cache_log" BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    env -u BRAVE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter_direct.sh" "cache-hit-direct" >/dev/null
}
direct_default_cache_hits="$(wc -l <"$direct_default_cache_log" | tr -d '[:space:]')"
[[ "$direct_default_cache_hits" == "2" ]] || fail "default query cache must be disabled for google-search direct flow"

direct_opt_in_cache_log="$tmp_dir/brave-direct-opt-in-cache.log"
{
  BRAVE_STUB_LOG="$direct_opt_in_cache_log" BRAVE_QUERY_CACHE_TTL_SECONDS=30 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    "$workflow_dir/scripts/script_filter_direct.sh" "cache-hit-direct" >/dev/null
  BRAVE_STUB_LOG="$direct_opt_in_cache_log" BRAVE_QUERY_CACHE_TTL_SECONDS=30 BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_CLI_BIN="$tmp_dir/stubs/brave-cli-ok" \
    "$workflow_dir/scripts/script_filter_direct.sh" "cache-hit-direct" >/dev/null
}
direct_opt_in_cache_hits="$(wc -l <"$direct_opt_in_cache_log" | tr -d '[:space:]')"
[[ "$direct_opt_in_cache_hits" == "1" ]] || fail "direct flow query cache should work when BRAVE_QUERY_CACHE_TTL_SECONDS is explicitly set"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\${1:-}" == "query" ]]; then
  [[ "\${2:-}" == "--input" ]] || exit 9
  input="\${3:-}"
  if [[ "\$input" == res::* ]]; then
    q="\${input#res::}"
    printf '{"items":[{"title":"${marker}","subtitle":"query=%s","arg":"https://example.com","valid":true}]}' "\$q"
    printf '\\n'
    exit 0
  fi
  printf '{"items":[{"title":"${marker}","subtitle":"query=%s","autocomplete":"res::%s","valid":false}]}' "\$input" "\$input"
  printf '\\n'
  exit 0
fi
if [[ "\${1:-}" == "search" ]]; then
  [[ "\${2:-}" == "--query" ]] || exit 9
  q="\${3:-}"
  printf '{"items":[{"title":"${marker}","subtitle":"query=%s","arg":"https://example.com","valid":true}]}' "\$q"
  printf '\\n'
  exit 0
fi
exit 9
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/google-search/scripts/script_filter.sh"
  local copied_direct_script="$layout/workflows/google-search/scripts/script_filter_direct.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  cp "$workflow_dir/scripts/script_filter_direct.sh" "$copied_direct_script"
  chmod +x "$copied_script"
  chmod +x "$copied_direct_script"
  mkdir -p "$layout/workflows/google-search/scripts/lib"
  cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$layout/workflows/google-search/scripts/lib/script_filter_query_policy.sh"
  cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$layout/workflows/google-search/scripts/lib/script_filter_async_coalesce.sh"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/google-search/bin/brave-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/brave-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/brave-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_QUERY_CACHE_TTL_SECONDS=0 "$copied_script" "demo")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "gg script_filter failed to resolve $mode brave-cli path"
  assert_jq_json "$output" '.items[0].autocomplete == "res::demo"' "gg layout check must keep res:: autocomplete token"

  local direct_output
  direct_output="$(BRAVE_QUERY_COALESCE_SETTLE_SECONDS=0 BRAVE_QUERY_CACHE_TTL_SECONDS=0 "$copied_direct_script" "demo")"
  assert_jq_json "$direct_output" ".items[0].title == \"$marker\"" "gb script_filter_direct failed to resolve $mode brave-cli path"
  assert_jq_json "$direct_output" '.items[0].subtitle == "query=demo"' "gb layout check should keep direct query semantics"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-brave-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/brave-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/brave-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id google-search >/dev/null

packaged_dir="$repo_root/build/workflows/google-search/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/brave-cli"
assert_file "$packaged_dir/scripts/script_filter_direct.sh"
assert_file "$packaged_dir/scripts/lib/script_filter_query_policy.sh"
assert_file "$packaged_dir/scripts/lib/script_filter_async_coalesce.sh"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length > 0' "packaged plist missing objects"
assert_jq_file "$packaged_json_file" '.connections | length > 0' "packaged plist missing connections"
assert_jq_file "$packaged_json_file" '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.type] | all(. == 8)' "script filter objects must be external script type=8"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptfile == "./scripts/script_filter.sh"' "script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "gg||google"' "keyword trigger must be gg"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptargtype == 1' "script filter must pass query via argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.alfredfiltersresults == false' "script filter must disable Alfred local filtering"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaycustom == 1' "script filter queue delay custom must be 1 second"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaymode == 0' "script filter queue delay mode must be custom seconds"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelayimmediatelyinitially == false' "script filter must disable immediate initial run"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="C3D0A8F1-3F8A-4DAA-9D5D-2A6C4F52A9E8") | .config.scriptfile == "./scripts/script_filter_direct.sh"' "direct script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="C3D0A8F1-3F8A-4DAA-9D5D-2A6C4F52A9E8") | .config.keyword == "gb"' "direct keyword trigger must be gb"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="C3D0A8F1-3F8A-4DAA-9D5D-2A6C4F52A9E8") | .config.scriptargtype == 1' "direct script filter must pass query via argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="C3D0A8F1-3F8A-4DAA-9D5D-2A6C4F52A9E8") | .config.alfredfiltersresults == false' "direct script filter must disable Alfred local filtering"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.type == 8' "action node must be external script type=8"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["C3D0A8F1-3F8A-4DAA-9D5D-2A6C4F52A9E8"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing direct script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["BRAVE_API_KEY","BRAVE_COUNTRY","BRAVE_MAX_RESULTS","BRAVE_SAFESEARCH"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BRAVE_API_KEY") | .config.required == true' "BRAVE_API_KEY must be required"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BRAVE_MAX_RESULTS") | .config.default == "10"' "BRAVE_MAX_RESULTS default must be 10"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BRAVE_SAFESEARCH") | .config.default == "off"' "BRAVE_SAFESEARCH default must be off"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BRAVE_COUNTRY") | .config.required == false' "BRAVE_COUNTRY must be optional"

echo "ok: google-search smoke test"

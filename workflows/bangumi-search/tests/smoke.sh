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
  scripts/script_filter.sh \
  scripts/script_filter_book.sh \
  scripts/script_filter_anime.sh \
  scripts/script_filter_music.sh \
  scripts/script_filter_game.sh \
  scripts/script_filter_real.sh \
  scripts/action_clear_cache.sh \
  scripts/action_clear_cache_dir.sh \
  scripts/action_open.sh \
  scripts/bangumi_scraper.mjs \
  scripts/lib/bangumi_routes.mjs \
  scripts/lib/extract_search.mjs \
  scripts/lib/error_classify.mjs \
  scripts/tests/bangumi_scraper_contract.test.mjs \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/script_filter_book.sh \
  scripts/script_filter_anime.sh \
  scripts/script_filter_music.sh \
  scripts/script_filter_game.sh \
  scripts/script_filter_real.sh \
  scripts/action_clear_cache.sh \
  scripts/action_clear_cache_dir.sh \
  scripts/action_open.sh \
  scripts/bangumi_scraper.mjs \
  scripts/tests/bangumi_scraper_contract.test.mjs \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg
require_bin node

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "bangumi-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "bangumi-cli" ]] || fail "rust_binary must be bangumi-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in BANGUMI_API_KEY BANGUMI_MAX_RESULTS BANGUMI_TIMEOUT_MS BANGUMI_USER_AGENT BANGUMI_CACHE_DIR BANGUMI_IMAGE_CACHE_TTL_SECONDS BANGUMI_IMAGE_CACHE_MAX_MB BANGUMI_API_FALLBACK; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

if rg -n "bangumi_scraper" "$workflow_dir/scripts/script_filter.sh" >/dev/null; then
  fail "script_filter must not reference bangumi_scraper"
fi

node --check "$workflow_dir/scripts/bangumi_scraper.mjs" >/dev/null
node --test "$workflow_dir/scripts/tests/bangumi_scraper_contract.test.mjs" >/dev/null

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/alfred-cache"
export BANGUMI_QUERY_CACHE_TTL_SECONDS=0
export BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=0
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

artifact_backup=""
if [[ -f "$artifact_path" ]]; then
  artifact_backup="$tmp_dir/$(basename "$artifact_path").backup"
  cp "$artifact_path" "$artifact_backup"
fi

artifact_sha_backup=""
if [[ -f "$artifact_sha_path" ]]; then
  artifact_sha_backup="$tmp_dir/$(basename "$artifact_sha_path").backup"
  cp "$artifact_sha_path" "$artifact_sha_backup"
fi

release_cli="$repo_root/target/release/bangumi-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/bangumi-cli.release.backup"
  cp "$release_cli" "$release_backup"
fi

cleanup() {
  if [[ -n "$release_backup" && -f "$release_backup" ]]; then
    mkdir -p "$(dirname "$release_cli")"
    cp "$release_backup" "$release_cli"
  elif [[ -f "$release_cli" ]]; then
    rm -f "$release_cli"
  fi

  if [[ -n "$artifact_backup" && -f "$artifact_backup" ]]; then
    mkdir -p "$(dirname "$artifact_path")"
    cp "$artifact_backup" "$artifact_path"
  else
    rm -f "$artifact_path"
  fi

  if [[ -n "$artifact_sha_backup" && -f "$artifact_sha_backup" ]]; then
    mkdir -p "$(dirname "$artifact_sha_path")"
    cp "$artifact_sha_backup" "$artifact_sha_path"
  else
    rm -f "$artifact_sha_path"
  fi

  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"

cat >"$tmp_dir/bin/open" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$OPEN_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/open"

set +e
"$workflow_dir/scripts/action_open.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_open.sh without args must exit 2"

action_arg="https://bgm.tv/subject/2782"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cache_clear_arg="__BANGUMI_CLEAR_CACHE__"
cache_state_dir="$ALFRED_WORKFLOW_CACHE/script-filter-async-coalesce/bangumi-search"
mkdir -p "$cache_state_dir/cache"
printf '%s\n' "dummy" >"$cache_state_dir/request.latest"
printf '%s\n' "payload" >"$cache_state_dir/cache/dummy.payload"
printf '%s\t%s\n' "0" "ok" >"$cache_state_dir/cache/dummy.meta"

OPEN_STUB_OUT="$tmp_dir/open-clear-cache.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$cache_clear_arg"
[[ ! -d "$cache_state_dir" ]] || fail "clear-cache action must remove bangumi query cache state directory"
[[ ! -f "$tmp_dir/open-clear-cache.txt" ]] || fail "clear-cache action must not call open"

cache_clear_dir_arg="__BANGUMI_CLEAR_CACHE_DIR__"
cache_dir_target="$tmp_dir/bangumi-cache-dir"
mkdir -p "$cache_dir_target/sub"
printf '%s\n' "image-cache" >"$cache_dir_target/sub/sample.txt"

OPEN_STUB_OUT="$tmp_dir/open-clear-cache-dir.txt" BANGUMI_CACHE_DIR="$cache_dir_target" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$cache_clear_dir_arg"
[[ -d "$cache_dir_target" ]] || fail "clear-cache-dir action must keep cache directory path"
if find "$cache_dir_target" -mindepth 1 -print -quit | grep -q .; then
  fail "clear-cache-dir action must remove contents under BANGUMI_CACHE_DIR"
fi
[[ ! -f "$tmp_dir/open-clear-cache-dir.txt" ]] || fail "clear-cache-dir action must not call open"

tilde_home="$tmp_dir/home-cache-dir"
tilde_cache_dir="$tilde_home/.cache/bangumi-dir"
mkdir -p "$tilde_cache_dir/sub"
printf '%s\n' "image-cache" >"$tilde_cache_dir/sub/sample.txt"

OPEN_STUB_OUT="$tmp_dir/open-clear-cache-dir-tilde.txt" HOME="$tilde_home" BANGUMI_CACHE_DIR=\~/.cache/bangumi-dir PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$cache_clear_dir_arg"
[[ -d "$tilde_cache_dir" ]] || fail "clear-cache-dir action must resolve home-prefixed BANGUMI_CACHE_DIR"
if find "$tilde_cache_dir" -mindepth 1 -print -quit | grep -q .; then
  fail "clear-cache-dir action must remove contents under home-prefixed BANGUMI_CACHE_DIR"
fi
[[ ! -f "$tmp_dir/open-clear-cache-dir-tilde.txt" ]] || fail "clear-cache-dir action must not call open for home-prefixed cache dir"

cat >"$tmp_dir/stubs/bangumi-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${BANGUMI_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$BANGUMI_STUB_LOG"
fi
[[ "${1:-}" == "query" ]] || exit 9
[[ "${2:-}" == "--input" ]] || exit 9
query="${3:-}"
printf '{"items":[{"title":"stub-subject","subtitle":"query=%s","arg":"https://bgm.tv/subject/1","valid":true}]}' "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/bangumi-cli-ok"

cat >"$tmp_dir/stubs/bangumi-cli-rate-limit" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "rate limit exceeded: status 429" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/bangumi-cli-rate-limit"

cat >"$tmp_dir/stubs/bangumi-cli-missing-key" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "missing BANGUMI_API_KEY" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/bangumi-cli-missing-key"

cat >"$tmp_dir/stubs/bangumi-cli-unavailable" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "bangumi api request failed: status 503" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/bangumi-cli-unavailable"

cat >"$tmp_dir/stubs/bangumi-cli-invalid-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid BANGUMI_MAX_RESULTS: not-an-int" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/bangumi-cli-invalid-config"

success_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "anime naruto"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must output items array"
assert_jq_json "$success_json" '.items[0].title == "stub-subject"' "script_filter should forward successful JSON"

env_query_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" alfred_workflow_query="book 三体" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].subtitle == "query=book 三体"' "script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'music evangelion' | BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].subtitle == "query=music evangelion"' "script_filter must support query via stdin fallback"

rate_limit_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-rate-limit" "$workflow_dir/scripts/script_filter.sh" "anime naruto"; })"
assert_jq_json "$rate_limit_json" '.items[0].title == "Bangumi API rate-limited"' "rate-limit title mapping mismatch"

missing_key_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-missing-key" "$workflow_dir/scripts/script_filter.sh" "anime naruto"; })"
assert_jq_json "$missing_key_json" '.items[0].title == "Bangumi API key is missing"' "missing-key title mapping mismatch"

unavailable_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-unavailable" "$workflow_dir/scripts/script_filter.sh" "anime naruto"; })"
assert_jq_json "$unavailable_json" '.items[0].title == "Bangumi API unavailable"' "API unavailable title mapping mismatch"

invalid_config_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-invalid-config" "$workflow_dir/scripts/script_filter.sh" "anime naruto"; })"
assert_jq_json "$invalid_config_json" '.items[0].title == "Invalid Bangumi workflow config"' "invalid config title mapping mismatch"

empty_query_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "empty query guidance title mismatch"
assert_jq_json "$empty_query_json" 'all(.items[]; has("skipknowledge") | not)' "skipknowledge must not be placed on individual items"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "empty query item must be invalid"
assert_jq_json "$empty_query_json" '.items | any(.title == "Clear Bangumi query cache" and .arg == "__BANGUMI_CLEAR_CACHE__" and .valid == true)' "empty query must include clear-cache quick action"
assert_jq_json "$empty_query_json" '.items | any(.title == "Clear Bangumi cache dir" and .arg == "__BANGUMI_CLEAR_CACHE_DIR__" and .valid == true)' "empty query must include clear-cache-dir quick action"
assert_jq_json "$empty_query_json" '.items[3].title == "Bangumi Search (Anime)" and .items[3].autocomplete == "anime " and .items[3].valid == false' "empty query category item order mismatch: anime should be first category row"
assert_jq_json "$empty_query_json" '.items[4].title == "Bangumi Search (Game)" and .items[4].autocomplete == "game " and .items[4].valid == false' "empty query category item order mismatch: game should be second category row"
assert_jq_json "$empty_query_json" '.items[5].title == "Bangumi Search (Music)" and .items[5].autocomplete == "music " and .items[5].valid == false' "empty query category item order mismatch: music should be third category row"
assert_jq_json "$empty_query_json" '.items[6].title == "Bangumi Search (Book)" and .items[6].autocomplete == "book " and .items[6].valid == false' "empty query category item order mismatch: book should be fourth category row"
assert_jq_json "$empty_query_json" '.items[7].title == "Bangumi Search (Real)" and .items[7].autocomplete == "real " and .items[7].valid == false' "empty query category item order mismatch: real should be fifth category row"

short_query_log="$tmp_dir/bangumi-short-query.log"
short_query_json="$({ BANGUMI_STUB_LOG="$short_query_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "a"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke bangumi-cli backend"

clear_cache_log="$tmp_dir/bangumi-clear-cache.log"
clear_cache_json="$({ BANGUMI_STUB_LOG="$clear_cache_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "clear cache"; })"
assert_jq_json "$clear_cache_json" '.items[0].title == "Clear Bangumi query cache"' "clear-cache command title mismatch"
assert_jq_json "$clear_cache_json" '.items[0].arg == "__BANGUMI_CLEAR_CACHE__"' "clear-cache command arg wiring mismatch"
assert_jq_json "$clear_cache_json" '.items[0].valid == true' "clear-cache command item must be actionable"
[[ ! -s "$clear_cache_log" ]] || fail "clear-cache command should not invoke bangumi-cli backend"

clear_cache_dir_log="$tmp_dir/bangumi-clear-cache-dir.log"
clear_cache_dir_json="$({ BANGUMI_STUB_LOG="$clear_cache_dir_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "clear cache dir"; })"
assert_jq_json "$clear_cache_dir_json" '.items[0].title == "Clear Bangumi cache dir"' "clear-cache-dir command title mismatch"
assert_jq_json "$clear_cache_dir_json" '.items[0].arg == "__BANGUMI_CLEAR_CACHE_DIR__"' "clear-cache-dir command arg wiring mismatch"
assert_jq_json "$clear_cache_dir_json" '.items[0].valid == true' "clear-cache-dir command item must be actionable"
[[ ! -s "$clear_cache_dir_log" ]] || fail "clear-cache-dir command should not invoke bangumi-cli backend"

clear_cache_shortcut_log="$tmp_dir/bangumi-clear-cache-shortcut.log"
clear_cache_shortcut_json="$({ BANGUMI_STUB_LOG="$clear_cache_shortcut_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_book.sh" "clear cache"; })"
assert_jq_json "$clear_cache_shortcut_json" '.items[0].arg == "__BANGUMI_CLEAR_CACHE__"' "clear-cache command must bypass default type wrappers"
[[ ! -s "$clear_cache_shortcut_log" ]] || fail "clear-cache command via wrappers should not invoke bangumi-cli backend"

clear_cache_dir_shortcut_log="$tmp_dir/bangumi-clear-cache-dir-shortcut.log"
clear_cache_dir_shortcut_json="$({ BANGUMI_STUB_LOG="$clear_cache_dir_shortcut_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_book.sh" "clear cache dir"; })"
assert_jq_json "$clear_cache_dir_shortcut_json" '.items[0].arg == "__BANGUMI_CLEAR_CACHE_DIR__"' "clear-cache-dir command must bypass default type wrappers"
[[ ! -s "$clear_cache_dir_shortcut_log" ]] || fail "clear-cache-dir command via wrappers should not invoke bangumi-cli backend"

default_cache_log="$tmp_dir/bangumi-default-cache.log"
{
  BANGUMI_STUB_LOG="$default_cache_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" \
    env -u BANGUMI_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "anime naruto" >/dev/null
  BANGUMI_STUB_LOG="$default_cache_log" BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" \
    env -u BANGUMI_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "anime naruto" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for bangumi-search"

opt_in_cache_log="$tmp_dir/bangumi-opt-in-cache.log"
{
  BANGUMI_STUB_LOG="$opt_in_cache_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=10 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "anime naruto" >/dev/null
  BANGUMI_STUB_LOG="$opt_in_cache_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=10 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "anime naruto" >/dev/null
}
opt_in_cache_hits="$(wc -l <"$opt_in_cache_log" | tr -d '[:space:]')"
[[ "$opt_in_cache_hits" == "1" ]] || fail "query cache should work when BANGUMI_QUERY_CACHE_TTL_SECONDS is explicitly set"

default_settle_log="$tmp_dir/bangumi-default-settle.log"
default_settle_json="$({ BANGUMI_STUB_LOG="$default_settle_log" env -u BANGUMI_QUERY_COALESCE_SETTLE_SECONDS BANGUMI_QUERY_CACHE_TTL_SECONDS=0 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "anime pasted"; })"
assert_jq_json "$default_settle_json" '.items[0].subtitle == "query=anime pasted"' "default settle should fetch Bangumi results immediately"
[[ "$(grep -c -- '--input anime pasted --output alfred-json' "$default_settle_log" || true)" -eq 1 ]] || fail "default settle should invoke bangumi-cli immediately"

coalesce_queue_log="$tmp_dir/bangumi-coalesce-queue.log"
coalesce_queue_se="$({ BANGUMI_STUB_LOG="$coalesce_queue_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=0 BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=1 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "se"; })"
coalesce_queue_sev="$({ BANGUMI_STUB_LOG="$coalesce_queue_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=0 BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=1 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "sev"; })"
coalesce_queue_seven_pending="$({ BANGUMI_STUB_LOG="$coalesce_queue_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=0 BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=1 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "seven"; })"
sleep 1.1
coalesce_queue_seven_final="$({ BANGUMI_STUB_LOG="$coalesce_queue_log" BANGUMI_QUERY_CACHE_TTL_SECONDS=0 BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=1 BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter.sh" "seven"; })"
assert_jq_json "$coalesce_queue_se" '.items[0].title == "Searching Bangumi..." and .items[0].valid == false' "coalesce queue-mode probe (se) must return pending item"
assert_jq_json "$coalesce_queue_sev" '.items[0].title == "Searching Bangumi..." and .items[0].valid == false' "coalesce queue-mode probe (sev) must return pending item"
assert_jq_json "$coalesce_queue_seven_pending" '.items[0].title == "Searching Bangumi..." and .items[0].valid == false' "coalesce queue-mode probe (seven pending) must return pending item"
assert_jq_json "$coalesce_queue_seven_final" '.items[0].subtitle == "query=seven"' "coalesce queue-mode probe must resolve final query"
[[ "$(grep -c -- '--input se --output alfred-json' "$coalesce_queue_log" || true)" -eq 0 ]] || fail "coalesce queue-mode should not invoke backend for se"
[[ "$(grep -c -- '--input sev --output alfred-json' "$coalesce_queue_log" || true)" -eq 0 ]] || fail "coalesce queue-mode should not invoke backend for sev"
[[ "$(grep -c -- '--input seven --output alfred-json' "$coalesce_queue_log" || true)" -eq 1 ]] || fail "coalesce queue-mode should invoke backend for seven exactly once"

book_shortcut_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_book.sh" "三体"; })"
assert_jq_json "$book_shortcut_json" '.items[0].subtitle == "query=book 三体"' "book shortcut must inject default type"

anime_shortcut_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_anime.sh" "evangelion"; })"
assert_jq_json "$anime_shortcut_json" '.items[0].subtitle == "query=anime evangelion"' "anime shortcut must inject default type"

music_shortcut_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_music.sh" "utada"; })"
assert_jq_json "$music_shortcut_json" '.items[0].subtitle == "query=music utada"' "music shortcut must inject default type"

game_shortcut_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_game.sh" "zelda"; })"
assert_jq_json "$game_shortcut_json" '.items[0].subtitle == "query=game zelda"' "game shortcut must inject default type"

real_shortcut_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_real.sh" "interstellar"; })"
assert_jq_json "$real_shortcut_json" '.items[0].subtitle == "query=real interstellar"' "real shortcut must inject default type"

book_shortcut_override_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" "$workflow_dir/scripts/script_filter_book.sh" "anime naruto"; })"
assert_jq_json "$book_shortcut_override_json" '.items[0].subtitle == "query=anime naruto"' "explicit subject type should override shortcut default"

book_alias_keyword_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" alfred_workflow_keyword="bgmb" "$workflow_dir/scripts/script_filter.sh" "三体"; })"
assert_jq_json "$book_alias_keyword_json" '.items[0].subtitle == "query=book 三体"' "bgmb alias keyword should inject book default type via primary script filter"

anime_alias_keyword_json="$({ BANGUMI_CLI_BIN="$tmp_dir/stubs/bangumi-cli-ok" ALFRED_WORKFLOW_KEYWORD="BGMA" "$workflow_dir/scripts/script_filter.sh" "eva"; })"
assert_jq_json "$anime_alias_keyword_json" '.items[0].subtitle == "query=anime eva"' "bgma alias keyword should inject anime default type case-insensitively"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"${marker}","subtitle":"ok","arg":"https://bgm.tv/subject/1","valid":true}]}'
printf '\\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/bangumi-search/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"
  mkdir -p "$layout/workflows/bangumi-search/scripts/lib"
  cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$layout/workflows/bangumi-search/scripts/lib/script_filter_query_policy.sh"
  cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$layout/workflows/bangumi-search/scripts/lib/script_filter_async_coalesce.sh"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/bangumi-search/bin/bangumi-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/bangumi-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/bangumi-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(BANGUMI_QUERY_COALESCE_SETTLE_SECONDS=0 BANGUMI_QUERY_CACHE_TTL_SECONDS=0 "$copied_script" "anime naruto")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode bangumi-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-bangumi-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/bangumi-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/bangumi-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id bangumi-search >/dev/null

packaged_dir="$repo_root/build/workflows/bangumi-search/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/bangumi-cli"
assert_file "$packaged_dir/scripts/script_filter.sh"
assert_file "$packaged_dir/scripts/script_filter_book.sh"
assert_file "$packaged_dir/scripts/script_filter_anime.sh"
assert_file "$packaged_dir/scripts/script_filter_music.sh"
assert_file "$packaged_dir/scripts/script_filter_game.sh"
assert_file "$packaged_dir/scripts/script_filter_real.sh"
assert_file "$packaged_dir/scripts/action_clear_cache.sh"
assert_file "$packaged_dir/scripts/action_clear_cache_dir.sh"
assert_file "$packaged_dir/scripts/bangumi_scraper.mjs"
assert_file "$packaged_dir/scripts/lib/bangumi_routes.mjs"
assert_file "$packaged_dir/scripts/lib/extract_search.mjs"
assert_file "$packaged_dir/scripts/lib/error_classify.mjs"
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
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "bgm||bangumi||bgmb||bgma||bgmm||bgmg||bgmr"' "primary keyword trigger must include bgm + typed aliases"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="B48A5D8C-5F5E-4709-A8A2-1BDB89E4E201") | .config.scriptfile == "./scripts/script_filter_book.sh"' "book script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="B48A5D8C-5F5E-4709-A8A2-1BDB89E4E201") | .config.keyword == "__bgmb_disabled__"' "secondary bgmb entrypoint should be disabled to keep bgm command order deterministic"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="912ADAAE-E70D-4B1E-A138-E7F3D0D670F2") | .config.scriptfile == "./scripts/script_filter_anime.sh"' "anime script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="912ADAAE-E70D-4B1E-A138-E7F3D0D670F2") | .config.keyword == "__bgma_disabled__"' "secondary bgma entrypoint should be disabled to keep bgm command order deterministic"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="8C8FD5A4-4C37-4ED4-A8E8-3D613F2CE8B9") | .config.scriptfile == "./scripts/script_filter_music.sh"' "music script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="8C8FD5A4-4C37-4ED4-A8E8-3D613F2CE8B9") | .config.keyword == "__bgmm_disabled__"' "secondary bgmm entrypoint should be disabled to keep bgm command order deterministic"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D9C69B12-6594-4A0A-B0AA-4F6B3DEFD5A6") | .config.scriptfile == "./scripts/script_filter_game.sh"' "game script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D9C69B12-6594-4A0A-B0AA-4F6B3DEFD5A6") | .config.keyword == "__bgmg_disabled__"' "secondary bgmg entrypoint should be disabled to keep bgm command order deterministic"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="F3F8745F-C75C-4058-BCE5-7F95A77A9C3E") | .config.scriptfile == "./scripts/script_filter_real.sh"' "real script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="F3F8745F-C75C-4058-BCE5-7F95A77A9C3E") | .config.keyword == "__bgmr_disabled__"' "secondary bgmr entrypoint should be disabled to keep bgm command order deterministic"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaycustom == 1' "script filter queue delay custom must be 1 second"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelayimmediatelyinitially == false' "script filter must disable immediate initial run"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["B48A5D8C-5F5E-4709-A8A2-1BDB89E4E201"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing book script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["912ADAAE-E70D-4B1E-A138-E7F3D0D670F2"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing anime script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["8C8FD5A4-4C37-4ED4-A8E8-3D613F2CE8B9"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing music script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["D9C69B12-6594-4A0A-B0AA-4F6B3DEFD5A6"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing game script-filter to action connection"
assert_jq_file "$packaged_json_file" '.connections["F3F8745F-C75C-4058-BCE5-7F95A77A9C3E"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing real script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[].variable] | sort == ["BANGUMI_API_FALLBACK","BANGUMI_API_KEY","BANGUMI_CACHE_DIR","BANGUMI_IMAGE_CACHE_MAX_MB","BANGUMI_IMAGE_CACHE_TTL_SECONDS","BANGUMI_MAX_RESULTS","BANGUMI_TIMEOUT_MS","BANGUMI_USER_AGENT"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_MAX_RESULTS") | .config.default == "10"' "BANGUMI_MAX_RESULTS default must be 10"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_TIMEOUT_MS") | .config.default == "8000"' "BANGUMI_TIMEOUT_MS default must be 8000"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_API_FALLBACK") | .config.default == "auto"' "BANGUMI_API_FALLBACK default must be auto"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_API_KEY") | .config.default == ""' "BANGUMI_API_KEY default must be empty"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_CACHE_DIR") | .config.default == ""' "BANGUMI_CACHE_DIR default must be empty"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_IMAGE_CACHE_TTL_SECONDS") | .config.default == "86400"' "BANGUMI_IMAGE_CACHE_TTL_SECONDS default must be 86400"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="BANGUMI_IMAGE_CACHE_MAX_MB") | .config.default == "128"' "BANGUMI_IMAGE_CACHE_MAX_MB default must be 128"

echo "ok: bangumi-search smoke test"

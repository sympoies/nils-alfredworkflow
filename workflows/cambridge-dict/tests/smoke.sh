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
  scripts/script_filter.sh \
  scripts/action_open.sh \
  scripts/cambridge_scraper.mjs \
  scripts/lib/cambridge_routes.mjs \
  scripts/lib/cambridge_selectors.mjs \
  scripts/lib/cambridge_runtime_bootstrap.sh \
  scripts/lib/extract_suggest.mjs \
  scripts/lib/extract_define.mjs \
  scripts/lib/error_classify.mjs \
  scripts/tests/cambridge_scraper.test.mjs \
  scripts/tests/cambridge_routes.test.mjs \
  scripts/tests/cambridge_selectors.test.mjs \
  scripts/tests/extract_suggest.test.mjs \
  scripts/tests/extract_define.test.mjs \
  scripts/tests/error_classify.test.mjs \
  scripts/tests/fixtures/suggest-english-open.html \
  scripts/tests/fixtures/suggest-english-chinese-traditional-open.html \
  scripts/tests/fixtures/define-english-open.html \
  scripts/tests/fixtures/define-english-chinese-traditional-open.html \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/action_open.sh \
  scripts/cambridge_scraper.mjs \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "cambridge-dict" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "cambridge-cli" ]] || fail "rust_binary must be cambridge-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in CAMBRIDGE_DICT_MODE CAMBRIDGE_MAX_RESULTS CAMBRIDGE_TIMEOUT_MS CAMBRIDGE_HEADLESS; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/alfred-cache"
export CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=0
export CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS=0
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

release_cli="$repo_root/target/release/cambridge-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/cambridge-cli.release.backup"
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

action_arg="https://dictionary.cambridge.org/dictionary/english/open"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/requery" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$CAMBRIDGE_REQUERY_OUT"
EOS
chmod +x "$tmp_dir/stubs/requery"

CAMBRIDGE_REQUERY_OUT="$tmp_dir/requery-define.txt" CAMBRIDGE_REQUERY_COMMAND="$tmp_dir/stubs/requery" \
  "$workflow_dir/scripts/action_open.sh" "cambridge-requery:define:open up"
[[ "$(cat "$tmp_dir/requery-define.txt")" == "cd open up" ]] || fail "define requery text mismatch"

CAMBRIDGE_REQUERY_OUT="$tmp_dir/requery-suggest.txt" CAMBRIDGE_REQUERY_COMMAND="$tmp_dir/stubs/requery" \
  "$workflow_dir/scripts/action_open.sh" "cambridge-requery:suggest:open up"
[[ "$(cat "$tmp_dir/requery-suggest.txt")" == "cds open up" ]] || fail "suggest requery text mismatch"

cat >"$tmp_dir/stubs/cambridge-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${CAMBRIDGE_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$CAMBRIDGE_STUB_LOG"
fi
[[ "${1:-}" == "query" ]] || exit 9
[[ "${2:-}" == "--input" ]] || exit 9
query="${3:-}"
if [[ "$query" == def::* ]]; then
  printf '{"items":[{"title":"open - Cambridge","subtitle":"adjective | /əʊ.pən/ | Press Enter on a row to open Cambridge","arg":"https://dictionary.cambridge.org/dictionary/english/open","valid":true,"mods":{"cmd":{"subtitle":"Show Cambridge suggestions for open","arg":"cambridge-requery:suggest:open","valid":true}}},{"title":"not closed","subtitle":"Definition 1","arg":"https://dictionary.cambridge.org/dictionary/english/open","valid":true,"mods":{"cmd":{"subtitle":"Show Cambridge suggestions for open","arg":"cambridge-requery:suggest:open","valid":true}}}]}'
  printf '\n'
  exit 0
fi
if [[ "$query" == sug::* ]]; then
  printf '{"items":[{"title":"open","subtitle":"verb | Press Enter to load definition","arg":"cambridge-requery:define:open","valid":true}]}'
  printf '\n'
  exit 0
fi
printf '{"items":[{"title":"open - Cambridge","subtitle":"adjective | /əʊ.pən/ | Press Enter on a row to open Cambridge","arg":"https://dictionary.cambridge.org/dictionary/english/open","valid":true,"mods":{"cmd":{"subtitle":"Show Cambridge suggestions for open","arg":"cambridge-requery:suggest:open","valid":true}}},{"title":"not closed","subtitle":"Definition 1","arg":"https://dictionary.cambridge.org/dictionary/english/open","valid":true,"mods":{"cmd":{"subtitle":"Show Cambridge suggestions for open","arg":"cambridge-requery:suggest:open","valid":true}}}]}'
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-ok"

cat >"$tmp_dir/stubs/cambridge-cli-timeout" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "timeout while waiting for Cambridge page" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-timeout"

cat >"$tmp_dir/stubs/cambridge-cli-anti-bot" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "anti_bot challenge page returned by cloudflare" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-anti-bot"

cat >"$tmp_dir/stubs/cambridge-cli-cookie" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "cookie_wall: enable cookies to continue" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-cookie"

cat >"$tmp_dir/stubs/cambridge-cli-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid CAMBRIDGE_DICT_MODE: bad-mode" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-config"

cat >"$tmp_dir/stubs/cambridge-cli-runtime" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "playwright chromium executable doesn't exist" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-runtime"

cat >"$tmp_dir/stubs/cambridge-runtime-bootstrap" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

workflow_dir=""
state_file=""
log_file=""
result_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --workflow-dir)
    workflow_dir="${2:-}"
    shift 2
    ;;
  --state-file)
    state_file="${2:-}"
    shift 2
    ;;
  --log-file)
    log_file="${2:-}"
    shift 2
    ;;
  --result-file)
    result_file="${2:-}"
    shift 2
    ;;
  *)
    echo "unexpected arg: $1" >&2
    exit 2
    ;;
  esac
done

mkdir -p "$(dirname "$state_file")" "$(dirname "$log_file")" "$(dirname "$result_file")"
printf '%s\n' "$workflow_dir" >"$CAMBRIDGE_BOOTSTRAP_WORKFLOW_DIR_OUT"
printf '%s\n' "$$" >"$state_file"
trap 'printf "ok\t%s\n" "$(date +%s)" >"$result_file"; rm -f "$state_file"' EXIT
echo "ok: stub runtime ready" >"$log_file"
sleep "${CAMBRIDGE_BOOTSTRAP_STUB_SLEEP_SECONDS:-1}"
EOS
chmod +x "$tmp_dir/stubs/cambridge-runtime-bootstrap"

cat >"$tmp_dir/stubs/cambridge-runtime-bootstrap-fail" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

state_file=""
log_file=""
result_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --state-file)
    state_file="${2:-}"
    shift 2
    ;;
  --log-file)
    log_file="${2:-}"
    shift 2
    ;;
  --result-file)
    result_file="${2:-}"
    shift 2
    ;;
  *)
    shift 2
    ;;
  esac
done

mkdir -p "$(dirname "$state_file")" "$(dirname "$log_file")" "$(dirname "$result_file")"
printf '%s\n' "$$" >"$state_file"
trap 'printf "err\t%s\n" "$(date +%s)" >"$result_file"; rm -f "$state_file"' EXIT
echo "error: stub runtime setup failed" >"$log_file"
exit 1
EOS
chmod +x "$tmp_dir/stubs/cambridge-runtime-bootstrap-fail"

cat >"$tmp_dir/stubs/cambridge-cli-env" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"%s","valid":false}]}' "$CAMBRIDGE_SCRAPER_SCRIPT"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/cambridge-cli-env"

success_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$success_json" '.items | type == "array" and length >= 1' "script_filter smart mode must output detail rows"
assert_jq_json "$success_json" '.items[0].title == "open - Cambridge"' "smart exact match should return detail header"
assert_jq_json "$success_json" '.items[0].mods.cmd.arg == "cambridge-requery:suggest:open"' "detail rows must expose cmd requery modifier"

env_query_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" alfred_workflow_query="open sesame" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].title == "open - Cambridge"' "script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'open' | CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].title == "open - Cambridge"' "script_filter must support query via stdin fallback"

force_suggest_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" alfred_workflow_keyword="cds" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$force_suggest_json" '.items[0].arg == "cambridge-requery:define:open"' "cds keyword must force suggestion mode"

explicit_suggest_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "sug::open"; })"
assert_jq_json "$explicit_suggest_json" '.items[0].arg == "cambridge-requery:define:open"' "sug token must return suggestion rows"

# Detail-stage query should return rows that still support Enter open URL.
detail_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "def::open"; })"
assert_jq_json "$detail_json" '.items | type == "array" and length >= 1' "detail stage must output rows"
assert_jq_json "$detail_json" '.items[0].arg == "https://dictionary.cambridge.org/dictionary/english/open"' "detail row should keep URL arg"

env_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-env" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$env_json" ".items[0].title == \"$workflow_dir/scripts/cambridge_scraper.mjs\"" "script_filter must export CAMBRIDGE_SCRAPER_SCRIPT"

timeout_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-timeout" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$timeout_json" '.items[0].title == "Cambridge request timed out"' "timeout title mapping mismatch"
assert_jq_json "$timeout_json" '.items[0].valid == false' "timeout fallback item must be invalid"

anti_bot_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-anti-bot" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$anti_bot_json" '.items[0].title == "Cambridge anti-bot challenge"' "anti-bot title mapping mismatch"

cookie_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-cookie" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$cookie_json" '.items[0].title == "Cambridge cookie consent required"' "cookie-wall title mapping mismatch"

config_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-config" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$config_json" '.items[0].title == "Invalid Cambridge workflow config"' "invalid config title mapping mismatch"

runtime_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-runtime" CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER="$tmp_dir/stubs/missing-runtime-bootstrap" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$runtime_json" '.items[0].title == "Node/Playwright runtime unavailable"' "runtime unavailable title mapping mismatch"

runtime_bootstrap_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-runtime" CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER="$tmp_dir/stubs/cambridge-runtime-bootstrap" CAMBRIDGE_BOOTSTRAP_WORKFLOW_DIR_OUT="$tmp_dir/bootstrap-workflow-dir.txt" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$runtime_bootstrap_json" '.items[0].title == "Installing Cambridge runtime..."' "runtime bootstrap should emit pending item"
assert_jq_json "$runtime_bootstrap_json" '.items[0].valid == false' "runtime bootstrap pending item must be invalid"
for _ in $(seq 1 20); do
  [[ -f "$tmp_dir/bootstrap-workflow-dir.txt" ]] && break
  sleep 0.1
done
[[ "$(cat "$tmp_dir/bootstrap-workflow-dir.txt")" == "$workflow_dir" ]] || fail "runtime bootstrap must target workflow install directory"

runtime_bootstrap_fail_cache="$tmp_dir/alfred-cache-runtime-fail"
runtime_bootstrap_fail_state_dir="$runtime_bootstrap_fail_cache/cambridge-runtime"
mkdir -p "$runtime_bootstrap_fail_state_dir"
printf 'err\t%s\n' "$(date +%s)" >"$runtime_bootstrap_fail_state_dir/bootstrap.result"
runtime_bootstrap_fail_json="$({ ALFRED_WORKFLOW_CACHE="$runtime_bootstrap_fail_cache" CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-runtime" CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER="$tmp_dir/stubs/cambridge-runtime-bootstrap-fail" "$workflow_dir/scripts/script_filter.sh" "open"; })"
assert_jq_json "$runtime_bootstrap_fail_json" '.items[0].title == "Automatic Cambridge runtime setup failed"' "recent runtime bootstrap failure should surface dedicated item"

missing_layout="$tmp_dir/layout-missing"
missing_script="$missing_layout/workflows/cambridge-dict/scripts/script_filter.sh"
mkdir -p "$(dirname "$missing_script")"
cp "$workflow_dir/scripts/script_filter.sh" "$missing_script"
chmod +x "$missing_script"
mkdir -p "$missing_layout/workflows/cambridge-dict/scripts/lib"
cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$missing_layout/workflows/cambridge-dict/scripts/lib/script_filter_query_policy.sh"
cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$missing_layout/workflows/cambridge-dict/scripts/lib/script_filter_async_coalesce.sh"
missing_binary_json="$({ CAMBRIDGE_CLI_BIN="$missing_layout/does-not-exist/cambridge-cli" "$missing_script" "open"; })"
assert_jq_json "$missing_binary_json" '.items[0].title == "cambridge-cli binary not found"' "missing binary title mapping mismatch"

empty_query_json="$({ CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a word"' "empty query guidance title mismatch"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "empty query item must be invalid"

short_query_log="$tmp_dir/cambridge-short-query.log"
short_query_json="$({ CAMBRIDGE_STUB_LOG="$short_query_log" CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "o"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
assert_jq_json "$short_query_json" '.items[0].subtitle | contains("2")' "short query guidance subtitle must mention minimum length"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke cambridge-cli backend"

coalesce_probe_log="$tmp_dir/cambridge-coalesce.log"
coalesce_pending_a="$({ CAMBRIDGE_STUB_LOG="$coalesce_probe_log" env -u CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=0 CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "sym"; })"
coalesce_pending_b="$({ CAMBRIDGE_STUB_LOG="$coalesce_probe_log" env -u CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=0 CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "symphony"; })"
sleep 1.1
coalesce_result="$({ CAMBRIDGE_STUB_LOG="$coalesce_probe_log" env -u CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=0 CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" "$workflow_dir/scripts/script_filter.sh" "symphony"; })"

assert_jq_json "$coalesce_pending_a" '.items[0].title == "Searching Cambridge..." and .items[0].valid == false' "coalesce first pending item mismatch"
assert_jq_json "$coalesce_pending_b" '.items[0].title == "Searching Cambridge..." and .items[0].valid == false' "coalesce second pending item mismatch"
assert_jq_json "$coalesce_result" '.items[0].title == "open - Cambridge"' "coalesce final result mismatch"
[[ "$(grep -c -- '--input sym --mode alfred' "$coalesce_probe_log" || true)" -eq 0 ]] || fail "coalesce should avoid sym backend invocation"
[[ "$(grep -c -- '--input symphony --mode alfred' "$coalesce_probe_log" || true)" -eq 1 ]] || fail "coalesce should invoke symphony exactly once"

default_cache_log="$tmp_dir/cambridge-default-cache.log"
{
  CAMBRIDGE_STUB_LOG="$default_cache_log" CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" \
    env -u CAMBRIDGE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "open" >/dev/null
  CAMBRIDGE_STUB_LOG="$default_cache_log" CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" \
    env -u CAMBRIDGE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "open" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for cambridge-dict"

opt_in_cache_log="$tmp_dir/cambridge-opt-in-cache.log"
{
  CAMBRIDGE_STUB_LOG="$opt_in_cache_log" CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=10 CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "open" >/dev/null
  CAMBRIDGE_STUB_LOG="$opt_in_cache_log" CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=10 CAMBRIDGE_CLI_BIN="$tmp_dir/stubs/cambridge-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "open" >/dev/null
}
opt_in_cache_hits="$(wc -l <"$opt_in_cache_log" | tr -d '[:space:]')"
[[ "$opt_in_cache_hits" == "1" ]] || fail "query cache should work when CAMBRIDGE_QUERY_CACHE_TTL_SECONDS is explicitly set"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"${marker}","subtitle":"%s","arg":"https://dictionary.cambridge.org/dictionary/english/open","valid":true}]}' "\$CAMBRIDGE_SCRAPER_SCRIPT"
printf '\\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/cambridge-dict/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"
  mkdir -p "$layout/workflows/cambridge-dict/scripts/lib"
  cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$layout/workflows/cambridge-dict/scripts/lib/script_filter_query_policy.sh"
  cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$layout/workflows/cambridge-dict/scripts/lib/script_filter_async_coalesce.sh"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/cambridge-dict/bin/cambridge-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/cambridge-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/cambridge-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS=0 CAMBRIDGE_QUERY_CACHE_TTL_SECONDS=0 "$copied_script" "open")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode cambridge-cli path"
  assert_jq_json "$output" ".items[0].subtitle == \"$layout/workflows/cambridge-dict/scripts/cambridge_scraper.mjs\"" "script_filter must set CAMBRIDGE_SCRAPER_SCRIPT for $mode layout"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-cambridge-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/cambridge-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/cambridge-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id cambridge-dict >/dev/null

packaged_dir="$repo_root/build/workflows/cambridge-dict/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/cambridge-cli"
assert_file "$packaged_dir/scripts/cambridge_scraper.mjs"
assert_file "$packaged_dir/scripts/lib/cambridge_routes.mjs"
assert_file "$packaged_dir/scripts/lib/cambridge_selectors.mjs"
assert_file "$packaged_dir/scripts/lib/cambridge_runtime_bootstrap.sh"
assert_file "$packaged_dir/scripts/lib/extract_suggest.mjs"
assert_file "$packaged_dir/scripts/lib/extract_define.mjs"
assert_file "$packaged_dir/scripts/lib/error_classify.mjs"
assert_file "$packaged_dir/scripts/lib/script_filter_query_policy.sh"
assert_file "$packaged_dir/scripts/lib/script_filter_async_coalesce.sh"
assert_file "$packaged_dir/scripts/lib/workflow_action_requery.sh"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length > 0' "packaged plist missing objects"
assert_jq_file "$packaged_json_file" '.connections | length > 0' "packaged plist missing connections"
assert_jq_file "$packaged_json_file" '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.type] | all(. == 8)' "script filter objects must be external script type=8"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptfile == "./scripts/script_filter.sh"' "script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptargtype == 1' "script filter scriptargtype must be argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "cd||cambridge||cds"' "keyword trigger must include smart and suggest aliases"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.alfredfiltersresults == false' "script filter must disable Alfred-side filtering"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaycustom == 1' "script filter queue delay custom must be 1 second"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaymode == 0' "script filter queue delay mode must be custom seconds"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelayimmediatelyinitially == false' "script filter must disable immediate initial run"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.type == 8' "action node must be external script type=8"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["CAMBRIDGE_DICT_MODE","CAMBRIDGE_HEADLESS","CAMBRIDGE_MAX_RESULTS","CAMBRIDGE_TIMEOUT_MS"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="CAMBRIDGE_DICT_MODE") | .config.default == "english"' "CAMBRIDGE_DICT_MODE default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="CAMBRIDGE_MAX_RESULTS") | .config.default == "8"' "CAMBRIDGE_MAX_RESULTS default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="CAMBRIDGE_TIMEOUT_MS") | .config.default == "8000"' "CAMBRIDGE_TIMEOUT_MS default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="CAMBRIDGE_HEADLESS") | .config.default == "true"' "CAMBRIDGE_HEADLESS default mismatch"

echo "ok: cambridge-dict smoke test"

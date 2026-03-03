#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workflow_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$workflow_dir/../.." && pwd)"

fail() {
  echo "error: $*" >&2
  exit 1
}

require_bin() {
  local binary="$1"
  command -v "$binary" >/dev/null 2>&1 || fail "missing required binary: $binary"
}

require_bin shellcheck
mapfile -t shellcheck_targets < <(find "$workflow_dir" -type f -name '*.sh' | sort)
if [[ "${#shellcheck_targets[@]}" -gt 0 ]]; then
  shellcheck -e SC1091 "${shellcheck_targets[@]}"
fi

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || fail "missing required file: $path"
}

assert_exec() {
  local path="$1"
  [[ -x "$path" ]] || fail "script must be executable: $path"
}

toml_string() {
  local file="$1"
  local key="$2"
  awk -F'=' -v key="$key" '
    $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      value=$2
      sub(/^[[:space:]]*/, "", value)
      sub(/[[:space:]]*$/, "", value)
      gsub(/^"|"$/, "", value)
      print value
      exit
    }
  ' "$file"
}

assert_jq_json() {
  local json_payload="$1"
  local filter="$2"
  local message="$3"
  if ! jq -e "$filter" >/dev/null <<<"$json_payload"; then
    fail "$message (jq: $filter)"
  fi
}

assert_jq_file() {
  local file="$1"
  local filter="$2"
  local message="$3"
  if ! jq -e "$filter" "$file" >/dev/null; then
    fail "$message (jq: $filter)"
  fi
}

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

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "bilibili-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "bilibili-cli" ]] || fail "rust_binary must be bilibili-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in BILIBILI_UID BILIBILI_MAX_RESULTS BILIBILI_TIMEOUT_MS BILIBILI_USER_AGENT; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/alfred-cache"
export BILIBILI_QUERY_CACHE_TTL_SECONDS=0
export BILIBILI_QUERY_COALESCE_SETTLE_SECONDS=0
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

release_cli="$repo_root/target/release/bilibili-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/bilibili-cli.release.backup"
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

action_arg="https://search.bilibili.com/all?keyword=naruto"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/bilibili-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${BILIBILI_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$BILIBILI_STUB_LOG"
fi
[[ "${1:-}" == "query" ]] || exit 9
[[ "${2:-}" == "--input" ]] || exit 9
query="${3:-}"
printf '{"items":[{"title":"stub-result","subtitle":"query=%s","arg":"https://search.bilibili.com/all?keyword=%s","valid":true}]}' "$query" "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/bilibili-cli-ok"

cat >"$tmp_dir/stubs/bilibili-cli-no-results" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"No suggestions found","subtitle":"Press Enter to search bilibili directly.","valid":false}]}'
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/bilibili-cli-no-results"

cat >"$tmp_dir/stubs/bilibili-cli-unavailable" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "bilibili api request failed" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/bilibili-cli-unavailable"

cat >"$tmp_dir/stubs/bilibili-cli-invalid-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid BILIBILI_MAX_RESULTS: abc" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/bilibili-cli-invalid-config"

success_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" "$workflow_dir/scripts/script_filter.sh" "naruto"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must output items array"
assert_jq_json "$success_json" '.items[0].title == "stub-result"' "script_filter should forward successful JSON"

env_query_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" alfred_workflow_query="naruto mobile" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].subtitle == "query=naruto mobile"' "script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'naruto stage' | BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].subtitle == "query=naruto stage"' "script_filter must support query via stdin fallback"

no_results_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-no-results" "$workflow_dir/scripts/script_filter.sh" "naruto"; })"
assert_jq_json "$no_results_json" '.items[0].title == "No suggestions found"' "script_filter should forward no-results item"

unavailable_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-unavailable" "$workflow_dir/scripts/script_filter.sh" "naruto"; })"
assert_jq_json "$unavailable_json" '.items[0].title == "Bilibili API unavailable"' "unavailable title mapping mismatch"
assert_jq_json "$unavailable_json" '.items[0].valid == false' "unavailable item must be invalid"

invalid_config_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-invalid-config" "$workflow_dir/scripts/script_filter.sh" "naruto"; })"
assert_jq_json "$invalid_config_json" '.items[0].title == "Invalid Bilibili workflow config"' "invalid config title mapping mismatch"
assert_jq_json "$invalid_config_json" '.items[0].subtitle | contains("BILIBILI_MAX_RESULTS")' "invalid config subtitle should mention BILIBILI_MAX_RESULTS"

empty_query_json="$({ BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "empty query guidance title mismatch"

short_query_log="$tmp_dir/bilibili-short-query.log"
short_query_json="$({ BILIBILI_STUB_LOG="$short_query_log" BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" "$workflow_dir/scripts/script_filter.sh" "n"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke bilibili-cli backend"

default_cache_log="$tmp_dir/bilibili-default-cache.log"
{
  BILIBILI_STUB_LOG="$default_cache_log" BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" \
    env -u BILIBILI_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "naruto" >/dev/null
  BILIBILI_STUB_LOG="$default_cache_log" BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" \
    env -u BILIBILI_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "naruto" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for bilibili-search"

opt_in_cache_log="$tmp_dir/bilibili-opt-in-cache.log"
{
  BILIBILI_STUB_LOG="$opt_in_cache_log" BILIBILI_QUERY_CACHE_TTL_SECONDS=10 BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "naruto" >/dev/null
  BILIBILI_STUB_LOG="$opt_in_cache_log" BILIBILI_QUERY_CACHE_TTL_SECONDS=10 BILIBILI_CLI_BIN="$tmp_dir/stubs/bilibili-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "naruto" >/dev/null
}
opt_in_cache_hits="$(wc -l <"$opt_in_cache_log" | tr -d '[:space:]')"
[[ "$opt_in_cache_hits" == "1" ]] || fail "query cache should work when BILIBILI_QUERY_CACHE_TTL_SECONDS is set"

check_layout_resolution() {
  local mode="$1"
  local marker="$2"

  local layout="$tmp_dir/layout-$mode"
  mkdir -p "$layout/workflows/bilibili-search/scripts"
  cp "$workflow_dir/scripts/script_filter.sh" "$layout/workflows/bilibili-search/scripts/script_filter.sh"
  chmod +x "$layout/workflows/bilibili-search/scripts/script_filter.sh"

  mkdir -p "$layout/workflows/bilibili-search/scripts/lib"
  cp "$repo_root/scripts/lib/script_filter_error_json.sh" "$layout/workflows/bilibili-search/scripts/lib/"
  cp "$repo_root/scripts/lib/workflow_cli_resolver.sh" "$layout/workflows/bilibili-search/scripts/lib/"
  cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$layout/workflows/bilibili-search/scripts/lib/"
  cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$layout/workflows/bilibili-search/scripts/lib/"
  cp "$repo_root/scripts/lib/script_filter_search_driver.sh" "$layout/workflows/bilibili-search/scripts/lib/"

  make_layout_cli() {
    local target="$1"
    local title="$2"
    mkdir -p "$(dirname "$target")"
    cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"$title","subtitle":"resolved","valid":true}]}'
printf '\n'
EOS
    chmod +x "$target"
  }

  case "$mode" in
  packaged) make_layout_cli "$layout/workflows/bilibili-search/bin/bilibili-cli" "$marker" ;;
  release) make_layout_cli "$layout/target/release/bilibili-cli" "$marker" ;;
  debug) make_layout_cli "$layout/target/debug/bilibili-cli" "$marker" ;;
  *) fail "unknown layout mode: $mode" ;;
  esac

  local output
  output="$(BILIBILI_QUERY_COALESCE_SETTLE_SECONDS=0 BILIBILI_QUERY_CACHE_TTL_SECONDS=0 "$layout/workflows/bilibili-search/scripts/script_filter.sh" "demo")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode bilibili-cli path"
}

check_layout_resolution packaged "resolved-packaged"
check_layout_resolution release "resolved-release"
check_layout_resolution debug "resolved-debug"

mkdir -p "$tmp_dir/bin"
cat >"$tmp_dir/bin/cargo" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
repo_root="${MOCK_REPO_ROOT:?MOCK_REPO_ROOT required}"
if [[ "$#" -eq 4 && "$1" == "build" && "$2" == "--release" && "$3" == "-p" && "$4" == "nils-bilibili-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/bilibili-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"packaged","subtitle":"ok","arg":"https://search.bilibili.com/all?keyword=ok","valid":true}]}'
printf '\n'
EOCLI
  chmod +x "$repo_root/target/release/bilibili-cli"
  exit 0
fi

real_cargo="${REAL_CARGO_BIN:-}"
if [[ -z "$real_cargo" ]]; then
  echo "unexpected cargo invocation: $*" >&2
  exit 1
fi

exec "$real_cargo" "$@"
EOS
chmod +x "$tmp_dir/bin/cargo"

REAL_CARGO_BIN="$(command -v cargo)"
MOCK_REPO_ROOT="$repo_root" REAL_CARGO_BIN="$REAL_CARGO_BIN" PATH="$tmp_dir/bin:$PATH" \
  "$repo_root/scripts/workflow-pack.sh" --id bilibili-search >/dev/null

packaged_dir="$repo_root/build/workflows/bilibili-search/pkg"
assert_file "$packaged_dir/info.plist"
assert_file "$packaged_dir/scripts/script_filter.sh"
assert_file "$packaged_dir/scripts/action_open.sh"
assert_file "$packaged_dir/bin/bilibili-cli"
assert_file "$packaged_dir/icon.png"

packaged_json_file="$tmp_dir/packaged-info.json"
if command -v plutil >/dev/null 2>&1; then
  plutil -convert json -o "$packaged_json_file" "$packaged_dir/info.plist"
else
  python3 - "$packaged_dir/info.plist" "$packaged_json_file" <<'PY'
import json
import plistlib
import sys
with open(sys.argv[1], 'rb') as src:
    payload = plistlib.load(src)
with open(sys.argv[2], 'w', encoding='utf-8') as dst:
    json.dump(payload, dst)
PY
fi

assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "bl||bilibili"' "keyword trigger must be bl"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["BILIBILI_MAX_RESULTS","BILIBILI_TIMEOUT_MS","BILIBILI_UID","BILIBILI_USER_AGENT"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptfile == "./scripts/script_filter.sh"' "script_filter path mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action script path mismatch"

echo "ok: bilibili-search smoke test"

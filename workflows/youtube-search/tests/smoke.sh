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

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "youtube-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "youtube-cli" ]] || fail "rust_binary must be youtube-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in YOUTUBE_API_KEY YOUTUBE_MAX_RESULTS YOUTUBE_REGION_CODE; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/alfred-cache"
export YOUTUBE_QUERY_CACHE_TTL_SECONDS=0
export YOUTUBE_QUERY_COALESCE_SETTLE_SECONDS=0
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

release_cli="$repo_root/target/release/youtube-cli"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"
release_backup="$(artifact_backup_file "$release_cli" "$tmp_dir" "youtube-cli.release")"

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

action_arg="https://www.youtube.com/watch?v=dQw4w9WgXcQ"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/youtube-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${YOUTUBE_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$YOUTUBE_STUB_LOG"
fi
[[ "${1:-}" == "search" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
printf '{"items":[{"title":"stub-result","subtitle":"query=%s","arg":"https://www.youtube.com/watch?v=abc123","valid":true}]}' "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/youtube-cli-ok"

cat >"$tmp_dir/stubs/youtube-cli-fail" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "quota exceeded" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/youtube-cli-fail"

cat >"$tmp_dir/stubs/youtube-cli-missing-key" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "error: missing YOUTUBE_API_KEY" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/youtube-cli-missing-key"

success_json="$({ YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" "$workflow_dir/scripts/script_filter.sh" "lofi"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must output items array"
assert_jq_json "$success_json" '.items[0].title == "stub-result"' "script_filter should forward successful JSON"

env_query_json="$({ YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" alfred_workflow_query="city pop" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].subtitle == "query=city pop"' "script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'focus music' | YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].subtitle == "query=focus music"' "script_filter must support query via stdin fallback"

failure_json="$({ YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-fail" "$workflow_dir/scripts/script_filter.sh" "lofi"; })"
assert_jq_json "$failure_json" '.items | type == "array" and length == 1' "script_filter failure fallback must output single item"
assert_jq_json "$failure_json" '.items[0].valid == false' "script_filter failure fallback item must be invalid"
assert_jq_json "$failure_json" '.items[0].title == "YouTube quota exceeded"' "script_filter should map quota errors to actionable title"

missing_key_json="$({ YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-missing-key" "$workflow_dir/scripts/script_filter.sh" "lofi"; })"
assert_jq_json "$missing_key_json" '.items[0].title == "YouTube API key is missing"' "script_filter should map missing key errors"
assert_jq_json "$missing_key_json" '.items[0].subtitle | contains("YOUTUBE_API_KEY")' "missing key subtitle should guide configuration"

empty_query_json="$({ YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "empty query guidance title mismatch"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "empty query item must be invalid"

short_query_log="$tmp_dir/youtube-short-query.log"
short_query_json="$({ YOUTUBE_STUB_LOG="$short_query_log" YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" "$workflow_dir/scripts/script_filter.sh" "l"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
assert_jq_json "$short_query_json" '.items[0].subtitle | contains("2")' "short query guidance subtitle must mention minimum length"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke youtube-cli backend"

default_cache_log="$tmp_dir/youtube-default-cache.log"
{
  YOUTUBE_STUB_LOG="$default_cache_log" YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" \
    env -u YOUTUBE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "lofi" >/dev/null
  YOUTUBE_STUB_LOG="$default_cache_log" YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" \
    env -u YOUTUBE_QUERY_CACHE_TTL_SECONDS "$workflow_dir/scripts/script_filter.sh" "lofi" >/dev/null
}
default_cache_hits="$(wc -l <"$default_cache_log" | tr -d '[:space:]')"
[[ "$default_cache_hits" == "2" ]] || fail "default query cache must be disabled for youtube-search"

opt_in_cache_log="$tmp_dir/youtube-opt-in-cache.log"
{
  YOUTUBE_STUB_LOG="$opt_in_cache_log" YOUTUBE_QUERY_CACHE_TTL_SECONDS=10 YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "lofi" >/dev/null
  YOUTUBE_STUB_LOG="$opt_in_cache_log" YOUTUBE_QUERY_CACHE_TTL_SECONDS=10 YOUTUBE_CLI_BIN="$tmp_dir/stubs/youtube-cli-ok" \
    "$workflow_dir/scripts/script_filter.sh" "lofi" >/dev/null
}
opt_in_cache_hits="$(wc -l <"$opt_in_cache_log" | tr -d '[:space:]')"
[[ "$opt_in_cache_hits" == "1" ]] || fail "query cache should work when YOUTUBE_QUERY_CACHE_TTL_SECONDS is explicitly set"

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
  local copied_script="$layout/workflows/youtube-search/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"
  mkdir -p "$layout/workflows/youtube-search/scripts/lib"
  cp "$repo_root/scripts/lib/script_filter_query_policy.sh" "$layout/workflows/youtube-search/scripts/lib/script_filter_query_policy.sh"
  cp "$repo_root/scripts/lib/script_filter_async_coalesce.sh" "$layout/workflows/youtube-search/scripts/lib/script_filter_async_coalesce.sh"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/youtube-search/bin/youtube-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/youtube-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/youtube-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(YOUTUBE_QUERY_COALESCE_SETTLE_SECONDS=0 YOUTUBE_QUERY_CACHE_TTL_SECONDS=0 "$copied_script" "demo")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode youtube-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-youtube-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/youtube-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/youtube-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id youtube-search >/dev/null

packaged_dir="$repo_root/build/workflows/youtube-search/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/youtube-cli"
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
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "yt||youtube"' "keyword trigger must be yt"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptargtype == 1' "script filter must pass query via argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.alfredfiltersresults == false' "script filter must disable Alfred local filtering"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaycustom == 1' "script filter queue delay custom must be 1 second"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelaymode == 0' "script filter queue delay mode must be custom seconds"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.queuedelayimmediatelyinitially == false' "script filter must disable immediate initial run"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.type == 8' "action node must be external script type=8"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["YOUTUBE_API_KEY","YOUTUBE_MAX_RESULTS","YOUTUBE_REGION_CODE"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="YOUTUBE_API_KEY") | .config.required == true' "YOUTUBE_API_KEY must be required"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="YOUTUBE_MAX_RESULTS") | .config.default == "10"' "YOUTUBE_MAX_RESULTS default must be 10"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="YOUTUBE_REGION_CODE") | .config.required == false' "YOUTUBE_REGION_CODE must be optional"

echo "ok: youtube-search smoke test"

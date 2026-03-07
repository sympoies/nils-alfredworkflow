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
[[ "$(toml_string "$manifest" id)" == "spotify-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "spotify-cli" ]] || fail "rust_binary must be spotify-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in SPOTIFY_CLIENT_ID SPOTIFY_CLIENT_SECRET SPOTIFY_MAX_RESULTS SPOTIFY_MARKET; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

if ! rg -n '^SPOTIFY_CLIENT_ID[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "SPOTIFY_CLIENT_ID default must be empty"
fi
if ! rg -n '^SPOTIFY_CLIENT_SECRET[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "SPOTIFY_CLIENT_SECRET default must be empty"
fi
if ! rg -n '^SPOTIFY_MAX_RESULTS[[:space:]]*=[[:space:]]*"10"' "$manifest" >/dev/null; then
  fail "SPOTIFY_MAX_RESULTS default must be 10"
fi

plist_json="$(plist_to_json "$workflow_dir/src/info.plist.template")"
script_filter_uid="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"

assert_jq_json "$plist_json" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.scriptfile == \"./scripts/script_filter.sh\"" "script_filter scriptfile wiring mismatch"
assert_jq_json "$plist_json" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.keyword == \"sp||spotify\"" "keyword trigger must be sp"
assert_jq_json "$plist_json" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaycustom == 1" "queue delay custom mismatch"
assert_jq_json "$plist_json" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaymode == 0" "queue delay mode mismatch"
assert_jq_json "$plist_json" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelayimmediatelyinitially == false" "queue immediate policy mismatch"

tmp_dir="$(mktemp -d)"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/cache"
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

release_cli="$repo_root/target/release/spotify-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/spotify-cli.release.backup"
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
printf '%s\n' "$@" >"$OPEN_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/open"

set +e
"$workflow_dir/scripts/action_open.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_open.sh without args must exit 2"

action_arg="https://open.spotify.com/track/1mCsF9Tw4AkIZOjvZbZZdT"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
cat >"$tmp_dir/expected-open-args.txt" <<'EOS'
-a
Spotify
spotify:track:1mCsF9Tw4AkIZOjvZbZZdT
EOS
if ! diff -u "$tmp_dir/expected-open-args.txt" "$tmp_dir/open-arg.txt" >/dev/null; then
  fail "action_open.sh must prefer Spotify app with spotify: URI for open.spotify.com links"
fi

external_arg="https://example.com/page"
OPEN_STUB_OUT="$tmp_dir/open-external-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$external_arg"
[[ "$(cat "$tmp_dir/open-external-arg.txt")" == "$external_arg" ]] || fail "action_open.sh must preserve non-Spotify URLs"

cat >"$tmp_dir/stubs/spotify-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "search" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
if [[ -n "${SPOTIFY_STUB_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"$SPOTIFY_STUB_LOG"
fi
printf '{"items":[{"title":"stub-result","subtitle":"query=%s","arg":"https://open.spotify.com/track/stub","valid":true}]}' "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/spotify-cli-ok"

cat >"$tmp_dir/stubs/spotify-cli-rate-limit" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "HTTP 429 too many requests" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/spotify-cli-rate-limit"

cat >"$tmp_dir/stubs/spotify-cli-missing-credentials" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "error: missing SPOTIFY_CLIENT_SECRET" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/spotify-cli-missing-credentials"

cat >"$tmp_dir/stubs/spotify-cli-unavailable" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "transport error: connection reset by peer" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/spotify-cli-unavailable"

cat >"$tmp_dir/stubs/spotify-cli-invalid-config" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid SPOTIFY_MARKET: TWW" >&2
exit 4
EOS
chmod +x "$tmp_dir/stubs/spotify-cli-invalid-config"

success_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "ambient"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must output items array"
assert_jq_json "$success_json" '.items[0].title == "stub-result"' "script_filter should forward successful JSON"

env_query_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" alfred_workflow_query="city pop" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[0].subtitle == "query=city pop"' "script_filter must support Alfred query via env fallback"

stdin_query_json="$(printf 'focus music' | SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[0].subtitle == "query=focus music"' "script_filter must support query via stdin fallback"

rate_limit_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-rate-limit" "$workflow_dir/scripts/script_filter.sh" "ambient"; })"
assert_jq_json "$rate_limit_json" '.items | type == "array" and length == 1' "rate limit fallback must output single item"
assert_jq_json "$rate_limit_json" '.items[0].valid == false' "rate limit fallback item must be invalid"
assert_jq_json "$rate_limit_json" '.items[0].title == "Spotify API rate limited"' "rate limit error title mapping mismatch"

missing_credentials_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-missing-credentials" "$workflow_dir/scripts/script_filter.sh" "ambient"; })"
assert_jq_json "$missing_credentials_json" '.items[0].title == "Spotify credentials are missing"' "missing credentials title mapping mismatch"
assert_jq_json "$missing_credentials_json" '.items[0].subtitle | contains("SPOTIFY_CLIENT_ID") and contains("SPOTIFY_CLIENT_SECRET")' "missing credentials subtitle should guide configuration"

unavailable_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-unavailable" "$workflow_dir/scripts/script_filter.sh" "ambient"; })"
assert_jq_json "$unavailable_json" '.items[0].title == "Spotify API unavailable"' "unavailable title mapping mismatch"

invalid_config_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-invalid-config" "$workflow_dir/scripts/script_filter.sh" "ambient"; })"
assert_jq_json "$invalid_config_json" '.items[0].title == "Invalid Spotify workflow config"' "invalid config title mapping mismatch"

empty_query_json="$({ SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a search query"' "empty query guidance title mismatch"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "empty query item must be invalid"

short_query_log="$tmp_dir/spotify-short-query.log"
short_query_json="$({ SPOTIFY_STUB_LOG="$short_query_log" SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "s"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
assert_jq_json "$short_query_json" '.items[0].subtitle | contains("2")' "short query guidance subtitle must mention minimum length"
[[ ! -s "$short_query_log" ]] || fail "short query should not invoke spotify-cli backend"

coalesce_probe_log="$tmp_dir/spotify-coalesce.log"
coalesce_pending_a="$({ SPOTIFY_STUB_LOG="$coalesce_probe_log" env -u SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS SPOTIFY_QUERY_CACHE_TTL_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "sym"; })"
coalesce_pending_b="$({ SPOTIFY_STUB_LOG="$coalesce_probe_log" env -u SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS SPOTIFY_QUERY_CACHE_TTL_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "symphony"; })"
sleep 1.1
coalesce_result="$({ SPOTIFY_STUB_LOG="$coalesce_probe_log" env -u SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS SPOTIFY_QUERY_CACHE_TTL_SECONDS=0 SPOTIFY_CLI_BIN="$tmp_dir/stubs/spotify-cli-ok" "$workflow_dir/scripts/script_filter.sh" "symphony"; })"

assert_jq_json "$coalesce_pending_a" '.items[0].title == "Searching Spotify..." and .items[0].valid == false' "coalesce first pending item mismatch"
assert_jq_json "$coalesce_pending_b" '.items[0].title == "Searching Spotify..." and .items[0].valid == false' "coalesce second pending item mismatch"
assert_jq_json "$coalesce_result" '.items[0].subtitle == "query=symphony"' "coalesce final query mismatch"
[[ "$(grep -c -- '--query sym --mode' "$coalesce_probe_log" || true)" -eq 0 ]] || fail "coalesce should avoid sym backend invocation"
[[ "$(grep -c -- '--query symphony --mode' "$coalesce_probe_log" || true)" -eq 1 ]] || fail "coalesce should invoke symphony exactly once"

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
  local copied_script="$layout/workflows/spotify-search/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/spotify-search/bin/spotify-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/spotify-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/spotify-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$(SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS=0 "$copied_script" "demo")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode spotify-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-spotify-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/spotify-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/spotify-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id spotify-search >/dev/null

packaged_dir="$repo_root/build/workflows/spotify-search/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/spotify-cli"
assert_file "$artifact_path"
assert_file "$artifact_sha_path"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length > 0' "packaged plist missing objects"
assert_jq_file "$packaged_json_file" '.connections | length > 0' "packaged plist missing connections"
assert_jq_file "$packaged_json_file" '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.type] | all(. == 8)' "script filter objects must be external script type=8"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.scriptfile == \"./scripts/script_filter.sh\"" "script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.keyword == \"sp||spotify\"" "keyword trigger must be sp"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.scriptargtype == 1" "script filter must pass query via argv"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.alfredfiltersresults == false" "script filter must disable Alfred local filtering"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaycustom == 1" "script filter queue delay custom mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelaymode == 0" "script filter queue delay mode mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$script_filter_uid\") | .config.queuedelayimmediatelyinitially == false" "script filter immediate queue policy mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="B8F6A479-8A88-4515-9D4D-6A0422CFEA2D") | .type == "alfred.workflow.trigger.hotkey"' "hotkey trigger node missing"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="B8F6A479-8A88-4515-9D4D-6A0422CFEA2D") | .config.hotkey == 0 and .config.hotmod == 0' "hotkey trigger must ship unassigned for user customization"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_open.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.type == 8' "action node must be external script type=8"
assert_jq_file "$packaged_json_file" '.connections["B8F6A479-8A88-4515-9D4D-6A0422CFEA2D"] | any(.destinationuid == "70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10" and .modifiers == 0)' "missing hotkey to script-filter connection"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["SPOTIFY_CLIENT_ID","SPOTIFY_CLIENT_SECRET","SPOTIFY_MARKET","SPOTIFY_MAX_RESULTS"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="SPOTIFY_CLIENT_ID") | .config.required == true' "SPOTIFY_CLIENT_ID must be required"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="SPOTIFY_CLIENT_SECRET") | .config.required == true' "SPOTIFY_CLIENT_SECRET must be required"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="SPOTIFY_MAX_RESULTS") | .config.default == "10"' "SPOTIFY_MAX_RESULTS default must be 10"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="SPOTIFY_MARKET") | .config.required == false' "SPOTIFY_MARKET must be optional"

echo "ok: spotify-search smoke test"

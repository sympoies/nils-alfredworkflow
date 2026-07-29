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
  scripts/action_copy.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/action_copy.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "multi-timezone" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "timezone-cli" ]] || fail "rust_binary must be timezone-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_copy.sh" ]] || fail "action mismatch"

if ! rg -n '^TIMEZONE_CLI_BIN[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "TIMEZONE_CLI_BIN default must be empty"
fi
if ! rg -n '^MULTI_TZ_ZONES[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "MULTI_TZ_ZONES default must be empty"
fi
if ! rg -n '^MULTI_TZ_LOCAL_OVERRIDE[[:space:]]*=[[:space:]]*"Europe/London"' "$manifest" >/dev/null; then
  fail "MULTI_TZ_LOCAL_OVERRIDE default must be Europe/London"
fi

tmp_dir="$(mktemp -d)"
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

release_cli="$repo_root/target/release/timezone-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/timezone-cli.release.backup"
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

cat >"$tmp_dir/bin/pbcopy" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
cat >"$PBCOPY_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/pbcopy"

set +e
"$workflow_dir/scripts/action_copy.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_copy.sh without args must exit 2"

copy_arg="Asia/Taipei 2026-02-10 20:35:00 UTC+08:00"
PBCOPY_STUB_OUT="$tmp_dir/pbcopy-out.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_copy.sh" "$copy_arg"
[[ "$(cat "$tmp_dir/pbcopy-out.txt")" == "$copy_arg" ]] || fail "action_copy.sh must pass exact arg to pbcopy"

cat >"$tmp_dir/stubs/timezone-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "now" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
[[ "${4:-}" == "--config-zones" ]] || exit 9
config="${5:-}"
python3 - "$query" "$config" <<'PY'
import json
import os
import sys

query = sys.argv[1]
config = sys.argv[2]

source = query if query.strip() else config
if source.strip():
    zones = [part.strip() for part in source.replace("\n", ",").split(",") if part.strip()]
else:
    local_tz = os.environ.get("MULTI_TZ_LOCAL_OVERRIDE", "UTC").strip() or "UTC"
    zones = [local_tz]

items = []
for zone in zones:
    items.append(
        {
            "uid": zone,
            "title": "2026-02-10 12:34:56",
            "subtitle": f"{zone} (UTC+00:00)",
            "arg": f"{zone} 2026-02-10 12:34:56 UTC+00:00",
            "valid": True,
        }
    )

print(json.dumps({"skipknowledge": True, "items": items}))
PY
EOS
chmod +x "$tmp_dir/stubs/timezone-cli-ok"

cat >"$tmp_dir/stubs/timezone-cli-invalid" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid timezone: unsupported timezone Mars/Olympus" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/timezone-cli-invalid"

cat >"$tmp_dir/stubs/timezone-cli-runtime" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "io error: failed to read timezone data" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/timezone-cli-runtime"

cat >"$tmp_dir/stubs/timezone-cli-malformed" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '{"unexpected":"shape"}\n'
EOS
chmod +x "$tmp_dir/stubs/timezone-cli-malformed"

success_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-ok" "$workflow_dir/scripts/script_filter.sh" "Asia/Taipei,America/New_York,Europe/London"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 3' "script_filter success must output three-item array"
assert_jq_json "$success_json" '.skipknowledge == true' "script_filter must retain uid without Alfred knowledge sorting"
assert_jq_json "$success_json" '[.items[].uid] == ["Asia/Taipei","America/New_York","Europe/London"]' "script_filter must preserve query order"

mixed_query_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-ok" "$workflow_dir/scripts/script_filter.sh" $' , Asia/Taipei,\n,America/New_York ,,Europe/London '; })"
assert_jq_json "$mixed_query_json" '[.items[].uid] == ["Asia/Taipei","America/New_York","Europe/London"]' "script_filter must preserve mixed-separator query order"

config_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-ok" MULTI_TZ_ZONES=$'Asia/Taipei\nAmerica/New_York,Europe/London' "$workflow_dir/scripts/script_filter.sh" ""; })"
assert_jq_json "$config_json" '[.items[].uid] == ["Asia/Taipei","America/New_York","Europe/London"]' "script_filter must preserve config order"

query_override_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-ok" MULTI_TZ_ZONES="Asia/Taipei,America/New_York" "$workflow_dir/scripts/script_filter.sh" "Europe/London,Asia/Tokyo"; })"
assert_jq_json "$query_override_json" '[.items[].uid] == ["Europe/London","Asia/Tokyo"]' "query input must override MULTI_TZ_ZONES"

empty_query_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-ok" MULTI_TZ_ZONES="" MULTI_TZ_LOCAL_OVERRIDE="Asia/Taipei" "$workflow_dir/scripts/script_filter.sh" ""; })"
assert_jq_json "$empty_query_json" '.items | length == 1' "empty query must return one local fallback row"
assert_jq_json "$empty_query_json" '.items[0].uid == "Asia/Taipei"' "local fallback row uid mismatch"

invalid_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-invalid" "$workflow_dir/scripts/script_filter.sh" "Mars/Olympus"; })"
assert_jq_json "$invalid_json" '.items | type == "array" and length == 1' "invalid input fallback must output single item"
assert_jq_json "$invalid_json" '.items[0].title == "Invalid timezone"' "invalid input title mapping mismatch"
assert_jq_json "$invalid_json" '.items[0].valid == false' "invalid input fallback item must be invalid"

runtime_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-runtime" "$workflow_dir/scripts/script_filter.sh" "Asia/Taipei"; })"
assert_jq_json "$runtime_json" '.items[0].title == "Timezone runtime failure"' "runtime failure title mapping mismatch"

malformed_json="$({ TIMEZONE_CLI_BIN="$tmp_dir/stubs/timezone-cli-malformed" "$workflow_dir/scripts/script_filter.sh" "Asia/Taipei"; })"
assert_jq_json "$malformed_json" '.items[0].title == "Multi Timezone error"' "malformed JSON should fallback to generic error"
assert_jq_json "$malformed_json" '.items[0].subtitle | contains("malformed Alfred JSON")' "malformed JSON subtitle mismatch"

missing_layout="$tmp_dir/layout-missing"
copied_missing_script="$missing_layout/workflows/multi-timezone/scripts/script_filter.sh"
mkdir -p "$(dirname "$copied_missing_script")"
cp "$workflow_dir/scripts/script_filter.sh" "$copied_missing_script"
chmod +x "$copied_missing_script"
missing_binary_json="$({ TIMEZONE_CLI_BIN="$missing_layout/does-not-exist/timezone-cli" "$copied_missing_script" "Asia/Taipei"; })"
assert_jq_json "$missing_binary_json" '.items[0].title == "timezone-cli binary not found"' "missing binary fallback title mismatch"
assert_jq_json "$missing_binary_json" '.items[0].valid == false' "missing binary fallback item must be invalid"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
[[ "\${1:-}" == "now" ]] || exit 9
[[ "\${2:-}" == "--query" ]] || exit 9
[[ "\${4:-}" == "--config-zones" ]] || exit 9
printf '{"items":[{"uid":"$marker","title":"2026-02-10 12:34:56","subtitle":"$marker (UTC+00:00)","arg":"$marker 2026-02-10 12:34:56 UTC+00:00","valid":true}]}'
printf '\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/multi-timezone/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/multi-timezone/bin/timezone-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/timezone-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/timezone-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$($copied_script "demo")"
  assert_jq_json "$output" ".items[0].uid == \"$marker\"" "script_filter failed to resolve $mode timezone-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-timezone-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/timezone-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/timezone-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id multi-timezone >/dev/null

packaged_dir="$repo_root/build/workflows/multi-timezone/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/timezone-cli"
assert_file "$artifact_path"
assert_file "$artifact_sha_path"

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length > 0' "packaged plist missing objects"
assert_jq_file "$packaged_json_file" '.connections | length > 0' "packaged plist missing connections"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptfile == "./scripts/script_filter.sh"' "script filter scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "tz||timezone"' "keyword trigger must be tz"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptargtype == 1' "script filter must pass query via argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_copy.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["MULTI_TZ_LOCAL_OVERRIDE","MULTI_TZ_ZONES","TIMEZONE_CLI_BIN"]' "user configuration variables mismatch"
echo "ok: multi-timezone smoke test"

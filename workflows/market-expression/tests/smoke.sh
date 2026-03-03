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
[[ "$(toml_string "$manifest" id)" == "market-expression" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "market-cli" ]] || fail "rust_binary must be market-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_copy.sh" ]] || fail "action mismatch"

if ! rg -n '^MARKET_CLI_BIN[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "MARKET_CLI_BIN default must be empty"
fi
if ! rg -n '^MARKET_DEFAULT_FIAT[[:space:]]*=[[:space:]]*"USD"' "$manifest" >/dev/null; then
  fail "MARKET_DEFAULT_FIAT default must be USD"
fi

tmp_dir="$(mktemp -d)"
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

release_cli="$repo_root/target/release/market-cli"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"
release_backup="$(artifact_backup_file "$release_cli" "$tmp_dir" "market-cli.release")"

cleanup() {
  artifact_restore_file "$release_cli" "$release_backup"
  artifact_restore_file "$artifact_path" "$artifact_backup"
  artifact_restore_file "$artifact_sha_path" "$artifact_sha_backup"
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"
workflow_smoke_write_pbcopy_stub "$tmp_dir/bin/pbcopy"
workflow_smoke_assert_action_requires_arg "$workflow_dir/scripts/action_copy.sh"

copy_arg="1 BTC + 2 ETH to USD"
PBCOPY_STUB_OUT="$tmp_dir/pbcopy-out.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_copy.sh" "$copy_arg"
[[ "$(cat "$tmp_dir/pbcopy-out.txt")" == "$copy_arg" ]] || fail "action_copy.sh must pass exact arg to pbcopy"

cat >"$tmp_dir/stubs/market-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "expr" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
[[ "${4:-}" == "--default-fiat" ]] || exit 9
default_fiat="${5:-}"
python3 - "$query" "$default_fiat" <<'PY'
import json
import sys

query = sys.argv[1]
default_fiat = sys.argv[2]

item_title = query if query.strip() else "(empty query)"
payload = {
    "items": [
        {
            "uid": "ok-item",
            "title": item_title,
            "subtitle": f"default fiat={default_fiat}",
            "arg": f"{item_title} => {default_fiat}",
            "valid": True,
        }
    ]
}
print(json.dumps(payload))
PY
EOS
chmod +x "$tmp_dir/stubs/market-cli-ok"

cat >"$tmp_dir/stubs/market-cli-unsupported-op" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "unsupported operator: *" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/market-cli-unsupported-op"

cat >"$tmp_dir/stubs/market-cli-mixed-terms" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "mixed asset and numeric terms are not supported" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/market-cli-mixed-terms"

cat >"$tmp_dir/stubs/market-cli-invalid-to" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid to clause: expected target after to" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/market-cli-invalid-to"

cat >"$tmp_dir/stubs/market-cli-invalid-expression" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid expression: unexpected token near 'to'" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/market-cli-invalid-expression"

cat >"$tmp_dir/stubs/market-cli-provider" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "provider error: upstream timeout" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/market-cli-provider"

cat >"$tmp_dir/stubs/market-cli-runtime" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "io error: evaluation pipeline failed" >&2
exit 3
EOS
chmod +x "$tmp_dir/stubs/market-cli-runtime"

cat >"$tmp_dir/stubs/market-cli-malformed" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '{"unexpected":"shape"}\n'
EOS
chmod +x "$tmp_dir/stubs/market-cli-malformed"

success_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-ok" MARKET_DEFAULT_FIAT="TWD" "$workflow_dir/scripts/script_filter.sh" "1 BTC + 2 ETH"; })"
assert_jq_json "$success_json" '.items | type == "array" and length == 1' "script_filter success must output one-item array"
assert_jq_json "$success_json" '.items[0].title == "1 BTC + 2 ETH"' "script_filter should preserve query in success output"
assert_jq_json "$success_json" '.items[0].subtitle == "default fiat=TWD"' "script_filter must pass MARKET_DEFAULT_FIAT to cli"

success_default_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-ok" MARKET_DEFAULT_FIAT="" "$workflow_dir/scripts/script_filter.sh" "BTC to"; })"
assert_jq_json "$success_default_json" '.items[0].subtitle == "default fiat=USD"' "empty MARKET_DEFAULT_FIAT should fallback to USD"

unsupported_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-unsupported-op" "$workflow_dir/scripts/script_filter.sh" "1 BTC * 2"; })"
assert_jq_json "$unsupported_json" '.items[0].title == "Unsupported operator"' "unsupported operator title mapping mismatch"
assert_jq_json "$unsupported_json" '.items[0].valid == false' "unsupported operator fallback item must be invalid"

mixed_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-mixed-terms" "$workflow_dir/scripts/script_filter.sh" "1 BTC + 2"; })"
assert_jq_json "$mixed_json" '.items[0].title == "Invalid expression terms"' "mixed terms title mapping mismatch"

invalid_to_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-invalid-to" "$workflow_dir/scripts/script_filter.sh" "1 BTC to"; })"
assert_jq_json "$invalid_to_json" '.items[0].title == "Invalid to-clause"' "invalid to-clause title mapping mismatch"

invalid_expression_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-invalid-expression" "$workflow_dir/scripts/script_filter.sh" "BTC to"; })"
assert_jq_json "$invalid_expression_json" '.items[0].title == "Invalid expression"' "invalid expression title mapping mismatch"

provider_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-provider" "$workflow_dir/scripts/script_filter.sh" "1 BTC to USD"; })"
assert_jq_json "$provider_json" '.items[0].title == "Market Expression provider failure"' "provider failure title mapping mismatch"

runtime_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-runtime" "$workflow_dir/scripts/script_filter.sh" "1 BTC to USD"; })"
assert_jq_json "$runtime_json" '.items[0].title == "Market Expression runtime failure"' "runtime failure title mapping mismatch"

malformed_json="$({ MARKET_CLI_BIN="$tmp_dir/stubs/market-cli-malformed" "$workflow_dir/scripts/script_filter.sh" "1 BTC to USD"; })"
assert_jq_json "$malformed_json" '.items[0].title == "Market Expression error"' "malformed JSON should fallback to generic title"
assert_jq_json "$malformed_json" '.items[0].subtitle | contains("malformed Alfred JSON")' "malformed JSON subtitle mismatch"

missing_layout="$tmp_dir/layout-missing"
copied_missing_script="$missing_layout/workflows/market-expression/scripts/script_filter.sh"
mkdir -p "$(dirname "$copied_missing_script")"
cp "$workflow_dir/scripts/script_filter.sh" "$copied_missing_script"
chmod +x "$copied_missing_script"
missing_binary_json="$({ MARKET_CLI_BIN="$missing_layout/does-not-exist/market-cli" "$copied_missing_script" "1 BTC"; })"
assert_jq_json "$missing_binary_json" '.items[0].title == "market-cli binary not found"' "missing binary fallback title mismatch"
assert_jq_json "$missing_binary_json" '.items[0].valid == false' "missing binary fallback item must be invalid"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
[[ "\${1:-}" == "expr" ]] || exit 9
[[ "\${2:-}" == "--query" ]] || exit 9
[[ "\${4:-}" == "--default-fiat" ]] || exit 9
printf '{"items":[{"uid":"$marker","title":"$marker","subtitle":"ok","arg":"$marker","valid":true}]}'
printf '\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"
  local copied_script="$layout/workflows/market-expression/scripts/script_filter.sh"

  mkdir -p "$(dirname "$copied_script")"
  cp "$workflow_dir/scripts/script_filter.sh" "$copied_script"
  chmod +x "$copied_script"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/market-expression/bin/market-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/market-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/market-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$($copied_script "demo")"
  assert_jq_json "$output" ".items[0].uid == \"$marker\"" "script_filter failed to resolve $mode market-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-market-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/market-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/market-cli"
  exit 0
fi

if [[ "\$#" -eq 2 && "\$1" == "test" && "\$2" == "--workspace" ]]; then
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id market-expression >/dev/null

workflow_test_root="$tmp_dir/workflow-test-entry"
workflow_test_script="$workflow_test_root/scripts/workflow-test.sh"
workflow_test_smoke="$workflow_test_root/workflows/market-expression/tests/smoke.sh"
workflow_test_third_party_audit="$workflow_test_root/scripts/ci/third-party-artifacts-audit.sh"
workflow_test_marker="$tmp_dir/workflow-test-smoke.marker"
workflow_test_output="$tmp_dir/workflow-test.out"

mkdir -p \
  "$(dirname "$workflow_test_script")" \
  "$(dirname "$workflow_test_smoke")" \
  "$(dirname "$workflow_test_third_party_audit")"
cp "$repo_root/scripts/workflow-test.sh" "$workflow_test_script"
chmod +x "$workflow_test_script"

cat >"$workflow_test_smoke" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
: "${WORKFLOW_TEST_STUB_MARKER:?WORKFLOW_TEST_STUB_MARKER must be set}"
touch "$WORKFLOW_TEST_STUB_MARKER"
echo "ok: workflow-test smoke stub"
EOS
chmod +x "$workflow_test_smoke"

cat >"$workflow_test_third_party_audit" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "ok: third-party artifacts audit stub"
EOS
chmod +x "$workflow_test_third_party_audit"

WORKFLOW_TEST_STUB_MARKER="$workflow_test_marker" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_test_script" --id market-expression >"$workflow_test_output"
assert_file "$workflow_test_marker"
if ! rg -n '^ok: tests passed$' "$workflow_test_output" >/dev/null; then
  fail "workflow-test entrypoint did not report success for market-expression id"
fi

packaged_dir="$repo_root/build/workflows/market-expression/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/market-cli"
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
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.keyword == "mx||market"' "keyword trigger must be mx"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.scriptargtype == 1' "script filter must pass query via argv"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10") | .config.alfredfiltersresults == false' "script filter must disable Alfred local filtering"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="96AC3342-84A9-449E-B0AB-114E2068FC34") | .type == "alfred.workflow.trigger.hotkey"' "hotkey trigger node missing"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="96AC3342-84A9-449E-B0AB-114E2068FC34") | .config.hotkey == 0' "hotkey trigger should default to empty key"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="96AC3342-84A9-449E-B0AB-114E2068FC34") | .config.hotmod == 0' "hotkey trigger should default to no modifiers"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.scriptfile == "./scripts/action_copy.sh"' "action scriptfile wiring mismatch"
assert_jq_file "$packaged_json_file" '.objects[] | select(.uid=="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A") | .config.type == 8' "action node must be external script type=8"
assert_jq_file "$packaged_json_file" '.connections["96AC3342-84A9-449E-B0AB-114E2068FC34"] | any(.destinationuid == "70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10" and .modifiers == 0)' "missing hotkey to script-filter connection"
assert_jq_file "$packaged_json_file" '.connections["70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"] | any(.destinationuid == "D7E624DB-D4AB-4D53-8C03-D051A1A97A4A" and .modifiers == 0)' "missing script-filter to action connection"
assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["MARKET_CLI_BIN","MARKET_DEFAULT_FIAT"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="MARKET_CLI_BIN") | .config.default == ""' "MARKET_CLI_BIN default must be empty string"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="MARKET_DEFAULT_FIAT") | .config.default == "USD"' "MARKET_DEFAULT_FIAT default must be USD"

echo "ok: market-expression smoke test"

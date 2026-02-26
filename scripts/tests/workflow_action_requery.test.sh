#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/scripts/lib/workflow_action_requery.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/workflow-action-requery.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "$label (expected='$expected', actual='$actual')"
  fi
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "$label (missing '$needle')"
  fi
}

test_parse_valid_payload() {
  wfar_parse_requery_payload "wiki-requery:zh:rust lang" "wiki-requery:"
  assert_eq "zh" "${WFAR_REQUERY_SELECTOR:-}" "selector parse"
  assert_eq "rust lang" "${WFAR_REQUERY_QUERY:-}" "query parse"

  wfar_parse_requery_payload "wiki-requery:ja:" "wiki-requery:"
  assert_eq "ja" "${WFAR_REQUERY_SELECTOR:-}" "selector parse with empty query"
  assert_eq "" "${WFAR_REQUERY_QUERY:-}" "empty query parse"
}

test_parse_invalid_payloads() {
  local stderr_file="$test_root/parse-invalid.stderr"
  if wfar_parse_requery_payload "wiki-requery:zh" "wiki-requery:" >"$test_root/out" 2>"$stderr_file"; then
    fail "payload without query separator should fail"
  fi
  assert_contains "$(cat "$stderr_file")" "invalid requery payload" "invalid payload error text"

  if wfar_parse_requery_payload "wiki-requery::query" "wiki-requery:" >"$test_root/out" 2>"$stderr_file"; then
    fail "payload with empty selector should fail"
  fi
  assert_contains "$(cat "$stderr_file")" "invalid requery payload" "empty selector error text"
}

test_write_state_file() {
  local state_file="$test_root/cache/wiki-language-override.state"
  wfar_write_state_file "$state_file" "zh"
  assert_eq "zh" "$(sed -n '1p' "$state_file")" "state file value"
}

test_trigger_command_override() {
  local command_stub="$test_root/bin/requery-command"
  local output_file="$test_root/command-output.txt"

  mkdir -p "$(dirname "$command_stub")"
  cat >"$command_stub" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$WFAR_TEST_OUTPUT"
EOS
  chmod +x "$command_stub"

  WFAR_TEST_OUTPUT="$output_file" wfar_trigger_requery "wk rust book" "$command_stub" "Alfred 5"
  assert_eq "wk rust book" "$(cat "$output_file")" "command override payload"
}

test_trigger_osascript_fallback() {
  local fake_bin="$test_root/fallback-bin"
  local osascript_stub="$fake_bin/osascript"
  local output_file="$test_root/osascript-output.txt"
  mkdir -p "$fake_bin"

  cat >"$osascript_stub" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >"$WFAR_TEST_OUTPUT"
EOS
  chmod +x "$osascript_stub"

  PATH="$fake_bin:$PATH" WFAR_TEST_OUTPUT="$output_file" \
    wfar_trigger_requery "st helldivers" "" "Alfred 5"

  assert_contains "$(cat "$output_file")" 'tell application "Alfred 5" to search "st helldivers"' "osascript payload"
}

test_trigger_missing_fallback() {
  local stderr_file="$test_root/missing-fallback.stderr"
  local empty_bin="$test_root/empty-bin"
  mkdir -p "$empty_bin"

  set +e
  PATH="$empty_bin" wfar_trigger_requery "wk rust" "" "Alfred 5" 2>"$stderr_file"
  local rc=$?
  set -e

  assert_eq "1" "$rc" "missing fallback exit code"
  assert_contains "$(cat "$stderr_file")" "cannot trigger Alfred requery" "missing fallback stderr"
}

main() {
  test_parse_valid_payload
  test_parse_invalid_payloads
  test_write_state_file
  test_trigger_command_override
  test_trigger_osascript_fallback
  test_trigger_missing_fallback
  printf 'ok: workflow_action_requery tests passed\n'
}

main "$@"

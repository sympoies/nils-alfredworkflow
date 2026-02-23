#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/scripts/lib/script_filter_cli_driver.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/script-filter-cli-driver.test.XXXXXX")"
fake_bin_dir="$test_root/bin"
no_jq_bin_dir="$test_root/no-jq-bin"
test_tmpdir="$test_root/tmp"
mkdir -p "$fake_bin_dir" "$no_jq_bin_dir" "$test_tmpdir"

cleanup() {
  rm -rf "$test_root"
}
trap cleanup EXIT

cat >"$fake_bin_dir/jq" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1-}" == "-e" ]]; then
  shift
fi

if [[ "${1-}" != '.items | type == "array"' ]]; then
  exit 2
fi

input="$(cat)"
if printf '%s' "$input" | grep -Eq '"items"[[:space:]]*:[[:space:]]*\['; then
  exit 0
fi

exit 1
EOF
chmod +x "$fake_bin_dir/jq"

for required_cmd in cat grep mktemp rm sed tr; do
  required_path="$(command -v "$required_cmd")"
  if [[ -z "$required_path" ]]; then
    printf 'missing required command for no-jq test path: %s\n' "$required_cmd" >&2
    exit 1
  fi
  ln -s "$required_path" "$no_jq_bin_dir/$required_cmd"
done

export PATH="$fake_bin_dir:$PATH"
export TMPDIR="$test_tmpdir"

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

assert_no_driver_err_files() {
  local label="$1"
  local leftover

  leftover="$(find "$TMPDIR" -maxdepth 1 -type f -name 'script-filter-cli-driver.err.*' -print -quit)"
  if [[ -n "$leftover" ]]; then
    fail "$label (leftover err file: $leftover)"
  fi
}

TEST_EXEC_MODE="success"
TEST_EXEC_STDOUT='{"items":[{"title":"ok","valid":false}]}'
TEST_EXEC_STDERR="execution failed"
TEST_MAPPER_MODE="ok"
TEST_DRIVER_OUTPUT=""
MAPPER_CALLS_FILE="$test_root/mapper-calls.log"
MAPPER_INPUT_FILE="$test_root/mapper-input.log"

EMPTY_SENTINEL="empty sentinel message"
MALFORMED_SENTINEL="malformed sentinel message"

test_exec_callback() {
  case "${TEST_EXEC_MODE}" in
  success)
    printf '%s' "$TEST_EXEC_STDOUT"
    ;;
  empty)
    return 0
    ;;
  malformed)
    printf '%s' "$TEST_EXEC_STDOUT"
    ;;
  fail)
    printf '%s' "$TEST_EXEC_STDERR" >&2
    return 23
    ;;
  *)
    printf 'unsupported TEST_EXEC_MODE: %s\n' "$TEST_EXEC_MODE" >&2
    return 2
    ;;
  esac
}

test_error_mapper() {
  printf '1\n' >>"$MAPPER_CALLS_FILE"
  printf '%s' "${1-}" >"$MAPPER_INPUT_FILE"

  case "${TEST_MAPPER_MODE}" in
  ok)
    printf '{"items":[{"title":"Mapped error","subtitle":"mapper subtitle","valid":false}]}'
    ;;
  empty)
    return 0
    ;;
  fail)
    return 17
    ;;
  malformed)
    printf '{"error":"mapper malformed output"}'
    ;;
  *)
    printf 'unsupported TEST_MAPPER_MODE: %s\n' "$TEST_MAPPER_MODE" >&2
    return 2
    ;;
  esac
}

reset_state() {
  : >"$MAPPER_CALLS_FILE"
  : >"$MAPPER_INPUT_FILE"
  TEST_DRIVER_OUTPUT=""
}

mapper_call_count() {
  wc -l <"$MAPPER_CALLS_FILE" | tr -d '[:space:]'
}

mapper_last_input() {
  cat "$MAPPER_INPUT_FILE"
}

run_driver_with_path() {
  local run_path="$1"
  shift
  local output_file
  output_file="$(PATH="$run_path" mktemp "$TMPDIR/driver-output.XXXXXX")"
  PATH="$run_path" sfcd_run_cli_flow "$@" >"$output_file"
  TEST_DRIVER_OUTPUT="$(PATH="$run_path" cat "$output_file")"
  PATH="$run_path" rm -f "$output_file"
}

run_driver() {
  run_driver_with_path "$PATH" "$@"
}

run_driver_no_jq() {
  run_driver_with_path "$no_jq_bin_dir" "$@"
}

test_success_passthrough() {
  reset_state
  TEST_EXEC_MODE="success"
  TEST_EXEC_STDOUT='{"items":[{"title":"Ready","valid":false}]}'
  TEST_MAPPER_MODE="ok"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "$TEST_EXEC_STDOUT" "$TEST_DRIVER_OUTPUT" "success output passthrough"
  assert_eq "0" "$(mapper_call_count)" "mapper not called on success"
  assert_no_driver_err_files "success err-file cleanup"
}

test_exec_failure_maps_stderr() {
  reset_state
  TEST_EXEC_MODE="fail"
  TEST_EXEC_STDERR="resolver failed"
  TEST_MAPPER_MODE="ok"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper called on execute failure"
  assert_eq "$TEST_EXEC_STDERR" "$(mapper_last_input)" "stderr forwarded to mapper"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Mapped error"' "mapped error row emitted"
  assert_no_driver_err_files "execute failure err-file cleanup"
}

test_empty_output_guard() {
  reset_state
  TEST_EXEC_MODE="empty"
  TEST_MAPPER_MODE="ok"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper called on empty output"
  assert_eq "$EMPTY_SENTINEL" "$(mapper_last_input)" "empty-output guard message passed"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Mapped error"' "mapped empty-output error emitted"
  assert_no_driver_err_files "empty-output err-file cleanup"
}

test_malformed_json_guard() {
  reset_state
  TEST_EXEC_MODE="malformed"
  TEST_EXEC_STDOUT='{"not_items":[]}'
  TEST_MAPPER_MODE="ok"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper called on malformed JSON"
  assert_eq "$MALFORMED_SENTINEL" "$(mapper_last_input)" "malformed guard message passed"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Mapped error"' "mapped malformed-json error emitted"
  assert_no_driver_err_files "malformed-json err-file cleanup"
}

test_malformed_json_guard_without_jq() {
  reset_state
  TEST_EXEC_MODE="malformed"
  TEST_EXEC_STDOUT='{"not_items":[]}'
  TEST_MAPPER_MODE="ok"

  run_driver_no_jq test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper called on malformed JSON without jq"
  assert_eq "$MALFORMED_SENTINEL" "$(mapper_last_input)" "malformed guard message passed without jq"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Mapped error"' "mapped malformed-json error emitted without jq"
  assert_no_driver_err_files "malformed-json without-jq err-file cleanup"
}

test_fallback_error_row_when_mapper_invalid() {
  reset_state
  TEST_EXEC_MODE="fail"
  TEST_EXEC_STDERR="hard failure"
  TEST_MAPPER_MODE="malformed"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper attempted before fallback"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Workflow runtime error"' "fallback title emitted"
  assert_contains "$TEST_DRIVER_OUTPUT" 'hard failure' "fallback row includes raw message"
  assert_no_driver_err_files "fallback err-file cleanup"
}

test_fallback_error_row_when_mapper_invalid_without_jq() {
  reset_state
  TEST_EXEC_MODE="fail"
  TEST_EXEC_STDERR="hard failure"
  TEST_MAPPER_MODE="malformed"

  run_driver_no_jq test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper attempted before fallback without jq"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Workflow runtime error"' "fallback title emitted without jq"
  assert_contains "$TEST_DRIVER_OUTPUT" 'hard failure' "fallback row includes raw message without jq"
  assert_no_driver_err_files "fallback without-jq err-file cleanup"
}

test_fallback_error_row_normalizes_control_chars() {
  reset_state
  TEST_EXEC_MODE="fail"
  TEST_EXEC_STDERR=$'line\twith\tcontrols'
  TEST_MAPPER_MODE="malformed"

  run_driver test_exec_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_contains "$TEST_DRIVER_OUTPUT" '"Workflow runtime error"' "fallback title emitted"
  assert_contains "$TEST_DRIVER_OUTPUT" 'line with controls' "fallback subtitle normalizes tabs"
  if [[ "$TEST_DRIVER_OUTPUT" == *$'\t'* ]]; then
    fail "fallback row should not include literal tab characters"
  fi
  assert_no_driver_err_files "control-char fallback err-file cleanup"
}

test_missing_execute_callback_guard() {
  reset_state
  TEST_MAPPER_MODE="ok"

  run_driver missing_execute_callback test_error_mapper "$EMPTY_SENTINEL" "$MALFORMED_SENTINEL"

  assert_eq "1" "$(mapper_call_count)" "mapper called when execute callback is missing"
  assert_eq "script-filter execute callback is not defined" "$(mapper_last_input)" "missing callback message"
  assert_contains "$TEST_DRIVER_OUTPUT" '"Mapped error"' "mapped missing-callback error emitted"
  assert_no_driver_err_files "missing-callback err-file cleanup"
}

main() {
  test_success_passthrough
  test_exec_failure_maps_stderr
  test_empty_output_guard
  test_malformed_json_guard
  test_malformed_json_guard_without_jq
  test_fallback_error_row_when_mapper_invalid
  test_fallback_error_row_when_mapper_invalid_without_jq
  test_fallback_error_row_normalizes_control_chars
  test_missing_execute_callback_guard
  printf 'ok: script_filter_cli_driver tests passed\n'
}

main "$@"

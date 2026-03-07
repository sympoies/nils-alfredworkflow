#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/scripts/lib/script_filter_async_coalesce.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/script-filter-async-coalesce.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_rc() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "$label (expected rc=$expected, actual rc=$actual)"
  fi
}

reset_context() {
  export ALFRED_WORKFLOW_CACHE="$test_root/cache"
  rm -rf "$ALFRED_WORKFLOW_CACHE"
  sfac_init_context "coalesce-test" "coalesce-test-fallback"
}

FAKE_NOW_MS=0
sfac_now_epoch_millis() {
  printf '%s\n' "$FAKE_NOW_MS"
}

test_wait_for_final_query_uses_millisecond_precision() {
  reset_context

  FAKE_NOW_MS=1700000000000
  set +e
  sfac_wait_for_final_query "symphony" "1"
  local rc=$?
  set -e
  assert_rc 1 "$rc" "first query should start pending"

  FAKE_NOW_MS=1700000000500
  set +e
  sfac_wait_for_final_query "symphony" "1"
  rc=$?
  set -e
  assert_rc 1 "$rc" "same query should remain pending before full settle window"

  FAKE_NOW_MS=1700000000999
  set +e
  sfac_wait_for_final_query "symphony" "1"
  rc=$?
  set -e
  assert_rc 1 "$rc" "same query should not dispatch just before settle threshold"

  FAKE_NOW_MS=1700000001000
  sfac_wait_for_final_query "symphony" "1"
}

test_wait_for_final_query_normalizes_legacy_second_timestamps() {
  reset_context

  local request_file
  request_file="$(sfac_request_file_path)"
  mkdir -p "$(dirname "$request_file")"
  printf 'legacy-seq\n1700000000\nopen\n' >"$request_file"

  FAKE_NOW_MS=1700000000999
  set +e
  sfac_wait_for_final_query "open" "1"
  local rc=$?
  set -e
  assert_rc 1 "$rc" "legacy second timestamps should stay pending until normalized settle threshold"

  FAKE_NOW_MS=1700000001000
  sfac_wait_for_final_query "open" "1"
}

main() {
  test_wait_for_final_query_uses_millisecond_precision
  test_wait_for_final_query_normalizes_legacy_second_timestamps
  printf 'ok: script_filter_async_coalesce tests passed\n'
}

main "$@"

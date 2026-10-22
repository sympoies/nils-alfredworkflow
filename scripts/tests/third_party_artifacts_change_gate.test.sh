#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
gate="$repo_root/scripts/ci/third-party-artifacts-change-gate.sh"

tests_total=0
tests_failed=0

pass() {
  printf 'ok - %s\n' "$1"
}

fail() {
  printf 'not ok - %s\n' "$1"
  tests_failed=$((tests_failed + 1))
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"

  if [[ "$haystack" != *"$needle"* ]]; then
    printf 'assert failed: %s\nmissing substring: %s\n' "$label" "$needle" >&2
    return 1
  fi
  return 0
}

run_plan() {
  bash "$gate" --plan-only "$@"
}

run_test() {
  local name="$1"
  shift
  tests_total=$((tests_total + 1))
  if "$@"; then
    pass "$name"
  else
    fail "$name"
  fi
}

test_lockfile_triggers_gate() {
  local output
  output="$(run_plan --changed-file Cargo.lock)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=run" "lockfile gate" &&
    assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE_TRIGGER=Cargo.lock" "lockfile trigger"
}

test_node_lockfile_triggers_gate() {
  local output
  output="$(run_plan --changed-file package-lock.json)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=run" "node lockfile gate" &&
    assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE_TRIGGER=package-lock.json" "node lockfile trigger"
}

test_runtime_pin_triggers_gate() {
  local output
  output="$(run_plan --changed-file scripts/lib/codex_cli_version.sh)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=run" "runtime pin gate" &&
    assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE_TRIGGER=scripts/lib/codex_cli_version.sh" "runtime pin trigger"
}

test_crate_manifest_triggers_gate() {
  local output
  output="$(run_plan --changed-file crates/workflow-cli/Cargo.toml)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=run" "crate manifest gate" &&
    assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE_TRIGGER=crates/workflow-cli/Cargo.toml" "crate manifest trigger"
}

test_artifact_output_triggers_gate() {
  local output
  output="$(run_plan --changed-file THIRD_PARTY_LICENSES.md)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=run" "artifact output gate" &&
    assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE_TRIGGER=THIRD_PARTY_LICENSES.md" "artifact output trigger"
}

test_unrelated_doc_skips_gate() {
  local output
  output="$(run_plan --changed-file README.md)"
  assert_contains "$output" "THIRD_PARTY_ARTIFACTS_GATE=skip" "doc skip"
}

run_test "lockfile triggers third-party gate" test_lockfile_triggers_gate
run_test "node lockfile triggers third-party gate" test_node_lockfile_triggers_gate
run_test "runtime pin triggers third-party gate" test_runtime_pin_triggers_gate
run_test "crate manifest triggers third-party gate" test_crate_manifest_triggers_gate
run_test "artifact output triggers third-party gate" test_artifact_output_triggers_gate
run_test "unrelated doc skips third-party gate" test_unrelated_doc_skips_gate

if [[ "$tests_failed" -ne 0 ]]; then
  printf 'FAIL: %s/%s tests failed\n' "$tests_failed" "$tests_total" >&2
  exit 1
fi

printf '\nPASS: third_party_artifacts_change_gate.test.sh (%s tests)\n' "$tests_total"

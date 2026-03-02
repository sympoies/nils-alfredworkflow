#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/third-party-licenses-audit.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

tests_total=0
tests_failed=0

last_rc=0
last_stdout=""
last_stderr=""

pass() {
  local name="$1"
  printf 'ok - %s\n' "$name"
}

fail() {
  local name="$1"
  printf 'not ok - %s\n' "$name"
  tests_failed=$((tests_failed + 1))
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local label="$3"

  if [[ "$actual" != "$expected" ]]; then
    printf 'assert failed: %s\nexpected: %s\nactual:   %s\n' "$label" "$expected" "$actual" >&2
    return 1
  fi
  return 0
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

dump_last_run() {
  printf 'last exit code: %s\n' "$last_rc" >&2
  if [[ -f "$last_stdout" ]]; then
    printf '%s\n' '--- stdout ---' >&2
    cat "$last_stdout" >&2 || true
  fi
  if [[ -f "$last_stderr" ]]; then
    printf '%s\n' '--- stderr ---' >&2
    cat "$last_stderr" >&2 || true
  fi
}

setup_fixture() {
  local fixture="$test_root/fixture-$RANDOM-$RANDOM"
  mkdir -p "$fixture/scripts/ci"

  cp "$repo_root/scripts/ci/third-party-licenses-audit.sh" "$fixture/scripts/ci/third-party-licenses-audit.sh"
  chmod +x "$fixture/scripts/ci/third-party-licenses-audit.sh"

  cat >"$fixture/scripts/generate-third-party-licenses.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--check" ]]; then
  echo "error: expected --check" >&2
  exit 2
fi

state="${AUDIT_FIXTURE_STATE:-clean}"

case "$state" in
clean)
  echo "PASS [check] $PWD/THIRD_PARTY_LICENSES.md is up to date"
  exit 0
  ;;
missing)
  echo "error: missing output artifact: $PWD/THIRD_PARTY_LICENSES.md (run --write first)" >&2
  exit 1
  ;;
drift)
  echo "FAIL [check] $PWD/THIRD_PARTY_LICENSES.md is stale" >&2
  echo "Run: bash scripts/generate-third-party-licenses.sh --write" >&2
  exit 1
  ;;
broken)
  echo "error: required input missing: $PWD/Cargo.lock" >&2
  exit 1
  ;;
*)
  echo "error: unknown fixture state: $state" >&2
  exit 2
  ;;
esac
EOF
  chmod +x "$fixture/scripts/generate-third-party-licenses.sh"

  cat >"$fixture/THIRD_PARTY_LICENSES.md" <<'EOF'
# fixture artifact
EOF

  fixture="$(cd "$fixture" && pwd)"
  printf '%s\n' "$fixture"
}

run_audit() {
  local fixture="$1"
  local state="$2"
  shift 2

  last_stdout="$fixture/stdout.log"
  last_stderr="$fixture/stderr.log"
  last_rc=0

  (
    cd "$fixture"
    AUDIT_FIXTURE_STATE="$state" bash "$fixture/scripts/ci/third-party-licenses-audit.sh" "$@"
  ) >"$last_stdout" 2>"$last_stderr" || last_rc=$?
}

read_combined_output() {
  local fixture="$1"
  cat "$fixture/stdout.log" "$fixture/stderr.log"
}

test_clean_passes() {
  local fixture
  fixture="$(setup_fixture)"

  run_audit "$fixture" clean --strict
  if ! assert_eq "0" "$last_rc" "clean strict exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "PASS [third-party-licenses]" "clean PASS output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: PASS" "clean result"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_missing_non_strict_warns() {
  local fixture
  fixture="$(setup_fixture)"

  rm -f "$fixture/THIRD_PARTY_LICENSES.md"
  run_audit "$fixture" missing
  if ! assert_eq "0" "$last_rc" "missing non-strict exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "WARN [third-party-licenses]" "missing WARN output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: PASS with warnings" "missing non-strict result"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_missing_strict_fails() {
  local fixture
  fixture="$(setup_fixture)"

  rm -f "$fixture/THIRD_PARTY_LICENSES.md"
  run_audit "$fixture" missing --strict
  if ! assert_eq "1" "$last_rc" "missing strict exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "WARN [third-party-licenses]" "missing strict WARN output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "FAIL [third-party-licenses] strict mode treats warnings as failures" "missing strict FAIL output"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_drift_non_strict_warns() {
  local fixture
  fixture="$(setup_fixture)"

  run_audit "$fixture" drift
  if ! assert_eq "0" "$last_rc" "drift non-strict exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "WARN [third-party-licenses] artifact drift detected" "drift WARN output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: PASS with warnings" "drift non-strict result"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_drift_strict_fails() {
  local fixture
  fixture="$(setup_fixture)"

  run_audit "$fixture" drift --strict
  if ! assert_eq "1" "$last_rc" "drift strict exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "WARN [third-party-licenses] artifact drift detected" "drift strict WARN output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "FAIL [third-party-licenses] strict mode treats warnings as failures" "drift strict FAIL output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: FAIL (strict mode treats warnings as failures)" "drift strict result"; then
    dump_last_run
    return 1
  fi

  return 0
}

run_test() {
  local test_name="$1"
  tests_total=$((tests_total + 1))
  if "$test_name"; then
    pass "$test_name"
  else
    fail "$test_name"
  fi
}

run_test test_clean_passes
run_test test_missing_non_strict_warns
run_test test_missing_strict_fails
run_test test_drift_non_strict_warns
run_test test_drift_strict_fails

if [[ "$tests_failed" -ne 0 ]]; then
  printf 'FAIL: %d/%d tests failed\n' "$tests_failed" "$tests_total" >&2
  exit 1
fi

printf 'PASS: %d tests\n' "$tests_total"

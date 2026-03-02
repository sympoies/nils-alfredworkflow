#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/third-party-licenses-release-bundle.test.XXXXXX")"
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

write_sha256() {
  local file_path="$1"
  local out_path="$2"

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" >"$out_path"
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" >"$out_path"
    return
  fi
  echo "missing sha256 tool (need shasum or sha256sum)" >&2
  exit 1
}

setup_fixture() {
  local fixture="$test_root/fixture-$RANDOM-$RANDOM"
  local dist_dir="$fixture/dist/release-bundles"

  mkdir -p "$fixture/scripts/ci" "$dist_dir"
  cp "$repo_root/scripts/ci/release-bundle-third-party-audit.sh" "$fixture/scripts/ci/release-bundle-third-party-audit.sh"
  chmod +x "$fixture/scripts/ci/release-bundle-third-party-audit.sh"

  cat >"$fixture/THIRD_PARTY_LICENSES.md" <<'EOF'
# Fixture Third-Party Licenses
EOF

  cat >"$dist_dir/workflows-v0.0.0-test.zip" <<'EOF'
fixture-zip-payload
EOF

  cat >"$dist_dir/workflow-clear-quarantine-standalone.sh" <<'EOF'
#!/usr/bin/env bash
echo fixture
EOF
  chmod +x "$dist_dir/workflow-clear-quarantine-standalone.sh"

  cp "$fixture/THIRD_PARTY_LICENSES.md" "$dist_dir/THIRD_PARTY_LICENSES.md"

  write_sha256 "$dist_dir/workflows-v0.0.0-test.zip" "$dist_dir/workflows-v0.0.0-test.zip.sha256"
  write_sha256 "$dist_dir/workflow-clear-quarantine-standalone.sh" "$dist_dir/workflow-clear-quarantine-standalone.sh.sha256"
  write_sha256 "$dist_dir/THIRD_PARTY_LICENSES.md" "$dist_dir/THIRD_PARTY_LICENSES.md.sha256"

  fixture="$(cd "$fixture" && pwd)"
  printf '%s\n' "$fixture"
}

run_audit() {
  local fixture="$1"
  shift

  last_stdout="$fixture/stdout.log"
  last_stderr="$fixture/stderr.log"
  last_rc=0
  (
    cd "$fixture"
    bash "$fixture/scripts/ci/release-bundle-third-party-audit.sh" "$@"
  ) >"$last_stdout" 2>"$last_stderr" || last_rc=$?
}

read_combined_output() {
  local fixture="$1"
  cat "$fixture/stdout.log" "$fixture/stderr.log"
}

test_release_bundle_passes_when_all_required_artifacts_exist() {
  local fixture
  fixture="$(setup_fixture)"

  run_audit "$fixture" --tag v0.0.0-test --dist-dir dist/release-bundles
  if ! assert_eq "0" "$last_rc" "release bundle pass exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "PASS [release-bundle-third-party]" "release bundle PASS prefix"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: PASS" "release bundle PASS result"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_release_bundle_fails_when_license_artifact_is_missing() {
  local fixture
  fixture="$(setup_fixture)"
  rm -f "$fixture/dist/release-bundles/THIRD_PARTY_LICENSES.md"

  run_audit "$fixture" --tag v0.0.0-test --dist-dir dist/release-bundles
  if ! assert_eq "1" "$last_rc" "release bundle missing license exit code"; then
    dump_last_run
    return 1
  fi

  local output
  output="$(read_combined_output "$fixture")"
  if ! assert_contains "$output" "FAIL [release-bundle-third-party] third-party license artifact missing:" "release bundle missing license FAIL detail"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$output" "Result: FAIL (release bundle third-party compliance issues detected)" "release bundle missing license FAIL result"; then
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

run_test test_release_bundle_passes_when_all_required_artifacts_exist
run_test test_release_bundle_fails_when_license_artifact_is_missing

if [[ "$tests_failed" -ne 0 ]]; then
  printf 'FAIL: %d/%d tests failed\n' "$tests_failed" "$tests_total" >&2
  exit 1
fi

printf 'PASS: %d tests\n' "$tests_total"

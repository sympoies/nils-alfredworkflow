#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/third-party-artifacts-generator.test.XXXXXX")"
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
  mkdir -p "$fixture/scripts/lib" "$fixture/bin"

  cp "$repo_root/scripts/generate-third-party-artifacts.sh" "$fixture/scripts/generate-third-party-artifacts.sh"
  chmod +x "$fixture/scripts/generate-third-party-artifacts.sh"

  cat >"$fixture/scripts/lib/codex_cli_version.sh" <<'__PIN__'
#!/usr/bin/env bash
CODEX_CLI_CRATE="fixture-runtime-crate"
CODEX_CLI_VERSION="1.2.3"
CODEX_CLI_PINNED_CRATE="$CODEX_CLI_CRATE"
CODEX_CLI_PINNED_VERSION="$CODEX_CLI_VERSION"
__PIN__

  cat >"$fixture/Cargo.toml" <<'__CARGO__'
[package]
name = "third-party-artifacts-generator-fixture"
version = "0.1.0"
edition = "2021"
__CARGO__

  mkdir -p "$fixture/src"
  cat >"$fixture/src/main.rs" <<'__RS__'
fn main() {}
__RS__

  (
    cd "$fixture"
    cargo generate-lockfile >/dev/null
  )

  cat >"$fixture/package-lock.json" <<'__LOCK__'
{
  "name": "third-party-artifacts-generator-fixture",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "third-party-artifacts-generator-fixture",
      "version": "1.0.0"
    }
  }
}
__LOCK__

  cat >"$fixture/bin/curl" <<'__CURL__'
#!/usr/bin/env bash
set -euo pipefail

url="${@: -1}"
expected="https://crates.io/api/v1/crates/fixture-runtime-crate/1.2.3"
if [[ "$url" != "$expected" ]]; then
  echo "unexpected URL: $url" >&2
  exit 1
fi

cat <<'JSON'
{
  "version": {
    "crate": "fixture-runtime-crate",
    "num": "1.2.3",
    "license": "MIT",
    "repository": "https://example.com/fixture-runtime"
  }
}
JSON
__CURL__
  chmod +x "$fixture/bin/curl"

  fixture="$(cd "$fixture" && pwd)"
  printf '%s\n' "$fixture"
}

run_generator() {
  local fixture="$1"
  shift

  last_stdout="$fixture/stdout.log"
  last_stderr="$fixture/stderr.log"
  last_rc=0
  (
    cd "$fixture"
    PATH="$fixture/bin:$PATH" bash "$fixture/scripts/generate-third-party-artifacts.sh" "$@"
  ) >"$last_stdout" 2>"$last_stderr" || last_rc=$?
}

test_clean_write_and_check() {
  local fixture
  fixture="$(setup_fixture)"

  run_generator "$fixture" --write
  if ! assert_eq "0" "$last_rc" "clean write exit code"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stdout")" "PASS [write] generated" "clean write success output"; then
    dump_last_run
    return 1
  fi
  if [[ ! -f "$fixture/THIRD_PARTY_LICENSES.md" ]]; then
    echo "expected generated artifact missing: $fixture/THIRD_PARTY_LICENSES.md" >&2
    dump_last_run
    return 1
  fi
  if [[ ! -f "$fixture/THIRD_PARTY_NOTICES.md" ]]; then
    echo "expected generated artifact missing: $fixture/THIRD_PARTY_NOTICES.md" >&2
    dump_last_run
    return 1
  fi

  run_generator "$fixture" --check
  if ! assert_eq "0" "$last_rc" "clean check exit code"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stdout")" "PASS [check] third-party artifacts are up to date" "clean check success output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$fixture/THIRD_PARTY_LICENSES.md")" "## Deterministic Provenance" "generated licenses content"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$fixture/THIRD_PARTY_NOTICES.md")" "## Notice Extraction Policy" "generated notices content"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_drift_detection() {
  local fixture
  fixture="$(setup_fixture)"

  run_generator "$fixture" --write
  if ! assert_eq "0" "$last_rc" "drift setup write exit code"; then
    dump_last_run
    return 1
  fi

  echo "<!-- drift marker -->" >>"$fixture/THIRD_PARTY_NOTICES.md"

  run_generator "$fixture" --check
  if ! assert_eq "1" "$last_rc" "drift check exit code"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stderr")" "FAIL [check]" "drift check failure output"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stderr")" "Run: bash scripts/generate-third-party-artifacts.sh --write" "drift remediation output"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_missing_input_error() {
  local fixture
  fixture="$(setup_fixture)"

  rm -f "$fixture/package-lock.json"

  run_generator "$fixture" --write
  if ! assert_eq "1" "$last_rc" "missing input exit code"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stderr")" "required input missing:" "missing input error prefix"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$(cat "$last_stderr")" "$fixture/package-lock.json" "missing input file path"; then
    dump_last_run
    return 1
  fi

  return 0
}

test_mpl_source_and_license_url_lines() {
  local fixture
  fixture="$(setup_fixture)"

  local fake_crate_dir="$fixture/fake-crates/option-ext-0.2.0"
  mkdir -p "$fake_crate_dir"
  cat >"$fake_crate_dir/Cargo.toml" <<'__FAKE_CARGO_TOML__'
[package]
name = "option-ext"
version = "0.2.0"
edition = "2021"
__FAKE_CARGO_TOML__
  cat >"$fake_crate_dir/LICENSE.txt" <<'__FAKE_LICENSE__'
Fake MPL text fixture.
__FAKE_LICENSE__

  local fake_metadata="$fixture/fake-cargo-metadata.json"
  cat >"$fake_metadata" <<__FAKE_METADATA__
{
  "packages": [
    {
      "name": "third-party-artifacts-generator-fixture",
      "version": "0.1.0",
      "id": "path+file://$fixture#third-party-artifacts-generator-fixture@0.1.0",
      "source": null,
      "manifest_path": "$fixture/Cargo.toml"
    },
    {
      "name": "option-ext",
      "version": "0.2.0",
      "id": "registry+https://github.com/rust-lang/crates.io-index#option-ext@0.2.0",
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "license": "MPL-2.0",
      "manifest_path": "$fake_crate_dir/Cargo.toml"
    }
  ]
}
__FAKE_METADATA__

  cat >"$fixture/bin/cargo" <<__FAKE_CARGO__
#!/usr/bin/env bash
set -euo pipefail

if [[ "\$#" -gt 0 && "\$1" == "metadata" ]]; then
  cat "$fake_metadata"
  exit 0
fi

echo "unexpected cargo invocation in fixture: \$*" >&2
exit 1
__FAKE_CARGO__
  chmod +x "$fixture/bin/cargo"

  run_generator "$fixture" --write
  if ! assert_eq "0" "$last_rc" "mpl fixture write exit code"; then
    dump_last_run
    return 1
  fi

  local notices
  notices="$(cat "$fixture/THIRD_PARTY_NOTICES.md")"
  if ! assert_contains "$notices" "- Source URL: <https://crates.io/crates/option-ext/0.2.0>" "mpl source url line"; then
    dump_last_run
    return 1
  fi
  if ! assert_contains "$notices" "- License text (MPL-2.0): <https://mozilla.org/MPL/2.0/>" "mpl license text line"; then
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

run_test test_clean_write_and_check
run_test test_drift_detection
run_test test_missing_input_error
run_test test_mpl_source_and_license_url_lines

if [[ "$tests_failed" -ne 0 ]]; then
  printf 'FAIL: %d/%d tests failed\n' "$tests_failed" "$tests_total" >&2
  exit 1
fi

printf 'PASS: %d tests\n' "$tests_total"

#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
search_script="$repo_root/scripts/devlog-search.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/devlog-search.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "$label (missing '$needle')"
  fi
}

create_fixture_repo() {
  local fixture_repo="$1"
  mkdir -p "$fixture_repo/scripts" "$fixture_repo/docs/devlog"

  cp "$search_script" "$fixture_repo/scripts/devlog-search.sh"
  chmod +x "$fixture_repo/scripts/devlog-search.sh"

  cat >"$fixture_repo/docs/devlog/2026-01.md" <<'EOF'
# Development log - 2026-01

## 2026-01-05 - January milestone

**Result**

- Shipped the january marker.
EOF

  cat >"$fixture_repo/docs/devlog/2026-02.md" <<'EOF'
# Development log - 2026-02

## 2026-02-05 - February milestone

**Result**

- Shipped the february marker.
EOF
}

run_search() {
  local fixture_repo="$1"
  shift
  (
    cd "$fixture_repo"
    bash scripts/devlog-search.sh "$@"
  )
}

test_search_matches_across_months() {
  local fixture_repo="$test_root/all-months"
  create_fixture_repo "$fixture_repo"

  local output
  output="$(run_search "$fixture_repo" milestone)"
  assert_contains "$output" "docs/devlog/2026-01.md" "january file is searched"
  assert_contains "$output" "docs/devlog/2026-02.md" "february file is searched"
}

test_search_is_case_insensitive_and_month_scoped() {
  local fixture_repo="$test_root/month-scope"
  create_fixture_repo "$fixture_repo"

  local output
  output="$(run_search "$fixture_repo" MARKER 2026-02)"
  assert_contains "$output" "february marker" "case-insensitive month-scoped match"
  if [[ "$output" == *"2026-01.md"* ]]; then
    fail "month-scoped search should not read other months"
  fi
}

test_no_match_exits_nonzero() {
  local fixture_repo="$test_root/no-match"
  create_fixture_repo "$fixture_repo"

  local output=""
  set +e
  output="$(run_search "$fixture_repo" absent-term 2>&1)"
  local rc=$?
  set -e

  [[ "$rc" -ne 0 ]] || fail "a search without matches must exit non-zero"
  assert_contains "$output" "(no matches for 'absent-term')" "no-match message"
}

test_absent_month_file_exits_one() {
  local fixture_repo="$test_root/absent-month"
  create_fixture_repo "$fixture_repo"

  local output=""
  set +e
  output="$(run_search "$fixture_repo" marker 2026-09 2>&1)"
  local rc=$?
  set -e

  [[ "$rc" -eq 1 ]] || fail "a well-formed but absent month must exit 1 (got $rc)"
  assert_contains "$output" "no devlog file for 2026-09" "absent month message"
}

test_invalid_usage_exits_two() {
  local fixture_repo="$test_root/usage"
  create_fixture_repo "$fixture_repo"

  local rc=0
  set +e
  run_search "$fixture_repo" >/dev/null 2>&1
  rc=$?
  set -e
  [[ "$rc" -eq 2 ]] || fail "missing term must exit 2 (got $rc)"

  set +e
  run_search "$fixture_repo" term 2026-13 >/dev/null 2>&1
  rc=$?
  set -e
  [[ "$rc" -eq 2 ]] || fail "malformed month must exit 2 (got $rc)"
}

main() {
  test_search_matches_across_months
  test_search_is_case_insensitive_and_month_scoped
  test_no_match_exits_nonzero
  test_absent_month_file_exits_one
  test_invalid_usage_exits_two
  printf 'ok: devlog search tests passed\n'
}

main "$@"

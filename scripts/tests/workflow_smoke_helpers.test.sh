#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/scripts/lib/workflow_smoke_helpers.sh"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/workflow-smoke-helpers.test.XXXXXX")"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

fail_test() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local label="$3"

  if [[ "$expected" != "$actual" ]]; then
    fail_test "$label (expected='$expected', actual='$actual')"
  fi
}

assert_file_contains() {
  local path="$1"
  local needle="$2"
  local label="$3"

  if ! grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    fail_test "$label (missing '$needle' in $path)"
  fi
}

test_open_stub_writer() {
  local stub_path="$tmp_dir/bin/open"
  local output_path="$tmp_dir/open.out"
  workflow_smoke_write_open_stub "$stub_path"

  OPEN_STUB_OUT="$output_path" "$stub_path" "https://example.com"
  assert_eq "https://example.com" "$(cat "$output_path")" "open stub should forward argv"
}

test_pbcopy_stub_writer() {
  local stub_path="$tmp_dir/bin/pbcopy"
  local output_path="$tmp_dir/pbcopy.out"
  workflow_smoke_write_pbcopy_stub "$stub_path"

  printf 'copy-me' | PBCOPY_STUB_OUT="$output_path" "$stub_path"
  assert_eq "copy-me" "$(cat "$output_path")" "pbcopy stub should preserve stdin bytes"
}

test_artifact_backup_and_restore() {
  local target_path="$tmp_dir/artifact.txt"
  local backup_path=""

  printf 'before\n' >"$target_path"
  backup_path="$(artifact_backup_file "$target_path" "$tmp_dir/backups" "artifact")"
  [[ -n "$backup_path" && -f "$backup_path" ]] || fail_test "artifact backup should create backup file"

  printf 'after\n' >"$target_path"
  artifact_restore_file "$target_path" "$backup_path"
  assert_eq "before" "$(tr -d '\n' <"$target_path")" "artifact restore should recover original contents"
}

test_artifact_restore_missing_backup_removes_target() {
  local target_path="$tmp_dir/remove-me.txt"
  printf 'stale\n' >"$target_path"

  artifact_restore_file "$target_path" "$tmp_dir/does-not-exist.backup"
  [[ ! -e "$target_path" ]] || fail_test "artifact restore should remove target when backup is missing"
}

test_action_requires_arg_helper() {
  local action_path="$tmp_dir/action.sh"
  cat >"$action_path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ $# -gt 0 ]] || exit 2
exit 0
EOF
  chmod +x "$action_path"

  workflow_smoke_assert_action_requires_arg "$action_path"
  "$action_path" "ok"
}

test_wait_for_file_contains_helper() {
  local log_path="$tmp_dir/wait.log"
  : >"$log_path"

  (
    sleep 1
    printf 'ready pattern\n' >>"$log_path"
  ) &

  workflow_smoke_wait_for_file_contains "$log_path" "ready pattern" 3 ||
    fail_test "wait helper should detect pattern before timeout"
  assert_file_contains "$log_path" "ready pattern" "wait helper should preserve log contents"
}

test_wait_for_file_contains_timeout() {
  local log_path="$tmp_dir/timeout.log"
  : >"$log_path"

  set +e
  workflow_smoke_wait_for_file_contains "$log_path" "never-happens" 1
  local rc=$?
  set -e

  [[ "$rc" -ne 0 ]] || fail_test "wait helper should fail when pattern does not appear"
}

test_open_stub_writer
test_pbcopy_stub_writer
test_artifact_backup_and_restore
test_artifact_restore_missing_backup_removes_target
test_action_requires_arg_helper
test_wait_for_file_contains_helper
test_wait_for_file_contains_timeout

printf 'PASS: workflow smoke helper tests\n'

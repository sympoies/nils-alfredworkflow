#!/usr/bin/env bash
set -euo pipefail

# Regression test for the Cambridge bootstrap EXIT-trap cleanup ordering.
#
# cleanup() reads $bootstrap_status and is installed via `trap cleanup EXIT`.
# Under `set -u`, if any step between installing the trap and the first
# bootstrap_status assignment fails, the EXIT trap itself would abort with
# "bootstrap_status: unbound variable" -- so the result file would never be
# written and the state file never removed, defeating the recent-failure
# cooldown. The fix initializes bootstrap_status="err" BEFORE the trap.
#
# This test forces a failure right after the trap is installed (before the
# normal flow can flip the status to "ok") and asserts that cleanup still:
#   1. ran without an unbound-variable error, and
#   2. wrote the result file with "err" status, and
#   3. removed the state file.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
bootstrap="$repo_root/workflows/cambridge-dict/scripts/lib/cambridge_runtime_bootstrap.sh"

# The repo source ships the helper non-executable (the workflow pack step chmods
# it +x); invoke it through `bash` so the test does not depend on the file mode.
[[ -f "$bootstrap" ]] || {
  printf 'FAIL: bootstrap helper missing: %s\n' "$bootstrap" >&2
  exit 1
}

test_root="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-runtime-cleanup.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

workflow_dir="$test_root/workflow"
state_file="$test_root/state/bootstrap.state"
log_file="$test_root/state/bootstrap.log"
result_file="$test_root/state/bootstrap.result"
mkdir -p "$workflow_dir" "$(dirname "$state_file")"

# Force a failure between trap-install and the "ok" status flip: make the
# state-file path a directory so the helper's `printf '%s\n' "$$" >"$state_file"`
# write fails under `set -e` immediately after the trap is installed. With the
# fix, bootstrap_status defaults to "err" before the trap, so cleanup runs
# cleanly; without the fix the EXIT trap aborts on the unbound variable.
mkdir -p "$state_file"

stderr_file="$test_root/bootstrap.stderr"
set +e
bash "$bootstrap" \
  --workflow-dir "$workflow_dir" \
  --state-file "$state_file" \
  --log-file "$log_file" \
  --result-file "$result_file" \
  >/dev/null 2>"$stderr_file"
rc=$?
set -e

# The helper must have failed (the induced write error), not succeeded.
[[ "$rc" -ne 0 ]] || fail "bootstrap should fail when the state-file write is sabotaged"

if grep -qi 'unbound variable' "$stderr_file"; then
  printf '%s\n' '--- bootstrap stderr ---' >&2
  cat "$stderr_file" >&2
  fail "EXIT trap must not abort with an unbound-variable error"
fi

[[ -f "$result_file" ]] || fail "cleanup must write the result file even on early failure"

status="$(cut -f1 <"$result_file")"
[[ "$status" == "err" ]] || fail "result status must be 'err' on failure (got '$status')"

# Run 1 used a directory state path (cleanup's `rm -f` cannot remove a
# directory), so it proves only the no-unbound-variable + err-result invariant.
# Run 2 uses a plain-file state path and induces a failure AFTER the state file
# is written, so it also exercises the state-file removal in cleanup.
#
# Seam: a read-only workflow dir makes cambridge_runtime_write_package_json's
# `cat >"$workflow_dir/package.json"` redirection fail under `set -e`. That step
# runs after the state-file write and before any node/npm install, so it is
# deterministic, network-free, and independent of where node lives on PATH.
rm -rf "$state_file" "$result_file"
ro_workflow_dir="$test_root/ro-workflow"
mkdir -p "$ro_workflow_dir"
chmod a-w "$ro_workflow_dir"
# Restore writability on cleanup so the test_root teardown can remove it.
trap 'chmod u+w "$ro_workflow_dir" 2>/dev/null || true; rm -rf "$test_root"' EXIT

set +e
bash "$bootstrap" \
  --workflow-dir "$ro_workflow_dir" \
  --state-file "$state_file" \
  --log-file "$log_file" \
  --result-file "$result_file" \
  >/dev/null 2>"$stderr_file"
rc=$?
set -e

[[ "$rc" -ne 0 ]] || fail "bootstrap should fail when the workflow dir is read-only"

if grep -qi 'unbound variable' "$stderr_file"; then
  printf '%s\n' '--- bootstrap stderr ---' >&2
  cat "$stderr_file" >&2
  fail "EXIT trap must not abort with an unbound-variable error (read-only path)"
fi

[[ -f "$result_file" ]] || fail "cleanup must write the result file on the read-only failure"
status="$(cut -f1 <"$result_file")"
[[ "$status" == "err" ]] || fail "result status must be 'err' on the read-only failure (got '$status')"
[[ ! -e "$state_file" ]] || fail "cleanup must remove the state file on failure"

# Structural guard for the precise FIX 1 ordering: the only code between the trap
# install and the original status assignment was ensure_common_runtime_path,
# which cannot be made to fail in a normal environment, so the behavioral runs
# above cannot distinguish the buggy ordering on their own. Assert directly that
# the first `bootstrap_status=` assignment precedes `trap cleanup EXIT`, so a
# regression that moves the default back below the trap is caught.
status_line="$(grep -n '^bootstrap_status=' "$bootstrap" | head -1 | cut -d: -f1)"
trap_line="$(grep -n '^trap cleanup EXIT' "$bootstrap" | head -1 | cut -d: -f1)"
[[ -n "$status_line" ]] || fail "could not locate a top-level bootstrap_status assignment in the helper"
[[ -n "$trap_line" ]] || fail "could not locate 'trap cleanup EXIT' in the helper"
[[ "$status_line" -lt "$trap_line" ]] ||
  fail "bootstrap_status must default before 'trap cleanup EXIT' (status line $status_line, trap line $trap_line)"

printf 'ok: cambridge runtime bootstrap cleanup tests passed\n'

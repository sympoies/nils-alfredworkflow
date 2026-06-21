#!/usr/bin/env bash
set -euo pipefail

# Regression test for PID-reuse false positives in the Cambridge runtime
# bootstrap liveness check.
#
# cambridge_runtime_bootstrap_running treated a within-window state file as
# "running" based solely on `kill -0 "$pid"`. If a bootstrap was SIGKILLed (so
# its EXIT trap never ran and the state file survived) and the OS later recycled
# that PID for an unrelated process, the recorded PID is alive but is NOT our
# bootstrap -- yet the Script Filter would stay pinned to a phantom
# "Installing..." pending item for the whole stale window.
#
# The fix verifies, in BOTH the within-window and stale branches, that
# `ps -p "$pid" -o command=` names cambridge_runtime_bootstrap.sh before
# reporting "running". This test writes a state file whose PID belongs to a live
# NON-bootstrap process (a `sleep`) with a fresh mtime and asserts the check
# reports not-running and removes the stale state file.
#
# Note: the temp state path is deliberately NOT named `state_file`.
# cambridge_runtime_bootstrap_running declares `local state_file`, and bash's
# dynamic scoping would shadow a global of that name inside the stubbed
# cambridge_runtime_state_file (making it unbound under `set -u`).

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
script_filter="$repo_root/workflows/cambridge-dict/scripts/script_filter.sh"

[[ -f "$script_filter" ]] || {
  printf 'FAIL: script_filter missing: %s\n' "$script_filter" >&2
  exit 1
}

# Sourcing script_filter.sh defines its functions without running the main flow
# (guarded by BASH_SOURCE == $0).
# shellcheck disable=SC1090
source "$script_filter"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-runtime-pid-reuse.test.XXXXXX")"

sleep_pid=""
cleanup() {
  if [[ -n "$sleep_pid" ]]; then
    kill "$sleep_pid" 2>/dev/null || true
  fi
  rm -rf "$test_root"
}
trap cleanup EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

test_state_path="$test_root/bootstrap.state"

# Redirect the runtime state file to our temp path for the duration of the test.
cambridge_runtime_state_file() {
  printf '%s\n' "$test_state_path"
}

# Spawn a live, long-running NON-bootstrap process and record its PID. Its
# command line is `sleep`, which does not match cambridge_runtime_bootstrap.sh.
sleep 120 &
sleep_pid="$!"

# Sanity: the spawned PID must really be alive (kill -0 succeeds), otherwise the
# test would pass for the wrong reason.
kill -0 "$sleep_pid" 2>/dev/null || fail "spawned sleep process is not alive; cannot exercise PID reuse"

# Write the state file with the live non-bootstrap PID and a fresh mtime (so the
# within-window branch is exercised, not the stale branch).
printf '%s\n' "$sleep_pid" >"$test_state_path"
[[ -f "$test_state_path" ]] || fail "state file was not created"

# A live PID whose command is not the bootstrap must NOT count as running.
if cambridge_runtime_bootstrap_running; then
  fail "PID reuse: a live non-bootstrap process must not be reported as running"
fi

# The stale (phantom) state file must be removed so the Script Filter can move on.
[[ ! -e "$test_state_path" ]] || fail "stale state file pointing at a recycled PID must be removed"

# The recycled process must NOT have been signalled (terminate is reserved for a
# genuinely-stale bootstrap, never an unrelated process).
kill -0 "$sleep_pid" 2>/dev/null || fail "the unrelated process must not be terminated by the liveness check"

printf 'ok: cambridge runtime PID-reuse tests passed\n'

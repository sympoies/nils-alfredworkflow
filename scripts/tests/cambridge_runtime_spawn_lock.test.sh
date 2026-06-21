#!/usr/bin/env bash
set -euo pipefail

# Regression test for the Cambridge bootstrap spawn race.
#
# Alfred re-runs the Script Filter on every keystroke. handle_runtime_bootstrap
# checks cambridge_runtime_bootstrap_running (false when no state file exists)
# and then start_cambridge_runtime_bootstrap, which `nohup ... &` and returns
# immediately; the child writes its PID to the state file only LATER. During
# that window a second Script Filter run also sees "not running" and would spawn
# a SECOND concurrent bootstrap -- two npm/playwright installs racing the same
# prefix.
#
# The fix gates the spawn behind an atomic `mkdir` lock claim. This test covers:
#   1. the lock-claim helper in isolation (first claim wins, second fails until
#      released, then a fresh claim wins again), and
#   2. the full handle_runtime_bootstrap spawn path invoked twice back-to-back
#      before the first child writes its state file, asserting exactly one launch.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
script_filter="$repo_root/workflows/cambridge-dict/scripts/script_filter.sh"

[[ -f "$script_filter" ]] || {
  printf 'FAIL: script_filter missing: %s\n' "$script_filter" >&2
  exit 1
}

# Sourcing script_filter.sh defines its functions (and, transitively,
# cambridge_runtime_common.sh) without running the main flow.
# shellcheck disable=SC1090
source "$script_filter"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-runtime-spawn-lock.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

# --- Part 1: lock-claim helper in isolation -------------------------------

lock_state_dir="$test_root/lock-iso"
mkdir -p "$lock_state_dir"
lock_state_path="$lock_state_dir/bootstrap.state"

set +e
cambridge_runtime_claim_bootstrap_lock "$lock_state_path"
claim1=$?
cambridge_runtime_claim_bootstrap_lock "$lock_state_path"
claim2=$?
set -e

[[ "$claim1" -eq 0 ]] || fail "first lock claim must succeed (got rc=$claim1)"
[[ "$claim2" -ne 0 ]] || fail "second lock claim must fail while the lock is held"

lock_dir="$(cambridge_runtime_bootstrap_lock_dir "$lock_state_path")"
[[ -d "$lock_dir" ]] || fail "lock directory must exist while held"

cambridge_runtime_release_bootstrap_lock "$lock_state_path"
[[ ! -e "$lock_dir" ]] || fail "release must remove the lock directory"

set +e
cambridge_runtime_claim_bootstrap_lock "$lock_state_path"
claim3=$?
set -e
[[ "$claim3" -eq 0 ]] || fail "a fresh claim after release must succeed (got rc=$claim3)"
cambridge_runtime_release_bootstrap_lock "$lock_state_path"

# --- Part 2: full handle_runtime_bootstrap spawn path, twice back-to-back ---

spawn_state_dir="$test_root/spawn"
mkdir -p "$spawn_state_dir"
spawn_state_path="$spawn_state_dir/bootstrap.state"
launch_log="$test_root/launches.log"
: >"$launch_log"

# Redirect the runtime state file to our temp path.
cambridge_runtime_state_file() {
  printf '%s\n' "$spawn_state_path"
}

# Treat the runtime as supported and not recently failed so the flow reaches the
# spawn/lock decision.
cambridge_runtime_bootstrap_supported() {
  return 0
}
cambridge_runtime_recent_failure() {
  return 1
}

# Model the real spawn window: nohup returns immediately and the child writes
# its state file only later, so the stub records a launch WITHOUT writing the
# state file. This is precisely the window in which a second Script Filter run
# would otherwise spawn a duplicate.
start_cambridge_runtime_bootstrap() {
  printf 'launch\n' >>"$launch_log"
  return 0
}

# Suppress the pending JSON emitted on each run.
emit_runtime_bootstrap_pending_item() {
  return 0
}

set +e
handle_runtime_bootstrap
hb1=$?
handle_runtime_bootstrap
hb2=$?
set -e

[[ "$hb1" -eq 0 ]] || fail "first handle_runtime_bootstrap must succeed (got rc=$hb1)"
[[ "$hb2" -eq 0 ]] || fail "second handle_runtime_bootstrap must surface pending, not error (got rc=$hb2)"

launches="$(wc -l <"$launch_log" | tr -d '[:space:]')"
[[ "$launches" -eq 1 ]] ||
  fail "spawn race: exactly one bootstrap must launch across two back-to-back runs (got $launches)"

# Releasing the lock (as the bootstrap EXIT trap would) lets a later run spawn.
cambridge_runtime_release_bootstrap_lock "$spawn_state_path"
set +e
handle_runtime_bootstrap
hb3=$?
set -e
[[ "$hb3" -eq 0 ]] || fail "third handle_runtime_bootstrap after release must succeed (got rc=$hb3)"
launches="$(wc -l <"$launch_log" | tr -d '[:space:]')"
[[ "$launches" -eq 2 ]] ||
  fail "after lock release a new run must spawn again (expected 2 launches, got $launches)"

printf 'ok: cambridge runtime spawn-lock tests passed\n'

#!/usr/bin/env bash
set -euo pipefail

# Regression test for the Cambridge bootstrap stale-window coupling.
#
# The background bootstrap writes its state file once and never refreshes the
# mtime during a progressing install, while the Script Filter expires the
# pending state after CAMBRIDGE_RUNTIME_BOOTSTRAP_STALE_SECONDS (default 600).
# A user who raises CAMBRIDGE_PLAYWRIGHT_INSTALL_TIMEOUT_SECONDS above the stale
# window -- but does not also raise the stale window -- would otherwise have a
# still-progressing install killed before its own install timeout elapses.
#
# The native install path downloads every CFT browser (chromium +
# chromium-headless-shell) under its own per-artifact --max-time = install
# timeout, so the effective stale window must never fall below the whole native
# download budget (native_count * install timeout) plus a buffer for the
# surrounding npm install.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/workflows/cambridge-dict/scripts/lib/cambridge_runtime_common.sh"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

# check <description> <expected> <stale> <install> <buffer>
# Empty string for any of the three env values exercises the unset/default path
# (the helper uses ${VAR:-default}, which also triggers on empty values).
check() {
  local description="$1" expected="$2" stale="$3" install="$4" buffer="$5"
  local actual
  actual="$(
    CAMBRIDGE_RUNTIME_BOOTSTRAP_STALE_SECONDS="$stale" \
      CAMBRIDGE_PLAYWRIGHT_INSTALL_TIMEOUT_SECONDS="$install" \
      CAMBRIDGE_RUNTIME_BOOTSTRAP_STALE_BUFFER_SECONDS="$buffer" \
      cambridge_runtime_effective_stale_seconds
  )"
  if [[ "$actual" != "$expected" ]]; then
    fail "$description: expected $expected, got $actual"
  fi
}

# The native install path downloads every CFT browser (chromium +
# chromium-headless-shell) under its OWN per-artifact --max-time = install
# timeout, so the worst-case native budget is native_count * install_timeout
# plus the buffer for the surrounding npm install. The floor must therefore be
# native_count * install_timeout + buffer (native_count = 2), not a single
# install timeout, or a slow-but-progressing two-download install is killed.

# Defaults: 2 * 300 + buffer 120 = 720 floor; configured 600 is below it.
check "default stale window covers both native downloads" "720" "" "" ""

# Raised install timeout above the default stale window: floor must follow it
# across both downloads (2 * 900 + 120 = 1920).
check "raised install timeout floors the stale window" "1920" "" "900" ""

# Explicit large stale window above the floor is respected unchanged.
check "explicit large stale window respected" "2000" "2000" "300" ""

# Invalid configured stale window falls back to 600, then floored by 720.
check "invalid stale window falls back to 600" "720" "not-a-number" "300" ""

# Invalid install timeout falls back to 300, keeping the 720 floor above 600.
check "invalid install timeout falls back to 300" "720" "" "not-a-number" ""

# Buffer override widens the floor (2 * 900 + 300 = 2100).
check "buffer override widens the floor" "2100" "" "900" "300"

# Buffer of exactly zero is valid (^[0-9]+$): the floor collapses to the bare
# native download budget (2 * 300 = 600), and a configured window below it is
# floored up.
check "buffer zero floors at native download budget" "600" "100" "300" "0"

# Configured stale window equal to the floor is on the boundary: the function
# only floors when configured < floor, so an exact match is respected as-is.
check "configured equals floor is respected" "720" "720" "300" ""

# Invalid buffer falls back to 120 (not 0). The configured 350 sits below both
# the correct 720 floor (2 * 300 + 120) and the broken-fallback 600 floor
# (2 * 300 + 0), so 720 proves the 120 fallback (a broken 0 fallback -> 600).
check "invalid buffer falls back to 120" "720" "350" "300" "bad"

printf 'ok: cambridge runtime stale-window tests passed\n'

#!/usr/bin/env bash
# Shared Cambridge workflow-local Playwright runtime helpers.

cambridge_runtime_playwright_version() {
  printf '%s\n' '1.58.2'
}

cambridge_runtime_install_timeout_seconds() {
  local raw="${CAMBRIDGE_PLAYWRIGHT_INSTALL_TIMEOUT_SECONDS:-300}"
  if [[ ! "$raw" =~ ^[1-9][0-9]*$ ]]; then
    raw="300"
  fi
  printf '%s\n' "$raw"
}

cambridge_runtime_bootstrap_stale_buffer_seconds() {
  local raw="${CAMBRIDGE_RUNTIME_BOOTSTRAP_STALE_BUFFER_SECONDS:-120}"
  if [[ ! "$raw" =~ ^[0-9]+$ ]]; then
    raw="120"
  fi
  printf '%s\n' "$raw"
}

# Effective age (seconds) after which a running bootstrap is treated as stale.
# The bootstrap writes its state file once and does not refresh the mtime during
# a progressing install, so the stale window must never fall below the install
# timeout plus a buffer for the surrounding npm install -- otherwise raising
# CAMBRIDGE_PLAYWRIGHT_INSTALL_TIMEOUT_SECONDS above the stale window would kill
# an install that is still legitimately running.
cambridge_runtime_effective_stale_seconds() {
  local configured="${CAMBRIDGE_RUNTIME_BOOTSTRAP_STALE_SECONDS:-600}"
  if [[ ! "$configured" =~ ^[1-9][0-9]*$ ]]; then
    configured="600"
  fi

  local install_timeout buffer floor
  install_timeout="$(cambridge_runtime_install_timeout_seconds)"
  buffer="$(cambridge_runtime_bootstrap_stale_buffer_seconds)"
  floor=$((install_timeout + buffer))

  if [[ "$configured" -lt "$floor" ]]; then
    printf '%s\n' "$floor"
    return 0
  fi
  printf '%s\n' "$configured"
}

cambridge_runtime_write_package_json() {
  local workflow_dir="$1"
  local playwright_version
  playwright_version="$(cambridge_runtime_playwright_version)"

  cat >"$workflow_dir/package.json" <<JSON
{
  "name": "cambridge-dict-runtime",
  "private": true,
  "version": "1.0.0",
  "dependencies": {
    "playwright": "$playwright_version"
  }
}
JSON
}

cambridge_runtime_terminate_tree() {
  local pid="$1"
  local child

  if command -v pgrep >/dev/null 2>&1; then
    while IFS= read -r child; do
      [[ -n "$child" ]] || continue
      cambridge_runtime_terminate_tree "$child"
    done < <(pgrep -P "$pid" 2>/dev/null || true)
  fi

  kill "$pid" 2>/dev/null || true
}

cambridge_runtime_kill_tree() {
  local pid="$1"
  local child

  if command -v pgrep >/dev/null 2>&1; then
    while IFS= read -r child; do
      [[ -n "$child" ]] || continue
      cambridge_runtime_kill_tree "$child"
    done < <(pgrep -P "$pid" 2>/dev/null || true)
  fi

  kill -9 "$pid" 2>/dev/null || true
}

# Age (seconds) of a filesystem path's mtime, or non-zero if it cannot be read.
cambridge_runtime_path_age_seconds() {
  local path="$1"
  local modified_at now

  if modified_at="$(stat -f %m "$path" 2>/dev/null)"; then
    :
  elif modified_at="$(stat -c %Y "$path" 2>/dev/null)"; then
    :
  else
    return 1
  fi

  now="$(date +%s)"
  printf '%s\n' "$((now - modified_at))"
}

# Spawn lock directory that gates a single concurrent bootstrap. It lives beside
# the state file so the same state dir owns both artifacts.
cambridge_runtime_bootstrap_lock_dir() {
  local state_file="$1"
  printf '%s/.bootstrap.lock\n' "$(dirname "$state_file")"
}

# Release the spawn lock. The bootstrap EXIT trap calls this so the lock is
# cleared whenever the installer process exits (success, failure, or signal that
# still runs the trap).
cambridge_runtime_release_bootstrap_lock() {
  local state_file="$1"
  rm -rf "$(cambridge_runtime_bootstrap_lock_dir "$state_file")"
}

# Atomically claim the spawn lock. Returns 0 if the caller now owns the lock and
# may spawn a bootstrap, non-zero if a bootstrap is already starting/running.
#
# `mkdir` is the atomic create-or-fail primitive: two concurrent Script Filter
# runs cannot both succeed. Stale-lock recovery handles a previous run that died
# without releasing the lock (e.g. SIGKILL, which skips the EXIT trap): if the
# lock directory is older than the effective stale window AND no live bootstrap
# owns the state file, the lock is reclaimed.
#
# Residual window: a lock claimed within the stale window whose owner was
# SIGKILLed before writing the state file is held until the window elapses. This
# only delays a retry; it never spawns a duplicate concurrent install, which is
# the corruption the lock prevents.
cambridge_runtime_claim_bootstrap_lock() {
  local state_file="$1"
  local lock_dir
  lock_dir="$(cambridge_runtime_bootstrap_lock_dir "$state_file")"

  if mkdir "$lock_dir" 2>/dev/null; then
    return 0
  fi

  # Lock already exists. Treat it as held unless it is provably stale.
  local max_age lock_age
  max_age="$(cambridge_runtime_effective_stale_seconds)"
  if lock_age="$(cambridge_runtime_path_age_seconds "$lock_dir" 2>/dev/null)" &&
    [[ "$lock_age" -le "$max_age" ]]; then
    # Within the window: a still-spawning or still-running bootstrap may own it.
    return 1
  fi

  # Past the window: only reclaim when no live bootstrap owns the state file.
  if [[ -f "$state_file" ]]; then
    local pid command
    pid="$(sed -n '1p' "$state_file" 2>/dev/null | tr -d '[:space:]')"
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
      command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
      if [[ "$command" == *"cambridge_runtime_bootstrap.sh"* ]]; then
        return 1
      fi
    fi
  fi

  rm -rf "$lock_dir"
  if mkdir "$lock_dir" 2>/dev/null; then
    return 0
  fi
  return 1
}

cambridge_runtime_run_with_timeout() {
  local timeout_seconds="$1"
  shift

  "$@" &
  local command_pid="$!"
  local elapsed=0

  while kill -0 "$command_pid" 2>/dev/null; do
    if [[ "$elapsed" -ge "$timeout_seconds" ]]; then
      cambridge_runtime_terminate_tree "$command_pid"
      sleep 2
      if kill -0 "$command_pid" 2>/dev/null; then
        cambridge_runtime_kill_tree "$command_pid"
      fi
      wait "$command_pid" 2>/dev/null || true
      printf 'error: command timed out after %ss: %s\n' "$timeout_seconds" "$*" >&2
      return 124
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done

  wait "$command_pid"
}

cambridge_runtime_verify_playwright_package() {
  local workflow_dir="$1"
  (
    cd "$workflow_dir" || exit
    node --input-type=module -e "import('playwright').then(() => process.stdout.write('ok: playwright package resolved\n'))"
  )
}

cambridge_runtime_verify_headless_chromium_launch() {
  local workflow_dir="$1"
  (
    cd "$workflow_dir" || exit
    node --input-type=module -e "
      import('playwright').then(async ({ chromium }) => {
        const browser = await chromium.launch({ headless: true });
        const executable = chromium.executablePath();
        await browser.close();
        process.stdout.write('ok: headless chromium launched');
        if (executable) {
          process.stdout.write(' at ' + executable);
        }
        process.stdout.write('\n');
      }).catch((error) => {
        process.stderr.write('error: headless chromium launch failed: ' + (error && error.message ? error.message : String(error)) + '\n');
        process.exit(1);
      });
    "
  )
}

cambridge_runtime_install_browsers() {
  local workflow_dir="$1"
  local timeout_seconds
  timeout_seconds="$(cambridge_runtime_install_timeout_seconds)"

  cambridge_runtime_run_with_timeout \
    "$timeout_seconds" \
    npx --prefix "$workflow_dir" playwright install chromium
}

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

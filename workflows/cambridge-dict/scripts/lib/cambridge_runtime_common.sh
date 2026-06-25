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

# Resolve the Playwright browser-download host. Honors the standard
# PLAYWRIGHT_DOWNLOAD_HOST override so a mirror still works.
cambridge_runtime_download_host() {
  printf '%s\n' "${PLAYWRIGHT_DOWNLOAD_HOST:-https://cdn.playwright.dev}"
}

# macOS Chrome-for-Testing platform suffix (mac-arm64 / mac-x64). Returns
# non-zero on any non-macOS host so callers fall back to Playwright's installer
# rather than guessing a Linux/Windows download path.
cambridge_runtime_macos_platform_suffix() {
  [[ "$(uname -s)" == "Darwin" ]] || return 1
  case "$(uname -m)" in
  arm64) printf '%s\n' "mac-arm64" ;;
  x86_64) printf '%s\n' "mac-x64" ;;
  *) return 1 ;;
  esac
}

# Build the Chrome-for-Testing CDN URL for a browser artifact, matching
# Playwright's own `builds/cft/<version>/<suffix>` scheme.
# args: <browser_version> <platform_suffix> <artifact_basename>
cambridge_runtime_browser_download_url() {
  local version="$1" platform="$2" artifact="$3"
  printf '%s/builds/cft/%s/%s/%s.zip\n' \
    "$(cambridge_runtime_download_host)" "$version" "$platform" "$artifact"
}

# Archive/extracted-directory basename for a browser + platform suffix. This is
# both the zip basename and the top-level directory inside the archive, so it
# becomes the build directory's single child after extraction.
# args: <browser_name> <platform_suffix>
cambridge_runtime_browser_artifact() {
  local browser="$1" platform="$2"
  case "$browser" in
  chromium) printf 'chrome-%s\n' "$platform" ;;
  chromium-headless-shell) printf 'chrome-headless-shell-%s\n' "$platform" ;;
  *) return 1 ;;
  esac
}

# Cache build-directory name for a browser + revision. Playwright maps dashes in
# the browser name to underscores (chromium-headless-shell -> chromium_headless_shell)
# and appends the revision (chromium-1208, chromium_headless_shell-1208).
# args: <browser_name> <revision>
cambridge_runtime_browser_cache_dir_name() {
  local browser="$1" revision="$2"
  printf '%s-%s\n' "${browser//-/_}" "$revision"
}

# Playwright's browser cache root. Honors PLAYWRIGHT_BROWSERS_PATH; defers
# (non-zero) when it is set to "0" (package-local browsers), which the native
# path cannot replicate, so the caller falls back to Playwright's installer.
cambridge_runtime_browsers_path() {
  local configured="${PLAYWRIGHT_BROWSERS_PATH:-}"
  if [[ "$configured" == "0" ]]; then
    return 1
  fi
  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi
  # The native install path is macOS-gated, so in the real flow only the Darwin
  # branch is reached. The Linux default is kept for cross-platform correctness
  # of this path helper (and the unit tests), not as a supported install target.
  if [[ "$(uname -s)" == "Darwin" ]]; then
    printf '%s/Library/Caches/ms-playwright\n' "$HOME"
  else
    printf '%s/.cache/ms-playwright\n' "$HOME"
  fi
}

# Extract a browser archive into a destination directory, preferring macOS-native
# ditto and falling back to unzip.
# args: <zip_path> <destination_dir>
cambridge_runtime_extract_browser_zip() {
  local zip="$1" dest="$2"
  mkdir -p "$dest"
  if command -v ditto >/dev/null 2>&1; then
    ditto -x -k "$zip" "$dest"
  elif command -v unzip >/dev/null 2>&1; then
    unzip -q -o "$zip" -d "$dest"
  else
    echo "error: no archive extractor (ditto/unzip) available" >&2
    return 1
  fi
}

# Download a URL to a destination path under a bounded timeout. --max-time caps
# the whole operation (including retries) so the call cannot wedge indefinitely
# the way Playwright's downloader was observed to; --connect-timeout fails a
# stalled TCP connect fast enough to leave budget for a retry.
# args: <url> <destination_path>
cambridge_runtime_download_file() {
  local url="$1" dest="$2"
  local timeout_seconds
  timeout_seconds="$(cambridge_runtime_install_timeout_seconds)"
  curl -fsSL --retry 2 --connect-timeout 30 --max-time "$timeout_seconds" -o "$dest" "$url"
}

# True when a browser build is already fully installed (completion marker
# present), so a re-run can skip the download.
# args: <cache_root> <cache_dir_name>
cambridge_runtime_browser_installed() {
  local cache_root="$1" cache_dir_name="$2"
  [[ -f "$cache_root/$cache_dir_name/INSTALLATION_COMPLETE" ]]
}

# Emit "<name>\t<revision>\t<browserVersion>" for each browser the workflow
# provisions, read from the installed playwright-core browsers.json. Only
# Chrome-for-Testing browsers with a browserVersion are emitted (ffmpeg and
# other non-CFT entries are skipped). Returns non-zero when the registry is
# missing or node is unavailable.
# args: <workflow_dir>
cambridge_runtime_browser_specs() {
  local workflow_dir="$1"
  local browsers_json="$workflow_dir/node_modules/playwright-core/browsers.json"
  [[ -f "$browsers_json" ]] || return 1
  command -v node >/dev/null 2>&1 || return 1
  # Single-quoted on purpose: the JS template literals (${browser.name}) must
  # reach node verbatim, not be expanded by the shell. browsers.json arrives via
  # process.argv[1].
  # shellcheck disable=SC2016
  node -e '
    const registry = require(process.argv[1]);
    const wanted = new Set(["chromium", "chromium-headless-shell"]);
    for (const browser of registry.browsers || []) {
      if (!wanted.has(browser.name)) continue;
      if (browser.installByDefault === false) continue;
      if (!browser.revision || !browser.browserVersion) continue;
      process.stdout.write(`${browser.name}\t${browser.revision}\t${browser.browserVersion}\n`);
    }
  ' "$browsers_json"
}

# Provision a single browser build via direct download + native extraction.
# Idempotent: a build with a completion marker is left untouched.
# args: <browser_name> <revision> <browser_version> <platform_suffix> <cache_root>
cambridge_runtime_install_browser_native() {
  local browser="$1" revision="$2" version="$3" platform="$4" cache_root="$5"
  local cache_dir_name dir artifact url tmp_dir tmp_zip

  cache_dir_name="$(cambridge_runtime_browser_cache_dir_name "$browser" "$revision")"
  dir="$cache_root/$cache_dir_name"

  if cambridge_runtime_browser_installed "$cache_root" "$cache_dir_name"; then
    return 0
  fi

  artifact="$(cambridge_runtime_browser_artifact "$browser" "$platform")" || return 1
  url="$(cambridge_runtime_browser_download_url "$version" "$platform" "$artifact")"

  tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-browser.XXXXXX")" || return 1
  tmp_zip="$tmp_dir/$artifact.zip"

  if ! cambridge_runtime_download_file "$url" "$tmp_zip"; then
    rm -rf "$tmp_dir"
    return 1
  fi

  # Extract into a clean build directory so a previous partial install never
  # leaks into the new one.
  rm -rf "$dir"
  if ! cambridge_runtime_extract_browser_zip "$tmp_zip" "$dir"; then
    rm -rf "$tmp_dir" "$dir"
    return 1
  fi
  rm -rf "$tmp_dir"

  # INSTALLATION_COMPLETE is Playwright's sole "is-installed" gate, so writing it
  # is enough for the build to launch without a re-download. The companion
  # DEPENDENCIES_VALIDATED marker is intentionally omitted: its only effect when
  # absent is a one-time host re-validation on launch, which is a no-op on macOS
  # (the sole supported native-install host).
  : >"$dir/INSTALLATION_COMPLETE"
  return 0
}

# Provision every required browser via the native path. Returns non-zero (so the
# caller falls back to Playwright's installer) when the host is unsupported, the
# tooling is missing, the registry cannot be read, or any browser fails.
# args: <workflow_dir>
cambridge_runtime_install_browsers_native() {
  local workflow_dir="$1"
  local platform cache_root specs name revision version

  platform="$(cambridge_runtime_macos_platform_suffix)" || return 1
  command -v curl >/dev/null 2>&1 || return 1
  { command -v ditto >/dev/null 2>&1 || command -v unzip >/dev/null 2>&1; } || return 1
  cache_root="$(cambridge_runtime_browsers_path)" || return 1

  specs="$(cambridge_runtime_browser_specs "$workflow_dir")" || return 1
  [[ -n "$specs" ]] || return 1

  while IFS=$'\t' read -r name revision version; do
    [[ -n "$name" && -n "$revision" && -n "$version" ]] || return 1
    cambridge_runtime_install_browser_native \
      "$name" "$revision" "$version" "$platform" "$cache_root" || return 1
  done <<<"$specs"

  return 0
}

cambridge_runtime_install_browsers() {
  local workflow_dir="$1"

  # Prefer a direct download + native (ditto/unzip) extraction. Playwright's
  # bundled out-of-process downloader extracts the Chromium archive through a
  # Node code path that can wedge indefinitely after a completed download on
  # some macOS hosts (observed: download reaches 100%, then the extract worker
  # sits at 0% CPU with the event loop idle and leaves a partial browser
  # behind). That makes the auto-bootstrap loop forever and is the root cause of
  # the endless "Installing Cambridge runtime..." reinstall cycle. Native
  # extraction of the identical archive is fast and bounded; fall back to the
  # Playwright installer only when the native path is unavailable.
  if cambridge_runtime_install_browsers_native "$workflow_dir"; then
    return 0
  fi

  local timeout_seconds
  timeout_seconds="$(cambridge_runtime_install_timeout_seconds)"

  cambridge_runtime_run_with_timeout \
    "$timeout_seconds" \
    npx --prefix "$workflow_dir" playwright install chromium
}

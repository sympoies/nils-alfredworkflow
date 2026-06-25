#!/usr/bin/env bash
set -euo pipefail

# Regression test for native Cambridge browser provisioning.
#
# Playwright's bundled out-of-process downloader extracts the Chromium archive
# through a Node code path that can wedge indefinitely after a completed
# download on some macOS hosts (observed: 0% CPU, event loop idle in kevent, a
# partial browser bundle left behind). That makes the auto-bootstrap loop
# forever and surfaces as the endless "Installing Cambridge runtime..."
# reinstall cycle. cambridge_runtime_install_browsers_native therefore
# provisions the browsers itself: it downloads the published Chrome-for-Testing
# archives and extracts them natively (ditto/unzip), which is fast and bounded;
# cambridge_runtime_install_browsers tries it first and falls back to the
# Playwright installer only when the native path is unavailable.
#
# These checks pin the URL/layout contract the native installer must satisfy so
# the resulting cache directory is exactly what Playwright expects to launch.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/workflows/cambridge-dict/scripts/lib/cambridge_runtime_common.sh"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

expect_eq() {
  local description="$1" expected="$2" actual="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "$description: expected '$expected', got '$actual'"
  fi
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-native-install.XXXXXX")"
cleanup() { rm -rf "$tmp_root"; }
trap cleanup EXIT

# --- pure URL / naming contract -------------------------------------------

expect_eq "default download host" \
  "https://cdn.playwright.dev" \
  "$(cambridge_runtime_download_host)"

expect_eq "download host honors PLAYWRIGHT_DOWNLOAD_HOST" \
  "https://mirror.example" \
  "$(PLAYWRIGHT_DOWNLOAD_HOST="https://mirror.example" cambridge_runtime_download_host)"

expect_eq "chromium cft url" \
  "https://cdn.playwright.dev/builds/cft/145.0.7632.6/mac-arm64/chrome-mac-arm64.zip" \
  "$(cambridge_runtime_browser_download_url "145.0.7632.6" "mac-arm64" "chrome-mac-arm64")"

expect_eq "headless-shell cft url honors mirror" \
  "https://mirror.example/builds/cft/145.0.7632.6/mac-arm64/chrome-headless-shell-mac-arm64.zip" \
  "$(PLAYWRIGHT_DOWNLOAD_HOST="https://mirror.example" cambridge_runtime_browser_download_url "145.0.7632.6" "mac-arm64" "chrome-headless-shell-mac-arm64")"

expect_eq "chromium cache dir name" \
  "chromium-1208" \
  "$(cambridge_runtime_browser_cache_dir_name "chromium" "1208")"

expect_eq "headless-shell cache dir name uses underscores" \
  "chromium_headless_shell-1208" \
  "$(cambridge_runtime_browser_cache_dir_name "chromium-headless-shell" "1208")"

expect_eq "chromium artifact (mac-arm64)" \
  "chrome-mac-arm64" \
  "$(cambridge_runtime_browser_artifact "chromium" "mac-arm64")"

expect_eq "chromium artifact (mac-x64)" \
  "chrome-mac-x64" \
  "$(cambridge_runtime_browser_artifact "chromium" "mac-x64")"

expect_eq "headless-shell artifact (mac-arm64)" \
  "chrome-headless-shell-mac-arm64" \
  "$(cambridge_runtime_browser_artifact "chromium-headless-shell" "mac-arm64")"

if cambridge_runtime_browser_artifact "totally-unknown" "mac-arm64" >/dev/null 2>&1; then
  fail "unknown browser artifact should fail"
fi

# --- browsers_path resolution ---------------------------------------------

expect_eq "browsers_path honors PLAYWRIGHT_BROWSERS_PATH" \
  "$tmp_root/custom-cache" \
  "$(PLAYWRIGHT_BROWSERS_PATH="$tmp_root/custom-cache" cambridge_runtime_browsers_path)"

if PLAYWRIGHT_BROWSERS_PATH="0" cambridge_runtime_browsers_path >/dev/null 2>&1; then
  fail "browsers_path must defer (non-zero) when PLAYWRIGHT_BROWSERS_PATH=0"
fi

# --- native extraction + install ------------------------------------------

command -v zip >/dev/null 2>&1 || fail "test prerequisite missing: zip"

# Build a fixture archive that mimics the Chrome-for-Testing layout: the zip
# top-level directory is the artifact basename, which extracts directly into
# the cache build directory.
fixture_src="$tmp_root/fixture/chrome-mac-arm64"
mkdir -p "$fixture_src"
printf 'dummy chromium binary\n' >"$fixture_src/Google Chrome for Testing"
chmod +x "$fixture_src/Google Chrome for Testing"
fixture_zip="$tmp_root/chrome-mac-arm64.zip"
(cd "$tmp_root/fixture" && zip -q -r "$fixture_zip" "chrome-mac-arm64")

extract_dest="$tmp_root/extract-dest"
cambridge_runtime_extract_browser_zip "$fixture_zip" "$extract_dest"
[[ -f "$extract_dest/chrome-mac-arm64/Google Chrome for Testing" ]] ||
  fail "extract_browser_zip did not produce the expected layout"

# A curl stub that records every invocation and writes the fixture archive to
# the requested -o target, so install_browser_native runs without the network.
stub_bin="$tmp_root/bin"
mkdir -p "$stub_bin"
call_log="$tmp_root/curl-calls.txt"
: >"$call_log"
cat >"$stub_bin/curl" <<STUB
#!/usr/bin/env bash
echo "\$@" >>"$call_log"
out=""
prev=""
for arg in "\$@"; do
  if [[ "\$prev" == "-o" ]]; then out="\$arg"; fi
  prev="\$arg"
done
[[ -n "\$out" ]] || { echo "stub curl: no -o target" >&2; exit 2; }
cp "$fixture_zip" "\$out"
STUB
chmod +x "$stub_bin/curl"

cache_root="$tmp_root/cache"
PATH="$stub_bin:$PATH" cambridge_runtime_install_browser_native \
  "chromium" "1208" "145.0.7632.6" "mac-arm64" "$cache_root" ||
  fail "install_browser_native failed on first install"

[[ -x "$cache_root/chromium-1208/chrome-mac-arm64/Google Chrome for Testing" ]] ||
  fail "install_browser_native did not place the browser binary"
[[ -f "$cache_root/chromium-1208/INSTALLATION_COMPLETE" ]] ||
  fail "install_browser_native did not write the INSTALLATION_COMPLETE marker"

calls_after_first="$(wc -l <"$call_log" | tr -d '[:space:]')"
expect_eq "first install downloads exactly once" "1" "$calls_after_first"

# The bounded-download contract is the anti-wedge guard: a missing --max-time
# would re-open the indefinite-hang class this fix exists to remove.
grep -q -- "--max-time" "$call_log" ||
  fail "download must pass curl --max-time to bound the transfer"
grep -q -- "--retry" "$call_log" ||
  fail "download must pass curl --retry"
grep -q -- "--connect-timeout" "$call_log" ||
  fail "download must pass curl --connect-timeout so a stalled connect fails fast"

# Second call must be idempotent: a completed install is reused without
# re-downloading.
PATH="$stub_bin:$PATH" cambridge_runtime_install_browser_native \
  "chromium" "1208" "145.0.7632.6" "mac-arm64" "$cache_root" ||
  fail "install_browser_native failed on idempotent re-run"
calls_after_second="$(wc -l <"$call_log" | tr -d '[:space:]')"
expect_eq "idempotent re-run does not re-download" "1" "$calls_after_second"

# A failed download must leave no build directory and no completion marker, so a
# partial install can never be mistaken for a finished one (the original bug).
fail_bin="$tmp_root/bin-fail"
mkdir -p "$fail_bin"
cat >"$fail_bin/curl" <<'STUB'
#!/usr/bin/env bash
exit 22
STUB
chmod +x "$fail_bin/curl"
fail_cache="$tmp_root/cache-fail"
if PATH="$fail_bin:$PATH" cambridge_runtime_install_browser_native \
  "chromium" "1208" "145.0.7632.6" "mac-arm64" "$fail_cache" 2>/dev/null; then
  fail "install_browser_native must fail when the download fails"
fi
[[ ! -e "$fail_cache/chromium-1208/INSTALLATION_COMPLETE" ]] ||
  fail "a failed download must not leave an INSTALLATION_COMPLETE marker"
[[ ! -d "$fail_cache/chromium-1208" ]] ||
  fail "a failed download must not leave a build directory"

# browsers.json spec resolution: only the Chrome-for-Testing browsers with a
# browserVersion are provisioned (firefox/ffmpeg and installByDefault:false are
# excluded), and the tab-separated shape the install loop reads is exact.
specs_wf="$tmp_root/spec-wf"
if command -v node >/dev/null 2>&1; then
  mkdir -p "$specs_wf/node_modules/playwright-core"
  cat >"$specs_wf/node_modules/playwright-core/browsers.json" <<'JSON'
{
  "browsers": [
    { "name": "chromium", "revision": "1208", "browserVersion": "145.0.7632.6", "installByDefault": true },
    { "name": "chromium-headless-shell", "revision": "1208", "browserVersion": "145.0.7632.6", "installByDefault": true },
    { "name": "firefox", "revision": "1490", "browserVersion": "100.0", "installByDefault": true },
    { "name": "ffmpeg", "revision": "1011", "installByDefault": true },
    { "name": "chromium", "revision": "9999", "browserVersion": "999.0.0.0", "installByDefault": false }
  ]
}
JSON
  expected_specs="$(printf 'chromium\t1208\t145.0.7632.6\nchromium-headless-shell\t1208\t145.0.7632.6')"
  expect_eq "browser_specs emits only versioned CFT browsers" \
    "$expected_specs" \
    "$(cambridge_runtime_browser_specs "$specs_wf")"
else
  printf 'note: skipping browser_specs check (node unavailable)\n'
fi

# Orchestrator fallback: when the native path defers (PLAYWRIGHT_BROWSERS_PATH=0),
# cambridge_runtime_install_browsers must fall back to the Playwright installer.
fallback_bin="$tmp_root/bin-fallback"
mkdir -p "$fallback_bin"
npx_log="$tmp_root/npx-calls.txt"
: >"$npx_log"
cat >"$fallback_bin/npx" <<STUB
#!/usr/bin/env bash
echo "\$@" >>"$npx_log"
STUB
chmod +x "$fallback_bin/npx"
PATH="$fallback_bin:$PATH" PLAYWRIGHT_BROWSERS_PATH="0" \
  cambridge_runtime_install_browsers "$tmp_root/fallback-wf" ||
  fail "install_browsers should succeed via the Playwright fallback"
grep -q "playwright install chromium" "$npx_log" ||
  fail "install_browsers must fall back to 'playwright install chromium' when the native path defers"

# Native orchestrator (macOS only): provisions every required browser end to end.
# Off-Darwin the native path is intentionally gated, so this asserts only where
# the path is actually supported.
if [[ "$(uname -s)" == "Darwin" ]] && command -v node >/dev/null 2>&1; then
  native_cache="$tmp_root/cache-native"
  PATH="$stub_bin:$PATH" PLAYWRIGHT_BROWSERS_PATH="$native_cache" \
    cambridge_runtime_install_browsers_native "$specs_wf" ||
    fail "install_browsers_native should provision all browsers on macOS"
  [[ -f "$native_cache/chromium-1208/INSTALLATION_COMPLETE" ]] ||
    fail "native orchestrator did not provision chromium"
  [[ -f "$native_cache/chromium_headless_shell-1208/INSTALLATION_COMPLETE" ]] ||
    fail "native orchestrator did not provision the headless shell"
fi

printf 'ok: cambridge runtime native-install tests passed\n'

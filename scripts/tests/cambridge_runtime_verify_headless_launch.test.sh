#!/usr/bin/env bash
set -euo pipefail

# Regression test for the Cambridge runtime headless-launch verification.
#
# The readiness check launches headless Chromium and must report success when
# Chromium is actually usable. A real Playwright `Browser` (returned by
# `chromium.launch()`) does NOT expose `process()` -- that method lives only on
# `BrowserServer`/`ElectronApplication`. Calling `browser.process()` therefore
# throws `browser.process is not a function` after a successful launch and makes
# the check report the runtime as unavailable even though Chromium is fine.
#
# This test stubs `playwright` with a module whose `chromium.launch()` resolves
# to a process-less `Browser`, mirroring the real API, and runs the verification
# through real `node`. It fails if the verification calls a method absent from
# the Playwright `Browser` surface.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/workflows/cambridge-dict/scripts/lib/cambridge_runtime_common.sh"

command -v node >/dev/null 2>&1 || {
  printf 'FAIL: node is required to run this test\n' >&2
  exit 1
}

test_root="$(mktemp -d "${TMPDIR:-/tmp}/cambridge-runtime-verify.test.XXXXXX")"
workflow_dir="$test_root/workflow"
playwright_dir="$workflow_dir/node_modules/playwright"

trap 'rm -rf "$test_root"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

mkdir -p "$playwright_dir"

cat >"$playwright_dir/package.json" <<'JSON'
{
  "name": "playwright",
  "version": "1.58.2-stub",
  "type": "module",
  "exports": {
    ".": "./index.mjs"
  }
}
JSON

# Stub mirrors the real Playwright surface: chromium.launch() resolves to a
# Browser with close()/contexts() but NO process() (process() exists only on
# BrowserServer/ElectronApplication).
cat >"$playwright_dir/index.mjs" <<'JS'
export const chromium = {
  async launch() {
    return {
      async close() {},
      contexts() {
        return [];
      },
    };
  },
  executablePath() {
    return '/stub/path/to/headless-chromium';
  },
};
JS

output_file="$test_root/verify.out"
if ! cambridge_runtime_verify_headless_chromium_launch "$workflow_dir" >"$output_file" 2>&1; then
  printf '%s\n' '--- verification output ---' >&2
  cat "$output_file" >&2
  fail "headless launch verification must succeed against a process-less Playwright Browser"
fi

if ! grep -q 'ok: headless chromium launched' "$output_file"; then
  printf '%s\n' '--- verification output ---' >&2
  cat "$output_file" >&2
  fail "verification must emit the headless launch success marker"
fi

if grep -qi 'is not a function' "$output_file"; then
  printf '%s\n' '--- verification output ---' >&2
  cat "$output_file" >&2
  fail "verification must not call methods absent from the Playwright Browser surface"
fi

printf 'ok: cambridge runtime headless-launch verification tests passed\n'

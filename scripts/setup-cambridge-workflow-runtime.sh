#!/usr/bin/env bash
set -euo pipefail

bundle_id="com.sympoies.cambridge-dict"
workflow_dir=""
check_only=0
install_browser=1
wait_for_install=0
quiet=0
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/workflows/cambridge-dict/scripts/lib/cambridge_runtime_common.sh"

usage() {
  cat <<'USAGE'
Usage:
  scripts/setup-cambridge-workflow-runtime.sh [--workflow-dir <path>] [--check-only] [--skip-browser] [--wait-for-install] [--quiet]

Options:
  --workflow-dir <path>  Target an explicit installed Alfred workflow directory.
  --check-only           Skip npm install and only verify runtime availability.
  --skip-browser         Do not run 'playwright install chromium'.
  --wait-for-install     Wait for installed workflow discovery (use after --install pack).
  --quiet                Minimize non-error logs.
USAGE
}

log() {
  if [[ "$quiet" -eq 0 ]]; then
    echo "$@"
  fi
}

find_installed_workflow_dir() {
  local prefs_root="${ALFRED_PREFS_ROOT:-$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows}"
  [[ -d "$prefs_root" ]] || return 1
  command -v plutil >/dev/null 2>&1 || return 1

  local info bid
  for info in "$prefs_root"/*/info.plist; do
    [[ -f "$info" ]] || continue
    bid="$(plutil -extract bundleid raw -o - "$info" 2>/dev/null || true)"
    if [[ "$bid" == "$bundle_id" ]]; then
      dirname "$info"
      return 0
    fi
  done

  return 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --workflow-dir)
    workflow_dir="${2:-}"
    [[ -n "$workflow_dir" ]] || {
      echo "error: --workflow-dir requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --check-only)
    check_only=1
    shift
    ;;
  --skip-browser)
    install_browser=0
    shift
    ;;
  --wait-for-install)
    wait_for_install=1
    shift
    ;;
  --quiet)
    quiet=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown argument: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done

if [[ -z "$workflow_dir" ]]; then
  if [[ "$wait_for_install" -eq 1 ]]; then
    for _ in $(seq 1 30); do
      if workflow_dir="$(find_installed_workflow_dir)"; then
        break
      fi
      sleep 1
    done
  fi

  if [[ -z "$workflow_dir" ]]; then
    workflow_dir="$(find_installed_workflow_dir || true)"
  fi
fi

[[ -n "$workflow_dir" && -d "$workflow_dir" ]] || {
  echo "error: Cambridge Dict installed workflow directory not found" >&2
  exit 1
}

command -v node >/dev/null 2>&1 || {
  echo "error: node is required" >&2
  exit 1
}

command -v npm >/dev/null 2>&1 || {
  echo "error: npm is required" >&2
  exit 1
}

expected_playwright_version="$(cambridge_runtime_playwright_version)"
installed_playwright_version=""
if [[ -f "$workflow_dir/package.json" ]]; then
  installed_playwright_version="$(node -e "try { const pkg = require(process.argv[1]); process.stdout.write(String(pkg.dependencies?.playwright || '')); } catch (_) {}" "$workflow_dir/package.json")"
fi

if [[ "$installed_playwright_version" != "$expected_playwright_version" ]]; then
  cambridge_runtime_write_package_json "$workflow_dir"
  log "info: wrote $workflow_dir/package.json"
fi

if [[ "$check_only" -eq 0 ]]; then
  npm --prefix "$workflow_dir" install --omit=dev --no-audit --no-fund
fi

cambridge_runtime_verify_playwright_package "$workflow_dir"

if [[ "$check_only" -eq 1 ]]; then
  if [[ "$install_browser" -eq 1 ]]; then
    if ! cambridge_runtime_verify_headless_chromium_launch "$workflow_dir"; then
      echo "error: headless chromium runtime unavailable; run scripts/setup-cambridge-workflow-runtime.sh to install" >&2
      exit 1
    fi
  else
    log "info: browser presence check skipped (--skip-browser)"
  fi
  log "ok: cambridge runtime check passed at $workflow_dir"
  exit 0
fi

if [[ "$install_browser" -eq 1 ]]; then
  command -v npx >/dev/null 2>&1 || {
    echo "error: npx is required" >&2
    exit 1
  }
  cambridge_runtime_install_browsers "$workflow_dir"
  if ! cambridge_runtime_verify_headless_chromium_launch "$workflow_dir"; then
    echo "error: headless chromium runtime still unavailable after 'playwright install chromium'" >&2
    exit 1
  fi
  log "ok: playwright chromium installed"
else
  log "info: browser install skipped (--skip-browser)"
fi

log "ok: cambridge runtime ready at $workflow_dir"

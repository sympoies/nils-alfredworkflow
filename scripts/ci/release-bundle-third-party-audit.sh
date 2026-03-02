#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

tag=""
dist_dir=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/release-bundle-third-party-audit.sh --tag <release-tag> --dist-dir <release-bundle-dir>

Options:
  --tag       Release tag used to compute expected bundle file names (example: v1.2.3).
  --dist-dir  Release bundle directory containing built assets (example: dist/release-bundles).
  -h, --help  Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --tag)
    tag="${2:-}"
    [[ -n "$tag" ]] || {
      echo "error: --tag requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --dist-dir)
    dist_dir="${2:-}"
    [[ -n "$dist_dir" ]] || {
      echo "error: --dist-dir requires a value" >&2
      exit 2
    }
    shift 2
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

[[ -n "$tag" ]] || {
  echo "error: --tag is required" >&2
  usage >&2
  exit 2
}

[[ -n "$dist_dir" ]] || {
  echo "error: --dist-dir is required" >&2
  usage >&2
  exit 2
}

if [[ "$dist_dir" != /* ]]; then
  dist_dir="$repo_root/$dist_dir"
fi

failures=0

audit_pass() {
  local message="$1"
  printf 'PASS [release-bundle-third-party] %s\n' "$message"
}

audit_fail() {
  local message="$1"
  printf 'FAIL [release-bundle-third-party] %s\n' "$message"
  failures=$((failures + 1))
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ -f "$path" ]]; then
    audit_pass "$label present: $path"
  else
    audit_fail "$label missing: $path"
  fi
}

echo "== Release bundle third-party audit =="
echo "tag: $tag"
echo "dist-dir: $dist_dir"

bundle_zip="$dist_dir/workflows-${tag}.zip"
bundle_sha="$bundle_zip.sha256"
standalone_script="$dist_dir/workflow-clear-quarantine-standalone.sh"
standalone_sha="$standalone_script.sha256"
license_file="$dist_dir/THIRD_PARTY_LICENSES.md"
license_sha="$license_file.sha256"

require_file "$bundle_zip" "workflow release bundle archive"
require_file "$bundle_sha" "workflow release bundle checksum"
require_file "$standalone_script" "standalone quarantine helper"
require_file "$standalone_sha" "standalone quarantine helper checksum"
require_file "$license_file" "third-party license artifact"
require_file "$license_sha" "third-party license artifact checksum"

if [[ -f "$repo_root/THIRD_PARTY_LICENSES.md" && -f "$license_file" ]]; then
  if cmp -s "$repo_root/THIRD_PARTY_LICENSES.md" "$license_file"; then
    audit_pass "release license artifact matches repository THIRD_PARTY_LICENSES.md"
  else
    audit_fail "release license artifact does not match repository THIRD_PARTY_LICENSES.md"
  fi
fi

echo
printf 'Summary: failures=%d\n' "$failures"
if [[ "$failures" -gt 0 ]]; then
  echo "Result: FAIL (release bundle third-party compliance issues detected)"
  exit 1
fi

echo "Result: PASS"

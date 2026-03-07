#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
workflow_catalog_lib="$repo_root/scripts/lib/workflow_catalog.sh"

prefs_root_default="$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows"
prefs_root="${ALFRED_PREFS_ROOT:-$prefs_root_default}"

declare -a requested_ids=()

[[ -f "$workflow_catalog_lib" ]] || {
  echo "error: missing helper library: $workflow_catalog_lib" >&2
  exit 1
}
# shellcheck disable=SC1090
source "$workflow_catalog_lib"

usage() {
  cat <<USAGE
Usage:
  scripts/workflow-clear-quarantine.sh [--all]
  scripts/workflow-clear-quarantine.sh --id <workflow-id> [--id <workflow-id> ...]

Behavior:
  - Clears macOS Gatekeeper quarantine recursively on installed Alfred workflows.
  - Resolves installed workflow directories by bundle id from workflows/<id>/workflow.toml.
  - Skips workflow ids that are not installed in Alfred (non-fatal).

Options:
  --all                 Target all tracked workflows (default when no --id is provided).
  --id <workflow-id>    Target one workflow id (can repeat).
  -h, --help            Show this help.

Environment:
  ALFRED_PREFS_ROOT     Override Alfred workflows directory.
USAGE
}

has_workflow_manifest() {
  local id="$1"
  [[ -f "$(wfc_manifest_path "$repo_root" "$id")" ]]
}

add_target_id() {
  local id="$1"
  local existing
  for existing in "${requested_ids[@]:-}"; do
    if [[ "$existing" == "$id" ]]; then
      return 0
    fi
  done
  requested_ids+=("$id")
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --id)
    [[ -n "${2:-}" ]] || {
      echo "error: --id requires a value" >&2
      exit 2
    }
    add_target_id "$2"
    shift 2
    ;;
  --all)
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

if [[ "$(uname -s 2>/dev/null || printf '')" != "Darwin" ]]; then
  echo "skip: workflow-clear-quarantine is macOS-only"
  exit 0
fi

if ! command -v plutil >/dev/null 2>&1; then
  echo "warn: plutil not found; cannot resolve installed workflows"
  exit 0
fi

if ! command -v xattr >/dev/null 2>&1; then
  echo "warn: xattr not found; cannot clear quarantine"
  exit 0
fi

if [[ ! -d "$prefs_root" ]]; then
  echo "warn: Alfred workflows directory not found: $prefs_root"
  exit 0
fi

if [[ "${#requested_ids[@]}" -eq 0 ]]; then
  while IFS= read -r id; do
    [[ -n "$id" ]] || continue
    requested_ids+=("$id")
  done < <(wfc_list_workflow_ids "$repo_root")
fi

cleared_count=0
skip_count=0
fail_count=0

for id in "${requested_ids[@]}"; do
  if ! has_workflow_manifest "$id"; then
    echo "warn: unknown workflow id (missing manifest): $id"
    fail_count=$((fail_count + 1))
    continue
  fi

  manifest="$(wfc_manifest_path "$repo_root" "$id")"
  bundle_id="$(wfc_bundle_id_for_workflow_id "$repo_root" "$id" || true)"
  if [[ -z "$bundle_id" ]]; then
    echo "warn: missing bundle_id in $manifest"
    fail_count=$((fail_count + 1))
    continue
  fi

  workflow_dir="$(wfc_find_installed_workflow_dir_by_bundle_id "$prefs_root" "$bundle_id" || true)"
  if [[ -z "$workflow_dir" ]]; then
    echo "skip: not installed ($id, $bundle_id)"
    skip_count=$((skip_count + 1))
    continue
  fi

  if xattr -dr com.apple.quarantine "$workflow_dir" >/dev/null 2>&1; then
    echo "ok: removed quarantine ($id -> $workflow_dir)"
    cleared_count=$((cleared_count + 1))
  else
    echo "warn: failed to clear quarantine ($id -> $workflow_dir)"
    fail_count=$((fail_count + 1))
  fi
done

echo "summary: cleared=$cleared_count skipped=$skip_count failed=$fail_count"

if [[ "$fail_count" -gt 0 ]]; then
  exit 1
fi

#!/usr/bin/env bash
set -euo pipefail

prefs_root_default="$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows"
prefs_root="${ALFRED_PREFS_ROOT:-$prefs_root_default}"

all_mode=0
list_mode=0

declare -a target_bundle_ids=()
declare -a target_labels=()

usage() {
  cat <<USAGE
Usage:
  workflow-clear-quarantine-standalone.sh [--all]
  workflow-clear-quarantine-standalone.sh --id <workflow-id> [--id <workflow-id> ...]
  workflow-clear-quarantine-standalone.sh --bundle-id <bundle-id> [--bundle-id <bundle-id> ...]

Behavior:
  - Clears macOS Gatekeeper quarantine recursively on installed Alfred workflows.
  - Works as a standalone script from GitHub Release assets (no repository checkout needed).
  - Missing/non-installed workflows are skipped (non-fatal).

Options:
  --all                    Target all known nils-alfredworkflow workflow ids.
  --id <workflow-id>       Target one known workflow id (repeatable).
  --bundle-id <bundle-id>  Target one explicit bundle id directly (repeatable).
  --list                   Print known workflow ids and exit.
  -h, --help               Show this help.

Environment:
  ALFRED_PREFS_ROOT        Override Alfred workflows directory.
USAGE
}

known_workflow_ids() {
  cat <<'EOF_IDS'
bangumi-search
bilibili-search
cambridge-dict
codex-cli
epoch-converter
google-search
imdb-search
market-expression
multi-timezone
netflix-search
open-project
quote-feed
randomer
spotify-search
weather
wiki-search
youtube-search
EOF_IDS
}

bundle_id_for_workflow_id() {
  case "${1:-}" in
  bangumi-search)
    printf '%s\n' 'com.sympoies.bangumi-search'
    ;;
  bilibili-search)
    printf '%s\n' 'com.sympoies.bilibili-search'
    ;;
  cambridge-dict)
    printf '%s\n' 'com.sympoies.cambridge-dict'
    ;;
  codex-cli)
    printf '%s\n' 'com.sympoies.codex-cli'
    ;;
  epoch-converter)
    printf '%s\n' 'com.sympoies.epoch-converter'
    ;;
  google-search)
    printf '%s\n' 'com.sympoies.google-search'
    ;;
  imdb-search)
    printf '%s\n' 'com.sympoies.imdb-search'
    ;;
  market-expression)
    printf '%s\n' 'com.sympoies.market-expression'
    ;;
  multi-timezone)
    printf '%s\n' 'com.sympoies.multi-timezone'
    ;;
  netflix-search)
    printf '%s\n' 'com.sympoies.netflix-search'
    ;;
  open-project)
    printf '%s\n' 'com.sympoies.open-project'
    ;;
  quote-feed)
    printf '%s\n' 'com.sympoies.quote-feed'
    ;;
  randomer)
    printf '%s\n' 'com.sympoies.randomer'
    ;;
  spotify-search)
    printf '%s\n' 'com.sympoies.spotify-search'
    ;;
  weather)
    printf '%s\n' 'com.sympoies.weather'
    ;;
  wiki-search)
    printf '%s\n' 'com.sympoies.wiki-search'
    ;;
  youtube-search)
    printf '%s\n' 'com.sympoies.youtube-search'
    ;;
  *)
    return 1
    ;;
  esac
}

has_target_bundle_id() {
  bundle_id="${1:-}"
  for existing in "${target_bundle_ids[@]:-}"; do
    if [[ "$existing" == "$bundle_id" ]]; then
      return 0
    fi
  done
  return 1
}

add_target_bundle() {
  bundle_id="${1:-}"
  label="${2:-}"
  if has_target_bundle_id "$bundle_id"; then
    return 0
  fi
  target_bundle_ids+=("$bundle_id")
  target_labels+=("$label")
}

add_target_workflow_id() {
  workflow_id="${1:-}"
  bundle_id="$(bundle_id_for_workflow_id "$workflow_id" || true)"
  if [[ -z "$bundle_id" ]]; then
    echo "error: unknown workflow id: $workflow_id" >&2
    echo "hint: run --list to see supported ids" >&2
    exit 2
  fi
  add_target_bundle "$bundle_id" "$workflow_id"
}

find_installed_workflow_dir_by_bundle_id() {
  bundle_id="$1"

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
  --all)
    all_mode=1
    shift
    ;;
  --id)
    [[ -n "${2:-}" ]] || {
      echo "error: --id requires a value" >&2
      exit 2
    }
    add_target_workflow_id "$2"
    shift 2
    ;;
  --bundle-id)
    [[ -n "${2:-}" ]] || {
      echo "error: --bundle-id requires a value" >&2
      exit 2
    }
    add_target_bundle "$2" "$2"
    shift 2
    ;;
  --list)
    list_mode=1
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

if [[ "$list_mode" -eq 1 ]]; then
  known_workflow_ids
  exit 0
fi

if [[ "$(uname -s 2>/dev/null || printf '')" != "Darwin" ]]; then
  echo "skip: workflow-clear-quarantine-standalone is macOS-only"
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

if [[ "$all_mode" -eq 1 || "${#target_bundle_ids[@]}" -eq 0 ]]; then
  while IFS= read -r workflow_id; do
    [[ -n "$workflow_id" ]] || continue
    add_target_workflow_id "$workflow_id"
  done < <(known_workflow_ids)
fi

cleared_count=0
skip_count=0
fail_count=0

for i in "${!target_bundle_ids[@]}"; do
  bundle_id="${target_bundle_ids[$i]}"
  label="${target_labels[$i]}"

  workflow_dir="$(find_installed_workflow_dir_by_bundle_id "$bundle_id" || true)"
  if [[ -z "$workflow_dir" ]]; then
    echo "skip: not installed ($label, $bundle_id)"
    skip_count=$((skip_count + 1))
    continue
  fi

  if xattr -dr com.apple.quarantine "$workflow_dir" >/dev/null 2>&1; then
    echo "ok: removed quarantine ($label -> $workflow_dir)"
    cleared_count=$((cleared_count + 1))
  else
    echo "warn: failed to clear quarantine ($label -> $workflow_dir)"
    fail_count=$((fail_count + 1))
  fi
done

echo "summary: cleared=$cleared_count skipped=$skip_count failed=$fail_count"

if [[ "$fail_count" -gt 0 ]]; then
  exit 1
fi

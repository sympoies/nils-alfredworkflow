#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STEAM_REQUERY_PREFIX="steam-requery:"

validate_region_code() {
  local value="${1:-}"
  [[ "$value" =~ ^[A-Za-z]{2}$ ]]
}

normalize_region_code() {
  printf '%s' "${1:-}" | tr '[:lower:]' '[:upper:]'
}

steam_override_state_file() {
  local cache_dir
  cache_dir="$(wfar_resolve_cache_dir "nils-steam-search-workflow")"
  wfar_state_file_path "$cache_dir" "steam-region-override.state"
}

dispatch_requery_payload() {
  local arg="$1"

  if ! wfar_parse_requery_payload "$arg" "$STEAM_REQUERY_PREFIX"; then
    echo "usage: action_open.sh steam-requery:<region>:<query>" >&2
    exit 2
  fi

  local region query normalized_region keyword requery_text
  region="${WFAR_REQUERY_SELECTOR:-}"
  query="${WFAR_REQUERY_QUERY:-}"

  if ! validate_region_code "$region"; then
    echo "invalid requery region: $region" >&2
    exit 2
  fi

  normalized_region="$(normalize_region_code "$region")"
  wfar_write_state_file "$(steam_override_state_file)" "$normalized_region"

  keyword="${STEAM_KEYWORD:-st}"
  requery_text="$(wfar_build_keyword_requery_text "$keyword" "$query")"

  if ! wfar_trigger_requery "$requery_text" "${STEAM_REQUERY_COMMAND:-}" "${STEAM_ALFRED_APP_NAME:-Alfred 5}"; then
    exit 1
  fi
}

if [[ $# -lt 1 || -z "${1:-}" ]]; then
  echo "usage: action_open.sh <url|steam-requery:region:query>" >&2
  exit 2
fi

helper_loader=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -z "$helper_loader" ]]; then
  echo "Workflow helper missing: Cannot locate workflow_helper_loader.sh runtime helper." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "workflow_action_requery.sh" off; then
  wfhl_print_missing_helper_stderr "workflow_action_requery.sh"
  exit 1
fi

if [[ "$1" == "$STEAM_REQUERY_PREFIX"* ]]; then
  dispatch_requery_payload "$1"
  exit 0
fi

helper="$(wfhl_resolve_helper_path "$script_dir" "workflow_action_open_url.sh" off || true)"
if [[ -z "$helper" ]]; then
  wfhl_print_missing_helper_stderr "workflow_action_open_url.sh"
  exit 1
fi

exec "$helper" "$@"

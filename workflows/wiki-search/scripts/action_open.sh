#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

WIKI_REQUERY_PREFIX="wiki-requery:"

validate_language_code() {
  local value="${1:-}"
  [[ "$value" =~ ^[a-z]{2,12}$ ]]
}

wiki_override_state_file() {
  local cache_dir
  cache_dir="$(wfar_resolve_cache_dir "nils-wiki-search-workflow")"
  wfar_state_file_path "$cache_dir" "wiki-language-override.state"
}

dispatch_requery_payload() {
  local arg="$1"

  if ! wfar_parse_requery_payload "$arg" "$WIKI_REQUERY_PREFIX"; then
    echo "usage: action_open.sh wiki-requery:<language>:<query>" >&2
    exit 2
  fi

  local language query
  language="${WFAR_REQUERY_SELECTOR:-}"
  query="${WFAR_REQUERY_QUERY:-}"

  if ! validate_language_code "$language"; then
    echo "invalid requery language: $language" >&2
    exit 2
  fi

  wfar_write_state_file "$(wiki_override_state_file)" "$language"

  local keyword requery_text
  keyword="${WIKI_KEYWORD:-wk}"
  requery_text="$(wfar_build_keyword_requery_text "$keyword" "$query")"

  if ! wfar_trigger_requery "$requery_text" "${WIKI_REQUERY_COMMAND:-}" "${WIKI_ALFRED_APP_NAME:-Alfred 5}"; then
    exit 1
  fi
}

if [[ $# -lt 1 || -z "${1:-}" ]]; then
  echo "usage: action_open.sh <url|wiki-requery:language:query>" >&2
  exit 2
fi

loader_path=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    loader_path="$candidate"
    break
  fi
done

if [[ -z "$loader_path" ]]; then
  echo "Workflow helper missing: Cannot locate workflow_helper_loader.sh runtime helper." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$loader_path"

if ! wfhl_source_helper "$script_dir" "workflow_action_requery.sh" off; then
  wfhl_print_missing_helper_stderr "workflow_action_requery.sh"
  exit 1
fi

if [[ "$1" == "$WIKI_REQUERY_PREFIX"* ]]; then
  dispatch_requery_payload "$1"
  exit 0
fi

helper="$(wfhl_resolve_helper_path "$script_dir" "workflow_action_open_url.sh" off || true)"
if [[ -z "$helper" ]]; then
  wfhl_print_missing_helper_stderr "workflow_action_open_url.sh"
  exit 1
fi

exec "$helper" "$@"

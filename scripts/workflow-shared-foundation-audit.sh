#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
mode="check"

usage() {
  cat <<'USAGE'
Usage:
  scripts/workflow-shared-foundation-audit.sh --check
USAGE
}

require_bin() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    echo "error: missing required binary: $name" >&2
    exit 1
  }
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --check)
    mode="check"
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

if [[ "$mode" != "check" ]]; then
  echo "error: unsupported mode: $mode" >&2
  exit 2
fi

require_bin rg

declare -ar migrated_action_wrappers=(
  "workflows/bangumi-search/scripts/action_open.sh"
  "workflows/bilibili-search/scripts/action_open.sh"
  "workflows/cambridge-dict/scripts/action_open.sh"
  "workflows/epoch-converter/scripts/action_copy.sh"
  "workflows/google-search/scripts/action_open.sh"
  "workflows/imdb-search/scripts/action_open.sh"
  "workflows/market-expression/scripts/action_copy.sh"
  "workflows/multi-timezone/scripts/action_copy.sh"
  "workflows/netflix-search/scripts/action_open.sh"
  "workflows/steam-search/scripts/action_open.sh"
  "workflows/weather/scripts/action_copy.sh"
  "workflows/wiki-search/scripts/action_open.sh"
  "workflows/youtube-search/scripts/action_open.sh"
)

declare -ar migrated_search_filters=(
  "workflows/bangumi-search/scripts/script_filter.sh"
  "workflows/cambridge-dict/scripts/script_filter.sh"
  "workflows/google-search/scripts/script_filter.sh"
  "workflows/netflix-search/scripts/script_filter.sh"
  "workflows/spotify-search/scripts/script_filter.sh"
  "workflows/steam-search/scripts/script_filter.sh"
  "workflows/wiki-search/scripts/script_filter.sh"
  "workflows/youtube-search/scripts/script_filter.sh"
)

declare -ar migrated_non_search_filters=(
  "workflows/bilibili-search/scripts/script_filter.sh"
  "workflows/epoch-converter/scripts/script_filter.sh"
  "workflows/imdb-search/scripts/script_filter.sh"
  "workflows/market-expression/scripts/script_filter.sh"
  "workflows/memo-add/scripts/script_filter.sh"
  "workflows/multi-timezone/scripts/script_filter.sh"
  "workflows/open-project/scripts/script_filter.sh"
  "workflows/quote-feed/scripts/script_filter.sh"
  "workflows/randomer/scripts/script_filter.sh"
  "workflows/randomer/scripts/script_filter_expand.sh"
  "workflows/randomer/scripts/script_filter_types.sh"
)

declare -ar migrated_additional_foundation_files=(
  "workflows/bangumi-search/scripts/action_clear_cache.sh"
  "workflows/bangumi-search/scripts/action_clear_cache_dir.sh"
  "workflows/codex-cli/scripts/action_open.sh"
  "workflows/codex-cli/scripts/script_filter.sh"
  "workflows/codex-cli/scripts/script_filter_auth_current.sh"
  "workflows/google-search/scripts/script_filter_direct.sh"
  "workflows/memo-add/scripts/action_run.sh"
  "workflows/memo-add/scripts/script_filter_copy.sh"
  "workflows/memo-add/scripts/script_filter_delete.sh"
  "workflows/memo-add/scripts/script_filter_recent.sh"
  "workflows/memo-add/scripts/script_filter_search.sh"
  "workflows/memo-add/scripts/script_filter_update.sh"
  "workflows/open-project/scripts/action_open.sh"
  "workflows/open-project/scripts/action_open_github.sh"
  "workflows/open-project/scripts/action_record_usage.sh"
  "workflows/weather/scripts/script_filter_common.sh"
)

declare -a migrated_files=(
  "${migrated_action_wrappers[@]}"
  "${migrated_search_filters[@]}"
  "${migrated_non_search_filters[@]}"
  "${migrated_additional_foundation_files[@]}"
)

declare -ir total_migrated_files="${#migrated_files[@]}"
failures=0

pass_check() {
  local label="$1"
  printf 'PASS [check] %s\n' "$label"
}

fail_file() {
  local rel_path="$1"
  local message="$2"
  printf 'FAIL [%s] %s\n' "$rel_path" "$message" >&2
  failures=$((failures + 1))
}

file_exists_or_fail() {
  local rel_path="$1"
  local abs_path="$repo_root/$rel_path"
  if [[ -f "$abs_path" ]]; then
    return 0
  fi

  fail_file "$rel_path" "migrated file is missing"
  return 1
}

run_check_resolve_helper_regression() {
  local rel_path=""
  local abs_path=""
  local initial_failures="$failures"

  for rel_path in "${migrated_files[@]}"; do
    abs_path="$repo_root/$rel_path"
    file_exists_or_fail "$rel_path" || continue
    if rg -n --max-count 1 '^[[:space:]]*resolve_helper[[:space:]]*\(\)' "$abs_path" >/dev/null; then
      fail_file "$rel_path" "reintroduced duplicate resolve_helper() block; migrate to shared loader primitives"
    fi
  done

  if [[ "$failures" -eq "$initial_failures" ]]; then
    pass_check "migrated files do not define duplicate resolve_helper() blocks"
  fi
}

run_check_shared_loader_wiring() {
  local rel_path=""
  local abs_path=""
  local initial_failures="$failures"

  for rel_path in "${migrated_files[@]}"; do
    abs_path="$repo_root/$rel_path"
    file_exists_or_fail "$rel_path" || continue
    if ! rg -n --max-count 1 'workflow_helper_loader|wfhl_source_helper[[:space:]]*\(' "$abs_path" >/dev/null; then
      fail_file "$rel_path" "missing shared foundation wiring; expected workflow_helper_loader or wfhl_source_helper()"
    fi
  done

  if [[ "$failures" -eq "$initial_failures" ]]; then
    pass_check "all migrated files are wired to shared helper loader foundations"
  fi
}

run_check_non_search_driver_usage() {
  local rel_path=""
  local abs_path=""
  local initial_failures="$failures"

  for rel_path in "${migrated_non_search_filters[@]}"; do
    abs_path="$repo_root/$rel_path"
    file_exists_or_fail "$rel_path" || continue
    if ! rg -n --max-count 1 '^[[:space:]]*sfcd_run_cli_flow([[:space:]]|$)' "$abs_path" >/dev/null; then
      fail_file "$rel_path" "missing shared CLI driver call sfcd_run_cli_flow() for non-search script filter"
    fi
  done

  if [[ "$failures" -eq "$initial_failures" ]]; then
    pass_check "all migrated non-search filters call sfcd_run_cli_flow()"
  fi
}

run_check_prohibited_placeholders() {
  local rel_path=""
  local abs_path=""
  local initial_failures="$failures"
  local spec=""
  local label=""
  local regex=""
  local match=""

  declare -ar prohibited_specs=(
    "TODO(shared-foundation) marker|TODO[[:space:]]*\\([[:space:]]*shared[-_ ]foundation[[:space:]]*\\)"
    "shared-foundation placeholder marker|(shared[-_ ]foundation[[:space:]]*(placeholder|stub)|placeholder[[:space:]]*\\([[:space:]]*shared[-_ ]foundation[[:space:]]*\\))"
    "no-op scaffolding marker|#[[:space:]]*(no-?op|noop)[[:space:]]*(scaffold|scaffolding|stub)"
    "not-implemented stub marker|#[[:space:]]*(TODO|FIXME).*(not[[:space:]]+implemented|placeholder|stub)"
    "echo not-implemented marker|echo[[:space:]]+[\"'][^\"']*not[[:space:]]+implemented[^\"']*[\"']"
    "return-0 placeholder marker|return[[:space:]]+0[[:space:]]*#[[:space:]]*(TODO|FIXME|placeholder|stub|no-?op|noop)"
    "colon no-op placeholder marker|:[[:space:]]*#[[:space:]]*(TODO|FIXME|placeholder|stub|no-?op|noop)"
  )

  for rel_path in "${migrated_files[@]}"; do
    abs_path="$repo_root/$rel_path"
    file_exists_or_fail "$rel_path" || continue

    for spec in "${prohibited_specs[@]}"; do
      label="${spec%%|*}"
      regex="${spec#*|}"
      match="$(rg -n --max-count 1 -i -e "$regex" "$abs_path" || true)"
      if [[ -n "$match" ]]; then
        fail_file "$rel_path" "prohibited $label found at $match"
      fi
    done
  done

  if [[ "$failures" -eq "$initial_failures" ]]; then
    pass_check "no prohibited placeholder/no-op scaffolding markers detected in migrated files"
  fi
}

echo "== Workflow shared-foundation audit =="
echo "mode: check"
printf 'scope: migrated_files=%d\n' "$total_migrated_files"
echo

run_check_resolve_helper_regression
run_check_shared_loader_wiring
run_check_non_search_driver_usage
run_check_prohibited_placeholders

echo
printf 'Summary: failures=%d\n' "$failures"
if [[ "$failures" -gt 0 ]]; then
  echo "Result: FAIL (shared-foundation drift detected)" >&2
  exit 1
fi

echo "Result: PASS"

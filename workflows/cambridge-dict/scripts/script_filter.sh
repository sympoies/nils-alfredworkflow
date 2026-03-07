#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

workflow_helper_loader="$script_dir/lib/workflow_helper_loader.sh"
if [[ ! -f "$workflow_helper_loader" ]]; then
  workflow_helper_loader="$script_dir/../../../scripts/lib/workflow_helper_loader.sh"
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    workflow_helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$workflow_helper_loader"

if ! wfhl_source_helper "$script_dir" "script_filter_error_json.sh"; then
  wfhl_emit_missing_helper_item_json "script_filter_error_json.sh"
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh"; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_cli_resolver.sh runtime helper."
  exit 0
fi

normalize_error_message() {
  sfej_normalize_error_message "${1-}"
}

emit_error_item() {
  local title="$1"
  local subtitle="$2"
  sfej_emit_error_item_json "$title" "$subtitle"
}

append_path_entry() {
  local dir="${1:-}"
  [[ -n "$dir" && -d "$dir" ]] || return 0

  case ":${PATH:-}:" in
  *":$dir:"*) ;;
  *)
    if [[ -n "${PATH:-}" ]]; then
      PATH="$dir:$PATH"
    else
      PATH="$dir"
    fi
    ;;
  esac
}

ensure_common_runtime_path() {
  append_path_entry "/opt/homebrew/bin"
  append_path_entry "/usr/local/bin"
  append_path_entry "/opt/local/bin"
  append_path_entry "/usr/bin"
  append_path_entry "/bin"
  export PATH
}

resolve_workflow_cache_dir() {
  if declare -F sfac_resolve_workflow_cache_dir >/dev/null 2>&1; then
    sfac_resolve_workflow_cache_dir "nils-cambridge-dict-workflow"
    return 0
  fi

  local candidate
  for candidate in \
    "${ALFRED_WORKFLOW_CACHE:-}" \
    "${ALFRED_WORKFLOW_DATA:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "${TMPDIR:-/tmp}/nils-cambridge-dict-workflow"
}

cambridge_runtime_state_dir() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  mkdir -p "$cache_dir/cambridge-runtime"
  printf '%s/cambridge-runtime\n' "$cache_dir"
}

cambridge_runtime_state_file() {
  printf '%s/bootstrap.state\n' "$(cambridge_runtime_state_dir)"
}

cambridge_runtime_log_file() {
  printf '%s/bootstrap.log\n' "$(cambridge_runtime_state_dir)"
}

cambridge_runtime_result_file() {
  printf '%s/bootstrap.result\n' "$(cambridge_runtime_state_dir)"
}

cambridge_runtime_bootstrap_helper_path() {
  if [[ -n "${CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER:-}" ]]; then
    printf '%s\n' "$CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER"
    return 0
  fi

  wfhl_resolve_helper_path "$script_dir" "cambridge_runtime_bootstrap.sh" auto
}

cambridge_runtime_issue_message() {
  local lower="${1:-}"
  [[ "$lower" == *"node"*"not found"* || "$lower" == *"playwright"* || "$lower" == *"chromium executable doesn't exist"* || "$lower" == *"browser executable"* || "$lower" == *"cambridge_node_bin"* ]]
}

cambridge_runtime_bootstrap_supported() {
  ensure_common_runtime_path

  local helper
  helper="$(cambridge_runtime_bootstrap_helper_path 2>/dev/null || true)"
  [[ -n "$helper" && -x "$helper" ]] || return 1

  command -v node >/dev/null 2>&1 || return 1
  command -v npm >/dev/null 2>&1 || return 1
  command -v npx >/dev/null 2>&1 || return 1
}

cambridge_runtime_bootstrap_running() {
  local state_file pid
  state_file="$(cambridge_runtime_state_file)"
  [[ -f "$state_file" ]] || return 1

  pid="$(sed -n '1p' "$state_file" 2>/dev/null | tr -d '[:space:]')"
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    return 0
  fi

  rm -f "$state_file"
  return 1
}

cambridge_runtime_recent_failure() {
  local cooldown_seconds="${1:-60}"
  local result_file status completed_at now age
  result_file="$(cambridge_runtime_result_file)"
  [[ -f "$result_file" ]] || return 1

  IFS=$'\t' read -r status completed_at <"$result_file" || return 1
  [[ "$status" == "err" ]] || return 1
  [[ "$completed_at" =~ ^[0-9]+$ ]] || return 1

  now="$(date +%s)"
  age=$((now - completed_at))
  [[ "$age" -ge 0 && "$age" -le "$cooldown_seconds" ]]
}

emit_runtime_bootstrap_pending_item() {
  sfac_emit_pending_item_json \
    "Installing Cambridge runtime..." \
    "Setting up workflow-local Playwright and Chromium. Alfred will retry automatically." \
    "${CAMBRIDGE_QUERY_COALESCE_RERUN_SECONDS:-0.4}"
}

emit_runtime_bootstrap_failed_item() {
  local log_file
  log_file="$(cambridge_runtime_log_file)"
  emit_error_item \
    "Automatic Cambridge runtime setup failed" \
    "Check the bootstrap log at $log_file, then retry after fixing Node/npm access."
}

start_cambridge_runtime_bootstrap() {
  local helper
  helper="$(cambridge_runtime_bootstrap_helper_path 2>/dev/null || true)"
  [[ -n "$helper" && -x "$helper" ]] || return 1

  local workflow_dir state_file log_file result_file
  workflow_dir="$(cd "$script_dir/.." && pwd)"
  state_file="$(cambridge_runtime_state_file)"
  log_file="$(cambridge_runtime_log_file)"
  result_file="$(cambridge_runtime_result_file)"

  nohup "$helper" \
    --workflow-dir "$workflow_dir" \
    --state-file "$state_file" \
    --log-file "$log_file" \
    --result-file "$result_file" \
    >/dev/null 2>&1 </dev/null &
}

handle_runtime_bootstrap() {
  if cambridge_runtime_bootstrap_running; then
    emit_runtime_bootstrap_pending_item
    return 0
  fi

  if ! cambridge_runtime_bootstrap_supported; then
    return 1
  fi

  if cambridge_runtime_recent_failure "${CAMBRIDGE_RUNTIME_BOOTSTRAP_FAILURE_COOLDOWN_SECONDS:-60}"; then
    emit_runtime_bootstrap_failed_item
    return 0
  fi

  if ! start_cambridge_runtime_bootstrap; then
    return 1
  fi

  emit_runtime_bootstrap_pending_item
  return 0
}

print_error_item() {
  local raw_message="${1:-cambridge-cli query failed}"
  local message
  message="$(normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="cambridge-cli query failed"

  local title="Cambridge Dict error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"query must not be empty"* || "$lower" == *"query cannot be empty"* || "$lower" == *"empty query"* ]]; then
    title="Enter a word"
    subtitle="Type a word after cd, then pick a candidate entry."
  elif [[ "$lower" == *"invalid cambridge_dict_mode"* || "$lower" == *"invalid cambridge_max_results"* || "$lower" == *"invalid cambridge_timeout_ms"* || "$lower" == *"invalid cambridge_headless"* || "$lower" == *"invalid config"* ]]; then
    title="Invalid Cambridge workflow config"
    subtitle="Check CAMBRIDGE_DICT_MODE/CAMBRIDGE_MAX_RESULTS/CAMBRIDGE_TIMEOUT_MS/CAMBRIDGE_HEADLESS."
  elif [[ "$lower" == *"anti_bot"* || "$lower" == *"cloudflare"* || "$lower" == *"challenge page"* || "$lower" == *"bot"* ]]; then
    title="Cambridge anti-bot challenge"
    subtitle="Cambridge blocked automation. Retry later or open dictionary site directly."
  elif [[ "$lower" == *"cookie_wall"* || "$lower" == *"cookie consent"* || "$lower" == *"enable cookies"* ]]; then
    title="Cambridge cookie consent required"
    subtitle="Open Cambridge Dictionary in browser once, accept cookies, then retry."
  elif [[ "$lower" == *"timed out"* || "$lower" == *"timeout"* ]]; then
    title="Cambridge request timed out"
    subtitle="Increase CAMBRIDGE_TIMEOUT_MS or retry with shorter query."
  elif cambridge_runtime_issue_message "$lower"; then
    if handle_runtime_bootstrap; then
      return 0
    fi
    title="Node/Playwright runtime unavailable"
    subtitle="Automatic setup needs Node.js + npm in Alfred PATH. Install Node.js or run setup manually."
  elif [[ "$lower" == *"binary not found"* || "$lower" == *"cambridge-cli binary not found"* ]]; then
    title="cambridge-cli binary not found"
    subtitle="Package workflow or set CAMBRIDGE_CLI_BIN to a cambridge-cli executable."
  elif [[ "$lower" == *"connection"* || "$lower" == *"dns"* || "$lower" == *"tls"* || "$lower" == *"network"* || "$lower" == *"status 500"* || "$lower" == *"status 502"* || "$lower" == *"status 503"* || "$lower" == *"status 504"* ]]; then
    title="Cambridge service unavailable"
    subtitle="Cannot reach Cambridge now. Check network and retry."
  fi

  emit_error_item "$title" "$subtitle"
}

resolve_keyword_lookup_mode() {
  local keyword="${alfred_workflow_keyword:-${ALFRED_WORKFLOW_KEYWORD:-}}"
  keyword="$(printf '%s' "$keyword" | tr '[:upper:]' '[:lower:]')"

  case "$keyword" in
  cds)
    printf '%s\n' "suggest"
    ;;
  *)
    printf '%s\n' "smart"
    ;;
  esac
}

query_has_explicit_mode_prefix() {
  local query="$1"
  [[ "$query" == def::* || "$query" == sug::* ]]
}

prepare_cli_query_input() {
  local query="$1"

  if query_has_explicit_mode_prefix "$query"; then
    printf '%s\n' "$query"
    return 0
  fi

  if [[ "$(resolve_keyword_lookup_mode)" == "suggest" ]]; then
    printf 'sug::%s\n' "$query"
    return 0
  fi

  printf '%s\n' "$query"
}

emit_direct_query_result() {
  local query="$1"
  local err_file
  err_file="$(sfsd_make_temp_err_file "cambridge-dict-direct.err")"

  local json_output
  if json_output="$(cambridge_query_fetch_json "$query" 2>"$err_file")"; then
    rm -f "$err_file"
    printf '%s\n' "$json_output"
    return 0
  fi

  local err_msg
  err_msg="$(cat "$err_file")"
  rm -f "$err_file"
  print_error_item "$err_msg"
}

resolve_cambridge_cli() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/cambridge-cli"

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_cli
  release_cli="$repo_root/target/release/cambridge-cli"

  local debug_cli
  debug_cli="$repo_root/target/debug/cambridge-cli"

  wfcr_resolve_binary \
    "CAMBRIDGE_CLI_BIN" \
    "$packaged_cli" \
    "$release_cli" \
    "$debug_cli" \
    "cambridge-cli binary not found (checked CAMBRIDGE_CLI_BIN/package/release/debug paths)"
}

cambridge_query_fetch_json() {
  local query="$1"
  local err_file="${TMPDIR:-/tmp}/cambridge-dict-script-filter.err.$$.$RANDOM"

  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  export CAMBRIDGE_SCRAPER_SCRIPT="$script_dir/cambridge_scraper.mjs"

  local cambridge_cli
  if ! cambridge_cli="$(resolve_cambridge_cli 2>"$err_file")"; then
    cat "$err_file" >&2
    rm -f "$err_file"
    return 1
  fi

  local json_output
  if json_output="$("$cambridge_cli" query --input "$query" --mode alfred 2>"$err_file")"; then
    rm -f "$err_file"

    if [[ -z "$json_output" ]]; then
      echo "cambridge-cli returned empty response" >&2
      return 1
    fi

    if command -v jq >/dev/null 2>&1; then
      if ! jq -e '.items | type == "array"' >/dev/null <<<"$json_output"; then
        echo "cambridge-cli returned malformed Alfred JSON" >&2
        return 1
      fi
    fi

    printf '%s\n' "$json_output"
    return 0
  fi

  cat "$err_file" >&2
  rm -f "$err_file"
  return 1
}

if ! wfhl_source_helper "$script_dir" "script_filter_query_policy.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_query_policy.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_async_coalesce.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_async_coalesce.sh runtime helper."
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "script_filter_search_driver.sh"; then
  emit_error_item "Workflow helper missing" "Cannot locate script_filter_search_driver.sh runtime helper."
  exit 0
fi

ensure_common_runtime_path

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
query="$trimmed_query"

if [[ -z "$query" ]]; then
  emit_error_item "Enter a word" "Type a word after cd, then pick a candidate entry."
  exit 0
fi

if sfqp_is_short_query "$query" 2; then
  sfqp_emit_short_query_item_json \
    2 \
    "Keep typing (2+ chars)" \
    "Type at least %s characters before searching Cambridge Dictionary."
  exit 0
fi

cli_query="$(prepare_cli_query_input "$query")"

: "${CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS:=0}"

if query_has_explicit_mode_prefix "$cli_query"; then
  emit_direct_query_result "$cli_query"
  exit 0
fi

# Shared driver owns cache/coalesce orchestration only.
# Cambridge-specific backend fetch and error mapping remain local in this script.
sfsd_run_search_flow \
  "$cli_query" \
  "cambridge-dict" \
  "nils-cambridge-dict-workflow" \
  "CAMBRIDGE_QUERY_CACHE_TTL_SECONDS" \
  "CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS" \
  "CAMBRIDGE_QUERY_COALESCE_RERUN_SECONDS" \
  "Searching Cambridge..." \
  "Waiting for final query before calling Cambridge backend." \
  "cambridge_query_fetch_json" \
  "print_error_item"

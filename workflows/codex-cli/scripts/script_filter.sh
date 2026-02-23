#!/usr/bin/env bash
set -euo pipefail

workflow_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runtime_meta="$workflow_script_dir/lib/codex_cli_runtime.sh"
if [[ ! -f "$runtime_meta" ]]; then
  printf '{"items":[{"title":"codex-cli runtime metadata missing","subtitle":"expected %s","valid":false}]}\n' "$runtime_meta"
  exit 0
fi
# shellcheck disable=SC1090
source "$runtime_meta"
# shellcheck disable=SC2153
codex_cli_pinned_version="${CODEX_CLI_PINNED_VERSION}"
# shellcheck disable=SC2153
codex_cli_pinned_crate="${CODEX_CLI_PINNED_CRATE}"

helper_loader=""
for candidate in \
  "$workflow_script_dir/lib/workflow_helper_loader.sh" \
  "$workflow_script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -n "$helper_loader" ]]; then
  # shellcheck disable=SC1090
  source "$helper_loader"
  wfhl_source_helper "$workflow_script_dir" "workflow_cli_resolver.sh" off || true
  wfhl_source_helper "$workflow_script_dir" "script_filter_query_policy.sh" off || true
  wfhl_source_helper "$workflow_script_dir" "script_filter_async_coalesce.sh" off || true
fi

if ! declare -F sfqp_trim >/dev/null 2>&1; then
  sfqp_trim() {
    local value="${1-}"
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    printf '%s' "$value"
  }
fi

if ! declare -F sfqp_resolve_query_input >/dev/null 2>&1; then
  sfqp_resolve_query_input() {
    local query="${1-}"
    if [[ -z "$query" && -n "${alfred_workflow_query:-}" ]]; then
      query="${alfred_workflow_query}"
    elif [[ -z "$query" && -n "${ALFRED_WORKFLOW_QUERY:-}" ]]; then
      query="${ALFRED_WORKFLOW_QUERY}"
    elif [[ -z "$query" && ! -t 0 ]]; then
      query="$(cat)"
    fi
    printf '%s' "$query"
  }
fi

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

trim() {
  sfqp_trim "${1-}"
}

expand_home_path() {
  local value="${1-}"

  case "$value" in
  "~")
    if [[ -n "${HOME:-}" ]]; then
      printf '%s\n' "${HOME%/}"
      return 0
    fi
    ;;
  \~/*)
    if [[ -n "${HOME:-}" ]]; then
      printf '%s/%s\n' "${HOME%/}" "${value#\~/}"
      return 0
    fi
    ;;
  esac

  printf '%s\n' "$value"
}

resolve_codex_cli_override() {
  local configured="${CODEX_CLI_BIN:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"
  [[ -n "$configured" ]] || return 1
  printf '%s\n' "$configured"
}

to_lower() {
  local value="${1-}"
  printf '%s' "$value" | tr '[:upper:]' '[:lower:]'
}

strip_ansi() {
  local line="${1:-}"
  printf '%s' "$line" | sed -E $'s/\\x1B\\[[0-9;]*[A-Za-z]//g'
}

resolve_workflow_cache_dir() {
  if declare -F sfac_resolve_workflow_cache_dir >/dev/null 2>&1; then
    sfac_resolve_workflow_cache_dir "nils-codex-cli-workflow"
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

  printf '%s\n' "${TMPDIR:-/tmp}/nils-codex-cli-workflow"
}

sanitize_diag_mode() {
  local mode="${1:-default}"
  mode="${mode//[^A-Za-z0-9._-]/_}"
  [[ -n "$mode" ]] || mode="default"
  printf '%s\n' "$mode"
}

canonical_diag_cache_mode() {
  local mode
  mode="$(sanitize_diag_mode "${1:-default}")"
  if [[ "$mode" == "all-json" ]]; then
    printf 'all-json\n'
    return 0
  fi
  printf 'default\n'
}

diag_result_meta_path() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.last.meta\n' "$cache_dir"
}

diag_result_output_path() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.last.out\n' "$cache_dir"
}

diag_result_meta_path_all_json() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.all-json.meta\n' "$cache_dir"
}

diag_result_output_path_all_json() {
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.all-json.out\n' "$cache_dir"
}

resolve_latest_diag_cache_mode() {
  local meta_path
  meta_path="$(diag_result_meta_path)"
  [[ -f "$meta_path" ]] || return 1

  local mode
  mode="$(read_meta_value "$meta_path" mode)"
  [[ -n "$mode" ]] || return 1

  canonical_diag_cache_mode "$mode"
}

resolve_diag_result_cache_paths_for_mode() {
  local expected_mode="${1:-}"
  local canonical_mode=""
  if [[ -n "$expected_mode" ]]; then
    canonical_mode="$(canonical_diag_cache_mode "$expected_mode")"
  fi

  if [[ "$canonical_mode" == "all-json" ]]; then
    local mode_meta_path mode_output_path
    mode_meta_path="$(diag_result_meta_path_all_json)"
    mode_output_path="$(diag_result_output_path_all_json)"
    if [[ -f "$mode_meta_path" && -f "$mode_output_path" ]]; then
      printf '%s\t%s\n' "$mode_meta_path" "$mode_output_path"
      return 0
    fi
  fi

  local last_meta_path last_output_path
  last_meta_path="$(diag_result_meta_path)"
  last_output_path="$(diag_result_output_path)"
  [[ -f "$last_meta_path" && -f "$last_output_path" ]] || return 1

  if [[ -n "$expected_mode" ]]; then
    local cached_mode
    cached_mode="$(read_meta_value "$last_meta_path" mode)"
    [[ "$cached_mode" == "$canonical_mode" ]] || return 1
  fi

  printf '%s\t%s\n' "$last_meta_path" "$last_output_path"
}

diag_refresh_lock_path_for_mode() {
  local mode
  mode="$(canonical_diag_cache_mode "${1:-default}")"
  local cache_dir
  cache_dir="$(resolve_workflow_cache_dir)"
  printf '%s/diag-rate-limits.%s.refresh.lock\n' "$cache_dir" "$mode"
}

resolve_diag_cache_ttl_seconds() {
  local raw="${CODEX_DIAG_CACHE_TTL_SECONDS:-300}"
  if [[ "$raw" =~ ^[0-9]+$ ]] && [[ "$raw" -ge 0 ]] && [[ "$raw" -le 86400 ]]; then
    printf '%s\n' "$raw"
    return 0
  fi
  printf '300\n'
}

is_diag_cache_fresh_from_meta() {
  local meta_path="$1"
  local ttl_seconds="$2"
  [[ -f "$meta_path" ]] || return 1
  [[ "$ttl_seconds" =~ ^[0-9]+$ ]] || return 1

  if [[ "$ttl_seconds" -eq 0 ]]; then
    return 1
  fi

  local timestamp
  timestamp="$(read_meta_value "$meta_path" timestamp)"
  [[ "$timestamp" =~ ^[0-9]+$ ]] || return 1

  local now age
  now="$(date +%s)"
  age=$((now - timestamp))
  if [[ "$age" -lt 0 ]]; then
    age=0
  fi

  [[ "$age" -le "$ttl_seconds" ]]
}

is_diag_cache_fresh_for_mode() {
  local mode="$1"
  local ttl_seconds="$2"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"

  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_result_cache_paths_for_mode "$canonical_mode" || true)"
  [[ -n "$cache_paths" ]] || return 1
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local cached_mode
  cached_mode="$(read_meta_value "$meta_path" mode)"
  [[ "$cached_mode" == "$canonical_mode" ]] || return 1

  is_diag_cache_fresh_from_meta "$meta_path" "$ttl_seconds"
}

is_diag_refresh_running_for_mode() {
  local mode="$1"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"
  local lock_path
  lock_path="$(diag_refresh_lock_path_for_mode "$canonical_mode")"
  [[ -f "$lock_path" ]] || return 1

  local lock_ts now age
  lock_ts="$(read_meta_value "$lock_path" timestamp)"
  if [[ ! "$lock_ts" =~ ^[0-9]+$ ]]; then
    rm -f "$lock_path"
    return 1
  fi

  now="$(date +%s)"
  age=$((now - lock_ts))
  if [[ "$age" -lt 0 ]]; then
    age=0
  fi

  # stale lock fallback
  if [[ "$age" -gt 600 ]]; then
    rm -f "$lock_path"
    return 1
  fi

  return 0
}

acquire_diag_refresh_lock_for_mode() {
  local mode="$1"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"
  local lock_path
  lock_path="$(diag_refresh_lock_path_for_mode "$canonical_mode")"
  mkdir -p "$(dirname "$lock_path")"

  if is_diag_refresh_running_for_mode "$canonical_mode"; then
    return 1
  fi

  local now
  now="$(date +%s)"
  if (
    set -o noclobber
    {
      printf 'timestamp=%s\n' "$now"
      printf 'pid=%s\n' "$$"
      printf 'mode=%s\n' "$canonical_mode"
    } >"$lock_path"
  ) 2>/dev/null; then
    printf '%s\n' "$lock_path"
    return 0
  fi

  return 1
}

store_diag_result() {
  local mode="$1"
  local summary="$2"
  local command="$3"
  local rc="$4"
  local output="$5"
  local normalized_mode
  normalized_mode="$(sanitize_diag_mode "$mode")"
  local timestamp
  timestamp="$(date +%s)"

  local last_meta_path last_output_path
  last_meta_path="$(diag_result_meta_path)"
  last_output_path="$(diag_result_output_path)"
  local output_dir
  output_dir="$(dirname "$last_output_path")"

  mkdir -p "$output_dir"

  {
    printf 'mode=%s\n' "$normalized_mode"
    printf 'summary=%s\n' "$summary"
    printf 'command=%s\n' "$command"
    printf 'exit_code=%s\n' "$rc"
    printf 'timestamp=%s\n' "$timestamp"
  } >"$last_meta_path"
  printf '%s\n' "$output" >"$last_output_path"

  if [[ "$normalized_mode" == "all-json" ]]; then
    local all_meta_path all_output_path
    all_meta_path="$(diag_result_meta_path_all_json)"
    all_output_path="$(diag_result_output_path_all_json)"
    mkdir -p "$(dirname "$all_output_path")"
    {
      printf 'mode=%s\n' "$normalized_mode"
      printf 'summary=%s\n' "$summary"
      printf 'command=%s\n' "$command"
      printf 'exit_code=%s\n' "$rc"
      printf 'timestamp=%s\n' "$timestamp"
    } >"$all_meta_path"
    printf '%s\n' "$output" >"$all_output_path"
  fi
}

capture_command_output_with_stdout_priority() {
  local __out_var="$1"
  local __rc_var="$2"
  shift 2

  local stdout_file stderr_file
  stdout_file="$(mktemp "${TMPDIR:-/tmp}/codex-diag-stdout.XXXXXX")"
  stderr_file="$(mktemp "${TMPDIR:-/tmp}/codex-diag-stderr.XXXXXX")"

  local capture_rc=0
  set +e
  "$@" >"$stdout_file" 2>"$stderr_file"
  capture_rc=$?
  set -e

  local captured_stdout captured_stderr chosen_output
  captured_stdout="$(cat "$stdout_file" 2>/dev/null || true)"
  captured_stderr="$(cat "$stderr_file" 2>/dev/null || true)"
  rm -f "$stdout_file" "$stderr_file"

  chosen_output="$captured_stderr"
  if [[ -n "$captured_stdout" ]]; then
    chosen_output="$captured_stdout"
  fi

  printf -v "$__out_var" '%s' "$chosen_output"
  printf -v "$__rc_var" '%s' "$capture_rc"
}

run_diag_cache_refresh_for_mode() {
  local mode="$1"
  local codex_cli
  codex_cli="$(resolve_codex_cli_path || true)"
  [[ -n "$codex_cli" ]] || return 0

  local resolved_secret_dir
  resolved_secret_dir="$(resolve_codex_secret_dir || true)"
  if [[ -n "$resolved_secret_dir" ]]; then
    export CODEX_SECRET_DIR="$resolved_secret_dir"
  fi

  local summary command output rc
  output=""
  rc=0

  case "$mode" in
  all-json)
    if secret_dir_has_saved_json "$resolved_secret_dir"; then
      summary="diag rate-limits --all --json"
      command="diag rate-limits --all --json"
      capture_command_output_with_stdout_priority output rc "$codex_cli" diag rate-limits --all --json
    else
      summary="diag rate-limits --json (auth.json)"
      command="diag rate-limits --json"
      capture_command_output_with_stdout_priority output rc "$codex_cli" diag rate-limits --json
    fi
    ;;
  *)
    summary="diag rate-limits --json"
    command="diag rate-limits --json"
    capture_command_output_with_stdout_priority output rc "$codex_cli" diag rate-limits --json
    ;;
  esac

  store_diag_result "$mode" "$summary" "$command" "$rc" "$output"
}

resolve_diag_auto_refresh_mode_for_query() {
  local lower_query="$1"
  case "$lower_query" in
  diag)
    printf 'default\n'
    return 0
    ;;
  diag\ all-json)
    printf 'all-json\n'
    return 0
    ;;
  *)
    return 1
    ;;
  esac
}

resolve_diag_cache_block_wait_seconds() {
  local raw="${CODEX_DIAG_CACHE_BLOCK_WAIT_SECONDS:-15}"
  if [[ "$raw" =~ ^[0-9]+$ ]] && [[ "$raw" -ge 0 ]] && [[ "$raw" -le 120 ]]; then
    printf '%s\n' "$raw"
    return 0
  fi
  printf '15\n'
}

diag_cache_exists_for_mode() {
  local mode="$1"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"
  local cache_paths
  cache_paths="$(resolve_diag_result_cache_paths_for_mode "$canonical_mode" || true)"
  [[ -n "$cache_paths" ]]
}

resolve_diag_display_cache_paths_for_mode() {
  local expected_mode="${1:-}"
  local canonical_mode=""
  if [[ -n "$expected_mode" ]]; then
    canonical_mode="$(canonical_diag_cache_mode "$expected_mode")"
  fi

  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_result_cache_paths_for_mode "$canonical_mode" || true)"
  [[ -n "$cache_paths" ]] || return 1
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local ttl_seconds
  ttl_seconds="$(resolve_diag_cache_ttl_seconds)"
  if [[ "$ttl_seconds" -ne 0 ]] && ! is_diag_cache_fresh_from_meta "$meta_path" "$ttl_seconds"; then
    return 1
  fi

  if [[ -n "$canonical_mode" ]]; then
    local cached_mode
    cached_mode="$(read_meta_value "$meta_path" mode)"
    [[ "$cached_mode" == "$canonical_mode" ]] || return 1
  fi

  printf '%s\t%s\n' "$meta_path" "$output_path"
}

wait_for_diag_refresh_completion_for_mode() {
  local mode="$1"
  local wait_seconds="$2"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"

  local max_ticks tick
  max_ticks=$((wait_seconds * 10))
  tick=0

  while is_diag_refresh_running_for_mode "$canonical_mode"; do
    if [[ "$tick" -ge "$max_ticks" ]]; then
      return 1
    fi
    sleep 0.1
    tick=$((tick + 1))
  done

  return 0
}

refresh_diag_cache_blocking_for_mode() {
  local mode="$1"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"
  local lock_path
  if ! lock_path="$(acquire_diag_refresh_lock_for_mode "$canonical_mode")"; then
    return 1
  fi

  (
    set -euo pipefail
    trap 'rm -f "$lock_path"' EXIT
    run_diag_cache_refresh_for_mode "$canonical_mode"
  )
}

ensure_diag_cache_ready_for_mode() {
  local mode="$1"
  local canonical_mode
  canonical_mode="$(canonical_diag_cache_mode "$mode")"
  local ttl_seconds
  ttl_seconds="$(resolve_diag_cache_ttl_seconds)"
  local wait_seconds
  wait_seconds="$(resolve_diag_cache_block_wait_seconds)"

  if [[ "$ttl_seconds" -ne 0 ]] && is_diag_cache_fresh_for_mode "$canonical_mode" "$ttl_seconds"; then
    return 0
  fi

  if is_diag_refresh_running_for_mode "$canonical_mode"; then
    wait_for_diag_refresh_completion_for_mode "$canonical_mode" "$wait_seconds" || true

    if [[ "$ttl_seconds" -eq 0 ]]; then
      if diag_cache_exists_for_mode "$canonical_mode"; then
        return 0
      fi
    elif is_diag_cache_fresh_for_mode "$canonical_mode" "$ttl_seconds"; then
      return 0
    fi
  fi

  if ! refresh_diag_cache_blocking_for_mode "$canonical_mode"; then
    if is_diag_refresh_running_for_mode "$canonical_mode"; then
      wait_for_diag_refresh_completion_for_mode "$canonical_mode" "$wait_seconds" || true
    fi
  fi

  if [[ "$ttl_seconds" -eq 0 ]]; then
    diag_cache_exists_for_mode "$canonical_mode"
    return
  fi

  is_diag_cache_fresh_for_mode "$canonical_mode" "$ttl_seconds"
}

read_meta_value() {
  local file="$1"
  local key="$2"
  sed -n "s/^${key}=//p" "$file" | head -n1
}

format_epoch() {
  local ts="${1:-}"
  if [[ -z "$ts" || ! "$ts" =~ ^[0-9]+$ ]]; then
    printf 'unknown time\n'
    return 0
  fi

  if date -r "$ts" '+%Y-%m-%d %H:%M:%S' >/dev/null 2>&1; then
    date -r "$ts" '+%Y-%m-%d %H:%M:%S'
    return 0
  fi

  if date -d "@$ts" '+%Y-%m-%d %H:%M:%S' >/dev/null 2>&1; then
    date -d "@$ts" '+%Y-%m-%d %H:%M:%S'
    return 0
  fi

  printf 'epoch:%s\n' "$ts"
}

format_compact_duration_minutes() {
  local total_minutes="${1:-}"
  [[ "$total_minutes" =~ ^[0-9]+$ ]] || return 1

  local days remainder hours minutes
  days=$((total_minutes / 1440))
  remainder=$((total_minutes % 1440))
  hours=$((remainder / 60))
  minutes=$((remainder % 60))

  if [[ "$days" -gt 0 ]]; then
    if [[ "$hours" -gt 0 ]]; then
      printf '%sd %sh\n' "$days" "$hours"
      return 0
    fi
    if [[ "$minutes" -gt 0 ]]; then
      printf '%sd %sm\n' "$days" "$minutes"
      return 0
    fi
    printf '%sd\n' "$days"
    return 0
  fi

  if [[ "$hours" -gt 0 ]]; then
    if [[ "$minutes" -gt 0 ]]; then
      printf '%sh %sm\n' "$hours" "$minutes"
      return 0
    fi
    printf '%sh\n' "$hours"
    return 0
  fi

  printf '%sm\n' "$minutes"
}

format_remaining_duration_from_epoch() {
  local reset_epoch="${1:-}"
  [[ "$reset_epoch" =~ ^[0-9]+$ ]] || return 1

  local now delta remaining_minutes
  now="$(date +%s)"
  [[ "$now" =~ ^[0-9]+$ ]] || return 1

  delta=$((reset_epoch - now))
  if [[ "$delta" -lt 0 ]]; then
    delta=0
  fi

  remaining_minutes=$(((delta + 59) / 60))
  format_compact_duration_minutes "$remaining_minutes"
}

estimate_remaining_duration_from_percent() {
  local window_label="${1:-}"
  local percentage="${2:-}"

  [[ "$window_label" =~ ^([0-9]+)([HhMm])$ ]] || return 1
  local window_value="${BASH_REMATCH[1]}"
  local window_unit="${BASH_REMATCH[2]}"
  [[ "$percentage" =~ ^[0-9]+([.][0-9]+)?$ ]] || return 1

  local minutes
  minutes="$(
    awk -v value="$window_value" -v unit="$window_unit" -v pct="$percentage" '
      BEGIN {
        total = value
        if (tolower(unit) == "h") total = total * 60
        if (pct < 0) pct = 0
        if (pct > 100) pct = 100
        mins = int((total * pct / 100) + 0.5)
        if (mins < 0) mins = 0
        printf "%d\n", mins
      }
    '
  )"
  [[ "$minutes" =~ ^[0-9]+$ ]] || return 1
  format_compact_duration_minutes "$minutes"
}

build_usage_metric_text() {
  local metric_label="${1:-}"
  local percentage="${2:-}"
  local reset_epoch="${3:-}"
  local estimate_window_label="${4:-}"

  local text="${metric_label} n/a"
  if [[ -n "$percentage" && "$percentage" != "null" && "$percentage" != "-" ]]; then
    text="${metric_label} ${percentage}%"
  fi

  local remaining_text=""
  if [[ -n "$reset_epoch" && "$reset_epoch" != "null" && "$reset_epoch" != "-" ]]; then
    remaining_text="$(format_remaining_duration_from_epoch "$reset_epoch" || true)"
  fi
  if [[ -z "$remaining_text" && -n "$estimate_window_label" && -n "$percentage" && "$percentage" != "null" && "$percentage" != "-" ]]; then
    remaining_text="$(estimate_remaining_duration_from_percent "$estimate_window_label" "$percentage" || true)"
  fi

  if [[ -n "$remaining_text" ]]; then
    printf '%s (%s)\n' "$text" "$remaining_text"
    return 0
  fi

  printf '%s\n' "$text"
}

emit_diag_all_json_account_items() {
  local lower_query="$1"
  local output_path="$2"

  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local dataset_filter
  if jq -e '.results | type == "array"' "$output_path" >/dev/null 2>&1; then
    dataset_filter='.results // []'
  elif jq -e '.result | type == "object"' "$output_path" >/dev/null 2>&1; then
    dataset_filter='[.result]'
  else
    return 1
  fi

  local max_rows=24
  if [[ "$lower_query" == *" raw"* || "$lower_query" == *"--raw"* ]]; then
    max_rows=200
  fi

  local row_count=0
  local truncated=0
  local row
  while IFS= read -r row || [[ -n "$row" ]]; do
    row_count=$((row_count + 1))
    if [[ "$row_count" -gt "$max_rows" ]]; then
      truncated=1
      break
    fi

    local name status label non_weekly weekly non_weekly_reset_epoch weekly_reset_epoch weekly_reset email
    IFS=$'\t' read -r name status label non_weekly weekly non_weekly_reset_epoch weekly_reset_epoch weekly_reset email <<<"$row"
    [[ -n "$name" ]] || name="(unknown)"
    [[ -n "$status" ]] || status="unknown"
    [[ -n "$label" ]] || label="5h"
    [[ -n "$non_weekly_reset_epoch" ]] || non_weekly_reset_epoch="null"
    [[ -n "$weekly_reset_epoch" ]] || weekly_reset_epoch="null"
    [[ -n "$weekly_reset" ]] || weekly_reset="-"
    [[ -n "$email" ]] || email="-"

    if [[ "$status" == "ok" ]]; then
      local non_weekly_text weekly_text
      non_weekly_text="$(build_usage_metric_text "$label" "$non_weekly" "$non_weekly_reset_epoch" "$label")"
      weekly_text="$(build_usage_metric_text "weekly" "$weekly" "$weekly_reset_epoch" "")"
      emit_item \
        "${name} | ${non_weekly_text} | ${weekly_text}" \
        "${email} | reset ${weekly_reset}" \
        "" \
        false \
        ""
    else
      emit_item \
        "${name} | status=${status}" \
        "${email}" \
        "" \
        false \
        ""
    fi
  done < <(jq -r "$dataset_filter | sort_by((.summary.weekly_reset_epoch // 9999999999), (.name // \"\"))[]? | [(.name // \"(current)\"), (.status // \"unknown\"), (.summary.non_weekly_label // \"5h\"), (.summary.non_weekly_remaining // \"null\"), (.summary.weekly_remaining // \"null\"), ((.summary.non_weekly_reset_epoch // \"null\") | tostring), ((.summary.weekly_reset_epoch // \"null\") | tostring), (.summary.weekly_reset_local // \"-\"), (.raw_usage.email // \"-\")] | @tsv" "$output_path")

  if [[ "$row_count" -eq 0 ]]; then
    emit_item \
      "No accounts in JSON result" \
      "diag --all returned zero entries." \
      "" \
      false \
      ""
  fi

  if [[ "$truncated" -eq 1 ]]; then
    emit_item \
      "Account list truncated (${max_rows} rows shown)" \
      "Type: cxda result raw" \
      "" \
      false \
      "diag result all-json raw"
  fi

  return 0
}

emit_diag_result_items() {
  local lower_query="$1"
  local expected_mode=""
  local run_alias="cxd"
  local result_alias="cxd result"
  local diag_alias
  diag_alias="$(to_lower "${CODEX_DIAG_ALIAS:-}")"

  if [[ "$lower_query" == *"all-json"* || "$lower_query" == *"--all-json"* ]]; then
    expected_mode="all-json"
    run_alias="cxda"
    result_alias="cxda result"
  fi

  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_display_cache_paths_for_mode "$expected_mode" || true)"
  if [[ -z "$cache_paths" ]]; then
    emit_item \
      "No fresh diag result yet" \
      "Run ${run_alias} to refresh diagnostics, then open ${result_alias}." \
      "" \
      false \
      "diag"
    return
  fi
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local mode rc timestamp command summary
  mode="$(read_meta_value "$meta_path" mode)"
  rc="$(read_meta_value "$meta_path" exit_code)"
  timestamp="$(read_meta_value "$meta_path" timestamp)"
  command="$(read_meta_value "$meta_path" command)"
  summary="$(read_meta_value "$meta_path" summary)"

  [[ -n "$mode" ]] || mode="unknown"
  [[ -n "$rc" ]] || rc="1"
  [[ -n "$command" ]] || command="diag rate-limits"
  [[ -n "$summary" ]] || summary="$command"
  local formatted_time
  formatted_time="$(format_epoch "$timestamp")"
  local summary_subtitle="${summary} | ${formatted_time}"

  if [[ "$rc" == "0" ]]; then
    emit_item \
      "Diag result ready (${mode})" \
      "$summary_subtitle" \
      "" \
      false \
      "diag result"
  else
    emit_item \
      "Diag failed (${mode}, rc=${rc})" \
      "$summary_subtitle" \
      "" \
      false \
      "diag result"
  fi

  if [[ "$mode" == "all-json" || "$mode" == "default" ]]; then
    if emit_diag_all_json_account_items "$lower_query" "$output_path"; then
      return
    fi
  fi

  local max_lines=12
  local raw_hint_title="Type: cxd result raw"
  local raw_hint_autocomplete="diag result raw"
  if [[ "$mode" == "all-json" ]]; then
    raw_hint_title="Type: cxda result raw"
    raw_hint_autocomplete="diag result all-json raw"
  fi
  if [[ "$lower_query" == *" raw"* || "$lower_query" == *"--raw"* ]]; then
    max_lines=60
  fi

  local line_count=0
  local truncated=0
  local line clean normalized_clean
  while IFS= read -r line || [[ -n "$line" ]]; do
    clean="$(trim "$(strip_ansi "$line")")"
    [[ -z "$clean" ]] && continue
    if [[ "$diag_alias" == "cxd" ]]; then
      normalized_clean="$(to_lower "$clean")"
      if [[ "$normalized_clean" =~ ^rate[[:space:]]+limits[[:space:]]+remaining:?$ ]]; then
        continue
      fi
    fi

    line_count=$((line_count + 1))
    if [[ "$line_count" -gt "$max_lines" ]]; then
      truncated=1
      break
    fi

    emit_item \
      "$clean" \
      "diag output" \
      "" \
      false \
      ""
  done <"$output_path"

  if [[ "$line_count" -eq 0 ]]; then
    emit_item \
      "(no output)" \
      "diag command finished without stdout/stderr." \
      "" \
      false \
      ""
  fi

  if [[ "$truncated" -eq 1 ]]; then
    emit_item \
      "Output truncated (${max_lines} lines shown)" \
      "$raw_hint_title" \
      "" \
      false \
      "$raw_hint_autocomplete"
  fi
}

is_truthy() {
  local value
  value="$(to_lower "${1:-}")"
  case "$value" in
  1 | true | yes | on)
    return 0
    ;;
  *)
    return 1
    ;;
  esac
}

query_has_assessment_flag() {
  local lower_query="${1:-}"
  [[ "$lower_query" == *"--assessment"* || "$lower_query" == *"--show-assessment"* ]]
}

strip_assessment_flags() {
  local raw_query="${1:-}"
  local token
  local output=()

  # shellcheck disable=SC2206
  local parts=($raw_query)
  for token in "${parts[@]}"; do
    case "$(to_lower "$token")" in
    --assessment | --show-assessment)
      continue
      ;;
    *)
      output+=("$token")
      ;;
    esac
  done

  printf '%s\n' "$(trim "${output[*]:-}")"
}

begin_items() {
  ITEM_COUNT=0
  printf '{"items":['
}

emit_item() {
  local title="$1"
  local subtitle="$2"
  local arg="${3-}"
  local valid="${4:-false}"
  local autocomplete="${5-}"

  if [[ "$ITEM_COUNT" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":%s' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$valid"

  if [[ -n "$arg" ]]; then
    printf ',"arg":"%s"' "$(json_escape "$arg")"
  fi

  if [[ -n "$autocomplete" ]]; then
    printf ',"autocomplete":"%s"' "$(json_escape "$autocomplete")"
  fi

  printf '}'
  ITEM_COUNT=$((ITEM_COUNT + 1))
}

end_items() {
  printf ']}\n'
}

resolve_codex_cli_path() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local packaged_cli
  packaged_cli="$script_dir/../bin/codex-cli"
  local release_cli
  release_cli="$repo_root/target/release/codex-cli"
  local debug_cli
  debug_cli="$repo_root/target/debug/codex-cli"

  if declare -F wfcr_resolve_binary >/dev/null 2>&1; then
    wfcr_resolve_binary \
      "CODEX_CLI_BIN" \
      "$packaged_cli" \
      "$release_cli" \
      "$debug_cli" \
      "codex-cli binary not found (re-import workflow bundle, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version} manually.)"
    return $?
  fi

  local configured_cli=""
  configured_cli="$(resolve_codex_cli_override || true)"
  if [[ -n "$configured_cli" && -x "$configured_cli" ]]; then
    printf '%s\n' "$configured_cli"
    return 0
  fi

  if [[ -x "$packaged_cli" ]]; then
    printf '%s\n' "$packaged_cli"
    return 0
  fi

  local resolved
  resolved="$(command -v codex-cli 2>/dev/null || true)"
  if [[ -n "$resolved" && -x "$resolved" ]]; then
    printf '%s\n' "$resolved"
    return 0
  fi

  return 1
}

emit_runtime_status() {
  if resolve_codex_cli_path >/dev/null 2>&1; then
    return
  fi

  emit_item \
    "codex-cli runtime missing" \
    "Re-import workflow, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version} manually." \
    "" \
    false \
    ""
}

emit_assessment_items() {
  emit_item \
    "Implemented now: auth login" \
    "Supports browser (default), --api-key, and --device-code from Alfred." \
    "" \
    false \
    ""
  emit_item \
    "Implemented now: auth save/use/remove" \
    "Use save/remove [--yes] <secret.json> and use <secret> (or cxau) from Alfred." \
    "" \
    false \
    ""
  emit_item \
    "Implemented now: diag rate-limits" \
    "Quick presets included: default, --cached, --one-line, --all, --all --async." \
    "" \
    false \
    ""
  emit_item \
    "Can be added next: auth refresh/current/sync" \
    "${codex_cli_pinned_version} supports these auth helpers; straightforward to map next." \
    "" \
    false \
    ""
  emit_item \
    "Can be added next: config / starship / agent" \
    "config show/set, starship render, and agent wrappers are available in the crate." \
    "" \
    false \
    ""
}

emit_auth_action_items() {
  emit_item \
    "auth login (browser)" \
    "Run codex-cli auth login" \
    "login::browser" \
    true \
    "login"
  emit_item \
    "auth login --api-key" \
    "Run codex-cli auth login --api-key" \
    "login::api-key" \
    true \
    "login --api-key"
  emit_item \
    "auth login --device-code" \
    "Run codex-cli auth login --device-code" \
    "login::device-code" \
    true \
    "login --device-code"
  emit_item \
    "auth save <secret.json>" \
    "Type: save team-alpha.json (or save --yes team-alpha.json)" \
    "" \
    false \
    "save "
  emit_item \
    "auth remove <secret.json>" \
    "Type: remove team-alpha.json (or remove --yes team-alpha.json)" \
    "" \
    false \
    "remove "
  emit_item \
    "auth use <secret>" \
    "Type: use alpha (or open cxau to pick from saved JSON secrets)." \
    "" \
    false \
    "use "
}

emit_diag_action_items() {
  emit_item \
    "diag rate-limits --json (parsed)" \
    "Run default diagnostics with JSON output parsing." \
    "diag::default" \
    true \
    "diag"
  emit_item \
    "diag rate-limits --cached" \
    "Use cache only; no network." \
    "diag::cached" \
    true \
    "diag cached"
  emit_item \
    "diag rate-limits --one-line" \
    "Compact one-line diagnostics output." \
    "diag::one-line" \
    true \
    "diag one-line"
  emit_item \
    "diag rate-limits --all" \
    "Query all secrets under CODEX_SECRET_DIR." \
    "diag::all" \
    true \
    "diag all"
  emit_item \
    "diag rate-limits --all --async --jobs 4" \
    "Concurrent diagnostics for all secrets." \
    "diag::async" \
    true \
    "diag async"
}

emit_default_action_items() {
  emit_auth_action_items
  emit_diag_action_items
}

normalize_save_secret() {
  local raw_secret="$1"
  local secret
  secret="$(trim "$raw_secret")"

  if [[ -z "$secret" ]]; then
    return 1
  fi

  if [[ "$secret" == */* ]]; then
    return 1
  fi

  if [[ "$secret" != *.json ]]; then
    secret="${secret}.json"
  fi

  if [[ ! "$secret" =~ ^[A-Za-z0-9._@-]+\.json$ ]]; then
    return 1
  fi

  printf '%s\n' "$secret"
}

resolve_default_codex_secret_dir() {
  if [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
    printf '%s/codex_secrets\n' "${XDG_CONFIG_HOME%/}"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    printf '%s/.config/codex_secrets\n' "${HOME%/}"
    return 0
  fi

  return 1
}

resolve_codex_secret_dir() {
  local configured="${CODEX_SECRET_DIR:-}"
  configured="$(trim "$configured")"

  if [[ -n "$configured" ]]; then
    configured="$(expand_home_path "$configured")"
    printf '%s\n' "$configured"
    return 0
  fi

  configured="$(resolve_default_codex_secret_dir || true)"
  configured="$(expand_home_path "$configured")"
  [[ -n "$configured" ]] || return 1
  printf '%s\n' "$configured"
}

resolve_codex_auth_file_env_value() {
  local configured="${CODEX_AUTH_FILE:-}"
  configured="$(trim "$configured")"
  configured="$(expand_home_path "$configured")"

  if [[ -n "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  if [[ -n "${HOME:-}" ]]; then
    printf '%s/.codex/auth.json\n' "${HOME%/}"
    return 0
  fi

  return 1
}

ensure_codex_auth_file_env() {
  local configured=""
  configured="$(resolve_codex_auth_file_env_value || true)"
  [[ -n "$configured" ]] || return 1
  export CODEX_AUTH_FILE="$configured"
  return 0
}

ensure_codex_secret_dir_env() {
  local configured="${CODEX_SECRET_DIR:-}"
  configured="$(trim "$configured")"

  if [[ -z "$configured" ]]; then
    configured="$(resolve_default_codex_secret_dir || true)"
  fi

  [[ -n "$configured" ]] || return 1
  configured="$(expand_home_path "$configured")"
  export CODEX_SECRET_DIR="$configured"
  printf '%s\n' "$configured"
}

secret_dir_has_saved_json() {
  local secret_dir="${1:-}"
  [[ -n "$secret_dir" ]] || return 1
  [[ -d "$secret_dir" ]] || return 1

  local any_file
  any_file="$(find "$secret_dir" -maxdepth 1 -type f -name '*.json' -print -quit 2>/dev/null || true)"
  [[ -n "$any_file" ]]
}

resolve_codex_auth_file() {
  local clean_output="${1:-}"
  local auth_file=""

  if [[ -n "$clean_output" ]]; then
    auth_file="$(printf '%s\n' "$clean_output" | sed -nE 's|^[^:]*:[[:space:]]*([^[:space:]]+/auth\.json).*|\1|p' | head -n1 || true)"
  fi

  if [[ -z "$auth_file" ]]; then
    auth_file="$(resolve_codex_auth_file_env_value || true)"
  fi

  [[ -n "$auth_file" ]] || return 1
  printf '%s\n' "$auth_file"
}

normalize_use_secret() {
  local raw_secret="$1"
  local secret
  secret="$(trim "$raw_secret")"

  if [[ -z "$secret" ]]; then
    return 1
  fi

  if [[ "$secret" == */* ]]; then
    return 1
  fi

  if [[ "$secret" == *.json ]]; then
    secret="${secret%.json}"
  fi

  if [[ -z "$secret" ]]; then
    return 1
  fi

  if [[ ! "$secret" =~ ^[A-Za-z0-9._@-]+$ ]]; then
    return 1
  fi

  printf '%s\n' "$secret"
}

detect_current_secret_json() {
  local codex_cli
  if ! codex_cli="$(resolve_codex_cli_path)"; then
    return 1
  fi

  local structured_output=""
  local structured_auth_file=""
  if command -v jq >/dev/null 2>&1; then
    set +e
    structured_output="$("$codex_cli" auth current --json 2>/dev/null)"
    set -e
    if [[ -n "$structured_output" ]] && printf '%s\n' "$structured_output" | jq -e 'type == "object"' >/dev/null 2>&1; then
      local structured_matched_json=""
      structured_matched_json="$(printf '%s\n' "$structured_output" | jq -r '.result.matched_secret // .error.details.matched_secret // empty' 2>/dev/null || true)"
      if [[ -n "$structured_matched_json" && "$structured_matched_json" != *.json ]]; then
        structured_matched_json="${structured_matched_json}.json"
      fi
      if [[ -n "$structured_matched_json" && "$structured_matched_json" != "auth.json" ]]; then
        printf '%s\n' "$structured_matched_json"
        return 0
      fi

      structured_auth_file="$(printf '%s\n' "$structured_output" | jq -r '.result.auth_file // .error.details.auth_file // empty' 2>/dev/null || true)"
    fi
  fi

  local output
  set +e
  output="$("$codex_cli" auth current 2>&1)"
  set -e

  local clean_output
  clean_output="$(
    printf '%s\n' "$output" |
      while IFS= read -r line || [[ -n "$line" ]]; do
        strip_ansi "$line"
        printf '\n'
      done
  )"

  local reported_json=""
  reported_json="$(printf '%s\n' "$clean_output" | sed -nE 's/.*matches[[:space:]]+([A-Za-z0-9._@-]+(\.json)?).*/\1/p' | head -n1 || true)"
  if [[ -n "$reported_json" && "$reported_json" != *.json ]]; then
    reported_json="${reported_json}.json"
  fi
  if [[ -n "$reported_json" && "$reported_json" != "auth.json" ]]; then
    printf '%s\n' "$reported_json"
    return 0
  fi

  local auth_file=""
  if [[ -n "$structured_auth_file" ]]; then
    auth_file="$structured_auth_file"
  else
    auth_file="$(resolve_codex_auth_file "$clean_output" || true)"
  fi
  if [[ -z "$auth_file" && -n "${CODEX_AUTH_FILE:-}" ]]; then
    auth_file="${CODEX_AUTH_FILE}"
  fi
  if [[ -n "$auth_file" && -f "$auth_file" ]]; then
    printf 'auth.json\n'
    return 0
  fi

  return 1
}

resolve_current_auth_info() {
  local current_json auth_file auth_email
  current_json="$(detect_current_secret_json || true)"
  auth_file="$(resolve_codex_auth_file "" || true)"

  if [[ -z "$current_json" && -n "$auth_file" && -f "$auth_file" ]]; then
    current_json="auth.json"
  fi
  [[ -n "$current_json" ]] || return 1

  auth_email="-"
  if [[ -n "$auth_file" && -f "$auth_file" ]]; then
    auth_email="$(extract_secret_email_from_file "$auth_file" || true)"
  fi
  [[ -n "$auth_email" ]] || auth_email="-"

  printf '%s\t%s\t%s\n' "$current_json" "$auth_email" "$auth_file"
}

build_diag_account_lookup_map() {
  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_display_cache_paths_for_mode "all-json" || true)"
  [[ -n "$cache_paths" ]] || return 1
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local mode
  mode="$(read_meta_value "$meta_path" mode)"
  [[ "$mode" == "all-json" ]] || return 1

  if ! jq -e '.results | type == "array"' "$output_path" >/dev/null 2>&1; then
    return 1
  fi

  local raw_file
  raw_file="$(mktemp "${TMPDIR:-/tmp}/codex-cxau-sort.raw.XXXXXX")"
  local map_file
  map_file="$(mktemp "${TMPDIR:-/tmp}/codex-cxau-sort.map.XXXXXX")"

  if ! jq -r '.results // [] | .[] | [(.source // ""), (.name // ""), (.summary.weekly_reset_epoch // 9999999999), (.raw_usage.email // ""), (.summary.weekly_reset_local // "-"), (.summary.non_weekly_label // "5h"), (.summary.non_weekly_remaining // "null"), (.summary.weekly_remaining // "null"), ((.summary.non_weekly_reset_epoch // "null") | tostring)] | @tsv' "$output_path" >"$raw_file"; then
    rm -f "$raw_file" "$map_file"
    return 1
  fi

  if [[ ! -s "$raw_file" ]]; then
    rm -f "$raw_file" "$map_file"
    return 1
  fi

  awk -F'\t' '
    function normalize_json_key(v) {
      gsub(/^.*\//, "", v)
      if (v == "") return ""
      if (v !~ /\.json$/) v = v ".json"
      return v
    }

    function normalize_base(v) {
      gsub(/^.*\//, "", v)
      gsub(/\.json$/, "", v)
      return v
    }

    function emit_key(k, epoch, email, weekly, label, non_weekly, weekly_remaining, non_weekly_reset_epoch) {
      if (k == "") return
      if (epoch !~ /^[0-9]+$/) epoch = 9999999999
      if (email == "") email = "-"
      if (weekly == "") weekly = "-"
      if (label == "") label = "5h"
      if (non_weekly == "") non_weekly = "null"
      if (weekly_remaining == "") weekly_remaining = "null"
      if (non_weekly_reset_epoch == "") non_weekly_reset_epoch = "null"
      print k "\t" epoch "\t" email "\t" weekly "\t" label "\t" non_weekly "\t" weekly_remaining "\t" non_weekly_reset_epoch
    }

    {
      src=$1
      name=$2
      epoch=$3
      email=$4
      weekly=$5
      label=$6
      non_weekly=$7
      weekly_remaining=$8
      non_weekly_reset_epoch=$9

      src_key=normalize_json_key(src)
      emit_key(src_key, epoch, email, weekly, label, non_weekly, weekly_remaining, non_weekly_reset_epoch)

      name_key=normalize_json_key(name)
      emit_key(name_key, epoch, email, weekly, label, non_weekly, weekly_remaining, non_weekly_reset_epoch)

      base_key=normalize_base(name)
      if (base_key != "") {
        emit_key(base_key ".json", epoch, email, weekly, label, non_weekly, weekly_remaining, non_weekly_reset_epoch)
      }
    }
  ' "$raw_file" |
    LC_ALL=C sort -t$'\t' -k1,1 -k2,2n |
    awk -F'\t' '!seen[$1]++ { print $1 "\t" $2 "\t" $3 "\t" $4 "\t" $5 "\t" $6 "\t" $7 "\t" $8 }' >"$map_file"
  rm -f "$raw_file"

  if [[ ! -s "$map_file" ]]; then
    rm -f "$map_file"
    return 1
  fi

  printf '%s\n' "$map_file"
}

lookup_diag_account_meta() {
  local map_file="$1"
  local key="$2"
  [[ -n "$map_file" && -f "$map_file" ]] || return 1

  awk -F'\t' -v key="$key" '$1 == key { print $2 "\t" $3 "\t" $4 "\t" $5 "\t" $6 "\t" $7 "\t" $8; exit }' "$map_file"
}

lookup_current_diag_meta() {
  local current_json="${1:-}"
  [[ -n "$current_json" ]] || return 1
  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_display_cache_paths_for_mode "all-json" || true)"
  [[ -n "$cache_paths" ]] || return 1
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local mode
  mode="$(read_meta_value "$meta_path" mode)"
  [[ "$mode" == "all-json" ]] || return 1

  local dataset_filter
  if jq -e '.results | type == "array"' "$output_path" >/dev/null 2>&1; then
    dataset_filter='.results // []'
  elif jq -e '.result | type == "object"' "$output_path" >/dev/null 2>&1; then
    dataset_filter='[.result]'
  else
    return 1
  fi

  local matched_meta
  matched_meta="$(
    jq -r --arg current_json "$current_json" '
      '"$dataset_filter"' |
      map(
        . as $row |
        (
          ($row.source // "")
          | tostring
          | split("/")[-1]
          | if . == "" then "" elif endswith(".json") then . else . + ".json" end
        ) as $source_json |
        (
          ($row.name // "")
          | tostring
          | split("/")[-1]
          | if . == "" then "" elif endswith(".json") then . else . + ".json" end
        ) as $name_json |
        {
          key_candidates: [$source_json, $name_json],
          weekly_reset_epoch: ($row.summary.weekly_reset_epoch // 9999999999),
          email: ($row.raw_usage.email // "-"),
          weekly_reset_local: ($row.summary.weekly_reset_local // "-"),
          non_weekly_label: ($row.summary.non_weekly_label // "5h"),
          non_weekly_remaining: ($row.summary.non_weekly_remaining // "null"),
          weekly_remaining: ($row.summary.weekly_remaining // "null"),
          non_weekly_reset_epoch: ($row.summary.non_weekly_reset_epoch // "null")
        }
      ) |
      map(select(.key_candidates | index($current_json))) |
      sort_by(.weekly_reset_epoch) |
      .[0] // empty |
      [
        (.weekly_reset_epoch | tostring),
        .email,
        .weekly_reset_local,
        .non_weekly_label,
        (.non_weekly_remaining | tostring),
        (.weekly_remaining | tostring),
        (.non_weekly_reset_epoch | tostring)
      ] |
      @tsv
    ' "$output_path" 2>/dev/null || true
  )"

  if [[ -n "$matched_meta" ]]; then
    printf '%s\n' "$matched_meta"
    return 0
  fi

  if [[ "$current_json" != "auth.json" ]]; then
    return 1
  fi

  jq -r '
    '"$dataset_filter"' |
    sort_by((.summary.weekly_reset_epoch // 9999999999), (.name // "")) |
    .[0] // empty |
    [
      ((.summary.weekly_reset_epoch // 9999999999) | tostring),
      (.raw_usage.email // "-"),
      (.summary.weekly_reset_local // "-"),
      (.summary.non_weekly_label // "5h"),
      ((.summary.non_weekly_remaining // "null") | tostring),
      ((.summary.weekly_remaining // "null") | tostring),
      ((.summary.non_weekly_reset_epoch // "null") | tostring)
    ] |
    @tsv
  ' "$output_path" 2>/dev/null || true
}

extract_secret_email_from_file() {
  local file_path="$1"
  [[ -f "$file_path" ]] || return 1
  if ! command -v jq >/dev/null 2>&1; then
    return 1
  fi

  local email
  email="$(
    jq -r '.. | objects | .email? // empty' "$file_path" 2>/dev/null |
      grep -E -m1 '^[^[:space:]@]+@[^[:space:]@]+\.[^[:space:]@]+$' || true
  )"
  [[ -n "$email" ]] || return 1
  printf '%s\n' "$email"
}

build_use_subtitle() {
  local email="$1"
  local weekly_reset="$2"
  local command_hint="$3"

  [[ -n "$email" ]] || email="-"
  [[ -n "$weekly_reset" ]] || weekly_reset="-"
  printf '%s | reset %s | %s\n' "$email" "$weekly_reset" "$command_hint"
}

build_use_usage_suffix() {
  local label="$1"
  local non_weekly="$2"
  local weekly="$3"
  local non_weekly_reset_epoch="${4:-}"
  local weekly_reset_epoch="${5:-}"

  [[ -n "$label" && "$label" != "-" ]] || label="5h"

  local non_weekly_text weekly_text
  non_weekly_text="$(build_usage_metric_text "$label" "$non_weekly" "$non_weekly_reset_epoch" "$label")"
  weekly_text="$(build_usage_metric_text "weekly" "$weekly" "$weekly_reset_epoch" "")"

  printf '%s | %s\n' "$non_weekly_text" "$weekly_text"
}

build_use_title() {
  local base="$1"
  local label="${2:-}"
  local non_weekly="${3:-}"
  local weekly="${4:-}"
  local non_weekly_reset_epoch="${5:-}"
  local weekly_reset_epoch="${6:-}"

  if [[ -z "$label" && -z "$non_weekly" && -z "$weekly" && -z "$non_weekly_reset_epoch" && -z "$weekly_reset_epoch" ]]; then
    printf '%s\n' "$base"
    return
  fi

  printf '%s | %s\n' "$base" "$(build_use_usage_suffix "$label" "$non_weekly" "$weekly" "$non_weekly_reset_epoch" "$weekly_reset_epoch")"
}

handle_use_query() {
  local raw_query="$1"
  local remainder
  local secret=""
  local token
  local seen_extra=0

  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?use[[:space:]]*//I')"

  # Bash 3.2 + set -u can treat empty arrays as unbound; iterate words directly.
  # shellcheck disable=SC2086
  for token in $remainder; do
    if [[ -z "$secret" ]]; then
      secret="$token"
    else
      seen_extra=1
    fi
  done

  if [[ "$seen_extra" -eq 1 ]]; then
    emit_item \
      "Invalid auth use arguments" \
      "Usage: use <secret> (example: use alpha)" \
      "" \
      false \
      "use "
    return
  fi

  if [[ -n "$secret" ]]; then
    local normalized_secret
    if ! normalized_secret="$(normalize_use_secret "$secret")"; then
      emit_item \
        "Invalid secret name" \
        "Use basename only, allowed chars: A-Z a-z 0-9 . _ @ - (optional .json suffix)." \
        "" \
        false \
        "use "
      return
    fi

    emit_item \
      "Run auth use ${normalized_secret}" \
      "Switch active auth to ${normalized_secret}.json" \
      "use::${normalized_secret}" \
      true \
      "use ${normalized_secret}"
    return
  fi

  ensure_diag_cache_ready_for_mode "all-json" || true

  local current_json=""
  local current_auth_email="-"
  local current_info
  current_info="$(resolve_current_auth_info || true)"
  if [[ -n "$current_info" ]]; then
    IFS=$'\t' read -r current_json current_auth_email _ <<<"$current_info"
  else
    current_json="$(detect_current_secret_json || true)"
  fi
  [[ -n "$current_auth_email" ]] || current_auth_email="-"

  local current_cached_email=""
  local current_cached_weekly="-"
  if [[ -n "$current_json" ]]; then
    local current_cached_meta
    current_cached_meta="$(lookup_current_diag_meta "$current_json" || true)"
    if [[ -n "$current_cached_meta" ]]; then
      IFS=$'\t' read -r _current_cached_weekly_epoch current_cached_email current_cached_weekly _current_cached_label _current_cached_non_weekly _current_cached_weekly_remaining _current_cached_non_weekly_reset_epoch <<<"$current_cached_meta"
    fi
  fi
  if [[ -z "$current_cached_email" || "$current_cached_email" == "-" ]]; then
    current_cached_email="$current_auth_email"
  fi

  local secret_dir
  if ! secret_dir="$(resolve_codex_secret_dir)"; then
    if [[ -n "$current_json" ]]; then
      local current_secret
      current_secret="${current_json%.json}"
      if [[ "$current_json" == "auth.json" ]]; then
        emit_item \
          "Current: ${current_json}" \
          "$(build_use_subtitle "${current_cached_email:-"-"}" "${current_cached_weekly:-"-"}" "Active auth file detected (no CODEX_SECRET_DIR list).")" \
          "" \
          false \
          "use "
      else
        emit_item \
          "Current: ${current_json}" \
          "$(build_use_subtitle "${current_cached_email:-"-"}" "${current_cached_weekly:-"-"}" "Press Enter to run codex-cli auth use ${current_secret}")" \
          "use::${current_secret}" \
          true \
          "use ${current_secret}"
      fi
    else
      emit_item \
        "Current: unknown" \
        "Unable to parse codex-cli auth current output." \
        "" \
        false \
        "use "
    fi

    emit_item \
      "No secret directory configured" \
      "Set CODEX_SECRET_DIR or HOME/XDG_CONFIG_HOME to list *.json secrets." \
      "" \
      false \
      "use "
    return
  fi

  if [[ ! -d "$secret_dir" ]]; then
    if [[ -n "$current_json" ]]; then
      local current_secret
      current_secret="${current_json%.json}"
      if [[ "$current_json" == "auth.json" ]]; then
        emit_item \
          "Current: ${current_json}" \
          "$(build_use_subtitle "${current_cached_email:-"-"}" "${current_cached_weekly:-"-"}" "Active auth file detected (no saved secrets yet).")" \
          "" \
          false \
          "use "
      else
        emit_item \
          "Current: ${current_json}" \
          "$(build_use_subtitle "${current_cached_email:-"-"}" "${current_cached_weekly:-"-"}" "Press Enter to run codex-cli auth use ${current_secret}")" \
          "use::${current_secret}" \
          true \
          "use ${current_secret}"
      fi
    else
      emit_item \
        "Current: unknown" \
        "Unable to parse codex-cli auth current output." \
        "" \
        false \
        "use "
    fi

    emit_item \
      "No secret directory found: ${secret_dir}" \
      "Create it and add *.json secrets (for example: cx save team-alpha.json)." \
      "" \
      false \
      "use "
    return
  fi

  local files=()
  local file
  while IFS= read -r file; do
    files+=("$file")
  done < <(find "$secret_dir" -maxdepth 1 -type f -name '*.json' -print 2>/dev/null | sed -E 's|.*/||' | LC_ALL=C sort)

  local account_lookup_file=""
  account_lookup_file="$(build_diag_account_lookup_map || true)"

  if [[ -n "$current_json" ]]; then
    local current_secret current_meta current_email current_weekly
    current_secret="${current_json%.json}"
    current_email="${current_cached_email:-"-"}"
    current_weekly="${current_cached_weekly:-"-"}"
    current_meta="$(lookup_diag_account_meta "$account_lookup_file" "$current_json" || true)"
    if [[ -n "$current_meta" ]]; then
      IFS=$'\t' read -r _current_weekly_epoch current_email current_weekly _current_label _current_non_weekly _current_weekly_remaining _current_non_weekly_reset_epoch <<<"$current_meta"
    fi
    if [[ "$current_json" != "auth.json" && (-z "${current_email:-}" || "$current_email" == "-") ]]; then
      current_email="$(extract_secret_email_from_file "${secret_dir%/}/${current_json}" || true)"
    fi
    if [[ -z "${current_email:-}" ]]; then
      current_email="${current_auth_email:-"-"}"
    fi
    if [[ "$current_json" == "auth.json" ]]; then
      emit_item \
        "Current: ${current_json}" \
        "$(build_use_subtitle "${current_email:-"-"}" "${current_weekly:-"-"}" "Active auth file (not mapped to saved *.json).")" \
        "" \
        false \
        "use "
    else
      emit_item \
        "Current: ${current_json}" \
        "$(build_use_subtitle "${current_email:-"-"}" "${current_weekly:-"-"}" "Press Enter to run codex-cli auth use ${current_secret}")" \
        "use::${current_secret}" \
        true \
        "use ${current_secret}"
    fi
  else
    emit_item \
      "Current: unknown" \
      "Unable to parse codex-cli auth current output." \
      "" \
      false \
      "use "
  fi

  if [[ "${#files[@]}" -eq 0 ]]; then
    if [[ -n "$account_lookup_file" && -f "$account_lookup_file" ]]; then
      rm -f "$account_lookup_file"
    fi
    emit_item \
      "No saved secrets (*.json)" \
      "Use cx save <name>.json first, then choose with cxau." \
      "" \
      false \
      "use "
    return
  fi

  local sorted_files=()
  if [[ -n "$account_lookup_file" && -f "$account_lookup_file" ]]; then
    local ranking_file
    ranking_file="$(mktemp "${TMPDIR:-/tmp}/codex-cxau-rank.XXXXXX")"
    for file in "${files[@]}"; do
      local meta epoch
      meta="$(lookup_diag_account_meta "$account_lookup_file" "$file" || true)"
      epoch="$(printf '%s\n' "$meta" | awk -F'\t' 'NR==1 { print $1 }')"
      if [[ ! "$epoch" =~ ^[0-9]+$ ]]; then
        epoch="9999999999"
      fi
      printf '%s\t%s\n' "$epoch" "$file" >>"$ranking_file"
    done
    while IFS=$'\t' read -r _ranked_epoch ranked_file || [[ -n "${ranked_file:-}" ]]; do
      [[ -n "${ranked_file:-}" ]] || continue
      sorted_files+=("$ranked_file")
    done < <(LC_ALL=C sort -t$'\t' -k1,1n -k2,2 "$ranking_file")
    rm -f "$ranking_file"
  else
    sorted_files=("${files[@]}")
  fi

  for file in "${sorted_files[@]}"; do
    local use_secret account_meta account_email account_weekly account_label account_non_weekly account_weekly_remaining account_weekly_epoch account_non_weekly_reset_epoch
    use_secret="${file%.json}"
    account_meta="$(lookup_diag_account_meta "$account_lookup_file" "$file" || true)"
    if [[ -n "$account_meta" ]]; then
      IFS=$'\t' read -r account_weekly_epoch account_email account_weekly account_label account_non_weekly account_weekly_remaining account_non_weekly_reset_epoch <<<"$account_meta"
    fi
    if [[ -z "${account_email:-}" ]]; then
      account_email="$(extract_secret_email_from_file "${secret_dir%/}/${file}" || true)"
    fi
    emit_item \
      "$(build_use_title "$file" "${account_label:-}" "${account_non_weekly:-}" "${account_weekly_remaining:-}" "${account_non_weekly_reset_epoch:-}" "${account_weekly_epoch:-}")" \
      "$(build_use_subtitle "${account_email:-}" "${account_weekly:-}" "Run codex-cli auth use ${use_secret}")" \
      "use::${use_secret}" \
      true \
      "use ${use_secret}"
  done

  if [[ -n "$account_lookup_file" && -f "$account_lookup_file" ]]; then
    rm -f "$account_lookup_file"
  fi
}

handle_login_query() {
  local lower_query="$1"
  local mode="browser"

  local has_api=0
  local has_device=0

  if [[ "$lower_query" == *"--api-key"* || "$lower_query" == *" api-key"* || "$lower_query" == *" apikey"* || "$lower_query" == *" api"* ]]; then
    has_api=1
  fi

  if [[ "$lower_query" == *"--device-code"* || "$lower_query" == *" device-code"* || "$lower_query" == *" device"* ]]; then
    has_device=1
  fi

  if [[ "$has_api" -eq 1 && "$has_device" -eq 1 ]]; then
    emit_item \
      "Invalid login mode selection" \
      "Use either --api-key or --device-code, not both." \
      "" \
      false \
      "login"
    return
  fi

  if [[ "$has_api" -eq 1 ]]; then
    mode="api-key"
  elif [[ "$has_device" -eq 1 ]]; then
    mode="device-code"
  fi

  case "$mode" in
  api-key)
    emit_item \
      "Run auth login --api-key" \
      "Login using API key flow." \
      "login::api-key" \
      true \
      "login --api-key"
    ;;
  device-code)
    emit_item \
      "Run auth login --device-code" \
      "Login using ChatGPT device-code flow." \
      "login::device-code" \
      true \
      "login --device-code"
    ;;
  *)
    emit_item \
      "Run auth login (browser)" \
      "Login using ChatGPT browser flow." \
      "login::browser" \
      true \
      "login"
    ;;
  esac
}

handle_save_query() {
  local raw_query="$1"
  local remainder
  local yes_flag=0
  local secret=""
  local token
  local seen_extra=0

  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?save[[:space:]]*//I')"

  # shellcheck disable=SC2206
  local parts=($remainder)
  for token in "${parts[@]}"; do
    case "$token" in
    --yes | -y)
      yes_flag=1
      ;;
    *)
      if [[ -z "$secret" ]]; then
        secret="$token"
      else
        seen_extra=1
      fi
      ;;
    esac
  done

  if [[ "$seen_extra" -eq 1 ]]; then
    emit_item \
      "Invalid auth save arguments" \
      "Usage: save [--yes] <secret.json>" \
      "" \
      false \
      "save "
    return
  fi

  if [[ -z "$secret" ]]; then
    emit_item \
      "Missing secret file name" \
      "Usage: save [--yes] <secret.json> (example: save team-alpha.json)" \
      "" \
      false \
      "save "
    return
  fi

  local normalized_secret
  if ! normalized_secret="$(normalize_save_secret "$secret")"; then
    emit_item \
      "Invalid secret file name" \
      "Use basename only, allowed chars: A-Z a-z 0-9 . _ @ - and suffix .json" \
      "" \
      false \
      "save "
    return
  fi

  emit_item \
    "Run auth save ${normalized_secret}" \
    "Save active auth into CODEX_SECRET_DIR/${normalized_secret}" \
    "save::${normalized_secret}::${yes_flag}" \
    true \
    "save ${normalized_secret}"

  if [[ "$yes_flag" -eq 0 ]]; then
    emit_item \
      "Run auth save --yes ${normalized_secret}" \
      "Force overwrite if file already exists." \
      "save::${normalized_secret}::1" \
      true \
      "save --yes ${normalized_secret}"
  fi
}

handle_remove_query() {
  local raw_query="$1"
  local remainder
  local yes_flag=0
  local secret=""
  local token
  local seen_extra=0

  remainder="$(printf '%s' "$raw_query" | sed -E 's/^[[:space:]]*(auth[[:space:]]+)?remove[[:space:]]*//I')"

  # shellcheck disable=SC2206
  local parts=($remainder)
  for token in "${parts[@]}"; do
    case "$token" in
    --yes | -y)
      yes_flag=1
      ;;
    *)
      if [[ -z "$secret" ]]; then
        secret="$token"
      else
        seen_extra=1
      fi
      ;;
    esac
  done

  if [[ "$seen_extra" -eq 1 ]]; then
    emit_item \
      "Invalid auth remove arguments" \
      "Usage: remove [--yes] <secret.json>" \
      "" \
      false \
      "remove "
    return
  fi

  if [[ -z "$secret" ]]; then
    emit_item \
      "Missing secret file name" \
      "Usage: remove [--yes] <secret.json> (example: remove team-alpha.json)" \
      "" \
      false \
      "remove "
    return
  fi

  local normalized_secret
  if ! normalized_secret="$(normalize_save_secret "$secret")"; then
    emit_item \
      "Invalid secret file name" \
      "Use basename only, allowed chars: A-Z a-z 0-9 . _ @ - and suffix .json" \
      "" \
      false \
      "remove "
    return
  fi

  local secret_dir
  if ! secret_dir="$(resolve_codex_secret_dir)"; then
    emit_item \
      "No secret directory configured" \
      "Set CODEX_SECRET_DIR or HOME/XDG_CONFIG_HOME before removing secrets." \
      "" \
      false \
      "remove "
    return
  fi

  if [[ ! -d "$secret_dir" ]]; then
    emit_item \
      "No secret directory found: ${secret_dir}" \
      "Create it first and save a secret before running remove." \
      "" \
      false \
      "remove "
    return
  fi

  local secret_path="${secret_dir%/}/${normalized_secret}"
  if [[ ! -f "$secret_path" ]]; then
    emit_item \
      "Secret file not found" \
      "No file: ${secret_path}" \
      "" \
      false \
      "remove ${normalized_secret}"
    return
  fi

  emit_item \
    "Run auth remove ${normalized_secret}" \
    "Remove ${normalized_secret} from CODEX_SECRET_DIR" \
    "remove::${normalized_secret}::${yes_flag}" \
    true \
    "remove ${normalized_secret}"

  if [[ "$yes_flag" -eq 0 ]]; then
    emit_item \
      "Run auth remove --yes ${normalized_secret}" \
      "Skip interactive confirmation when removing ${normalized_secret}." \
      "remove::${normalized_secret}::1" \
      true \
      "remove --yes ${normalized_secret}"
  fi
}

emit_latest_diag_result_items_inline() {
  local expected_mode="${1:-}"
  local cache_paths meta_path output_path
  cache_paths="$(resolve_diag_display_cache_paths_for_mode "$expected_mode" || true)"

  if [[ -z "$cache_paths" ]]; then
    emit_item \
      "Latest diag result unavailable" \
      "Diag refresh failed or cache is unavailable." \
      "" \
      false \
      "diag result"
    return
  fi
  IFS=$'\t' read -r meta_path output_path <<<"$cache_paths"

  local mode
  mode="$(read_meta_value "$meta_path" mode)"
  [[ -n "$mode" ]] || mode="unknown"

  if [[ -n "$expected_mode" && "$mode" != "$expected_mode" ]]; then
    emit_item \
      "Latest diag result unavailable (${expected_mode})" \
      "Latest cache mode is ${mode}; refresh did not produce ${expected_mode} data." \
      "" \
      false \
      "diag result"
    return
  fi

  local preview_query="diag result"
  if [[ "$mode" == "all-json" ]]; then
    preview_query="diag result all-json"
  fi

  emit_diag_result_items "$preview_query"
}

emit_current_auth_hint_item() {
  local current_info
  current_info="$(resolve_current_auth_info || true)"
  [[ -n "$current_info" ]] || return 0

  local current_json current_email _current_auth_file
  IFS=$'\t' read -r current_json current_email _current_auth_file <<<"$current_info"
  [[ -n "$current_json" ]] || return 0
  [[ -n "$current_email" ]] || current_email="-"

  local current_weekly_reset="-"
  local current_cached_meta
  current_cached_meta="$(lookup_current_diag_meta "$current_json" || true)"
  if [[ -n "$current_cached_meta" ]]; then
    local cached_email cached_weekly_reset
    IFS=$'\t' read -r _cached_epoch cached_email cached_weekly_reset _cached_label _cached_non_weekly _cached_weekly_remaining _cached_non_weekly_reset_epoch <<<"$current_cached_meta"
    if [[ -n "$cached_email" && "$cached_email" != "-" ]]; then
      current_email="$cached_email"
    fi
    if [[ -n "$cached_weekly_reset" && "$cached_weekly_reset" != "-" ]]; then
      current_weekly_reset="$cached_weekly_reset"
    fi
  fi

  local subtitle="$current_email"
  if [[ -n "$current_weekly_reset" && "$current_weekly_reset" != "-" ]]; then
    subtitle="${current_email} | reset ${current_weekly_reset}"
  fi

  emit_item \
    "Current: ${current_json}" \
    "${subtitle}" \
    "" \
    false \
    ""
}

handle_diag_query() {
  local lower_query="$1"
  local diag_remainder
  diag_remainder="$(printf '%s' "$lower_query" | sed -E 's/^[[:space:]]*diag([[:space:]]+|$)//I')"
  diag_remainder="$(trim "$diag_remainder")"
  if [[ -n "$diag_remainder" ]]; then
    local diag_first_token
    diag_first_token="${diag_remainder%%[[:space:]]*}"
    local normalized_token="${diag_first_token#--}"
    normalized_token="${normalized_token#-}"
    if [[ "${#normalized_token}" -lt 2 ]]; then
      emit_item \
        "Keep typing (2+ chars)" \
        "Type at least 2 characters to disambiguate diag options." \
        "" \
        false \
        "diag "
      return
    fi
  fi

  if [[ "$lower_query" == "diag result"* ]]; then
    local result_mode=""
    if [[ "$lower_query" == *"all-json"* || "$lower_query" == *"--all-json"* ]]; then
      result_mode="all-json"
    else
      result_mode="$(resolve_latest_diag_cache_mode || true)"
      [[ -n "$result_mode" ]] || result_mode="default"
    fi
    ensure_diag_cache_ready_for_mode "$result_mode" || true
    emit_diag_result_items "$lower_query"
    return
  fi

  local mode="default"
  local resolved_secret_dir=""
  local has_saved_secrets=0
  resolved_secret_dir="$(resolve_codex_secret_dir || true)"
  if secret_dir_has_saved_json "$resolved_secret_dir"; then
    has_saved_secrets=1
  fi

  if [[ "$lower_query" == *"all-json"* || "$lower_query" == *"--all-json"* ]]; then
    mode="all-json"
  elif [[ "$lower_query" == *"async"* ]]; then
    mode="async"
  elif [[ "$lower_query" == *"one-line"* || "$lower_query" == *" oneline"* || "$lower_query" == *" one line"* ]]; then
    mode="one-line"
  elif [[ "$lower_query" == *"cached"* ]]; then
    mode="cached"
  elif [[ "$lower_query" == *" all"* || "$lower_query" == *"--all"* ]]; then
    mode="all"
  fi

  local auto_refresh_mode=""
  auto_refresh_mode="$(resolve_diag_auto_refresh_mode_for_query "$lower_query" || true)"
  local diag_alias
  diag_alias="$(to_lower "${CODEX_DIAG_ALIAS:-}")"
  local is_diag_alias=0
  if [[ "$diag_alias" == "cxd" || "$diag_alias" == "cxda" ]]; then
    is_diag_alias=1
  fi

  case "$mode" in
  all-json)
    if [[ "$has_saved_secrets" -eq 1 ]]; then
      emit_item \
        "Run diag rate-limits --all --json (parsed)" \
        "Parse JSON and render one row per account." \
        "diag::all-json" \
        true \
        "diag all-json"
    else
      emit_item \
        "Run diag rate-limits --json (parsed)" \
        "No saved secrets; fallback to current auth.json diagnostics." \
        "diag::all-json" \
        true \
        "diag all-json"
    fi
    ;;
  cached)
    emit_item \
      "Run diag rate-limits --cached" \
      "Cached diagnostics only; no network refresh." \
      "diag::cached" \
      true \
      "diag cached"
    ;;
  one-line)
    emit_item \
      "Run diag rate-limits --one-line" \
      "Compact one-line diagnostics output." \
      "diag::one-line" \
      true \
      "diag one-line"
    ;;
  all)
    if [[ "$has_saved_secrets" -eq 1 ]]; then
      emit_item \
        "Run diag rate-limits --all" \
        "Query all secrets under CODEX_SECRET_DIR." \
        "diag::all" \
        true \
        "diag all"
    else
      emit_item \
        "Run diag rate-limits (auth.json fallback)" \
        "No saved secrets; run diagnostics for current auth only." \
        "diag::all" \
        true \
        "diag all"
    fi
    ;;
  async)
    if [[ "$has_saved_secrets" -eq 1 ]]; then
      emit_item \
        "Run diag rate-limits --all --async --jobs 4" \
        "Concurrent diagnostics across secrets." \
        "diag::async" \
        true \
        "diag async"
    else
      emit_item \
        "Run diag rate-limits (auth.json fallback)" \
        "No saved secrets; async all-account mode is unavailable." \
        "diag::async" \
        true \
        "diag async"
    fi
    ;;
  *)
    emit_item \
      "Run diag rate-limits --json (parsed)" \
      "Default diagnostics for current secret via JSON output." \
      "diag::default" \
      true \
      "diag"
    ;;
  esac

  if [[ "$is_diag_alias" -ne 1 ]]; then
    emit_item \
      "Also available: --cached / --one-line / --all / all-json / async" \
      "Type diag cached, diag one-line, diag all, diag all-json, or diag async." \
      "" \
      false \
      "diag "
  fi

  if [[ -n "$auto_refresh_mode" ]]; then
    ensure_diag_cache_ready_for_mode "$auto_refresh_mode" || true
  fi

  emit_current_auth_hint_item
  emit_latest_diag_result_items_inline "$auto_refresh_mode"
}

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(trim "$query")"
lower_query_raw="$(to_lower "$trimmed_query")"

show_assessment=0
if is_truthy "${CODEX_SHOW_ASSESSMENT:-0}"; then
  show_assessment=1
fi

if query_has_assessment_flag "$lower_query_raw"; then
  show_assessment=1
  trimmed_query="$(strip_assessment_flags "$trimmed_query")"
fi

lower_query="$(to_lower "$trimmed_query")"

begin_items
ensure_codex_auth_file_env >/dev/null 2>&1 || true
ensure_codex_secret_dir_env >/dev/null 2>&1 || true
emit_runtime_status

if [[ -z "$trimmed_query" ]]; then
  if [[ "$show_assessment" -eq 1 ]]; then
    emit_assessment_items
  fi
  emit_default_action_items
  end_items
  exit 0
fi

if [[ "$lower_query" == "help" || "$lower_query" == "?" || "$lower_query" == "eval" || "$lower_query" == "assessment" || "$lower_query" == "features" ]]; then
  if [[ "$show_assessment" -eq 1 || "$lower_query" == "eval" || "$lower_query" == "assessment" || "$lower_query" == "features" ]]; then
    emit_assessment_items
  fi
  emit_default_action_items
  end_items
  exit 0
fi

if [[ "$lower_query" == "auth" ]]; then
  emit_auth_action_items
  end_items
  exit 0
fi

if [[ "$lower_query" == login* || "$lower_query" == auth\ login* ]]; then
  handle_login_query "$lower_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == use* || "$lower_query" == auth\ use* ]]; then
  handle_use_query "$trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == save* || "$lower_query" == auth\ save* ]]; then
  handle_save_query "$trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == remove* || "$lower_query" == auth\ remove* ]]; then
  handle_remove_query "$trimmed_query"
  end_items
  exit 0
fi

if [[ "$lower_query" == --yes* || "$lower_query" == -y* ]]; then
  handle_save_query "save ${trimmed_query}"
  end_items
  exit 0
fi

if [[ "$lower_query" == diag* ]]; then
  handle_diag_query "$lower_query"
  end_items
  exit 0
fi

emit_item \
  "Unknown command: ${trimmed_query}" \
  "Try: login, use <secret>, save/remove <secret.json>, diag, or help (--assessment optional)." \
  "" \
  false \
  "help"
emit_default_action_items
end_items

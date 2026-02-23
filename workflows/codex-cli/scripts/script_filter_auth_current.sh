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

if ! declare -F sfqp_is_short_query >/dev/null 2>&1; then
  sfqp_is_short_query() {
    local query="${1-}"
    local min_chars="${2:-2}"
    [[ "$min_chars" =~ ^[0-9]+$ ]] || min_chars=2
    local trimmed
    trimmed="$(sfqp_trim "$query")"
    [[ "${#trimmed}" -lt "$min_chars" ]]
  }
fi

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
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

begin_items() {
  ITEM_COUNT=0
  printf '{"items":['
}

emit_item() {
  local title="$1"
  local subtitle="${2-}"
  local valid="${3:-false}"

  if [[ "$ITEM_COUNT" -gt 0 ]]; then
    printf ','
  fi

  printf '{"title":"%s","subtitle":"%s","valid":%s}' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")" \
    "$valid"

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
      "codex-cli binary not found (re-import workflow, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version})."
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
  return 0
}

compact_text() {
  local text="${1-}"
  printf '%s' "$text" | tr '\n' ' ' | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//'
}

title_from_json_output() {
  local payload="$1"
  local matched_secret=""
  local auth_file=""

  matched_secret="$(printf '%s\n' "$payload" | jq -r '.result.matched_secret // .error.details.matched_secret // empty' 2>/dev/null || true)"
  auth_file="$(printf '%s\n' "$payload" | jq -r '.result.auth_file // .error.details.auth_file // empty' 2>/dev/null || true)"

  if [[ -n "$matched_secret" ]]; then
    if [[ "$matched_secret" != *.json ]]; then
      matched_secret="${matched_secret}.json"
    fi
    printf 'Current: %s\n' "$matched_secret"
    return 0
  fi

  if [[ -n "$auth_file" ]]; then
    printf 'Current: auth.json\n'
    return 0
  fi

  printf 'Current: unknown\n'
}

subtitle_from_json_output() {
  local payload="$1"
  local ok match_mode auth_file

  ok="$(printf '%s\n' "$payload" | jq -r '.ok // false' 2>/dev/null || true)"
  match_mode="$(printf '%s\n' "$payload" | jq -r '.result.match_mode // .error.details.match_mode // empty' 2>/dev/null || true)"
  auth_file="$(printf '%s\n' "$payload" | jq -r '.result.auth_file // .error.details.auth_file // empty' 2>/dev/null || true)"

  [[ -n "$match_mode" ]] || match_mode="-"
  [[ -n "$auth_file" ]] || auth_file="-"
  printf 'ok=%s | match_mode=%s | auth_file=%s\n' "$ok" "$match_mode" "$auth_file"
}

begin_items

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(sfqp_trim "$query")"
if [[ -n "$trimmed_query" ]] && sfqp_is_short_query "$trimmed_query" 2; then
  emit_item \
    "Keep typing (2+ chars)" \
    "Type at least 2 characters before running auth current." \
    false
  end_items
  exit 0
fi

ensure_codex_auth_file_env >/dev/null 2>&1 || true
ensure_codex_secret_dir_env >/dev/null 2>&1 || true
codex_cli=""
if ! codex_cli="$(resolve_codex_cli_path)"; then
  emit_item \
    "codex-cli runtime missing" \
    "Re-import workflow, set CODEX_CLI_BIN, or install ${codex_cli_pinned_crate} ${codex_cli_pinned_version}."
  end_items
  exit 0
fi

set +e
output="$("$codex_cli" auth current --json 2>&1)"
rc=$?
set -e

if command -v jq >/dev/null 2>&1 && [[ -n "$output" ]] && printf '%s\n' "$output" | jq -e 'type == "object"' >/dev/null 2>&1; then
  emit_item \
    "$(title_from_json_output "$output")" \
    "$(subtitle_from_json_output "$output")"
  emit_item \
    "Raw: codex-cli auth current --json (rc=${rc})" \
    "$(compact_text "$output")"
else
  if [[ "$rc" -eq 0 ]]; then
    emit_item \
      "auth current --json returned non-JSON" \
      "$(compact_text "$output")"
  else
    emit_item \
      "auth current --json failed (rc=${rc})" \
      "$(compact_text "$output")"
  fi
fi

end_items

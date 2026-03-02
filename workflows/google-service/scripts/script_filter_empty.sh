#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

emit_item_json() {
  local title="$1"
  local subtitle="$2"
  local autocomplete="${3-}"

  printf '{"items":[{"title":"%s","subtitle":"%s","valid":false' \
    "$(json_escape "$title")" \
    "$(json_escape "$subtitle")"

  if [[ -n "$autocomplete" ]]; then
    printf ',"autocomplete":"%s"' "$(json_escape "$autocomplete")"
  fi

  printf '}]}'
}

sf_trim() {
  local value="${1-}"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
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

resolve_workflow_data_dir() {
  local candidate
  for candidate in \
    "${ALFRED_WORKFLOW_DATA:-}" \
    "${ALFRED_WORKFLOW_CACHE:-}"; do
    if [[ -n "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "${TMPDIR:-/tmp}/nils-google-service-workflow"
}

read_active_account() {
  command -v jq >/dev/null 2>&1 || return 1

  local data_dir
  data_dir="$(resolve_workflow_data_dir)"
  local active_file="$data_dir/active-account.v1.json"
  [[ -f "$active_file" ]] || return 1

  local active
  active="$(jq -r '.active_account // empty' "$active_file" 2>/dev/null || true)"
  [[ -n "$active" ]] || return 1
  printf '%s\n' "$active"
}

resolve_google_cli() {
  local configured="${GOOGLE_CLI_BIN:-}"
  configured="$(sf_trim "$configured")"
  configured="$(expand_home_path "$configured")"
  if [[ -n "$configured" && -x "$configured" ]]; then
    printf '%s\n' "$configured"
    return 0
  fi

  local packaged="$script_dir/../bin/google-cli"
  if [[ -x "$packaged" ]]; then
    printf '%s\n' "$packaged"
    return 0
  fi

  local repo_root
  repo_root="$(cd "$script_dir/../../.." && pwd)"

  local release_bin="$repo_root/target/release/google-cli"
  if [[ -x "$release_bin" ]]; then
    printf '%s\n' "$release_bin"
    return 0
  fi

  local debug_bin="$repo_root/target/debug/google-cli"
  if [[ -x "$debug_bin" ]]; then
    printf '%s\n' "$debug_bin"
    return 0
  fi

  if command -v google-cli >/dev/null 2>&1; then
    command -v google-cli
    return 0
  fi

  return 1
}

read_default_account() {
  command -v jq >/dev/null 2>&1 || return 1

  local google_cli
  google_cli="$(resolve_google_cli || true)"
  [[ -n "$google_cli" ]] || return 1

  local output
  output="$("$google_cli" --json auth list 2>/dev/null || true)"
  [[ -n "$output" ]] || return 1

  local account
  account="$(printf '%s\n' "$output" | jq -r '
    if type == "object" and has("result") then
      (.result.default_account // empty)
    else
      (.default_account // empty)
    end
  ' 2>/dev/null || true)"
  [[ -n "$account" ]] || return 1
  printf '%s\n' "$account"
}

active_account="$(read_active_account || true)"
if [[ -n "$active_account" ]]; then
  emit_item_json "Current account: ${active_account}" "Workflow active account" "gsa switch "
  exit 0
fi

default_account="$(read_default_account || true)"
if [[ -n "$default_account" ]]; then
  emit_item_json "Current account: ${default_account}" "google-cli default account (active not set)" "gsa switch "
  exit 0
fi

emit_item_json "Current account: (none)" "Run gsa login or gsa switch to set account" "gsa "

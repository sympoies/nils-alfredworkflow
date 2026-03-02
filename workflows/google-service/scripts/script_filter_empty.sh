#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

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
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}'
  exit 0
fi

# shellcheck disable=SC1090
source "$helper_loader"

load_helper_or_exit() {
  local helper_name="$1"
  if ! wfhl_source_helper "$script_dir" "$helper_name" auto; then
    wfhl_emit_missing_helper_item_json "$helper_name"
    exit 0
  fi
}

load_helper_or_exit "workflow_cli_resolver.sh"

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
  wfcr_resolve_binary \
    "GOOGLE_CLI_BIN" \
    "$script_dir/../bin/google-cli" \
    "$repo_root/target/release/google-cli" \
    "$repo_root/target/debug/google-cli" \
    "google-cli binary not found (set GOOGLE_CLI_BIN or install nils-google-cli)"
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

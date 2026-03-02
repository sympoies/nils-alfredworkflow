#!/usr/bin/env bash
# Shared pinned codex-cli runtime metadata for this workflow.

codex_cli_runtime_source_version_contract() {
  local runtime_lib_dir candidate
  runtime_lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  candidate="$runtime_lib_dir/codex_cli_version.sh"
  if [[ -f "$candidate" ]]; then
    # shellcheck disable=SC1090
    source "$candidate"
    return 0
  fi

  candidate="$runtime_lib_dir/../../../../scripts/lib/codex_cli_version.sh"
  if [[ -f "$candidate" ]]; then
    # shellcheck disable=SC1090
    source "$candidate"
    return 0
  fi

  return 1
}

if ! codex_cli_runtime_source_version_contract; then
  if [[ -z "${CODEX_CLI_VERSION:-}" ]]; then
    CODEX_CLI_VERSION="0.6.2"
  fi

  if [[ -z "${CODEX_CLI_CRATE:-}" ]]; then
    CODEX_CLI_CRATE="nils-codex-cli"
  fi
fi

if [[ -z "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_PINNED_VERSION="${CODEX_CLI_VERSION}"
fi

if [[ -z "${CODEX_CLI_PINNED_CRATE:-}" ]]; then
  CODEX_CLI_PINNED_CRATE="${CODEX_CLI_CRATE}"
fi

codex_cli_runtime_install_hint() {
  printf '%s %s' "${CODEX_CLI_PINNED_CRATE}" "${CODEX_CLI_PINNED_VERSION}"
}

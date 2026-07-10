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
    CODEX_CLI_VERSION="1.21.6"
  fi

  if [[ -z "${CODEX_CLI_RELEASE_REPO:-}" ]]; then
    CODEX_CLI_RELEASE_REPO="sympoies/nils-cli"
  fi

  if [[ -z "${CODEX_CLI_RELEASE_TARGET:-}" ]]; then
    CODEX_CLI_RELEASE_TARGET="aarch64-apple-darwin"
  fi

  if [[ -z "${CODEX_CLI_LICENSE:-}" ]]; then
    CODEX_CLI_LICENSE="MIT"
  fi
fi

if [[ -z "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_PINNED_VERSION="${CODEX_CLI_VERSION}"
fi

codex_cli_runtime_install_hint() {
  printf 'sympoies/nils-cli v%s' "${CODEX_CLI_PINNED_VERSION}"
}

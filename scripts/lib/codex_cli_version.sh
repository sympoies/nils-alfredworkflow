#!/usr/bin/env bash
# Canonical codex-cli crate/version metadata for CI, release, and runtime scripts.

if [[ -z "${CODEX_CLI_VERSION:-}" && -n "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_VERSION="${CODEX_CLI_PINNED_VERSION}"
fi

if [[ -z "${CODEX_CLI_CRATE:-}" && -n "${CODEX_CLI_PINNED_CRATE:-}" ]]; then
  CODEX_CLI_CRATE="${CODEX_CLI_PINNED_CRATE}"
fi

if [[ -z "${CODEX_CLI_VERSION:-}" ]]; then
  CODEX_CLI_VERSION="0.6.5"
fi

if [[ -z "${CODEX_CLI_CRATE:-}" ]]; then
  CODEX_CLI_CRATE="nils-codex-cli"
fi

# Backward-compatible aliases for existing workflow runtime consumers.
if [[ -z "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_PINNED_VERSION="${CODEX_CLI_VERSION}"
fi

if [[ -z "${CODEX_CLI_PINNED_CRATE:-}" ]]; then
  CODEX_CLI_PINNED_CRATE="${CODEX_CLI_CRATE}"
fi

codex_cli_version_install_hint() {
  printf '%s %s' "${CODEX_CLI_CRATE}" "${CODEX_CLI_VERSION}"
}

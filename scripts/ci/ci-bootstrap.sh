#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

context=""
install_codex_cli=0
apt_updated=0
linux_id=""
linux_like=""

if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  source /etc/os-release
  linux_id="${ID:-}"
  linux_like="${ID_LIKE:-}"
fi

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/ci-bootstrap.sh --context <ci|release|publish-crates> [--install-codex-cli]
USAGE
}

die() {
  echo "error: $*" >&2
  exit 2
}

require_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "error: missing required binary: cargo" >&2
    exit 1
  fi
}

run() {
  local -a run_cmd=("$@")
  echo "+ ${run_cmd[*]}"
  "${run_cmd[@]}"
}

run_privileged() {
  local -a privileged_cmd=("$@")
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    run "${privileged_cmd[@]}"
    return
  fi

  if command -v sudo >/dev/null 2>&1; then
    run sudo "${privileged_cmd[@]}"
    return
  fi

  return 1
}

is_debian_family() {
  [[ "$linux_id" == "ubuntu" || "$linux_id" == "debian" || "$linux_like" == *debian* ]]
}

ensure_apt_packages() {
  local -a packages=("$@")

  if ! command -v apt-get >/dev/null 2>&1; then
    return 1
  fi

  if [[ "$apt_updated" -eq 0 ]]; then
    run_privileged apt-get update
    apt_updated=1
  fi

  run_privileged apt-get install -y --no-install-recommends "${packages[@]}"
}

add_unique_item() {
  local target_name="$1"
  local item="$2"
  local existing_items=""

  eval "existing_items=\" \${${target_name}[*]-} \""
  case "$existing_items" in
  *" $item "*) return 0 ;;
  esac

  eval "${target_name}+=(\"\$item\")"
}

ensure_runtime_binaries() {
  local -a required=(git jq rg curl zip unzip)
  local -a missing=()
  local -a packages=()
  local required_cmd

  if [[ "$context" == "ci" ]]; then
    required+=(shellcheck shfmt)
  fi

  for required_cmd in "${required[@]}"; do
    if ! command -v "$required_cmd" >/dev/null 2>&1; then
      missing+=("$required_cmd")
      case "$required_cmd" in
      git)
        add_unique_item packages git
        ;;
      jq)
        add_unique_item packages jq
        ;;
      rg)
        add_unique_item packages ripgrep
        ;;
      curl)
        add_unique_item packages curl
        add_unique_item packages ca-certificates
        ;;
      zip)
        add_unique_item packages zip
        ;;
      unzip)
        add_unique_item packages unzip
        ;;
      shellcheck)
        add_unique_item packages shellcheck
        ;;
      shfmt)
        add_unique_item packages shfmt
        ;;
      *) ;;
      esac
    fi
  done

  if [[ "${#missing[@]}" -eq 0 ]]; then
    return
  fi

  if is_debian_family && [[ "${#packages[@]}" -gt 0 ]]; then
    ensure_apt_packages "${packages[@]}" || true
  fi

  missing=()
  for required_cmd in "${required[@]}"; do
    if ! command -v "$required_cmd" >/dev/null 2>&1; then
      missing+=("$required_cmd")
    fi
  done

  if [[ "${#missing[@]}" -gt 0 ]]; then
    echo "error: missing required runtime binaries: ${missing[*]}" >&2
    echo "hint: install dependencies from BINARY_DEPENDENCIES.md and rerun ci-bootstrap" >&2
    exit 1
  fi
}

install_codex_cli_runtime() {
  # shellcheck source=/dev/null
  source "$repo_root/scripts/lib/codex_cli_version.sh"
  cargo install "${CODEX_CLI_CRATE}" --version "${CODEX_CLI_VERSION}" --locked
}

while [[ $# -gt 0 ]]; do
  case "${1:-}" in
  --context)
    [[ $# -ge 2 ]] || die "--context requires a value"
    context="${2:-}"
    shift 2
    ;;
  --install-codex-cli)
    install_codex_cli=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    die "unknown argument: ${1:-}"
    ;;
  esac
done

case "$context" in
ci | release | publish-crates) ;;
*)
  die "--context must be one of: ci, release, publish-crates"
  ;;
esac

require_cargo
ensure_runtime_binaries

if [[ "$install_codex_cli" -eq 1 ]]; then
  install_codex_cli_runtime
fi

echo "ok: ci bootstrap complete (context=$context, install_codex_cli=$install_codex_cli)"

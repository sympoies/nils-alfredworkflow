#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runtime_meta="$script_dir/lib/codex_cli_runtime.sh"
if [[ ! -f "$runtime_meta" ]]; then
  echo "error: missing runtime metadata: $runtime_meta" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$runtime_meta"

stage_dir=""
workflow_root=""
default_expected_version="${CODEX_CLI_PINNED_VERSION}"
expected_version="${CODEX_CLI_BUNDLE_VERSION:-$default_expected_version}"
skip_version_check="${CODEX_CLI_PACK_SKIP_VERSION_CHECK:-0}"
skip_arch_check="${CODEX_CLI_PACK_SKIP_ARCH_CHECK:-0}"
release_repo="${CODEX_CLI_RELEASE_REPO:-sympoies/nils-cli}"
release_target="${CODEX_CLI_RELEASE_TARGET:-aarch64-apple-darwin}"

usage() {
  cat <<USAGE
Usage:
  prepare_package.sh --stage-dir <path> --workflow-root <path>
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --stage-dir)
    stage_dir="${2:-}"
    [[ -n "$stage_dir" ]] || {
      echo "error: --stage-dir requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --workflow-root)
    workflow_root="${2:-}"
    [[ -n "$workflow_root" ]] || {
      echo "error: --workflow-root requires a value" >&2
      exit 2
    }
    shift 2
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

[[ -n "$stage_dir" ]] || {
  usage >&2
  exit 2
}

[[ -n "$workflow_root" ]] || {
  usage >&2
  exit 2
}

resolve_source_bin() {
  local source_bin=""
  if [[ -n "${CODEX_CLI_PACK_BIN:-}" ]]; then
    if [[ "${CODEX_CLI_RELEASE_TEST_MODE:-0}" != 1 ]]; then
      echo "error: CODEX_CLI_PACK_BIN is restricted to CODEX_CLI_RELEASE_TEST_MODE=1" >&2
      exit 1
    fi
    if [[ ! -x "${CODEX_CLI_PACK_BIN}" ]]; then
      echo "error: CODEX_CLI_PACK_BIN is not executable: ${CODEX_CLI_PACK_BIN}" >&2
      exit 1
    fi
    source_bin="${CODEX_CLI_PACK_BIN}"
  fi

  if [[ -n "$source_bin" ]]; then
    if [[ "$skip_version_check" == "1" || "$(detect_version "$source_bin" || true)" == "$expected_version" ]]; then
      printf '%s\n' "$source_bin"
      return 0
    fi
    echo "info: test codex-cli does not match pinned $expected_version; resolving the pinned release asset." >&2
  fi

  local install_root=""
  if [[ -n "${CODEX_CLI_PACK_INSTALL_ROOT:-}" ]]; then
    install_root="${CODEX_CLI_PACK_INSTALL_ROOT}"
  elif [[ -n "${XDG_CACHE_HOME:-}" ]]; then
    install_root="${XDG_CACHE_HOME%/}/nils-alfredworkflow/nils-cli-release/codex-cli/${expected_version}/${release_target}"
  elif [[ -n "${HOME:-}" ]]; then
    install_root="${HOME%/}/.cache/nils-alfredworkflow/nils-cli-release/codex-cli/${expected_version}/${release_target}"
  else
    install_root="${workflow_root%/}/.cache/nils-cli-release/codex-cli/${expected_version}/${release_target}"
  fi
  mkdir -p "$install_root"

  local installed_bin="${install_root%/}/bin/codex-cli"
  if ! declare -F codex_cli_release_install >/dev/null 2>&1; then
    echo "error: codex-cli release installer is unavailable from runtime metadata" >&2
    exit 1
  fi
  CODEX_CLI_VERSION="$expected_version" CODEX_CLI_RELEASE_REPO="$release_repo" \
    codex_cli_release_install "$release_target" "$install_root" "$installed_bin"

  source_bin="$installed_bin"
  if [[ ! -x "$source_bin" ]]; then
    echo "error: installed codex-cli binary missing: $source_bin" >&2
    exit 1
  fi

  if [[ "$skip_version_check" != "1" ]]; then
    validate_version "$source_bin"
  fi

  printf '%s\n' "$source_bin"
  return 0
}

parse_semver_from_text() {
  local text="$1"
  if [[ "$text" =~ ([0-9]+\.[0-9]+\.[0-9]+) ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return 0
  fi
  return 1
}

validate_version() {
  local source_bin="$1"
  local actual_version
  actual_version="$(detect_version "$source_bin" || true)"

  if [[ -z "$actual_version" ]]; then
    local version_line
    version_line="$("$source_bin" --version 2>/dev/null | head -n1 || true)"
    echo "error: unable to detect codex-cli version from: $version_line" >&2
    exit 1
  fi

  if [[ "$actual_version" != "$expected_version" ]]; then
    echo "error: codex-cli version mismatch (expected $expected_version, got $actual_version)" >&2
    exit 1
  fi
}

detect_version() {
  local source_bin="$1"
  local version_line
  version_line="$("$source_bin" --version 2>/dev/null | head -n1 || true)"
  local actual_version
  actual_version="$(parse_semver_from_text "$version_line" || true)"
  [[ -n "$actual_version" ]] || return 1
  printf '%s\n' "$actual_version"
}

supports_arm64() {
  local source_bin="$1"

  if command -v lipo >/dev/null 2>&1; then
    local archs
    archs="$(lipo -archs "$source_bin" 2>/dev/null || true)"
    if [[ "$archs" == *"arm64"* ]]; then
      return 0
    fi
  fi

  if command -v file >/dev/null 2>&1; then
    local info
    info="$(file -b "$source_bin" 2>/dev/null || true)"
    if [[ "$info" == *"arm64"* ]]; then
      return 0
    fi
  fi

  return 1
}

validate_arch() {
  local source_bin="$1"
  local host_os
  host_os="$(uname -s 2>/dev/null || printf '')"
  if [[ "$host_os" != "Darwin" ]]; then
    cat >&2 <<EOF
error: codex-cli bundled runtime is configured for macOS arm64 packaging
hint: run packaging on Apple Silicon macOS, or set CODEX_CLI_PACK_SKIP_ARCH_CHECK=1 for non-release local checks
EOF
    exit 1
  fi

  if ! supports_arm64 "$source_bin"; then
    echo "error: codex-cli binary does not appear to contain arm64 architecture: $source_bin" >&2
    exit 1
  fi
}

source_bin="$(resolve_source_bin)"

if [[ "$skip_version_check" != "1" ]]; then
  validate_version "$source_bin"
fi

if [[ "$skip_arch_check" != "1" ]]; then
  validate_arch "$source_bin"
fi

mkdir -p "$stage_dir/bin"
cp "$source_bin" "$stage_dir/bin/codex-cli"
chmod +x "$stage_dir/bin/codex-cli"

echo "ok: bundled codex-cli from $source_bin"

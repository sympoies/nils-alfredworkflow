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
    if [[ ! -x "${CODEX_CLI_PACK_BIN}" ]]; then
      echo "error: CODEX_CLI_PACK_BIN is not executable: ${CODEX_CLI_PACK_BIN}" >&2
      exit 1
    fi
    source_bin="${CODEX_CLI_PACK_BIN}"
  else
    local resolved
    resolved="$(command -v codex-cli 2>/dev/null || true)"
    if [[ -n "$resolved" && -x "$resolved" ]]; then
      source_bin="$resolved"
    fi
  fi

  if [[ -n "$source_bin" && "$skip_version_check" == "1" ]]; then
    printf '%s\n' "$source_bin"
    return 0
  fi

  local source_version=""
  if [[ -n "$source_bin" ]]; then
    source_version="$(detect_version "$source_bin" || true)"
    if [[ "$source_version" == "$expected_version" ]]; then
      printf '%s\n' "$source_bin"
      return 0
    fi
    if [[ -n "$source_version" ]]; then
      echo "info: local codex-cli version $source_version does not match pinned $expected_version; resolving the pinned release asset." >&2
    else
      echo "info: unable to detect local codex-cli version from $source_bin; resolving the pinned release asset." >&2
    fi
  else
    echo "info: local codex-cli not found; resolving the pinned release asset." >&2
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
  if [[ -x "$installed_bin" ]]; then
    if [[ "$skip_version_check" == "1" ]]; then
      printf '%s\n' "$installed_bin"
      return 0
    fi
    local installed_version=""
    installed_version="$(detect_version "$installed_bin" || true)"
    if [[ "$installed_version" == "$expected_version" ]]; then
      echo "info: reusing cached pinned codex-cli $expected_version from $installed_bin." >&2
      printf '%s\n' "$installed_bin"
      return 0
    fi
  fi

  if ! command -v curl >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1; then
    cat >&2 <<EOF
error: curl and tar are required to fetch pinned codex-cli for packaging
hint: install the packaging tools, or set CODEX_CLI_PACK_BIN to a codex-cli ${expected_version} binary
EOF
    exit 1
  fi

  local archive_name="nils-cli-v${expected_version}-${release_target}.tar.gz"
  local release_base="${CODEX_CLI_RELEASE_BASE_URL:-https://github.com/${release_repo}/releases/download/v${expected_version}}"
  local archive_path="${install_root%/}/${archive_name}"
  local checksum_path="${archive_path}.sha256"
  local extract_dir="${install_root%/}/extract"

  rm -f "${archive_path}.partial" "${checksum_path}.partial"
  if ! curl --fail --location --retry 3 --silent --show-error \
    "${release_base%/}/${archive_name}" -o "${archive_path}.partial" \
    || ! curl --fail --location --retry 3 --silent --show-error \
      "${release_base%/}/${archive_name}.sha256" -o "${checksum_path}.partial"; then
    cat >&2 <<EOF
error: failed to download ${archive_name} from ${release_base}
hint: retry with network access, install ${release_repo} v${expected_version}, or set CODEX_CLI_PACK_BIN
EOF
    exit 1
  fi
  mv "${archive_path}.partial" "$archive_path"
  mv "${checksum_path}.partial" "$checksum_path"

  local expected_sha actual_sha
  expected_sha="$(awk 'NF { print $1; exit }' "$checksum_path")"
  if command -v shasum >/dev/null 2>&1; then
    actual_sha="$(shasum -a 256 "$archive_path" | awk '{print $1}')"
  elif command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "$archive_path" | awk '{print $1}')"
  elif command -v openssl >/dev/null 2>&1; then
    actual_sha="$(openssl dgst -sha256 "$archive_path" | awk '{print $NF}')"
  else
    echo "error: no SHA-256 tool available for release verification" >&2
    exit 1
  fi
  if [[ -z "$expected_sha" || "$actual_sha" != "$expected_sha" ]]; then
    echo "error: checksum mismatch for ${archive_name}" >&2
    exit 1
  fi

  rm -rf "$extract_dir"
  mkdir -p "$extract_dir"
  if ! tar -xzf "$archive_path" -C "$extract_dir"; then
    echo "error: failed to extract ${archive_name}" >&2
    exit 1
  fi

  local extracted_bin=""
  extracted_bin="$(find "$extract_dir" -type f -path '*/bin/codex-cli' -print -quit 2>/dev/null || true)"
  if [[ -z "$extracted_bin" ]]; then
    echo "error: release asset does not contain bin/codex-cli: ${archive_name}" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$installed_bin")"
  cp "$extracted_bin" "$installed_bin"
  chmod +x "$installed_bin"

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

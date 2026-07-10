#!/usr/bin/env bash
# Canonical codex-cli release metadata for CI, packaging, and runtime scripts.

if [[ -z "${CODEX_CLI_VERSION:-}" && -n "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_VERSION="${CODEX_CLI_PINNED_VERSION}"
fi

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

# Backward-compatible aliases for existing workflow runtime consumers.
if [[ -z "${CODEX_CLI_PINNED_VERSION:-}" ]]; then
  CODEX_CLI_PINNED_VERSION="${CODEX_CLI_VERSION}"
fi

codex_cli_release_expected_sha256() {
  local target="$1"
  if [[ "${CODEX_CLI_RELEASE_TEST_MODE:-0}" == 1 && -n "${CODEX_CLI_RELEASE_TEST_SHA256:-}" ]]; then
    printf '%s\n' "$CODEX_CLI_RELEASE_TEST_SHA256"
    return 0
  fi
  case "$target" in
    aarch64-apple-darwin) printf '%s\n' '7331a5a2116a321c308179d883af69fcfa0cca6f0ebcb4bfe59c6be708872395' ;;
    x86_64-apple-darwin) printf '%s\n' 'f7c49910473469608074811985121c6fd4b9a301d9890668994df6c0a2ebb200' ;;
    x86_64-unknown-linux-gnu) printf '%s\n' '2ae7433838714234b041ddd2fb1b0979444a286cd2d2ee70bec1171e3d182b73' ;;
    aarch64-unknown-linux-gnu) printf '%s\n' '232753fa6f18ffbe0f8d765d453259df4695f1f6e11b3cf68a6954d1f0ac9f92' ;;
    *) return 1 ;;
  esac
}

codex_cli_release_detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Darwin:arm64 | Darwin:aarch64) printf '%s\n' aarch64-apple-darwin ;;
    Darwin:x86_64) printf '%s\n' x86_64-apple-darwin ;;
    Linux:x86_64 | Linux:amd64) printf '%s\n' x86_64-unknown-linux-gnu ;;
    Linux:arm64 | Linux:aarch64) printf '%s\n' aarch64-unknown-linux-gnu ;;
    *) return 1 ;;
  esac
}

codex_cli_release_sha256_file() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    return 127
  fi
}

codex_cli_release_install() {
  local target="$1"
  local install_root="$2"
  local destination="$3"
  local expected_sha archive_name release_base archive_path checksum_path extract_dir
  local published_sha actual_sha extracted_bin

  expected_sha="$(codex_cli_release_expected_sha256 "$target")" || {
    echo "error: no repository-pinned codex-cli digest for target: $target" >&2
    return 1
  }
  archive_name="nils-cli-v${CODEX_CLI_VERSION}-${target}.tar.gz"
  release_base="https://github.com/${CODEX_CLI_RELEASE_REPO}/releases/download/v${CODEX_CLI_VERSION}"
  if [[ -n "${CODEX_CLI_RELEASE_BASE_URL:-}" ]]; then
    if [[ "${CODEX_CLI_RELEASE_TEST_MODE:-0}" != 1 ]]; then
      echo "error: CODEX_CLI_RELEASE_BASE_URL requires CODEX_CLI_RELEASE_TEST_MODE=1" >&2
      return 1
    fi
    release_base="${CODEX_CLI_RELEASE_BASE_URL%/}"
  fi

  mkdir -p "$install_root"
  archive_path="${install_root%/}/$archive_name"
  checksum_path="${archive_path}.sha256"
  extract_dir="${install_root%/}/extract"
  rm -f "${archive_path}.partial" "${checksum_path}.partial"
  if ! curl --fail --location --retry 3 --silent --show-error \
    "${release_base%/}/${archive_name}" -o "${archive_path}.partial" \
    || ! curl --fail --location --retry 3 --silent --show-error \
      "${release_base%/}/${archive_name}.sha256" -o "${checksum_path}.partial"; then
    echo "error: failed to download pinned codex-cli release asset: $archive_name" >&2
    return 1
  fi
  mv "${archive_path}.partial" "$archive_path"
  mv "${checksum_path}.partial" "$checksum_path"

  published_sha="$(awk 'NF {print $1; exit}' "$checksum_path")"
  if [[ "$published_sha" != "$expected_sha" ]]; then
    echo "error: published checksum does not match repository-pinned checksum for $archive_name" >&2
    return 1
  fi
  actual_sha="$(codex_cli_release_sha256_file "$archive_path")" || {
    echo "error: no SHA-256 tool available for codex-cli release verification" >&2
    return 1
  }
  if [[ "$actual_sha" != "$expected_sha" ]]; then
    echo "error: checksum mismatch for $archive_name" >&2
    return 1
  fi

  rm -rf "$extract_dir"
  mkdir -p "$extract_dir"
  tar -xzf "$archive_path" -C "$extract_dir" || {
    echo "error: failed to extract $archive_name" >&2
    return 1
  }
  extracted_bin="$(find "$extract_dir" -type f -path '*/bin/codex-cli' -print -quit 2>/dev/null || true)"
  [[ -n "$extracted_bin" ]] || {
    echo "error: release asset does not contain bin/codex-cli: $archive_name" >&2
    return 1
  }
  mkdir -p "$(dirname "$destination")"
  install -m 755 "$extracted_bin" "$destination"
}

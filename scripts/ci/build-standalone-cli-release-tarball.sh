#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

package_name="nils-alfred-cli"
release_tag=""
target=""
dist_dir=""
manifest_path=""
binary_dir=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/build-standalone-cli-release-tarball.sh --tag <release-tag> --target <target-triple> [options]

Options:
  --tag <tag>           Release tag name, for example v1.3.3. Required.
  --target <triple>     Rust target triple used in the tarball name. Required.
  --package-name <name> Package asset prefix. Default: nils-alfred-cli.
  --manifest <path>     Release manifest. Default: release/standalone-cli-bins.txt.
  --binary-dir <path>   Built binary directory. Default: target/<triple>/release.
  --repo-root <path>    Repository root path. Defaults to script-derived repo root.
  --dist-dir <path>     Dist directory. Defaults to <repo-root>/dist.
  -h, --help            Show this help.
USAGE
}

die() {
  echo "error: $*" >&2
  exit 2
}

trim() {
  printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

write_sha256() {
  local file_path="$1"
  local checksum_path="$2"

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" >"$checksum_path"
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" >"$checksum_path"
    return
  fi

  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 -r "$file_path" >"$checksum_path"
    return
  fi

  echo "error: missing checksum tool (need shasum, sha256sum, or openssl)" >&2
  exit 1
}

read_manifest() {
  local manifest="$1"
  local raw line bin crate_dir note

  while IFS= read -r raw || [[ -n "$raw" ]]; do
    line="$(trim "$raw")"
    [[ -n "$line" ]] || continue
    [[ "$line" == \#* ]] && continue

    IFS='|' read -r bin crate_dir note <<<"$line"
    bin="$(trim "${bin:-}")"
    crate_dir="$(trim "${crate_dir:-}")"
    note="$(trim "${note:-}")"

    if [[ -z "$bin" || -z "$crate_dir" ]]; then
      echo "error: invalid manifest row: $raw" >&2
      exit 1
    fi

    printf '%s\t%s\t%s\n' "$bin" "$crate_dir" "$note"
  done <"$manifest"
}

while [[ $# -gt 0 ]]; do
  case "${1:-}" in
  --tag)
    release_tag="${2:-}"
    [[ -n "$release_tag" ]] || die "--tag requires a value"
    shift 2
    ;;
  --target)
    target="${2:-}"
    [[ -n "$target" ]] || die "--target requires a value"
    shift 2
    ;;
  --package-name)
    package_name="${2:-}"
    [[ -n "$package_name" ]] || die "--package-name requires a value"
    shift 2
    ;;
  --manifest)
    manifest_path="${2:-}"
    [[ -n "$manifest_path" ]] || die "--manifest requires a value"
    shift 2
    ;;
  --binary-dir)
    binary_dir="${2:-}"
    [[ -n "$binary_dir" ]] || die "--binary-dir requires a value"
    shift 2
    ;;
  --repo-root)
    repo_root="${2:-}"
    [[ -n "$repo_root" ]] || die "--repo-root requires a value"
    shift 2
    ;;
  --dist-dir)
    dist_dir="${2:-}"
    [[ -n "$dist_dir" ]] || die "--dist-dir requires a value"
    shift 2
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown argument: ${1:-}" >&2
    usage >&2
    exit 2
    ;;
  esac
done

if [[ -z "$release_tag" || -z "$target" ]]; then
  echo "error: --tag and --target are required" >&2
  usage >&2
  exit 2
fi

if ! [[ "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  die "--tag must look like vX.Y.Z: $release_tag"
fi

if ! [[ "$package_name" =~ ^[A-Za-z0-9._-]+$ ]]; then
  die "--package-name contains unsupported characters: $package_name"
fi

if [[ -z "$dist_dir" ]]; then
  dist_dir="$repo_root/dist"
fi
mkdir -p "$dist_dir"
dist_dir="$(cd "$dist_dir" && pwd)"

if [[ -z "$manifest_path" ]]; then
  manifest_path="$repo_root/release/standalone-cli-bins.txt"
elif [[ "$manifest_path" != /* ]]; then
  manifest_path="$repo_root/$manifest_path"
fi

if [[ -z "$binary_dir" ]]; then
  binary_dir="$repo_root/target/$target/release"
elif [[ "$binary_dir" != /* ]]; then
  binary_dir="$repo_root/$binary_dir"
fi

if [[ ! -f "$manifest_path" ]]; then
  echo "error: missing release manifest: $manifest_path" >&2
  exit 1
fi

if [[ ! -d "$binary_dir" ]]; then
  echo "error: missing binary directory: $binary_dir" >&2
  exit 1
fi

license_source="$repo_root/LICENSE"
license_artifact="$repo_root/THIRD_PARTY_LICENSES.md"
notices_artifact="$repo_root/THIRD_PARTY_NOTICES.md"

for required in "$license_source" "$license_artifact" "$notices_artifact"; do
  if [[ ! -f "$required" ]]; then
    echo "error: missing required packaging input: $required" >&2
    exit 1
  fi
done

entries=()
while IFS= read -r entry; do
  entries+=("$entry")
done < <(read_manifest "$manifest_path")
if ((${#entries[@]} == 0)); then
  echo "error: release manifest has no binaries: $manifest_path" >&2
  exit 1
fi

bundle_dir_name="${package_name}-${release_tag}-${target}"
stage_parent="$dist_dir/${package_name}-tarball-stage"
package_dir="$stage_parent/$bundle_dir_name"
tarball="$dist_dir/${bundle_dir_name}.tar.gz"

rm -rf "$stage_parent"
mkdir -p "$package_dir/bin" "$package_dir/docs"

{
  printf '# %s\n\n' "$package_name"
  printf 'Release tag: %s\n\n' "$release_tag"
  printf 'Target: %s\n\n' "$target"
  printf 'This archive contains standalone command-line binaries from sympoies/nils-alfredworkflow.\n\n'
  printf '## Binaries\n\n'
  printf '| Binary | Notes |\n'
  printf '| --- | --- |\n'
} >"$package_dir/README.md"

{
  printf 'binary\tcrate_dir\tnote\n'
} >"$package_dir/MANIFEST.tsv"

for entry in "${entries[@]}"; do
  IFS=$'\t' read -r bin crate_dir note <<<"$entry"
  src="$binary_dir/$bin"
  crate_root="$repo_root/$crate_dir"
  crate_readme="$crate_root/README.md"
  crate_docs="$crate_root/docs"

  if [[ ! -x "$src" ]]; then
    echo "error: release binary not found or not executable: $src" >&2
    exit 1
  fi
  if [[ ! -f "$crate_readme" ]]; then
    echo "error: missing crate README for $bin: $crate_readme" >&2
    exit 1
  fi
  if [[ ! -d "$crate_docs" ]]; then
    echo "error: missing crate docs directory for $bin: $crate_docs" >&2
    exit 1
  fi

  install -m 0755 "$src" "$package_dir/bin/$bin"

  doc_dir="$package_dir/docs/$bin"
  mkdir -p "$doc_dir/docs"
  cp "$crate_readme" "$doc_dir/README.md"
  cp -R "$crate_docs"/. "$doc_dir/docs/"

  printf '| %s | %s |\n' "$bin" "$note" >>"$package_dir/README.md"
  printf '%s\t%s\t%s\n' "$bin" "$crate_dir" "$note" >>"$package_dir/MANIFEST.tsv"
done

cp "$license_source" "$package_dir/LICENSE"
cp "$license_artifact" "$package_dir/THIRD_PARTY_LICENSES.md"
cp "$notices_artifact" "$package_dir/THIRD_PARTY_NOTICES.md"

rm -f "$tarball" "$tarball.sha256"
(
  cd "$stage_parent"
  tar -czf "$tarball" "$bundle_dir_name"
)
write_sha256 "$tarball" "$tarball.sha256"
rm -rf "$stage_parent"

echo "ok: built standalone CLI release tarball $tarball"

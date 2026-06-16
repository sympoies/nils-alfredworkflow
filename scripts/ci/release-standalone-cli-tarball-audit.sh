#!/usr/bin/env bash
set -euo pipefail

package_name="nils-alfred-cli"
target=""
tag=""
dist_dir="dist"
manifest_path=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/release-standalone-cli-tarball-audit.sh --target <target-triple> [--tag <tag>] [options]

Checks that a standalone CLI release tarball includes required binaries,
crate docs, and compliance artifacts.

Options:
  --target <triple>     Required Rust target triple used in tarball name.
  --tag <tag>           Optional release tag. If omitted, script requires exactly one tarball for the target.
  --package-name <name> Package asset prefix. Default: nils-alfred-cli.
  --manifest <path>     Release manifest. Default: release/standalone-cli-bins.txt.
  --dist-dir <path>     Dist directory. Default: dist.
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
  --target)
    [[ $# -ge 2 ]] || die "--target requires a value"
    target="$2"
    shift 2
    ;;
  --tag)
    [[ $# -ge 2 ]] || die "--tag requires a value"
    tag="$2"
    shift 2
    ;;
  --package-name)
    [[ $# -ge 2 ]] || die "--package-name requires a value"
    package_name="$2"
    shift 2
    ;;
  --manifest)
    [[ $# -ge 2 ]] || die "--manifest requires a value"
    manifest_path="$2"
    shift 2
    ;;
  --dist-dir)
    [[ $# -ge 2 ]] || die "--dist-dir requires a value"
    dist_dir="$2"
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

if [[ -z "$target" ]]; then
  echo "error: --target is required" >&2
  usage >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$repo_root" || ! -d "$repo_root" ]]; then
  echo "error: must run inside a git work tree" >&2
  exit 2
fi
cd "$repo_root"

if [[ -z "$manifest_path" ]]; then
  manifest_path="$repo_root/release/standalone-cli-bins.txt"
elif [[ "$manifest_path" != /* ]]; then
  manifest_path="$repo_root/$manifest_path"
fi

if [[ ! -f "$manifest_path" ]]; then
  echo "error: missing release manifest: $manifest_path" >&2
  exit 1
fi

if [[ ! -d "$dist_dir" ]]; then
  echo "FAIL: missing dist directory: $dist_dir"
  exit 1
fi

if [[ -n "$tag" ]]; then
  tarball="$dist_dir/${package_name}-${tag}-${target}.tar.gz"
else
  mapfile -t matches < <(find "$dist_dir" -maxdepth 1 -type f -name "${package_name}-*-${target}.tar.gz" -print | LC_ALL=C sort)
  if ((${#matches[@]} == 0)); then
    echo "FAIL: no standalone CLI tarball found for target: ${target} in ${dist_dir}"
    exit 1
  fi
  if ((${#matches[@]} > 1)); then
    echo "error: multiple standalone CLI tarballs found for target ${target}; pass --tag to disambiguate" >&2
    printf '%s\n' "${matches[@]}" >&2
    exit 2
  fi
  tarball="${matches[0]}"
fi

if [[ ! -f "$tarball" ]]; then
  echo "FAIL: missing tarball: $tarball"
  exit 1
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/standalone-cli-tarball-audit.XXXXXX")"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

if ! tar -xzf "$tarball" -C "$tmp_dir"; then
  echo "error: failed to extract tarball: $tarball" >&2
  exit 2
fi

root_dir="$tmp_dir/$(basename "$tarball" .tar.gz)"
if [[ ! -d "$root_dir" ]]; then
  echo "FAIL: tarball root directory missing: $(basename "$tarball" .tar.gz)"
  exit 1
fi

missing_count=0
require_file() {
  local rel="$1"
  if [[ ! -f "$root_dir/$rel" ]]; then
    echo "FAIL: missing required file in tarball: $rel"
    missing_count=$((missing_count + 1))
  fi
}

require_executable() {
  local rel="$1"
  if [[ ! -f "$root_dir/$rel" ]]; then
    echo "FAIL: missing required executable in tarball: $rel"
    missing_count=$((missing_count + 1))
    return
  fi
  if [[ ! -x "$root_dir/$rel" ]]; then
    echo "FAIL: packaged file is not executable: $rel"
    missing_count=$((missing_count + 1))
  fi
}

require_file "README.md"
require_file "MANIFEST.tsv"
require_file "LICENSE"
require_file "THIRD_PARTY_LICENSES.md"
require_file "THIRD_PARTY_NOTICES.md"

mapfile -t entries < <(read_manifest "$manifest_path")
if ((${#entries[@]} == 0)); then
  echo "error: release manifest has no binaries: $manifest_path" >&2
  exit 1
fi

for entry in "${entries[@]}"; do
  IFS=$'\t' read -r bin _crate_dir _note <<<"$entry"
  require_executable "bin/$bin"
  require_file "docs/$bin/README.md"
  require_file "docs/$bin/docs/README.md"
done

if ((missing_count > 0)); then
  echo "FAIL: standalone CLI release tarball audit (target=${target}, missing=${missing_count}, tarball=${tarball})"
  exit 1
fi

echo "PASS: standalone CLI release tarball audit (target=${target}, missing=0, tarball=${tarball})"

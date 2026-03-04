#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

release_tag=""
dist_dir=""
bundle_dir=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/ci/build-release-bundle.sh --tag <release-tag> [--repo-root <path>] [--dist-dir <path>] [--bundle-dir <path>]

Options:
  --tag        Release tag name (for example: v1.2.3). Required.
  --repo-root  Repository root path. Defaults to script-derived repo root.
  --dist-dir   Dist directory containing .alfredworkflow artifacts. Defaults to <repo-root>/dist.
  --bundle-dir Release bundle output directory. Defaults to <dist-dir>/release-bundles.
  -h, --help   Show this help.
USAGE
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

  echo "error: missing checksum tool (need shasum or sha256sum)" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --tag)
    release_tag="${2:-}"
    [[ -n "$release_tag" ]] || {
      echo "error: --tag requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --repo-root)
    repo_root="${2:-}"
    [[ -n "$repo_root" ]] || {
      echo "error: --repo-root requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --dist-dir)
    dist_dir="${2:-}"
    [[ -n "$dist_dir" ]] || {
      echo "error: --dist-dir requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --bundle-dir)
    bundle_dir="${2:-}"
    [[ -n "$bundle_dir" ]] || {
      echo "error: --bundle-dir requires a value" >&2
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

if [[ -z "$release_tag" ]]; then
  echo "error: --tag is required" >&2
  usage >&2
  exit 2
fi

if [[ -z "$dist_dir" ]]; then
  dist_dir="$repo_root/dist"
fi

if [[ -z "$bundle_dir" ]]; then
  bundle_dir="$dist_dir/release-bundles"
fi

standalone_script="$repo_root/scripts/workflow-clear-quarantine-standalone.sh"
license_source="$repo_root/THIRD_PARTY_LICENSES.md"
notices_source="$repo_root/THIRD_PARTY_NOTICES.md"

if [[ ! -f "$standalone_script" ]]; then
  echo "error: missing required standalone script at repository root: scripts/workflow-clear-quarantine-standalone.sh" >&2
  exit 1
fi

if [[ ! -f "$license_source" ]]; then
  echo "error: missing required license artifact at repository root: THIRD_PARTY_LICENSES.md" >&2
  exit 1
fi

if [[ ! -f "$notices_source" ]]; then
  echo "error: missing required notices artifact at repository root: THIRD_PARTY_NOTICES.md" >&2
  exit 1
fi

mkdir -p "$bundle_dir"

bundle_name="workflows-${release_tag}.zip"
bundle_path="$bundle_dir/$bundle_name"

bundle_input_count=0
while IFS= read -r bundle_input; do
  [[ -n "$bundle_input" ]] || continue
  bundle_input_count=$((bundle_input_count + 1))
done < <(
  find "$dist_dir" -type f \( -name '*.alfredworkflow' -o -name '*.alfredworkflow.sha256' \) | sort
)

if [[ "$bundle_input_count" -eq 0 ]]; then
  echo "error: no workflow artifacts found under $dist_dir" >&2
  exit 1
fi

(
  cd "$dist_dir"
  rel_inputs=()
  while IFS= read -r rel_input; do
    [[ -n "$rel_input" ]] || continue
    rel_inputs+=("$rel_input")
  done < <(find . -type f \( -name '*.alfredworkflow' -o -name '*.alfredworkflow.sha256' \) | sort)
  zip -q -r "$bundle_path" "${rel_inputs[@]}"
)
write_sha256 "$bundle_path" "$bundle_path.sha256"

standalone_asset="$bundle_dir/workflow-clear-quarantine-standalone.sh"
cp "$standalone_script" "$standalone_asset"
chmod +x "$standalone_asset"
write_sha256 "$standalone_asset" "$standalone_asset.sha256"

license_asset="$bundle_dir/THIRD_PARTY_LICENSES.md"
cp "$license_source" "$license_asset"
write_sha256 "$license_asset" "$license_asset.sha256"

notices_asset="$bundle_dir/THIRD_PARTY_NOTICES.md"
cp "$notices_source" "$notices_asset"
write_sha256 "$notices_asset" "$notices_asset.sha256"

echo "ok: built release bundle $bundle_path"

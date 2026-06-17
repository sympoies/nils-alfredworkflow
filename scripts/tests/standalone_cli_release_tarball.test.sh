#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || fail "expected file: $path"
}

trim() {
  printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

manifest_bins() {
  local manifest="$1"
  local raw line bin _crate_dir _note
  while IFS= read -r raw || [[ -n "$raw" ]]; do
    line="$(trim "$raw")"
    [[ -n "$line" ]] || continue
    [[ "$line" == \#* ]] && continue
    IFS='|' read -r bin _crate_dir _note <<<"$line"
    bin="$(trim "${bin:-}")"
    [[ -n "$bin" ]] || fail "manifest row is missing binary: $raw"
    printf '%s\n' "$bin"
  done <"$manifest"
}

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/standalone-cli-release-tarball.test.XXXXXX")"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

tag="v9.9.9"
target="x86_64-unknown-linux-gnu"
dist_dir="$tmp_dir/dist"
binary_dir="$tmp_dir/bin"
manifest="$repo_root/release/standalone-cli-bins.txt"
tarball="$dist_dir/nils-alfred-cli-${tag}-${target}.tar.gz"

mkdir -p "$binary_dir"
bins=()
while IFS= read -r bin; do
  bins+=("$bin")
done < <(manifest_bins "$manifest")
((${#bins[@]} > 0)) || fail "expected at least one manifest binary"

for bin in "${bins[@]}"; do
  fake_bin="$binary_dir/$bin"
  cat >"$fake_bin" <<EOF
#!/usr/bin/env bash
echo "$bin 9.9.9"
EOF
  chmod +x "$fake_bin"
done

cd "$repo_root"
bash scripts/ci/build-standalone-cli-release-tarball.sh \
  --tag "$tag" \
  --target "$target" \
  --binary-dir "$binary_dir" \
  --dist-dir "$dist_dir"

assert_file "$tarball"
assert_file "$tarball.sha256"

checksum_target="$(awk '{print $2}' "$tarball.sha256")"
expected_checksum_target="$(basename "$tarball")"
if [[ "$checksum_target" != "$expected_checksum_target" ]]; then
  fail "checksum should reference $expected_checksum_target, got $checksum_target"
fi

(
  cd "$dist_dir"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$expected_checksum_target.sha256"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$expected_checksum_target.sha256"
  else
    fail "missing checksum verification tool"
  fi
)

bash scripts/ci/release-standalone-cli-tarball-audit.sh \
  --tag "$tag" \
  --target "$target" \
  --dist-dir "$dist_dir"

contents="$(tar -tzf "$tarball")"
for rel in \
  "README.md" \
  "MANIFEST.tsv" \
  "LICENSE" \
  "THIRD_PARTY_LICENSES.md" \
  "THIRD_PARTY_NOTICES.md"; do
  if [[ "$contents" != *"nils-alfred-cli-${tag}-${target}/${rel}"* ]]; then
    fail "tarball missing $rel"
  fi
done

for bin in "${bins[@]}"; do
  for rel in "bin/$bin" "docs/$bin/README.md" "docs/$bin/docs/README.md"; do
    if [[ "$contents" != *"nils-alfred-cli-${tag}-${target}/${rel}"* ]]; then
      fail "tarball missing $rel"
    fi
  done
done

echo "PASS: standalone_cli_release_tarball.test.sh"

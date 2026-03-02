#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
output_file="$repo_root/THIRD_PARTY_LICENSES.md"

usage() {
  cat <<'USAGE'
Usage:
  scripts/generate-third-party-licenses.sh --write
  scripts/generate-third-party-licenses.sh --check
USAGE
}

fail() {
  echo "error: $*" >&2
  exit 1
}

require_bin() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || fail "missing required binary: $name"
}

sha256_file() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $2}'
    return
  fi
  fail "missing sha256 tool (need shasum, sha256sum, or openssl)"
}

sha256_stdin() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{print $1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
    return
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 | awk '{print $2}'
    return
  fi
  fail "missing sha256 tool (need shasum, sha256sum, or openssl)"
}

md_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/|/\\|/g'
}

md_url_or_dash() {
  local value="${1:-}"
  if [[ -z "$value" || "$value" == "-" ]]; then
    printf '%s' "-"
    return
  fi
  printf '<%s>' "$value"
}

mode=""
while [[ $# -gt 0 ]]; do
  case "$1" in
  --write)
    if [[ -n "$mode" && "$mode" != "write" ]]; then
      fail "choose exactly one mode: --write or --check"
    fi
    mode="write"
    shift
    ;;
  --check)
    if [[ -n "$mode" && "$mode" != "check" ]]; then
      fail "choose exactly one mode: --write or --check"
    fi
    mode="check"
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    fail "unknown argument: $1"
    ;;
  esac
done

[[ -n "$mode" ]] || {
  usage >&2
  fail "choose exactly one mode: --write or --check"
}

cargo_lock="$repo_root/Cargo.lock"
package_lock="$repo_root/package-lock.json"
codex_cli_version_script="$repo_root/scripts/lib/codex_cli_version.sh"

[[ -f "$cargo_lock" ]] || fail "required input missing: $cargo_lock"
[[ -f "$package_lock" ]] || fail "required input missing: $package_lock"
[[ -f "$codex_cli_version_script" ]] || fail "required input missing: $codex_cli_version_script"

require_bin cargo
require_bin jq
require_bin curl
require_bin mktemp
require_bin sort
require_bin awk
require_bin sed
require_bin cmp

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/third-party-licenses.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

cargo_metadata_json="$tmp_root/cargo-metadata.json"
rust_packages_json="$tmp_root/rust-packages.json"
rust_summary_tsv="$tmp_root/rust-summary.tsv"
rust_crates_tsv="$tmp_root/rust-crates.tsv"
node_packages_json="$tmp_root/node-packages.json"
node_packages_tsv="$tmp_root/node-packages.tsv"
runtime_response_json="$tmp_root/runtime-crate.json"
generated_file="$tmp_root/THIRD_PARTY_LICENSES.md"

(
  cd "$repo_root"
  cargo metadata --format-version 1 --locked >"$cargo_metadata_json"
)

jq -c '
[
  .packages[]
  | select(.source != null)
  | {
      name: .name,
      version: .version,
      license: (.license // .license_file // "UNKNOWN"),
      repository: (.repository // .homepage // "-")
    }
]
| sort_by(.name, .version)
| unique_by(.name, .version)
' "$cargo_metadata_json" >"$rust_packages_json"

jq -r '
group_by(.license)
| map({license: .[0].license, count: length})
| sort_by(-.count, .license)
| .[]
| [.count, .license]
| @tsv
' "$rust_packages_json" >"$rust_summary_tsv"

jq -r '.[] | [.name, .version, .license, .repository] | @tsv' "$rust_packages_json" >"$rust_crates_tsv"

jq -c '
[
  .packages
  | to_entries[]
  | select(.key != "")
  | . as $entry
  | ($entry.value) as $value
  | {
      name: ($value.name // ($entry.key | split("node_modules/") | last)),
      version: ($value.version // "UNKNOWN"),
      license: ($value.license // "UNKNOWN"),
      resolved: ($value.resolved // "-")
    }
]
| sort_by(.name, .version)
| unique_by(.name, .version)
' "$package_lock" >"$node_packages_json"

jq -r '.[] | [.name, .version, .license, .resolved] | @tsv' "$node_packages_json" >"$node_packages_tsv"

# shellcheck source=/dev/null
source "$codex_cli_version_script"
[[ -n "${CODEX_CLI_CRATE:-}" ]] || fail "missing CODEX_CLI_CRATE after sourcing $codex_cli_version_script"
[[ -n "${CODEX_CLI_VERSION:-}" ]] || fail "missing CODEX_CLI_VERSION after sourcing $codex_cli_version_script"

runtime_source_url="https://crates.io/api/v1/crates/${CODEX_CLI_CRATE}/${CODEX_CLI_VERSION}"
if ! curl -fsSL "$runtime_source_url" >"$runtime_response_json"; then
  fail "failed to fetch runtime crate metadata from crates.io: $runtime_source_url"
fi

runtime_crate="$(jq -r '.version.crate // ""' "$runtime_response_json")"
runtime_version="$(jq -r '.version.num // ""' "$runtime_response_json")"
runtime_license="$(jq -r '.version.license // "UNKNOWN"' "$runtime_response_json")"
runtime_repository="$(jq -r '.version.repository // .version.homepage // "-"' "$runtime_response_json")"

[[ -n "$runtime_crate" ]] || fail "crates.io response missing .version.crate for $runtime_source_url"
[[ -n "$runtime_version" ]] || fail "crates.io response missing .version.num for $runtime_source_url"

if [[ "$runtime_crate" != "$CODEX_CLI_CRATE" || "$runtime_version" != "$CODEX_CLI_VERSION" ]]; then
  fail "runtime crate mismatch: expected ${CODEX_CLI_CRATE}@${CODEX_CLI_VERSION}, got ${runtime_crate}@${runtime_version}"
fi

if [[ -z "$runtime_repository" ]]; then
  runtime_repository="-"
fi

cargo_lock_sha="$(sha256_file "$cargo_lock")"
package_lock_sha="$(sha256_file "$package_lock")"
runtime_pin_script_sha="$(sha256_file "$codex_cli_version_script")"
runtime_metadata_sha="$(printf '%s\n' \
  "crate=$runtime_crate" \
  "version=$runtime_version" \
  "license=$runtime_license" \
  "repository=$runtime_repository" \
  "source=$runtime_source_url" |
  sha256_stdin)"
data_source_fingerprint="$(printf '%s\n' \
  "cargo_lock_sha=$cargo_lock_sha" \
  "package_lock_sha=$package_lock_sha" \
  "runtime_pin_script_sha=$runtime_pin_script_sha" \
  "runtime_metadata_sha=$runtime_metadata_sha" |
  sha256_stdin)"

rust_count="$(jq -r 'length' "$rust_packages_json")"
node_count="$(jq -r 'length' "$node_packages_json")"

{
  cat <<EOF
# Third-Party Licenses

This file is generated by \`scripts/generate-third-party-licenses.sh\`.
Do not edit manually.

## Scope

- Rust third-party crates resolved from \`Cargo.lock\` via \`cargo metadata --locked\` (workspace crates excluded).
- Node third-party packages resolved from \`package-lock.json\` (root package excluded).
- External packaged runtime crate resolved from \`scripts/lib/codex_cli_version.sh\` with metadata from crates.io.
- Contract: \`docs/specs/third-party-license-artifact-contract-v1.md\`.

## Deterministic Provenance

- Data source fingerprint (SHA256): \`$data_source_fingerprint\`
- Runtime metadata fingerprint (SHA256): \`$runtime_metadata_sha\`

## Data Sources

| Source | Locator | SHA256 | Notes |
| --- | --- | --- | --- |
EOF

  printf "| Cargo lockfile | \`%s\` | \`%s\` | \`%s\` |\n" "Cargo.lock" "$cargo_lock_sha" \
    "cargo metadata --format-version 1 --locked"
  printf "| Node lockfile | \`%s\` | \`%s\` | \`%s\` |\n" "package-lock.json" "$package_lock_sha" \
    "jq package-lock extraction"
  printf "| Runtime crate pin | \`%s\` | \`%s\` | \`%s\` |\n" "scripts/lib/codex_cli_version.sh" "$runtime_pin_script_sha" \
    "source for \$CODEX_CLI_CRATE and \$CODEX_CLI_VERSION"
  printf "| Runtime crate metadata | %s | \`%s\` | \`%s\` |\n" "$(md_url_or_dash "$runtime_source_url")" "$runtime_metadata_sha" \
    "curl crates.io API plus jq normalized fields"

  printf '\n## Rust License Summary (%s crates)\n\n' "$rust_count"
  printf '| Count | License Expression |\n'
  printf '| ---: | --- |\n'
  while IFS=$'\t' read -r count license; do
    printf '| %s | %s |\n' "$count" "$(md_escape "$license")"
  done <"$rust_summary_tsv"

  printf '\n## Rust Crates (from Cargo.lock)\n\n'
  printf '| Crate | Version | License | Repository |\n'
  printf '| --- | --- | --- | --- |\n'
  while IFS=$'\t' read -r name version license repository; do
    printf '| %s | %s | %s | %s |\n' \
      "$(md_escape "$name")" \
      "$(md_escape "$version")" \
      "$(md_escape "$license")" \
      "$(md_url_or_dash "$repository")"
  done <"$rust_crates_tsv"

  printf '\n## Node Packages (%s packages)\n\n' "$node_count"
  printf '| Package | Version | License | Resolved |\n'
  printf '| --- | --- | --- | --- |\n'
  while IFS=$'\t' read -r name version license resolved; do
    printf '| %s | %s | %s | %s |\n' \
      "$(md_escape "$name")" \
      "$(md_escape "$version")" \
      "$(md_escape "$license")" \
      "$(md_url_or_dash "$resolved")"
  done <"$node_packages_tsv"

  printf '\n## External Packaged Runtime\n\n'
  printf '| Crate | Version | License | Repository | Source |\n'
  printf '| --- | --- | --- | --- | --- |\n'
  printf '| %s | %s | %s | %s | %s |\n' \
    "$(md_escape "$runtime_crate")" \
    "$(md_escape "$runtime_version")" \
    "$(md_escape "$runtime_license")" \
    "$(md_url_or_dash "$runtime_repository")" \
    "$(md_url_or_dash "$runtime_source_url")"

  cat <<'EOF'

## Regeneration

```bash
bash scripts/generate-third-party-licenses.sh --write
bash scripts/generate-third-party-licenses.sh --check
```
EOF
} >"$generated_file"

if [[ "$mode" == "write" ]]; then
  cp "$generated_file" "$output_file"
  echo "PASS [write] generated $output_file"
  exit 0
fi

if [[ ! -f "$output_file" ]]; then
  fail "missing output artifact: $output_file (run --write first)"
fi

if cmp -s "$output_file" "$generated_file"; then
  echo "PASS [check] $output_file is up to date"
  exit 0
fi

echo "FAIL [check] $output_file is stale" >&2
echo "Run: bash scripts/generate-third-party-licenses.sh --write" >&2
exit 1

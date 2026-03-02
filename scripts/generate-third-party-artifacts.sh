#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
licenses_output_file="$repo_root/THIRD_PARTY_LICENSES.md"
notices_output_file="$repo_root/THIRD_PARTY_NOTICES.md"

usage() {
  cat <<'USAGE'
Usage:
  scripts/generate-third-party-artifacts.sh --write
  scripts/generate-third-party-artifacts.sh --check
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
require_bin python3

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/third-party-artifacts.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

cargo_metadata_json="$tmp_root/cargo-metadata.json"
rust_packages_json="$tmp_root/rust-packages.json"
rust_summary_tsv="$tmp_root/rust-summary.tsv"
rust_crates_tsv="$tmp_root/rust-crates.tsv"
node_packages_json="$tmp_root/node-packages.json"
node_packages_tsv="$tmp_root/node-packages.tsv"
runtime_response_json="$tmp_root/runtime-crate.json"
generated_licenses_file="$tmp_root/THIRD_PARTY_LICENSES.md"
generated_notices_file="$tmp_root/THIRD_PARTY_NOTICES.md"

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
runtime_fetch_attempts="${THIRD_PARTY_LICENSES_CRATES_IO_MAX_ATTEMPTS:-4}"
runtime_fetch_backoff_seconds="${THIRD_PARTY_LICENSES_CRATES_IO_RETRY_BASE_SECONDS:-1}"
runtime_fetch_user_agent="${THIRD_PARTY_LICENSES_USER_AGENT:-nils-alfredworkflow-third-party-artifacts/1.0 (+https://github.com/sympoies/nils-alfredworkflow)}"

[[ "$runtime_fetch_attempts" =~ ^[1-9][0-9]*$ ]] || fail "THIRD_PARTY_LICENSES_CRATES_IO_MAX_ATTEMPTS must be a positive integer"
[[ "$runtime_fetch_backoff_seconds" =~ ^[0-9]+$ ]] || fail "THIRD_PARTY_LICENSES_CRATES_IO_RETRY_BASE_SECONDS must be a non-negative integer"

runtime_fetch_error_file="$tmp_root/runtime-crates-io-fetch.stderr"
runtime_fetch_ok=0

for ((runtime_fetch_attempt = 1; runtime_fetch_attempt <= runtime_fetch_attempts; runtime_fetch_attempt++)); do
  : >"$runtime_fetch_error_file"
  if curl -fsSL \
    --retry 0 \
    --connect-timeout 10 \
    --max-time 30 \
    -A "$runtime_fetch_user_agent" \
    -H 'Accept: application/json' \
    "$runtime_source_url" >"$runtime_response_json" 2>"$runtime_fetch_error_file"; then
    runtime_fetch_ok=1
    if [[ "$runtime_fetch_attempt" -gt 1 ]]; then
      echo "note: crates.io fetch succeeded after retry ${runtime_fetch_attempt}/${runtime_fetch_attempts}" >&2
    fi
    break
  fi

  runtime_fetch_rc=$?
  runtime_fetch_error="$(tr '\n' ' ' <"$runtime_fetch_error_file" | sed -e 's/[[:space:]]\+/ /g' -e 's/^ //' -e 's/ $//')"
  [[ -n "$runtime_fetch_error" ]] || runtime_fetch_error="no stderr output"
  echo "warn: crates.io fetch attempt ${runtime_fetch_attempt}/${runtime_fetch_attempts} failed (exit=${runtime_fetch_rc}): ${runtime_fetch_error}" >&2

  if [[ "$runtime_fetch_attempt" -lt "$runtime_fetch_attempts" ]]; then
    runtime_backoff_sleep="$((runtime_fetch_backoff_seconds * runtime_fetch_attempt))"
    sleep "$runtime_backoff_sleep"
  fi
done

if [[ "$runtime_fetch_ok" -ne 1 ]]; then
  fail "failed to fetch runtime crate metadata from crates.io after ${runtime_fetch_attempts} attempts: $runtime_source_url"
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
  cat <<__LICENSE_MD__
# Third-Party Licenses

This file is generated by \`scripts/generate-third-party-artifacts.sh\`.
Do not edit manually.

## Scope

- Rust third-party crates resolved from \`Cargo.lock\` via \`cargo metadata --locked\` (workspace crates excluded).
- Node third-party packages resolved from \`package-lock.json\` (root package excluded).
- External packaged runtime crate resolved from \`scripts/lib/codex_cli_version.sh\` with metadata from crates.io.
- Contract: \`docs/specs/third-party-artifacts-contract-v1.md\`.

## Deterministic Provenance

- Data source fingerprint (SHA256): \`$data_source_fingerprint\`
- Runtime metadata fingerprint (SHA256): \`$runtime_metadata_sha\`

## Data Sources

| Source | Locator | SHA256 | Notes |
| --- | --- | --- | --- |
__LICENSE_MD__

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

  cat <<'__LICENSE_REGEN__'

## Regeneration

```bash
bash scripts/generate-third-party-artifacts.sh --write
bash scripts/generate-third-party-artifacts.sh --check
```
__LICENSE_REGEN__
} >"$generated_licenses_file"

python3 - "$cargo_metadata_json" "$cargo_lock_sha" "$generated_notices_file" <<'__NOTICE_PY__'
import json
import pathlib
import re
import sys
from urllib.parse import urlparse

metadata_path = pathlib.Path(sys.argv[1]).resolve()
lock_hash = sys.argv[2]
output_path = pathlib.Path(sys.argv[3]).resolve()

with metadata_path.open("r", encoding="utf-8") as fh:
    metadata = json.load(fh)

packages = metadata.get("packages", [])
third_party = [pkg for pkg in packages if pkg.get("source")]
third_party.sort(
    key=lambda pkg: (
        pkg.get("name", ""),
        pkg.get("version", ""),
        pkg.get("source", ""),
        pkg.get("id", ""),
    )
)


def cargo_source_label(source: str) -> str:
    if source == "registry+https://github.com/rust-lang/crates.io-index":
        return "crates.io"
    if source.startswith("registry+"):
        return source.removeprefix("registry+")
    if source.startswith("git+"):
        return source.removeprefix("git+").split("#", 1)[0]
    return source


def cargo_source_url(pkg: dict) -> str | None:
    source = (pkg.get("source") or "").strip()
    name = (pkg.get("name") or "").strip()
    version = (pkg.get("version") or "").strip()

    if source == "registry+https://github.com/rust-lang/crates.io-index" and name and version:
        return f"https://crates.io/crates/{name}/{version}"
    if source.startswith("registry+"):
        return source.removeprefix("registry+")
    if source.startswith("git+"):
        return source.removeprefix("git+").split("#", 1)[0]
    return None


def normalize_manifest_dir(manifest_path: str) -> pathlib.Path:
    parsed = urlparse(manifest_path)
    if parsed.scheme == "file":
        return pathlib.Path(parsed.path).resolve().parent
    return pathlib.Path(manifest_path).resolve().parent


def license_value(pkg: dict) -> str:
    license_expr = (pkg.get("license") or "").strip()
    if license_expr:
        return license_expr
    license_file = (pkg.get("license_file") or "").strip()
    if license_file:
        return f"SEE LICENSE FILE ({license_file})"
    return "UNKNOWN"


def crate_top_level_files(crate_dir: pathlib.Path) -> list[str]:
    if not crate_dir.is_dir():
        return []
    files = [child.name for child in crate_dir.iterdir() if child.is_file()]
    files.sort(key=lambda value: value.lower())
    return files


def casefold_lookup(file_names: list[str]) -> dict[str, str]:
    lookup: dict[str, str] = {}
    for name in file_names:
        key = name.lower()
        if key not in lookup:
            lookup[key] = name
    return lookup


def find_notice_files(crate_dir: pathlib.Path) -> list[str]:
    preferred = [
        "NOTICE",
        "NOTICE.md",
        "NOTICE.txt",
        "NOTICE.rst",
        "notice",
        "notice.md",
        "notice.txt",
        "notice.rst",
    ]

    file_names = crate_top_level_files(crate_dir)
    name_lookup = casefold_lookup(file_names)

    found: list[str] = []
    seen: set[str] = set()

    for candidate in preferred:
        key = candidate.lower()
        resolved = name_lookup.get(key)
        if resolved and key not in seen:
            found.append(resolved)
            seen.add(key)

    for file_name in file_names:
        if re.match(r"(?i)^notice(?:[._-].*)?$", file_name):
            key = file_name.lower()
            if key in seen:
                continue
            found.append(file_name)
            seen.add(key)

    return found


def resolve_license_file_ref(crate_dir: pathlib.Path, raw_ref: str | None) -> str | None:
    if not raw_ref:
        return None

    ref = raw_ref.strip()
    if not ref:
        return None

    candidate = pathlib.Path(ref)
    if candidate.is_absolute():
        target = candidate
    else:
        target = crate_dir / candidate

    if target.exists():
        try:
            return str(target.relative_to(crate_dir))
        except ValueError:
            return str(target)

    return ref


def find_license_files(crate_dir: pathlib.Path) -> list[str]:
    preferred = [
        "LICENSE",
        "LICENSE.md",
        "LICENSE.txt",
        "LICENSE-APACHE",
        "LICENSE-MIT",
        "COPYING",
        "COPYING.md",
        "COPYING.txt",
        "UNLICENSE",
        "UNLICENSE.txt",
    ]

    file_names = crate_top_level_files(crate_dir)
    name_lookup = casefold_lookup(file_names)

    found: list[str] = []
    seen: set[str] = set()

    for candidate in preferred:
        key = candidate.lower()
        resolved = name_lookup.get(key)
        if resolved and key not in seen:
            found.append(resolved)
            seen.add(key)

    for file_name in file_names:
        if re.match(r"(?i)^(license|copying|unlicense)(?:[._-].*)?$", file_name):
            key = file_name.lower()
            if key in seen:
                continue
            found.append(file_name)
            seen.add(key)

    return found


fallback_notice_line = "No explicit NOTICE file discovered."

lines: list[str] = [
    "# THIRD_PARTY_NOTICES",
    "",
    "This file documents third-party notice-file discovery for Rust crates used by this workspace.",
    "",
    "- Data source: `cargo metadata --format-version 1 --locked`",
    f"- Cargo.lock SHA256: `{lock_hash}`",
    f"- Third-party crates (`source != null`): {len(third_party)}",
    "",
    "## Notice Extraction Policy",
    "",
    "- The generator checks each crate directory for notice files using deterministic name matching.",
    "- If no notice file is found, the fallback wording below is emitted exactly.",
    f"- Standard fallback wording: `{fallback_notice_line}`",
    "",
    "## Dependency Notices",
    "",
]

for pkg in third_party:
    name = pkg.get("name", "")
    version = pkg.get("version", "")
    source = cargo_source_label(pkg.get("source", ""))
    manifest_path = pkg.get("manifest_path", "")
    crate_dir = normalize_manifest_dir(manifest_path) if manifest_path else pathlib.Path(".")
    license_expr = license_value(pkg)

    notice_refs = find_notice_files(crate_dir)

    license_refs: list[str] = []
    seen_license_refs: set[str] = set()

    metadata_license_ref = resolve_license_file_ref(crate_dir, pkg.get("license_file"))
    if metadata_license_ref:
        lower_ref = metadata_license_ref.lower()
        license_refs.append(metadata_license_ref)
        seen_license_refs.add(lower_ref)

    for discovered_ref in find_license_files(crate_dir):
        key = discovered_ref.lower()
        if key in seen_license_refs:
            continue
        license_refs.append(discovered_ref)
        seen_license_refs.add(key)

    lines.append(f"### {name} {version}")
    lines.append("")
    lines.append(f"- License: `{license_expr}`")
    lines.append(f"- Source: `{source}`")
    is_mpl = re.search(r"(?i)\bMPL-2\.0\b", license_expr) is not None
    source_url = cargo_source_url(pkg)
    if source_url and is_mpl:
        lines.append(f"- Source URL: <{source_url}>")
    if is_mpl:
        lines.append("- License text (MPL-2.0): <https://mozilla.org/MPL/2.0/>")

    if notice_refs:
        lines.append("- Notice files:")
        for ref in notice_refs:
            lines.append(f"  - `{ref}`")
    else:
        lines.append(f"- Notice files: {fallback_notice_line}")

    if license_refs:
        lines.append("- License file references:")
        for ref in license_refs:
            lines.append(f"  - `{ref}`")
    else:
        lines.append("- License file reference: none declared")

    lines.append("")

output_path.write_text("\n".join(lines), encoding="utf-8")
__NOTICE_PY__

if [[ "$mode" == "write" ]]; then
  cp "$generated_licenses_file" "$licenses_output_file"
  cp "$generated_notices_file" "$notices_output_file"
  echo "PASS [write] generated $licenses_output_file"
  echo "PASS [write] generated $notices_output_file"
  exit 0
fi

stale=0

if [[ ! -f "$licenses_output_file" ]]; then
  echo "FAIL [check] missing output artifact: $licenses_output_file (run --write first)" >&2
  stale=1
elif ! cmp -s "$licenses_output_file" "$generated_licenses_file"; then
  echo "FAIL [check] $licenses_output_file is stale" >&2
  stale=1
fi

if [[ ! -f "$notices_output_file" ]]; then
  echo "FAIL [check] missing output artifact: $notices_output_file (run --write first)" >&2
  stale=1
elif ! cmp -s "$notices_output_file" "$generated_notices_file"; then
  echo "FAIL [check] $notices_output_file is stale" >&2
  stale=1
fi

if [[ "$stale" -ne 0 ]]; then
  echo "Run: bash scripts/generate-third-party-artifacts.sh --write" >&2
  exit 1
fi

echo "PASS [check] third-party artifacts are up to date"

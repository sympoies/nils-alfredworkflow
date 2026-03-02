# Third-Party Artifacts Contract v1

## Purpose

This contract defines deterministic generation requirements for:

- `THIRD_PARTY_LICENSES.md`
- `THIRD_PARTY_NOTICES.md`

## Generator Entrypoint

- Write mode: `bash scripts/generate-third-party-artifacts.sh --write`
- Check mode: `bash scripts/generate-third-party-artifacts.sh --check`

The generator is the only supported way to refresh the artifacts. Manual edits are out of contract.

## Mandatory Section Order

### `THIRD_PARTY_LICENSES.md`

Must render sections in this exact order:

1. `# Third-Party Licenses`
2. `## Scope`
3. `## Deterministic Provenance`
4. `## Data Sources`
5. `## Rust License Summary (<count> crates)`
6. `## Rust Crates (from Cargo.lock)`
7. `## Node Packages (<count> packages)`
8. `## External Packaged Runtime`
9. `## Regeneration`

### `THIRD_PARTY_NOTICES.md`

Must render sections in this exact order:

1. `# THIRD_PARTY_NOTICES`
2. Intro paragraph describing notice discovery for third-party Rust crates
3. Metadata bullets (`cargo metadata` source, `Cargo.lock` SHA256, third-party crate count)
4. `## Notice Extraction Policy`
5. `## Dependency Notices`

## Table Schemas

`THIRD_PARTY_LICENSES.md` must include these markdown tables with exact column order:

- `## Data Sources`
  - `Source | Locator | SHA256 | Notes`
- `## Rust License Summary`
  - `Count | License Expression`
- `## Rust Crates (from Cargo.lock)`
  - `Crate | Version | License | Repository`
- `## Node Packages`
  - `Package | Version | License | Resolved`
- `## External Packaged Runtime`
  - `Crate | Version | License | Repository | Source`

`THIRD_PARTY_NOTICES.md` uses deterministic per-crate bullet sections under `## Dependency Notices`.
Each per-crate section renders bullets in this order:

1. `- License: ...`
2. `- Source: ...`
3. `- Source URL: ...` (only when resolved `License` includes `MPL-2.0` and URL is derivable from source metadata)
4. `- License text (MPL-2.0): <https://mozilla.org/MPL/2.0/>` (only when the resolved license expression includes `MPL-2.0`)
5. Notice files bullets / fallback line
6. License file references bullets / fallback line

## Input Sources

- Rust dependencies: `Cargo.lock` via `cargo metadata --format-version 1 --locked`
- Node dependencies: `package-lock.json`
- Runtime crate pin: `scripts/lib/codex_cli_version.sh` (`CODEX_CLI_CRATE`, `CODEX_CLI_VERSION`)
- Runtime crate metadata: `https://crates.io/api/v1/crates/<runtime crate>/<version>`

## Deterministic Rendering Rules

- No wall-clock timestamp fields are allowed (for example `Generated on` or `Generated at`).
- Provenance fields use stable SHA256 values derived from source inputs and normalized runtime metadata fields.
- Rust crate rows sort by `(crate name ASC, version ASC)`.
- Rust license summary rows sort by `(count DESC, license expression ASC)`.
- Node package rows sort by `(package name ASC, version ASC)`.
- Notice entry order sorts by `(crate name ASC, version ASC, source ASC, package id ASC)`.
- Markdown table cells escape `|` consistently.

## Notice Extraction Policy

For each third-party Rust crate (`source != null`):

1. Resolve crate directory from package `manifest_path`.
2. Discover notice files with deterministic filename matching.
3. If no notice files are discovered, emit this exact fallback wording:
   - `No explicit NOTICE file discovered.`
4. Collect license file references in deterministic order:
   - `license_file` metadata reference when present
   - then top-level files matching `LICENSE*`, `COPYING*`, `UNLICENSE*`
5. If no license file references are discovered, emit:
   - `License file reference: none declared`

## `--write` / `--check` Semantics

- `--write`
  - Regenerates both artifacts from inputs and replaces repository file content.
  - Exit code `0` on success.
  - Exit non-zero on missing inputs, command failures, or runtime metadata lookup failures.
- `--check`
  - Regenerates both artifacts to temp outputs and compares against repository artifacts.
  - Exit code `0` when both artifacts are byte-identical.
  - Exit code `1` when drift or missing artifact is detected, with remediation to run `--write`.
  - Exit non-zero on missing inputs, command failures, or runtime metadata lookup failures.

## Failure Behavior

The generator must fail closed (non-zero) when:

- Required inputs are missing (`Cargo.lock`, `package-lock.json`, `scripts/lib/codex_cli_version.sh`).
- Runtime crate pin variables are missing after sourcing `scripts/lib/codex_cli_version.sh`.
- crates.io lookup fails (network/HTTP failure, invalid payload, or crate/version mismatch).
- Required command dependencies are unavailable (`cargo`, `jq`, `curl`, `python3`).

Diagnostics must include the failing input or command and expected corrective action.

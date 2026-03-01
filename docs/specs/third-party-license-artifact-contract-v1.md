# Third-Party License Artifact Contract v1

## Purpose

This contract defines the deterministic generation requirements for
`THIRD_PARTY_LICENSES.md`.

## Generator Entrypoint

- Write mode: `bash scripts/generate-third-party-licenses.sh --write`
- Check mode: `bash scripts/generate-third-party-licenses.sh --check`

The generator is the only supported way to refresh `THIRD_PARTY_LICENSES.md`.
Manual edits are out of contract.

## Mandatory Section Order

`THIRD_PARTY_LICENSES.md` MUST render sections in this exact order:

1. `# Third-Party Licenses`
2. `## Scope`
3. `## Deterministic Provenance`
4. `## Data Sources`
5. `## Rust License Summary (<count> crates)`
6. `## Rust Crates (from Cargo.lock)`
7. `## Node Packages (<count> packages)`
8. `## External Packaged Runtime`
9. `## Regeneration`

## Table Schemas

The artifact MUST include these markdown tables with exact column order:

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

## Input Sources

- Rust dependencies: `Cargo.lock` via `cargo metadata --format-version 1 --locked`.
- Node dependencies: `package-lock.json`.
- Runtime crate pin: `scripts/lib/codex_cli_version.sh`, using
  `CODEX_CLI_CRATE` and `CODEX_CLI_VERSION`.
- Runtime crate metadata: crates.io version API at
  `https://crates.io/api/v1/crates/<runtime crate>/<version>`.

## Deterministic Rendering Rules

- No wall-clock timestamp fields are allowed (for example `Generated on` or
  `Generated at`).
- Provenance MUST use stable SHA256 values derived from source inputs and
  normalized runtime metadata fields.
- Rust crate rows MUST sort by `(crate name ASC, version ASC)`.
- Rust license summary rows MUST sort by `(count DESC, license expression ASC)`.
- Node package rows MUST sort by `(package name ASC, version ASC)`.
- Markdown table cells MUST escape the `|` character consistently.

## `--write` / `--check` Semantics

- `--write`
  - Regenerate `THIRD_PARTY_LICENSES.md` from inputs and replace file content.
  - Exit code `0` on success.
  - Exit non-zero on missing inputs, command failures, or runtime metadata
    lookup failures.
- `--check`
  - Regenerate content in-memory/temp output and compare with
    `THIRD_PARTY_LICENSES.md`.
  - Exit code `0` when no drift exists.
  - Exit code `1` when drift exists with actionable remediation to run
    `--write`.
  - Exit non-zero on missing inputs, command failures, or runtime metadata
    lookup failures.

## Failure Behavior

The generator MUST fail closed (non-zero) in these cases:

- Required input file is missing (`Cargo.lock`, `package-lock.json`, or
  `scripts/lib/codex_cli_version.sh`).
- Runtime crate pin variables are missing after sourcing
  `scripts/lib/codex_cli_version.sh`.
- crates.io lookup fails (network/HTTP failure, invalid payload, or
  crate/version mismatch in response).
- Required command dependencies are unavailable (`cargo`, `jq`, `curl`).

When failures occur, diagnostics MUST include the failing input or command and
the expected corrective action.

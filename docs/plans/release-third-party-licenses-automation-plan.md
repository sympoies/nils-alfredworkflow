# Plan: Release Third-Party Licenses Automation

## Overview

This plan introduces a deterministic automation flow for `THIRD_PARTY_LICENSES.md`, then enforces freshness in CI,
and finally wires license artifacts into release outputs for traceable compliance. The rollout follows sequential sprint
gates: generator contract first, CI and troubleshooting routing second, release packaging and audit third. Existing
workflow runtime behavior stays unchanged outside license/compliance artifacts and release packaging metadata.

## Scope

- In scope:
  - Add a repository generator script for `THIRD_PARTY_LICENSES.md` that covers Rust crates, Node packages, and the
    external `nils-codex-cli` runtime crate metadata.
  - Define deterministic generation rules (stable ordering, no date-based drift, strict `--write/--check` behavior).
  - Add CI/license audit checks and wire them into existing lint/CI entrypoints.
  - Update release workflow so generated license artifacts are present and verified before publishing release assets.
  - Add troubleshooting routing for license generation/audit/release failures in `TROUBLESHOOTING.md`.
- Out of scope:
  - Legal interpretation beyond machine-resolved metadata and declared package licenses.
  - Adding `THIRD_PARTY_NOTICES.md` scope in this phase.
  - Changing workflow feature behavior or Alfred runtime contracts.

## Assumptions

1. `cargo metadata --format-version 1 --locked` remains the canonical Rust dependency source.
2. `package-lock.json` remains the canonical Node dependency source for repo-level tooling deps.
3. `scripts/lib/codex_cli_version.sh` remains the source of truth for packaged `nils-codex-cli` crate/version pin.
4. CI and release jobs can access crates.io API for runtime crate license metadata lookup, with explicit failure
   diagnostics when network/API failures occur.

## Success criteria

- A single generator command can deterministically refresh `THIRD_PARTY_LICENSES.md` with no manual table editing.
- CI fails closed when `THIRD_PARTY_LICENSES.md` drifts from lockfile/package metadata inputs.
- `scripts/workflow-lint.sh` and `.github/workflows/ci.yml` run the license audit gate.
- `.github/workflows/release.yml` includes generated license artifacts in release outputs and runs an audit gate before
  upload.
- `TROUBLESHOOTING.md` includes a direct route for license generation/audit/release failures.

## Sprint 1: Deterministic Generator and License Contract

**Goal**: Replace manual regeneration steps with a deterministic script contract for `THIRD_PARTY_LICENSES.md`. **Demo/Validation**:

- Command(s):
  - `plan-tooling validate --file docs/plans/release-third-party-licenses-automation-plan.md`
  - `plan-tooling split-prs --file docs/plans/release-third-party-licenses-automation-plan.md --scope sprint --sprint 1 --strategy auto --default-pr-grouping group --format json`
  - `bash scripts/generate-third-party-licenses.sh --write`
  - `bash scripts/generate-third-party-licenses.sh --check`
- Verify:
  - `THIRD_PARTY_LICENSES.md` is produced from locked Rust/Node/runtime metadata only.
  - Re-running `--write` without metadata changes yields no diff.
- **PR grouping intent**: group
- **Execution Profile**: parallel-x2
- Sprint scorecard:
  - `Execution Profile`: parallel-x2
  - `TotalComplexity`: 16
  - `CriticalPathComplexity`: 12
  - `MaxBatchWidth`: 2
  - `OverlapHotspots`: `scripts/generate-third-party-licenses.sh`; `THIRD_PARTY_LICENSES.md`; `scripts/lib/codex_cli_version.sh`

### Task 1.1: Define third-party license artifact contract

- **Location**:
  - `docs/specs/third-party-license-artifact-contract-v1.md`
  - `THIRD_PARTY_LICENSES.md`
- **Description**: Define required sections, deterministic ordering keys, runtime crate metadata rules, and exact `--write/--check` semantics for a generated `THIRD_PARTY_LICENSES.md` contract.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - Contract specifies mandatory section order and table schemas.
  - Contract forbids wall-clock timestamps that cause false drift and defines stable provenance fields.
  - Contract specifies failure behavior for missing inputs and crates.io lookup failures.
- **Validation**:
  - `test -f docs/specs/third-party-license-artifact-contract-v1.md`
  - `rg -n 'THIRD_PARTY_LICENSES\.md|deterministic|--write|--check|crates\.io|runtime crate' docs/specs/third-party-license-artifact-contract-v1.md`

### Task 1.2: Implement generator entrypoint for Rust, Node, and runtime crate metadata

- **Location**:
  - `scripts/generate-third-party-licenses.sh`
  - `THIRD_PARTY_LICENSES.md`
  - `scripts/lib/codex_cli_version.sh`
- **Description**: Implement a single entrypoint that renders `THIRD_PARTY_LICENSES.md` from locked metadata sources and runtime crate lookup, with stable sorting and deterministic formatting.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 5
- **Acceptance criteria**:
  - `--write` regenerates the document in-place.
  - `--check` exits non-zero on drift and prints actionable diagnostics.
  - Rust/Node/runtime sections are generated from command outputs rather than manual edits.
- **Validation**:
  - `bash scripts/generate-third-party-licenses.sh --write`
  - `bash scripts/generate-third-party-licenses.sh --check`
  - `tmp_file="$(mktemp)" && bash scripts/generate-third-party-licenses.sh --write && cp THIRD_PARTY_LICENSES.md "$tmp_file" && bash scripts/generate-third-party-licenses.sh --write && cmp -s THIRD_PARTY_LICENSES.md "$tmp_file" && rm -f "$tmp_file"`

### Task 1.3: Normalize nondeterministic fields and provenance rendering

- **Location**:
  - `scripts/generate-third-party-licenses.sh`
  - `THIRD_PARTY_LICENSES.md`
  - `docs/specs/third-party-license-artifact-contract-v1.md`
- **Description**: Remove or replace time-variant output fields (for example generated-at date) with deterministic provenance fields (for example lockfile hash/input fingerprint) and codify expected formatting.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - No date/time-dependent line changes occur across repeated runs with unchanged inputs.
  - Provenance fields are deterministic and documented in the contract.
  - Generator emits consistent markdown escaping/order for all table rows.
- **Validation**:
  - `bash scripts/generate-third-party-licenses.sh --write`
  - `bash scripts/generate-third-party-licenses.sh --check`
  - `! rg -n 'Generated on|Generated at' THIRD_PARTY_LICENSES.md`
  - `rg -n 'SHA256|fingerprint|Data source' THIRD_PARTY_LICENSES.md`

### Task 1.4: Add generator regression tests and contributor usage docs

- **Location**:
  - `tests/third-party-licenses/generator.test.sh`
  - `DEVELOPMENT.md`
  - `BINARY_DEPENDENCIES.md`
- **Description**: Add shell regression tests for `--write/--check` paths and document one-command regeneration and expected failure modes.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Tests cover clean run, drift detection, and missing-input error behavior.
  - `DEVELOPMENT.md` and `BINARY_DEPENDENCIES.md` include the generator command and dependency prerequisites.
- **Validation**:
  - `bash tests/third-party-licenses/generator.test.sh`
  - `rg -n 'generate-third-party-licenses\.sh' DEVELOPMENT.md BINARY_DEPENDENCIES.md`

## Sprint 2: CI Gate, Lint Integration, and Troubleshooting Routing

**Goal**: Enforce license artifact freshness in local and CI checks, and provide fast failure routing in troubleshooting docs. **Demo/Validation**:

- Command(s):
  - `bash scripts/ci/third-party-licenses-audit.sh --strict`
  - `plan-tooling split-prs --file docs/plans/release-third-party-licenses-automation-plan.md --scope sprint --sprint 2 --strategy auto --default-pr-grouping group --format json`
  - `scripts/workflow-lint.sh`
  - `rg -n 'third-party-licenses-audit\.sh' .github/workflows/ci.yml scripts/workflow-lint.sh`
- Verify:
  - CI/local lint fails closed when license artifact is stale or missing.
  - Root troubleshooting index includes direct remediation commands for license failures.
- **PR grouping intent**: group
- **Execution Profile**: parallel-x2
- Sprint scorecard:
  - `Execution Profile`: parallel-x2
  - `TotalComplexity`: 14
  - `CriticalPathComplexity`: 12
  - `MaxBatchWidth`: 2
  - `OverlapHotspots`: `scripts/ci/third-party-licenses-audit.sh`; `scripts/workflow-lint.sh`; `.github/workflows/ci.yml`; `TROUBLESHOOTING.md`

### Task 2.1: Implement strict CI third-party license audit script

- **Location**:
  - `scripts/ci/third-party-licenses-audit.sh`
  - `scripts/generate-third-party-licenses.sh`
- **Description**: Add PASS/WARN/FAIL-style audit behavior (aligned with `nils-cli` CI patterns) that checks required files and runs generator `--check` with strict/non-strict modes.
- **Dependencies**:
  - Task 1.3
  - Task 1.4
- **Complexity**: 4
- **Acceptance criteria**:
  - Supports `--strict` and non-strict mode with clear exit-code semantics.
  - Detects missing `THIRD_PARTY_LICENSES.md` and artifact drift.
  - Emits concise diagnostics suitable for CI logs.
- **Validation**:
  - `bash scripts/ci/third-party-licenses-audit.sh --strict`
  - `bash scripts/ci/third-party-licenses-audit.sh`

### Task 2.2: Wire license audit into lint entrypoint and CI workflow

- **Location**:
  - `scripts/workflow-lint.sh`
  - `.github/workflows/ci.yml`
  - `DEVELOPMENT.md`
- **Description**: Add audit invocation to local lint entrypoint and Ubuntu CI job, and document that gate as part of required checks.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 4
- **Acceptance criteria**:
  - `scripts/workflow-lint.sh` runs the new audit.
  - CI includes explicit third-party license audit step before expensive packaging checks.
  - `DEVELOPMENT.md` required checks list includes the license audit command.
- **Validation**:
  - `rg -n 'third-party-licenses-audit\.sh' scripts/workflow-lint.sh .github/workflows/ci.yml DEVELOPMENT.md`
  - `scripts/workflow-lint.sh`

### Task 2.3: Add troubleshooting index route for license incidents

- **Location**:
  - `TROUBLESHOOTING.md`
  - `docs/RELEASE.md`
- **Description**: Add a dedicated troubleshooting route with commands for generator drift, crates.io lookup failures, and CI/release gate failures.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 2
- **Acceptance criteria**:
  - Root troubleshooting index has a distinct "third-party license" route.
  - `docs/RELEASE.md` links to remediation commands for release-time license gate failures.
- **Validation**:
  - `rg -n 'third-party|license|generate-third-party-licenses|third-party-licenses-audit' TROUBLESHOOTING.md docs/RELEASE.md`

### Task 2.4: Add audit regression tests for strict/non-strict behavior

- **Location**:
  - `tests/third-party-licenses/audit.test.sh`
  - `scripts/ci/third-party-licenses-audit.sh`
- **Description**: Add regression tests that assert strict/non-strict exit codes and expected diagnostic prefixes for clean, missing, and drift states.
- **Dependencies**:
  - Task 2.2
  - Task 2.3
- **Complexity**: 4
- **Acceptance criteria**:
  - Tests verify exit behavior for both strict and non-strict modes.
  - Tests verify PASS/WARN/FAIL output contract.
  - Tests run in CI-compatible shell environments.
- **Validation**:
  - `bash tests/third-party-licenses/audit.test.sh`

## Sprint 3: Release Packaging and License Artifact Compliance Gate

**Goal**: Ensure release assets always carry current third-party license documentation and fail before publish when contract breaks. **Demo/Validation**:

- Command(s):
  - `bash scripts/generate-third-party-licenses.sh --check`
  - `bash scripts/workflow-pack.sh --all`
  - `bash tests/third-party-licenses/release-bundle.test.sh`
  - `bash scripts/ci/release-bundle-third-party-audit.sh --tag v0.0.0-test --dist-dir dist/release-bundles`
- Verify:
  - Release workflow generates/checks license artifact before bundling.
  - Release upload set includes explicit `THIRD_PARTY_LICENSES.md` artifact (and checksum if configured).
  - Pre-upload release audit fails closed on missing/stale license artifact.
- **PR grouping intent**: per-sprint
- **Execution Profile**: serial
- Sprint scorecard:
  - `Execution Profile`: serial
  - `TotalComplexity`: 13
  - `CriticalPathComplexity`: 13
  - `MaxBatchWidth`: 1
  - `OverlapHotspots`: `.github/workflows/release.yml`; `scripts/ci/release-bundle-third-party-audit.sh`; `docs/RELEASE.md`

### Task 3.1: Regenerate and verify license artifact in release workflow

- **Location**:
  - `.github/workflows/release.yml`
  - `scripts/generate-third-party-licenses.sh`
- **Description**: Add release job steps that run generator `--write` and `--check` before release bundle creation.
- **Dependencies**:
  - Task 2.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Release job fails if generation/check fails.
  - Generation occurs before any bundle zip or release upload step.
- **Validation**:
  - `rg -n 'generate-third-party-licenses\.sh --write|generate-third-party-licenses\.sh --check' .github/workflows/release.yml`
  - `write_line=\"$(rg -n 'generate-third-party-licenses\\.sh --write' .github/workflows/release.yml | head -n1 | cut -d: -f1)\" && bundle_line=\"$(rg -n 'Build bundled release archive' .github/workflows/release.yml | head -n1 | cut -d: -f1)\" && upload_line=\"$(rg -n 'Upload release assets' .github/workflows/release.yml | head -n1 | cut -d: -f1)\" && test -n \"$write_line\" && test -n \"$bundle_line\" && test -n \"$upload_line\" && test \"$write_line\" -lt \"$bundle_line\" && test \"$bundle_line\" -lt \"$upload_line\"`

### Task 3.2: Include license artifact in release assets and checksums

- **Location**:
  - `.github/workflows/release.yml`
  - `docs/RELEASE.md`
- **Description**: Update release packaging and upload contract so `THIRD_PARTY_LICENSES.md` (and checksum file when configured) is published with release assets.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 3
- **Acceptance criteria**:
  - Release assets include `THIRD_PARTY_LICENSES.md` as a first-class artifact.
  - Release docs describe where the license file is published and how to validate it.
- **Validation**:
  - `rg -n 'THIRD_PARTY_LICENSES\.md|sha256' .github/workflows/release.yml docs/RELEASE.md`

### Task 3.3: Add release bundle third-party compliance audit script

- **Location**:
  - `scripts/ci/release-bundle-third-party-audit.sh`
  - `.github/workflows/release.yml`
- **Description**: Add a pre-upload audit script that validates release output contains required license artifact files and fails closed when missing.
- **Dependencies**:
  - Task 3.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Script supports explicit tag/dist-dir inputs for deterministic CI invocation.
  - Release workflow executes the audit before `softprops/action-gh-release`.
  - Failure diagnostics identify missing artifact paths.
- **Validation**:
  - `bash tests/third-party-licenses/release-bundle.test.sh`
  - `bash scripts/ci/release-bundle-third-party-audit.sh --tag v0.0.0-test --dist-dir dist/release-bundles`
  - `rg -n 'release-bundle-third-party-audit\.sh' .github/workflows/release.yml`

### Task 3.4: Add release audit fixture test and final docs wiring

- **Location**:
  - `tests/third-party-licenses/release-bundle.test.sh`
  - `TROUBLESHOOTING.md`
  - `docs/RELEASE.md`
- **Description**: Add release-fixture regression test plus final documentation wiring so operators can diagnose and recover from release license gate failures quickly.
- **Dependencies**:
  - Task 3.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Fixture test covers missing file and pass scenarios for release audit.
  - Troubleshooting and release docs include exact remediation command sequence.
- **Validation**:
  - `bash tests/third-party-licenses/release-bundle.test.sh`
  - `rg -n 'release-bundle-third-party-audit|generate-third-party-licenses|THIRD_PARTY_LICENSES' TROUBLESHOOTING.md docs/RELEASE.md`

## Testing Strategy

- Unit:
  - Generator helper parsing/normalization paths (sorting, escaping, metadata fallback logic) if extracted into helper
    functions.
- Integration:
  - Generator `--write/--check` regression suite and CI audit strict/non-strict behavior tests.
- E2E/manual:
  - Release bundle artifact audit on generated `dist/` fixtures and release-workflow command parity checks.

## Risks & gotchas

- Crates.io API availability can introduce transient failures for runtime crate metadata lookup.
- Node lockfile schema changes may break jq extraction unless schema compatibility is tested.
- Release gate ordering mistakes can produce stale artifacts if audit runs before generation.
- Overly strict diagnostics without remediation text can increase triage time during release windows.

## Rollback plan

- Revert CI and release workflow wiring first (`.github/workflows/ci.yml`, `.github/workflows/release.yml`) to unblock
  delivery while preserving existing build/test/package behavior.
- Keep generator script available for manual regeneration and commit-time verification.
- Temporarily downgrade `scripts/ci/third-party-licenses-audit.sh` to non-strict mode in local lint while issues are
  triaged.
- Preserve troubleshooting documentation and rerun full lint/test/package gates after rollback changes.

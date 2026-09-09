# Architecture

Repository architecture baseline:

- Workspace monorepo with shared Rust crates under `crates/`.
- Workflow adapters under `workflows/<id>/scripts` stay thin; domain logic lives in Rust crates.
- `crates/google-cli` is a scoped native Rust crate (package `nils-google-cli`, binary `google-cli`).
  It implements `auth/gmail/drive` directly via pinned Google API crates and local native modules.
- Shared runtime shell mechanics live in `scripts/lib/`.
- Packaging and validation use deterministic entrypoints under `scripts/`.
- Runtime target is Alfred on macOS; development/CI validation supports Linux and macOS.

Shared foundation extraction boundary:

- Share runtime mechanics in `scripts/lib/` (helper loading, script-filter guard drivers, smoke test helpers).
- Keep workflow-local domain semantics in `workflows/<id>/scripts` (provider mapping, ranking, workflow-specific copy/error UX).
- Enforce the boundary with:
  - `bash scripts/workflow-shared-foundation-audit.sh --check`
  - `bash scripts/workflow-sync-script-filter-policy.sh --check`

Documentation ownership boundaries:

- Repository/global navigation: [README.md](../README.md)
- Architecture/runtime baseline: [docs/ARCHITECTURE.md](ARCHITECTURE.md)
- CLI shared runtime/output contract:
  [docs/specs/cli-shared-runtime-contract.md](specs/cli-shared-runtime-contract.md)
- Native Google command contract + validation entrypoints:
  - [docs/specs/google-cli-native-contract.md](specs/google-cli-native-contract.md)
  - [crates/google-cli/README.md](../crates/google-cli/README.md)
- Workflow runtime/operator behavior: `workflows/<workflow-id>/README.md`

For operator standards and command gates, see:

- [ALFRED_WORKFLOW_DEVELOPMENT.md](../ALFRED_WORKFLOW_DEVELOPMENT.md)
- [DEVELOPMENT.md](../DEVELOPMENT.md)
- [docs/MAINTENANCE.md](MAINTENANCE.md)
- [docs/PACKAGING.md](PACKAGING.md)

## Spec navigation

Canonical contracts live in [`docs/specs/`](specs/). Specs are grouped below by domain. Every active spec opens
with a `> Status:` banner; treat the banner as the source of truth for whether a spec is binding.

### CLI runtime contracts

- [`cli-shared-runtime-contract.md`](specs/cli-shared-runtime-contract.md) —
  shared runtime/output contract for every CLI crate (entrypoints, json mode, exit codes).
- [`cli-json-envelope-v1.md`](specs/cli-json-envelope-v1.md) —
  v1 JSON envelope shape (`status`, `data`, `error`) emitted by every CLI subcommand.
- [`cli-error-code-registry.md`](specs/cli-error-code-registry.md) —
  registry of allowed `error_code` values, with provenance and historical reservations.

### Workflow and shared-foundation policies

- [`workflow-shared-foundations-policy.md`](specs/workflow-shared-foundations-policy.md) —
  extraction boundary, helper inventory, rollout/rollback rules for `scripts/lib/*`.
- [`workflow-script-refactor-contract.md`](specs/workflow-script-refactor-contract.md) —
  per-workflow script refactor checklist and migration guarantees.
- [`script-filter-input-policy.md`](specs/script-filter-input-policy.md) —
  Alfred Script Filter input policy (queue delay, alfredfiltersresults, `config.type`).
- [`crate-docs-placement-policy.md`](specs/crate-docs-placement-policy.md) —
  required `crates/<name>/README.md` + `crates/<name>/docs/README.md`, allowed root markdown set, and
  `docs/` category list.

### CI and release contracts

- [`ci-refactor-contract.md`](specs/ci-refactor-contract.md) —
  CI gate ordering, sprint/task labels, and rollout invariants for the `validate` job.
- [`third-party-artifacts-contract-v1.md`](specs/third-party-artifacts-contract-v1.md) —
  contract for generating and shipping `THIRD_PARTY_*.md` plus `.sha256` release-bundle assets.
- [`third-party-license-artifact-contract-v1.md`](specs/third-party-license-artifact-contract-v1.md) —
  superseded forwarding stub; redirects to the canonical artifacts contract above.

### Per-domain contracts

- [`google-cli-native-contract.md`](specs/google-cli-native-contract.md) —
  `nils-google-cli` native command tree (`auth/gmail/drive`) and config/env-var contract.
- [`steam-search-workflow-contract.md`](specs/steam-search-workflow-contract.md) —
  Steam region rotation, language switching, and result-row contract.

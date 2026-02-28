# Architecture

Repository architecture baseline:

- Workspace monorepo with shared Rust crates under `crates/`.
- Workflow adapters under `workflows/<id>/scripts` stay thin; domain logic lives in Rust crates.
- `crates/google-cli` is a scoped native Rust crate.
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

For operator standards and command gates, see:

- `ALFRED_WORKFLOW_DEVELOPMENT.md`
- `DEVELOPMENT.md`

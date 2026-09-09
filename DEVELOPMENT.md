# Development Guide

Principles and routine workflow for maintaining `nils-alfredworkflow`.
Command-by-command setup, build, lint, test, and coverage instructions live in
the [`maintenance reference`](docs/MAINTENANCE.md).

## Maintenance principles

- Keep reusable behavior in shared Rust crates and workflow adapters thin.
- Keep generated Alfred bundles, workflow metadata, lockfiles, and compliance
  artifacts synchronized with their canonical sources.
- Preserve Linux as the portable development and CI baseline. Treat packaging,
  installation, Alfred UI checks, and live workflow probes as macOS-specific
  acceptance.
- Never commit credentials, local Alfred state, generated runtime state, or
  live-probe output.
- Keep crate-owned documentation beside its crate and workspace-wide material
  in the governed root or `docs/` categories.
- Prefer the smallest focused test while iterating; run the repository-owned
  finish-line gate once for the final candidate.

## Change workflow

1. Classify the change by owner: shared crate, workflow adapter, generated
   bundle, packaging, dependency/tooling, documentation, or release.
2. Inspect affected callers, tests, manifests, generated artifacts, and
   operator documentation before editing.
3. Capture a meaningful regression failure for testable behavior when
   practical. For documentation-only work, validate ownership, links, lint,
   and routing instead.
4. Make the smallest observable change and keep every generated or packaged
   surface aligned with its source.
5. Run focused checks, then the routine finish-line command:

   ```bash
   bash scripts/local-pre-commit.sh
   ```

6. Add package smoke, live probes, macOS packaging, install, or Alfred UI
   acceptance only when the changed surface requires them.

## Documentation routing

| Need | Canonical document |
| --- | --- |
| Setup, build, lint, test, and coverage commands | [`docs/MAINTENANCE.md`](docs/MAINTENANCE.md) |
| Toolchain and external dependencies | [`BINARY_DEPENDENCIES.md`](BINARY_DEPENDENCIES.md) |
| Workflow runtime and Script Filter standards | [`ALFRED_WORKFLOW_DEVELOPMENT.md`](ALFRED_WORKFLOW_DEVELOPMENT.md) |
| Architecture and ownership boundaries | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| Packaging, install, and macOS acceptance | [`docs/PACKAGING.md`](docs/PACKAGING.md) |
| Release and publishing | [`docs/RELEASE.md`](docs/RELEASE.md) |
| Workflow-specific operation and recovery | `workflows/<workflow-id>/README.md` and `TROUBLESHOOTING.md` |

`AGENTS.md` owns agent-specific repository rules. Detailed contracts remain in
`docs/specs/`, and retained plans or reports do not override current source,
policy, or canonical documentation.

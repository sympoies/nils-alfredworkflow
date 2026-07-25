# AGENTS.md

## Scope

Repo-local policy for `nils-alfredworkflow`. Inherit the home agent policy and
keep this file limited to project-specific boundaries and routing.

## Project Boundaries

- Shared Rust crates own reusable workflow behavior; generated Alfred bundles
  and checked-in workflow metadata must stay synchronized with their sources.
- Keep credentials and local Alfred state out of git. Live API probes are
  task-specific acceptance and must not become the ordinary repository gate.
- Linux owns the portable development gate. Packaging, install, and Alfred UI
  acceptance remain macOS-specific.

## Documentation Routing

Read only the references relevant to the requested change:

- Day-to-day commands and validation: `DEVELOPMENT.md`
- External tool setup or degradation: `BINARY_DEPENDENCIES.md`
- Script Filter, workflow runtime, or troubleshooting conventions:
  `ALFRED_WORKFLOW_DEVELOPMENT.md`
- Workflow-specific operation and recovery:
  `workflows/<workflow-id>/TROUBLESHOOTING.md`
- Packaging, install, or macOS acceptance: `docs/PACKAGING.md`
- Release and publish work: `docs/RELEASE.md`

## Validation

- The ordinary finish-line command is
  `bash scripts/local-pre-commit.sh`; its script owns the portable lint, policy,
  scraper, and workflow test phases.
- During iteration, run the smallest affected test or audit. Add package smoke,
  live probes, or macOS acceptance only when the changed surface requires them.

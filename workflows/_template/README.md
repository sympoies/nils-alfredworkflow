# Workflow Template - Alfred Workflow Scaffold

Starter scaffold for creating a new workflow in this monorepo.

## Features

- Includes baseline workflow files: `workflow.toml`, `src/info.plist.template`, `scripts/`, and `tests/smoke.sh`.
- Keeps script-filter and action script structure aligned with existing workflows.
- Uses the same packaging conventions expected by `scripts/workflow-pack.sh`.
- Script entrypoints default to shared foundation bootstrap:
  - `scripts/lib/workflow_helper_loader.sh`
  - `scripts/lib/script_filter_cli_driver.sh` (script filter safety guard)
  - `scripts/lib/workflow_smoke_helpers.sh` (smoke helper scaffolding)
  - `scripts/lib/workflow_cli_resolver.sh` (`wfcr_resolve_binary` runtime resolution)

## Template Parameters

Update these fields before packaging a new workflow:

| Field           | File            | Description                                                 |
| --------------- | --------------- | ----------------------------------------------------------- |
| `id`            | `workflow.toml` | Workflow id slug (used for packaging path and identifiers). |
| `name`          | `workflow.toml` | Human-readable workflow name shown in Alfred.               |
| `bundle_id`     | `workflow.toml` | Unique Alfred bundle id (`com.example.<id>` style).         |
| `script_filter` | `workflow.toml` | Script file name for script filter entrypoint.              |
| `action`        | `workflow.toml` | Script file name for action entrypoint.                     |
| `rust_binary`   | `workflow.toml` | Binary name packaged into `bin/` for runtime scripts.       |

## Example Configuration Variables

| Variable      | Required | Default   | Description                                                              |
| ------------- | -------- | --------- | ------------------------------------------------------------------------ |
| `EXAMPLE_VAR` | No       | `example` | Demonstration variable placeholder. Replace with real workflow settings. |

## Usage Notes

- This folder is a template, not a final end-user workflow.
- After scaffolding, replace placeholder values (for example `__WORKFLOW_ID__`) and update scripts/tests accordingly.
- Preserve shared-vs-local extraction boundary during customization:
  - keep helper wiring and guard mechanics shared;
  - keep domain/provider semantics local to the workflow script.
- Keep bundled runtime resolution on shared policy:
  - source `workflow_cli_resolver.sh`;
  - resolve package/release/debug binaries through `wfcr_resolve_binary`.
- Every new workflow README must include a `## Troubleshooting` section that links to `./TROUBLESHOOTING.md`.

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

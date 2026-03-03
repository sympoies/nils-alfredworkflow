# Workflow Script Refactor Contract

## Scope

This contract freezes Sprint 3 shared-lane behavior for Task 3.1, Task 3.2, and Task 3.4:

- Build a workflow script inventory from workflow manifests.
- Standardize shared helper-loader and script-filter runtime mechanics.
- Enforce non-orphan checks in lint while preserving required non-manifest hooks.

## Canonical Entrypoint Contract

1. Every workflow must keep canonical manifest entrypoint declarations in `workflows/<id>/workflow.toml`:
   - `script_filter = "..."`
   - `action = "..."`
2. Runtime script files under `workflows/<id>/scripts/*.sh` are allowed only when at least one of the following is true:
   - It is a manifest entrypoint.
   - It is referenced by workflow plist script objects (`./scripts/*.sh`).
   - It is referenced by another workflow runtime script.
3. Files that match none of the above are treated as orphan scripts and fail `bash scripts/workflow-shared-foundation-audit.sh --check`.

## Shared Helper Stack Requirements

### Loader and helper resolution

- Script filters/actions must source `workflow_helper_loader.sh`.
- Required helpers must be loaded through `wfhl_source_required_helper`.
- Legacy per-script helper-missing branches should be replaced with shared helper primitives.

### Script-filter runtime drivers

- Search-family script filters must use:
  - `script_filter_query_policy.sh`
  - `script_filter_async_coalesce.sh`
  - `script_filter_search_driver.sh`
- Non-search script filters must use `script_filter_cli_driver.sh`.

### Action wrappers

- URL open actions must route through `scripts/lib/workflow_action_open_url.sh`.
- Clipboard actions must route through `scripts/lib/workflow_action_copy.sh`.

## Required Non-Manifest Scripts (Task 3.4)

The following scripts are required by workflow packaging/tests and must remain present:

| File | Required reason | Usage evidence |
| --- | --- | --- |
| `workflows/google-search/scripts/script_filter_direct.sh` | Direct-search entrypoint is still wired in workflow plist and smoke coverage. | `workflows/google-search/src/info.plist.template` and `workflows/google-search/tests/smoke.sh` reference `script_filter_direct.sh`. |
| `workflows/codex-cli/scripts/prepare_package.sh` | Packaging hook is consumed by workflow pack flow and codex-cli smoke tests. | `scripts/workflow-pack.sh` and `workflows/codex-cli/tests/smoke.sh` call `prepare_package.sh`. |

Removing either file is a regression and must fail validation.

## Non-Orphan Enforcement

`bash scripts/workflow-shared-foundation-audit.sh --check` is the enforcement gate and must:

1. Validate shared-foundation wiring and driver usage for migrated scripts.
2. Fail when required non-manifest hook scripts are missing/non-executable.
3. Fail when new orphan workflow scripts appear.
4. Keep a documented exemption list for utility scripts that are intentionally non-runtime.

`scripts/workflow-lint.sh` must keep this audit in its default lint path so orphan checks run in normal developer/CI lint entrypoints.

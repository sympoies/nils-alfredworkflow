# Workflow Name Troubleshooting (Template)

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

Use this file as a scaffold for new workflows. Replace all `<PLACEHOLDER>` values.

## Quick operator checks

Run from repository root.

```bash
# Required scripts (replace placeholders)
ls -l \
  workflows/<WORKFLOW_ID>/scripts/<SCRIPT_FILTER_ENTRY>.sh \
  workflows/<WORKFLOW_ID>/scripts/<ACTION_ENTRY>.sh

# Runtime candidate check
# Keep this conservative: check bundled binary first, then PATH
# If your workflow is external-runtime based, also include explicit env override check.
test -x workflows/<WORKFLOW_ID>/bin/<RUNTIME_BIN> && echo "bundled <RUNTIME_BIN> found"
command -v <RUNTIME_BIN> || true

# Script filter should return Alfred JSON rows
bash workflows/<WORKFLOW_ID>/scripts/<SCRIPT_FILTER_ENTRY>.sh "<SAMPLE_QUERY>" | jq -e '.items | type == "array"'

# Shared foundation bootstrap markers should be present
rg -n "workflow_helper_loader|wfhl_source_helper|sfcd_run_cli_flow" \
  workflows/<WORKFLOW_ID>/scripts/<SCRIPT_FILTER_ENTRY>.sh \
  workflows/<WORKFLOW_ID>/scripts/<ACTION_ENTRY>.sh
rg -n "workflow_smoke_helpers" workflows/<WORKFLOW_ID>/tests/smoke.sh

# Confirm manifest defaults
cat workflows/<WORKFLOW_ID>/workflow.toml
```

## Common failures and actions

Document at least these rows for each new workflow:

| Symptom                              | Likely cause                           | Action                                                                               |
| ------------------------------------ | -------------------------------------- | ------------------------------------------------------------------------------------ |
| `<runtime binary not found message>` | `<binary missing or wrong path>`       | `Re-package workflow and/or set <RUNTIME_OVERRIDE_ENV> to executable absolute path.` |
| `<invalid input message>`            | `<bad query/argument format>`          | `Provide one valid example query and expected format.`                               |
| `<provider/network failure message>` | `<upstream transient>`                 | `Retry later first; do not assume local script defect immediately.`                  |
| `<output format error message>`      | `<malformed Alfred JSON from runtime>` | `Use pinned packaged runtime or update override binary.`                             |

## Validation

```bash
bash workflows/<WORKFLOW_ID>/tests/smoke.sh
scripts/workflow-test.sh --id <WORKFLOW_ID>
scripts/workflow-pack.sh --id <WORKFLOW_ID>
bash scripts/workflow-shared-foundation-audit.sh --check
bash scripts/workflow-sync-script-filter-policy.sh --check --workflows <WORKFLOW_ID>
```

Optional (if workflow has extra acceptance checks):

```bash
# Example:
bash workflows/<WORKFLOW_ID>/tests/<extra-check>.sh
```

## Rollback guidance

1. Re-install previous known-good artifact from `dist/<WORKFLOW_ID>/<version>/`.
2. Reset workflow variables to documented defaults in `workflow.toml`.
3. If source regression remains, roll back only `workflows/<WORKFLOW_ID>/` on a branch and rerun Validation commands.

## Placeholder checklist

- Replace `<WORKFLOW_ID>` with `workflow.toml` `id`.
- Replace `<RUNTIME_BIN>` and `<RUNTIME_OVERRIDE_ENV>` with actual runtime binary/env names.
- Replace `<SCRIPT_FILTER_ENTRY>`, `<ACTION_ENTRY>`, `<SAMPLE_QUERY>` with real entrypoints and query.
- Ensure failure rows map to actual script error titles/subtitles.

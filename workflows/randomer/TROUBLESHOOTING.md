# Randomer Workflow Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

Run from repository root.

```bash
# Required scripts
ls -l \
  workflows/randomer/scripts/script_filter.sh \
  workflows/randomer/scripts/script_filter_types.sh \
  workflows/randomer/scripts/script_filter_expand.sh \
  workflows/randomer/scripts/action_open.sh

# Runtime candidate check
test -x workflows/randomer/bin/randomer-cli && echo "bundled randomer-cli found"
command -v randomer-cli || true

# Primary/type/expand filters should all return Alfred JSON rows
bash workflows/randomer/scripts/script_filter.sh "uuid" | jq -e '.items | type == "array"'
bash workflows/randomer/scripts/script_filter_types.sh "in" | jq -e '.items | type == "array"'
bash workflows/randomer/scripts/script_filter_expand.sh "uuid" | jq -e '.items | type == "array" and length == 10'
```

## Common failures and actions

| Symptom                             | Likely cause                                  | Action                                                                                     |
| ----------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `randomer-cli binary not found` row | Runtime binary absent in all lookup paths     | Re-package workflow or set `RANDOMER_CLI_BIN` to executable absolute path.                 |
| `Select a format first` row         | Expand stage triggered without format         | Use `rrv <type>` first, then open expanded list; or run expand with explicit format query. |
| `Unknown format` row                | Unsupported key passed to `generate --format` | Retry with supported keys shown by `rr`/`rrv`.                                             |
| `Randomer output format error`      | Non-conforming JSON from custom binary        | Use packaged pinned runtime, or update override binary.                                    |

For environment-driven expand triage:

```bash
RANDOMER_FORMAT="uuid" bash workflows/randomer/scripts/script_filter_expand.sh | jq -e '.items | type == "array" and length == 10'
```

## Validation

```bash
bash workflows/randomer/tests/smoke.sh
scripts/workflow-test.sh --id randomer
scripts/workflow-pack.sh --id randomer
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/randomer/<version>/`.
2. Remove temporary runtime override (`RANDOMER_CLI_BIN`) and retest with packaged binary.
3. If needed, roll back only `workflows/randomer/` on a branch and rerun all Validation commands.

# Codex CLI Workflow Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

Run from repository root.

```bash
# Required scripts and runtime metadata
ls -l \
  workflows/codex-cli/scripts/script_filter.sh \
  workflows/codex-cli/scripts/action_open.sh \
  workflows/codex-cli/scripts/lib/codex_cli_runtime.sh

# Runtime candidate check (bundled first, then PATH)
test -x workflows/codex-cli/bin/codex-cli && echo "bundled codex-cli found"
command -v codex-cli || true

# Script filter should always return Alfred JSON rows (success or mapped error row)
bash workflows/codex-cli/scripts/script_filter.sh "help" | jq -e '.items | type == "array"'

# Confirm pinned runtime metadata used by packaging/runtime docs
sed -n '1,120p' workflows/codex-cli/scripts/lib/codex_cli_runtime.sh
```

## Common failures and actions

| Symptom                                           | Likely cause                                  | Action                                                                                                                            |
| ------------------------------------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `codex-cli binary not found` row                  | Bundled binary missing and no usable fallback | Re-package workflow (`scripts/workflow-pack.sh --id codex-cli`) or set `CODEX_CLI_BIN` to an executable absolute path.            |
| `auth save`/`auth remove`/`auth use` actions fail | Secret directory is unset/unwritable          | In Alfred workflow variables, set `CODEX_SECRET_DIR` to a writable directory (for example `~/.config/codex_secrets`), then retry. |
| `diag`/`cxda result` looks stale                  | Diag cache still within TTL                   | Temporarily set `CODEX_DIAG_CACHE_TTL_SECONDS=0`, rerun query, then restore default (`300`).                                      |
| `diag` requests appear blocked too long           | Contention on refresh lock                    | Reduce contention and verify `CODEX_DIAG_CACHE_BLOCK_WAIT_SECONDS` (default `15`).                                                |
| macOS blocks bundled executable                   | Quarantine attribute on packaged binary       | Re-import workflow and retry. If needed, clear quarantine on installed workflow bundle per root troubleshooting policy.           |

For conservative local cache cleanup during triage:

```bash
rm -f "${TMPDIR:-/tmp}/nils-codex-cli-workflow"/diag-rate-limits.* || true
```

## Validation

```bash
bash workflows/codex-cli/tests/smoke.sh
scripts/workflow-test.sh --id codex-cli
scripts/workflow-pack.sh --id codex-cli
```

Optional focused checks:

```bash
bash workflows/codex-cli/scripts/script_filter_diag.sh "diag"
bash workflows/codex-cli/scripts/script_filter_diag_all.sh "diag all-json"
```

## Rollback guidance

1. Re-install the last known good packaged artifact from `dist/codex-cli/<version>/`.
2. Reset workflow variables to known defaults (`CODEX_CLI_BIN=""`, `CODEX_SECRET_DIR=""`,
   `CODEX_DIAG_CACHE_TTL_SECONDS="300"`, `CODEX_DIAG_CACHE_BLOCK_WAIT_SECONDS="15"`).
3. If regression is source-level, roll back `workflows/codex-cli/` on a dedicated branch and re-run the Validation
   commands before re-packaging.

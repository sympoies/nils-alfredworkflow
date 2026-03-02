# Google Service Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick checks

Run from repository root.

```bash
# Runtime and script entrypoints
ls -l \
  workflows/google-service/scripts/script_filter_empty.sh \
  workflows/google-service/scripts/script_filter.sh \
  workflows/google-service/scripts/action_open.sh

# google-cli / jq availability
command -v google-cli || true
command -v jq || true

# Script filter JSON output
bash workflows/google-service/scripts/script_filter_empty.sh "" | jq -e '.items | type == "array"'
bash workflows/google-service/scripts/script_filter.sh "" | jq -e '.items | type == "array"'

# Workflow-local active account file (when workflow has been used)
ls -l "${ALFRED_WORKFLOW_DATA:-}"/active-account.v1.json 2>/dev/null || true
```

## Common failures

| Symptom | Likely cause | Action |
| --- | --- | --- |
| `google-cli binary not found` | Runtime not installed/resolved | Install `nils-google-cli` or set `GOOGLE_CLI_BIN` to absolute executable path. |
| `No accounts configured` but files exist under `~/.config/google/credentials` | Workflow is reading a different config dir | Keep `GOOGLE_CLI_CONFIG_DIR` empty (auto-fallback to `~/.config/google/credentials`) or set it explicitly to that directory. |
| `jq is required to parse google-cli JSON output` | `jq` missing | Install `jq` and rerun workflow. |
| `gs` shows `(none)` | No workflow active account and no native default account available | Run `gsa login <email>` then `gsa switch <email>`. |
| `NILS_GOOGLE_005` | Invalid auth input (`state`/`code`/account missing) | Re-run with full arguments and check remote/manual command format. |
| Step 2 feels too verbose | Manually extracting `state` and `code` is cumbersome | Use shortcut: `gsa login <callback-url>` (or `gsa <callback-url>`). |
| `Cannot resolve account for step 2` | Callback `state` has no matching pending remote login | Re-run `login <email>` (step 1), then paste the new callback URL. |
| `NILS_GOOGLE_006` | Ambiguous account selection in native auth | Remove ambiguity by setting native default account or reducing account set. |
| `NILS_GOOGLE_008` | Remote step 2 state mismatch | Restart from `login <email>` (remote step 1) and use the newly generated state/code pair. |
| Remove cancelled silently | Confirmation dialog dismissed | Re-run remove and confirm, or use `remove --yes <email>`. |

## Validation commands

```bash
bash workflows/google-service/tests/smoke.sh
bash scripts/workflow-sync-script-filter-policy.sh --check --workflows google-service
scripts/workflow-pack.sh --id google-service
```

## Rollback

1. Re-import previous known-good `.alfredworkflow` artifact.
2. Clear workflow-local active pointer if needed:
   - `rm -f "$ALFRED_WORKFLOW_DATA/active-account.v1.json"`
3. Re-run validation commands above.

# multi-timezone Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id multi-timezone --install`
2. Confirm Alfred workflow variables are valid:
   - `TIMEZONE_CLI_BIN` (optional; executable timezone-cli override path)
   - `MULTI_TZ_ZONES` (optional; comma/newline separated IANA timezone IDs)
   - `MULTI_TZ_LOCAL_OVERRIDE` (optional; default `Europe/London`, used in local fallback mode)
3. Confirm script-filter contract output is JSON:
   - `bash workflows/multi-timezone/scripts/script_filter.sh "Asia/Taipei,America/New_York" | jq -e '.items | type == "array"'`
4. Confirm empty-query fallback behavior:
   - `MULTI_TZ_ZONES="" MULTI_TZ_LOCAL_OVERRIDE="Asia/Taipei" bash workflows/multi-timezone/scripts/script_filter.sh "" \`
     `| jq -e '.items[0].uid == "Asia/Taipei"'`

## Common failures and actions

| Symptom in Alfred                                         | Likely cause                                                                                                                                                                  | Action                                                                                                                       |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `Invalid timezone`                                        | Query/config includes non-IANA timezone IDs (for example typos such as `Asia/Taipe` or unsupported zones).                                                                    | Replace with valid IANA timezone IDs (`Region/City`, for example `Asia/Taipei`) and keep comma/newline separators only.      |
| `timezone-cli binary not found`                           | Packaged binary missing, `TIMEZONE_CLI_BIN` points to non-executable path, or runtime path resolution failed.                                                                 | Re-pack workflow, or set `TIMEZONE_CLI_BIN` to an executable `timezone-cli` path and retry.                                  |
| `Timezone runtime failure`                                | `timezone-cli` hit runtime/IO failure (timeout/internal error/panic).                                                                                                         | Retry query, inspect stderr from `script_filter.sh`, and verify `timezone-cli` build/runtime integrity.                      |
| Empty `tz` query shows unexpected local timezone or `UTC` | Query and `MULTI_TZ_ZONES` are both empty, so fallback chain uses `MULTI_TZ_LOCAL_OVERRIDE` first (default `Europe/London`); terminal fallback is `UTC` when all probes fail. | Set `MULTI_TZ_ZONES` or `MULTI_TZ_LOCAL_OVERRIDE` for deterministic output; otherwise treat `UTC` as expected safe fallback. |

## Validation

- Re-run quick operator checks after any runtime/config change.
- Recommended workflow check: `bash workflows/multi-timezone/tests/smoke.sh`

## Rollback guidance

Use this when timezone output is unstable or local fallback behavior regresses.

1. Stop rollout of new `multi-timezone` artifacts (pause release/distribution link).
2. Revert Multi Timezone changeset(s), including:
   - `workflows/multi-timezone/`
   - `crates/timezone-cli/`
   - workspace member changes in `Cargo.toml`
   - docs updates tied to rollout (`crates/timezone-cli/docs/workflow-contract.md`,
     `workflows/multi-timezone/README.md`, `workflows/multi-timezone/TROUBLESHOOTING.md`, and
     `ALFRED_WORKFLOW_DEVELOPMENT.md` if changed)
3. Rebuild and validate rollback state:
   - `scripts/workflow-lint.sh`
   - `scripts/workflow-test.sh`
   - `scripts/workflow-pack.sh --all`
4. Publish known-good artifact set and post operator notice:
   - Explain that `multi-timezone` is temporarily disabled.
   - Provide ETA/workaround and support contact path.

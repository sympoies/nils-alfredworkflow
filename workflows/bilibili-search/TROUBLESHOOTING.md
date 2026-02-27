# bilibili-search Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id bilibili-search --install`
2. Confirm Alfred workflow variables are valid:
   - `BILIBILI_UID` (optional)
   - `BILIBILI_MAX_RESULTS` (optional, default `10`)
   - `BILIBILI_TIMEOUT_MS` (optional, default `8000`)
3. Confirm script-filter contract output is JSON:
   - `bash workflows/bilibili-search/scripts/script_filter.sh "naruto" | jq -e '.items | type == "array"'`
4. Confirm deterministic checks pass:
   - `bash workflows/bilibili-search/tests/smoke.sh`

## Common failures and actions

| Symptom in Alfred                  | Likely cause                                                      | Action                                                            |
| ---------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------- |
| `Invalid Bilibili workflow config` | `BILIBILI_MAX_RESULTS` or `BILIBILI_TIMEOUT_MS` value is invalid. | Correct variable values and retry.                                |
| `Bilibili API unavailable`         | DNS/TLS/network timeout, malformed payload, or upstream `5xx`.    | Check local network and retry; if sustained, pause rollout.       |
| `No suggestions found`             | Query has no suggest rows.                                        | Press Enter on direct-search fallback row or use a broader query. |
| `Keep typing (2+ chars)`           | Query is shorter than minimum length (`<2`).                      | Continue typing until at least 2 characters.                      |

## Validation

Run these checks after any runtime/config change:

- `bash workflows/bilibili-search/tests/smoke.sh`
- `scripts/workflow-test.sh --id bilibili-search`
- `scripts/workflow-pack.sh --id bilibili-search`

## First-release support window (D0-D2)

- Monitor failure classes separately: invalid config, API unavailable, malformed payload, empty suggestions.
- Emergency disable triggers:
  - Script-filter malformed JSON observed at any time.
  - API unavailable failures exceed 30% of sampled queries for 30 minutes.
- Operator response template:
  - Current status (degraded/disabled)
  - Scope (`bilibili-search` only)
  - Workaround (manual browser search)
  - Next update time

## Rollback guidance

Use this when API failures are sustained or workflow usability drops sharply.

1. Stop rollout of new `bilibili-search` artifacts.
2. Revert bilibili-search changeset(s), including:
   - `workflows/bilibili-search/`
   - `crates/bilibili-cli/`
   - workspace membership in `Cargo.toml`
   - docs updates tied to rollout (`workflows/bilibili-search/README.md`,
     `workflows/bilibili-search/TROUBLESHOOTING.md`, and `ALFRED_WORKFLOW_DEVELOPMENT.md` if changed)
3. Rebuild and validate rollback state:
   - `scripts/workflow-lint.sh`
   - `scripts/workflow-test.sh`
   - `scripts/workflow-pack.sh --all`
4. Publish known-good artifact set and note temporary workaround.

# spotify-search Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id spotify-search --install`
2. Confirm Alfred workflow variables are set:
   - `SPOTIFY_CLIENT_ID` (required)
   - `SPOTIFY_CLIENT_SECRET` (required)
   - `SPOTIFY_MAX_RESULTS` (optional)
   - `SPOTIFY_MARKET` (optional)
3. Confirm script-filter contract output is JSON:
   - `bash workflows/spotify-search/scripts/script_filter.sh "test query" | jq -e '.items | type == "array"'`
4. Confirm queue policy is synced:
   - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows spotify-search`

## Common failures and actions

| Symptom in Alfred | Likely cause | Action |
| --- | --- | --- |
| `Spotify credentials are missing` | `SPOTIFY_CLIENT_ID` / `SPOTIFY_CLIENT_SECRET` unset in Alfred workflow variables. | Set both variables, then retry. |
| `Keep typing (2+ chars)` | Query is shorter than minimum length (`<2`). | Continue typing until at least 2 characters; no API request is sent before that. |
| `Searching Spotify...` persists briefly while typing | Query coalescing is waiting for the latest query to stabilize. | Pause typing momentarily and wait for Alfred rerun to complete the final query. |
| `Spotify credentials are invalid` | Wrong credential pair or revoked app secret. | Regenerate or replace credentials and retry. |
| `Spotify API rate limited` | 429 from Spotify. | Retry later and reduce `SPOTIFY_MAX_RESULTS` if bursts are frequent. |
| `Spotify API unavailable` | Network/transport/upstream outage. | Check network and retry later; treat as transient first. |
| `spotify-cli binary not found` | Runtime binary missing from lookup paths. | Re-package workflow or set `SPOTIFY_CLI_BIN` explicitly. |

Conservative action-open check is covered by smoke test stubs; avoid forcing live `open` calls during incident triage
unless needed.

## Validation

- Re-run quick operator checks after any runtime/config change.
- Recommended workflow check: `bash workflows/spotify-search/tests/smoke.sh`

## Rollback guidance

1. Re-install the previous known-good package from `dist/spotify-search/<version>/`.
2. Reset workflow variables to stable defaults (`SPOTIFY_MAX_RESULTS="10"`, `SPOTIFY_MARKET=""`) and verify
   credentials again.
3. If regression is code-level, roll back only `workflows/spotify-search/` in a branch and rerun all Validation
   commands before shipping.

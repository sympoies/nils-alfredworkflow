# Spotify Search Workflow Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

Run from repository root.

```bash
# Required scripts
ls -l \
  workflows/spotify-search/scripts/script_filter.sh \
  workflows/spotify-search/scripts/action_open.sh

# Runtime candidate check
test -x workflows/spotify-search/bin/spotify-cli && echo "bundled spotify-cli found"
command -v spotify-cli || true

# Script filter should return Alfred JSON rows (including mapped error rows)
bash workflows/spotify-search/scripts/script_filter.sh "test query" | jq -e '.items | type == "array"'

# Confirm env keys expected by workflow
rg -n "SPOTIFY_CLIENT_ID|SPOTIFY_CLIENT_SECRET|SPOTIFY_MAX_RESULTS|SPOTIFY_MARKET" workflows/spotify-search/workflow.toml
```

Credential presence check (local shell context):

```bash
printf 'SPOTIFY_CLIENT_ID=%s\n' "${SPOTIFY_CLIENT_ID:+set}"
printf 'SPOTIFY_CLIENT_SECRET=%s\n' "${SPOTIFY_CLIENT_SECRET:+set}"
```

## Common failures and actions

| Symptom                           | Likely cause                                                                     | Action                                                            |
| --------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `Spotify credentials are missing` | `SPOTIFY_CLIENT_ID` / `SPOTIFY_CLIENT_SECRET` unset in Alfred workflow variables | Set both variables, then re-run query.                            |
| `Spotify credentials are invalid` | Wrong credential pair or revoked app secret                                      | Regenerate/replace credentials and retry.                         |
| `Spotify API rate limited`        | 429 from Spotify                                                                 | Retry later; reduce `SPOTIFY_MAX_RESULTS` if bursts are frequent. |
| `Spotify API unavailable`         | Network/transport/upstream outage                                                | Check network and retry later; treat as transient first.          |
| `spotify-cli binary not found`    | Runtime binary missing from lookup paths                                         | Re-package workflow or set `SPOTIFY_CLI_BIN` explicitly.          |

Conservative action-open check is covered by smoke test stubs; avoid forcing live `open` calls during incident triage
unless needed.

## Validation

```bash
bash workflows/spotify-search/tests/smoke.sh
scripts/workflow-test.sh --id spotify-search
scripts/workflow-pack.sh --id spotify-search
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/spotify-search/<version>/`.
2. Reset workflow variables to stable defaults (`SPOTIFY_MAX_RESULTS="10"`, `SPOTIFY_MARKET=""`) and verify credentials
   again.
3. If regression is code-level, roll back only `workflows/spotify-search/` in a branch and rerun all Validation commands
   before shipping.

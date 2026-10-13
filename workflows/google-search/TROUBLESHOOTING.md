# google-search Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id google-search --install`
2. Confirm Alfred workflow variables are set:
   - `BRAVE_API_KEY` (required)
   - `BRAVE_MAX_RESULTS` (optional)
   - `BRAVE_SAFESEARCH` (optional)
   - `BRAVE_COUNTRY` (optional)
3. Confirm two-stage (`gg`) script-filter output is JSON:
   - `bash workflows/google-search/scripts/script_filter.sh "rust language" | jq -e '.items | type == "array"'`
4. Confirm direct (`gb`) script-filter output is JSON:
   - `bash workflows/google-search/scripts/script_filter_direct.sh "rust language" | jq -e '.items | type == "array"'`
5. Confirm queue policy is synced:
   - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows google-search`

## Common failures and actions

| Symptom in Alfred | Likely cause | Action |
| --- | --- | --- |
| `Google suggestions unavailable` | Suggest endpoint request failed or returned invalid payload. | Retry shortly, check network, or switch to direct Brave mode (`gb`). |
| `Brave API key is missing` | `BRAVE_API_KEY` is empty/missing. | Set a valid token in workflow config, then retry. |
| `Keep typing (2+ chars)` | Query is shorter than minimum length (`<2`). | Continue typing until at least 2 characters; no API request is sent before that. |
| `Brave API rate limited` | Rate limit/quota exhausted (`429`/quota errors). | Wait and retry later, reduce query frequency, and lower `BRAVE_MAX_RESULTS`. |
| `Brave API unavailable` | Network/DNS/TLS issue, timeout, or upstream `5xx`. | Check local network/DNS, retry later, and verify Brave API status. |
| `No results found` | Query is too narrow or country/safesearch filters are restrictive. | Use broader keywords or adjust `BRAVE_COUNTRY`/`BRAVE_SAFESEARCH`. |
| `"brave-cli" Not Opened` / `Apple could not verify ...` | Downloaded/packaged `brave-cli` carries `com.apple.quarantine`; Gatekeeper blocks execution. | Run `./workflow-clear-quarantine-standalone.sh --id google-search` (from release assets), then retry Alfred query. |

## Validation

- Re-run quick operator checks after any runtime/config change.
- Recommended workflow check: `bash workflows/google-search/tests/smoke.sh`

First-release support checklist:

- Track missing-key, quota/rate-limit, and API/network errors separately.
- If quota/rate-limit errors spike, lower default `BRAVE_MAX_RESULTS` and notify operators.
- Keep fallback titles/subtitles stable so support can match screenshots quickly.
- Record short incident notes for each production-facing outage window.

## Rollback guidance

Use this when Brave API failures are sustained or workflow usability drops sharply.

1. Stop rollout of new `google-search` artifacts (pause release/distribution link).
2. Revert Google-search changeset(s), including:
   - `workflows/google-search/`
   - `crates/brave-cli/`
   - workspace member changes in `Cargo.toml`
   - docs updates tied to rollout (`crates/brave-cli/docs/workflow-contract.md` and rollout references)
3. Rebuild and validate rollback state:
   - `scripts/workflow-lint.sh`
   - `scripts/workflow-test.sh`
   - `scripts/workflow-pack.sh --all`
4. Publish known-good artifact set and post operator notice:
   - Explain that `google-search` is temporarily disabled.
   - Provide ETA/workaround and support contact path.

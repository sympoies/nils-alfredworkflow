# bangumi-search Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id bangumi-search --install`
2. Confirm Alfred workflow variables are valid:
   - `BANGUMI_MAX_RESULTS` (optional, default `10`)
   - `BANGUMI_TIMEOUT_MS` (optional, default `8000`)
   - `BANGUMI_API_FALLBACK` (`auto`, `never`, `always`; default `auto`)
3. Confirm script-filter contract output is JSON:
   - `bash workflows/bangumi-search/scripts/script_filter.sh "anime naruto" | jq -e '.items | type == "array"'`
4. Confirm deterministic checks pass:
   - `node --test workflows/bangumi-search/scripts/tests/bangumi_scraper_contract.test.mjs`
   - `bash workflows/bangumi-search/tests/smoke.sh`

## Common failures and actions

| Symptom in Alfred                 | Likely cause                                                                       | Action                                                      |
| --------------------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `Invalid Bangumi workflow config` | `BANGUMI_MAX_RESULTS`, `BANGUMI_TIMEOUT_MS`, or `BANGUMI_API_FALLBACK` is invalid. | Correct variable values and retry.                          |
| `Bangumi API rate-limited`        | Upstream API returned `429` or equivalent throttle signal.                         | Retry later and reduce `BANGUMI_MAX_RESULTS` if needed.     |
| `Bangumi API unavailable`         | DNS/TLS/network timeout or upstream `5xx`.                                         | Check local network and retry; if sustained, pause rollout. |
| `Bangumi API key is missing`      | Runtime path requires key and `BANGUMI_API_KEY` is empty.                          | Set `BANGUMI_API_KEY` in workflow config and retry.         |
| `bangumi-cli binary not found`    | Packaged binary missing, build path mismatch, or bad `BANGUMI_CLI_BIN`.            | Re-pack workflow or set valid `BANGUMI_CLI_BIN`.            |
| `Keep typing (2+ chars)`          | Query is shorter than minimum length (`<2`).                                       | Continue typing until at least 2 characters.                |

Notes:

- Current production runtime is API-first.
- `scripts/script_filter.sh` does not call `bangumi_scraper.mjs`.
- Playwright bridge scaffold is disabled by default until rollout gates are completed.

## Validation

Run these checks after any runtime/config change:

- `node --check workflows/bangumi-search/scripts/bangumi_scraper.mjs`
- `node --test workflows/bangumi-search/scripts/tests/bangumi_scraper_contract.test.mjs`
- `bash workflows/bangumi-search/tests/smoke.sh`
- `scripts/workflow-test.sh --id bangumi-search`
- `scripts/workflow-pack.sh --id bangumi-search`

## Rollback guidance

Use this when API regressions or operator load rises above acceptable threshold.

1. Stop rollout of new `bangumi-search` artifacts.
2. Revert Bangumi workflow changeset(s), including:
   - `workflows/bangumi-search/`
   - `crates/bangumi-cli/`
   - docs updates in `workflows/bangumi-search/README.md`, `workflows/bangumi-search/TROUBLESHOOTING.md`, and
     `ALFRED_WORKFLOW_DEVELOPMENT.md` (if changed)
3. Rebuild and validate rollback state:
   - `scripts/workflow-lint.sh`
   - `scripts/workflow-test.sh`
   - `scripts/workflow-pack.sh --all`
4. Publish known-good artifact set and note that scraper bridge remains disabled by default.

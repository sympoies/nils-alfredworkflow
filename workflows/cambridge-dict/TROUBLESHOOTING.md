# cambridge-dict Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

1. Confirm latest package was used:
   - `scripts/workflow-pack.sh --id cambridge-dict --install`
2. Confirm Alfred workflow variables are valid:
   - `CAMBRIDGE_DICT_MODE` (`english` or `english-chinese-traditional`)
   - `CAMBRIDGE_MAX_RESULTS` (optional, default `8`)
   - `CAMBRIDGE_TIMEOUT_MS` (optional, default `8000`)
   - `CAMBRIDGE_HEADLESS` (optional, default `true`)
3. Confirm installed workflow runtime is available:
   - `scripts/setup-cambridge-workflow-runtime.sh --check-only --skip-browser`
4. Confirm deterministic workflow/Node checks pass:
   - `npm run test:cambridge-scraper`
   - `bash workflows/cambridge-dict/tests/smoke.sh`

## Common failures and actions

| Symptom in Alfred                     | Likely cause                                                                                                      | Action                                                                              |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `cambridge-cli binary not found`      | Packaged binary missing or runtime path mismatch.                                                                 | Re-pack workflow, or set `CAMBRIDGE_CLI_BIN` to executable path.                    |
| `Node/Playwright runtime unavailable` | `node` missing, workflow-local `playwright` package missing, or Chromium browser not installed for live scraping. | Run `scripts/setup-cambridge-workflow-runtime.sh` and retry.                        |
| `Cambridge anti-bot challenge`        | Cambridge returned Cloudflare/anti-bot page.                                                                      | Retry later, reduce query frequency, or open Cambridge page directly in browser.    |
| `Cambridge cookie consent required`   | Cookie wall rendered instead of dictionary content.                                                               | Open Cambridge Dictionary in browser once, accept cookies, then retry Alfred query. |
| `Cambridge request timed out`         | Timeout too low for current network/page latency.                                                                 | Increase `CAMBRIDGE_TIMEOUT_MS` and retry.                                          |
| `Invalid Cambridge workflow config`   | Invalid mode/max-results/timeout/headless values.                                                                 | Correct `CAMBRIDGE_*` variables in Alfred config.                                   |

## Validation

- Default smoke/test commands are fixture/stub based and do not require live Cambridge network calls.
- If you need to validate live scraping behavior, run manual checks separately and treat failures as external-site noise
  unless reproduced with fixtures.

Run these checks:

- `scripts/setup-cambridge-workflow-runtime.sh --check-only --skip-browser`
- `npm run test:cambridge-scraper`
- `bash workflows/cambridge-dict/tests/smoke.sh`

## Rollback guidance

Use this when anti-bot/cookie/network volatility makes the workflow unstable.

1. Stop rollout of new `cambridge-dict` artifacts.
2. Revert Cambridge workflow changeset(s), including:
   - `workflows/cambridge-dict/`
   - `crates/cambridge-cli/docs/workflow-contract.md`
   - docs updates in `README.md`, `TROUBLESHOOTING.md`, and `ALFRED_WORKFLOW_DEVELOPMENT.md` (if changed)
3. Rebuild and validate rollback state:
   - `scripts/workflow-lint.sh`
   - `scripts/workflow-test.sh`
   - `scripts/workflow-pack.sh --all`

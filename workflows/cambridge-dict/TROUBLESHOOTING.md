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
3. Confirm Alfred can see `node` and `npm`:
   - `command -v node`
   - `command -v npm`
4. Confirm installed workflow runtime is available:
   - `scripts/setup-cambridge-workflow-runtime.sh --check-only --skip-browser`
5. Confirm deterministic workflow/Node checks pass:
   - `npm run test:cambridge-scraper`
   - `bash workflows/cambridge-dict/tests/smoke.sh`

## Common failures and actions

- `cambridge-cli binary not found`: packaged binary missing or runtime path mismatch.
  Action: re-pack workflow, or set `CAMBRIDGE_CLI_BIN` to executable path.
- `Installing Cambridge runtime...`: workflow-local Playwright/Chromium runtime is being bootstrapped automatically
  after first-use runtime detection.
  Action: wait for Alfred auto-rerun; if it does not finish, inspect the workflow cache log.
- `Automatic Cambridge runtime setup failed`: auto-bootstrap ran but `npm install` or
  `playwright install chromium` failed.
  Action: check the bootstrap log in Alfred cache, fix Node/npm/network access, then retry.
- `Node/Playwright runtime unavailable`: Alfred cannot locate `node`/`npm`, or auto-bootstrap is disabled or
  unavailable.
  Action: install Node.js, ensure Alfred PATH can resolve it, or run
  `scripts/setup-cambridge-workflow-runtime.sh`.
- `Cambridge anti-bot challenge`: Cambridge returned Cloudflare or an anti-bot page.
  Action: retry later, reduce query frequency, or open Cambridge page directly in browser.
- `Cambridge cookie consent required`: cookie wall rendered instead of dictionary content.
  Action: open Cambridge Dictionary in browser once, accept cookies, then retry Alfred query.
- `Cambridge request timed out`: timeout too low for current network/page latency.
  Action: increase `CAMBRIDGE_TIMEOUT_MS` and retry.
- `Invalid Cambridge workflow config`: invalid mode/max-results/timeout/headless values.
  Action: correct `CAMBRIDGE_*` variables in Alfred config.

## Validation

- Default smoke/test commands are fixture/stub based and do not require live Cambridge network calls.
- If you need to validate live scraping behavior, run manual checks separately and treat failures as external-site noise
  unless reproduced with fixtures.

Run these checks:

- `scripts/setup-cambridge-workflow-runtime.sh --check-only --skip-browser`
- `npm run test:cambridge-scraper`
- `bash workflows/cambridge-dict/tests/smoke.sh`

Bootstrap logs are stored under:

- `$ALFRED_WORKFLOW_CACHE/cambridge-runtime/bootstrap.log`

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

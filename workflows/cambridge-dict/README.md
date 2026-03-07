# Cambridge Dict - Alfred Workflow

Search Cambridge Dictionary from Alfred with smart exact-match lookup, suggestion fallback, and one-key requery between
detail and suggestion views.

## Screenshot

![Cambridge Dict workflow screenshot](./screenshot.png)

## Features

- Trigger smart dictionary lookup with `cd <query>`.
- Exact matches render detail rows directly; unmatched queries fall back to suggestions.
- `cds <query>` forces suggestion mode even when Cambridge has a direct entry.
- Suggestion rows use `Enter` to load detail rows; detail rows use `Cmd+Enter` to reopen suggestions.
- Detail stage renders definitions and example sentences from the selected entry.
- Press `Enter` on detail rows to open the entry URL from `arg`.
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips backend calls.
- Script-level guardrails: short TTL cache and optional coalescing controls are available, but Cambridge now defaults to
  immediate backend dispatch instead of the old extra settle wait.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; Cambridge-specific fetch/error
  mapping remains local.
- Uses `cambridge-cli` as the Alfred bridge and Playwright scraper backend.
- Missing workflow-local Playwright/Chromium runtime now triggers automatic bootstrap on first live lookup when
  `node`/`npm` are available in Alfred's environment.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable                | Required | Default   | Description                                                                      |
| ----------------------- | -------- | --------- | -------------------------------------------------------------------------------- |
| `CAMBRIDGE_DICT_MODE`   | No       | `english` | Dictionary mode. Allowed values: `english`, `english-chinese-traditional`.       |
| `CAMBRIDGE_MAX_RESULTS` | No       | `8`       | Max candidate rows in suggest stage. Effective range is clamped to `1..20`.      |
| `CAMBRIDGE_TIMEOUT_MS`  | No       | `8000`    | Playwright timeout in milliseconds. Effective range is clamped to `1000..30000`. |
| `CAMBRIDGE_HEADLESS`    | No       | `true`    | Playwright headless mode flag. Allowed values: `true`, `false`.                  |

## Keyword

- `cd <query>`: Smart lookup via `cambridge-cli query --input <query>`; exact matches open detail rows directly,
  otherwise Cambridge suggestions are shown.
- `cds <query>`: Force suggestion mode when you want Cambridge suggestion rows even for exact matches.

## Advanced Runtime Parameters

| Parameter                                 | Description                                                                                     |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `CAMBRIDGE_CLI_BIN`                       | Optional absolute executable path override for `cambridge-cli`.                                 |
| `CAMBRIDGE_SCRAPER_SCRIPT`                | Exported by `script_filter.sh` to point to bundled `scripts/cambridge_scraper.mjs`.             |
| `CAMBRIDGE_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits). |
| `CAMBRIDGE_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `0` for immediate dispatch.                  |
| `CAMBRIDGE_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`.               |
| `CAMBRIDGE_RUNTIME_BOOTSTRAP_HELPER`      | Optional override for the bundled runtime bootstrap helper; mainly for testing.                 |

## Runtime bootstrap

On a packaged workflow install, the first live lookup that detects missing Playwright/Chromium will bootstrap the
workflow-local runtime automatically inside the installed Alfred workflow directory.

Manual fallback remains available if Alfred cannot see `node`/`npm`:

- `scripts/setup-cambridge-workflow-runtime.sh`

## Deterministic tests (no live network by default)

- Node fixture tests: `npm run test:cambridge-scraper`
- Workflow smoke: `bash workflows/cambridge-dict/tests/smoke.sh`

Live scraping checks are intentionally not part of default smoke gates.

### Change-scoped checks

- If changes include `workflows/cambridge-dict/scripts/` or root `package.json`, run:
  - `npm run test:cambridge-scraper`
- If changes include any files under `workflows/cambridge-dict/`, run:
  - `bash workflows/cambridge-dict/tests/smoke.sh`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

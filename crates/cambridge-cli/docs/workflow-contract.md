# Cambridge Dict Contract

This document defines the functional/runtime contract for `workflows/cambridge-dict`.

## Keyword and Query Handling

- Alfred keyword: `cd`.
- Script filter adapter: `workflows/cambridge-dict/scripts/script_filter.sh`.
- Adapter invocation contract:
  - `cambridge-cli query --input "<query>"`
  - `CAMBRIDGE_SCRAPER_SCRIPT` must be exported to bundled `scripts/cambridge_scraper.mjs` path.
- Empty input must not crash and must return a valid Alfred JSON fallback item.

## Two-Stage Query Token Grammar

- Candidate stage input: arbitrary user query (`open`, `take off`, ...).
- Candidate rows from Rust side must expose Alfred `autocomplete` tokens using prefix grammar:
  - `def::WORD`
- Detail stage input: query string beginning with `def::`.
- `WORD` is the selected headword/entry token consumed by `cambridge-cli` for detail extraction.

## Alfred Item JSON Contract

- Script filter stdout must always be parseable JSON object with top-level `items` array.
- Candidate-stage row contract:
  - `title`: headword
  - `subtitle`: short candidate guidance
  - `autocomplete`: `def::WORD`
  - `arg`: canonical Cambridge URL for Enter action
  - `valid`: boolean
- Detail-stage row contract:
  - `title`: headword/definition/example line
  - `subtitle`: optional POS/phonetics/detail text
  - `arg`: canonical Cambridge URL
  - `valid`: boolean (rows can remain valid so Enter opens URL)
- Error/empty fallback rows:
  - single-item `items` array
  - `valid: false`
  - actionable `title` + `subtitle`

## Error Mapping

Script filter fallback titles should map runtime failures into stable operator-facing categories:

- Empty query -> `Enter a word`
- Invalid env config -> `Invalid Cambridge workflow config`
- Missing binary -> `cambridge-cli binary not found`
- Node/Playwright dependency issues -> `Node/Playwright runtime unavailable`
- Anti-bot challenge -> `Cambridge anti-bot challenge`
- Cookie wall -> `Cambridge cookie consent required`
- Timeout -> `Cambridge request timed out`
- Network/upstream failures -> `Cambridge service unavailable`

Node scraper structured error object contract (`ok: false`):

```json
{
  "ok": false,
  "stage": "suggest|define|unknown",
  "mode": "english|english-chinese-traditional",
  "error": {
    "code": "anti_bot|cookie_wall|timeout|network|parse_error|invalid_args|unknown",
    "message": "machine-readable summary",
    "hint": "operator guidance",
    "retriable": true
  }
}
```

## Environment Variables and Constraints

- `CAMBRIDGE_DICT_MODE`
  - allowed: `english`, `english-chinese-traditional`
  - default: `english`
- `CAMBRIDGE_MAX_RESULTS`
  - parsed integer, clamped to `1..20`
  - default: `8`
- `CAMBRIDGE_TIMEOUT_MS`
  - parsed integer milliseconds, clamped to `1000..30000`
  - default: `8000`
- `CAMBRIDGE_HEADLESS`
  - allowed: `true`, `false`
  - default: `true`
- `CAMBRIDGE_CLI_BIN`
  - optional executable path override for script-filter runtime
- `CAMBRIDGE_SCRAPER_SCRIPT`
  - exported by script-filter to bundled Node scraper path

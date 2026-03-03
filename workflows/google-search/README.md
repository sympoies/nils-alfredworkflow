# Google Search - Alfred Workflow

Search Google via Alfred with a two-stage suggestion flow (`gg`) plus a direct Brave-search mode (`gb`), then open
selected links in your browser.

## Screenshot

![Google Search workflow screenshot](./screenshot.png)

## Features

- Trigger two-stage search suggestions with `gg <query>`.
- Suggest stage returns candidate queries and sets Alfred `autocomplete` tokens like `res::rust book`.
- Search stage renders Brave web results after selecting a suggestion token.
- Use `gb <query>` for direct Brave web search.
- Open selected URL in your default browser with `Enter`.
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips API calls.
- Script Filter queue policy: 1 second delay with initial immediate run disabled.
- Script-level guardrails: async query coalescing (final query priority) and short TTL cache reduce duplicate API calls
  while typing.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; Google-specific fetch/error mapping
  remains local.
- Map common failures (missing API key, rate limiting, API unavailable, invalid config) to actionable Alfred messages.
- Tune result count, safe search mode, and country bias through workflow variables.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable            | Required | Default | Description                                                                        |
| ------------------- | -------- | ------- | ---------------------------------------------------------------------------------- |
| `BRAVE_API_KEY`     | Yes      | (empty) | Brave Search API subscription token.                                               |
| `BRAVE_MAX_RESULTS` | No       | `10`    | Max results per query. Effective range is clamped to `1..20`.                      |
| `BRAVE_SAFESEARCH`  | No       | `off`   | Safe search mode: `strict`, `moderate`, or `off`.                                  |
| `BRAVE_COUNTRY`     | No       | (empty) | Optional uppercase ISO 3166-1 alpha-2 country code (for example `US`, `TW`, `JP`). |

## Keyword

| Keyword      | Behavior                                                                                                        |
| ------------ | --------------------------------------------------------------------------------------------------------------- |
| `gg <query>` | Two-stage flow: first fetch Google suggestions, then load Brave web results through `res::` autocomplete token. |
| `gb <query>` | Direct Brave mode: call `brave-cli search` immediately and open selected URL.                                   |

## Advanced Runtime Parameters

| Parameter                             | Description                                                                                     |
| ------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `BRAVE_CLI_BIN`                       | Optional override path for `brave-cli` (useful for local debugging).                            |
| `BRAVE_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits). |
| `BRAVE_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `2`.                                         |
| `BRAVE_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for async result. Default `0.4`.                   |

## Validation

- `bash workflows/google-search/tests/smoke.sh`
- `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows google-search`
- `scripts/workflow-test.sh --id google-search`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

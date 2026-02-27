# Bilibili Search - Alfred Workflow

Search bilibili suggestions from Alfred and open selected search pages in your browser.

## Screenshot

![Bilibili Search workflow screenshot](./screenshot.png)

## Features

- Trigger bilibili search with `bl <query>`.
- Show bilibili suggestion terms directly in Alfred results.
- Open selected bilibili search URL in your default browser with `Enter`.
- Optional personalization via `BILIBILI_UID` (`userid` query param).
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips API calls.
- Script Filter queue policy: 1 second delay with initial immediate run disabled.
- Script-level guardrails: async query coalescing (final query priority) and short TTL cache reduce duplicate API calls
  while typing.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; bilibili-specific fetch/error
  mapping remains local.
- Map common failures (invalid config, API unavailable) to actionable Alfred messages.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable               | Required | Default | Description                                                                   |
| ---------------------- | -------- | ------- | ----------------------------------------------------------------------------- |
| `BILIBILI_UID`         | No       | (empty) | Optional bilibili UID used as suggest endpoint `userid` parameter.            |
| `BILIBILI_MAX_RESULTS` | No       | `10`    | Max suggestions per query. Effective range is clamped to `1..20`.             |
| `BILIBILI_TIMEOUT_MS`  | No       | `8000`  | Request timeout in milliseconds. Effective range is clamped to `1000..30000`. |
| `BILIBILI_USER_AGENT`  | No       | (empty) | Optional explicit User-Agent override for API calls.                          |

## Keyword

| Keyword      | Behavior                                                             |
| ------------ | -------------------------------------------------------------------- |
| `bl <query>` | Search and list bilibili suggestions, then open selected search URL. |

## Advanced Runtime Parameters

| Parameter                                | Description                                                                                     |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `BILIBILI_CLI_BIN`                       | Optional override path for `bilibili-cli` (useful for local debugging).                         |
| `BILIBILI_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits). |
| `BILIBILI_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `2`.                                         |
| `BILIBILI_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`.               |

## Deterministic checks

- Workflow smoke test: `bash workflows/bilibili-search/tests/smoke.sh`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

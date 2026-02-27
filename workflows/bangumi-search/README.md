# Bangumi Search - Alfred Workflow

Search Bangumi subjects from Alfred via `bangumi-cli` (API-first path), then open selected subject pages in your
browser.

## Screenshot

![Bangumi Search workflow screenshot](./screenshot.png)

## Features

- Trigger Bangumi search with `bgm <query>` (`all` mode).
- Fixed type shortcuts: `bgmb`, `bgma`, `bgmm`, `bgmg`, `bgmr`.
- Built-in cache maintenance command: `bgm clear cache`.
- Built-in cache-dir maintenance command: `bgm clear cache dir`.
- `bgm` empty query menu uses deterministic order; type-category quick rows are pinned at the bottom.
- Support typed prefixes in one input grammar: `[type] query`.
- Type mapping: `all`, `book`, `anime`, `music`, `game`, `real`.
- Script-level guardrails: queue-safe async query coalescing and TTL cache for repeated typing.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; Bangumi API fetch/error mapping
  remains local.
- API-first production path: `script_filter.sh` calls `bangumi-cli query` only.
- Playwright scraper scaffold exists for future handoff and is disabled by default.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable                          | Required | Default | Description                                                               |
| --------------------------------- | -------- | ------- | ------------------------------------------------------------------------- |
| `BANGUMI_API_KEY`                 | No       | ``      | Optional API key. Keep empty for anonymous API mode.                      |
| `BANGUMI_MAX_RESULTS`             | No       | `10`    | Max results returned. Effective range is clamped to `1..20`.              |
| `BANGUMI_TIMEOUT_MS`              | No       | `8000`  | API timeout in milliseconds. Effective range is clamped to `1000..30000`. |
| `BANGUMI_USER_AGENT`              | No       | ``      | Optional User-Agent override. Empty means use `bangumi-cli` default UA.   |
| `BANGUMI_CACHE_DIR`               | No       | ``      | Optional cache directory override for image cache files.                  |
| `BANGUMI_IMAGE_CACHE_TTL_SECONDS` | No       | `86400` | Image cache TTL in seconds.                                               |
| `BANGUMI_IMAGE_CACHE_MAX_MB`      | No       | `128`   | Image cache size cap in MB.                                               |
| `BANGUMI_API_FALLBACK`            | No       | `auto`  | Compatibility fallback policy: `auto`, `never`, `always`.                 |

## Keyword

| Keyword               | Behavior                                                      |
| --------------------- | ------------------------------------------------------------- |
| `bgm <query>`         | Search in default `all` mode and list Bangumi subjects.       |
| `bgmb <query>`        | Search Bangumi `book` subjects only.                          |
| `bgma <query>`        | Search Bangumi `anime` subjects only.                         |
| `bgmm <query>`        | Search Bangumi `music` subjects only.                         |
| `bgmg <query>`        | Search Bangumi `game` subjects only.                          |
| `bgmr <query>`        | Search Bangumi `real` subjects only.                          |
| `bgm all <query>`     | Explicit `all` mode search.                                   |
| `bgm book <query>`    | Search Bangumi `book` subjects only.                          |
| `bgm anime <query>`   | Search Bangumi `anime` subjects only.                         |
| `bgm music <query>`   | Search Bangumi `music` subjects only.                         |
| `bgm game <query>`    | Search Bangumi `game` subjects only.                          |
| `bgm real <query>`    | Search Bangumi `real` subjects only.                          |
| `bgm clear cache`     | Clear local `bangumi-search` Script Filter query cache files. |
| `bgm clear cache dir` | Clear files under `BANGUMI_CACHE_DIR` (if configured).        |

## URL behavior and fallback

- Preferred subject URL comes from API response `url` when available.
- If API `url` is absent, workflow falls back to canonical `https://bgm.tv/subject/<id>`.
- If no actionable subject item is available, use direct Bangumi search page fallback:
  - `https://bgm.tv/subject_search?cat=all&search_text=<encoded-query>`

## Advanced Runtime Parameters

| Parameter                               | Description                                                                                                                                                                |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BANGUMI_CLI_BIN`                       | Optional absolute executable override for `bangumi-cli`.                                                                                                                   |
| `BANGUMI_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits).                                                                            |
| `BANGUMI_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `2`. Shared coalesce helper uses non-blocking stability checks so queued typing does not dispatch prefix queries early. |
| `BANGUMI_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`.                                                                                          |
| `BANGUMI_SCRAPER_ENABLE`                | Future bridge feature flag. Default disabled; do not enable in production yet.                                                                                             |

## Deterministic checks

- Node scaffold contract test: `node --test workflows/bangumi-search/scripts/tests/bangumi_scraper_contract.test.mjs`
- Workflow smoke test: `bash workflows/bangumi-search/tests/smoke.sh`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

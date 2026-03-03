# Steam Search - Alfred Workflow

Search Steam Store games from Alfred and open selected app pages in your browser.

## Screenshot

![Steam Search workflow screenshot](./screenshot.png)

## Features

- Trigger Steam search with `st <query>` (alias: `steam`).
- Optional region-switch rows (disabled by default) using the `steam-requery:<region>:<query>` action arg contract.
- Press `Enter` on a region row to requery the same keywords in the selected region.
- Open selected Steam app URLs in your default browser.
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips API calls.
- Script Filter queue policy: 1 second delay with initial immediate run disabled.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; Steam-specific fetch/error mapping stays local.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `STEAM_REGION` | No | `US` | Optional two-letter region code used for Steam Store `cc` parameter. |
| `STEAM_REGION_OPTIONS` | No | `US,JP` | Optional comma/newline list of switch-row regions. Order is preserved exactly. |
| `STEAM_SHOW_REGION_OPTIONS` | No | `0` | Show region switch rows (`1/true/yes/on` to enable). |
| `STEAM_LANGUAGE` | No | `(empty)` | Steam search locale language (for example `english`, `tchinese`); leave empty for endpoint default locale behavior. |
| `STEAM_MAX_RESULTS` | No | `10` | Max results per query. Effective range is clamped by `steam-cli`. |
| `STEAM_SEARCH_API` | No | `search-suggestions` | Search backend selector: `search-suggestions` (default) or `storesearch` (alternate JSON endpoint). |

## Keyword

| Keyword | Behavior |
| --- | --- |
| `st <query>` / `steam <query>` | Search Steam Store games and open selected app URLs. |

## Advanced Runtime Parameters

| Parameter | Description |
| --- | --- |
| `STEAM_CLI_BIN` | Optional override path for `steam-cli` (useful for local debugging). |
| `STEAM_REQUERY_COMMAND` | Optional override command used by `action_open.sh` to trigger Alfred requery (test/debug helper). |
| `STEAM_QUERY_CACHE_TTL_SECONDS` | Optional same-query cache TTL (seconds). Default `0` (disabled). |
| `STEAM_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `0` for immediate responses. |
| `STEAM_QUERY_COALESCE_RERUN_SECONDS` | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`. |

## Validation

- `shellcheck workflows/steam-search/scripts/script_filter.sh workflows/steam-search/scripts/action_open.sh`
- `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows steam-search`
- `bash workflows/steam-search/tests/smoke.sh`
- `scripts/workflow-test.sh --id steam-search`
- `scripts/workflow-pack.sh --id steam-search`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

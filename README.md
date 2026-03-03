# nils-alfredworkflow

Alfred workflows for macOS users.

![Quote Feed workflow screenshot](workflows/quote-feed/screenshot.png)

## Install

1. Download a `.alfredworkflow` package from the [Releases](https://github.com/sympoies/nils-alfredworkflow/releases) page.
2. Double-click the package to import it into Alfred.
3. For API-based workflows, open Alfred's `Configure Workflow...` and fill in required credentials.

## Workflows

| Workflow | Keyword(s) | What it does | Requires setup |
| --- | --- | --- | --- |
| [Google Search](workflows/google-search/README.md) | `gg`, `google` | Search web results (Brave backend) and open selected links. | `BRAVE_API_KEY` |
| [Google Service](workflows/google-service/README.md) | `gs`, `gsa`, `gsd`, `gsm` | `gs` shows account status (optional all-account unread summary); `gsa` manages auth login/remove/switch; `gsd` supports Drive home/search/download; `gsm` supports Gmail unread/latest/search list. | No |
| [YouTube Search](workflows/youtube-search/README.md) | `yt`, `youtube` | Search YouTube videos and open selected videos in browser. | `YOUTUBE_API_KEY` |
| [Netflix Search](workflows/netflix-search/README.md) | `nf`, `netflix` | Search Netflix title pages (`site:netflix.com/title`) and open selected links. | `BRAVE_API_KEY` |
| [Spotify Search](workflows/spotify-search/README.md) | `sp`, `spotify` | Search Spotify tracks and open selected results in Spotify app. | `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET` |
| [Wiki Search](workflows/wiki-search/README.md) | `wk`, `wiki` | Search Wikipedia articles and open selected page links. | No |
| [Steam Search](workflows/steam-search/README.md) | `st`, `steam` | Search Steam Store games, switch region rows, and open selected app pages. | Optional tuning: `STEAM_REGION`, `STEAM_REGION_OPTIONS`, `STEAM_SHOW_REGION_OPTIONS`, `STEAM_LANGUAGE`, `STEAM_MAX_RESULTS`, `STEAM_SEARCH_API` |
| [IMDb Search](workflows/imdb-search/README.md) | `im`, `imdb` | Search IMDb and open result pages in browser. | Optional: `IMDB_SEARCH_SECTION` |
| [Bilibili Search](workflows/bilibili-search/README.md) | `bl`, `bilibili` | Search bilibili suggestions and open selected search links in browser. | Optional: `BILIBILI_UID` |
| [Bangumi Search](workflows/bangumi-search/README.md) | `bgm`, `bangumi` | Search Bangumi subjects and open selected subject pages in browser. | Optional: `BANGUMI_API_KEY` |
| [Weather Forecast](workflows/weather/README.md) | `wt`, `ww`, `weather` | Show today rows then hourly (`wt`) / city picker then 7-day (`ww`) forecasts, then copy selected rows. | Optional tuning: `WEATHER_CLI_BIN` |
| [Cambridge Dict](workflows/cambridge-dict/README.md) | `cd`, `cambridge` | Two-stage Cambridge dictionary lookup (candidate -> detail) with Enter-to-open entry URL. | Node + Playwright runtime for scraper backend |
| [Market Expression](workflows/market-expression/README.md) | `mx`, `market` | Evaluate market expressions (numeric: `+ - * /`, assets: `+ -`) with FX/crypto conversion and copy selected rows. | Optional tuning: `MARKET_CLI_BIN`, `MARKET_DEFAULT_FIAT` |
| [Quote Feed](workflows/quote-feed/README.md) | `qq`, `quote` | Show cached quotes, refresh in background, and copy a selected quote. | Optional tuning: `QUOTE_DISPLAY_COUNT`, `QUOTE_REFRESH_INTERVAL`, `QUOTE_FETCH_COUNT`, `QUOTE_MAX_ENTRIES`, `QUOTE_DATA_DIR` |
| [Memo Add](workflows/memo-add/README.md) | `mm`, `memo` | Add/search memo text quickly into sqlite storage, with optional one-click db init and latest-record preview. | Optional tuning: `MEMO_DB_PATH`, `MEMO_SOURCE`, `MEMO_REQUIRE_CONFIRM`, `MEMO_MAX_INPUT_BYTES`, `MEMO_RECENT_LIMIT`, `MEMO_WORKFLOW_CLI_BIN` |
| [Open Project](workflows/open-project/README.md) | `c`, `code`, `github` | Fuzzy-find local Git projects, open in editor, and jump to GitHub remotes. | Optional tuning: `OPEN_PROJECT_MAX_RESULTS` |
| [Epoch Converter](workflows/epoch-converter/README.md) | `ts`, `epoch` | Convert epoch/datetime values and copy selected output. | No |
| [Multi Timezone](workflows/multi-timezone/README.md) | `tz`, `timezone` | Show current time across one or more IANA timezones and copy selected output. | Optional tuning: `TIMEZONE_CLI_BIN`, `MULTI_TZ_ZONES`, `MULTI_TZ_LOCAL_OVERRIDE` |
| [Randomer](workflows/randomer/README.md) | `rr`, `rrv`, `random` | Generate random values by format and copy results. | No |
| [Codex CLI](workflows/codex-cli/README.md) | `cx`, `codex` | Run Codex auth (`login`, `use`, `save`) and diagnostics (`diag rate-limits`) commands from Alfred. | No |

## macOS Gatekeeper standalone script

- Script asset: `workflow-clear-quarantine-standalone.sh` from [Releases](https://github.com/sympoies/nils-alfredworkflow/releases)
- Bulk fix (safe when some workflows are not installed):
  `chmod +x ./workflow-clear-quarantine-standalone.sh && ./workflow-clear-quarantine-standalone.sh --all`
- Single workflow fix:
  `./workflow-clear-quarantine-standalone.sh --id <workflow-id>`
- Repository checkout helper (for maintainers):
  `scripts/workflow-clear-quarantine.sh --id <workflow-id>`

## Canonical documentation map

- Docs ownership and retention decisions: [docs/reports/docs-ownership-matrix.md](docs/reports/docs-ownership-matrix.md)
- Architecture/runtime boundaries: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- CLI runtime/output contract mapping: [docs/specs/cli-standards-mapping.md](docs/specs/cli-standards-mapping.md)
- Release/tagging flow and gates: [docs/RELEASE.md](docs/RELEASE.md)
- Workflow-specific runtime/query/validation details: `workflows/<workflow-id>/README.md`

## Troubleshooting

- Global standards and shared operator playbooks: [ALFRED_WORKFLOW_DEVELOPMENT.md](ALFRED_WORKFLOW_DEVELOPMENT.md)
- Workflow-specific runtime failures: `workflows/<workflow-id>/TROUBLESHOOTING.md`
- List all workflow-local troubleshooting docs quickly:
  `rg --files workflows | rg 'TROUBLESHOOTING\.md$'`

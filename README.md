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
| [Google Search](workflows/google-search/README.md) | `gg`, `google` | Search web results (Brave backend) and open selected links. | Required: `BRAVE_API_KEY`; Optional: `BRAVE_COUNTRY`, `BRAVE_SAFESEARCH` |
| [Google Service](workflows/google-service/README.md) | `gs`, `gsa`, `gsd`, `gsm` | `gs` shows account status (optional all-account unread summary + per-account unread rows for accounts with unread mail); `gsa` manages auth login/remove/switch; `gsd` supports Drive home/search/download; `gsm` supports Gmail unread/latest/search list, including `unread --account <email>` override. | Optional: `GOOGLE_CLI_CONFIG_DIR`, `GOOGLE_DRIVE_DOWNLOAD_DIR`, `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD` |
| [YouTube Search](workflows/youtube-search/README.md) | `yt`, `youtube` | Search YouTube videos and open selected videos in browser. | Required: `YOUTUBE_API_KEY`; Optional: `YOUTUBE_REGION_CODE`, `YOUTUBE_MAX_RESULTS` |
| [Netflix Search](workflows/netflix-search/README.md) | `nf`, `netflix` | Search Netflix title pages (`site:netflix.com/title`) and open selected links. | Required: `BRAVE_API_KEY`; Optional: `NETFLIX_CATALOG_REGION`, `BRAVE_COUNTRY` |
| [Spotify Search](workflows/spotify-search/README.md) | `sp`, `spotify` | Search Spotify tracks and open selected results in Spotify app. | Required: `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET`; Optional: `SPOTIFY_MARKET` |
| [Wiki Search](workflows/wiki-search/README.md) | `wk`, `wiki` | Search Wikipedia articles and open selected page links. | Optional: `WIKI_LANGUAGE`, `WIKI_LANGUAGE_OPTIONS`, `WIKI_MAX_RESULTS` |
| [Steam Search](workflows/steam-search/README.md) | `st`, `steam` | Search Steam Store games, switch region rows, and open selected app pages. | Optional: `STEAM_REGION`, `STEAM_SHOW_REGION_OPTIONS`, `STEAM_LANGUAGE` |
| [IMDb Search](workflows/imdb-search/README.md) | `im`, `imdb` | Search IMDb and open result pages in browser. | Optional: `IMDB_SEARCH_SECTION`, `IMDB_MAX_RESULTS` |
| [Bilibili Search](workflows/bilibili-search/README.md) | `bl`, `bilibili` | Search bilibili suggestions and open selected search links in browser. | Optional: `BILIBILI_UID`, `BILIBILI_MAX_RESULTS`, `BILIBILI_TIMEOUT_MS` |
| [Bangumi Search](workflows/bangumi-search/README.md) | `bgm`, `bangumi` | Search Bangumi subjects and open selected subject pages in browser. | Optional: `BANGUMI_API_KEY`, `BANGUMI_MAX_RESULTS`, `BANGUMI_API_FALLBACK` |
| [Weather Forecast](workflows/weather/README.md) | `wt`, `ww`, `weather` | Show today rows then hourly (`wt`) / city picker then 7-day (`ww`) forecasts, then copy selected rows. | Optional: `WEATHER_LOCALE`, `WEATHER_DEFAULT_CITIES`, `WEATHER_CACHE_TTL_SECS` |
| [Cambridge Dict](workflows/cambridge-dict/README.md) | `cd`, `cambridge` | Two-stage Cambridge dictionary lookup (candidate -> detail) with Enter-to-open entry URL. | Optional: `CAMBRIDGE_DICT_MODE`, `CAMBRIDGE_MAX_RESULTS`, `CAMBRIDGE_TIMEOUT_MS` |
| [Market Expression](workflows/market-expression/README.md) | `mx`, `market` | Show a prompt row on empty query, optionally append favorite quotes, or evaluate market expressions (numeric: `+ - * /`, assets: `+ -`) with FX/crypto conversion and copy selected rows. | Optional: `MARKET_DEFAULT_FIAT`, `MARKET_FX_CACHE_TTL`, `MARKET_CRYPTO_CACHE_TTL`, `MARKET_FAVORITES_ENABLED`, `MARKET_FAVORITE_LIST` |
| [Quote Feed](workflows/quote-feed/README.md) | `qq`, `quote` | Show cached quotes, refresh in background, and copy a selected quote. | Optional: `QUOTE_DISPLAY_COUNT`, `QUOTE_REFRESH_INTERVAL`, `QUOTE_FETCH_COUNT` |
| [Memo Add](workflows/memo-add/README.md) | `mm`, `memo` | Add/search memo text quickly into sqlite storage, with optional one-click db init and latest-record preview. | Optional: `MEMO_DB_PATH`, `MEMO_REQUIRE_CONFIRM`, `MEMO_SEARCH_MATCH` |
| [Open Project](workflows/open-project/README.md) | `c`, `code`, `github` | Fuzzy-find local Git projects, open in editor, and jump to GitHub remotes. | Optional: `PROJECT_DIRS`, `OPEN_PROJECT_MAX_RESULTS`, `VSCODE_PATH` |
| [Epoch Converter](workflows/epoch-converter/README.md) | `ts`, `epoch` | Convert epoch/datetime values and copy selected output. | None |
| [Multi Timezone](workflows/multi-timezone/README.md) | `tz`, `timezone` | Show current time across one or more IANA timezones and copy selected output. | Optional: `MULTI_TZ_ZONES`, `MULTI_TZ_LOCAL_OVERRIDE` |
| [Randomer](workflows/randomer/README.md) | `rr`, `rrv`, `random` | Generate random values by format and copy results. | None |
| [Codex CLI](workflows/codex-cli/README.md) | `cx`, `codex` | Run Codex auth (`login`, `use`, `save`) and diagnostics (`diag rate-limits`) commands from Alfred. | Optional: `CODEX_AUTH_FILE`, `CODEX_API_KEY`, `CODEX_SECRET_DIR` |

## macOS Gatekeeper standalone script

- Script asset: `workflow-clear-quarantine-standalone.sh` from [Releases](https://github.com/sympoies/nils-alfredworkflow/releases)
- Bulk fix (safe when some workflows are not installed):
  `chmod +x ./workflow-clear-quarantine-standalone.sh && ./workflow-clear-quarantine-standalone.sh --all`
- Single workflow fix:
  `./workflow-clear-quarantine-standalone.sh --id <workflow-id>`
- Repository checkout helper (for maintainers):
  `scripts/workflow-clear-quarantine.sh --id <workflow-id>`

## Troubleshooting

- Global standards and shared operator playbooks: [ALFRED_WORKFLOW_DEVELOPMENT.md](ALFRED_WORKFLOW_DEVELOPMENT.md)
- Workflow-specific runtime failures: `workflows/<workflow-id>/TROUBLESHOOTING.md`

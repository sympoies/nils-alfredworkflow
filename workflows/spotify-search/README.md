# Spotify Search - Alfred Workflow

Search Spotify tracks from Alfred and play selected results in Spotify app.

## Screenshot

![Spotify Search workflow screenshot](./screenshot.png)

## Features

- Trigger Spotify search with `sp <query>`.
- Includes an unassigned hotkey trigger that users can bind in Alfred.
- Show track title and artist summary directly in Alfred.
- Open selected result in Spotify app with `Enter`.
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips API calls.
- Script Filter queue policy: 1 second delay with initial immediate run disabled.
- Script-level guardrails: async query coalescing reduces transient prefix searches while typing.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; Spotify-specific fetch/error mapping
  remains local.
- Map common failures (missing credentials, rate limit, API unavailable, invalid config) to actionable Alfred messages.
- Tune result count and market targeting through workflow variables.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable                | Required | Default | Description                                                                       |
| ----------------------- | -------- | ------- | --------------------------------------------------------------------------------- |
| `SPOTIFY_CLIENT_ID`     | Yes      | (empty) | Spotify application client ID for Client Credentials flow.                        |
| `SPOTIFY_CLIENT_SECRET` | Yes      | (empty) | Spotify application client secret for Client Credentials flow.                    |
| `SPOTIFY_MAX_RESULTS`   | No       | `10`    | Max results per query. Effective range is clamped by CLI.                         |
| `SPOTIFY_MARKET`        | No       | (empty) | Optional uppercase ISO 3166-1 alpha-2 market code (for example `US`, `TW`, `JP`). |

## Keyword

| Keyword      | Behavior                                                                         |
| ------------ | -------------------------------------------------------------------------------- |
| `sp <query>` | Search and list Spotify track results, then open selected result in Spotify app. |

## Hotkey (Optional)

- A hotkey trigger is included but ships unassigned by default.
- Set it from Alfred Workflow canvas: click the Hotkey node, then record your preferred key combo.

## Advanced Runtime Parameters

| Parameter                               | Description                                                                                       |
| --------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `SPOTIFY_CLI_BIN`                       | Optional override path for `spotify-cli` (useful for local debugging).                            |
| `SPOTIFY_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits).   |
| `SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `1`.                                           |
| `SPOTIFY_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`.                 |

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

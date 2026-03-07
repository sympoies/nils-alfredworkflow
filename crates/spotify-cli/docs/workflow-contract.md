# Spotify Search Workflow Contract

## Purpose

This document defines the runtime behavior contract for the `spotify-search` Alfred workflow.
It is the source of truth for query handling, Alfred item JSON shape, truncation behavior,
error-to-feedback mapping, and environment variable constraints.

## Keyword and Query Handling

- Workflow keyword: `sp` (or the configured keyword in Alfred for this workflow object).
- Input query is read from Alfred script filter argument.
- Query normalization:
  - Trim leading/trailing whitespace.
  - Preserve internal spacing and Unicode characters as provided by user input.
- Empty query behavior:
  - Do not call Spotify Accounts API or Spotify Search API.
  - Return one non-actionable Alfred item with:
    - `title = "Enter a search query"`
    - `subtitle = "Type keywords after sp to search Spotify tracks."`
- Short query behavior:
  - Queries shorter than 2 characters after trim must not call Spotify Accounts API or Spotify Search API.
  - Return one non-actionable Alfred item with:
    - `title = "Keep typing (2+ chars)"`
    - `subtitle = "Type at least 2 characters before searching Spotify."`
- Search dispatch behavior:
  - Alfred Script Filter queue policy must use a 1 second delay with initial immediate run disabled.
  - Script-side coalescing must prefer the latest stable query before dispatching backend calls.
- Non-empty query behavior (2+ chars after trim and once coalesced):
  - Request access token via Spotify Client Credentials flow.
  - Call Spotify Search API with `type=track`, `q=<query>`, `limit=<effective max>`, and optional `market`.

## Alfred Item JSON Contract

Top-level output must always be valid Alfred JSON:

```json
{
  "items": []
}
```

Success item schema (track result):

```json
{
  "title": "Track title",
  "subtitle": "Truncated artist + album summary",
  "arg": "https://open.spotify.com/track/<trackId>"
}
```

Rules:

- `title` is required and sourced from Spotify track name.
- `subtitle` is required and sourced from normalized + truncated metadata summary.
- `arg` is required for result items and must be the canonical Spotify track URL.
- URL format must be exactly `https://open.spotify.com/track/<trackId>`.

## Action Handling Contract

- `action_open.sh` accepts Alfred item `arg` values as either Spotify web URLs or Spotify URIs.
- For Spotify web URLs (`https://open.spotify.com/...`), action script must:
  - Convert to canonical URI form (`spotify:<kind>:<id>`).
  - Prefer opening via Spotify desktop app (`open -a Spotify <uri>` on macOS).
  - Fallback to `open <uri>` when app-prefixed open is unavailable.
- For non-Spotify URLs, action script must pass through and open original URL unchanged.

Non-success informational/error items:

- Must still include `title` and `subtitle`.
- Must set `valid: false`.
- Must omit `arg` to prevent accidental open actions.

## Subtitle Truncation Rules

- Source text: joined artist names + album name.
- Summary format: `<artist1>, <artist2> | <album>`.
- Normalize to a single line:
  - Replace CR/LF/tab with spaces.
  - Collapse repeated spaces.
  - Trim leading/trailing spaces.
- If normalized subtitle length is `<= 120` characters: use as-is.
- If normalized subtitle length is `> 120` characters:
  - Keep first 117 characters.
  - Append `...`.
- If artist names are unavailable after normalization: use `Unknown artist`.
- If album name is unavailable after normalization: use `Unknown album`.

## Error Mapping

The workflow must never crash or emit non-JSON output for handled failures.

| Scenario | Detection signal | Alfred title | Alfred subtitle | Item behavior |
| --- | --- | --- | --- | --- |
| Empty query | Query is empty after trim | `Enter a search query` | `Type keywords after sp to search Spotify tracks.` | `valid: false` |
| Short query | Query length is `1` after trim | `Keep typing (2+ chars)` | `Type at least 2 characters before searching Spotify.` | `valid: false` |
| Missing credentials | `SPOTIFY_CLIENT_ID` or `SPOTIFY_CLIENT_SECRET` missing/empty | `Spotify credentials are missing` | `Set SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET in workflow configuration.` | `valid: false` |
| Invalid credentials | Spotify token endpoint returns `invalid_client` / unauthorized | `Spotify credentials are invalid` | `Verify SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET and retry.` | `valid: false` |
| Rate limited | API response `HTTP 429` or rate-limit signal in error payload | `Spotify API rate limited` | `Rate limit reached. Retry later or lower SPOTIFY_MAX_RESULTS.` | `valid: false` |
| API unavailable | DNS/TLS/timeout/network failure or upstream `5xx` | `Spotify API unavailable` | `Cannot reach Spotify API now. Check network and retry.` | `valid: false` |
| Empty results | API succeeds but returns zero track items | `No tracks found` | `Try a different search query` | `valid: false` |
| Invalid workflow config | Invalid `SPOTIFY_MAX_RESULTS` or `SPOTIFY_MARKET` | `Invalid Spotify workflow config` | `<underlying config error message>` | `valid: false` |

## Environment Variables and Constraints

### `SPOTIFY_CLIENT_ID` (required)

- Required for Spotify access token requests.
- Must be non-empty after trimming.
- If missing/empty, return mapped credentials-missing Alfred error item (no API call).
- Must not be logged to stdout/stderr in plaintext.

### `SPOTIFY_CLIENT_SECRET` (required)

- Required for Spotify access token requests.
- Must be non-empty after trimming.
- If missing/empty, return mapped credentials-missing Alfred error item (no API call).
- Must not be logged to stdout/stderr in plaintext.

### `SPOTIFY_MAX_RESULTS` (optional)

- Optional integer.
- Default: `10`.
- Parse mode: base-10 integer only.
- Guardrails:
  - Minimum effective value: `1`.
  - Maximum effective value: `50`.
  - Values outside range are clamped to `[1, 50]`.
  - Invalid values return an actionable config error item (`Invalid Spotify workflow config`).

### `SPOTIFY_MARKET` (optional)

- Optional market filter passed to Spotify Search API `market` parameter.
- Must be a 2-letter ISO 3166-1 alpha-2 country code when provided.
- Input is uppercased before request construction.
- Invalid values return an actionable config error item (`Invalid Spotify workflow config`).

### `SPOTIFY_QUERY_CACHE_TTL_SECONDS` (optional)

- Optional same-query cache TTL in seconds for the script filter orchestration layer.
- Default: `0` (disabled).
- `0` means repeated same-query responses are not cached between invocations.

### `SPOTIFY_QUERY_COALESCE_SETTLE_SECONDS` (optional)

- Optional query coalescing settle window in seconds for the script filter orchestration layer.
- Default: `1`.
- Backend dispatch should wait until the latest query remains unchanged for the configured settle window.

### `SPOTIFY_QUERY_COALESCE_RERUN_SECONDS` (optional)

- Optional Alfred rerun interval while coalescing is waiting for the final query.
- Default: `0.4`.
- During this window, the workflow may emit a non-actionable pending item titled `Searching Spotify...`.

## Compatibility Notes

- Contract targets Alfred 5 script filter JSON shape.
- This contract covers `spotify-search` MVP search-only behavior (track search + open in Spotify app).
- Playback control and user-login scopes are out of scope for this contract.

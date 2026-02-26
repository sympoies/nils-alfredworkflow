# Steam Search Workflow Contract

## Purpose

This document defines the `nils-steam-cli` runtime contract for `steam-search`: query handling,
region/language runtime config, Steam Store API usage, Alfred JSON mapping, region-switch requery
args, and deterministic error behavior.

## Keyword and Query Handling

- Command: `steam-cli search --query <QUERY>`.
- Query normalization:
  - Trim leading/trailing whitespace.
  - Preserve internal spacing and Unicode content.
- Empty query behavior:
  - Do not call Steam Store API.
  - Return user error `query must not be empty` (stderr in Alfred mode).

## Runtime Config Contract

- `STEAM_REGION`:
  - Optional, default `us`.
  - Normalized to lowercase.
  - Must be exactly two ASCII letters (`^[a-z]{2}$`).
- `STEAM_REGION_OPTIONS`:
  - Optional comma/newline list of regions for switch rows.
  - Default `[STEAM_REGION]`.
  - Tokens normalized to lowercase, deduplicated by first appearance, and order preserved.
- `STEAM_MAX_RESULTS`:
  - Optional integer, default `10`.
  - Effective value clamped to `1..50`.
  - Non-integer values are config errors.
- `STEAM_LANGUAGE`:
  - Optional, default `english`.
  - Normalized to lowercase.
  - Allowed pattern: lowercase letters and `-`, length `2..24`.

Invalid config produces user error text and exit code `2`.

## Steam Store API Contract

- Endpoint: `https://store.steampowered.com/api/storesearch`
  - Test override: `STEAM_STORE_SEARCH_ENDPOINT`.
- Query parameters must always include:
  - `term=<query>`
  - `cc=<steam_region>`
  - `l=<steam_language>`
- Additional parameters:
  - `json=1`
  - `max_results=<effective max>`
- Non-2xx responses surface status + message (when present) as runtime errors.
- Malformed success payloads return typed runtime parse errors.
- Empty and partial item arrays are handled deterministically; invalid items are skipped.

## Alfred Item JSON Contract

Top-level output is always valid Alfred JSON:

```json
{
  "items": []
}
```

Current-region row (always first):

```json
{
  "title": "Current region: US",
  "subtitle": "Searching Steam Store in US (english).",
  "valid": false
}
```

Region-switch row:

```json
{
  "title": "Search in JP region",
  "subtitle": "Press Enter to requery \"dota 2\" in JP.",
  "arg": "steam-requery:jp:dota 2",
  "valid": true
}
```

Result row:

```json
{
  "title": "Counter-Strike 2",
  "subtitle": "Free | Platforms: Windows, Linux",
  "arg": "https://store.steampowered.com/app/730/?cc=us&l=english"
}
```

Rules:

- Region-switch rows follow `STEAM_REGION_OPTIONS` order exactly.
- Result URLs must include both region (`cc`) and language (`l`) query parameters.
- Subtitles are single-line, whitespace-normalized, and deterministically truncated to `<= 120`
  chars.

## Error Mapping

- User/config/input errors:
  - Exit code `2`.
  - Alfred mode: `stderr` line prefixed with `error:`.
  - Service JSON mode: `{"schema_version":"v1","command":"search","ok":false,...}`.
- Runtime/API errors:
  - Exit code `1`.
  - Same output-channel contract as above by mode.

## Output Modes

- `--mode alfred` (default): outputs Alfred Script Filter JSON directly.
- `--mode service-json`: wraps success/error into `schema_version=v1` service envelope.

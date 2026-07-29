# Bangumi Search Workflow Contract

> Status: active

This document defines the runtime contract for `workflows/bangumi-search` and `nils-bangumi-cli`.
Cross-references:

- Shared runtime + envelope: [`docs/specs/cli-shared-runtime-contract.md`](../../../docs/specs/cli-shared-runtime-contract.md)
- JSON envelope shape: [`docs/specs/cli-json-envelope-v1.md`](../../../docs/specs/cli-json-envelope-v1.md)
- Reserved error-code prefix (future allocation): [`docs/specs/cli-error-code-registry.md`](../../../docs/specs/cli-error-code-registry.md)
- Companion design doc for the future scraper bridge:
  [`playwright-bridge-design.md`](playwright-bridge-design.md).

## Input Grammar

- Workflow keyword: `bgm`.
- Input grammar: `[type] query`.
- If first token is a supported type, the remaining text is treated as query.
- If first token is not a supported type, workflow defaults to `all` and uses full input as query.
- Empty query (after trim) must not call API and must return a non-actionable Alfred item.

## Type Mapping

Supported type tokens:

- `all`
- `book`
- `anime`
- `music`
- `game`
- `real`

Mapping rules:

- Token matching is case-insensitive after trim.
- `all` is default when no explicit type token is provided.
- Unknown type token is treated as part of query (default `all`) unless explicitly parsed in strict CLI mode.

## Alfred Item Mapping

Top-level output must always be valid Alfred Script Filter JSON:

```json
{
  "items": []
}
```

Success item contract:

```json
{
  "uid": "subject-123",
  "title": "Subject name",
  "subtitle": "Type + localized metadata",
  "arg": "https://bgm.tv/subject/123",
  "valid": true
}
```

Rules:

- Successful subject feedback sets top-level `skipknowledge: true` so stable subject UIDs retain API match order.
- `title` uses localized name when available, otherwise canonical name.
- `subtitle` should include type tag and score/rank metadata when available.
- `arg` uses API `url` when present.
- URL fallback when API `url` is missing: `https://bgm.tv/subject/<id>`.
- Error/empty guidance rows must set `valid: false`.

## API Strategy

- Primary endpoint (v0-first): `https://api.bgm.tv/v0/search/subjects`.
- Required request posture:
  - Use explicit timeout from `BANGUMI_TIMEOUT_MS` effective value.
  - Always send `User-Agent` header (from `BANGUMI_USER_AGENT` if set; otherwise default UA).
  - Send `Authorization: Bearer <token>` only when `BANGUMI_API_KEY` is non-empty.
- Fallback policy is controlled by `BANGUMI_API_FALLBACK`:
  - `auto`: allow narrow compatibility fallback for endpoint/schema regressions.
  - `never`: disable fallback and fail fast on v0 failures.
  - `always`: force compatibility fallback path (operator use only).

## Error Mapping

| Scenario | Detection signal | Alfred title | Alfred subtitle |
| --- | --- | --- | --- |
| Empty query | Query empty after trim | `Enter a search query` | `Type keywords after bgm to search Bangumi.` |
| Short query | Query length `<2` | `Keep typing (2+ chars)` | `Type at least 2 characters before searching Bangumi.` |
| Invalid config | Invalid `BANGUMI_*` value | `Invalid Bangumi workflow config` | `Check BANGUMI_* values and retry.` |
| Missing API key | Key required but absent | `Bangumi API key is missing` | `Set BANGUMI_API_KEY and retry.` |
| Rate limit | API `429` / throttle signal | `Bangumi API rate-limited` | `Retry later or lower BANGUMI_MAX_RESULTS.` |
| API unavailable | DNS/TLS/network/timeout/upstream `5xx` | `Bangumi API unavailable` | `Cannot reach Bangumi API now. Check network and retry.` |
| No results | API success with empty result set | `No subjects found` | `Try broader keywords or switch type token.` |

## Environment Variables

| Variable | Required | Default | Effective rule |
| --- | --- | --- | --- |
| `BANGUMI_API_KEY` | No | `` | Optional API token; workflow config value has precedence over inherited env. |
| `BANGUMI_MAX_RESULTS` | No | `10` | Base-10 integer, clamped to `1..20`. |
| `BANGUMI_TIMEOUT_MS` | No | `8000` | Base-10 integer milliseconds, clamped to `1000..30000`. |
| `BANGUMI_USER_AGENT` | No | `` | Optional explicit UA override; empty means built-in default UA. |
| `BANGUMI_CACHE_DIR` | No | `` | Cache dir precedence: explicit var -> Alfred cache (`ALFRED_WORKFLOW_CACHE/bangumi-cli`) -> `${XDG_CACHE_HOME:-$HOME/.cache}/nils-bangumi-cli`. |
| `BANGUMI_IMAGE_CACHE_TTL_SECONDS` | No | `86400` | Base-10 integer seconds, clamped to `>= 0`. |
| `BANGUMI_IMAGE_CACHE_MAX_MB` | No | `128` | Base-10 integer, clamped to `1..1024`. |
| `BANGUMI_API_FALLBACK` | No | `auto` | Allowed values: `auto`, `never`, `always`. |

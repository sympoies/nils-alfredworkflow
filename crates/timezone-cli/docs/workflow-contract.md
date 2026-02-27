# Multi Timezone Workflow Contract

## Purpose

This document defines the runtime behavior contract for the `multi-timezone` Alfred workflow.
It is the source of truth for timezone input precedence, local-timezone fallback behavior,
row ordering, Alfred item JSON shape, copy-action behavior, and error mapping.

## Keyword and Query Handling

- Workflow keyword: `tz`.
- Input query is read from Alfred script filter argument.
- Input source precedence:
  1. Query text (`tz <zones>`) when non-empty.
  2. Workflow config field `MULTI_TZ_ZONES` when query is empty.
  3. Local-timezone detection fallback chain when both are empty.
- Supported separators in query/config values: comma (`,`) and newline (`\n`).
- Tokenization and ordering semantics are shared through `nils-workflow-common` ordered-list parser utilities.
- Timezone IDs must be valid IANA zone names (for example `Asia/Taipei`).

## Local Timezone Detection Fallback Chain

When no explicit timezone list is provided, local timezone must be resolved in this order:

1. `MULTI_TZ_LOCAL_OVERRIDE`
2. `TZ`
3. `iana_time_zone` runtime lookup
4. Platform command lookup
   - macOS: `/usr/sbin/systemsetup -gettimezone`
   - Linux: `timedatectl show -p Timezone --value`
5. `/etc/localtime` symlink parse (`.../zoneinfo/<IANA>`)
6. `UTC` terminal fallback

Rules:

- Invalid values at any step do not stop execution.
- Resolution must continue to the next step on parse/probe failure.
- Terminal fallback must always produce a valid timezone (`UTC`).

## Output Contract

Each success row represents current time for one timezone.

Success row schema:

```json
{
  "uid": "Asia/Taipei",
  "title": "2026-02-10 20:35:00",
  "subtitle": "Asia/Taipei (UTC+08:00)",
  "arg": "Asia/Taipei 2026-02-10 20:35:00 UTC+08:00",
  "valid": true
}
```

Rules:

- `uid` must equal timezone ID for deterministic order assertions.
- `title` is formatted local time (`YYYY-MM-DD HH:MM:SS`) in that timezone.
- `subtitle` must include timezone ID and UTC offset.
- `arg` is the copy payload and must be non-empty for success rows.
- `valid` is explicitly `true` for success rows.
- Output row order must strictly follow resolved input order.

Fallback/error row schema:

```json
{
  "title": "Invalid timezone",
  "subtitle": "Use IANA timezone IDs, for example Asia/Taipei",
  "valid": false
}
```

Rules:

- Fallback rows must be valid Alfred JSON items.
- Fallback rows must set `valid: false`.
- Fallback rows must not include `arg`.

## Action Handling Contract

- `action_copy.sh` accepts one argument (selected row `arg`).
- Missing/empty argument:
  - Print usage to stderr.
  - Exit with code `2`.
- Valid argument:
  - Copy exact bytes to clipboard via `pbcopy`.
  - Do not append extra newline.

## Error Mapping

| Scenario | Detection signal | Alfred title | Alfred subtitle | Item behavior |
| --- | --- | --- | --- | --- |
| Missing binary | `timezone-cli binary not found` | `timezone-cli binary not found` | `Package workflow or set TIMEZONE_CLI_BIN to an executable timezone-cli path.` | `valid: false` |
| Invalid timezone input | parse error, unsupported timezone | `Invalid timezone` | `Use IANA timezone IDs, for example Asia/Taipei.` | `valid: false` |
| Runtime failure | IO/process/runtime errors | `Timezone runtime failure` | `timezone-cli failed during conversion. Retry or inspect stderr details.` | `valid: false` |
| Generic failure | any other stderr case | `Multi Timezone error` | `<normalized error message>` | `valid: false` |

## Environment Variables

### `TIMEZONE_CLI_BIN` (optional)

- Optional override path for `timezone-cli` executable.
- Resolution order:
  1. `TIMEZONE_CLI_BIN` (if executable)
  2. Packaged binary `./bin/timezone-cli`
  3. `target/release/timezone-cli`
  4. `target/debug/timezone-cli`

### `MULTI_TZ_ZONES` (optional)

- Workflow-level default timezone list.
- Used only when query input is empty.

### `MULTI_TZ_LOCAL_OVERRIDE` (optional)

- Highest-priority local-timezone override used by local fallback mode.
- Workflow default is `Europe/London`.
- Must be an IANA timezone ID when set.

## Compatibility Notes

- Contract targets Alfred 5 script filter JSON shape.
- Runtime targets macOS 13+ for end-user Alfred execution.
- Linux compatibility is required for CI lint/test/package validation.

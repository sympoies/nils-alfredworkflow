# Google CLI native contract

> Status: active

## Purpose

Define the native Rust command contract for `google-cli` over the repo-scoped Google surface: `auth`, `gmail`,
`drive`, and `calendar`.

- Package: `nils-google-cli`
- Binary: `google-cli`

## Scope

- Native command ownership includes:
  - `auth credentials set|list`
  - `auth add|list|status|remove|alias|manage`
  - `gmail search|get|send|thread get|thread modify`
  - `drive ls|search|get|download|upload`
  - `calendar calendars list`, `calendar events list|get|create`
- Out of scope:
  - browser account-manager UI rebuild
  - non-scoped domains (`chat`, `docs`, `forms`, `people`, and similar)
  - calendar mutation beyond event creation (`events update|delete`, calendar
    creation/deletion, ACL changes)
  - service-account flows in this phase unless explicitly added later

## Output and error envelope

- Native responses keep repository CLI envelope behavior (`schema_version`, `command`, `ok`, and `result`/`error`).
- Native runtime error taxonomy continues to separate user errors from runtime failures.
- Native command IDs remain stable and service-scoped (`google.auth.*`, `google.gmail.*`, `google.drive.*`,
  `google.calendar.*`).

## OAuth modes

`auth add` supports three native modes:

- `loopback`: launch browser, receive callback on loopback listener, exchange code.
- `manual`: display auth URL and accept pasted code for exchange.
- `remote`: run explicit step-based exchange where state is generated, persisted, and validated by native runtime.

Required behavior:

- Remote/manual state tracking must prevent wrapper-era state mismatch failures.
- Browser launch is an auth helper concern only; account-manager UI is not opened.

## Account and default resolution semantics

Native account targeting order for auth-adjacent commands:

1. explicit `--account`
2. alias mapping
3. configured default account
4. single stored account when unambiguous
5. deterministic error when none of the above resolve

`auth status` contract:

- `auth status` without `--account` must apply the same default account resolution order.
- `auth status` must never return an empty account payload when multiple accounts exist without a default account.
- Ambiguous-account failures must include explicit corrective guidance.

## `auth manage` contract

- `auth manage` is terminal-native only.
- No browser account-manager page is launched.
- The command returns account summary/help output and, when appropriate, guidance to use `auth alias` and default-account
  configuration.

## Service behavior contract

- `gmail`, `drive`, and `calendar` commands execute through native client modules owned by this crate.
- Generated API clients are the primary transport path.
- `reqwest` is an allowed fallback path when generated coverage is incomplete for a command edge case.
- `calendar` uses the blocking `reqwest` path against the Calendar v3 REST endpoints, matching how the existing `gmail`
  and `drive` client modules are actually implemented today.

## Requested OAuth scopes

`auth add` requests `gmail.modify`, `drive`, and `calendar`. Google returns the union of scopes already granted to this
client for the account, so an account with an older grant can carry more than this list while a fresh grant carries
exactly this list. Callers must not infer a scope boundary from this list alone.

## Calendar service contract

Command IDs are `google.calendar.calendars.list` and `google.calendar.events.{list,get,create}`.

- `--calendar-id` is required for every `events` command. The crate does not resolve calendar aliases, own a default
  calendar, or know about any caller-side grouping; a consumer that maps its own identifiers to calendars owns that
  mapping and its fail-closed behavior.
- `events list` sets `singleEvents=true` and `orderBy=startTime` so recurring events arrive as concrete instances.
- `--from` / `--to` map to `timeMin` / `timeMax` and must be RFC3339 instants carrying an offset or `Z`. The list API
  has no companion `timeZone` field, so a bare wall-clock value is rejected rather than assumed to be UTC.
- `events create` accepts either an all-day date pair or a timed pair, never a mix. A timed `--start` without an offset
  requires `--time-zone <IANA zone>`, which is sent as the event body's `timeZone`; an offset-bearing value keeps its
  offset.
- Timed values are normalized to a complete RFC3339 `dateTime` before the request is sent, because Calendar v3 answers
  HTTP 400 for a seconds-less `HH:MM`. A malformed clock is rejected locally instead of being forwarded.
- An omitted all-day `--end` becomes start + 1 day because the Calendar API treats `end.date` as exclusive.
- `--private-property key=value` is repeatable and maps to `extendedProperties.private`, which `events list` can filter
  on through `privateExtendedProperty`. This is the supported way for a consumer to store and query its own
  back-references on an event.
- `GOOGLE_CLI_CALENDAR_FIXTURE_PATH` / `GOOGLE_CLI_CALENDAR_FIXTURE_JSON` serve a local fixture store so command wiring
  is testable without network access. Fixture mode never mutates remote state: `events create` echoes the built request.

## Compatibility notes

- This contract replaces wrapper pass-through ownership language for future implementation work.
- Sprint 1 freezes behavior definitions; implementation arrives incrementally in later sprints.

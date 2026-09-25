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
  - `drive ls|search|get|download|upload|mkdir|update|rename|move|copy|trash|untrash`
  - `calendar calendars list`, `calendar events list|get|create|update|delete|respond`
- Out of scope:
  - browser account-manager UI rebuild
  - non-scoped domains (`chat`, `docs`, `forms`, `people`, and similar)
  - calendar creation/deletion, ACL changes, and changing another attendee's
    response
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
- The remote `state` must be unguessable, generated fresh for every step 1.
- Remote step 2 may take the redirected callback URL on stdin
  (`--callback-url-stdin`) so the one-time code never appears in process
  arguments; the code is percent-decoded and a callback `error` is reported.
- Browser launch is an auth helper concern only; account-manager UI is not opened.

## Default account and removal

- `auth default <account>` sets the configured default account used in step 3
  of the resolution order below.
- `auth remove --revoke` revokes the refresh token at the provider before
  forgetting it. A token the provider already rejects counts as revoked; any
  other failure keeps the account so a live grant is never left untracked.

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

## Drive write contract

`drive mkdir <name> --parent <folderId>`, `update <fileId> <localPath> [--mime <type>]`,
`rename <fileId> --name <name>`, `move <fileId> --parent <newFolderId> --from <oldFolderId>`,
`copy <fileId> --parent <folderId> [--name <name>]`, `trash <fileId>`, and
`untrash <fileId>` require an explicit `-a <account>` / `--account <account>`.
They return the standard JSON envelope with `result.file` read back from Drive
after the mutation; callers must not infer success from the mutation response
alone. `move` checks that `--from` is a current parent before changing it.
`trash` is reversible; there is no permanent-delete command. These general
purpose CLI verbs do not enforce a caller's folder or audience boundary.

## Calendar service contract

Command IDs are `google.calendar.calendars.list` and `google.calendar.events.{list,get,create,delete}`.

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
- `events delete <eventId>` removes one event and answers `deleted: true`. Calendar returns 410 Gone for an event that
  was already deleted, which maps to the same not-found error as 404 so a repeated delete says so plainly instead of
  surfacing a raw HTTP failure. It never reports success for an id that is not there.
- `events respond <eventId> --response accepted|declined|tentative` answers an invitation. It reads the event, changes
  only the `responseStatus` of the attendee marked `self`, and sends the whole `attendees` array back with
  `sendUpdates` (`all` by default). A non-attendee or the organizer is refused as invalid input. Event views carry
  `self_attendee` so unanswered invitations (`needsAction`) are visible.
- `GOOGLE_CLI_CALENDAR_FIXTURE_PATH` / `GOOGLE_CLI_CALENDAR_FIXTURE_JSON` serve a local fixture store so command wiring
  is testable without network access. Fixture mode never mutates remote state: `events create` echoes the built request
  and `events delete` resolves the id, then reports success without removing anything.

## Compatibility notes

- This contract replaces wrapper pass-through ownership language for future implementation work.
- Sprint 1 freezes behavior definitions; implementation arrives incrementally in later sprints.

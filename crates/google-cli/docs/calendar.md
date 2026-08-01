# google-cli calendar

Authoritative Calendar documentation for `google-cli`.

## Scope

- `calendar calendars list`
- `calendar events list`
- `calendar events get <eventId>`
- `calendar events create`
- `calendar events delete <eventId>`

Editing an existing event (`events update`), calendar creation/deletion, and ACL changes are deliberately out of
scope. Delete is in scope because an event a consumer created is an event it must be able to retract; without it the
only way back is the Calendar UI.

## Runtime model

- The default path calls the live Calendar v3 API with the OAuth bearer token from the auth module, refreshing and
  persisting the access token the same way the Gmail and Drive clients do.
- `auth add` requests the `calendar` scope. Google returns the union of scopes already granted to this client for the
  account, so an existing grant can carry more; do not infer a scope boundary from the request list.
- Fixture mode is enabled only when one of these env vars is set:
  - `GOOGLE_CLI_CALENDAR_FIXTURE_PATH`
  - `GOOGLE_CLI_CALENDAR_FIXTURE_JSON`
- Fixture mode never mutates remote state. `events create` echoes the request it built and `events delete` resolves
  the id then reports success without removing anything, so command wiring stays testable without network access.

## Calendar selection

`--calendar-id` is required for every `events` command. There is no default calendar and no alias resolution: a
consumer that maps its own identifiers (chat groups, projects, people) to calendars owns that mapping and its
fail-closed behavior.

List the calendars visible to the account to discover ids:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com calendar calendars list --max 50
```

## Time values

`events create` accepts either an all-day date pair or a timed pair, never a mix.

| Form | Example | Notes |
| --- | --- | --- |
| All-day | `--start 2026-08-15` | `--end` is **exclusive**, per the Calendar API. Omitted `--end` becomes start + 1 day. |
| Timed, with offset | `--start 2026-08-15T11:20:00+08:00` | The offset is preserved; `--time-zone` is optional. |
| Timed, wall clock | `--start 2026-08-15T11:20 --time-zone Asia/Taipei` | `--time-zone` is **required**; it is sent as the event body's `timeZone`. |

Seconds are optional on input and are filled in before the request is sent: Calendar v3 requires a complete RFC3339
`dateTime` and answers HTTP 400 for a bare `HH:MM`. A malformed clock (`1:20`, `24:00`, `11:60`) is rejected locally
rather than forwarded.

`events list` uses `--from` / `--to` for `timeMin` / `timeMax`, which must be RFC3339 instants carrying an offset or
`Z`. Unlike an event body, the list API has no companion `timeZone` field, so a bare wall-clock value is rejected
rather than assumed to be UTC.

## Back-references

`--private-property key=value` is repeatable and maps to `extendedProperties.private`. `events list` can filter on the
same pairs through `privateExtendedProperty`, which is the supported way to store and later find a consumer's own
identifiers on an event.

## Command notes

List calendars:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com calendar calendars list
```

List events in a window:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events list \
  --calendar-id "<calendar_id>" \
  --from 2026-08-01T00:00:00+08:00 \
  --to 2026-09-01T00:00:00+08:00 \
  --max 50
```

Find only the events a given consumer created:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events list \
  --calendar-id "<calendar_id>" \
  --private-property gn_group_id=C5be37c3cbf563864ed61df6a38f7c8b0
```

Get one event:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events get <event_id> --calendar-id "<calendar_id>"
```

Create a timed event with a back-reference:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events create \
  --calendar-id "<calendar_id>" \
  --summary "全家去吃千葉火鍋" \
  --start 2026-08-15T11:20 \
  --end 2026-08-15T13:00 \
  --time-zone Asia/Taipei \
  --location "千葉火鍋" \
  --private-property gn_note_id=n_e923e876
```

Create an all-day event:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events create \
  --calendar-id "<calendar_id>" \
  --summary "Trip" \
  --start 2026-08-15 \
  --end 2026-08-18
```

Delete an event:

```bash
cargo run -p nils-google-cli -- --output json -a you@example.com \
  calendar events delete <event_id> --calendar-id "<calendar_id>"
```

`deleted: true` means that id is gone. An id that was never there, or was already deleted, is
`NILS_GOOGLE_016` — the command never reports success for an event it did not remove.

## Error codes

| Code | Meaning |
| --- | --- |
| `NILS_GOOGLE_015` | User input error: unknown flag, missing required flag, ambiguous or malformed time value. |
| `NILS_GOOGLE_016` | Requested calendar or event was not found. |
| `NILS_GOOGLE_017` | Calendar runtime failure: transport error, non-2xx response, or unparseable body. |

## Validation

- `cargo test -p nils-google-cli --lib calendar`
- `cargo test -p nils-google-cli --test integration calendar`

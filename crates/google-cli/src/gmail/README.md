# google-cli gmail module

Authoritative Gmail documentation for `google-cli`.

## Scope

- `gmail search <query...>`
- `gmail get <messageId>`
- `gmail send`
- `gmail thread get|modify`

## Runtime model

- Default path calls live Gmail API with OAuth bearer token from auth module.
- Fixture mode is enabled only when one of these env vars is set:
  - `GOOGLE_CLI_GMAIL_FIXTURE_PATH`
  - `GOOGLE_CLI_GMAIL_FIXTURE_JSON`
- Output contract remains stable across JSON/plain modes.

## Command notes

Search:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  gmail search --query "in:inbox" --max 3 --format metadata --headers Subject,From,Date
```

Get message:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  gmail get <message_id> --format full --headers Subject,From,Date
```

Send message:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  gmail send --to recipient@example.com --subject "hello" --body "world"
```

Thread read/modify:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  gmail thread get <thread_id> --format metadata --headers Subject,From

cargo run -p google-cli -- --json -a you@example.com \
  gmail thread modify <thread_id> --add-label STARRED --remove-label UNREAD
```

## MIME behavior

- `gmail send` assembles MIME using `mail-builder`.
- Attachment content type is inferred with `mime_guess` unless overridden upstream.
- Live send path submits raw MIME to Gmail API.

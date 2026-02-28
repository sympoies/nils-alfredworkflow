# google-cli

Native Rust CLI for scoped Google `auth`, `gmail`, and `drive` commands.

## Overview

- Runs auth, Gmail, and Drive commands with native Rust implementations.
- Uses real OAuth token exchange/refresh for API calls.
- Keeps deterministic local test paths via explicit fixture/test env switches.

## Quick start

Set runtime environment:

```bash
export GOOGLE_CLI_CONFIG_DIR="$HOME/.config/google/credentials"
export GOOGLE_CLI_KEYRING_MODE=file
```

Set OAuth credentials:

```bash
cargo run -p google-cli -- auth credentials set \
  --client-id "<client_id>" \
  --client-secret "<client_secret>"
```

Login account (remote flow):

```bash
cargo run -p google-cli -- --json auth add you@example.com --remote --step 1
# Open result.authorization_url, then run step 2:
cargo run -p google-cli -- --json auth add you@example.com \
  --remote --step 2 \
  --state "<state>" \
  --code "<code>"
```

Validate account status:

```bash
cargo run -p google-cli -- --json auth status -a you@example.com
```

Detailed auth operations guide: `docs/auth-setup-guide.md`.

## Module docs (single source of truth)

- Auth: `src/auth/README.md`
- Gmail: `src/gmail/README.md`
- Drive: `src/drive/README.md`

## Command help

```bash
cargo run -p google-cli -- auth --help
cargo run -p google-cli -- gmail --help
cargo run -p google-cli -- drive --help
```

## Environment variables

- `GOOGLE_CLI_CONFIG_DIR`: override auth config directory.
- `GOOGLE_CLI_KEYRING_MODE`: token storage mode (`keyring`, `file`, `fail`, `keyring-strict`).
- `GOOGLE_CLI_AUTH_DISABLE_BROWSER`: disable browser auto-launch for auth flows.
- `GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE`: test-only OAuth bypass switch. Do not use in normal runs.
- `GOOGLE_CLI_GMAIL_FIXTURE_PATH`: Gmail fixture JSON file path for local tests.
- `GOOGLE_CLI_GMAIL_FIXTURE_JSON`: inline Gmail fixture JSON for local tests.
- `GOOGLE_CLI_DRIVE_FIXTURE_PATH`: Drive fixture JSON file path for local tests.
- `GOOGLE_CLI_DRIVE_FIXTURE_JSON`: inline Drive fixture JSON for local tests.

## Output contract

- Envelope keys: `schema_version`, `command`, `ok`.
- Success payload key: `result`.
- Error payload key: `error` (stable error code + details).
- `--json` for machine-readable output.
- `--plain` for stable plain text.

## Validation

```bash
cargo test -p google-cli
```

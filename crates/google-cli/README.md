# google-cli

Native Rust migration crate for scoped Google `auth`, `gmail`, and `drive` commands.

## Sprint 3 status

- Native dependency stack is pinned in `crates/google-cli/Cargo.toml`.
- Auth commands now execute through native Rust modules (`src/auth/*`) with local config + token persistence.
- Gmail commands now execute through native Rust modules (`src/gmail/*`) with native account resolution reuse.
- Drive remains wrapper-backed through `src/runtime.rs` until the native Drive sprint lands.

## Command scope to preserve

## Auth

| Command | Sprint 1 contract stance |
| --- | --- |
| `auth credentials <...>` | Fully native config/credential read-write behavior. |
| `auth add <email> [flags...]` | Fully native OAuth modes: `loopback`, `manual`, `remote`. |
| `auth list` | Native account inventory behavior (accounts/default/aliases). |
| `auth status` | Deterministic default-account resolution or explicit ambiguity error. |
| `auth remove <email>` | Native token + metadata removal behavior. |
| `auth alias <...>` | Native alias metadata behavior. |
| `auth manage [flags...]` | Terminal summary-only behavior (no browser account-manager UI). |

## Gmail

| Command | Sprint 1 contract stance |
| --- | --- |
| `gmail search <query...> [flags...]` | Primary path is generated client; reqwest fallback allowed. |
| `gmail get <messageId> [flags...]` | Primary generated client path. |
| `gmail send [flags...]` | Native MIME path using `mail-builder` and `mime_guess`. |
| `gmail thread <...>` | Primary generated client path with fallback allowance. |

## Drive

| Command | Sprint 1 contract stance |
| --- | --- |
| `drive ls [flags...]` | Primary generated client path with fallback allowance. |
| `drive search <query...> [flags...]` | Primary generated client path with fallback allowance. |
| `drive get <fileId>` | Primary generated client path with fallback allowance. |
| `drive download <fileId> [flags...]` | Primary generated client path with fallback allowance. |
| `drive upload <localPath> [flags...]` | Primary generated client path with fallback allowance. |

## Environment variables

- `GOOGLE_CLI_GOG_BIN`: explicit override for wrapper-backed commands (`drive`) during migration.
- `GOOGLE_CLI_CONFIG_DIR`: override native auth config directory.
- `GOOGLE_CLI_KEYRING_MODE`: auth storage mode (`keyring`, `file`, `fail`, `keyring-strict`).
- `GOOGLE_CLI_AUTH_DISABLE_BROWSER`: disable automatic browser launch for loopback auth.
- `GOOGLE_CLI_GMAIL_FIXTURE_PATH`: optional fixture JSON path for local/native Gmail integration testing.

## Output contract

- Envelope stays repository-standard: `schema_version`, `command`, `ok`, and `result`/`error`.
- `--json` expects machine-readable output; `--plain` requests stable text output.

## Validation

- `cargo test -p google-cli --test auth_storage`
- `cargo test -p google-cli --test auth_oauth_flow`
- `cargo test -p google-cli --test auth_account_resolution`
- `cargo test -p google-cli --test auth_cli_contract`
- `cargo test -p google-cli --test gmail_read`
- `cargo test -p google-cli --test gmail_thread`
- `cargo test -p google-cli --test gmail_send`
- `cargo test -p google-cli --test gmail_cli_contract`
- `cargo test -p google-cli --test account_resolution_shared`
- `cargo test -p google-cli --test native_no_gog`

## Manual smoke checklist (auth)

1. `cargo run -p google-cli -- auth credentials set --client-id <id> --client-secret <secret>`
2. `cargo run -p google-cli -- auth add <email>` (loopback mode; callback flow)
3. `cargo run -p google-cli -- auth list`
4. `cargo run -p google-cli -- auth status` (verifies default account resolution)
5. `cargo run -p google-cli -- auth manage` (summary-only; no browser manager page)
6. Optional remote flow step 1:
   `cargo run -p google-cli -- auth add <email> --remote --step 1`
7. Optional remote flow step 2:
   `cargo run -p google-cli -- auth add <email> --remote --step 2 --state <state> --code <code>`

## Manual smoke checklist (gmail)

1. `cargo run -p google-cli -- gmail search "from:team@example.com" --max 5 --format metadata --headers Subject,From`
2. `cargo run -p google-cli -- gmail get <messageId> --format full`
3. `cargo run -p google-cli -- gmail thread get <threadId> --format metadata --headers Subject`
4. `cargo run -p google-cli -- gmail thread modify <threadId> --add-label STARRED --remove-label UNREAD`
5. `cargo run -p google-cli -- gmail send --to team@example.com --subject "Sprint Update" --body "Native Gmail path"`

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/features/auth.md`](docs/features/auth.md)
- [`docs/features/gmail.md`](docs/features/gmail.md)
- [`docs/features/drive.md`](docs/features/drive.md)
- [`../../docs/specs/google-cli-native-contract.md`](../../docs/specs/google-cli-native-contract.md)

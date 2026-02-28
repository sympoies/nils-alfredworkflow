# google-cli

Native Rust migration crate for scoped Google `auth`, `gmail`, and `drive` commands.

## Sprint 5 status

- Native dependency stack is pinned in `crates/google-cli/Cargo.toml`.
- Auth commands now execute through native Rust modules (`src/auth/*`) with local config + token persistence.
- Gmail commands now execute through native Rust modules (`src/gmail/*`) with native account resolution reuse.
- Drive commands (`ls/search/get/download/upload`) now execute through native Rust modules (`src/drive/*`).
- Wrapper-era runtime shelling and `gog` override paths are removed from production code.

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
| `drive download <fileId> [flags...]` | Native destination/export path with overwrite controls. |
| `drive upload <localPath> [flags...]` | Primary generated client path with fallback allowance. |

## Environment variables

- `GOOGLE_CLI_CONFIG_DIR`: override native auth config directory.
- `GOOGLE_CLI_KEYRING_MODE`: auth storage mode (`keyring`, `file`, `fail`, `keyring-strict`).
- `GOOGLE_CLI_AUTH_DISABLE_BROWSER`: disable automatic browser launch for loopback auth.
- `GOOGLE_CLI_GMAIL_FIXTURE_PATH`: optional fixture JSON path for local/native Gmail integration testing.
- `GOOGLE_CLI_DRIVE_FIXTURE_PATH`: optional fixture JSON path for local/native Drive integration testing.

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
- `cargo test -p google-cli --test drive_read`
- `cargo test -p google-cli --test drive_download`
- `cargo test -p google-cli --test drive_upload`
- `cargo test -p google-cli --test drive_cli_contract`
- `cargo test -p google-cli --test native_no_gog`
- `cargo run -p google-cli -- auth --help`
- `cargo run -p google-cli -- gmail --help`
- `cargo run -p google-cli -- drive --help`

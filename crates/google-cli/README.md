# google-cli

Rust wrapper over `gog` for scoped Google auth, Gmail, and Drive commands.

## Quickstart

1. Install `gog` v0.11.x and ensure it is on `PATH`, or set `GOOGLE_CLI_GOG_BIN=/absolute/path/to/gog`.
2. Run one of the supported wrapper groups:
   - `cargo run -p google-cli -- auth list`
   - `cargo run -p google-cli -- gmail search "from:me newer_than:7d"`
   - `cargo run -p google-cli -- drive ls --parent root`
3. Use `--json` when a caller needs a stable wrapper envelope, or `--plain` to forward `gog`'s TSV-oriented text.

## Commands

## Auth

| Command | Notes | Example |
| --- | --- | --- |
| `auth credentials <...>` | Pass-through wrapper for scoped OAuth client credential management. | `cargo run -p google-cli -- auth credentials list` |
| `auth add <email> [flags...]` | Supports pass-through auth flags such as `--manual`, `--remote`, `--services`, `--readonly`. | `cargo run -p google-cli -- auth add me@example.com --manual --services gmail,drive` |
| `auth list` | Lists stored accounts through `gog auth list`. | `cargo run -p google-cli -- auth list` |
| `auth status` | Shows auth/keyring status. | `cargo run -p google-cli -- auth status` |
| `auth remove <email>` | Removes a stored refresh token. | `cargo run -p google-cli -- auth remove me@example.com` |
| `auth alias <...>` | Pass-through wrapper for alias subcommands such as `list`, `set`, `unset`. | `cargo run -p google-cli -- auth alias set work me@example.com` |
| `auth manage [flags...]` | Opens the upstream account manager. | `cargo run -p google-cli -- auth manage --force-consent` |

## Gmail

| Command | Notes | Example |
| --- | --- | --- |
| `gmail search <query...> [flags...]` | Scoped wrapper for Gmail search/list/query aliases. | `cargo run -p google-cli -- gmail search "label:inbox newer_than:7d" --max 10` |
| `gmail get <messageId> [flags...]` | Retrieves a message with pass-through format flags. | `cargo run -p google-cli -- gmail get 190abc --format metadata --headers Subject,From` |
| `gmail send [flags...]` | Send-focused wrapper that forwards address/body/attachment flags unchanged. | `cargo run -p google-cli -- gmail send --to team@example.com --subject Status --body Wrapped` |
| `gmail thread <...>` | Pass-through thread wrapper for selected read/modify thread operations. | `cargo run -p google-cli -- gmail thread get 18ff-thread --format metadata` |

## Drive

| Command | Notes | Example |
| --- | --- | --- |
| `drive ls [flags...]` | Lists files with pass-through paging/filter flags. | `cargo run -p google-cli -- drive ls --parent root --query "mimeType='application/pdf'"` |
| `drive search <query...> [flags...]` | Search-focused wrapper for Drive full-text queries. | `cargo run -p google-cli -- drive search report --max 10` |
| `drive get <fileId>` | Retrieves file metadata. | `cargo run -p google-cli -- drive get 1AbCdE` |
| `drive download <fileId> [flags...]` | Download/export wrapper that forwards `--out` and `--format`. | `cargo run -p google-cli -- drive download 1AbCdE --out /tmp/report.pdf --format pdf` |
| `drive upload <localPath> [flags...]` | Upload wrapper that forwards `--parent`, `--name`, `--replace`, `--convert*`. | `cargo run -p google-cli -- drive upload ./report.pdf --parent folder-1 --name report.pdf` |

## Environment Variables

- `GOOGLE_CLI_GOG_BIN`: explicit override for the wrapped `gog` binary.

## Output Contract

- Default mode is human-oriented passthrough from `gog`.
- `--plain` requests upstream stable text output and forwards it without wrapper mutation.
- `--json` requires valid upstream JSON and wraps it in a repo-local envelope: `schema_version`, `command`, `ok`, `result` or `error`.
- Exit codes follow repo CLI policy:
  - `0`: success
  - `1`: runtime/dependency/process/invalid-output failure
  - `2`: user/input/output-mode failure

## Standards Status

- Wrapper contract documented in `docs/specs/google-cli-wrapper-contract.md`.
- Crate docs and feature docs live under `crates/google-cli/docs/`.
- Contract tests cover success envelopes, process failures, invalid JSON decoding, and fake-`gog` routing assertions.

## Known limitations

- Scope is intentionally limited to `auth`, `gmail`, and `drive`; other `gog` groups are out of phase for this crate.
- This crate is not wired into Alfred workflows or workflow packaging in this phase.
- Wrapper help is wrapper-owned; advanced upstream flags are forwarded after the scoped subcommand rather than re-modeled one-by-one in clap.

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/features/auth.md`](docs/features/auth.md)
- [`docs/features/gmail.md`](docs/features/gmail.md)
- [`docs/features/drive.md`](docs/features/drive.md)

## Validation

- `cargo check -p google-cli`
- `cargo test -p google-cli`
- `cargo run -p google-cli -- --help`
- `cargo run -p google-cli -- auth --help`
- `cargo run -p google-cli -- gmail --help`
- `cargo run -p google-cli -- drive --help`

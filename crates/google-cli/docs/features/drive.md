# Drive native contract

## Scope

- Repo-owned native scope:
  - `ls`
  - `search <query...>`
  - `get <fileId>`
  - `download <fileId>`
  - `upload <localPath>`
- Non-goals in this phase:
  - `copy`, `mkdir`, `delete`, `move`, `rename`, permission/comment administration

## Native semantics

- Account resolution is shared with auth/gmail: explicit `--account` first, then alias/default, then deterministic fallback.
- `drive download` runs through native Rust handling for destination path creation, overwrite control, and export format selection.
- `--out <path>` writes to an explicit target path.
- `--overwrite` allows replacing an existing file; without it, existing paths return a user error.
- `--format <fmt>` performs export mode when fixture/API metadata supports that format.
- Remaining Drive subcommands stay on the migration path and may still use fallback behavior until Sprint 4 lane completion.

## Manual smoke commands

1. `cargo run -p google-cli -- drive ls --parent root`
2. `cargo run -p google-cli -- drive search "name contains 'report'" --max 5`
3. `cargo run -p google-cli -- drive get <fileId>`
4. `cargo run -p google-cli -- drive upload ./report.pdf --name report.pdf --parent <folderId>`
5. `cargo run -p google-cli -- drive download <fileId> --out ./downloads/report.pdf --overwrite`

## Validation

- `cargo test -p google-cli --test drive_download`
- `cargo test -p google-cli --test drive_cli_contract`
- `cargo test -p google-cli --test account_resolution_shared`
- `rg -n "drive ls|drive search|drive upload|drive download|gog" crates/google-cli/README.md crates/google-cli/docs/features/drive.md`

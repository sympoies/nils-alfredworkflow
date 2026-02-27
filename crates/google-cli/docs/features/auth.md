# Auth wrapper

## Scope

- Supported wrapper surface in this phase:
  - `credentials <...>`
  - `add <email>`
  - `list`
  - `status`
  - `remove <email>`
  - `alias <...>`
  - `manage`
- Non-goals for this phase:
  - `service-account`
  - `tokens`
  - `keyring`
  - `keep`

## Command contract

- The wrapper forwards global `--account`, `--client`, `--json`, `--plain`, `--results-only`, `--select`, `--dry-run`, `--force`, `--no-input`, `--verbose`, `--color`, and `--enable-commands` flags before `gog auth ...`.
- Auth-specific pass-through flags remain upstream-owned. Representative supported flags include:
  - `auth add --manual --remote --services --readonly --drive-scope`
  - `auth manage --force-consent --services --timeout`
  - `auth alias list|set|unset`
  - `auth credentials list|set`
- When `--json` is selected, the wrapper expects valid `gog` JSON and re-wraps it in the local envelope.
- When `--plain` is selected, the wrapper forwards upstream stable text output unchanged.

## Usage examples

- `cargo run -p google-cli -- auth add me@example.com --manual --services gmail,drive`
- `cargo run -p google-cli -- auth list`
- `cargo run -p google-cli -- auth status`
- `cargo run -p google-cli -- auth remove me@example.com`
- `cargo run -p google-cli -- auth alias set work me@example.com`
- `cargo run -p google-cli -- auth credentials list`

## Validation

- `cargo run -p google-cli -- auth --help`
- `cargo run -p google-cli -- auth list --help`
- `cargo test -p google-cli --test auth_cli_contract`

## Troubleshooting

- Missing `gog`: check `GOOGLE_CLI_GOG_BIN` or confirm `gog` is installed on `PATH`.
- `NILS_GOOGLE_003`: upstream process launch/execution failed. Re-run the equivalent `gog auth ...` command directly to isolate upstream behavior.
- `NILS_GOOGLE_004`: upstream returned non-JSON output while the wrapper was in `--json` mode. Retry without `--json` or inspect the raw `gog` invocation first.

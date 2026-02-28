# nils-google-cli

Native Rust package for the `google-cli` binary, scoped to Google `auth`, `gmail`, and `drive` commands.

## Commands

| Command | Description |
| --- | --- |
| `google-cli auth <...>` | Manage OAuth credentials, account login, aliases, and account status. |
| `google-cli gmail <...>` | Search, inspect, and send Gmail messages through the native Gmail API client. |
| `google-cli drive <...>` | List, inspect, download, and upload Drive files through the native Drive API client. |

## Quick Start

Set runtime environment:

```bash
export GOOGLE_CLI_CONFIG_DIR="$HOME/.config/google/credentials"
export GOOGLE_CLI_KEYRING_MODE=file
```

Set OAuth credentials:

```bash
cargo run -p nils-google-cli -- auth credentials set \
  --client-id "<client_id>" \
  --client-secret "<client_secret>"
```

Login account (remote flow):

```bash
cargo run -p nils-google-cli -- --json auth add you@example.com --remote --step 1
# Open result.authorization_url, then run step 2:
cargo run -p nils-google-cli -- --json auth add you@example.com \
  --remote --step 2 \
  --state "<state>" \
  --code "<code>"
```

Validate account status:

```bash
cargo run -p nils-google-cli -- --json auth status -a you@example.com
```

## Command Help

```bash
cargo run -p nils-google-cli -- auth --help
cargo run -p nils-google-cli -- gmail --help
cargo run -p nils-google-cli -- drive --help
```

## Environment Variables

- `GOOGLE_CLI_CONFIG_DIR`: override auth config directory.
- `GOOGLE_CLI_KEYRING_MODE`: token storage mode (`keyring`, `file`, `fail`, `keyring-strict`).
- `GOOGLE_CLI_AUTH_DISABLE_BROWSER`: disable browser auto-launch for auth flows.
- `GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE`: test-only OAuth bypass switch. Do not use in normal runs.
- `GOOGLE_CLI_GMAIL_FIXTURE_PATH`: Gmail fixture JSON file path for local tests.
- `GOOGLE_CLI_GMAIL_FIXTURE_JSON`: inline Gmail fixture JSON for local tests.
- `GOOGLE_CLI_DRIVE_FIXTURE_PATH`: Drive fixture JSON file path for local tests.
- `GOOGLE_CLI_DRIVE_FIXTURE_JSON`: inline Drive fixture JSON for local tests.

## Output Contract

- Default output: human-readable native text for direct terminal usage.
- `--plain`: stable plain text without the JSON envelope.
- `--json`: repository envelope with `schema_version`, `command`, `ok`, and exactly one of `result` or `error`.
- `stderr`: deterministic user/runtime error text for non-JSON runs.
- Exit codes: `0` success, `1` runtime failure, `2` user/input/config error.

## Standards Status

- README/command docs: compliant.
- JSON service envelope (`schema_version/command/ok`): implemented.
- Default human-readable mode: implemented.

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/auth-setup-guide.md`](docs/auth-setup-guide.md)
- [`docs/auth.md`](docs/auth.md)
- [`docs/gmail.md`](docs/gmail.md)
- [`docs/drive.md`](docs/drive.md)

## Validation

- `cargo run -p nils-google-cli -- --help`
- `cargo run -p nils-google-cli -- auth --help`
- `cargo run -p nils-google-cli -- gmail --help`
- `cargo run -p nils-google-cli -- drive --help`
- `cargo test -p nils-google-cli`

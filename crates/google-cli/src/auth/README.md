# google-cli auth module

Authoritative auth documentation for `google-cli`.

Detailed operator guide: `../../docs/auth-setup-guide.md`.

## Scope

- `auth credentials set|list`
- `auth add <email>` (supports `--manual` and `--remote`)
- `auth list`
- `auth status`
- `auth remove <email-or-alias>`
- `auth alias set|remove|list`
- `auth manage`

## Runtime model

- Account resolution order: explicit `--account` -> alias -> default account -> single account -> deterministic error.
- Tokens are persisted via `GOOGLE_CLI_KEYRING_MODE` (`keyring`, `file`, `fail`, `keyring-strict`).
- Default mode uses real OAuth token exchange and refresh.

## Storage files

Under `GOOGLE_CLI_CONFIG_DIR` (or platform default config dir):

- `credentials.v1.json`: OAuth client credentials.
- `accounts.v1.json`: accounts/default/aliases.
- `tokens.v1.json`: file backend token store.
- `remote-state.v1.json`: temporary remote auth step state.

## Recommended login flow (remote)

Set runtime env:

```bash
export GOOGLE_CLI_CONFIG_DIR="$HOME/.config/google/credentials"
export GOOGLE_CLI_KEYRING_MODE=file
```

Set OAuth client credentials:

```bash
cargo run -p google-cli -- auth credentials set \
  --client-id "<client_id>" \
  --client-secret "<client_secret>"
```

Generate auth URL:

```bash
cargo run -p google-cli -- --json auth add you@example.com --remote --step 1
```

Open `result.authorization_url` in browser, then exchange code:

```bash
cargo run -p google-cli -- --json auth add you@example.com \
  --remote --step 2 \
  --state "<state>" \
  --code "<code>"
```

Verify token state:

```bash
cargo run -p google-cli -- --json auth status -a you@example.com
```

## Manual mode

```bash
cargo run -p google-cli -- auth add you@example.com --manual --code "<authorization_code>"
```

## Multi-account and alias

Add another account using the same remote flow.

Set alias:

```bash
cargo run -p google-cli -- --json auth alias set work terry@sympoies.com
```

List aliases:

```bash
cargo run -p google-cli -- --json auth alias list
```

## Troubleshooting

- `NILS_GOOGLE_005`: invalid input or missing auth prerequisites.
- `NILS_GOOGLE_006`: ambiguous account selection.
- `NILS_GOOGLE_007`: token store failure.
- Runtime HTTP 400 during exchange/refresh usually means expired code, reused code, or incorrect client credentials.

## Test-only switches

- `GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE=1` bypasses real token exchange for tests only.
- Do not use this flag in normal runs.

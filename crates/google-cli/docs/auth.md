# google-cli auth

Authoritative auth documentation for `google-cli`.

Detailed operator guide: [`auth-setup-guide.md`](auth-setup-guide.md).

## Scope

- `auth credentials set|list`
- `auth add <email>` (supports `--manual` and `--remote`)
- `auth list`
- `auth status`
- `auth default <email-or-alias>`
- `auth remove <email-or-alias> [--revoke]`
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
cargo run -p nils-google-cli -- auth credentials set \
  --client-id "<client_id>" \
  --client-secret "<client_secret>"
```

Generate auth URL:

```bash
cargo run -p nils-google-cli -- --json auth add you@example.com --remote --step 1
```

Open `result.authorization_url` in browser, then exchange code:

```bash
cargo run -p nils-google-cli -- --json auth add you@example.com \
  --remote --step 2 \
  --state "<state>" \
  --code "<code>"
```

Or paste the whole redirected address-bar URL on stdin, which keeps the
one-time code out of the process list and decodes it (`4%2F0A...`):

```bash
cargo run -p nils-google-cli -- --json auth add you@example.com \
  --remote --step 2 --callback-url-stdin
```

The `state` is random per step 1 and must match the pasted callback. A callback
carrying `error=` (for example a denied consent) is reported, not exchanged.

Verify token state:

```bash
cargo run -p nils-google-cli -- --json auth status -a you@example.com
```

## Manual mode

```bash
cargo run -p nils-google-cli -- auth add you@example.com --manual --code "<authorization_code>"
```

## Multi-account and alias

Add another account using the same remote flow.

Set alias:

```bash
cargo run -p nils-google-cli -- --json auth alias set work terry@sympoies.com
```

List aliases:

```bash
cargo run -p nils-google-cli -- --json auth alias list
```

## Default account and removal

Make a stored account the default that unqualified commands resolve to:

```bash
cargo run -p nils-google-cli -- --json auth default work
```

`auth remove <email-or-alias>` forgets the local token only; Google still
honours the grant until it is revoked in the account's security settings.
`--revoke` revokes the refresh token at the client's `revoke_uri` (default
`https://oauth2.googleapis.com/revoke`, override with
`auth credentials set --revoke-uri`) first. A token Google already rejects as
`invalid_token` counts as revoked. Any other failure keeps the account, so a
grant is never forgotten while it is still live. The result reports `revoked`
as `revoked`, `already-invalid`, `no-token`, or `null` without `--revoke`.

## Troubleshooting

- `NILS_GOOGLE_005`: invalid input or missing auth prerequisites.
- `NILS_GOOGLE_006`: ambiguous account selection.
- `NILS_GOOGLE_007`: token store failure.
- Runtime HTTP 400 during exchange/refresh usually means expired code, reused code, or incorrect client credentials.

## Test-only switches

- `GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE=1` bypasses real token exchange for tests only.
- Do not use this flag in normal runs.

# Auth Setup Guide

Step-by-step guide for OAuth login and multi-account operations in `google-cli`.

## Recommended runtime environment

Use a fixed config directory so account state is deterministic across runs:

```bash
export GOOGLE_CLI_CONFIG_DIR="$HOME/.config/google/credentials"
export GOOGLE_CLI_KEYRING_MODE=file
```

Notes:

- `GOOGLE_CLI_KEYRING_MODE=file` is recommended for local/dev repeatability.
- `GOOGLE_CLI_KEYRING_MODE=keyring` is supported if you want system keychain storage.

## 1. Configure OAuth client credentials

If you already have `client_id` and `client_secret`:

```bash
cargo run -p google-cli -- auth credentials set \
  --client-id "<client_id>" \
  --client-secret "<client_secret>"
```

If you have a Google OAuth JSON file (Desktop app format):

```bash
CONFIG_JSON="$HOME/.config/google/credentials/nils-google-cli.json"
CID=$(jq -r '.installed.client_id' "$CONFIG_JSON")
CSECRET=$(jq -r '.installed.client_secret' "$CONFIG_JSON")
AUTH_URI=$(jq -r '.installed.auth_uri' "$CONFIG_JSON")
TOKEN_URI=$(jq -r '.installed.token_uri' "$CONFIG_JSON")
REDIRECT_URI=$(jq -r '.installed.redirect_uris[0]' "$CONFIG_JSON")

cargo run -p google-cli -- auth credentials set \
  --client-id "$CID" \
  --client-secret "$CSECRET" \
  --auth-uri "$AUTH_URI" \
  --token-uri "$TOKEN_URI" \
  --redirect-uri "$REDIRECT_URI"
```

Verify:

```bash
cargo run -p google-cli -- --json auth credentials list
```

## 2. Login account using remote flow (recommended)

### Step 2.1: Generate auth URL

```bash
cargo run -p google-cli -- --json auth add you@example.com --remote --step 1
```

Copy from JSON:

- `result.authorization_url`
- `result.state`

Open `authorization_url` in browser and complete consent.

### Step 2.2: Exchange code

Browser redirect example:

```text
http://localhost/?state=state-xxxx&code=4/0A...
```

Run:

```bash
cargo run -p google-cli -- --json auth add you@example.com \
  --remote --step 2 \
  --state "state-xxxx" \
  --code "4/0A..."
```

## 3. Verify account and token state

```bash
cargo run -p google-cli -- --json auth list
cargo run -p google-cli -- --json auth status -a you@example.com
```

Expected:

- `auth list` includes your account in `result.accounts`
- `auth status` returns `result.has_token=true`

## 4. Add multiple accounts

Repeat remote flow for each account:

```bash
cargo run -p google-cli -- --json auth add another@example.com --remote --step 1
# browser consent
cargo run -p google-cli -- --json auth add another@example.com --remote --step 2 --state "<state>" --code "<code>"
```

Check final list:

```bash
cargo run -p google-cli -- --json auth list
```

## 5. Set and use aliases

Create alias:

```bash
cargo run -p google-cli -- --json auth alias set work terry@sympoies.com
```

List aliases:

```bash
cargo run -p google-cli -- --json auth alias list
```

Use alias in Gmail/Drive commands:

```bash
cargo run -p google-cli -- --json -a work gmail search --query "in:inbox" --max 3
cargo run -p google-cli -- --json -a work drive ls --max 5
```

## 6. Re-login or rotate token

Re-login existing account (safe overwrite):

```bash
cargo run -p google-cli -- --json auth add you@example.com --remote --step 1
# browser consent
cargo run -p google-cli -- --json auth add you@example.com --remote --step 2 --state "<state>" --code "<code>"
```

Remove account:

```bash
cargo run -p google-cli -- --json auth remove you@example.com
```

## 7. Troubleshooting

- `NILS_GOOGLE_005`
  - Input/flow issue (missing `--code`, no accounts configured, unknown account, etc.)
  - Fix command arguments or run `auth add <email>` first.
- `NILS_GOOGLE_007`
  - Token store failure (keyring/file backend issue)
  - Use `GOOGLE_CLI_KEYRING_MODE=file` for deterministic local fallback.
- Runtime refresh/exchange HTTP 400
  - Usually expired/reused code or invalid client config.
  - Re-run remote flow from step 1 and ensure credentials match your OAuth client.

## 8. Security notes

- Never commit `credentials.v1.json`, `accounts.v1.json`, or `tokens.v1.json`.
- Never share raw auth callback URLs publicly (they include temporary auth codes).

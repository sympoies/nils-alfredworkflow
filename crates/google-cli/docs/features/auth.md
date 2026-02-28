# Auth native contract

## Scope

- Repo-owned native scope:
  - `credentials <...>`
  - `add <email>`
  - `list`
  - `status`
  - `remove <email>`
  - `alias <...>`
  - `manage`
- Non-goals in this phase:
  - browser account-manager UI rebuild
  - non-scoped Google domains

## Native semantics

- `auth add` supports `loopback`, `manual`, and `remote` OAuth modes.
- Native state tracking must eliminate wrapper-era state mismatch issues.
- Account resolution order is: explicit `--account` -> alias -> configured default account -> single stored account ->
  deterministic error.
- `auth status` without `--account` must never emit an empty account payload.
- `auth manage` is terminal-native only and does not open an external account manager page.

## Native storage and prerequisites

- Credentials are stored under `GOOGLE_CLI_CONFIG_DIR` (or platform app config dir) in `credentials.v1.json`.
- Account/default/alias metadata is stored in `accounts.v1.json`.
- Tokens use system keyring by default, with deterministic file fallback support via `GOOGLE_CLI_KEYRING_MODE`.
- Browser auto-launch can be disabled with `GOOGLE_CLI_AUTH_DISABLE_BROWSER=1`.

## Manual smoke checklist

1. Configure credentials:
   `google-cli auth credentials set --client-id <id> --client-secret <secret>`
2. Add account with loopback mode:
   `google-cli auth add <email>`
3. Validate inventory + status:
   `google-cli auth list` then `google-cli auth status`
4. Verify account management stance:
   `google-cli auth manage` (summary-only; no browser manager page)
5. Optional remote flow:
   `google-cli auth add <email> --remote --step 1`
   then
   `google-cli auth add <email> --remote --step 2 --state <state> --code <code>`

## Validation

- `rg -n "native|default account|auth status|auth manage|loopback|manual|remote" docs/specs/google-cli-native-contract.md`
- `cargo test -p google-cli --test auth_storage`
- `cargo test -p google-cli --test auth_oauth_flow`
- `cargo test -p google-cli --test auth_account_resolution`
- `cargo test -p google-cli --test auth_cli_contract`

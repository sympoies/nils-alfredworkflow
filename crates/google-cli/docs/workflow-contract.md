# nils-google-cli workflow contract

> Status: active

## Scope

Per-subcommand JSON envelope, error-code, and exit-code contract for the `nils-google-cli` binary
(`google-cli`). Covers all four native sub-namespaces — `auth`, `gmail`, `drive`, and `calendar` — that back the
`google-service` Alfred workflow. The native command tree definition (clap clauses, subcommand semantics)
lives in [`docs/specs/google-cli-native-contract.md`](../../../docs/specs/google-cli-native-contract.md);
this document is the per-binary envelope and operator contract.

## Output mode flags

`google-cli` exposes JSON and plain-text output via two top-level flags applied to every subcommand:

- `-j` / `--json`: emit the shared CLI JSON envelope. Used by all script-adapter consumers in
  `workflows/google-service/scripts/`.
- `-p` / `--plain`: emit stable plain-text output. Intended for terminal use where the operator wants a
  scriptable, structured single-value result without the envelope wrapper.
- (No flag): default human-readable native text for direct terminal usage.

These flags are pre-subcommand: `google-cli --json auth status -a you@example.com` and
`google-cli auth status -a you@example.com --json` both work.

## Subcommand surface

Authoritative help: `cargo run -p nils-google-cli -- <namespace> <subcommand> --help`.

### `auth`

| Subcommand | Inputs | Behavior |
| --- | --- | --- |
| `auth credentials set` | `--client-id`, `--client-secret` | Persist OAuth client credentials. |
| `auth credentials list` | — | List configured credentials by alias. |
| `auth add <account>` | `--remote --step 1` then `--remote --step 2 --state ... --code ...`; or `--manual`; or default loopback | Authorize and persist a refresh token. Three modes per the native contract. |
| `auth list` | — | List stored accounts. |
| `auth status` | `-a <account>` (optional) | Show backend status for one account or the resolved default. |
| `auth remove <account>` | — | Remove a stored refresh token. |
| `auth alias` | get/set/clear forms | Manage account aliases. |
| `auth manage` | — | Terminal-native account management summary (no browser). |

### `gmail`

| Subcommand | Inputs | Behavior |
| --- | --- | --- |
| `gmail search` | `--query <gmail-query>` | Search threads using Gmail query syntax. |
| `gmail get` | message id | Fetch a message. |
| `gmail send` | message inputs | Send an email. |
| `gmail thread` | `get` / `modify` subforms | Thread-level operations. |

### `drive`

| Subcommand | Inputs | Behavior |
| --- | --- | --- |
| `drive ls` | folder id (optional) | List files in a folder. |
| `drive search` | `--query <drive-query>` | Full-text search across Drive. |
| `drive get` | file id | Fetch file metadata. |
| `drive download <target>` | file id / share link | Download a file. |
| `drive upload` | upload inputs | Upload a file. |

### `calendar`

| Subcommand | Inputs | Behavior |
| --- | --- | --- |
| `calendar calendars list` | `--max <n>` | List calendars visible to the account. |
| `calendar events list` | `--calendar-id`, `--from`, `--to`, `--query`, `--max`, `--private-property` | List events, expanded to single instances and ordered by start time. |
| `calendar events get` | event id, `--calendar-id` | Fetch one event. |
| `calendar events create` | `--calendar-id`, `--summary`, `--start`, `--end`, `--time-zone`, `--location`, `--description`, `--attendee`, `--private-property` | Create an all-day or timed event. |

## JSON envelope shape

Cross-references:

- Envelope shape: [`docs/specs/cli-json-envelope-v1.md`](../../../docs/specs/cli-json-envelope-v1.md)
- Error code registry: [`docs/specs/cli-error-code-registry.md`](../../../docs/specs/cli-error-code-registry.md)
- Native command contract: [`docs/specs/google-cli-native-contract.md`](../../../docs/specs/google-cli-native-contract.md)

Success envelope (single result branch):

```json
{
  "schema_version": "v1",
  "command": "google.auth.status",
  "ok": true,
  "result": { "account": "you@example.com", "backend": "keyring" }
}
```

Failure envelope:

```json
{
  "schema_version": "v1",
  "command": "google.gmail.search",
  "ok": false,
  "error": {
    "code": "NILS_GOOGLE_009",
    "message": "Gmail query is empty"
  }
}
```

Native command identifiers stay scoped to the namespace: `google.auth.<verb>`, `google.gmail.<verb>`,
`google.drive.<verb>`, `google.calendar.<group>.<verb>`.

## Reserved error codes

The reserved domain prefix is `NILS_GOOGLE_` (range `001-099`). Per the registry seeds:

- `NILS_GOOGLE_001` — invalid input / conflicting output flags.
- `NILS_GOOGLE_002`–`004` — reserved (legacy external-runtime codes, retained after native migration).
- `NILS_GOOGLE_005`–`008` — auth (invalid input, ambiguous account, store/runtime persistence, remote/manual
  state mismatch).
- `NILS_GOOGLE_009`–`011` — Gmail (invalid input, resource not found, runtime).
- `NILS_GOOGLE_012`–`014` — Drive (invalid input, resource not found, runtime).
- `NILS_GOOGLE_015`–`017` — Calendar (invalid input, resource not found, runtime).

Adding a new code requires a registry update in
[`cli-error-code-registry.md`](../../../docs/specs/cli-error-code-registry.md), a contract test update in
this crate, and a migration note in the PR summary.

## Account and default resolution

Per [`google-cli-native-contract.md`](../../../docs/specs/google-cli-native-contract.md), auth-adjacent
commands resolve the target account in this order:

1. explicit `--account` / `-a`
2. alias mapping
3. configured default account
4. single stored account when unambiguous
5. deterministic error otherwise (with corrective guidance)

`auth status` without `--account` applies the same order; it must never return an empty account payload
when multiple accounts exist without a configured default.

## Environment variables

- `GOOGLE_CLI_CONFIG_DIR`: override auth config directory (default `$HOME/.config/google/credentials`).
- `GOOGLE_CLI_KEYRING_MODE`: token storage mode (`keyring`, `file`, `fail`, `keyring-strict`).
- `GOOGLE_CLI_AUTH_DISABLE_BROWSER`: skip browser auto-launch for auth flows.
- `GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE`: test-only OAuth bypass switch. **Do not use in normal runs.**
- `GOOGLE_CLI_GMAIL_FIXTURE_PATH` / `GOOGLE_CLI_GMAIL_FIXTURE_JSON`: Gmail fixture JSON for local tests.
- `GOOGLE_CLI_DRIVE_FIXTURE_PATH` / `GOOGLE_CLI_DRIVE_FIXTURE_JSON`: Drive fixture JSON for local tests.

Workflow-side env vars surfaced by the `google-service` Alfred workflow (e.g.,
`GOOGLE_DRIVE_DOWNLOAD_DIR`, `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD`, `GOOGLE_AUTH_REMOVE_CONFIRM`) are read
by the workflow's adapter scripts and influence how `google-cli` is invoked; they do not appear in the CLI
itself.

## Exit code semantics

Aligned with the shared runtime contract:

- `0`: success.
- `1`: runtime / dependency / provider failure (e.g., transport error, persistence write failure).
- `2`: user / input / config failure (e.g., missing credentials, malformed query, unknown account).

## Validation

- `cargo run -p nils-google-cli -- --help`
- `cargo run -p nils-google-cli -- auth --help`
- `cargo run -p nils-google-cli -- gmail --help`
- `cargo run -p nils-google-cli -- drive --help`
- `cargo test -p nils-google-cli`
- `bash scripts/cli-standards-audit.sh`

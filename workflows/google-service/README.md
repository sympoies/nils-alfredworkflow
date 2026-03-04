# Google Service - Alfred Workflow

Manage Google auth accounts, Drive search/download, and Gmail search/list from Alfred using `google-cli` native commands.

## Screenshot

![Google Service workflow screenshot](./screenshot.png)

## Scope

Implemented now:

- `login` (remote step 1/2 and manual mode)
- `switch` (workflow-local active account)
- `remove` (with optional confirmation)
- `gs` optional all-accounts unread summary + per-account unread rows for accounts with unread mail (workflow toggle)
- `drive search` (keyword: `gsd`, Enter=download, Cmd+Enter=open Drive web search)
- `open Drive home` from `gsd`
- `gmail unread/latest/search` (keyword: `gsm`, Enter=open message, Cmd+Enter=open Gmail web search, optional explicit account for unread)
- Docs Editors files are auto-exported on download (`document -> docx`, `spreadsheet -> xlsx`, `presentation -> pptx`).

## Keywords

| Keyword | Behavior |
| --- | --- |
| `gs` | Show current account row (active account first, otherwise native default account). Optional all-accounts unread summary row and per-account unread rows (only for accounts with unread mail) are shown when `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD=1`. |
| `gsa` | Auth command menu with login/switch/remove rows, then account rows. |
| `gsd` | Drive home row + Drive search rows (Enter download, Cmd+Enter open Drive web search). |
| `gsm` | Gmail inbox home row + unread/latest/search rows (Enter open message, Cmd+Enter open Gmail web search). |

## Query examples

| Query | Result |
| --- | --- |
| `gs` | Show current account row. If `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD=1`, also show all-accounts unread summary row and one clickable unread row for each account with unread mail. |
| `gsa` | Show command rows (`Google Service Auth Login/Switch/Remove`) and account rows. |
| `gsa login you@example.com` | Run remote login step 1 (`auth add --remote --step 1`). |
| `gsa login <callback-url>` | Finish remote login step 2 (account auto-resolved from saved state). |
| `gsa login you@example.com http://localhost/?state=...&code=...` | Finish remote login step 2 by directly pasting callback URL. |
| `gsa login you@example.com --manual --code <code>` | Run manual login flow. |
| `gsa switch you@example.com` | Set workflow active account to selected account. |
| `gsa remove you@example.com` | Remove account (confirmation by default). |
| `gsa remove --yes you@example.com` | Remove account without workflow confirmation dialog. |
| `gsd` | Show `Open Google Drive Home` and search usage row. |
| `gsd open` | Open Google Drive home page in browser. |
| `gsd search keyboard` | Run `google-cli drive search keyboard`; Enter downloads selected file; Cmd+Enter opens Drive web search page. |
| `gsm` | Show `Open Gmail Inbox` and unread/latest/search usage rows. |
| `gsm unread` | Run `google-cli gmail search --query "in:inbox is:unread"` and list unread inbox messages. |
| `gsm unread --account you@example.com` | Run unread search with explicit account override (`google-cli -a you@example.com ...`) without changing workflow current account. |
| `gsm latest` | Run `google-cli gmail search --query "in:inbox"` and list latest inbox messages. |
| `gsm search keyboard` | Run `google-cli gmail search --query "keyboard"` and list matches. |

## Notifications

- Success notifications are shown for `login`, `switch`, `remove`, Drive download, and Gmail open actions.
- Failure notifications are also shown (for example invalid token/state, missing account, or CLI/auth errors).

## Active account model

- Native account/token source of truth remains `google-cli` auth storage.
- Workflow keeps an extra local pointer for active account switching:
  - path: `$ALFRED_WORKFLOW_DATA/active-account.v1.json`
- Rebalance behavior after remove:
  1. keep current active account if still present
  2. else use native `default_account`
  3. else use first account in `auth list`
  4. else clear active pointer
- `gsm unread --account <email>` and `gs` per-account unread rows only override account for that query; they do not mutate this active pointer.

## Runtime requirements

- `google-cli` resolution order:
  1. `GOOGLE_CLI_BIN` absolute path override
  2. bundled workflow runtime `bin/google-cli`
  3. local dev binaries (`target/release/google-cli`, `target/debug/google-cli`)
- `jq` is required for JSON parsing in script runtime.

For local development, build crate runtime:

```bash
cargo build -p nils-google-cli
```

## Configuration

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `GOOGLE_CLI_BIN` | No | empty | Optional absolute path override for `google-cli`. |
| `GOOGLE_CLI_CONFIG_DIR` | No | empty | Optional auth config root override. If empty and `~/.config/google/credentials` exists, workflow auto-uses that path. |
| `GOOGLE_CLI_KEYRING_MODE` | No | empty | Optional token backend mode (`keyring`, `file`, `fail`, `keyring-strict`). |
| `GOOGLE_DRIVE_DOWNLOAD_DIR` | No | `~/Downloads` | Optional download destination override for `gsd` download action. |
| `GOOGLE_MAIL_SEARCH_MAX` | No | `25` | Max rows for `gsm search` results (range `1..500`). |
| `GOOGLE_MAIL_LATEST_MAX` | No | `25` | Max rows for `gsm latest` and `gsm unread` results (range `1..500`). |
| `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD` | No | `0` | Show one extra unread summary row in `gs` root (`0=off`, `1=on`). |
| `GOOGLE_AUTH_REMOVE_CONFIRM` | No | `1` | Require confirmation dialog before remove when possible. |

## Validation

Run before packaging/release:

- `bash workflows/google-service/tests/smoke.sh`
- `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows google-service`
- `scripts/workflow-pack.sh --id google-service`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

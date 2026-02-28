# Gmail native contract

## Scope

- Repo-owned native scope:
  - `search <query...>`
  - `get <messageId>`
  - `send`
  - `thread <...>`
- Non-goals in this phase:
  - `labels`, `batch`, `drafts`, and settings automation beyond listed commands

## Native semantics

- Gmail command routing is native (`google.gmail.*`) and no longer shells out to `gog`.
- Native account/default resolution is shared with auth semantics.
- Response adapters map fixture/transport payloads into stable local JSON/plain output.
- `gmail send` uses native MIME composition and attachment type inference.
- Output/error envelopes stay native and repo-standard.
- Thread label mutation supports add/remove semantics over thread-scoped messages.

## Validation

- `cargo test -p google-cli --test gmail_read`
- `cargo test -p google-cli --test gmail_thread`
- `cargo test -p google-cli --test gmail_send`
- `cargo test -p google-cli --test gmail_cli_contract`
- `cargo test -p google-cli --test account_resolution_shared`
- `cargo run -p google-cli -- gmail search --help`
- `cargo run -p google-cli -- gmail thread --help`

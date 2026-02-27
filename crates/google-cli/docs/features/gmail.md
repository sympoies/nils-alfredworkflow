# Gmail wrapper

## Scope

- Supported wrapper surface in this phase:
  - `search <query...>`
  - `get <messageId>`
  - `send`
  - `thread <...>`
- Non-goals for this phase:
  - `messages`
  - `attachment`
  - `labels`
  - `batch`
  - `drafts`
  - `settings`
  - `track`

## Command contract

- Gmail commands reuse the shared wrapper pass-through policy for global `--account`, `--client`, `--json`, `--plain`, `--results-only`, `--select`, and runtime flags.
- Output mode expectations:
  - `--json`: wrapper expects upstream Gmail JSON and emits wrapper envelope `v1`.
  - `--plain`: wrapper forwards upstream parseable text output unchanged.
  - default: wrapper forwards human-readable upstream output unchanged.
- Representative pass-through flags:
  - `gmail search --max --page --all --fail-empty --oldest --timezone`
  - `gmail get --format --headers`
  - `gmail send --to --cc --bcc --subject --body --body-file --attach --thread-id --reply-all`
  - `gmail thread get|modify ...`

## Usage examples

- `cargo run -p google-cli -- gmail search "label:inbox newer_than:7d" --max 10`
- `cargo run -p google-cli -- gmail get 190abc --format metadata --headers Subject,From`
- `cargo run -p google-cli -- gmail send --to team@example.com --subject Status --body Wrapped`
- `cargo run -p google-cli -- gmail thread get 18ff-thread --format metadata`

## Validation

- `cargo run -p google-cli -- gmail --help`
- `cargo run -p google-cli -- gmail search --help`
- `cargo test -p google-cli --test gmail_cli_contract`

## Troubleshooting

- Wrapper/runtime failure: verify the equivalent `gog gmail ...` command directly.
- `NILS_GOOGLE_003`: `gog` exited non-zero; inspect the first stderr line and retry with `--verbose` when safe.
- `NILS_GOOGLE_004`: `gog` produced invalid JSON while wrapper `--json` mode was active.

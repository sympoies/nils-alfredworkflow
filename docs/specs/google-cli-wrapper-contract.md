# Google CLI wrapper contract

## Wrapper boundary

- Crate: `crates/google-cli`
- Role: `gog` wrapper focused on `auth`, `gmail`, and `drive`
- Scope in this phase:
  - `auth`: `credentials`, `add`, `list`, `status`, `remove`, `alias`, `manage`
  - `gmail`: `search`, `get`, `send`, `thread`
  - `drive`: `ls`, `search`, `get`, `download`, `upload`
- Non-goals in this phase:
  - Alfred workflow integration
  - workflow packaging
  - other `gog` domains (`calendar`, `chat`, `forms`, `people`, etc.)
  - modeling every upstream flag as native clap options

## Command naming strategy

- Wrapper-owned success/error `command` identifiers use stable dotted names:
  - `google.auth.add`
  - `google.auth.alias`
  - `google.gmail.search`
  - `google.gmail.thread`
  - `google.drive.ls`
  - `google.drive.download`
- Nested pass-through wrappers may append the first nested token when it is stable enough for test/report visibility (for example `google.auth.alias.set`).

## Global flag mapping

| Wrapper flag | Wrapped `gog` behavior |
| --- | --- |
| `--account` | Forwarded unchanged ahead of the selected `gog` subcommand. |
| `--client` | Forwarded unchanged ahead of the selected `gog` subcommand. |
| `--json` | Forwarded to `gog`; wrapper validates upstream JSON and re-wraps it in the local JSON envelope. |
| `--plain` | Forwarded to `gog`; wrapper returns stable, parseable text unchanged. |
| `--results-only` | Forwarded only when `--json` is active; rejected otherwise as user input. |
| `--select` | Forwarded only when `--json` is active; rejected otherwise as user input. |
| `--dry-run`, `--force`, `--no-input`, `--verbose`, `--color`, `--enable-commands` | Forwarded unchanged. |

## Pass-through policy

- Wrapper-owned clap models only the scoped command groups and their primary required positional arguments.
- Command-specific option flags remain pass-through arguments after the scoped subcommand.
- Representative pass-through examples:
  - `auth add --manual --remote --services --readonly --drive-scope`
  - `gmail search --max --page --timezone`
  - `gmail send --to --subject --body --attach`
  - `drive ls --parent --query`
  - `drive upload --parent --name --replace --convert-to`
- This pass-through policy keeps wrapper churn low when `gog` v0.11.x adds or adjusts scoped flags.

## Runtime resolution

- The wrapper resolves `gog` in this order:
  1. explicit `GOOGLE_CLI_GOG_BIN` override
  2. regular `PATH` lookup
- Missing `gog` is a first-class runtime error with a stable wrapper code and searched-path details.

## Output behavior

- Default mode: human-readable passthrough from upstream `gog`.
- `--plain`: stable text passthrough from upstream `gog`.
- `--json`: JSON-first wrapper behavior:
  - upstream `gog --json` must emit valid JSON
  - wrapper emits envelope keys `schema_version`, `command`, `ok`, and `result` or `error`
  - invalid upstream JSON becomes wrapper error `NILS_GOOGLE_004`

## Error taxonomy

| Category | Wrapper code | Meaning |
| --- | --- | --- |
| user input | `NILS_GOOGLE_001` | invalid wrapper-owned input such as conflicting output flags |
| missing `gog` | `NILS_GOOGLE_002` | `gog` binary could not be resolved |
| process failure | `NILS_GOOGLE_003` | `gog` launched but exited non-zero or could not be launched |
| invalid output decoding | `NILS_GOOGLE_004` | `gog --json` returned invalid JSON |

## Compatibility note

- The wrapper is pinned to observed `gog` v0.11.0 / v0.11.x command and flag behavior for `auth`, `gmail`, and `drive`.
- If upstream `gog` changes command names or option semantics, update this contract and the fake-`gog` tests in the same change set.

# nils-steam-cli

CLI backend for the `steam-search` workflow.

## Commands

| Command | Options | Description |
| --- | --- | --- |
| `steam-cli search` | `--query <QUERY>`, `--mode <alfred\|service-json>` | Search Steam apps and print Alfred Script Filter JSON or service envelope JSON. |

## Environment Variables

- Optional: `STEAM_REGION` (default: `us`)
- Optional: `STEAM_REGION_OPTIONS` (default: current `STEAM_REGION`)
- Optional: `STEAM_SHOW_REGION_OPTIONS` (default: `false`, accepts `1/0`, `true/false`, `yes/no`, `on/off`)
- Optional: `STEAM_MAX_RESULTS` (default: `10`, clamped to `1..50`)
- Optional: `STEAM_LANGUAGE` (default: unset/empty; when set, includes `l` query parameter)
- Optional test override: `STEAM_STORE_SEARCH_ENDPOINT`

## Output Contract

- `stdout`: Alfred Script Filter JSON payload (default mode) or JSON service envelope (`--mode service-json`).
- `stderr`: deterministic user/runtime error text in Alfred mode.
- Exit codes: `0` success, `1` runtime/API error, `2` user/config/input error.

## Region Requery Contract

- When `STEAM_SHOW_REGION_OPTIONS=true`, first row is `Current region: <REGION>`.
- When `STEAM_SHOW_REGION_OPTIONS=true`, region switch rows follow `STEAM_REGION_OPTIONS` order exactly.
- Switch row `arg` format is `steam-requery:<region>:<query>`.
- Result rows always include `cc=<region>`; include `l=<language>` only when `STEAM_LANGUAGE` is configured.

## Standards Status

- README/command docs: compliant.
- JSON service envelope (`schema_version/command/ok`): implemented.
- Default human-readable mode: not implemented (workflow JSON-first contract).

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/workflow-contract.md`](docs/workflow-contract.md)

## Validation

- `cargo run -p nils-steam-cli -- --help`
- `cargo run -p nils-steam-cli -- search --help`
- `cargo test -p nils-steam-cli`

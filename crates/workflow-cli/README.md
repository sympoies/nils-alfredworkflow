# nils-workflow-cli

Shared CLI for open-project workflow actions and script-filter rendering.

## Commands

- `workflow-cli script-filter`
  - Options: `--query <QUERY> [--mode <open|github>] [--output <human|json|alfred-json>]`
  - Description: Render script-filter results in Alfred JSON (default), human lines, or JSON envelope mode.
- `workflow-cli record-usage`
  - Options: `--path <PATH>`
  - Description: Record usage timestamp for a selected project path.
- `workflow-cli github-url`
  - Options: `--path <PATH>`
  - Description: Resolve project origin URL to canonical GitHub URL.

## Environment Variables

Configured via `workflow-common` runtime config:

- `PROJECT_DIRS`, `USAGE_FILE`, `VSCODE_PATH`, `OPEN_PROJECT_MAX_RESULTS`

## Output Contract

- `script-filter`:
  - `--output alfred-json` (default): Alfred Script Filter JSON on `stdout`.
  - `--output human`: newline-delimited item summary lines on `stdout`.
  - `--output json`: service envelope JSON (`schema_version/command/ok`) on `stdout`.
- `record-usage` / `github-url`: plain text value on `stdout`.
- `stderr`: user/runtime error text for human mode.
- Exit codes: `0` success, `1` runtime error, `2` user/input error.

## Standards Status

- README/command docs: compliant.
- Human-readable mode: compliant (`script-filter --output human` plus plain text for non-script-filter commands).
- JSON service envelope (`schema_version/command/ok`): compliant for `script-filter` in JSON output mode.

## Contract References

- Shared runtime contract: [`docs/specs/cli-shared-runtime-contract.md`](../../docs/specs/cli-shared-runtime-contract.md)
- Compatibility debt matrix: [`docs/reports/crate-legacy-removal-matrix.md`](../../docs/reports/crate-legacy-removal-matrix.md)

## Documentation

- [`docs/README.md`](docs/README.md)
- [`Open Project Port Parity contract`](../../crates/workflow-cli/docs/README.md#canonical-documents)

## Validation

- `cargo run -p nils-workflow-cli -- --help`
- `cargo run -p nils-workflow-cli -- script-filter --help`
- `cargo run -p nils-workflow-cli -- record-usage --help`
- `cargo run -p nils-workflow-cli -- github-url --help`
- `cargo test -p nils-workflow-cli`

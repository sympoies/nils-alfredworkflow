# Drive wrapper

## Scope

- Supported wrapper surface in this phase:
  - `ls`
  - `search <query...>`
  - `get <fileId>`
  - `download <fileId>`
  - `upload <localPath>`
- Non-goals for this phase:
  - `copy`
  - `mkdir`
  - `delete`
  - `move`
  - `rename`
  - `share`
  - `permissions`
  - `comments`
  - `drives`

## Command contract

- Drive commands follow the shared wrapper pass-through policy for global flags and runtime execution.
- Output handling:
  - `--json`: validate upstream JSON and wrap it in the local success/error envelope.
  - `--plain`: forward upstream stable text output unchanged.
  - default: forward upstream human-readable output.
- Representative pass-through flags:
  - `drive ls --parent --query --max --page --no-all-drives`
  - `drive search --raw-query --max --page --no-all-drives`
  - `drive download --out --format`
  - `drive upload --parent --name --replace --mime-type --convert --convert-to`

## Usage examples

- `cargo run -p google-cli -- drive ls --parent root --query "mimeType='application/pdf'"`
- `cargo run -p google-cli -- drive search report --max 10`
- `cargo run -p google-cli -- drive get 1AbCdE`
- `cargo run -p google-cli -- drive download 1AbCdE --out /tmp/report.pdf --format pdf`
- `cargo run -p google-cli -- drive upload ./report.pdf --parent folder-1 --name report.pdf`

## Validation

- `cargo run -p google-cli -- drive --help`
- `cargo run -p google-cli -- drive ls --help`
- `cargo test -p google-cli --test drive_cli_contract`

## Troubleshooting

- Missing runtime binary: set `GOOGLE_CLI_GOG_BIN` or install `gog`.
- `NILS_GOOGLE_003`: upstream process exited non-zero; inspect `gog drive ...` directly with the same arguments.
- `NILS_GOOGLE_004`: upstream emitted invalid JSON under wrapper `--json` mode.

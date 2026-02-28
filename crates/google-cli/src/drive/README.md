# google-cli drive module

Authoritative Drive documentation for `google-cli`.

## Scope

- `drive ls`
- `drive search <query...>`
- `drive get <fileId>`
- `drive download <fileId>`
- `drive upload <localPath>`

## Runtime model

- Default path calls live Drive API with OAuth bearer token from auth module.
- Fixture mode is enabled only when one of these env vars is set:
  - `GOOGLE_CLI_DRIVE_FIXTURE_PATH`
  - `GOOGLE_CLI_DRIVE_FIXTURE_JSON`

## Command notes

List:

```bash
cargo run -p google-cli -- --json -a you@example.com drive ls --max 10
```

Search:

```bash
cargo run -p google-cli -- --json -a you@example.com drive search "name:report" --max 5
```

Get metadata:

```bash
cargo run -p google-cli -- --json -a you@example.com drive get <file_id>
```

Download:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  drive download <file_id> --out ./downloads/file.bin --overwrite
```

Export during download:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  drive download <file_id> --format pdf --out ./downloads/file.pdf --overwrite
```

Upload:

```bash
cargo run -p google-cli -- --json -a you@example.com \
  drive upload ./report.pdf --name report.pdf --parent <folder_id>
```

## Upload behavior

- MIME type is inferred by default; `--mime` can override.
- `--replace` updates an existing same-name file in the target parent when found.
- `--convert` requests conversion to Google Docs/Sheets/Slides where supported.

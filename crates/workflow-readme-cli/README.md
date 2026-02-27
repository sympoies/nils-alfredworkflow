# nils-workflow-readme-cli

Convert a workflow `README.md` into Alfred-compatible readme markdown and inject it into plist `readme`.

## Usage

```bash
cargo run -p nils-workflow-readme-cli -- convert \
  --workflow-root workflows/codex-cli \
  --readme-source README.md \
  --stage-dir build/workflows/codex-cli/pkg \
  --plist build/workflows/codex-cli/pkg/info.plist
```

Dry-run mode (no file writes):

```bash
cargo run -p nils-workflow-readme-cli -- convert \
  --workflow-root workflows/codex-cli \
  --readme-source README.md \
  --stage-dir build/workflows/codex-cli/pkg \
  --plist build/workflows/codex-cli/pkg/info.plist \
  --dry-run
```

JSON output mode:

```bash
cargo run -p nils-workflow-readme-cli -- convert \
  --workflow-root workflows/codex-cli \
  --readme-source README.md \
  --stage-dir build/workflows/codex-cli/pkg \
  --plist build/workflows/codex-cli/pkg/info.plist \
  --output json
```

Behavior summary:

- Reads full README content.
- Downgrades markdown tables to deterministic bullet rows (no raw `|---|` table separators).
- Detects markdown image references and copies local assets into `--stage-dir` with the same relative path.
- Rejects remote image URLs.
- Injects converted markdown into `<key>readme</key><string>...</string>` with XML-safe escaping.

## Exit codes

- `0`: success
- `1`: runtime failure (I/O, file write/copy/create-dir errors)
- `2`: user/input failure (invalid paths, missing README/image/plist, malformed markdown image syntax, remote image URL,
  missing plist readme key)

## Output Contract

- `stdout`: single-line progress summary (`converted ... bytes, copied ... local image asset(s)`).
- `stderr`: `error[<code>]: <message>` on failure.
- Exit code mapping:
  - `0` success
  - `1` runtime failure
  - `2` user/input failure

## Troubleshooting

- `error[user.readme_not_found]`: ensure `--readme-source` exists under `--workflow-root`.
- `error[user.remote_image_not_allowed]`: replace image URL with a local relative path and include the file in the
  workflow root.
- `error[user.plist_readme_key_missing]`: ensure plist contains `<key>readme</key>` followed by a
  `<string>...</string>`.
- `error[runtime.copy_failed]`: verify write permissions for `--stage-dir`.

## Standards Status

- README/command docs: compliant.
- Human-readable mode: compliant.
- JSON service envelope (`schema_version/command/ok`): intentionally not implemented for this utility command.

## Validation

- `cargo test -p nils-workflow-readme-cli`
- `cargo run -p nils-workflow-readme-cli -- --help`

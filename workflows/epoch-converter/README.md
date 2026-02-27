# Epoch Converter - Alfred Workflow

Convert between epoch timestamps and datetime values from Alfred, then copy any selected value to clipboard.

## Screenshot

![Epoch Converter workflow screenshot](./screenshot.png)

## Features

- Trigger conversion with `epoch <value>` (or `ts <value>`).
- Supports `epoch-cli` output directly (Alfred JSON passthrough on success).
- Empty query runs current timestamp + clipboard-assist mode from `epoch-cli`.
- Epoch input includes an extra formatted date row: `Local formatted (YYYY-MM-DD HH:MM:SS)`.
- Always degrades to valid Alfred fallback JSON for invalid input, missing binary, and runtime errors.
- Press `Enter` on any row to copy the selected result via `pbcopy`.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable        | Required | Default | Description                                                                   |
| --------------- | -------- | ------- | ----------------------------------------------------------------------------- |
| `EPOCH_CLI_BIN` | No       | (empty) | Optional absolute path override for `epoch-cli` (useful for local debugging). |

## Keyword

| Keyword                                                 | Behavior                                                                        |
| ------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `epoch <epoch-or-datetime>` or `ts <epoch-or-datetime>` | Convert input via `epoch-cli` and show copyable result rows.                    |
| `epoch` or `ts`                                         | Show current epoch rows and best-effort conversion rows from clipboard content. |

## Output Rows

- Epoch input rows:
  - `Local ISO-like`
  - `UTC ISO-like`
  - `Local formatted (YYYY-MM-DD HH:MM:SS)` (additional formatted-date row)
- Datetime input rows:
  - `Local epoch (s|ms|us|ns)`
  - `UTC epoch (s|ms|us|ns)`
- Empty query rows:
  - `Now epoch (s|ms|us|ns)`
  - Optional `(clipboard) ...` rows when clipboard text is parseable.

## Validation

- `bash workflows/epoch-converter/tests/smoke.sh`
- `scripts/workflow-test.sh --id epoch-converter`
- `scripts/workflow-pack.sh --id epoch-converter`

## Runtime Binary Resolution

`scripts/script_filter.sh` resolves `epoch-cli` in this order:

1. `EPOCH_CLI_BIN` (if executable)
2. Packaged binary: `./bin/epoch-cli`
3. Repository release binary: `target/release/epoch-cli`
4. Repository debug binary: `target/debug/epoch-cli`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

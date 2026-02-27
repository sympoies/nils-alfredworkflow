# Quote Feed - Alfred Workflow

Show cached quotes in Alfred and copy one quickly, while refreshing from ZenQuotes on a configurable interval.

## Screenshot

![Quote Feed workflow screenshot](./screenshot.png)

## Features

- Trigger quote list with `qq` (optional query filter supported).
- Local-first output: cached quotes are shown immediately.
- Automatic refresh from ZenQuotes when interval is due.
- Long quotes are split across title/subtitle lines to improve readability.
- Enter on a row copies the full quote text via `pbcopy`.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable                 | Required | Default   | Description                                                                                                                               |
| ------------------------ | -------- | --------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `QUOTE_DISPLAY_COUNT`    | No       | `3`       | Number of rows shown each run. Parsed as base-10 integer and clamped to `1..20`.                                                          |
| `QUOTE_REFRESH_INTERVAL` | No       | `1h`      | Refresh interval format: `<positive-int><s\|m\|h>`.                                                                                       |
| `QUOTE_FETCH_COUNT`      | No       | `5`       | Quotes fetched when refresh is due. Parsed as base-10 integer and clamped to `1..20`.                                                     |
| `QUOTE_MAX_ENTRIES`      | No       | `100`     | Max retained cached quotes. Parsed as base-10 integer and clamped to `1..1000`.                                                           |
| `QUOTE_DATA_DIR`         | No       | `(empty)` | Override quote cache directory. If empty, uses Alfred workflow data dir; if unavailable, falls back to `${TMPDIR:-/tmp}/nils-quote-feed`. |

## Keyword

| Keyword     | Behavior                                            |
| ----------- | --------------------------------------------------- |
| `qq`        | Show up to `QUOTE_DISPLAY_COUNT` cached quotes.     |
| `qq <text>` | Filter cached quotes by case-insensitive substring. |

## Advanced Runtime Parameters

| Parameter       | Description                                                          |
| --------------- | -------------------------------------------------------------------- |
| `QUOTE_CLI_BIN` | Optional executable path override for `quote-cli` (local debugging). |

## Validation

- `bash workflows/quote-feed/tests/smoke.sh`
- `scripts/workflow-test.sh --id quote-feed`
- `scripts/workflow-pack.sh --id quote-feed`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

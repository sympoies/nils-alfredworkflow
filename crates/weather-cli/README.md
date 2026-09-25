# nils-weather-cli

No-token weather CLI for the Alfred weather workflow.

## Commands

- `weather-cli today`
  - Location input: repeated `--city <CITY>` for single-city or batch city mode,
    or `--lat <LAT> --lon <LON>`
  - Output: `--output <human|json|alfred-json>` or `--json`
  - Labels: `--lang <en|zh>`
  - Description: current-day daily forecast
- `weather-cli week`
  - Location input: repeated `--city <CITY>` for single-city or batch city mode,
    or `--lat <LAT> --lon <LON>`
  - Output: `--output <human|json|alfred-json>` or `--json`
  - Labels: `--lang <en|zh>`
  - Description: 7-day daily forecast
- `weather-cli hourly`
  - Location input: single `--city <CITY>` or `--lat <LAT> --lon <LON>`
  - Output: `--output <human|json|alfred-json>` or `--json`
  - Labels: `--lang <en|zh>`
  - Extras: `--hours <1..48>`
  - Description: hourly forecast from the current local hour (24h default)
- `weather-cli default-locations`
  - Input: `--fallback <LIST>` (comma/newline, e.g. `WEATHER_DEFAULT_CITIES`) and optional
    `--preference-projection-file <PATH>`
  - Output: `--output <human|json|alfred-json>` (human prints one location per line)
  - Description: resolve empty-query default locations; projection labels are never comma-split
- `weather-cli preference-status`
  - Input: optional `--preference-projection-file <PATH>`
  - Output: `--output <human|json|alfred-json>` (Alfred JSON default)
  - Description: non-selectable external preference projection status row; see
    [`preference-projection-contract.md`](../workflow-common/docs/preference-projection-contract.md)

## Batch Behavior

- Repeating `--city` is supported on `today` and `week` only.
- Batch city input is trimmed and deduped case-insensitively while preserving
  first-seen order. After geocoding, a city whose coordinates (rounded to four
  decimals) match an earlier city is dropped, so one place is listed once.
- Batch mode reuses persistent geocoding cache, resolves cache misses in
  parallel, and issues one Open-Meteo daily batch request once all coordinates
  are known.
- If the batch primary request fails, fallback remains per city through
  MET Norway, with stale weather cache reuse preserved per city.
- `hourly` remains a single-location command.

## Environment Variables

- Optional cache root override: `WEATHER_CACHE_DIR`
- Optional cache TTL override in seconds: `WEATHER_CACHE_TTL_SECS` (default: `1800`)
- Alfred fallback cache roots: `ALFRED_WORKFLOW_CACHE`, `ALFRED_WORKFLOW_DATA`

## Output Contract

- Default mode is human-readable text.
- `--output json` and `--json` return the shared runtime success envelope:
  - Success: `{ "schema_version": "v1", "command": "...", "ok": true, "result": ... }`
  - Error: `{ "schema_version": "v1", "command": "...", "ok": false, "error": ... }`
- Single-city `result` payloads use the daily or hourly forecast object shape.
- Repeated `--city` on `today` / `week` returns a batch `result` payload with
  `entries[]`, where each entry contains either `result` or `error`.
- `--output alfred-json` returns Alfred Script Filter JSON:
  - Single-city daily and hourly outputs include a header item followed by
    forecast rows.
  - Batch daily outputs are already flattened into forecast rows without header items.
- `--lang` only affects human-readable and Alfred labels; machine JSON fields stay stable.
- Exit codes: `0` success, `1` runtime/provider error, `2` user/input error.

## Provider Stack

- Open-Meteo primary
- MET Norway fallback
- Freshness states: `live`, `cache_fresh`, `cache_stale_fallback`
- Geocoding cache is stored separately under `<cache>/weather-cli/geocode/*.json`

## Standards Status

- README/command docs: compliant.
- Human-readable default plus explicit `json` / `alfred-json` modes: compliant.
- JSON success and error envelopes: compliant.

## References

- Shared runtime contract: [`docs/specs/cli-shared-runtime-contract.md`](../../docs/specs/cli-shared-runtime-contract.md)
- Crate docs index: [`docs/README.md`](docs/README.md)
- Weather workflow contract: [`docs/workflow-contract.md`](docs/workflow-contract.md)
- Compliance gate: `scripts/cli-standards-audit.sh`

## Validation

- `cargo run -p nils-weather-cli -- --help`
- `cargo run -p nils-weather-cli -- today --help`
- `cargo run -p nils-weather-cli -- week --help`
- `cargo run -p nils-weather-cli -- hourly --help`
- `cargo test -p nils-weather-cli`

# nils-weather-cli

CLI backend for one-day and seven-day weather forecast retrieval.

## Commands

- `weather-cli today`
  - Options: `--city <CITY>` or `--lat <LAT> --lon <LON>` `[--output <human|json|alfred-json>]`
    `[--lang <en|zh>]`
  - Description: Today weather forecast.
- `weather-cli week`
  - Options: `--city <CITY>` or `--lat <LAT> --lon <LON>` `[--output <human|json|alfred-json>]`
    `[--lang <en|zh>]`
  - Description: 7-day weather forecast.
- `weather-cli hourly`
  - Options: `--city <CITY>` or `--lat <LAT> --lon <LON>` `[--output <human|json|alfred-json>]`
    `[--lang <en|zh>]` `[--hours <1..48>]`
  - Description: Hourly weather forecast from current hour (24h default).

## Environment Variables

- Optional cache override: `WEATHER_CACHE_DIR`
- Optional cache TTL override (seconds): `WEATHER_CACHE_TTL_SECS` (default: `1800`)
- Alfred fallback cache paths: `ALFRED_WORKFLOW_CACHE`, `ALFRED_WORKFLOW_DATA`

## Output Contract

- Default mode: human-readable text summary.
- JSON mode: `--output json` returns structured forecast object.
- Language mode: `--lang` controls text/Alfred labels (`en` default, `zh` optional).
- `stderr`: user/runtime error text.
- Exit codes: `0` success, `1` runtime/provider error, `2` user/input error.

### Provider stack (no token)

- Open-Meteo primary
- MET Norway fallback
- Freshness states: `live`, `cache_fresh`, `cache_stale_fallback`

## Standards Status

- README/command docs: compliant.
- Human-readable default + explicit JSON mode: compliant.
- JSON service envelope (`schema_version/command/ok`): not yet migrated.

## Contract References

- Shared runtime contract: [`docs/specs/cli-shared-runtime-contract.md`](../../docs/specs/cli-shared-runtime-contract.md)
- Compatibility debt matrix: [`docs/reports/crate-legacy-removal-matrix.md`](../../docs/reports/crate-legacy-removal-matrix.md)

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/workflow-contract.md`](docs/workflow-contract.md)

## Validation

- `cargo run -p nils-weather-cli -- --help`
- `cargo run -p nils-weather-cli -- today --help`
- `cargo run -p nils-weather-cli -- week --help`
- `cargo run -p nils-weather-cli -- hourly --help`
- `cargo test -p nils-weather-cli`

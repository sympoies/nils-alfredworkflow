# weather-cli contract

## Goal

Provide token-free weather forecast data for current day, 7-day horizon, and hourly forecast.
Primary source is Open-Meteo, with MET Norway as fallback where supported.

## Commands

- `weather-cli today --city <name> [--city <name> ...] [--output <human|json|alfred-json> | --json] [--lang <en|zh>]`
- `weather-cli today --lat <f64> --lon <f64> [--output <human|json|alfred-json> | --json] [--lang <en|zh>]`
- `weather-cli week --city <name> [--city <name> ...] [--output <human|json|alfred-json> | --json] [--lang <en|zh>]`
- `weather-cli week --lat <f64> --lon <f64> [--output <human|json|alfred-json> | --json] [--lang <en|zh>]`
- `weather-cli hourly --city <name> [--output <human|json|alfred-json> | --json] [--lang <en|zh>] [--hours <1..48>]`
- `weather-cli hourly --lat <f64> --lon <f64> [--output <human|json|alfred-json> | --json] [--lang <en|zh>] [--hours <1..48>]`

Location input rules:

- Use either repeated `--city` OR `--lat/--lon`.
- `--lat` and `--lon` must be provided together.
- `--city` cannot be empty.
- Repeating `--city` enables multi-city batch mode for `today` and `week` only.
- Repeated `--city` input is trimmed, deduped case-insensitively, and preserves first-seen order.
- `hourly` supports only a single city or one coordinate pair.
- `--lang` affects human-readable and Alfred labels only; machine JSON fields stay stable.
- `--json` is shorthand for JSON envelope output and conflicts with an explicit non-JSON `--output`.
- `hourly` output starts from the current local hour.

## JSON Mode Envelope

Successful JSON output uses the shared runtime envelope:

```json
{
  "schema_version": "v1",
  "command": "weather.today",
  "ok": true,
  "result": {}
}
```

Error JSON output uses:

```json
{
  "schema_version": "v1",
  "command": "weather.today",
  "ok": false,
  "error": {
    "code": "user.invalid_input",
    "message": "missing location input: use --city or --lat/--lon",
    "details": {}
  }
}
```

The `result` payload shape depends on the command.

### Single-city daily result (`today` / `week`)

```json
{
  "period": "today|week",
  "location": {
    "name": "Taipei City",
    "latitude": 25.0531,
    "longitude": 121.5264
  },
  "timezone": "Asia/Taipei",
  "forecast": [
    {
      "date": "2026-02-11",
      "weather_code": 3,
      "summary_zh": "陰天",
      "temp_min_c": 14.5,
      "temp_max_c": 19.9,
      "precip_prob_max_pct": 13
    }
  ],
  "source": "open_meteo|met_no",
  "source_trace": ["open_meteo: transport error: timeout"],
  "fetched_at": "2026-02-11T03:30:00Z",
  "freshness": {
    "status": "live|cache_fresh|cache_stale_fallback",
    "key": "today-taipei-city-25.0531-121.5264",
    "ttl_secs": 1800,
    "age_secs": 0
  }
}
```

### Multi-city daily result (`today` / `week` with repeated `--city`)

```json
{
  "period": "today|week",
  "entries": [
    {
      "city": "Taipei",
      "result": {
        "period": "today",
        "location": {
          "name": "Taipei",
          "latitude": 25.033,
          "longitude": 121.5654
        },
        "timezone": "Asia/Taipei",
        "forecast": [
          {
            "date": "2026-02-11",
            "weather_code": 3,
            "summary_zh": "陰天",
            "temp_min_c": 14.5,
            "temp_max_c": 19.9,
            "precip_prob_max_pct": 13
          }
        ],
        "source": "open_meteo",
        "source_trace": [],
        "fetched_at": "2026-02-11T03:30:00Z",
        "freshness": {
          "status": "live|cache_fresh|cache_stale_fallback",
          "key": "today-taipei-25.0330-121.5654",
          "ttl_secs": 1800,
          "age_secs": 0
        }
      }
    },
    {
      "city": "Tokyo",
      "error": "failed to resolve city 'Tokyo': open_meteo: ..."
    }
  ]
}
```

### Hourly result (`hourly`)

```json
{
  "location": {
    "name": "Tokyo",
    "latitude": 35.6762,
    "longitude": 139.6503
  },
  "timezone": "Asia/Tokyo",
  "hourly": [
    {
      "datetime": "2026-02-12T00:00",
      "weather_code": 3,
      "temp_c": 1.2,
      "precip_prob_pct": 10
    }
  ],
  "source": "open_meteo",
  "source_trace": [],
  "fetched_at": "2026-02-12T00:00:00Z",
  "freshness": {
    "status": "live|cache_fresh|cache_stale_fallback",
    "key": "hourly-city-tokyo",
    "ttl_secs": 1800,
    "age_secs": 0
  }
}
```

## Alfred JSON Notes

- `--output alfred-json` returns Alfred Script Filter JSON on `stdout`.
- Single-city `today`, `week`, and `hourly` outputs include a header item with
  `weather_meta.item_kind="header"`, followed by forecast rows.
- Batch `today` / `week` outputs are already flattened into forecast rows and do
  not include a header item.
- Forecast rows carry `weather_meta` fields such as `item_kind`, `summary`,
  `weather_code`, `icon_key`, `is_night`, timezone, coordinate labels, plus
  locale-aware weekday metadata (`weekday_label`, `date_with_weekday`) for
  workflow display and timezone display metadata (`utc_offset_label`,
  `timezone_display`).
- Icon selection is Rust-owned and based on `weather_code` plus local time where
  applicable; workflow shell should not infer icons from summary strings.

## Provider Policy

- No token is required for all command paths.
- Forecast order:
  1. Open-Meteo primary
  2. MET Norway fallback
- Multi-city daily mode resolves uncached geocoding misses in parallel and uses
  one Open-Meteo batch forecast request once all target coordinates are known.
- If the Open-Meteo batch request fails, fallback remains per city through MET Norway.
- Hourly currently uses Open-Meteo only, with stale cache fallback on upstream error.
- If both providers fail and stale weather cache exists, return stale cache with `freshness.status=cache_stale_fallback`.
- If all providers fail and no usable cache exists, command exits with runtime error.

## Cache Policy

- Default weather TTL is 30 minutes (`1800` seconds).
- TTL can be overridden by `WEATHER_CACHE_TTL_SECS`.
- The Alfred weather workflow sets `WEATHER_CACHE_TTL_SECS=900` by default.
- Weather cache keys include period plus normalized location identity.
- Corrupt weather cache payload is treated as cache miss.
- Geocoding cache is stored separately under `<cache>/weather-cli/geocode/*.json`.
- Geocoding cache has no TTL and is treated as a persistent city-to-location mapping unless the file is removed.

## Exit Codes

- `0`: success
- `1`: runtime/provider failure
- `2`: user input validation failure

## No-token Statement

This CLI intentionally uses free and no-token endpoints only:

- Open-Meteo geocoding + forecast API
- MET Norway Locationforecast API

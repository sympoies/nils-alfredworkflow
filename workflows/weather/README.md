# Weather Forecast - Alfred Workflow

Show weather forecasts from `weather-cli` with separate keywords for today and week views.

## Screenshot

![Weather Forecast workflow screenshot](./screenshot.png)

## Keywords

| Keyword | Behavior |
| --- | --- |
| `wt` | Show today rows first, then select a row to open hourly forecast. |
| `ww` | Pick city, then show 7-day forecast. |

## Query Format

- City name: `wt Taipei`
- Multi-city by comma: `wt Tokyo,Osaka,Taipei`
- Coordinates: `wt 25.03,121.56`
- Today view is two-stage:
  1. `wt <query>` shows today forecast rows (same display format as before).
  2. Select a today row to show hourly forecast rows for that city.
- Week view is two-stage:
  1. `ww <query>` to pick a city.
  2. Select a city row to show fixed 7-day forecast rows.
- Empty query uses `WEATHER_DEFAULT_CITIES`.

## Workflow Variables

Set these via Alfred's `Configure Workflow...` UI:

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `WEATHER_CLI_BIN` | No | `(empty)` | Optional executable path override for `weather-cli`. |
| `WEATHER_LOCALE` | No | `en` | Output locale for weather labels (`en` default, `zh` optional). |
| `WEATHER_DEFAULT_CITIES` | No | `Tokyo` | Default city list when query is empty (comma-separated). |
| `WEATHER_CACHE_TTL_SECS` | No | `900` | Cache TTL in seconds for weather responses (15 minutes). |

## Notes

- `wt` row format: `City HH:MM Temp°C Summary x%` (single-line title).
- `ww` row format: `City min~max°C Summary x%` (single-line title).
- Subtitle shows `Date Timezone Latitude,Longitude`.
- Result rows consume `weather-cli` icon metadata and render PNG assets under `assets/icons/weather/*.png`.
- Editable SVG source lives under `assets-src/icons/weather/*.svg`; `scripts/generate_weather_icons.sh` converts every source SVG to runtime PNG via `image-processing`.
- `wt` current-city rows use the city's current local time to select `*-day` / `*-night` variants; hourly rows use each row's local hour, so night rows now keep the same weather glyph with a darker night background and moon variants where applicable.
- `wt` stage-one autocomplete now uses `city::<location>` instead of embedding coordinates in the visible query; when a cached geocoding result exists, the script reuses cached coordinates before fetching hourly rows.
- `wt` keeps original today-row display as stage one, then opens hourly rows as stage two.
- `ww` uses city-picker stage first, then returns fixed 7-day rows for the selected city.
- Cache TTL is configurable via `WEATHER_CACHE_TTL_SECS` (`900` by default in workflow).
- Enter on result rows copies the selected row argument.
- The workflow calls `weather-cli` with `--output alfred-json --lang <locale>` (`hourly` for `wt`, `week` for `ww`).

## Local Validation

- `bash workflows/weather/tests/smoke.sh`
- `scripts/workflow-test.sh --id weather`
- `scripts/workflow-pack.sh --id weather`

## Optional live smoke (maintainer)

- `bash scripts/weather-cli-live-smoke.sh`
- This live check is optional maintainer validation for provider/fallback/contract behavior.
- It is not required for commit gates or CI pass/fail.

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

# Weather Workflow Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

Run from repository root.

```bash
# Required scripts
ls -l \
  workflows/weather/scripts/script_filter_today.sh \
  workflows/weather/scripts/script_filter_week.sh \
  workflows/weather/scripts/script_filter_common.sh \
  workflows/weather/scripts/action_copy.sh

# Runtime candidate check
test -x workflows/weather/bin/weather-cli && echo "bundled weather-cli found"
command -v weather-cli || true

# CLI contract checks
cargo run -q -p nils-weather-cli -- today --output json --city Tokyo | jq -e '.schema_version == "cli-envelope@v1" and .ok == true'
cargo run -q -p nils-weather-cli -- today --output alfred-json --city Tokyo | jq -e '.items | type == "array"'
cargo run -q -p nils-weather-cli -- today --output alfred-json --city Tokyo --city Osaka | jq -e '.items | type == "array"'

# Workflow entrypoints
bash workflows/weather/scripts/script_filter_today.sh "Tokyo" | jq -e '.items | type == "array"'
bash workflows/weather/scripts/script_filter_today.sh "Tokyo,Osaka" | jq -e '.items | type == "array"'
bash workflows/weather/scripts/script_filter_today.sh "city::Tokyo" | jq -e '.items | type == "array"'
bash workflows/weather/scripts/script_filter_week.sh "Tokyo" | jq -e '.items | type == "array"'
bash workflows/weather/scripts/script_filter_week.sh "city::Tokyo" | jq -e '.items | type == "array"'

# Confirm default env configuration
rg -n "WEATHER_CLI_BIN|WEATHER_LOCALE|WEATHER_DEFAULT_CITIES|WEATHER_CACHE_TTL_SECS|PREFERENCE_PROJECTION_FILE" workflows/weather/workflow.toml

# External preference projection: resolved default locations and status row
cargo run -q -p nils-weather-cli -- default-locations --fallback "Tokyo,Osaka" \
  --preference-projection-file "$HOME/path/to/preference-projection.json" --output json | jq '.result'
cargo run -q -p nils-weather-cli -- preference-status \
  --preference-projection-file "$HOME/path/to/preference-projection.json" | jq '.items'
```

`jq` is recommended for local validation and shell-side normalization/token rewriting:

```bash
command -v jq || echo "jq missing: single-city normalization and local validation will be degraded"
```

## Common failures and actions

| Symptom | Likely cause | Action |
| --- | --- | --- |
| `weather-cli binary not found` row | Binary absent in lookup paths | Re-package workflow or set `WEATHER_CLI_BIN` to executable absolute path. |
| `Invalid location input` | Bad city/coordinate format | Use `City` or `lat,lon` (example: `25.03,121.56`). |
| `Location not found` | Ambiguous/unknown city | Use more specific name or coordinates. |
| `Weather provider unavailable` | Upstream provider/API transient issue | Retry later before changing workflow code/config. |
| `Weather output format error` | Custom/old `weather-cli` returned unexpected JSON | Use packaged pinned binary or update local override binary. |
| `Single-city rows show raw header / extra metadata` | `jq` missing, so shell cannot normalize single-city Alfred rows | Install `jq` for local runs or use the packaged workflow environment. |
| Status row `Preferences: projection unavailable — using workflow settings` | `PREFERENCE_PROJECTION_FILE` points to a missing or unreadable file | Check the path (`~/` is expanded) and that the preference owner has published the file. Clear the variable to hide the row. |
| Status row `Preferences: projection stale — using workflow settings` | The projection `generatedAt` is older than 7 days | Refresh the projection from its owner. Until then `WEATHER_DEFAULT_CITIES` applies. |
| Status row `Preferences: projection invalid — using workflow settings` | The file is oversize, not JSON, has a wrong schema, extra or missing fields, a bad label, or a future timestamp | Validate the file against `crates/workflow-common/docs/preference-projection-contract.md`; `weather-cli default-locations --output json` names only the state, never values. |
| Status row reports `has no usable entries` | The projection default location is empty and it has no saved locations | Add locations at the preference owner; `WEATHER_DEFAULT_CITIES` applies meanwhile. |
| No status row although `PREFERENCE_PROJECTION_FILE` is set | `jq` is missing, or an old `weather-cli` override lacks `preference-status` | Install `jq` and use the packaged `weather-cli`. Defaults still resolve through `default-locations`. |
| A projection location like `Springfield, Oregon` is split into two cities | An old workflow script comma-splits the default list | Re-install the current package; projection labels are read one per line and city tokens stay whole. |

If only `ww` mode looks odd, verify the two-stage flow first: `ww <query>` to pick a city, then select the city row.
If only `wt` stage two looks odd, inspect the persistent geocoding cache under the workflow cache root:

```bash
find "${ALFRED_WORKFLOW_CACHE:-${TMPDIR:-/tmp}/nils-weather-cli}/weather-cli/geocode" -maxdepth 1 -type f -name '*.json' 2>/dev/null | sort
```

## Validation

```bash
bash workflows/weather/tests/smoke.sh
scripts/workflow-test.sh --id weather
scripts/workflow-pack.sh --id weather
```

Optional asset consistency check:

```bash
bash workflows/weather/scripts/generate_weather_icons.sh
bash scripts/weather-cli-live-smoke.sh
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/weather/<version>/`.
2. Reset variables to defaults (`WEATHER_CLI_BIN=""`, `WEATHER_LOCALE="en"`, `WEATHER_DEFAULT_CITIES="Tokyo"`,
   `WEATHER_CACHE_TTL_SECS="900"`, `PREFERENCE_PROJECTION_FILE=""`).
3. If regression remains, roll back `workflows/weather/` on a branch, then rerun Validation before release.

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

# Today/week entrypoints should both return Alfred JSON rows
bash workflows/weather/scripts/script_filter_today.sh "Tokyo" | jq -e '.items | type == "array"'
bash workflows/weather/scripts/script_filter_week.sh "Tokyo" | jq -e '.items | type == "array"'

# Confirm default env configuration
rg -n "WEATHER_CLI_BIN|WEATHER_LOCALE|WEATHER_DEFAULT_CITIES|WEATHER_CACHE_TTL_SECS" workflows/weather/workflow.toml
```

Multi-city mode combines JSON with `jq`; check dependency explicitly:

```bash
command -v jq || echo "jq missing: multi-city aggregation is limited"
```

## Common failures and actions

| Symptom                            | Likely cause                                      | Action                                                                    |
| ---------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------- |
| `weather-cli binary not found` row | Binary absent in lookup paths                     | Re-package workflow or set `WEATHER_CLI_BIN` to executable absolute path. |
| `Invalid location input`           | Bad city/coordinate format                        | Use `City` or `lat,lon` (example: `25.03,121.56`).                        |
| `Location not found`               | Ambiguous/unknown city                            | Use more specific name or coordinates.                                    |
| `Weather provider unavailable`     | Upstream provider/API transient issue             | Retry later before changing workflow code/config.                         |
| `Weather output format error`      | Custom/old `weather-cli` returned unexpected JSON | Use packaged pinned binary or update local override binary.               |

If only week mode looks odd, verify two-stage usage first: `ww <query>` to pick city, then select city row.

## Validation

```bash
bash workflows/weather/tests/smoke.sh
scripts/workflow-test.sh --id weather
scripts/workflow-pack.sh --id weather
```

Optional asset consistency check:

```bash
bash workflows/weather/scripts/generate_weather_icons.sh
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/weather/<version>/`.
2. Reset variables to defaults (`WEATHER_CLI_BIN=""`, `WEATHER_LOCALE="en"`, `WEATHER_DEFAULT_CITIES="Tokyo"`,
   `WEATHER_CACHE_TTL_SECS="900"`).
3. If regression remains, roll back `workflows/weather/` on a branch, then rerun Validation before release.

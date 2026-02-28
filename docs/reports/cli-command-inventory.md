# CLI Command Inventory (Sprint 1 Baseline)

## Inventory Scope

- Scope: workspace CLI crates under `crates/*-cli`, plus the scoped native crate `crates/google-cli`.
- Baseline date: 2026-02-27.
- Sources:
  - `crates/*-cli/src/main.rs` command definitions.
  - workflow consumer scripts under `workflows/*/scripts/*.sh`.

## Command Surface + Consumer Mapping

| Crate                | Binary          | Command         | Key options                                                                            | Current output mode                                                  | Target mode (migration)                                                    | Primary consumer mapping (workflow/script_filter)                                                                                      |
| -------------------- | --------------- | --------------- | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `nils-brave-cli`     | `brave-cli`     | `search`        | `--query <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/google-search/scripts/script_filter.sh`                                                                                     |
| `nils-cambridge-cli` | `cambridge-cli` | `query`         | `--input <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/cambridge-dict/scripts/script_filter.sh`                                                                                    |
| `nils-epoch-cli`     | `epoch-cli`     | `convert`       | `--query <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/epoch-converter/scripts/script_filter.sh`                                                                                   |
| `nils-market-cli`    | `market-cli`    | `fx`            | `--base --quote --amount`                                                              | JSON object (service payload, no envelope)                           | `human-readable` default + explicit `--json` envelope                      | Service-style callers and `scripts/market-cli-live-smoke.sh`                                                                           |
| `nils-market-cli`    | `market-cli`    | `crypto`        | `--base --quote --amount`                                                              | JSON object (service payload, no envelope)                           | `human-readable` default + explicit `--json` envelope                      | Service-style callers and `scripts/market-cli-live-smoke.sh`                                                                           |
| `nils-market-cli`    | `market-cli`    | `expr`          | `--query --default-fiat`                                                               | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/market-expression/scripts/script_filter.sh`                                                                                 |
| `nils-quote-cli`     | `quote-cli`     | `feed`          | `--query`                                                                              | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/quote-feed/scripts/script_filter.sh`                                                                                        |
| `nils-randomer-cli`  | `randomer-cli`  | `list-formats`  | `--query`                                                                              | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/randomer/scripts/script_filter.sh`                                                                                          |
| `nils-randomer-cli`  | `randomer-cli`  | `list-types`    | `--query`                                                                              | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/randomer/scripts/script_filter_types.sh`                                                                                    |
| `nils-randomer-cli`  | `randomer-cli`  | `generate`      | `--format --count`                                                                     | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/randomer/scripts/script_filter_expand.sh`                                                                                   |
| `nils-spotify-cli`   | `spotify-cli`   | `search`        | `--query <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/spotify-search/scripts/script_filter.sh`                                                                                    |
| `nils-timezone-cli`  | `timezone-cli`  | `now`           | `--query --config-zones`                                                               | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/multi-timezone/scripts/script_filter.sh`                                                                                    |
| `nils-weather-cli`   | `weather-cli`   | `today`         | `--city/--lat/--lon`, `--json`, `--output alfred-json`, `--lang <en \| zh>`            | Text default (`en`); `--json` returns envelope v1; `--output` Alfred | Keep text default; preserve explicit JSON/Alfred modes                     | `workflows/weather/scripts/script_filter_today.sh`                                                                                     |
| `nils-weather-cli`   | `weather-cli`   | `week`          | `--city/--lat/--lon`, `--json`, `--output alfred-json`, `--lang <en \| zh>`            | Text default (`en`); `--json` returns envelope v1; `--output` Alfred | Keep text default; preserve explicit JSON/Alfred modes                     | `workflows/weather/scripts/script_filter_week.sh`                                                                                      |
| `nils-weather-cli`   | `weather-cli`   | `hourly`        | `--city/--lat/--lon`, `--json`, `--output alfred-json`, `--lang <en \| zh>`, `--hours` | Text default (`en`); `--json` returns envelope v1; `--output` Alfred | Keep text default; preserve explicit JSON/Alfred modes                     | `workflows/weather/scripts/script_filter_today.sh`                                                                                     |
| `nils-wiki-cli`      | `wiki-cli`      | `search`        | `--query <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/wiki-search/scripts/script_filter.sh`                                                                                       |
| `nils-workflow-cli`  | `workflow-cli`  | `script-filter` | `--query`, `--mode <open \| github>`                                                   | Legacy Alfred JSON (top-level `items`)                               | Keep command for compatibility; add explicit standards JSON mode           | `workflows/open-project/scripts/script_filter.sh`, `workflows/open-project/scripts/script_filter_github.sh`, `_template/script_filter` |
| `nils-workflow-cli`  | `workflow-cli`  | `record-usage`  | `--path <dir>`                                                                         | Plain text (path echo)                                               | Keep plain text action contract                                            | `workflows/open-project/scripts/action_record_usage.sh`                                                                                |
| `nils-workflow-cli`  | `workflow-cli`  | `github-url`    | `--path <dir>`                                                                         | Plain text (URL)                                                     | Keep plain text action contract                                            | `workflows/open-project/scripts/action_open_github.sh`                                                                                 |
| `nils-youtube-cli`   | `youtube-cli`   | `search`        | `--query <text>`                                                                       | Legacy Alfred JSON (top-level `items`)                               | `human-readable` default + explicit `--json` envelope + compatibility mode | `workflows/youtube-search/scripts/script_filter.sh`                                                                                    |

## Scoped native crate

- `google-cli` / `google-cli`
  - Commands: `auth <...>`, `gmail <...>`, `drive <...>`
  - Key options: `--account`, `--json`, `--plain`, plus command-local native flags
  - Current output mode: native human/plain output; explicit JSON envelope mode
  - Target mode: keep scoped native behavior for direct CLI and future integrations
  - Primary consumer mapping: direct terminal use and native contract tests; no Alfred workflow consumer in this phase

## Consumer Risk Notes

- Highest migration sensitivity: all workflow `script_filter` and `script-filter` callers currently assume Alfred JSON
  by default.
- `google-cli` is a scoped direct-use native crate; its main risk is Google API behavior drift rather than Alfred consumer breakage.
- `weather-cli` now has direct workflow consumers (`wt` / `ww`), so script-filter compatibility mode must remain stable.
- `workflow-cli` must preserve action-command plain text behavior while adding explicit machine mode for structured
  integrations.

## Migration Priority Hints

1. Pilot crates: `weather-cli`, `market-cli` (already have partial mode separation or service-like JSON payloads).
2. Then migrate JSON-first workflow-facing crates with explicit compatibility flags.
3. Keep consumer script updates atomic with command-mode changes to avoid script_filter regressions.

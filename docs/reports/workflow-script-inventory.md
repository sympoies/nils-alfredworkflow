# Workflow Script Inventory (Sprint 3)

## Summary

- Source of truth for canonical workflow entrypoint declarations: `workflows/*/workflow.toml`.
- Inventory objective: map each manifest entrypoint to script files, then mark orphan/delete candidates.
- Sprint 3 outcome: true orphan scripts are blocked while required non-manifest hook scripts are preserved.

## Manifest Entrypoint Inventory

| Workflow | script_filter entrypoint | action entrypoint |
| --- | --- | --- |
| `_template` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `bangumi-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `bilibili-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `cambridge-dict` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `codex-cli` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `epoch-converter` | `scripts/script_filter.sh` | `scripts/action_copy.sh` |
| `google-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `google-service` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `imdb-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `market-expression` | `scripts/script_filter.sh` | `scripts/action_copy.sh` |
| `memo-add` | `scripts/script_filter_entry.sh` | `scripts/action_run.sh` |
| `multi-timezone` | `scripts/script_filter.sh` | `scripts/action_copy.sh` |
| `netflix-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `open-project` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `quote-feed` | `scripts/script_filter.sh` | `scripts/action_copy.sh` |
| `randomer` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `spotify-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `steam-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `weather` | `scripts/script_filter_today.sh` | `scripts/action_copy.sh` |
| `wiki-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |
| `youtube-search` | `scripts/script_filter.sh` | `scripts/action_open.sh` |

## Shared-Lane Runtime Inventory (S3T2)

| File | Runtime role | Status |
| --- | --- | --- |
| `scripts/lib/workflow_helper_loader.sh` | Shared helper-loader primitives | kept; added required-helper utility |
| `scripts/lib/script_filter_cli_driver.sh` | Non-search script_filter runtime driver | kept |
| `scripts/lib/script_filter_search_driver.sh` | Search script_filter runtime driver | kept |
| `scripts/lib/workflow_action_open_url.sh` | Shared open-url action runner | kept |
| `scripts/lib/workflow_action_copy.sh` | Shared clipboard action runner | kept |
| `workflows/google-search/scripts/script_filter.sh` | Google suggestion script_filter entrypoint | kept; helper stack migration |
| `workflows/youtube-search/scripts/script_filter.sh` | YouTube script_filter entrypoint | kept; helper stack migration |
| `workflows/weather/scripts/script_filter_today.sh` | Weather today script_filter entrypoint | kept; helper stack migration |
| `workflows/market-expression/scripts/script_filter.sh` | Market expression script_filter entrypoint | kept; helper stack migration |
| `workflows/open-project/scripts/script_filter.sh` | Open-project script_filter entrypoint | kept; helper stack migration |
| `workflows/quote-feed/scripts/script_filter.sh` | Quote-feed script_filter entrypoint | kept; helper stack migration |

## Orphan / Delete Matrix (S3T4)

| File | Classification before cleanup | Decision | Current state |
| --- | --- | --- | --- |
| `workflows/google-search/scripts/script_filter_direct.sh` | direct-search workflow entrypoint referenced by plist/tests | keep | retained |
| `workflows/codex-cli/scripts/prepare_package.sh` | packaging hook script referenced by pack flow/tests | keep | retained |
| `workflows/weather/scripts/generate_weather_icons.sh` | utility script outside runtime entrypoint graph | keep (explicit exemption) | retained |

## Non-Orphan Baseline

- `scripts/workflow-shared-foundation-audit.sh --check` now enforces orphan detection.
- New workflow scripts without a manifest/plist/script entrypoint reference fail lint.
- Required non-manifest hooks (`script_filter_direct.sh`, `prepare_package.sh`) must exist and stay executable.

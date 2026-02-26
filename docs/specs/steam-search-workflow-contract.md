# Steam Search Workflow Contract

## Goal

- Define the `steam-search` source contract, region semantics, and fallback behavior.
- Lock what must use shared helper foundations versus what must stay local to Steam domain logic.

## Source Contract (Steam Store)

- Primary search source: `https://store.steampowered.com/api/storesearch`.
- Detail enrichment source (optional, per result): `https://store.steampowered.com/api/appdetails`.
- Request contract:
  - `q`: user query string.
  - `cc`: region/country code used for store availability and pricing context.
  - `l`: Steam language parameter used for localized titles/subtitles when configured.
- Response parsing contract:
  - Parse only documented/observed JSON fields needed for Alfred rows (app id, title, URL, price/platform text).
  - Treat missing optional fields as partial-success rows, not fatal parser errors.

## Region Semantics

- Region values are two-letter country codes used for Steam `cc`.
- `STEAM_REGION` defines the default region for search calls.
- `STEAM_REGION_OPTIONS` defines switchable region rows and preserves configured order.
- Action requery rows use the `steam-requery:<region>:<query>` argument contract.
- Region switching persists override state in workflow cache/data and re-runs the current keyword query.

## Fallback And Error Strategy

- No scraping requirements are allowed for this workflow contract.
- If Steam request fails (network, timeout, DNS/TLS, or non-2xx):
  - emit a deterministic Alfred error item;
  - keep action rows non-destructive and retry-safe;
  - preserve original query text for retry.
- If payload parsing fails:
  - emit a deterministic malformed-response error item;
  - do not crash script adapters.
- If results are empty:
  - emit a deterministic no-results row;
  - still include region-switch rows when configured.

## Shared Helper Adoption Matrix

| Area | Contract | Ownership |
| --- | --- | --- |
| Helper loading | Must use `scripts/lib/workflow_helper_loader.sh` (`wfhl_source_helper`). | Shared helper |
| Search orchestration | Must use `scripts/lib/script_filter_search_driver.sh` (`sfsd_run_search_flow`) for cache/coalesce flow. | Shared helper |
| Query normalization | Must use `scripts/lib/script_filter_query_policy.sh` for input/query guards. | Shared helper |
| Action requery parse/persist/trigger | Must use `scripts/lib/workflow_action_requery.sh`. | Shared helper |
| URL open action | Must use `scripts/lib/workflow_action_open_url.sh`. | Shared helper |
| Steam endpoint choice/params | API URL selection, `storesearch` request params, and Steam-specific response mapping. | Must stay local |
| Steam error interpretation text | Steam-specific row titles/subtitles and user guidance copy. | Must stay local |
| Steam ranking/selection rules | Ordering and domain-specific display choices. | Must stay local |

## Local-Only Boundary

- Steam provider semantics must stay local:
  - endpoint-specific field mapping;
  - region/language defaults specific to Steam;
  - Steam domain ranking and wording rules.
- Shared helper extraction must not move provider-specific parsing or ranking logic into `scripts/lib`.

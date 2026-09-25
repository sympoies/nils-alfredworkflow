# Preference Projection Contract

> Status: active

## Purpose

An external preference projection lets a deployment keep Alfred defaults
aligned with another preference owner without hard-coding personal data into a
workflow package. The owner publishes one bounded JSON file; the Market
Expression and Weather workflows read it as consumers only. Workflows never
write the file, never manage the owner's credentials, and never treat a query
as a preference change.

The loader is `nils_workflow_common::preference_projection`
(`load_preference_projection`). Workflow-specific mapping stays in the
consuming CLI crates (`market-cli favorites`, `weather-cli default-locations`).

## Configuration

- Workflow variable: `PREFERENCE_PROJECTION_FILE` (Market Expression and
  Weather). Default is empty.
- Empty keeps today's behavior exactly and shows no status row.
- A leading `~/` is expanded by the shell adapter through
  `wfcr_expand_home_path` before the path is passed to the CLI.

## File format

One UTF-8 JSON object of at most 16384 bytes with exactly these top-level
fields. Unknown or missing fields at any level make the file invalid.

```json
{
  "schema": "sympoies.alfred-preference-projection/v1",
  "generatedAt": "2026-03-01T11:48:00Z",
  "revision": 2,
  "digest": "sha256:<64 hex characters>",
  "market": {
    "default_quote_currency": "EUR",
    "watchlist": ["USD", "JPY", "BTC", "ETH"]
  },
  "weather": {
    "default_location": "Springfield, Oregon",
    "saved_locations": ["東京", "Kyoto"]
  },
  "sources": {
    "market.default_quote_currency": "profile",
    "market.watchlist": "owner_override",
    "weather.default_location": "profile",
    "weather.saved_locations": "profile"
  }
}
```

| Field | Rule |
| --- | --- |
| `schema` | Exactly `sympoies.alfred-preference-projection/v1`. |
| `generatedAt` | UTC timestamp `YYYY-MM-DDTHH:MM:SSZ`. |
| `revision` | Integer `>= 1`, or `null`. |
| `digest` | `sha256:` plus 64 hex characters, or `null`. It is `null` exactly when `revision` is `null`. |
| `market.default_quote_currency` | Label (at most 16 characters). Empty means unset. |
| `market.watchlist` | Ordered list of unique non-empty labels, at most 32 entries of at most 16 characters. |
| `weather.default_location` | Label (at most 128 characters). Empty means unset. |
| `weather.saved_locations` | Ordered list of unique non-empty labels, at most 32 entries of at most 128 characters. |
| `sources.<field>` | `profile` or `owner_override` for each of the four preference fields. |

Labels contain no Unicode control, format, line-separator, or
paragraph-separator characters (`Cc`, `Cf`, `Zl`, `Zp`) and no leading or
trailing whitespace. Location labels may contain commas and non-ASCII text, for
example `Taipei, Taiwan` or `東京`; consumers never split them on commas.
Uniqueness inside a list is exact; case variants are accepted and deduplicated
by the consumer.

## Freshness

- A `generatedAt` older than 7 days is stale.
- A `generatedAt` more than 5 minutes in the future is invalid.

## Precedence

| Workflow | Empty-query precedence |
| --- | --- |
| Market Expression | valid fresh projection > `MARKET_FAVORITE_LIST` > built-in default |
| Weather | valid fresh projection > `WEATHER_DEFAULT_CITIES` > built-in default |

A non-empty query always wins and never consults the projection. Weather keeps
`lat,lon` query priority and its existing query filter semantics.

## Workflow mapping

Market Expression (`market-cli favorites --preference-projection-file`):

- A watchlist entry that is an active ISO 4217 fiat code becomes the explicit
  pair `SYM/<default_quote_currency>`. A fiat equal to that currency is
  omitted. A fiat entry is skipped when the quote currency is unset or not a
  fiat code.
- Any other entry that passes market-cli symbol validation stays a bare symbol
  quoted in the workflow's `MARKET_DEFAULT_FIAT`. The projection's quote
  currency does not replace `MARKET_DEFAULT_FIAT`.
- Rejected entries are dropped and counted in the status row; the rest of the
  list is kept. When nothing usable remains, the workflow settings apply.

Example: watchlist `USD, JPY, BTC, ETH`, quote currency `EUR`, and
`MARKET_DEFAULT_FIAT=USD` give `USD/EUR`, `JPY/EUR`, `BTC`, `ETH` (crypto quoted
in USD).

Weather (`weather-cli default-locations --preference-projection-file`):

- Default locations are `default_location` followed by `saved_locations`,
  deduplicated case-insensitively while keeping the first occurrence.
- `weather-cli default-locations` prints one location per line; the shell
  adapter reads it line by line instead of comma-splitting.
- When the resulting list is empty, the workflow settings apply.

## Status row

When `PREFERENCE_PROJECTION_FILE` is set, an empty query adds one
non-selectable row (`valid=false`, no `arg`):

| State | Title |
| --- | --- |
| used | `Preferences: projection revision 2 · synced 12m ago` |
| empty | `Preferences: projection revision 2 has no usable entries — using workflow settings` |
| unavailable | `Preferences: projection unavailable — using workflow settings` |
| stale | `Preferences: projection stale — using workflow settings` |
| invalid | `Preferences: projection invalid — using workflow settings` |

`unavailable` covers a missing or unreadable file. `invalid` covers oversize,
non-UTF-8 or malformed JSON, schema or field violations, and future
timestamps. A projection without a revision is shown as
`projection (no revision)`.

## Privacy

Status rows and error messages never contain the configured path or any
preference value. Validation failures name only the contract field, for
example `projection field failed validation: market.watchlist`.

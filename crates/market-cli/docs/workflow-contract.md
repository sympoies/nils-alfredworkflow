# Market CLI Contract

## Purpose

This document defines the command and JSON output contract for the `market-cli` capability.
Scope includes market data retrieval (`fx`, `crypto`) and Alfred-facing expression output (`expr`).
It also includes favorites-list output for the `market-expression` workflow empty-query state (`favorites`).

## Command Contract

### FX

- Command:
  - `market-cli fx --base <ISO4217> --quote <ISO4217> --amount <decimal>`
- Required flags:
  - `--base`: base fiat currency (for example `USD`)
  - `--quote`: quote fiat currency (for example `JPY`)
  - `--amount`: amount to convert, must be a positive decimal

### Crypto

- Command:
  - `market-cli crypto --base <SYMBOL> --quote <SYMBOL> --amount <decimal>`
- Required flags:
  - `--base`: base asset symbol (for example `BTC`)
  - `--quote`: quote symbol (for example `USD`)
  - `--amount`: amount to convert, must be a positive decimal

### Expr

- Command:
  - `market-cli expr --query "<expression>" [--default-fiat <ISO4217>]`
- Required flags:
  - `--query`: expression string
- Optional flags:
  - `--default-fiat`: 3-letter fiat code used when query omits `to <fiat>` (default `USD`)
- Expression behavior:
  - Numeric-only terms -> one Alfred item with final result (`1+5` -> `6`; supports `+ - * /`)
  - Asset-only terms -> unit-price items for each unique asset, then total item
  - Mixed asset and numeric terms -> user error
  - Asset expressions with unsupported operators (`*`, `/`) -> user error

### Favorites

- Command:
  - `market-cli favorites [--list "<comma/newline symbols>"] [--default-fiat <ISO4217>] [--output <human|json|alfred-json> | --json]`
- Optional flags:
  - `--list`: ordered favorites list, typically sourced from Alfred workflow variable `MARKET_FAVORITE_LIST`
  - `--default-fiat`: fallback fiat symbol used when the list is missing or empty (default `USD`)
  - `--output`: explicit output mode override (`human`, `json`, `alfred-json`)
  - `--json`: shorthand for service-envelope JSON output
- Favorites behavior:
  - Empty-query workflow state may call `market-cli favorites`
    when workflow variable `MARKET_FAVORITES_ENABLED` is enabled;
    otherwise the workflow may render only the prompt row locally
  - Successful output is Alfred Script Filter JSON by default
  - Favorites output always starts with a non-actionable prompt row, followed by one non-actionable row per favorite symbol
  - Quote rows render `1 <SYMBOL> = <PRICE> <DEFAULT_FIAT>` when pricing succeeds
  - If one favorite quote cannot be resolved, that row falls back to a symbol hint instead of failing the whole payload
  - Every favorites row remains non-actionable / non-selectable (`valid: false`)
  - Ordered parsing preserves source order, trims surrounding whitespace, accepts comma/newline separators, and de-duplicates by first occurrence
  - Empty or delimiter-only list input falls back to `BTC,ETH,<DEFAULT_FIAT>,JPY`
  - Invalid non-empty tokens surface a user error rather than being silently skipped

### Exit Behavior

- Exit code `0`: success (stdout prints exactly one JSON object)
- Exit code `2`: user/input error (invalid symbol format, invalid expression, non-positive amount, missing required flags)
- Exit code `1`: runtime/provider/cache error without usable fallback
- `favorites` follows the same exit contract and prints exactly one JSON object on success

## Provider and Cache Policy

- No API key is required for any command path.
- FX provider stack:
  - `Frankfurter` (single provider)
  - Fixed TTL: `86400` seconds (`24h`)
- Crypto provider stack:
  - Primary: `Coinbase`
  - Fallback: `Kraken`
  - Fixed TTL: `300` seconds (`5m`)
- Freshness states:
  - `live`: freshly fetched from provider
  - `cache_fresh`: served from cache within TTL
  - `cache_stale_fallback`: provider failed, stale cache returned as fallback
- Retry/backoff policy:
  - bounded retries only (`max_attempts = 3`)
  - exponential backoff from base `200ms` (200ms, 400ms)
  - retryable: transport failures and HTTP `429`/`5xx`
  - non-retryable: invalid payload and unsupported pair errors (fail fast)

## Output JSON Schema

Successful output is one JSON object with this shape:

```json
{
  "kind": "fx|crypto",
  "base": "USD",
  "quote": "JPY",
  "amount": "100",
  "unit_price": "31.25",
  "converted": "3125",
  "provider": "frankfurter",
  "fetched_at": "2026-02-10T09:30:12Z",
  "cache": {
    "status": "live|cache_fresh|cache_stale_fallback",
    "key": "fx-usd-twd",
    "ttl_secs": 86400,
    "age_secs": 0
  }
}
```

For `expr`, successful output is Alfred Script Filter JSON:

```json
{
  "items": [
    {
      "title": "1 BTC = 3200000 JPY",
      "subtitle": "provider: coinbase · freshness: live",
      "arg": "3200000 JPY",
      "valid": true
    },
    {
      "title": "Total = 12800000 JPY",
      "subtitle": "Formula: 1*3200000(BTC) + 3*3200000(BTC) = 12800000 JPY",
      "arg": "12800000 JPY",
      "valid": true
    }
  ]
}
```

For `favorites`, successful output is Alfred Script Filter JSON with a prompt row plus non-actionable favorite quote rows:

```json
{
  "items": [
    {
      "uid": "market-favorites-prompt",
      "title": "Enter a market expression",
      "subtitle": "Example: 1 BTC + 3 ETH to JPY (default fiat: USD)",
      "valid": false
    },
    {
      "uid": "market-favorite-btc-usd",
      "title": "1 BTC = 68194 USD",
      "subtitle": "provider: coinbase · freshness: live",
      "valid": false
    },
    {
      "uid": "market-favorite-usd-usd",
      "title": "1 USD = 1 USD",
      "subtitle": "provider: identity · freshness: fixed",
      "valid": false
    }
  ]
}
```

Field requirements:

| Field | Type | Notes |
| --- | --- | --- |
| `kind` | string | `fx` or `crypto` |
| `base` | string | Uppercase symbol |
| `quote` | string | Uppercase symbol |
| `amount` | string | Requested conversion amount (normalized decimal string) |
| `unit_price` | string | Price of 1 `base` in `quote` (normalized decimal string) |
| `converted` | string | `amount * unit_price` (normalized decimal string) |
| `provider` | string | Final provider used for returned data |
| `fetched_at` | string | RFC3339 UTC timestamp of source data |
| `cache` | object | Cache metadata block |
| `cache.status` | string | `live`, `cache_fresh`, or `cache_stale_fallback` |
| `cache.key` | string | Stable cache key (`<kind>-<base>-<quote>`) |
| `cache.ttl_secs` | number | Fixed per market kind (`86400` or `300`) |
| `cache.age_secs` | number | Cache age in seconds at response time |

Favorites row requirements:

| Field | Type | Notes |
| --- | --- | --- |
| `items[].uid` | string | Stable Alfred row identity for prompt and favorite quote rows |
| `items[].title` | string | Prompt title or favorite quote title (`1 BTC = ... USD`) |
| `items[].subtitle` | string | Prompt guidance, quote metadata, or symbol-hint fallback when quote lookup fails |
| `items[].valid` | boolean | Must be `false` for every favorites item (non-actionable / non-selectable policy) |

## `script_filter.sh` Integration Notes

- `market-expression` workflow calls `market-cli favorites`
  for `mx` empty query only when `MARKET_FAVORITES_ENABLED` is enabled,
  then passes through Alfred JSON.
- `market-expression` workflow calls `market-cli expr` for `mx <expression>` and passes through Alfred JSON.
- Workflow variable `MARKET_FAVORITES_ENABLED` defaults to enabled;
  `0`, `false`, `no`, or `off` should disable favorite quote rows
  and keep only the prompt row.
- Workflow variable `MARKET_FAVORITE_LIST` should be passed to `--list`; empty or delimiter-only config falls back to `BTC,ETH,<MARKET_DEFAULT_FIAT>,JPY`.
- For non-zero exits, script filter should render one fallback item with `valid: false`.

Minimal shell examples:

```bash
# FX
json="$(market-cli fx --base USD --quote JPY --amount 100)"
JSON="$json" python3 - <<'PY'
import json, os
data = json.loads(os.environ["JSON"])
print(data["converted"])
PY

# Crypto
json="$(market-cli crypto --base BTC --quote USD --amount 0.5)"
JSON="$json" python3 - <<'PY'
import json, os
data = json.loads(os.environ["JSON"])
print(f'{data["provider"]} / {data["cache"]["status"]}')
PY

# Expr (Alfred JSON passthrough)
market-cli expr --query "1 btc + 3 eth to jpy" --default-fiat USD

# Favorites (Alfred JSON passthrough)
market-cli favorites --list "btc,eth,usd,jpy" --default-fiat USD
```

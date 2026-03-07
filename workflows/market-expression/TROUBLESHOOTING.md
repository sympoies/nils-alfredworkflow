# Market Expression Workflow Troubleshooting

Reference: [ALFRED_WORKFLOW_DEVELOPMENT.md](../../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Quick operator checks

Run from repository root.

```bash
# Required scripts
ls -l \
  workflows/market-expression/scripts/script_filter.sh \
  workflows/market-expression/scripts/action_copy.sh

# Runtime candidate check (bundled/release/debug/PATH via script logic)
test -x workflows/market-expression/bin/market-cli && echo "bundled market-cli found"
command -v market-cli || true

# Baseline expression should return Alfred JSON rows
bash workflows/market-expression/scripts/script_filter.sh "1 BTC + 2 ETH to USD" | jq -e '.items | type == "array"'

# Quote rows should surface cached icon paths when available
MARKET_CLI_BIN="$(pwd)/target/debug/market-cli" \
  bash workflows/market-expression/scripts/script_filter.sh "1 BTC + 2 ETH to USD" \
  | jq -r '.items[] | select(.icon.path? != null) | .title + " -> " + .icon.path'

# Empty query should return one prompt row plus non-selectable favorites rows when enabled
MARKET_CLI_BIN="$(pwd)/target/debug/market-cli" \
MARKET_FX_CACHE_TTL="1d" \
MARKET_CRYPTO_CACHE_TTL="5m" \
MARKET_FAVORITES_ENABLED="1" \
MARKET_FAVORITE_LIST="BTC,ETH,JPY/USD,JPY/TWD" \
  bash workflows/market-expression/scripts/script_filter.sh "" \
  | jq -e '.items | length == 5 and .[0].title == "Enter a market expression" and all(.[]; .valid == false)'

# Confirm defaults in workflow manifest
rg -n "MARKET_CLI_BIN|MARKET_DEFAULT_FIAT|MARKET_FX_CACHE_TTL|MARKET_CRYPTO_CACHE_TTL|MARKET_FAVORITES_ENABLED|MARKET_FAVORITE_LIST" workflows/market-expression/workflow.toml
```

## Common failures and actions

| Symptom | Likely cause | Action |
| ---------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `market-cli binary not found` row | Binary is absent in all lookup paths | Package workflow again or set `MARKET_CLI_BIN` to executable absolute path. |
| FX or crypto cache feels too fresh/stale | `MARKET_FX_CACHE_TTL` or `MARKET_CRYPTO_CACHE_TTL` is unset, too small, too large, or invalid | Leave them empty for defaults, or set explicit durations like `15m`, `1h`, or `1d`. Invalid values fall back to the built-in defaults. |
| Empty query shows only the prompt row | `MARKET_FAVORITES_ENABLED` is disabled | Set `MARKET_FAVORITES_ENABLED` to `1`, `true`, or `on` if you want favorite quotes below the prompt. |
| Empty query shows unexpected order or missing favorites | `MARKET_FAVORITE_LIST` contains duplicates, different order, or custom separators | `MARKET_FAVORITE_LIST` preserves first-occurrence order after trimming comma/newline tokens. Duplicates are evaluated by effective base/quote pair, so `USD` and `USD/TWD` collapse when `MARKET_DEFAULT_FIAT=TWD`. |
| Empty query falls back to `BTC,ETH,<MARKET_DEFAULT_FIAT>,JPY` | `MARKET_FAVORITE_LIST` is empty or delimiter-only | This is expected fallback behavior. Set a non-empty comma/newline list to override it. |
| Empty query shows a generic `Market Expression error` row | `MARKET_FAVORITE_LIST` contains an invalid symbol/pair token or `MARKET_DEFAULT_FIAT` is invalid | Use uppercase symbol tokens like `BTC`, `JPY` or uppercase FX pairs like `JPY/USD`. Empty or delimiter-only input falls back automatically; malformed non-empty tokens do not. |
| Empty query shows a raw symbol/pair instead of `1 BASE = ... QUOTE` | Quote lookup for that favorite failed and the row degraded to hint mode | Retry after provider recovery, or inspect cache/provider connectivity if it persists for the same symbol/pair. |
| Quote rows show no icon | Cold icon cache, icon CDN issue, or symbol has no dedicated icon and generic fallback was unavailable | Retry once to allow cold-cache fill, then inspect the market cache tree under `market-cli/icons/cryptocurrency-icons/0.18.1/32/color/`. Rows should still work without icons. |
| First render feels slower than later renders | Cold icon cache download happened during row rendering | Re-run the same query once. Warm-cache renders should reuse the same cached icon path. |
| `Unsupported operator` row | Asset expression used `*` or `/` | Use `+`/`-` for asset terms. Keep `*`/`/` for numeric-only expressions. |
| `Invalid expression terms` row | Mixed raw numeric and asset terms in same expression | Use a single expression type per side (all numeric or all asset terms). |
| `Invalid to-clause` row | Missing/incomplete `to <FIAT>` target | Use complete target clause, e.g. `1 BTC + 2 ETH to USD`. |
| `provider failure` or transient runtime errors | Upstream provider/API issue | Retry after a short delay; do not assume local script defect first. |

Syntax probe example (safe, no clipboard action):

```bash
bash workflows/market-expression/scripts/script_filter.sh "1 BTC * 2 ETH" | jq -r '.items[0].title, .items[0].subtitle'

# Empty query probe (prompt row + favorites rows should stay non-selectable)
MARKET_CLI_BIN="$(pwd)/target/debug/market-cli" \
MARKET_FX_CACHE_TTL="1d" \
MARKET_CRYPTO_CACHE_TTL="5m" \
MARKET_FAVORITES_ENABLED="1" \
MARKET_FAVORITE_LIST=$'ETH\nBTC,JPY/USD,JPY/TWD' \
  bash workflows/market-expression/scripts/script_filter.sh "" \
  | jq -r '.items[] | [.title, .subtitle, (.valid|tostring)] | @tsv'

# Maintainer cold/warm icon cache probe
MARKET_CACHE_DIR="$(mktemp -d)" bash scripts/market-cli-live-smoke.sh
```

## Validation

```bash
bash workflows/market-expression/tests/smoke.sh
scripts/workflow-test.sh --id market-expression
scripts/workflow-pack.sh --id market-expression
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/market-expression/<version>/`.
2. Restore workflow variables to defaults
   (`MARKET_CLI_BIN=""`, `MARKET_DEFAULT_FIAT="USD"`,
   `MARKET_FX_CACHE_TTL=""`, `MARKET_CRYPTO_CACHE_TTL=""`,
   `MARKET_FAVORITES_ENABLED="1"`, `MARKET_FAVORITE_LIST="BTC,ETH,EUR,JPY"`)
   and retest.
3. If issue persists, roll back only `workflows/market-expression/` on a branch, then run all Validation commands before
   release.

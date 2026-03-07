# nils-market-cli

CLI backend for market data (`fx`, `crypto`) and market-expression workflow support.

## Commands

| Command | Options | Description |
| --- | --- | --- |
| `market-cli fx` | `--base <BASE> --quote <QUOTE> --amount <AMOUNT>` | Query fiat exchange rate (Frankfurter). |
| `market-cli crypto` | `--base <BASE> --quote <QUOTE> --amount <AMOUNT>` | Query crypto spot price (Coinbase primary, Kraken fallback). |
| `market-cli expr` | `--query <QUERY> [--default-fiat <DEFAULT_FIAT>]` | Evaluate market expressions and return Alfred Script Filter JSON. |
| `market-cli favorites` | `[--list <LIST>] [--default-fiat <DEFAULT_FIAT>] [--output <MODE> \| --json]` | Render the empty-query market prompt row plus non-actionable favorite quote rows for the `market-expression` workflow. |

## Environment Variables

- Optional cache override: `MARKET_CACHE_DIR`
- Optional FX cache TTL override: `MARKET_FX_CACHE_TTL` (supports `1s`, `1m`, `1h`, `1d`; empty keeps the built-in `1d` default)
- Optional crypto cache TTL override: `MARKET_CRYPTO_CACHE_TTL` (supports `1s`, `1m`, `1h`, `1d`; empty keeps the built-in `5m` default)
- Alfred fallback cache paths: `ALFRED_WORKFLOW_CACHE`, `ALFRED_WORKFLOW_DATA`
- Icon cache subtree: `<cache>/market-cli/icons/cryptocurrency-icons/0.18.1/32/color/`
- Workflow favorites source: `MARKET_FAVORITE_LIST` (typically passed to `market-cli favorites --list`)
- Workflow toggle: `MARKET_FAVORITES_ENABLED` controls whether
  the Alfred workflow calls `market-cli favorites` for empty query
- Favorites list semantics: comma/newline ordered input, trim per token, preserve first occurrence of the effective
  base/quote pair.
  Tokens may be plain symbols (`BTC`, `JPY`) or explicit FX pairs (`JPY/USD`, `JPY/TWD`).
  Plain symbols resolve against `MARKET_DEFAULT_FIAT`.
  Empty or delimiter-only input falls back to `BTC,ETH,<MARKET_DEFAULT_FIAT>,JPY`

## Output Contract

- `fx` / `crypto`: deterministic JSON object on `stdout`.
- `expr` / `favorites`: Alfred Script Filter JSON on `stdout` by default.
- `favorites` output starts with a non-actionable prompt row, then one non-actionable quote row per favorite symbol/pair.
- Favorite quote rows render `1 <BASE> = <PRICE> <QUOTE>` when pricing succeeds.
- If a favorite quote cannot be resolved, that row degrades to a symbol/pair hint instead of failing the whole empty-query payload.
- `fx` / `crypto` Alfred rows, favorite quote rows, and asset-expression quote rows
  may include Alfred `icon.path` values pointing at cached local PNG files.
- Icon resolution is best-effort: cached symbol icon first, then cached/downloaded `generic.png`, otherwise no icon field.
- `favorites` is the empty-query companion to `expr`:
  when `MARKET_FAVORITES_ENABLED` is on, workflow `mx` calls `favorites`;
  `mx <expression>` still calls `expr`.
- `favorites --json` returns the service envelope with the Alfred payload nested under `result`.
- `stderr`: user/runtime error text.
- Exit codes: `0` success, `1` runtime/provider error, `2` user/input error.

### Provider stack (no API key)

- FX: Frankfurter primary + FloatRates fallback (`24h` TTL by default)
- Crypto: Coinbase primary + Kraken fallback (`5m` TTL by default)
- `MARKET_FX_CACHE_TTL` overrides only FX TTL
- `MARKET_CRYPTO_CACHE_TTL` overrides only crypto TTL
- Freshness states: `live`, `cache_fresh`, `cache_stale_fallback`

### Icon source policy

- Pinned icon source: `cryptocurrency-icons@0.18.1` via jsDelivr `32/color/*.png`
- Cache ownership: `market-cli`, not workflow shell scripts
- Missing upstream icon or unsupported symbol: fall back to `generic.png`
- Icon fetch/cache failure: keep quote rows and omit icon metadata instead of failing the command

## Standards Status

- README/command docs: compliant.
- JSON service envelope (`schema_version/command/ok`): not yet migrated.
- Default human-readable mode: partially migrated (still JSON-first for `fx/crypto`).

## Documentation

- [`docs/README.md`](docs/README.md)
- [`docs/workflow-contract.md`](docs/workflow-contract.md)
- [`docs/expression-rules.md`](docs/expression-rules.md)

## Validation

- `cargo run -p nils-market-cli -- --help`
- `cargo run -p nils-market-cli -- fx --help`
- `cargo run -p nils-market-cli -- crypto --help`
- `cargo run -p nils-market-cli -- expr --help`
- `cargo run -p nils-market-cli -- favorites --help`
- `cargo test -p nils-market-cli`

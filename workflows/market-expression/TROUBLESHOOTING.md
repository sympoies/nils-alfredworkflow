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

# Confirm defaults in workflow manifest
rg -n "MARKET_CLI_BIN|MARKET_DEFAULT_FIAT" workflows/market-expression/workflow.toml
```

## Common failures and actions

| Symptom                                        | Likely cause                                         | Action                                                                      |
| ---------------------------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------- |
| `market-cli binary not found` row              | Binary is absent in all lookup paths                 | Package workflow again or set `MARKET_CLI_BIN` to executable absolute path. |
| `Unsupported operator` row                     | Asset expression used `*` or `/`                     | Use `+`/`-` for asset terms. Keep `*`/`/` for numeric-only expressions.     |
| `Invalid expression terms` row                 | Mixed raw numeric and asset terms in same expression | Use a single expression type per side (all numeric or all asset terms).     |
| `Invalid to-clause` row                        | Missing/incomplete `to <FIAT>` target                | Use complete target clause, e.g. `1 BTC + 2 ETH to USD`.                    |
| `provider failure` or transient runtime errors | Upstream provider/API issue                          | Retry after a short delay; do not assume local script defect first.         |

Syntax probe example (safe, no clipboard action):

```bash
bash workflows/market-expression/scripts/script_filter.sh "1 BTC * 2 ETH" | jq -r '.items[0].title, .items[0].subtitle'
```

## Validation

```bash
bash workflows/market-expression/tests/smoke.sh
scripts/workflow-test.sh --id market-expression
scripts/workflow-pack.sh --id market-expression
```

## Rollback guidance

1. Re-install the previous known-good package from `dist/market-expression/<version>/`.
2. Restore workflow variables to defaults (`MARKET_CLI_BIN=""`, `MARKET_DEFAULT_FIAT="USD"`) and retest.
3. If issue persists, roll back only `workflows/market-expression/` on a branch, then run all Validation commands before
   release.

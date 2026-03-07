# Market Expression Rules (Alfred v1)

## Purpose

Define the input and calculation rules for Alfred FX/crypto expressions. This document is the implementation and testing
contract for the upcoming workflow.

## Operators and Syntax

- Supported operators:
  - Numeric mode: `+`, `-`, `*`, `/`
  - Asset mode: `+`, `-`
- Unsupported operators:
  - Asset mode with `*`, `/` is a syntax error
- Target fiat syntax: append `to <fiat>` at the end, for example `to jpy`
- Default target fiat: `USD` when `to <fiat>` is not provided
- Asset-term shorthand is accepted: `1btc` is normalized as `1 btc`, `3eth` as `3 eth`

### EBNF (v1)

```text
expression      = asset_expression [target] | numeric_expression ;
target          = "to" WS fiat ;
asset_expression = asset_term { WS? ("+" | "-") WS? asset_term } ;
numeric_expression = number { WS? ("+" | "-" | "*" | "/") WS? number } ;
asset_term      = number [WS] asset ;
fiat            = /[a-zA-Z]{3}/ ;
asset           = /[a-zA-Z0-9]{2,10}/ ;
number          = signed_decimal ;
```

## Mode Resolution

1. If all terms are numeric only (for example `1+5`):

- Use numeric mode.
- Return exactly one output line (final result only).

1. If all terms are asset terms (for example `1 btc + 3 eth`):

- Use asset conversion mode.
- Show per-asset unit price lines first, then the final total line.
- Compact form without spaces is accepted (for example `1btc + 3eth`).

1. If numeric terms and asset terms are mixed (for example `2 btc + 5`):

- Treat as syntax error (no calculation).

## Calculation Rules

### Numeric Mode

- Evaluate `+`, `-`, `*`, `/` left-to-right.
- Division by zero is treated as syntax/user error.

### Asset Conversion Mode

1. Fetch the `1 unit` price of each unique asset against the target fiat.
2. Convert each asset term into target fiat, then apply `+` and `-`.
3. Show each unique asset unit-price line once only (for example `1 btc + 3 btc` shows one `1 BTC = ...` line).

## Alfred Output Rules

### A. Numeric Mode (1 line)

- Show final result only.
  - Input: `1+5`
  - Output: `6`

### B. Asset Conversion Mode (multiple lines)

Example: `1 btc + 3 eth to jpy`

1. `1 BTC = xxx JPY`
2. `1 ETH = yyy JPY`
3. `Total = 1*xxx + 3*yyy = zzz JPY`

Example (compact input): `1btc + 3eth to jpy`

1. Input is normalized as `1 btc + 3 eth to jpy`
2. Output rows follow the same structure as regular asset mode.

Example: `1 btc + 3 btc` (without `to`)

1. `1 BTC = xxx USD`
2. `Total = (1+3)*xxx = zzz USD`

## Display Formatting (Rounding)

- Perform full-precision calculation first, then apply display rounding as the final step.
- Decimal places are determined by absolute value `|x|`:
  - `|x| < 10`: show 3 decimal places
  - `10 <= |x| < 100`: show 2 decimal places
  - `100 <= |x| < 1000`: show 1 decimal place
  - `|x| >= 1000`: show no decimal places
- Rounding rule: `half-up`

Examples:

- `9.8764 -> 9.876`
- `9.8765 -> 9.877`
- `12.345 -> 12.35`
- `456.78 -> 456.8`
- `1234.56 -> 1235`

## Error Conditions

- Asset expression uses unsupported operators (`*`, `/`).
- Asset and numeric terms are mixed (for example `2 btc + 5`).
- Invalid `number` / `asset` / `fiat` tokens.
- Incomplete `to` clause (for example `1 btc + 2 eth to`).
- Asset or FX pricing data cannot be fetched (provider error).
- Division by zero in numeric mode.

## Recommended Test Cases

- `1+5` -> numeric mode with one-line output.
- `8/2*3` -> numeric mode with one-line output (`12`).
- `10/0` -> syntax/user error.
- `1 btc + 3 eth to jpy` -> 3 lines (2 unit-price lines + 1 total line).
- `1btc + 3eth to jpy` -> same as spaced form (3 lines).
- `1 btc + 3 btc` -> 2 lines (1 unit-price line + 1 total line).
- `2 btc + 5` -> syntax error.
- `1 btc * 2 eth` -> syntax error.

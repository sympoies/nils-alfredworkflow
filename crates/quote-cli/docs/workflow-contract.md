# Quote Feed Workflow Contract

## Keyword and Query Handling

- Keyword: `qq`
- Query input is optional.
- Empty query: return up to `QUOTE_DISPLAY_COUNT` quotes from local cache.
- Non-empty query: filter cache by case-insensitive substring and return up to `QUOTE_DISPLAY_COUNT` matches.
- If no cached quotes exist, return one non-actionable fallback item (valid=`false`).
- If query has no matches, return one non-actionable fallback item (valid=`false`).

## Alfred Item JSON Contract

Success item contract:

- `title`: full quote text in canonical format (`"<quote>" — <author>`)
- `subtitle`: static guidance such as `Press Enter to copy quote.`
- `arg`: same quote text for copy action
- `valid`: `true`

Fallback/error item contract:

- `title`: human-friendly status/error title
- `subtitle`: recovery guidance
- `valid`: `false`
- `arg`: omitted

All `script_filter.sh` code paths must emit valid Alfred JSON:

- root object with `items` array
- no raw stderr mixed into stdout JSON payload

## Cache Lifecycle and Refresh Policy

- Cache is local-first and served immediately from quote storage.
- Runtime storage location:
  - preferred when set: `$QUOTE_DATA_DIR/quotes.txt` and `$QUOTE_DATA_DIR/quotes.timestamp`
  - otherwise preferred: `$ALFRED_WORKFLOW_DATA/quotes.txt` and `$ALFRED_WORKFLOW_DATA/quotes.timestamp`
  - fallback: `${TMPDIR:-/tmp}/nils-quote-feed/quotes.txt` and `${TMPDIR:-/tmp}/nils-quote-feed/quotes.timestamp`
- Refresh decision:
  - refresh runs only when `now - last_refresh > QUOTE_REFRESH_INTERVAL`.
- Refresh behavior:
  - fetch up to `QUOTE_FETCH_COUNT` quotes from ZenQuotes (`https://zenquotes.io/api/random`), one request per quote.
  - append only valid `q` + `a` entries.
  - deduplicate exact quote lines.
  - trim to latest `QUOTE_MAX_ENTRIES` lines.
  - update timestamp only after a successful cache write.
- On network/API/parsing failure:
  - keep existing cache unchanged
  - return cached quotes when available

## Migration Note (`quote-init.zsh` storage vs workflow storage)

Legacy storage definitions come from `/Users/terry/.config/zsh/bootstrap/quote-init.zsh`.

| Storage concern | Legacy bootstrap storage | New quote-feed workflow storage |
| --- | --- | --- |
| Quotes file | `$ZDOTDIR/assets/quotes.txt` | preferred `$QUOTE_DATA_DIR/quotes.txt` (when set), otherwise `$ALFRED_WORKFLOW_DATA/quotes.txt`; fallback `${TMPDIR:-/tmp}/nils-quote-feed/quotes.txt` |
| Refresh timestamp | `$ZSH_CACHE_DIR/quotes.timestamp` | preferred `$QUOTE_DATA_DIR/quotes.timestamp` (when set), otherwise `$ALFRED_WORKFLOW_DATA/quotes.timestamp`; fallback `${TMPDIR:-/tmp}/nils-quote-feed/quotes.timestamp` |
| Runtime trigger | Shell login init (`zsh`) | Alfred keyword runtime (`qq`) |

Migration guidance:

- v1 does not auto-import legacy files from shell bootstrap paths.
- To keep historical cache, copy legacy files into the active workflow data directory before first run.
- If not copied, workflow starts with empty cache and rebuilds from ZenQuotes refreshes.

## Error Mapping

- Invalid `QUOTE_*` config -> user configuration error item.
- Missing `quote-cli` binary -> setup error item.
- API/network timeout/unavailable -> non-actionable runtime warning item when no cache exists; otherwise keep cached output.
- Empty fetch payload -> treated as no-op refresh, not a hard failure.

## Environment Variables and Defaults

| Variable | Required | Default | Validation / Effective range |
| --- | --- | --- | --- |
| `QUOTE_DISPLAY_COUNT` | No | `3` | base-10 integer, clamped to `1..20` |
| `QUOTE_REFRESH_INTERVAL` | No | `1h` | required format: `<positive-int><s\|m\|h>` |
| `QUOTE_FETCH_COUNT` | No | `5` | base-10 integer, clamped to `1..20` |
| `QUOTE_MAX_ENTRIES` | No | `100` | base-10 integer, clamped to `1..1000` |
| `QUOTE_DATA_DIR` | No | `(empty)` | non-empty path overrides quote cache directory |

Advanced runtime override:

- `QUOTE_CLI_BIN` (optional): absolute executable path override for local/debug runtime.

## Constraints and Non-Goals

- No authenticated quote providers in v1.
- No background daemon; refresh runs synchronously inside `quote-cli` invocation.
- No schema migration from shell bootstrap files in v1 (migration guidance is documentation-only).

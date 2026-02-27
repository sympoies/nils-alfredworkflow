# Memo Add Workflow Contract

## Goal

Provide a capture-first Alfred workflow for quick memo insertion backed by `nils-memo-cli@0.5.5`.

## Primary user behavior

- Keywords: `mm`, `mmr`, `mma`, `mmu`, `mmd`, `mmc`, `mmq`
- `mm` -> command entry menu that appends one-letter suffix and switches directly to `mmr` / `mma` / `mmu` / `mmd` /
  `mmc` / `mmq`.
- `mmr` -> forces empty-query rendering and shows recent memo rows in newest-first order.
- `mmr <number>` -> routes to `item <number>` for memo item action menu.
- `mmu` / `mmd` / `mmc` -> default to same newest-first recent list behavior as `mmr`.
- `mmu <number>` -> routes to update flow for that id (single update row, not full item menu).
- `mmd <number>` -> routes to delete flow for that id (single delete row, not full item menu).
- `mmc <number>` -> routes to copy flow for that id (single copy row, not full item menu).
- `mma buy milk` -> script-filter returns actionable add row -> action runs add and persists one inbox record.
- `mmu itm_00000001 buy oat milk` -> script-filter returns actionable update row.
- `mmd itm_00000001` -> script-filter returns actionable delete row.
- `mmc itm_00000001` -> script-filter returns actionable copy row.
- `mmq <query>` -> search rows are always non-actionable and route with `autocomplete=item <item_id>`.
- Enter on a search row routes to `item <item_id>` and opens full item action menu (`copy` / `update` / `delete`).
- choose `Copy` row (from `mmr <id>` item menu) -> Enter copies memo text; `Cmd` modifier switches action to copy raw
  JSON for that item.
- choose `Update` row (from `mmr <id>` item menu) -> query autocompletes to `update <item_id>`; type new text and press
  Enter to execute update.
- `mma <text>` routes to add intent.
- `mmu <item_id> <text>` routes to update intent.
- `mmd <item_id>` routes to delete intent.
- `mmc <item_id>` routes to copy intent.
- `mmq <query>` routes to search intent (`search <query>`).

## Runtime commands

The workflow runtime binary is `memo-workflow-cli` with these commands:

- `script-filter --query <text>`: returns Alfred JSON.
- `action --token <token>`: executes workflow action token.
- `add --text <text>`: direct add operation (for debug/manual use).
- `update --item-id <id> --text <text>`: direct update operation (for debug/manual use).
- `delete --item-id <id>`: direct delete operation (for debug/manual use).
- `db-init`: direct db initialization operation (for debug/manual use).
- `list --limit <n> --offset <n>`: direct newest-first memo query (for debug/manual use).
- `search --query <text> --match <fts|prefix|contains> --limit <n> --offset <n>`: direct memo search (`fts` default;
  `prefix` and `contains` optional for debug/manual use).

## Action token contract

- `db-init`: initialize sqlite database and schema.
- `add::<raw-text>`: add one memo with raw text payload.
- `update::<item-id>::<raw-text>`: update one memo row by item id.
- `delete::<item-id>`: delete one memo row by item id.
- `copy::<item-id>`: output memo text for clipboard copy path.
- `copy-json::<item-id>`: output raw memo JSON row for clipboard copy path.

`update` token parsing splits only the first two `::` delimiters, so update text keeps raw suffix bytes. Malformed
update/delete token shapes are handled as user errors.

`action_run.sh` forwards selected Alfred `arg` token into `memo-workflow-cli action --token`.

## Workflow parameters

| Variable                | Default    | Required | Notes                                                                         |
| ----------------------- | ---------- | -------- | ----------------------------------------------------------------------------- |
| `MEMO_DB_PATH`          | `""`       | No       | Empty: use Alfred workflow data dir + `memo.db`; otherwise use explicit path. |
| `MEMO_SOURCE`           | `"alfred"` | No       | Source label stored in `inbox_items.source`. Must be non-empty after trim.    |
| `MEMO_REQUIRE_CONFIRM`  | `"0"`      | No       | Truthy (`1/true/yes/on`) adds explicit confirm row before add action.         |
| `MEMO_MAX_INPUT_BYTES`  | `"4096"`   | No       | Max input bytes for one memo. Integer range `1..=1048576`.                    |
| `MEMO_RECENT_LIMIT`     | `"8"`      | No       | Count of recent rows shown for empty query. Integer range `1..=50`.           |
| `MEMO_SEARCH_MATCH`     | `"fts"`    | No       | Default search match mode for `search <query>` (`fts`, `prefix`, `contains`). |
| `MEMO_WORKFLOW_CLI_BIN` | `""`       | No       | Optional absolute binary override for workflow runtime.                       |

## DB init semantics

- `db init` is idempotent.
- First run creates parent directory and sqlite file if missing.
- Repeated runs keep schema stable and return success.
- Runtime should surface readable errors for permission/path failures.

## Add semantics

- Input text is trimmed before validation.
- Empty text is rejected as usage/user error.
- Oversize text (> `MEMO_MAX_INPUT_BYTES`) is rejected as usage/user error.
- Success path persists one row and returns item id/timestamp acknowledgment.

## Update semantics

- Query intent form: `update <item_id> <new text>`.
- Requires valid `item_id` and non-empty update text.
- Invalid `item_id` or malformed update syntax is rejected as usage/user error.
- Success path updates target row text and returns updated metadata acknowledgment.

## Delete semantics

- Query intent form: `delete <item_id>`.
- Delete uses hard-delete semantics (row is permanently removed; no soft-delete/undo path).
- Invalid/missing `item_id` or malformed delete syntax is rejected as usage/user error.
- Success path returns deletion acknowledgment for the target item id.

## Query semantics

- Empty query with existing db includes a recent-records section so users can verify latest captures immediately.
- Recent records default to `MEMO_RECENT_LIMIT=8` and are ordered by `created_at DESC`, then `item_id DESC`.
- Recent rows are informational (`valid=false`) but include `autocomplete=item <number>` for item-level action routing.
- Recent/search row titles render short item refs (`#<number>`) for readability; internal action tokens remain canonical
  `itm_XXXXXXXX`.
- `item <item_id>` intent renders an action menu in order: copy (action token) + update (autocomplete) + delete (action
  token).
- Additional script-filters:
  - `mm` renders command-entry rows only (no query intent execution).
  - `mmr` forwards empty/non-numeric query to newest-first recent rows.
  - `mmr <number>` forwards numeric query to `item <number>` lookup.
  - `mmr` passes through explicit intents (`item|update|delete|copy|search`) so Enter on autocomplete rows can continue
    multi-step flows.
  - `mma` forwards query to default add intent.
  - `mmu` forwards empty query to newest-first recent rows, otherwise prepends `update` before forwarding query.
  - `mmd` forwards empty query to newest-first recent rows, otherwise prepends `delete` before forwarding query.
  - `mmc` forwards empty query to newest-first recent rows, otherwise prepends `copy` before forwarding query.
- `mmq` defaults to prepending `search` for plain query text (`MEMO_SEARCH_MATCH` controls default match mode when query
  does not include `--match`), but passes through explicit intents (`item|update|delete|copy|search`) for multi-step
  manage flow.
- Copy row title includes text preview for the default copy payload (overflow moves to subtitle).
- Copy row also provides a `cmd` modifier action token (`copy-json::<item_id>`) with JSON preview subtitle.
- `update <item_id>` without text renders guidance/autocomplete instead of hard error row.
- `search` without query text renders guidance row and no executable action token.
- `search <query>` always returns non-destructive rows with `autocomplete=item <number>`.
- `search --match <fts|prefix|contains> <query>` is accepted for optional match mode override (default `fts`).
- db path row is informational (`valid=false`), while `db init` stays actionable when db is missing.
- Non-empty query defaults to add unless explicit `update` / `delete` / `copy` / `search` intent prefix is matched (for
  keyword wrappers / internal script-filter paths).
- Malformed mutation query syntax returns non-actionable guidance rows instead of malformed JSON.

## Error mapping

- Config/user validation failures -> exit code `2`.
- Runtime/storage failures -> exit code `1`.
- `script_filter.sh` always returns Alfred JSON; on runtime errors it emits non-actionable fallback rows.

## Validation checklist

- `cargo run -p nils-memo-workflow-cli -- script-filter --query "buy milk" | jq -e '.items | type == "array"'`
- `cargo run -p nils-memo-workflow-cli -- script-filter --query "" | jq -e '.items | type == "array" and length >= 2'`
- `cargo run -p nils-memo-workflow-cli -- script-filter --query "update itm_00000001 revised text" | jq -e '.items[0].arg | startswith("update::")'`
- `cargo run -p nils-memo-workflow-cli -- script-filter --query "delete itm_00000001" | jq -e '.items[0].arg | startswith("delete::")'`
- `cargo run -p nils-memo-workflow-cli -- script-filter --query "search milk" | jq -e '.items | length == 3 and .items[0].arg | startswith("copy::")'`
- `cargo run -p nils-memo-workflow-cli -- db-init`
- `cargo run -p nils-memo-workflow-cli -- add --text "buy milk"`
- Update flow check:

  ```bash
  tmpdir="$(mktemp -d)" && db="$tmpdir/memo.db" && \
    add_json="$(cargo run -p nils-memo-workflow-cli -- add --db "$db" --text "before" --mode json)" && \
    item_id="$(jq -r '.result.item_id' <<<"$add_json")" && \
    cargo run -p nils-memo-workflow-cli -- update --db "$db" --item-id "$item_id" --text "after" --mode json
  ```

- Delete flow check:

  ```bash
  tmpdir="$(mktemp -d)" && db="$tmpdir/memo.db" && \
    add_json="$(cargo run -p nils-memo-workflow-cli -- add --db "$db" --text "to-delete" --mode json)" && \
    item_id="$(jq -r '.result.item_id' <<<"$add_json")" && \
    cargo run -p nils-memo-workflow-cli -- delete --db "$db" --item-id "$item_id" --mode json
  ```

- `cargo run -p nils-memo-workflow-cli -- list --limit 8 --mode json`
- Search flow check:

  ```bash
  tmpdir="$(mktemp -d)" && db="$tmpdir/memo.db" && \
    cargo run -p nils-memo-workflow-cli -- add --db "$db" --text "search target" >/dev/null && \
    cargo run -p nils-memo-workflow-cli -- search --db "$db" --query "target" --mode json \
      | jq -e '.ok == true and (.result | length) >= 1'
  ```

- `bash workflows/memo-add/tests/smoke.sh`

# Script Filter Input Policy (Shared)

## Defaults

- `queue_delay_seconds`: `1 second`
- `queue_delay_mode`: `0`
- `queue_delay_custom`: `1`
- `queuedelayimmediatelyinitially`: `false`
- `min_query_chars`: `2`

## Canonical Mapping

For the target workflows in this repository, the 1-second Script Filter delay policy is encoded as:

- `<key>queuedelaymode</key><integer>0</integer>`
- `<key>queuedelaycustom</key><integer>1</integer>`
- `<key>queuedelayimmediatelyinitially</key><false/>`

This means:

- first-character immediate execution is disabled;
- queue delay defaults to 1 second;
- backend/expensive branches should require at least 2 query characters.

## Evidence Notes

- Alfred Script Filter UI exposes queue delay and immediate-run behavior as separate controls; plist stores both
  controls via `queuedelay*` keys.
- Local Alfred-exported workflows under
  `~/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows/*/info.plist` use the same key family
  (`queuedelaycustom`, `queuedelaymode`, `queuedelayimmediatelyinitially`) with integer delay values (`1`, `2`, `3`).
- Repository target templates currently use `queuedelaycustom=3` and `queuedelayimmediatelyinitially=true`; this policy
  standardizes them to 1-second delay + no immediate first run.

## Target Scope

The single source of truth for target workflow/object scope is:

- `docs/specs/script-filter-input-policy.json`

The shared helper runtime contract is:

- source: `scripts/lib/script_filter_query_policy.sh`
- package destination: `scripts/lib/script_filter_query_policy.sh`

## Shared Foundation Policy Check

`scripts/workflow-sync-script-filter-policy.sh --check` now validates two layers:

- queue policy parity (`queuedelay*` fields) for `targets.*.object_uids`;
- shared foundation wiring for `shared_foundation.targets`.

Shared foundation policy checks enforce that designated script filters include:

- helper loader wiring (`workflow_helper_loader.sh` + `wfhl_source_helper`);
- search-driver wiring (`script_filter_search_driver.sh` + `sfsd_run_search_flow`) for search-family targets;
- CLI-driver safety wiring (`script_filter_cli_driver.sh` + `sfcd_run_cli_flow`) for non-search and hybrid targets.

The same check also rejects prohibited placeholder markers so incomplete migration scaffolding fails fast in CI/local
lint.

## Auto-Syncable Vs Manual Fields

Auto-syncable by `--apply`:

- `queuedelaycustom`
- `queuedelaymode`
- `queuedelayimmediatelyinitially`

Manual-by-design (validated by policy check, not auto-rewritten):

- `shared_foundation.targets.*.script_filter`
- `shared_foundation.targets.*.requires`
- `shared_foundation.profiles.*.required_markers`

Reason: these fields describe code-level shared foundation boundaries (helper loader + guard contracts) and must be
updated together with script changes during migration PRs.

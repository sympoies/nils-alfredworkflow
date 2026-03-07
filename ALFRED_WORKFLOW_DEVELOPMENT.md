# Alfred Workflow Development Standard

## Purpose

This document defines repository-wide Alfred workflow development and troubleshooting standards. Use this file for
cross-workflow runtime rules and global operator playbooks.

## Documentation Ownership Model

### Layer 1: Global standards (this file)

In scope:

- Cross-workflow runtime behavior and standards (Script Filter contract, queue policy, Gatekeeper handling).
- Shared troubleshooting procedures reusable across workflows.
- Governance policy for documentation ownership and migration rules.

Out of scope:

- Workflow-specific API failure handling or workflow-specific variable semantics.
- Workflow-specific smoke command variants that only apply to one workflow.

### Layer 2: Workflow-local troubleshooting

Location:

- `workflows/<workflow-id>/TROUBLESHOOTING.md`

In scope:

- Workflow-specific `Quick operator checks`, `Common failures and actions`, `Validation`, and `Rollback guidance`.
- Workflow-specific operator commands, runtime overrides, and known failure signatures.

Out of scope:

- Repository-wide standards duplicated verbatim across all workflows.

### Layer 3: Development flow, packaging, and quality gates

Location:

- `DEVELOPMENT.md`
- `docs/PACKAGING.md`

In scope:

- Build/lint/test/coverage commands and contribution gate expectations in `DEVELOPMENT.md`.
- Packaging/install/macOS acceptance entrypoints in `docs/PACKAGING.md`.
- CI-oriented quality requirements and commit-time checks.

Out of scope:

- Detailed troubleshooting knowledge base content.

### No-duplication migration rule

- During documentation migration, keep one canonical owner per operational fact.
- Do not mirror workflow-specific details into this global file when the workflow-local docs already own them.
- If content is stale, duplicated, or conflicts with workflow-local docs, remove it instead of copying it forward.
- Reintroducing a central workflow-details encyclopedia is disallowed.
- Keep file-level owner/retention decisions in the owning canonical docs (`README.md`, `DEVELOPMENT.md`,
  `docs/PACKAGING.md`, and `docs/ARCHITECTURE.md`).

## Troubleshooting Routing Policy

- Use a two-layer troubleshooting route:
  - Global standards and shared playbooks: `ALFRED_WORKFLOW_DEVELOPMENT.md`
  - Workflow-specific runbooks: `workflows/<workflow-id>/TROUBLESHOOTING.md`
- Routing rules:
  1. Cross-workflow failures (Script Filter contract, queue policy, packaging wiring, Gatekeeper) start from global
     standards.
  2. Workflow-specific failures (keyword flow, workflow env vars, provider/API behavior) start from workflow-local
     troubleshooting.
  3. If scope is unclear, start global then jump to the workflow-local `Quick operator checks`.
- Navigation shortcuts:
  - List workflow-local troubleshooting docs: `rg --files workflows | rg 'TROUBLESHOOTING\.md$'`
  - Open target runbook: `workflows/<workflow-id>/TROUBLESHOOTING.md`

## Workflow Onboarding And Packaging Governance

### Add a new workflow

1. `scripts/workflow-new.sh --id <workflow-id>`
2. Edit `workflows/<workflow-id>/workflow.toml`.
3. Update `workflows/<workflow-id>/scripts/*.sh` adapters.
4. Implement or reuse shared logic in `crates/workflow-common` where applicable.
5. Validate and package:
   - `scripts/workflow-lint.sh --id <workflow-id>`
   - `scripts/workflow-test.sh --id <workflow-id>`
   - `scripts/workflow-pack.sh --id <workflow-id> --install`

### Manifest contract

Required keys in `workflow.toml`:

- `id`
- `name`
- `bundle_id`
- `version`
- `script_filter`
- `action`

Optional keys:

- `rust_binary`
- `assets`
- `readme_source`

### README sync during packaging

- `scripts/workflow-pack.sh` auto-syncs workflow README into packaged `info.plist` readme when
  `workflows/<id>/README.md` exists.
- `readme_source` can override the source path (relative to workflow root) when README is not at the default location.
- Pack runs `nils-workflow-readme-cli convert` to copy README content into packaged `info.plist`.
- Markdown tables are normalized during sync, so packaged Alfred readme should not contain `|---|` separators.
- If README references local images (for example `./screenshot.png`), keep those files in workflow root so packaging can
  stage them into `build/workflows/<id>/pkg/`.
- Validation command:

  ```bash
  bash -c 'scripts/workflow-pack.sh --id codex-cli && \
    plutil -convert json -o - build/workflows/codex-cli/pkg/info.plist \
      | jq -r ".readme" \
      | rg -n "# Codex CLI - Alfred Workflow|\\|---\\|"'
  ```

## Shared Troubleshooting Standards

Use these standards in all workflow troubleshooting documents.

### Required sections for each workflow troubleshooting file

Every `workflows/<workflow-id>/TROUBLESHOOTING.md` must include:

- `## Quick operator checks`
- `## Common failures and actions`
- `## Validation`
- `## Rollback guidance`

### Script Filter JSON contract

- Script Filter scripts must always return valid Alfred JSON, including failure paths.
- Fallback errors should be non-actionable rows (`valid=false`) with clear operator guidance.
- Keep payload arguments newline-safe for action-chain handoff.

### `alfredfiltersresults` guardrail

- Keep `alfredfiltersresults=false` when Script Filter output is fully controlled by script JSON.
- Do not enable Alfred secondary filtering unless there is an explicit functional need.
- Validation command:

  ```bash
  plutil -convert json -o - build/workflows/<workflow-id>/pkg/info.plist \
    | jq -e '(.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.alfredfiltersresults) == false'
  ```

### `config.type` and `scriptfile` guardrail

- Script Filter and script action nodes that use external files must set `config.type=8`.
- Validate expected `scriptfile` wiring in installed `info.plist` before issue triage.

### Script Filter queue policy

- Keep queue behavior synchronized with repository policy tooling.
- Validation command:
  - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows <workflow-id>`
- Remediation command:
  - `bash scripts/workflow-sync-script-filter-policy.sh --apply --workflows <workflow-id>`
- Policy check scope:
  - queue-delay fields for `object_uids`
  - shared foundation wiring (`workflow_helper_loader`, search/CLI driver markers) for designated workflows

### Shared Script Filter helper libraries (`scripts/lib`)

- Shared Script Filter runtime helpers are:
  - `scripts/lib/script_filter_query_policy.sh` (`sfqp_*`)
  - `scripts/lib/script_filter_async_coalesce.sh` (`sfac_*`)
- Shared foundation bootstrap helpers are:
  - `scripts/lib/workflow_helper_loader.sh` (`wfhl_*`)
  - `scripts/lib/script_filter_cli_driver.sh` (`sfcd_*`)
  - `scripts/lib/workflow_smoke_helpers.sh`
- Shared path resolver helper is:
  - `scripts/lib/workflow_cli_resolver.sh` (`wfcr_*`)
- Bundled runtime adapters that execute files under `../bin/*` must:
  - source `workflow_cli_resolver.sh`
  - resolve runtime candidates through `wfcr_resolve_binary`
- Additional workflow runtime helpers may live in `scripts/lib/` (for example resolver/error/driver helpers) when they
  are runtime mechanics rather than domain behavior.
- `scripts/workflow-pack.sh` must stage `scripts/lib/*.sh` into packaged workflows at `scripts/lib/` via a deterministic
  rule (no per-file ad hoc list).
- Script Filter adapters should resolve packaged helper first, then local-repo fallback for development/tests.
- If a required helper file cannot be resolved at runtime, emit a non-crashing Alfred error item (`valid=false`) and
  exit successfully (`exit 0`).
- Resolver policy validation command:
  - `bash scripts/workflow-cli-resolver-audit.sh --check`

### Path config expansion standard

- Any workflow shell adapter that accepts path-like env overrides (for example `*_CLI_BIN`, `*_PATH`, `*_DIR`, `*_FILE`)
  must normalize `~/...` via `wfcr_expand_home_path` before `-x`, `-f`, `-d`, or command execution checks.
- Avoid local ad-hoc `~` expansion snippets in workflow scripts when `wfcr_expand_home_path` is available.
- Rust config parsers for path-like env values must expand `~/...` (and keep behavior consistent with shell adapters)
  before building `PathBuf` values.

### `scripts/lib` extraction boundary

- Extract to `scripts/lib` only when logic is both:
  - Cross-workflow runtime mechanics (for example query normalization, cache/coalesce orchestration, binary resolver
    plumbing, JSON-safe emitters).
  - Repeated in multiple workflows with identical or near-identical behavior.
- Keep local in workflow scripts when logic is:
  - Product/domain semantics (API-specific error mapping, ranking, rendering phrasing, business policy).
  - Workflow-specific UX behavior that intentionally diverges.
- Prefer thin local adapters over generic mega-helpers: shared helpers should expose deterministic primitives, while
  each workflow keeps its own domain rules and copy.
- Canonical shared foundation extraction boundary and migration/rollback constraints:
  - `docs/specs/workflow-shared-foundations-policy.md`
- Lint enforcement hook for migrated workflow families:
  - `bash scripts/workflow-shared-foundation-audit.sh --check`

### Ordered config list parsing standard

- For workflow config/query lists that support comma/newline input (for example timezone IDs, language options), use the
  shared parser from `nils-workflow-common`:
  - `split_ordered_list`
  - `parse_ordered_list_with`
- Parsing rules are normative:
  - separators: comma (`,`) and newline (`\\n`)
  - trim per-token surrounding whitespace
  - ignore empty tokens
  - preserve non-empty token order exactly as provided
- Keep domain validation local to each workflow crate (for example IANA timezone parse, wiki language-code validation);
  shared parser owns tokenization/order only.
- When both query list and config list are supported, query-over-config precedence must preserve source order unchanged.
- Required coverage for workflows relying on ordered lists:
  - unit tests for parser/validator edge cases
  - workflow smoke assertions that emitted row order matches input/config order

### `sfqp_*` query policy usage standard

- Normalize input via `sfqp_resolve_query_input` and `sfqp_trim` before validation/backend calls.
- Enforce short-query guardrails with `sfqp_is_short_query` and return operator guidance via
  `sfqp_emit_short_query_item_json`.
- Keep JSON error rows newline-safe and non-actionable through helper emitters.

### `sfac_*` async coalesce usage standard

- Initialize workflow-scoped context before cache/coalesce operations:
  - `sfac_init_context "<workflow-id>" "<fallback-cache-dir>"`
- Resolve tunables with helper validators (avoid inline parsing):
  - cache TTL: `sfac_resolve_positive_int_env "<PREFIX>_QUERY_CACHE_TTL_SECONDS" "0"`
  - settle window: `sfac_resolve_non_negative_number_env "<PREFIX>_QUERY_COALESCE_SETTLE_SECONDS" "2"`
  - rerun interval: `sfac_resolve_non_negative_number_env "<PREFIX>_QUERY_COALESCE_RERUN_SECONDS" "0.4"`
- Async flow contract:
  1. Shared driver (`sfsd_run_search_flow`) checks cache before settle-window final-query coalescing.
  2. For live-typing suggest/search Script Filters, keep default cache TTL at `0` to avoid stale prefix hits.
  3. Shared coalesce helper must be queue-safe: settle-window checks are non-blocking and require the latest query to
     remain unchanged for `settle` seconds.
  4. If query is not final yet, return pending row via `sfac_emit_pending_item_json` with `rerun`.
  5. On backend completion, write cache via `sfac_store_cache_result` for both success (`ok`) and error (`err`) paths.

### `sfsd_run_search_flow` cache policy standard

- Workflows using `sfsd_run_search_flow` must treat `<PREFIX>_QUERY_CACHE_TTL_SECONDS` as opt-in.
- Do not hardcode non-zero TTL defaults in workflow scripts; rely on shared default `0`.
- If enabling non-zero TTL for a workflow:
  - Document the tradeoff in workflow `README.md` advanced runtime section.
  - Add smoke coverage for both paths:
    - default/unset TTL does not cache repeated same-query calls.
    - explicit non-zero TTL does cache repeated same-query calls.

### Workflow package/install command standard (macOS)

- Rebuild and install latest artifact:
  `scripts/workflow-pack.sh --id <workflow-id> --install`.
- Install latest already-built artifact from `dist/` without rebuilding:
  `scripts/workflow-pack.sh --id <workflow-id> --install-only`.
- Keep troubleshooting docs aligned to `scripts/workflow-pack.sh` options; removed wrappers must not be reintroduced.

### Installed-workflow debug checklist

1. Confirm the latest package was installed (`scripts/workflow-pack.sh --id <workflow-id> --install`).
2. Locate installed workflow directory by bundle id in Alfred preferences.
3. Inspect installed `info.plist` node runtime wiring (`type`, `scriptfile`, `connections`).
4. Execute installed scripts directly from workflow directory to isolate Alfred UI factors.
5. Reproduce and verify action-chain payload handoff with exact query/arg values.

### Gatekeeper and quarantine handling (macOS)

If a bundled binary is blocked by Gatekeeper (`Not Opened` / `Apple could not verify`):

```bash
WORKFLOW_DIR="$(for p in "$HOME"/Library/Application\ Support/Alfred/Alfred.alfredpreferences/workflows/*/info.plist; do
  [ -f "$p" ] || continue
  bid="$(plutil -extract bundleid raw -o - "$p" 2>/dev/null || true)"
  [ "$bid" = "<bundle-id>" ] && dirname "$p"
done | head -n1)"

[ -n "$WORKFLOW_DIR" ] || { echo "workflow not found"; exit 1; }
xattr -dr com.apple.quarantine "$WORKFLOW_DIR"
```

### Generic rollback principles

1. Stop rollout/distribution of the affected workflow.
2. Revert workflow-specific code and workflow-specific docs in one rollback changeset.
3. Rebuild and run repository validation gates (`scripts/workflow-lint.sh`, `scripts/workflow-test.sh`, packaging
   checks).
4. Republish known-good artifact and notify operators with scope/ETA.

### Shared foundation rollout operations

- Canonical staged rollout guide:
  - `docs/specs/workflow-shared-foundations-policy.md`
- Required rollout checkpoints:
  - canary workflows pass before promotion.
  - promotion criteria are recorded before each rollout stage.
  - stop condition triggers require immediate revert to known-good paths/artifacts.

## Troubleshooting Documentation Map

### Global standards

- Cross-workflow runtime and troubleshooting standards are defined in this file:
  - `ALFRED_WORKFLOW_DEVELOPMENT.md`
- Canonical shared foundation policy:
  - `docs/specs/workflow-shared-foundations-policy.md`

### Workflow-local runbooks

- `workflows/_template/TROUBLESHOOTING.md`
- `workflows/cambridge-dict/TROUBLESHOOTING.md`
- `workflows/codex-cli/TROUBLESHOOTING.md`
- `workflows/epoch-converter/TROUBLESHOOTING.md`
- `workflows/google-search/TROUBLESHOOTING.md`
- `workflows/imdb-search/TROUBLESHOOTING.md`
- `workflows/market-expression/TROUBLESHOOTING.md`
- `workflows/memo-add/TROUBLESHOOTING.md`
- `workflows/multi-timezone/TROUBLESHOOTING.md`
- `workflows/open-project/TROUBLESHOOTING.md`
- `workflows/quote-feed/TROUBLESHOOTING.md`
- `workflows/randomer/TROUBLESHOOTING.md`
- `workflows/spotify-search/TROUBLESHOOTING.md`
- `workflows/weather/TROUBLESHOOTING.md`
- `workflows/wiki-search/TROUBLESHOOTING.md`
- `workflows/youtube-search/TROUBLESHOOTING.md`

### Reference policy

- Active entry-point documents (`README.md`, `DEVELOPMENT.md`, `docs/PACKAGING.md`, `AGENT_DOCS.toml`, and
  `workflows/<workflow-id>/README.md`) must link to:
  - `ALFRED_WORKFLOW_DEVELOPMENT.md` for global standards.
  - `workflows/<workflow-id>/TROUBLESHOOTING.md` for workflow-specific operations.

## Rollout Rehearsal Checklist

A maintainer should complete the following flow in under three minutes:

1. Open `README.md` and follow troubleshooting navigation to global standards.
2. Jump from workflow README to local `TROUBLESHOOTING.md`.
3. Run `agent-docs resolve --context project-dev --strict --format checklist`.
4. Confirm rollback path in the target workflow's `Rollback guidance` section is actionable.

## Validation

- `agent-docs resolve --context startup --strict --format checklist`
- `agent-docs resolve --context project-dev --strict --format checklist`
- `rg -n "Troubleshooting|Validation|Rollback guidance" workflows/*/TROUBLESHOOTING.md`

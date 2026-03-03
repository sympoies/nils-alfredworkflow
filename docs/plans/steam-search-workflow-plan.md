# Plan: Steam Search Workflow

## Overview

This plan adds a new `steam-search` Alfred workflow that searches Steam games by keyword and opens selected app pages in
the browser. The workflow will support region-aware search and pricing context via a selectable region contract (`cc`),
while keeping behavior aligned with repository shared foundations. Delivery is split into sequential sprint gates (no
cross-sprint execution parallelism) so shared helper alignment lands before workflow and crate integration. The
implementation path prioritizes existing `scripts/lib` helpers and only introduces new shared helpers where reuse is
justified.

## Scope

- In scope: new `steam-search` workflow package, new `nils-steam-cli` crate, region selection UX, Steam Store search
  integration, shared-foundation policy/audit wiring, smoke/docs updates.
- In scope: region switch rows in Alfred results and action-level requery flow that re-runs the current query under a
  selected region.
- Out of scope: authenticated Steam user APIs, wishlist/library data, age-gate bypass, scraping SteamDB HTML,
  personalized recommendations.

## Assumptions (if any)

1. Store search uses Steam Store JSON endpoints (`/api/storesearch`, `/api/appdetails`) with region code via `cc` and
   language via `l`.
2. `steam-search` will follow existing search-workflow conventions (`script_filter_search_driver.sh`, queue delay
   policy, helper loader wiring).
3. Region selection contract follows the same action-driven requery pattern already proven in `wiki-search`, but
   extracted into shared helper primitives for reuse.
4. Workspace quality gates and required local tools from `DEVELOPMENT.md`/`BINARY_DEPENDENCIES.md` are available.

## Sprint 1: Shared Foundation Inventory And Region Requery Baseline

**Goal**: Lock a reusable region-requery foundation and scaffold `steam-search` with shared policies before crate
implementation. **Parallelization**: Task 1.1 first, tasks 1.2 and 1.3 run in parallel, task 1.4 integrates both
outputs. **PR Grouping Intent**: per-sprint **Execution Profile**: parallel-x2 (intended width: 2) **Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 16
- CriticalPathComplexity: 12
- MaxBatchWidth: 2
- OverlapHotspots: `docs/specs/script-filter-input-policy.json` and workflow audit scripts can conflict when policy
  wiring and scaffold registration are edited concurrently. **Demo/Validation**:
- Command(s): `plan-tooling validate --file docs/plans/steam-search-workflow-plan.md`,
  `bash scripts/workflow-shared-foundation-audit.sh --check`
- Verify: shared foundation docs/policies include `steam-search`, and no duplicate helper-loading patterns are
  introduced.

### Task 1.1: Publish Steam Search Source And Shared-Helper Adoption Contract

- **Location**:
  - `docs/specs/steam-search-workflow-contract.md`
  - `docs/specs/workflow-shared-foundations-policy.md`
- **Description**: Add a contract for Steam search source selection, region semantics, and explicit `scripts/lib`
  adoption priorities (must-use helpers vs workflow-local logic).
- **Dependencies**:
  - none
- **Complexity**: 4
- **Acceptance criteria**:
  - Contract defines endpoint usage, region parameter behavior, and fallback/error strategy without introducing scraping
    requirements.
  - Contract clearly separates shared mechanics from Steam-specific domain logic.
  - Shared-helper adoption matrix is explicit and traceable for `steam-search`.
- **Validation**:
  - `rg -n "steam|storesearch|region|shared helper|must stay local" docs/specs/steam-search-workflow-contract.md docs/specs/workflow-shared-foundations-policy.md`

### Task 1.2: Extract Shared Action Requery Helper For Region/Language Switch Workflows

- **Location**:
  - `scripts/lib/workflow_action_requery.sh`
  - `scripts/tests/workflow_action_requery.test.sh`
  - `workflows/wiki-search/scripts/action_open.sh`
- **Description**: Extract reusable action-side state persistence and Alfred requery primitives from `wiki-search` so
  `steam-search` can reuse the same mechanism.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 5
- **Acceptance criteria**:
  - Shared helper provides deterministic contracts for requery prefix parsing, state-file persistence, and requery
    trigger execution.
  - `wiki-search` action script migrates to the helper without changing existing behavior.
  - Helper tests cover valid payloads, invalid payloads, and fallback trigger paths.
- **Validation**:
  - `bash scripts/tests/workflow_action_requery.test.sh`
  - `bash workflows/wiki-search/tests/smoke.sh`
  - `shellcheck scripts/lib/workflow_action_requery.sh workflows/wiki-search/scripts/action_open.sh`

### Task 1.3: Scaffold Steam Workflow Skeleton With Shared-Foundation Wiring

- **Location**:
  - `workflows/steam-search/workflow.toml`
  - `workflows/steam-search/src/info.plist.template`
  - `workflows/steam-search/scripts/script_filter.sh`
  - `workflows/steam-search/scripts/action_open.sh`
  - `workflows/steam-search/tests/smoke.sh`
- **Description**: Create `steam-search` workflow skeleton from template and align it to search-workflow shared
  foundations (helper loader, search driver, open-url wrapper).
- **Dependencies**:
  - Task 1.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Manifest/plist include search keyword wiring and queue-delay policy defaults consistent with repo policy.
  - Script filter is wired to shared helper loader and search driver placeholders, not duplicated inline helper code.
  - Action script supports standard URL-open behavior and reserved requery path for region switching.
- **Validation**:
  - `test -d workflows/steam-search`
  - `rg -n '^id[[:space:]]*=[[:space:]]*"steam-search"$' workflows/steam-search/workflow.toml`
  - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows steam-search`
  - `shellcheck workflows/steam-search/scripts/script_filter.sh workflows/steam-search/scripts/action_open.sh`

### Task 1.4: Register Steam Workflow In Shared Foundation Policy And Audits

- **Location**:
  - `docs/specs/script-filter-input-policy.json`
  - `scripts/workflow-sync-script-filter-policy.sh`
  - `scripts/workflow-shared-foundation-audit.sh`
- **Description**: Add `steam-search` as a first-class shared-foundation/policy target so future drift is caught by
  standard gates.
- **Dependencies**:
  - Task 1.2
  - Task 1.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Policy includes queue-delay target and shared-foundation profile requirements for `steam-search`.
  - Shared-foundation audit covers `steam-search` script filter and action wrapper files.
  - Policy check/audit commands pass with `steam-search` included.
- **Validation**:
  - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows steam-search`
  - `bash scripts/workflow-shared-foundation-audit.sh --check`

## Sprint 2: Steam CLI Domain Implementation (Region-Aware Search)

**Goal**: Implement `nils-steam-cli` with deterministic config, Steam Store query/parse, and Alfred feedback contracts
including region-switch rows. **Parallelization**: Task 2.1 first, task 2.2 establishes config contract, tasks 2.3 and
2.4 run in parallel, task 2.5 integrates and hardens. **PR Grouping Intent**: per-sprint **Execution Profile**:
parallel-x2 (intended width: 2) **Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 16
- CriticalPathComplexity: 14
- MaxBatchWidth: 2
- OverlapHotspots: parallel task lane overlap risk centers on `crates/steam-cli/src/lib.rs` exports and shared type
  wiring. **Demo/Validation**:
- Command(s): `plan-tooling validate --file docs/plans/steam-search-workflow-plan.md`, `cargo test -p nils-steam-cli`
- Verify: CLI emits valid Alfred JSON for success/no-result/error cases and supports region-switch arg contract.

### Task 2.1: Add Steam CLI Crate Scaffold And Workspace Wiring

- **Location**:
  - `Cargo.toml`
  - `crates/steam-cli/Cargo.toml`
  - `crates/steam-cli/src/lib.rs`
  - `crates/steam-cli/src/main.rs`
- **Description**: Introduce `nils-steam-cli` crate scaffold with `search` command entrypoint and output mode contract
  aligned to existing workflow crates.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - Workspace includes `crates/steam-cli` member and compiles.
  - CLI help exposes `search` subcommand and `--mode` output option.
  - Command routing scaffold is in place, with full runtime error contract deferred to Task 2.5.
- **Validation**:
  - `cargo check -p nils-steam-cli`
  - `cargo run -p nils-steam-cli -- --help | rg -n "search|service-json|alfred"`

### Task 2.2: Implement Region-Centric Runtime Config Parsing

- **Location**:
  - `crates/steam-cli/src/config.rs`
  - `crates/steam-cli/src/main.rs`
- **Description**: Parse and validate `STEAM_REGION`, `STEAM_REGION_OPTIONS`, `STEAM_MAX_RESULTS`, and optional language
  override with normalized defaults.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 3
- **Acceptance criteria**:
  - Region code validation is strict and deterministic (2-letter country codes normalized to lowercase/uppercase per
    contract).
  - Region options preserve configured order while deduplicating.
  - Invalid env values return user-facing config errors compatible with workflow error mapping.
- **Validation**:
  - `cargo test -p nils-steam-cli config::tests`

### Task 2.3: Implement Steam Store Search Client And Response Parser

- **Location**:
  - `crates/steam-cli/src/steam_store_api.rs`
  - `crates/steam-cli/src/lib.rs`
- **Description**: Add blocking HTTP client layer for Steam Store search endpoint and robust parser for app
  id/title/price/platform fields used by workflow feedback.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Request builder always includes query, region (`cc`), and language (`l`) parameters from runtime config.
  - Parser handles empty/partial/malformed payloads with typed runtime errors.
  - HTTP non-2xx errors preserve status and useful message for workflow-side classification.
- **Validation**:
  - `cargo test -p nils-steam-cli steam_store_api::tests`

### Task 2.4: Implement Alfred Feedback Mapping And Region-Switch Item Contract

- **Location**:
  - `crates/steam-cli/src/feedback.rs`
  - `crates/steam-cli/src/lib.rs`
- **Description**: Map Steam search results into Alfred items, including current-region row and switch-region rows using
  a stable requery arg prefix.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Feedback includes deterministic switch rows ordered by configured region options.
  - Result items provide canonical Steam app URLs with region/language parameters.
  - Subtitle normalization and truncation rules are deterministic and single-line.
- **Validation**:
  - `cargo test -p nils-steam-cli feedback::tests`

### Task 2.5: Integrate CLI Flow, Error Mapping, And Contract Tests

- **Location**:
  - `crates/steam-cli/src/main.rs`
  - `crates/steam-cli/tests/cli_contract.rs`
  - `crates/steam-cli/README.md`
  - `crates/steam-cli/docs/workflow-contract.md`
  - `crates/steam-cli/docs/README.md`
- **Description**: Wire config + API + feedback in CLI execution flow, then lock behavior with contract tests and crate
  documentation.
- **Dependencies**:
  - Task 2.3
  - Task 2.4
- **Complexity**: 4
- **Acceptance criteria**:
  - `search` command returns valid Alfred JSON on success and deterministic stderr error messages on failures.
  - CLI contract tests cover empty query, invalid config, API failure, and success mapping.
  - README/docs document env vars, output contract, and region-switch semantics.
- **Validation**:
  - `cargo test -p nils-steam-cli`
  - `cargo test -p nils-steam-cli --test cli_contract`

## Sprint 3: Workflow Integration, Policy Hardening, And Acceptance Gates

**Goal**: Connect `steam-search` workflow scripts to `nils-steam-cli`, enforce policy gates, and ship complete operator
docs/tests. **Parallelization**: Task 3.1 first, tasks 3.2 and 3.3 run in parallel, task 3.4 is final integration gate.
**PR Grouping Intent**: per-sprint **Execution Profile**: parallel-x2 (intended width: 2) **Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 17
- CriticalPathComplexity: 13
- MaxBatchWidth: 2
- OverlapHotspots: `workflows/steam-search/scripts/action_open.sh` and policy/docs edits can conflict if region arg
  contract changes late. **Demo/Validation**:
- Command(s): `scripts/workflow-test.sh --id steam-search`,
  `scripts/workflow-pack.sh --id steam-search`
- Verify: packaged workflow includes runtime binaries/helpers, passes smoke tests, and opens Steam app URLs from Alfred
  args.

### Task 3.1: Wire Steam Workflow Script Filter And Action Flow To Shared Helpers

- **Location**:
  - `workflows/steam-search/scripts/script_filter.sh`
  - `workflows/steam-search/scripts/action_open.sh`
  - `workflows/steam-search/workflow.toml`
  - `workflows/steam-search/src/info.plist.template`
- **Description**: Implement workflow adapters to call `steam-cli search --mode alfred`, map backend errors to
  actionable rows, and support region requery arg handling via shared action helper.
- **Dependencies**:
  - Task 2.5
- **Complexity**: 5
- **Acceptance criteria**:
  - Script filter uses helper loader + search driver + query policy helpers; no duplicated coalesce/cache logic.
  - Workflow env contract includes `STEAM_REGION`, `STEAM_REGION_OPTIONS`, and `STEAM_MAX_RESULTS` with sensible
    defaults.
  - Action script includes both direct URL-open path and `steam-requery` dispatch path; behavior verification is owned
    by Task 3.2 smoke coverage.
- **Validation**:
  - `shellcheck workflows/steam-search/scripts/script_filter.sh workflows/steam-search/scripts/action_open.sh`
  - `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows steam-search`

### Task 3.2: Add Steam Workflow Smoke Coverage

- **Location**:
  - `workflows/steam-search/tests/smoke.sh`
  - `workflows/steam-search/scripts/script_filter.sh`
  - `workflows/steam-search/scripts/action_open.sh`
- **Description**: Add smoke tests covering manifest wiring, action-open contract, script-filter short-query guards,
  cache/coalesce behavior, and package artifact checks.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 5
- **Acceptance criteria**:
  - Smoke test validates plist object wiring (`scriptfile`, queue-delay, connection graph, user config variables).
  - Smoke test verifies region requery payload handling without requiring Alfred UI interaction.
  - Smoke test passes in local/dev layout and packaged layout resolution modes.
- **Validation**:
  - `bash workflows/steam-search/tests/smoke.sh`

### Task 3.3: Finalize Workflow Documentation And Repository Catalog Entries

- **Location**:
  - `workflows/steam-search/README.md`
  - `workflows/steam-search/TROUBLESHOOTING.md`
  - `README.md`
- **Description**: Publish operator docs for setup/region switching/troubleshooting and add `steam-search` to the
  repository workflow catalog.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Workflow README documents keyword, region variables, and runtime tuning parameters.
  - Troubleshooting runbook includes API/region failure signatures and remediation steps.
  - Root workflow table includes `steam-search` entry and setup requirements.
- **Validation**:
  - `rg -n "steam-search|STEAM_REGION|steam-requery" workflows/steam-search/README.md workflows/steam-search/TROUBLESHOOTING.md README.md`

### Task 3.4: Run End-To-End Gates And Packaging Acceptance

- **Location**:
  - `DEVELOPMENT.md`
  - `workflows/steam-search/README.md`
- **Description**: Execute required lint/test/pack checks for `steam-search` and record concise validation evidence for
  release confidence.
- **Dependencies**:
  - Task 3.2
  - Task 3.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Targeted workflow lint/test/pack commands pass with no policy/audit regressions.
  - Validation report records commands, pass/fail status, and artifact path.
  - Any non-blocking residual risk (for example upstream Steam endpoint volatility) is explicitly captured.
- **Validation**:
  - `scripts/workflow-lint.sh --id steam-search`
  - `scripts/workflow-test.sh --id steam-search`
  - `scripts/workflow-pack.sh --id steam-search`
  - `bash scripts/workflow-shared-foundation-audit.sh --check`

## Testing Strategy

- Unit: `nils-steam-cli` modules (`config`, `steam_store_api`, `feedback`) with deterministic fixtures and error-path
  assertions.
- Integration: CLI contract tests in `crates/steam-cli/tests/cli_contract.rs` for command I/O, exit code, and envelope
  compatibility.
- Workflow smoke: `workflows/steam-search/tests/smoke.sh` validating script contracts, plist wiring, policy compliance,
  and packaging artifact shape.
- Repository gates: targeted `scripts/workflow-lint.sh --id steam-search`,
  `scripts/workflow-test.sh --id steam-search`, and `scripts/workflow-pack.sh --id steam-search`,
  plus shared-foundation/policy checks.

## Risks & gotchas

- Steam Store endpoints used for search are operational but not guaranteed as stable public contracts; response schema
  drift is possible.
- Region restrictions can yield zero results or changed price fields; workflow messaging must distinguish empty results
  from transport/runtime errors.
- Requery flow depends on Alfred automation trigger behavior; helper must keep deterministic fallback/error handling for
  non-Alfred test environments.
- Policy/audit registration drift can cause CI failures if scaffold and policy updates land out of sync.

## Rollback plan

1. Revert `steam-search` workflow directory and `nils-steam-cli` crate in one rollback changeset.
2. Revert policy/audit registrations (`docs/specs/script-filter-input-policy.json`, audit scripts, README catalog
   entries) to the last known-good state.
3. Restore `wiki-search` action flow to pre-extraction behavior only if shared requery helper rollback is required.
4. Re-run validation gates: `scripts/workflow-lint.sh --id wiki-search`,
   `scripts/workflow-test.sh --id wiki-search`, `bash scripts/workflow-shared-foundation-audit.sh --check`.
5. Re-package unaffected workflows and publish rollback note with scope and residual risk.

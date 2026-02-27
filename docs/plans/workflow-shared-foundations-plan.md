# Plan: Workflow Shared Foundations

## Overview

This plan standardizes shared runtime and test foundations across Alfred workflows so bug fixes are applied once and
propagated everywhere. The implementation keeps workflow-specific product semantics local (ranking, wording, provider
rules), while centralizing repeated mechanics (helper loading, CLI invocation guards, smoke test scaffolding, and policy
audits). Delivery is phased in migration waves to avoid broad regressions and to keep rollback scope small. The end
state is one unified development mode for workflows with automated guardrails that prevent old bug classes from
reappearing.

## Scope

- In scope: shared shell helper loading, shared script-filter execution driver, shared smoke-test helper library,
  migration of high-duplication workflows, lint/policy enforcement, CI/release version-source unification.
- In scope: updates to workflow template and development docs so new workflows default to the unified pattern.
- Out of scope: changing workflow-specific search/business behavior, changing Alfred UX copy policy per workflow,
  rewriting workflow adapters from Bash to another runtime.

## Assumptions (if any)

1. Workflow adapters remain Bash-first and keep current observable behavior contracts.
2. Changes can be delivered as multiple small PRs, not one monolithic refactor.
3. Full workspace lint/test/package gates are available in CI and locally.
4. `plan-tooling`, `jq`, and existing repo scripts remain available on PATH during execution.

## Sprint 1: Foundation Contracts And Shared Primitives

**Goal**: Define hard extraction boundaries and introduce reusable primitives without changing workflow semantics.
**Parallelization**: Tasks 1.2 and 1.3 can run in parallel after task 1.1; task 1.4 depends on 1.1 and can run in
parallel with 1.2/1.3 once interfaces are fixed. **Demo/Validation**:

- Command(s): `plan-tooling validate --file docs/plans/workflow-shared-foundations-plan.md`, `scripts/workflow-lint.sh`
- Verify: new shared helper files exist, are shellcheck-clean, and no placeholder content remains.

### Task 1.1: Publish Shared-Foundation Contract

- **Location**:
  - `docs/specs/workflow-shared-foundations-policy.md`
  - `ALFRED_WORKFLOW_DEVELOPMENT.md`
- **Description**: Add a canonical policy defining what must be shared (runtime mechanics) and what must stay
  workflow-local (domain semantics), plus mandatory migration/rollback rules.
- **Dependencies**:
  - none
- **Complexity**: 4
- **Acceptance criteria**:
  - Contract explicitly lists allowed shared domains: helper loading, binary resolution, error-row emission, CLI
    invocation guard, smoke scaffolding.
  - Contract explicitly forbids over-sharing domain mappings and provider-specific query semantics.
  - Migration and rollback constraints are documented and referenced from the global workflow development guide.
- **Validation**:
  - `rg -n "shared foundation|extraction boundary|must stay local|rollback" docs/specs/workflow-shared-foundations-policy.md ALFRED_WORKFLOW_DEVELOPMENT.md`

### Task 1.2: Add Shared Workflow Helper Loader

- **Location**:
  - `scripts/lib/workflow_helper_loader.sh`
  - `scripts/tests/workflow_helper_loader.test.sh`
- **Description**: Implement one loader utility to resolve and source helpers consistently across packaged workflow,
  repo-relative, and optional git-root fallback paths.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 6
- **Acceptance criteria**:
  - Loader provides deterministic resolution order and a single error contract for missing helper files.
  - Loader supports both script filters and action scripts without duplicated path logic.
  - Loader test script covers success, missing helper, and fallback path branches.
- **Validation**:
  - `bash scripts/tests/workflow_helper_loader.test.sh`
  - `shellcheck scripts/lib/workflow_helper_loader.sh scripts/tests/workflow_helper_loader.test.sh`

### Task 1.3: Add Shared Smoke-Test Helper Library

- **Location**:
  - `scripts/lib/workflow_smoke_helpers.sh`
  - `workflows/_template/tests/smoke.sh`
- **Description**: Extract repeated smoke primitives (`fail`, assertions, manifest parsing, plist conversion, artifact
  backup/restore) into one shared helper and migrate template smoke to source it.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 7
- **Acceptance criteria**:
  - Shared smoke helper exposes all currently duplicated primitives used by cloned smoke scripts.
  - Template smoke script uses shared helper and remains executable.
  - No behavior regression in template smoke error handling.
- **Validation**:
  - `bash workflows/_template/tests/smoke.sh`
  - `shellcheck scripts/lib/workflow_smoke_helpers.sh workflows/_template/tests/smoke.sh`

### Task 1.4: Add Shared CLI Execution Driver For Script Filters

- **Location**:
  - `scripts/lib/script_filter_cli_driver.sh`
  - `scripts/tests/script_filter_cli_driver.test.sh`
- **Description**: Introduce a shared driver for non-search script filters that standardizes err-file handling, empty
  output checks, JSON items-array guard, and fallback-to-error-row behavior.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 7
- **Acceptance criteria**:
  - Driver accepts workflow-local callbacks for command execution and error mapping.
  - Driver handles empty output and malformed JSON uniformly.
  - Driver tests cover success path and all guarded failure branches.
- **Validation**:
  - `bash scripts/tests/script_filter_cli_driver.test.sh`
  - `shellcheck scripts/lib/script_filter_cli_driver.sh scripts/tests/script_filter_cli_driver.test.sh`

## Sprint 2: High-Impact Workflow Migration Waves

**Goal**: Migrate the highest-duplication workflow scripts and tests to shared foundations while preserving
workflow-local behavior. **Parallelization**: Tasks 2.1, 2.2, and 2.3 can run in parallel after Sprint 1. Task 2.4 can
run in parallel with 2.2/2.3 after 1.3 is complete. **Demo/Validation**:

- Command(s): targeted smoke runs per migrated workflow + `scripts/workflow-lint.sh`
- Verify: migrated files no longer duplicate loader/assert boilerplate and all targeted smoke tests pass.

### Task 2.1: Migrate Action Wrappers To Shared Loader

- **Location**:
  - `workflows/bangumi-search/scripts/action_open.sh`
  - `workflows/bilibili-search/scripts/action_open.sh`
  - `workflows/cambridge-dict/scripts/action_open.sh`
  - `workflows/google-search/scripts/action_open.sh`
  - `workflows/imdb-search/scripts/action_open.sh`
  - `workflows/netflix-search/scripts/action_open.sh`
  - `workflows/wiki-search/scripts/action_open.sh`
  - `workflows/youtube-search/scripts/action_open.sh`
  - `workflows/epoch-converter/scripts/action_copy.sh`
  - `workflows/market-expression/scripts/action_copy.sh`
  - `workflows/multi-timezone/scripts/action_copy.sh`
  - `workflows/weather/scripts/action_copy.sh`
- **Description**: Replace duplicated per-file `resolve_helper()` wrappers with shared loader usage for open/copy action
  helpers.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 5
- **Acceptance criteria**:
  - Open/copy wrappers use the same shared loader contract.
  - Existing action argument validation and exit-code behavior are unchanged.
  - No workflow-specific helper path breakage in packaged mode.
  - Migration is delivered in three PR batches (open-url wrappers, copy wrappers, residual cleanup), each independently
    releasable.
- **Validation**:
  - `for wf in bangumi-search bilibili-search cambridge-dict google-search imdb-search netflix-search wiki-search \`
    `youtube-search; do bash "workflows/$wf/tests/smoke.sh"; done`
  - `for wf in epoch-converter market-expression multi-timezone weather; do bash "workflows/$wf/tests/smoke.sh"; done`

### Task 2.2: Migrate Search-Family Script Filters To Shared Loader Pattern

- **Location**:
  - `workflows/google-search/scripts/script_filter.sh`
  - `workflows/youtube-search/scripts/script_filter.sh`
  - `workflows/netflix-search/scripts/script_filter.sh`
  - `workflows/wiki-search/scripts/script_filter.sh`
  - `workflows/bangumi-search/scripts/script_filter.sh`
  - `workflows/cambridge-dict/scripts/script_filter.sh`
  - `workflows/spotify-search/scripts/script_filter.sh`
- **Description**: Standardize helper bootstrap in search-family filters using shared loader while keeping
  `sfsd_run_search_flow` and per-workflow error/query semantics local.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 8
- **Acceptance criteria**:
  - Search-family scripts no longer inline duplicate loader function blocks.
  - Workflow-local `print_error_item` mapping and fetch callbacks remain untouched semantically.
  - All migrated workflows still support query-input fallback (`arg`, env, stdin as applicable).
  - Each migrated script still defines workflow-local mapping/fetch callbacks (`print_error_item` and fetch function) in
    the workflow file rather than shared helpers.
- **Validation**:
  - `for wf in google-search youtube-search netflix-search wiki-search bangumi-search cambridge-dict spotify-search; \`
    `do bash "workflows/$wf/tests/smoke.sh"; done`
  - `for f in workflows/google-search/scripts/script_filter.sh workflows/youtube-search/scripts/script_filter.sh \`
    `workflows/netflix-search/scripts/script_filter.sh workflows/wiki-search/scripts/script_filter.sh \`
    `workflows/bangumi-search/scripts/script_filter.sh workflows/cambridge-dict/scripts/script_filter.sh \`
    `workflows/spotify-search/scripts/script_filter.sh; do rg -n \"^print_error_item\\(\\)|fetch_json|search_fetch\" \"$f\"; done`

### Task 2.3: Migrate Non-Search Script Filters To Shared CLI Driver

- **Location**:
  - `workflows/epoch-converter/scripts/script_filter.sh`
  - `workflows/multi-timezone/scripts/script_filter.sh`
  - `workflows/market-expression/scripts/script_filter.sh`
  - `workflows/quote-feed/scripts/script_filter.sh`
  - `workflows/imdb-search/scripts/script_filter.sh`
  - `workflows/bilibili-search/scripts/script_filter.sh`
- **Description**: Move repeated CLI invocation + JSON guard logic into the shared CLI driver while preserving
  workflow-specific titles/subtitles and config checks.
- **Dependencies**:
  - Task 1.2
  - Task 1.4
- **Complexity**: 8
- **Acceptance criteria**:
  - Duplicate err-file and malformed-JSON guard code is removed from migrated scripts.
  - Workflow-specific validation and messaging remain functionally equivalent.
  - All migrated workflows pass existing smoke tests.
  - Each migrated workflow retains a local `print_error_item` mapping function to preserve workflow-specific semantics.
- **Validation**:
  - `for wf in epoch-converter multi-timezone market-expression quote-feed imdb-search bilibili-search; do \`
    `bash "workflows/$wf/tests/smoke.sh"; done`
  - `for f in workflows/epoch-converter/scripts/script_filter.sh workflows/multi-timezone/scripts/script_filter.sh \`
    `workflows/market-expression/scripts/script_filter.sh workflows/quote-feed/scripts/script_filter.sh \`
    `workflows/imdb-search/scripts/script_filter.sh workflows/bilibili-search/scripts/script_filter.sh; do \`
    `rg -n \"^print_error_item\\(\\)\" \"$f\"; done`

### Task 2.4: Migrate Clone-Style Smoke Scripts To Shared Helper

- **Location**:
  - `workflows/google-search/tests/smoke.sh`
  - `workflows/youtube-search/tests/smoke.sh`
  - `workflows/netflix-search/tests/smoke.sh`
  - `workflows/wiki-search/tests/smoke.sh`
  - `workflows/cambridge-dict/tests/smoke.sh`
  - `workflows/bangumi-search/tests/smoke.sh`
  - `workflows/spotify-search/tests/smoke.sh`
  - `workflows/market-expression/tests/smoke.sh`
  - `workflows/multi-timezone/tests/smoke.sh`
  - `workflows/epoch-converter/tests/smoke.sh`
  - `workflows/quote-feed/tests/smoke.sh`
- **Description**: Convert the highest-duplication smoke scripts to source the shared smoke helper, keeping
  workflow-specific assertions local.
- **Dependencies**:
  - Task 1.3
- **Complexity**: 9
- **Acceptance criteria**:
  - Shared helper is sourced by migrated smoke scripts.
  - Workflow-specific assertions remain explicit and readable.
  - No change in pass/fail semantics for existing smoke scenarios.
  - Migration is executed in two reviewable waves (search-family smokes first, utility workflows second) to keep
    rollback scoped.
- **Validation**:
  - `for wf in google-search youtube-search netflix-search wiki-search cambridge-dict bangumi-search spotify-search; do \`
    `bash "workflows/$wf/tests/smoke.sh"; done`
  - `for wf in market-expression multi-timezone epoch-converter quote-feed; do bash "workflows/$wf/tests/smoke.sh"; done`

## Sprint 3: Enforcement And CI Consistency

**Goal**: Add automated guardrails so new changes cannot reintroduce duplicated bug-prone patterns. **Parallelization**:
Tasks 3.1 and 3.3 can run in parallel after Sprint 2 starts stabilizing; task 3.2 depends on 3.1. **Demo/Validation**:

- Command(s): `scripts/workflow-lint.sh`, policy check scripts, targeted workflow-pack smoke.
- Verify: CI fails on policy drift and codex-cli version source is unified.

### Task 3.1: Add Shared-Foundation Audit To Lint Gate

- **Location**:
  - `scripts/workflow-shared-foundation-audit.sh`
  - `scripts/workflow-lint.sh`
- **Description**: Add a repository audit that detects reintroduced duplicate loader blocks, missing shared-guard usage
  in migrated files, and prohibited placeholder patterns.
- **Dependencies**:
  - Task 2.1
  - Task 2.2
  - Task 2.3
- **Complexity**: 7
- **Acceptance criteria**:
  - Audit script has check/apply-free mode with actionable error messages.
  - `scripts/workflow-lint.sh` executes the audit in CI and local lint flows.
  - Known migrated files are enforced by explicit checks (no silent drift).
- **Validation**:
  - `bash scripts/workflow-shared-foundation-audit.sh --check`
  - `scripts/workflow-lint.sh`

### Task 3.2: Extend Script-Filter Policy Sync Checks

- **Location**:
  - `scripts/workflow-sync-script-filter-policy.sh`
  - `docs/specs/script-filter-input-policy.json`
  - `docs/specs/script-filter-input-policy.md`
- **Description**: Extend existing policy tooling beyond queue-delay fields to verify required shared helper and
  script-filter safety wiring for designated workflows.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 7
- **Acceptance criteria**:
  - Policy schema includes new guard targets and check semantics.
  - `--check` reports deterministic failures for missing required policy conditions.
  - Policy docs explain what is auto-syncable vs manual.
- **Validation**:
  - `bash scripts/workflow-sync-script-filter-policy.sh --check`
  - `rg -n \"shared foundation|helper loader|policy check\" docs/specs/script-filter-input-policy.md`
  - `rg -n \"shared_helper|targets|object_uids\" docs/specs/script-filter-input-policy.json`

### Task 3.3: Unify Codex-CLI Version Source For CI And Release

- **Location**:
  - `scripts/lib/codex_cli_version.sh`
  - `.github/workflows/ci.yml`
  - `.github/workflows/release.yml`
  - `workflows/codex-cli/scripts/lib/codex_cli_runtime.sh`
- **Description**: Introduce a single version source for `nils-codex-cli` and consume it in CI/release/package-related
  scripts to eliminate version drift.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 6
- **Acceptance criteria**:
  - CI and release workflows read codex-cli version from one canonical source.
  - Workflow runtime metadata and packaging script consume the same version contract.
  - Hardcoded duplicate versions are removed from workflow YAML files.
- **Validation**:
  - `rg -n "nils-codex-cli --version" .github/workflows/ci.yml .github/workflows/release.yml`
  - `rg -n "CODEX_CLI_PINNED_VERSION|CODEX_CLI_VERSION" scripts/lib/codex_cli_version.sh workflows/codex-cli/scripts/lib/codex_cli_runtime.sh`

## Sprint 4: Template, Rollout, And Hardening

**Goal**: Make shared foundations the default for new workflows and prove repo-wide stability before release.
**Parallelization**: Task 4.1 can start once Sprint 2 conventions stabilize; task 4.2 can run after enforcement tasks
converge; task 4.3 depends on tasks 4.1 and 4.2; task 4.4 depends on 4.3. **Demo/Validation**:

- Command(s): full required checks + package run + staged install checks.
- Verify: template-generated workflows follow new conventions and all gates pass.

### Task 4.1: Update Template And Workflow Bootstrap Defaults

- **Location**:
  - `workflows/_template/scripts/script_filter.sh`
  - `workflows/_template/scripts/action_open.sh`
  - `workflows/_template/tests/smoke.sh`
  - `scripts/workflow-new.sh`
- **Description**: Update template/bootstrap outputs so newly created workflows automatically use shared loader and
  smoke helper conventions.
- **Dependencies**:
  - Task 2.1
  - Task 2.2
  - Task 2.4
- **Complexity**: 6
- **Acceptance criteria**:
  - Newly scaffolded workflow contains shared-foundation bootstrap pattern by default.
  - No placeholder references to deprecated local helper boilerplate remain in template scripts.
  - Bootstrap command still produces a valid workflow skeleton.
- **Validation**:
  - `tmp_id="tmp-shared-foundation-check"; scripts/workflow-new.sh --id "$tmp_id"; \`
    `rg -n "workflow_helper_loader|workflow_smoke_helpers" "workflows/$tmp_id"; rm -rf "workflows/$tmp_id"`

### Task 4.2: Synchronize Root And Workflow Development Docs

- **Location**:
  - `README.md`
  - `DEVELOPMENT.md`
  - `ALFRED_WORKFLOW_DEVELOPMENT.md`
  - `docs/ARCHITECTURE.md`
  - `workflows/_template/README.md`
  - `workflows/_template/TROUBLESHOOTING.md`
- **Description**: Align root-level and workflow-development documents with shared-foundation conventions, including new
  helper boundaries, required checks, and migration expectations.
- **Dependencies**:
  - Task 4.1
  - Task 3.1
  - Task 3.2
  - Task 3.3
- **Complexity**: 6
- **Acceptance criteria**:
  - Root docs explicitly describe shared-vs-local extraction boundaries and enforcement hooks.
  - `DEVELOPMENT.md` references new shared-foundation audit/policy commands where relevant.
  - Workflow development docs and template troubleshooting reference the new default helper/bootstrap conventions.
- **Validation**:
  - `rg -n "shared foundation|extraction boundary|workflow-shared-foundation-audit|workflow_helper_loader|workflow_smoke_helpers" \`
    `README.md DEVELOPMENT.md ALFRED_WORKFLOW_DEVELOPMENT.md docs/ARCHITECTURE.md workflows/_template/README.md \`
    `workflows/_template/TROUBLESHOOTING.md`
  - `bash scripts/docs-placement-audit.sh --strict`

### Task 4.3: Execute Full Regression Gates And Package Verification

- **Location**:
  - `.github/workflows/ci.yml`
  - `docs/reports/workflow-shared-foundations-readiness.md`
- **Description**: Run full lint/test/package checks and document release-readiness evidence after migration.
- **Dependencies**:
  - Task 2.4
  - Task 3.1
  - Task 3.2
  - Task 3.3
  - Task 4.1
  - Task 4.2
- **Complexity**: 8
- **Acceptance criteria**:
  - Required repository checks pass with no new regressions.
  - Packaging smoke (`--all`) succeeds with unified shared helper staging.
  - Readiness report records command outputs and any accepted residual risk.
- **Validation**:
  - `scripts/workflow-lint.sh`
  - `scripts/workflow-test.sh`
  - `CODEX_CLI_PACK_SKIP_ARCH_CHECK=1 scripts/workflow-pack.sh --all`

### Task 4.4: Stage Rollout And Operational Safeguards

- **Location**:
  - `docs/reports/workflow-shared-foundations-rollout.md`
  - `ALFRED_WORKFLOW_DEVELOPMENT.md`
- **Description**: Define staged rollout order, owner checklist, and emergency recovery commands for runtime regressions
  discovered after packaging/install.
- **Dependencies**:
  - Task 4.3
- **Complexity**: 5
- **Acceptance criteria**:
  - Rollout document includes canary workflows, promotion criteria, and stop conditions.
  - Recovery section contains exact commands for reverting affected workflow paths and reinstalling last known-good
    artifacts.
  - Operational guide links to troubleshooting and validation checkpoints.
- **Validation**:
  - `rg -n "canary|promotion|stop condition|revert|known-good" docs/reports/workflow-shared-foundations-rollout.md ALFRED_WORKFLOW_DEVELOPMENT.md`

## Testing Strategy

- Unit: Add shell-level tests for new shared helper modules (`workflow_helper_loader`, `script_filter_cli_driver`) and
  keep them in lint/test flows.
- Integration: Run migrated workflow smoke scripts in focused batches per sprint, then full `scripts/workflow-test.sh`
  in Sprint 4.
- E2E/manual: Rebuild/install representative workflows (`google-search`, `weather`, `codex-cli`) and verify Alfred
  runtime behavior for query, action, and error fallbacks.

## Risks & gotchas

- Over-sharing risk: accidentally moving workflow-specific semantics into shared helpers can break UX contracts.
- Migration churn risk: touching many script files can cause merge conflicts; deliver in small PR waves by workflow
  family.
- Tooling strictness risk: new audits may create false positives if patterns are not scoped carefully.
- CI drift risk: codex-cli version-source unification can fail if any consumer bypasses the canonical version file.

## Rollback plan

- Keep migration isolated in sequential PR waves (helpers first, then family migrations, then enforcement) so rollback
  can be path-scoped.
- If runtime regressions occur, revert the affected workflow family paths first (`workflows/<family>/scripts`,
  `workflows/<family>/tests`) while retaining shared helpers.
- If shared-helper regression is confirmed, revert helper-introduction commits and rerun `scripts/workflow-lint.sh`,
  `scripts/workflow-test.sh`, and `scripts/workflow-pack.sh --all` before redeploy.
- Reinstall last known-good artifacts from `dist/<workflow>/<version>/` for impacted workflows and document incident
  details in rollout report.

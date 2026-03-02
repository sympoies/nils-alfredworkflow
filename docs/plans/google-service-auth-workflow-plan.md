# Plan: Google Service Workflow Auth (Login/Remove Multi-Account)

## Overview

This plan introduces a new `google-service` Alfred workflow focused only on auth operations, aligned with
`workflows/codex-cli` interaction patterns. Scope is intentionally narrow: implement `login` and `remove`, and add a
workflow-owned active-account switch layer to support multi-account usage without relying on `save` or alias flows.
`google-cli` native auth remains the source of truth for account/token storage; workflow state only tracks the selected
active account for command routing. Delivery is split into setup, auth flow implementation, and validation hardening so
the first release is testable and low-risk.

## Scope

- In scope:
  - New `workflows/google-service` scaffold (manifest, plist template, scripts, smoke test, docs).
  - Auth command palette rows for `login` and `remove` only.
  - Multi-account account picker and active-account switching state inside workflow data.
  - Login via `google-cli auth add <email>` with `--remote` step flow as primary path and `--manual` as fallback path.
  - Remove via `google-cli auth remove <email-or-alias>`, with confirmation and post-remove active-account rebalance.
  - Script-filter/action token contracts modeled after `codex-cli` (`<verb>::<payload>`).
- Out of scope:
  - `save`/`use`/`alias` command surfaces in this phase.
  - Gmail/Drive command integration.
  - Browser account manager UI.
  - Reworking native `google-cli` auth internals or storage schema.

## Assumptions

1. Workflow ID and directory name will be `google-service` (`workflows/google-service`).
2. `google-cli` binary is resolved from workflow packaged runtime or local workspace build, with `GOOGLE_CLI_BIN` as
   explicit override.
3. Multi-account switching in workflow means selecting an active account used for downstream routing, not mutating
   native `default_account` in `accounts.v1.json`.
4. Native `auth add` default loopback mode is not the reliable primary path; workflow should favor `--remote` flow and
   support `--manual` fallback.

## Sprint 1: Workflow Scaffold And Auth State Contract

**Goal**: Create `google-service` workflow skeleton and lock auth state/action token contracts before implementing
runtime behavior.
**Demo/Validation**:

- Command(s): `bash workflows/google-service/tests/smoke.sh`,
  `bash scripts/workflow-sync-script-filter-policy.sh --check --workflows google-service`
- Verify: workflow scaffold passes shared-foundation checks and exposes deterministic action token grammar.

### Task 1.1: Scaffold `google-service` Workflow Baseline

- **Location**:
  - `workflows/google-service/workflow.toml`
  - `workflows/google-service/src/info.plist.template`
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
  - `workflows/google-service/tests/smoke.sh`
- **Description**: Generate workflow from template and wire keyword/script/action plumbing for auth-only behavior.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - Workflow metadata (`id/name/bundle_id`) and script paths are correct for `google-service`.
  - Script filter and action script are executable and include shared helper-loader bootstrap markers.
  - Smoke test validates required files and executable bits.
- **Validation**:
  - `bash workflows/google-service/tests/smoke.sh`
  - `rg -n '^id[[:space:]]*=[[:space:]]*"google-service"$' workflows/google-service/workflow.toml`

### Task 1.2: Define Auth Query Grammar And Action Tokens

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/README.md`
- **Description**: Define command grammar and parser routing for `auth`, `login`, `remove`, and account switch rows.
  Canonical action token forms:
  - `login::remote::step1::<email>`
  - `login::remote::step2::<email>::<state>::<code>`
  - `login::manual::<email>::<code>`
  - `switch::<email>`
  - `remove::<email>::<yes-flag>`
- **Dependencies**:
  - Task 1.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Empty query shows auth action menu with only login/remove/switch-related items.
  - Invalid argument combinations produce non-actionable feedback rows.
  - Token format is stable and documented in workflow README.
- **Validation**:
  - `shellcheck workflows/google-service/scripts/script_filter.sh`
  - `rg -n "login::remote::step1|login::manual|switch::|remove::" workflows/google-service/scripts/script_filter.sh workflows/google-service/README.md`

### Task 1.3: Define Active-Account Persistence Contract

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
  - `workflows/google-service/README.md`
- **Description**: Add workflow-local active-account store (for example
  `$ALFRED_WORKFLOW_DATA/active-account.v1.json`) with read/write helpers, fallbacks, and stale-account cleanup.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Active account file writes atomically and survives Alfred restarts.
  - If active account no longer exists in `google-cli auth list`, workflow auto-falls back to native default account or
    first available account.
  - README documents active account semantics and fallback precedence.
- **Validation**:
  - `shellcheck workflows/google-service/scripts/script_filter.sh workflows/google-service/scripts/action_open.sh`
  - `rg -n "active-account.v1.json|fallback|default_account" workflows/google-service/scripts/script_filter.sh workflows/google-service/README.md`

## Sprint 2: Implement Auth Login/Remove And Multi-Account Switching

**Goal**: Implement end-to-end login/remove execution and account switching UX using `google-cli` auth commands.
**Demo/Validation**:

- Command(s): `bash workflows/google-service/tests/smoke.sh`,
  `bash scripts/workflow-test.sh --id google-service`
- Verify: login/remove actions execute with expected command arguments and state transitions.

### Task 2.1: Implement Login Flows (Remote First, Manual Fallback)

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
- **Description**: Implement login row generation and execution:
  - Remote step 1: run `google-cli --json auth add <email> --remote --step 1`, open authorization URL, persist `state`.
  - Remote step 2: accept callback URL or pasted code, then run
    `google-cli --json auth add <email> --remote --step 2 --state <state> --code <code>`.
  - Manual fallback: run `google-cli --json auth add <email> --manual --code <code>`.
  On success, set active account to the logged-in email.
- **Dependencies**:
  - Task 1.3
- **Complexity**: 5
- **Acceptance criteria**:
  - Login commands always run with explicit account email target.
  - Remote step mismatch/invalid input errors are surfaced as actionable Alfred rows.
  - Successful login updates active-account state and triggers Alfred requery to account list view.
- **Validation**:
  - `shellcheck workflows/google-service/scripts/script_filter.sh workflows/google-service/scripts/action_open.sh`
  - `rg -n "auth add .*--remote|auth add .*--manual|authorization_url|--state|--code" workflows/google-service/scripts/action_open.sh`

### Task 2.2: Implement Account Switch Rows (No `save/use`)

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
- **Description**: Build switch list from `google-cli --json auth list`, render each account as selectable row, and
  store selection via `switch::<email>` token.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Current active account is visually marked in list rows.
  - Selecting another row updates active-account file without mutating native auth metadata.
  - Unknown/stale active account is auto-healed on next render.
- **Validation**:
  - `rg -n "auth list|switch::|active account|current" workflows/google-service/scripts/script_filter.sh workflows/google-service/scripts/action_open.sh`

### Task 2.3: Implement Remove Flow With Confirmation And Rebalance

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
- **Description**: Remove rows are generated from real account list (not filename/alias list). Action path confirms
  removal, executes `google-cli --json auth remove <email>`, then rebalances active account to remaining default/first
  account.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Remove only appears for existing accounts.
  - Cancelled remove preserves state and emits a deterministic cancellation message.
  - Removing active account reassigns active pointer safely or clears it when no accounts remain.
- **Validation**:
  - `rg -n "auth remove|confirm|Cancelled|rebalance|remaining_accounts" workflows/google-service/scripts/action_open.sh`
  - `shellcheck workflows/google-service/scripts/action_open.sh`

## Sprint 3: Hardening, Documentation, And Acceptance

**Goal**: Lock behavior with smoke coverage and publish operator docs for first auth-only release.
**Demo/Validation**:

- Command(s): `bash workflows/google-service/tests/smoke.sh`,
  `bash scripts/workflow-test.sh --id google-service`,
  `bash scripts/workflow-pack.sh --id google-service`
- Verify: workflow package is releasable and auth-only UX is documented.

### Task 3.1: Add Deterministic Smoke Coverage With Stub `google-cli`

- **Location**:
  - `workflows/google-service/tests/smoke.sh`
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/scripts/action_open.sh`
- **Description**: Add smoke tests that stub `google-cli` JSON responses to validate login/remove/switch token routing,
  confirmation behavior, and active-account file updates.
- **Dependencies**:
  - Task 2.3
- **Complexity**: 5
- **Acceptance criteria**:
  - Smoke tests cover remote step1/step2 success and failure paths.
  - Smoke tests cover remove confirm/cancel and active-account rebalance.
  - Smoke tests assert no references to `save` or alias actions in auth menu.
- **Validation**:
  - `bash workflows/google-service/tests/smoke.sh`

### Task 3.2: Publish Auth-Only README And Troubleshooting

- **Location**:
  - `workflows/google-service/README.md`
  - `workflows/google-service/TROUBLESHOOTING.md`
  - `README.md`
- **Description**: Document command examples (`login`, `remove`, switch rows), required env vars, remote/manual login
  steps, and ambiguity troubleshooting for multi-account state.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 3
- **Acceptance criteria**:
  - README explicitly states phase scope: login/remove/switch only; no save/alias features.
  - Troubleshooting includes `NILS_GOOGLE_005/006/008` handling.
  - Root workflow catalog includes `google-service` entry.
- **Validation**:
  - `rg -n "login|remove|switch|NILS_GOOGLE_005|NILS_GOOGLE_006|NILS_GOOGLE_008|no save|no alias" \`
    `workflows/google-service/README.md workflows/google-service/TROUBLESHOOTING.md README.md`

## Testing Strategy

- Unit:
  - Shell-level parser/normalization checks inside smoke assertions for query grammar and token construction.
- Integration:
  - Stub-driven `script_filter.sh` + `action_open.sh` flows validating `google-cli` command invocation and state-file
    transitions.
- E2E/manual:
  - Real Google OAuth remote flow (step1/step2) on a disposable test account.
  - Multi-account scenario: login A, login B, switch active, remove B, verify fallback to A.

## Risks & gotchas

- `google-cli auth add` loopback mode is not production-ready by default; workflow must avoid assuming browser callback
  capture is automatic.
- Active-account workflow state can drift from native account metadata if external commands modify accounts; render
  path must auto-heal on every refresh.
- Remove-by-email must use canonical account IDs from `auth list`; never rely on mutable display labels.
- Confirmation dialogs may behave differently on non-macOS/headless runs; tests must include no-dialog fallback path.

## Rollback plan

- Revert `workflows/google-service` additions and remove root README entry.
- Keep `google-cli` crate untouched so rollback is workflow-only and low blast radius.
- Validation after rollback:
  - `bash scripts/workflow-test.sh`
  - `bash scripts/workflow-pack.sh`

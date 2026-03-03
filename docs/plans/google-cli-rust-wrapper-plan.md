# Plan: Google CLI Rust Wrapper (Auth/Gmail/Drive)

## Overview

This plan introduces a new Rust crate `crates/google-cli` as a wrapper over the existing `gog` CLI, scoped to
`auth`, `gmail`, and `drive` commands only. Delivery is split into sequential sprint gates: architecture first, then
one feature per sprint, and a final integration sprint. The wrapper will prioritize stable command construction,
deterministic JSON-first behavior, and clear error contracts without adding workflow integration in this phase.

## Scope

- In scope: new `google-cli` crate, wrapper runtime for invoking `gog`, feature implementation for `auth`,
  `gmail`, `drive`, per-feature docs/README sections, and per-feature tests.
- In scope: final integration sprint covering full CLI test pass and root-level documentation updates.
- Out of scope: Alfred workflow script integration, workflow packaging, workflow smoke harness changes, and non-target
  `gog` domains (calendar/chat/forms/etc.).

## Assumptions (if any)

1. Host environment has `gog` available on `PATH` (or explicit path override) during runtime/tests.
2. Wrapper behavior follows `gog` v0.11.x command/flag surface for `auth`, `gmail`, and `drive`.
3. Existing workspace conventions for crate layout, docs placement, and `cli_contract` testing remain the default.
4. This phase optimizes wrapper correctness and maintainability, not command-surface expansion beyond target scope.

## Sprint 1: Crate Architecture And Wrapper Contracts

**Goal**: Establish the full crate architecture and execution contracts before feature implementation starts.
**Parallelization**: Sprint executes as a serial integration gate to freeze interfaces used by all later feature
sprints.
**PR grouping intent**: per-sprint
**Execution Profile**: serial
**Scorecard**:

- Execution Profile: serial
- TotalComplexity: 14
- CriticalPathComplexity: 14
- MaxBatchWidth: 1
- OverlapHotspots: `docs/specs/google-cli-native-contract.md` and `crates/google-cli/tests/common/mod.rs` are reused
  across multiple tasks; keep sequencing strict to avoid churn.
**Demo/Validation**:
- Command(s): `plan-tooling validate --file docs/plans/google-cli-rust-wrapper-plan.md`, `cargo check -p nils-google-cli`
- Verify: crate compiles with architecture modules in place and documented command/runtime contracts.

### Task 1.1: Publish Wrapper Command And Runtime Contract

- **Location**:
  - `docs/specs/google-cli-native-contract.md`
  - `docs/specs/cli-standards-mapping.md`
- **Description**: Define wrapper boundaries for `auth/gmail/drive`, command naming strategy, pass-through flag policy,
  JSON/text mode behavior, and wrapper-owned error taxonomy.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - Contract explicitly defines supported command groups and non-goals for this phase.
  - Contract defines how global flags (`--account`, `--client`, output mode flags) are mapped to wrapped `gog`
    execution.
  - Error categories distinguish user input, missing `gog`, process failure, and invalid output decoding.
- **Validation**:
  - `rg -n "auth|gmail|drive|error|scope|contract" docs/specs/google-cli-native-contract.md docs/specs/cli-standards-mapping.md`

### Task 1.2: Scaffold `crates/google-cli` And Workspace Wiring

- **Location**:
  - `Cargo.toml`
  - `crates/google-cli/Cargo.toml`
  - `crates/google-cli/src/main.rs`
  - `crates/google-cli/src/lib.rs`
  - `crates/google-cli/src/cmd/mod.rs`
  - `crates/google-cli/src/cmd/auth.rs`
  - `crates/google-cli/src/cmd/gmail.rs`
  - `crates/google-cli/src/cmd/drive.rs`
- **Description**: Add the new crate scaffold and command router skeleton consistent with workspace conventions.
- **Dependencies**:
  - Task 1.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Workspace includes `crates/google-cli` and crate compiles.
  - Binary entrypoint exposes top-level groups `auth`, `gmail`, `drive`.
  - Command router skeleton and empty auth/gmail/drive module stubs are present with compile-only wiring (no feature
    behavior).
- **Validation**:
  - `cargo check -p nils-google-cli`
  - `cargo run -p nils-google-cli -- --help | rg -n "auth|gmail|drive"`
  - `test -f crates/google-cli/src/cmd/auth.rs && test -f crates/google-cli/src/cmd/gmail.rs && test -f crates/google-cli/src/cmd/drive.rs`

### Task 1.3: Build Shared Wrapper Runtime Layer

- **Location**:
  - `crates/google-cli/src/runtime.rs`
  - `crates/google-cli/src/output.rs`
  - `crates/google-cli/src/error.rs`
  - `crates/google-cli/src/cmd/common.rs`
- **Description**: Implement shared process-execution primitives for invoking `gog`, normalizing stdout/stderr,
  and decoding output envelopes for downstream command modules.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Runtime resolves `gog` executable deterministically and returns actionable errors when missing.
  - Shared command builder supports global pass-through flags and command-specific arguments.
  - Output decoding utilities support JSON mode and plain-text mode handling paths.
- **Validation**:
  - `cargo test -p nils-google-cli --lib`

### Task 1.4: Create Baseline Docs And Test Harness Skeleton

- **Location**:
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/auth.md`
  - `crates/google-cli/docs/gmail.md`
  - `crates/google-cli/docs/drive.md`
  - `crates/google-cli/tests/common/mod.rs`
  - `crates/google-cli/tests/cli_contract.rs`
- **Description**: Establish baseline crate docs and reusable test harness utilities for feature sprints.
- **Dependencies**:
  - Task 1.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Crate README and docs index include architecture overview and planned feature sections.
  - Feature doc files exist as stubs with command/validation headings and complete prose sections.
  - Test harness provides reusable process fixture helpers for wrapped `gog` execution tests.
- **Validation**:
  - `cargo test -p nils-google-cli --test cli_contract -- --nocapture`
  - `rg -n "auth|gmail|drive|validation|contract" crates/google-cli/README.md`
  - `rg -n "auth|gmail|drive|validation|contract" crates/google-cli/docs/auth.md crates/google-cli/docs/gmail.md crates/google-cli/docs/drive.md`

## Sprint 2: Auth Feature Implementation

**Goal**: Deliver the `auth` command group wrapper with complete tests and auth-specific documentation.
**Parallelization**: Tasks 2.1 -> 2.2 are serial; Tasks 2.3 and 2.4 run in parallel after 2.2.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 13
- CriticalPathComplexity: 11
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/auth.rs` and crate README auth sections are tightly coupled.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test auth_cli_contract`, `cargo run -p nils-google-cli -- auth --help`
- Verify: auth wrapper commands execute expected `gog auth ...` calls and docs/tests are complete.

### Task 2.1: Lock Auth Command Surface And Usage Contract

- **Location**:
  - `crates/google-cli/docs/auth.md`
  - `docs/specs/google-cli-native-contract.md`
- **Description**: Finalize auth subcommand scope and option policy for this phase (`credentials`, `add`, `list`,
  `status`, `remove`, `alias`, `manage`).
- **Dependencies**:
  - Task 1.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Auth docs define supported subcommands and explicitly defer non-scoped operations.
  - Contract defines pass-through expectations for account/client/manual/remote auth options.
  - Validation and error behavior for auth command failures is documented.
- **Validation**:
  - `rg -n "credentials|add|list|status|remove|alias|manage|auth" crates/google-cli/docs/auth.md docs/specs/google-cli-native-contract.md`

### Task 2.2: Implement Auth Wrapper Commands

- **Location**:
  - `crates/google-cli/src/cmd/auth.rs`
  - `crates/google-cli/src/cmd/mod.rs`
  - `crates/google-cli/src/main.rs`
- **Description**: Implement auth command parsing and execution over shared runtime, including argument forwarding and
  deterministic error propagation.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 4
- **Acceptance criteria**:
  - All scoped auth subcommands are callable from wrapper CLI.
  - Wrapper forwards command arguments and global flags without mutation bugs.
  - Runtime/process errors map to stable wrapper exit/error contract.
- **Validation**:
  - `cargo test -p nils-google-cli --lib`
  - `cargo run -p nils-google-cli -- auth list --help`

### Task 2.3: Add Auth Contract Tests

- **Location**:
  - `crates/google-cli/tests/auth_cli_contract.rs`
  - `crates/google-cli/tests/common/mod.rs`
  - `crates/google-cli/tests/fixtures/fake_gog.sh`
- **Description**: Add CLI contract coverage for auth command routing, flag forwarding, and failure mapping.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Tests verify command-path correctness for each scoped auth subcommand.
  - Tests verify process failure and malformed output handling.
  - Fixture strategy keeps tests deterministic without requiring real Google auth.
- **Validation**:
  - `cargo test -p nils-google-cli --test auth_cli_contract`

### Task 2.4: Finalize Auth README/Docs Section

- **Location**:
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/auth.md`
- **Description**: Publish auth usage examples, environment/flag notes, and validation commands in crate docs.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 2
- **Acceptance criteria**:
  - README includes auth command table and example invocations.
  - Feature doc includes troubleshooting and expected error signatures.
  - Validation commands in docs match executable test/help commands.
- **Validation**:
  - `rg -n "## Auth|auth add|auth list|auth status|validation" crates/google-cli/README.md crates/google-cli/docs/auth.md`

## Sprint 3: Gmail Feature Implementation

**Goal**: Deliver the `gmail` command group wrapper with feature-level docs and contract tests.
**Parallelization**: Tasks 3.1 -> 3.2 are serial; Tasks 3.3 and 3.4 run in parallel after 3.2.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 13
- CriticalPathComplexity: 11
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/gmail.rs` and test fixtures must evolve together to keep command
  assertions stable.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test gmail_cli_contract`, `cargo run -p nils-google-cli -- gmail --help`
- Verify: wrapper supports scoped Gmail subcommands with deterministic forwarding/docs/tests.

### Task 3.1: Lock Gmail Command Contract For Wrapper Scope

- **Location**:
  - `crates/google-cli/docs/gmail.md`
  - `docs/specs/google-cli-native-contract.md`
- **Description**: Define scoped Gmail subcommands and option policy for the wrapper phase (query/list/get/send-focused
  command set).
- **Dependencies**:
  - Task 2.4
- **Complexity**: 2
- **Acceptance criteria**:
  - Gmail scope and non-goals are explicit in feature docs.
  - Contract includes required account/client/pass-through handling for Gmail commands.
  - Output mode expectations (`--json`/`--plain`) are documented.
- **Validation**:
  - `rg -n "gmail|search|thread|get|send|json|plain|scope" crates/google-cli/docs/gmail.md docs/specs/google-cli-native-contract.md`

### Task 3.2: Implement Gmail Wrapper Commands

- **Location**:
  - `crates/google-cli/src/cmd/gmail.rs`
  - `crates/google-cli/src/cmd/mod.rs`
  - `crates/google-cli/src/main.rs`
- **Description**: Implement Gmail command routing and runtime execution for the scoped command set.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 5
- **Acceptance criteria**:
  - Wrapper exposes scoped Gmail subcommands and arguments with clap help output.
  - Shared runtime is used consistently (no command-local process spawning duplication).
  - Error mapping for non-zero `gog` exits is deterministic and testable.
- **Validation**:
  - `cargo test -p nils-google-cli --lib`
  - `cargo run -p nils-google-cli -- gmail search --help`

### Task 3.3: Add Gmail Contract Tests

- **Location**:
  - `crates/google-cli/tests/gmail_cli_contract.rs`
  - `crates/google-cli/tests/common/mod.rs`
  - `crates/google-cli/tests/fixtures/fake_gog.sh`
- **Description**: Add command-contract tests for Gmail routing, argument pass-through, and output/error handling.
- **Dependencies**:
  - Task 3.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Tests cover success and failure branches for scoped Gmail commands.
  - Tests verify forwarding of representative Gmail query/send flags.
  - Tests remain deterministic without live Gmail API dependency.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_cli_contract`

### Task 3.4: Finalize Gmail README/Docs Section

- **Location**:
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/gmail.md`
- **Description**: Document Gmail wrapper usage, examples, limitations, and test/validation commands.
- **Dependencies**:
  - Task 3.2
- **Complexity**: 2
- **Acceptance criteria**:
  - README contains Gmail command table and examples.
  - Feature doc contains troubleshooting guidance for wrapper/runtime failures.
  - Doc commands are consistent with tested command surface.
- **Validation**:
  - `rg -n "## Gmail|gmail search|gmail get|gmail send|validation" crates/google-cli/README.md crates/google-cli/docs/gmail.md`

## Sprint 4: Drive Feature Implementation

**Goal**: Deliver the `drive` command group wrapper with feature-level docs and contract tests.
**Parallelization**: Tasks 4.1 -> 4.2 are serial; Tasks 4.3 and 4.4 run in parallel after 4.2.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 13
- CriticalPathComplexity: 11
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/drive.rs` and docs command tables must stay synchronized.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test drive_cli_contract`, `cargo run -p nils-google-cli -- drive --help`
- Verify: wrapper supports scoped Drive subcommands with complete docs/tests.

### Task 4.1: Lock Drive Command Contract For Wrapper Scope

- **Location**:
  - `crates/google-cli/docs/drive.md`
  - `docs/specs/google-cli-native-contract.md`
- **Description**: Define scoped Drive subcommands and option policy for wrapper phase (`ls/search/get/download/upload`
  focused command set).
- **Dependencies**:
  - Task 3.4
- **Complexity**: 2
- **Acceptance criteria**:
  - Drive scope and deferred commands are explicit in docs.
  - Contract defines forwarding behavior for path/id/query arguments and global flags.
  - Output handling expectations are documented for JSON/plain modes.
- **Validation**:
  - `rg -n "drive|ls|search|get|download|upload|json|plain|scope" crates/google-cli/docs/drive.md docs/specs/google-cli-native-contract.md`

### Task 4.2: Implement Drive Wrapper Commands

- **Location**:
  - `crates/google-cli/src/cmd/drive.rs`
  - `crates/google-cli/src/cmd/mod.rs`
  - `crates/google-cli/src/main.rs`
- **Description**: Implement Drive command routing and runtime execution for scoped subcommands.
- **Dependencies**:
  - Task 4.1
- **Complexity**: 5
- **Acceptance criteria**:
  - Wrapper exposes scoped Drive subcommands with help and argument parsing.
  - Shared runtime + error mapping patterns match auth/gmail implementation.
  - No feature-specific process layer duplication outside shared runtime.
- **Validation**:
  - `cargo test -p nils-google-cli --lib`
  - `cargo run -p nils-google-cli -- drive ls --help`

### Task 4.3: Add Drive Contract Tests

- **Location**:
  - `crates/google-cli/tests/drive_cli_contract.rs`
  - `crates/google-cli/tests/common/mod.rs`
  - `crates/google-cli/tests/fixtures/fake_gog.sh`
- **Description**: Add command-contract tests for Drive command routing, option forwarding, and error/output behavior.
- **Dependencies**:
  - Task 4.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Tests cover success and failure branches for scoped Drive commands.
  - Tests verify forwarding of representative Drive options (`--parent`, `--query`, `--out`).
  - Tests run deterministically without live Google Drive API.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_cli_contract`

### Task 4.4: Finalize Drive README/Docs Section

- **Location**:
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/drive.md`
- **Description**: Document Drive wrapper command usage, examples, and troubleshooting.
- **Dependencies**:
  - Task 4.2
- **Complexity**: 2
- **Acceptance criteria**:
  - README includes Drive command table and examples.
  - Feature doc includes failure-mode and remediation notes for wrapper/runtime errors.
  - Validation/test command references are present and accurate.
- **Validation**:
  - `rg -n "## Drive|drive ls|drive search|drive download|validation" crates/google-cli/README.md crates/google-cli/docs/drive.md`

## Sprint 5: Final Integration, Full Test Gates, And Root Documentation

**Goal**: Integrate auth/gmail/drive into one coherent CLI release candidate, pass full crate tests, and finalize root
documentation.
**Parallelization**: Serial final gate to minimize late-stage merge churn and keep release validation deterministic.
**PR grouping intent**: per-sprint
**Execution Profile**: serial
**Scorecard**:

- Execution Profile: serial
- TotalComplexity: 14
- CriticalPathComplexity: 14
- MaxBatchWidth: 1
- OverlapHotspots: root README/docs updates and final test evidence can drift if command surface changes late.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli`,
  `cargo test -p nils-google-cli --test cli_contract --test auth_cli_contract --test gmail_cli_contract --test drive_cli_contract`,
  `cargo run -p nils-google-cli -- --help`,
  `plan-tooling validate --file docs/plans/google-cli-rust-wrapper-plan.md`
- Verify: all scoped CLI contracts pass and repository-level docs describe wrapper usage/status accurately.

### Task 5.1: Normalize Cross-Feature CLI UX And Shared Flags

- **Location**:
  - `crates/google-cli/src/main.rs`
  - `crates/google-cli/src/cmd/mod.rs`
  - `crates/google-cli/src/cmd/common.rs`
- **Description**: Align help text, global flags, and shared execution behavior across auth/gmail/drive commands.
- **Dependencies**:
  - Task 4.4
- **Complexity**: 4
- **Acceptance criteria**:
  - Global flags and mode behavior are consistent across all feature groups.
  - Help output and error messages follow one shared UX contract.
  - No feature-specific divergence in shared runtime invocation path.
- **Validation**:
  - `cargo run -p nils-google-cli -- --help`
  - `cargo run -p nils-google-cli -- auth --help`
  - `cargo run -p nils-google-cli -- gmail --help`
  - `cargo run -p nils-google-cli -- drive --help`

### Task 5.2: Execute Full CLI Contract Test Matrix

- **Location**:
  - `crates/google-cli/tests/cli_contract.rs`
  - `crates/google-cli/tests/auth_cli_contract.rs`
  - `crates/google-cli/tests/gmail_cli_contract.rs`
  - `crates/google-cli/tests/drive_cli_contract.rs`
- **Description**: Finalize and run complete test matrix covering all scoped command groups and shared failure paths.
- **Dependencies**:
  - Task 5.1
- **Complexity**: 5
- **Acceptance criteria**:
  - All feature-specific contract tests pass in one run.
  - Cross-feature shared runtime/error behavior is explicitly asserted in tests.
  - Test matrix documents required fixture assumptions (`fake_gog`, env overrides).
- **Validation**:
  - `cargo test -p nils-google-cli`

### Task 5.3: Update Root-Level README And Architecture Docs

- **Location**:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/specs/cli-standards-mapping.md`
- **Description**: Add `google-cli` crate to repository docs and document its wrapper role, scope, and limitations.
- **Dependencies**:
  - Task 5.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Root README lists `google-cli` with scope (`auth/gmail/drive`) and non-goals.
  - Architecture docs include wrapper relationship to `gog`.
  - Standards mapping references wrapper-specific command/error/output contracts.
- **Validation**:
  - `rg -n "google-cli|gog wrapper|auth/gmail/drive" README.md docs/ARCHITECTURE.md docs/specs/cli-standards-mapping.md`

### Task 5.4: Record Final Validation Evidence And Release Readiness Notes

- **Location**:
  - `docs/reports/google-cli-native-validation-report.md`
  - `crates/google-cli/README.md`
- **Description**: Capture final command/test evidence and confirm crate-level docs are complete for handoff.
- **Dependencies**:
  - Task 5.3
- **Complexity**: 2
- **Acceptance criteria**:
  - Validation report records final commands and pass/fail status.
  - Crate README/docs contain runnable quickstart and verification commands.
  - Remaining known risks/limitations are explicitly listed.
- **Validation**:
  - `rg -n "Automated native matrix|Live smoke checklist|Release readiness note" docs/reports/google-cli-native-validation-report.md crates/google-cli/README.md`

## Testing Strategy

- Unit: command argument builders, runtime command construction, output decoding, and error mapping in
  `crates/google-cli/src/**`.
- Integration: feature contract tests in `crates/google-cli/tests/*_cli_contract.rs` with deterministic `fake_gog`
  fixtures.
- End-to-end crate gate: `cargo test -p nils-google-cli` covering auth/gmail/drive and shared behavior.
- Documentation verification: grep-based checks for command examples, validation commands, and known-limitation sections.

## Risks & gotchas

- Upstream `gog` command/flag changes can break wrapper assumptions; wrapper contract must pin supported behavior.
- Wrapper can accidentally drift into pass-through ambiguity if feature-specific parsers diverge.
- Fixture tests may become brittle if mocked `gog` outputs do not track real envelope shapes.
- Missing `gog` binary or inconsistent runtime environment must remain a first-class tested error path.

## Rollback plan

1. Remove `crates/google-cli` from workspace membership and delete crate directory in one rollback changeset.
2. Revert root/docs entries that advertise `google-cli` availability and scope.
3. Remove wrapper-specific specs/reports introduced for this effort.
4. Re-run baseline workspace checks: `cargo check`, `cargo test`, and documentation audits to confirm no residual
   references remain.
5. Publish rollback note summarizing removed scope and reasons (for example upstream contract drift or test instability).

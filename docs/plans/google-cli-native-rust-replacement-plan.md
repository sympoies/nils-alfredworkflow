# Plan: Google CLI Native Rust Replacement (Auth/Gmail/Drive)

## Overview

This plan replaces the current `gog`-shelling implementation inside `crates/google-cli` with a native Rust runtime for
the repo-scoped Google surface: `auth`, `gmail`, and `drive`. The design keeps Google-hosted login/consent pages and
the loopback-port callback flow, but it does not rebuild the browser account-manager UI that `gog auth manage`
exposes today. Delivery is split into sequential sprint gates: dependency survey and architecture freeze first, then
native auth, Gmail, Drive, and a final de-wrapper/integration sprint. Sprint 4 starts only after Sprint 3 demo gates
pass, and Sprint 5 starts only after Sprint 4 demo gates pass; there is no intended cross-sprint execution
parallelism.

## Scope

- In scope: native Rust auth/token runtime, loopback callback handling, multi-account state, Gmail commands, Drive
  commands, native error/output contracts, docs/spec updates, and live validation gates.
- In scope: exact-pin dependency decisions for all new external crates and a documented fallback path when a chosen
  Google API crate proves insufficient.
- Out of scope: browser account-manager UI, non-scoped Google domains (`calendar`, `chat`, `docs`, `forms`, etc.),
  service-account support, and Alfred workflow integration beyond the existing `google-cli` crate boundary.

## Assumptions (if any)

1. Replacement scope is the current repo-owned `google-cli` command surface only: `auth`, `gmail`, and `drive`.
2. Google login and OAuth consent pages remain Google-hosted; native Rust code only launches the browser, listens on a
   loopback callback, and exchanges codes for tokens.
3. The loopback-port auth flow is already viable in the local environment and should be the primary interactive path.
4. Browser account-manager UI will not be rebuilt. Sprint 1 must explicitly decide whether `auth manage` becomes a
   terminal-only summary command or a documented unsupported command.
5. `auth status` must never return an empty account silently when `--account` is omitted. Native semantics must resolve
   a configured default account, derive an unambiguous single account, or return a deterministic multi-account warning.
6. New third-party crates must be exact-pinned in `Cargo.toml` and recorded in `Cargo.lock` per repo policy.

## Sprint 1: Native Architecture Survey And Contract Freeze

**Goal**: Prove the native stack is viable, exact-pin the crate set, and freeze the auth/account contract before any
feature implementation starts.
**Parallelization**: Task 1.1 and Task 1.2 can run in parallel; Task 1.3 and Task 1.4 then converge the findings into
one implementation contract.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 15
- CriticalPathComplexity: 12
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/Cargo.toml`, `docs/specs/google-cli-native-contract.md`, and
  `crates/google-cli/src/lib.rs` become shared touch points once the survey crystallizes into code shape.
**Merge order**: Task 1.1 and Task 1.2 may merge independently; then Task 1.3; then Task 1.4.
**Demo/Validation**:
- Command(s): `plan-tooling validate --file docs/plans/google-cli-native-rust-replacement-plan.md`,
  `cargo test -p nils-google-cli --test native_dependency_probe`,
  `rg -n "google-gmail1|google-drive3|yup-oauth2|keyring|directories|open|mail-builder|wiremock" docs/reports/google-cli-native-crate-survey.md`,
  `rg -n "usable via generated crate|usable via reqwest fallback|blocked|auth add|gmail send|drive upload" docs/reports/google-cli-native-capability-matrix.md`
- Verify: the repo has a pinned dependency decision record, a per-command capability matrix, a compile-only native
  probe, and a written contract for auth/account semantics before feature work starts.

### Task 1.1: Inventory Current Scope, Live E2E Findings, And Native Gaps

- **Location**:
  - `docs/reports/google-cli-native-gap-analysis.md`
  - `docs/reports/google-cli-native-validation-report.md`
  - `crates/google-cli/README.md`
- **Description**: Convert the wrapper-era live findings into an implementation inventory: commands to preserve, known
  bugs to eliminate (`auth add --remote --step 2` state mismatch), and explicit non-goals such as the browser account
  manager UI.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - Gap analysis lists every repo-scoped command that must survive the native migration.
  - Gap analysis records current live findings, including the `auth status` ambiguity when no account is selected.
  - Non-goals are explicit: no local account-manager page and no expansion into non-scoped Google services.
- **Validation**:
  - `rg -n "state mismatch|auth status|manage|non-goal|gmail|drive" docs/reports/google-cli-native-gap-analysis.md`

### Task 1.2: Survey Candidate Crates, Exact-Pin Them, And Add A Compile Probe

- **Location**:
  - `docs/reports/google-cli-native-crate-survey.md`
  - `docs/reports/google-cli-native-capability-matrix.md`
  - `crates/google-cli/Cargo.toml`
  - `crates/google-cli/tests/native_dependency_probe.rs`
  - `Cargo.lock`
- **Description**: Evaluate and pin the native dependency stack, including Google API clients, OAuth, token storage,
  browser launching, MIME construction, and HTTP-test tooling. The survey must compare the primary plan
  (`google-gmail1`, `google-drive3`, `google-apis-common`, `yup-oauth2`, `keyring`, `directories`, `open`,
  `mail-builder`, `mime_guess`, `wiremock`) against a documented fallback path (`reqwest` + hand-written REST calls).
  The output must also include a capability matrix for every repo-scoped operation (`auth credentials set/list`, `auth
  add/status/remove/alias/manage`, `gmail search/get/send/thread get/thread modify`, `drive ls/search/get/download/upload`)
  with one of three statuses: `usable via generated crate`, `usable via reqwest fallback`, or `blocked`.
- **Dependencies**:
  - none
- **Complexity**: 5
- **Acceptance criteria**:
  - Survey report records candidate crate versions, role, rationale, and rejection/fallback notes.
  - Capability matrix covers every repo-scoped operation and marks each one as generated-crate, reqwest-fallback, or
    blocked.
  - `Cargo.toml` exact-pins the chosen crates needed for the compile spike.
  - `tests/native_dependency_probe.rs` validates the selected OAuth + Gmail + Drive client stack without external API calls or browser launches.
- **Validation**:
  - `cargo test -p nils-google-cli --test native_dependency_probe`
  - `cargo tree -p nils-google-cli`
  - `rg -n "Primary|Fallback|google-gmail1|google-drive3|yup-oauth2|keyring|wiremock" docs/reports/google-cli-native-crate-survey.md`
  - `rg -n "usable via generated crate|usable via reqwest fallback|blocked|auth add|gmail send|drive upload" docs/reports/google-cli-native-capability-matrix.md`

### Task 1.3: Publish Native Command Contract And Account Semantics

- **Location**:
  - `docs/specs/google-cli-native-contract.md`
  - `crates/google-cli/docs/auth.md`
  - `crates/google-cli/docs/gmail.md`
  - `crates/google-cli/docs/drive.md`
- **Description**: Replace the wrapper contract with a native contract covering command scope, native output/error
  envelope behavior, OAuth modes, multi-account/default resolution, and the no-UI decision for account management.
- **Dependencies**:
  - Task 1.1
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Contract defines native ownership of auth, Gmail, and Drive behavior and removes `gog` pass-through language.
  - Contract explicitly defines `auth status` semantics when `--account` is omitted.
  - Contract states how `auth manage` is handled without a browser UI.
- **Validation**:
  - `rg -n "native|default account|auth status|auth manage|loopback|manual|remote" docs/specs/google-cli-native-contract.md`
  - `rg -n "native|default account|auth status|auth manage|loopback|manual|remote" crates/google-cli/docs/auth.md`
  - `rg -n "native|default account|auth status|auth manage|loopback|manual|remote" crates/google-cli/docs/gmail.md crates/google-cli/docs/drive.md`

### Task 1.4: Reshape Crate Layout For Native Service Modules

- **Location**:
  - `crates/google-cli/src/lib.rs`
  - `crates/google-cli/src/main.rs`
  - `crates/google-cli/src/auth/mod.rs`
  - `crates/google-cli/src/auth/config.rs`
  - `crates/google-cli/src/auth/oauth.rs`
  - `crates/google-cli/src/auth/store.rs`
  - `crates/google-cli/src/gmail/mod.rs`
  - `crates/google-cli/src/drive/mod.rs`
  - `crates/google-cli/src/client.rs`
- **Description**: Introduce the native module skeleton that later sprints will fill, while keeping the crate compiling
  against the newly pinned dependency set.
- **Dependencies**:
  - Task 1.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Native module tree exists for auth, Gmail, Drive, and shared client concerns.
  - Existing wrapper-only runtime layout is isolated behind the new native module boundaries or marked for deletion.
  - The crate still compiles after the module split.
- **Validation**:
  - `cargo check -p nils-google-cli`
  - `test -f crates/google-cli/src/auth/oauth.rs && test -f crates/google-cli/src/gmail/mod.rs && test -f crates/google-cli/src/drive/mod.rs`

## Sprint 2: Native Auth Core, Storage, And Multi-Account Semantics

**Goal**: Deliver native auth flows, token storage, account selection rules, and auth command behavior without any
dependency on `gog`.
**Parallelization**: Task 2.2 and Task 2.3 run in parallel after Task 2.1; Task 2.4 integrates remote/manual auth into
CLI behavior; Task 2.5 finalizes docs and native-no-`gog` validation.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 16
- CriticalPathComplexity: 13
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/auth.rs`, `crates/google-cli/src/auth/mod.rs`, and the config/token store
  code are shared across almost every auth task.
**Merge order**: Task 2.1; then Task 2.2 and Task 2.3 in parallel; then Task 2.4; then Task 2.5.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test auth_storage`,
  `cargo test -p nils-google-cli --test auth_oauth_flow`,
  `cargo test -p nils-google-cli --test auth_account_resolution`,
  `cargo test -p nils-google-cli --test auth_cli_contract`,
  `cargo test -p nils-google-cli --test native_no_gog`,
  `cargo run -p nils-google-cli -- auth --help`
- Verify: native auth storage and OAuth exchange work locally, and auth commands no longer require the `gog` binary.

### Task 2.1: Implement Native Config, Credentials, And Token Persistence

- **Location**:
  - `crates/google-cli/src/auth/config.rs`
  - `crates/google-cli/src/auth/store.rs`
  - `crates/google-cli/src/auth/credentials.rs`
  - `crates/google-cli/tests/auth_storage.rs`
- **Description**: Implement app-dir config, OAuth client credential storage, keyring-backed token persistence, and a
  deterministic on-disk schema for aliases/default account metadata.
- **Dependencies**:
  - Task 1.4
- **Complexity**: 3
- **Acceptance criteria**:
  - `auth credentials set/list` reads and writes native config without `gog`.
  - Tokens are stored via system keyring with a deterministic fallback/error path when keyring access fails.
  - Alias/default metadata is versioned and round-trips in tests.
- **Validation**:
  - `cargo test -p nils-google-cli --test auth_storage`

### Task 2.2: Implement Browser, Loopback, And Callback Capture

- **Location**:
  - `crates/google-cli/src/auth/oauth.rs`
  - `crates/google-cli/src/auth/callback.rs`
  - `crates/google-cli/src/auth/browser.rs`
  - `crates/google-cli/tests/auth_oauth_flow.rs`
- **Description**: Build the native interactive OAuth runner: browser launch, loopback callback listener, local callback
  capture, and timeout/cancel handling for the primary interactive auth path.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Local interactive auth works through the loopback port without an external helper binary.
  - Callback parsing, state capture, timeout, and user-cancel scenarios are covered in tests.
  - Interactive flow does not depend on `gog` binaries or wrapper-era process contracts.
- **Validation**:
  - `cargo test -p nils-google-cli --test auth_oauth_flow`

### Task 2.3: Implement Multi-Account Resolution And Status Semantics

- **Location**:
  - `crates/google-cli/src/auth/account.rs`
  - `crates/google-cli/src/auth/defaults.rs`
  - `crates/google-cli/tests/auth_account_resolution.rs`
- **Description**: Implement default-account rules, alias lookup, and no-ambiguity resolution so commands either use a
  clear target account or fail with a deterministic message instead of returning empty account data.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 3
- **Acceptance criteria**:
  - Commands resolve `--account`, alias, configured default, or single stored account deterministically.
  - `auth status` without `--account` never emits an empty account payload.
  - Tests cover zero-account, one-account, multi-account-with-default, and multi-account-without-default cases.
- **Validation**:
  - `cargo test -p nils-google-cli --test auth_account_resolution`

### Task 2.4: Implement Manual/Remote Exchange And Wire Native Auth Commands

- **Location**:
  - `crates/google-cli/src/cmd/auth.rs`
  - `crates/google-cli/src/auth/mod.rs`
  - `crates/google-cli/src/main.rs`
  - `crates/google-cli/tests/auth_cli_contract.rs`
  - `crates/google-cli/tests/native_no_gog.rs`
- **Description**: Add manual URL-paste and remote step-based code exchange on top of the interactive OAuth runtime,
  then rewire `auth credentials`, `auth add`, `auth list`, `auth status`, `auth remove`, and `auth alias` to the
  native runtime and define the non-UI behavior for `auth manage`.
- **Dependencies**:
  - Task 2.2
  - Task 2.3
- **Complexity**: 4
- **Acceptance criteria**:
  - Manual and remote auth paths use native state tracking and explicitly prevent the wrapper-era state mismatch bug.
  - Auth subcommands execute fully natively and do not shell out to `gog`.
  - `auth manage` behavior is finalized per Sprint 1 contract without opening a browser manager page.
  - CLI contract tests cover success, user error, keyring failure, ambiguous-account cases, and remote/manual auth
    exchange failures.
- **Validation**:
  - `cargo test -p nils-google-cli --test auth_cli_contract`
  - `cargo test -p nils-google-cli --test native_no_gog`
  - `cargo run -p nils-google-cli -- auth list --help`

### Task 2.5: Publish Native Auth Docs And Manual Smoke Procedure

- **Location**:
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/auth.md`
  - `docs/reports/google-cli-native-gap-analysis.md`
- **Description**: Update auth docs with the native flow, exact prerequisites, non-UI account-management stance, and a
  manual smoke checklist that uses the loopback callback flow.
- **Dependencies**:
  - Task 2.4
- **Complexity**: 2
- **Acceptance criteria**:
  - Auth docs no longer mention `gog` as a runtime dependency.
  - Docs explain how default-account resolution affects `auth status` and other auth-adjacent commands.
  - Manual smoke checklist is runnable and references current commands only.
- **Validation**:
  - `rg -n "loopback|default account|auth manage|gog" crates/google-cli/README.md crates/google-cli/docs/auth.md docs/reports/google-cli-native-gap-analysis.md`

## Sprint 3: Native Gmail Commands

**Goal**: Deliver native Gmail search/get/send/thread behavior over the chosen Google API client stack.
**Parallelization**: Task 3.2 and Task 3.4 run in parallel after Task 3.1; Task 3.3 extends the read lane; Task 3.5
then integrates CLI tests and docs.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 17
- CriticalPathComplexity: 13
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/gmail.rs` and `crates/google-cli/src/gmail/mod.rs` must stay stable while
  read/send paths are built in parallel.
**Merge order**: Task 3.1; then Task 3.2 and Task 3.4 in parallel; then Task 3.3; then Task 3.5.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test gmail_read`,
  `cargo test -p nils-google-cli --test gmail_thread`,
  `cargo test -p nils-google-cli --test gmail_send`,
  `cargo test -p nils-google-cli --test account_resolution_shared`,
  `cargo test -p nils-google-cli --test gmail_cli_contract`,
  `cargo run -p nils-google-cli -- gmail --help`
- Verify: Gmail commands execute through native API clients, including MIME message construction and thread mutation.

### Task 3.1: Build Shared Gmail Client And Request Adapters

- **Location**:
  - `crates/google-cli/src/gmail/mod.rs`
  - `crates/google-cli/src/gmail/client.rs`
  - `crates/google-cli/src/client.rs`
  - `crates/google-cli/tests/gmail_read.rs`
  - `crates/google-cli/tests/account_resolution_shared.rs`
- **Description**: Establish the native Gmail client wrapper, auth wiring, and reusable request/response adapters for
  Gmail message/thread operations.
- **Dependencies**:
  - Task 2.5
- **Complexity**: 4
- **Acceptance criteria**:
  - Gmail service initialization uses the native auth/token store.
  - Shared conversion helpers map Google API responses into stable local types.
  - Tests cover client bootstrap, representative response decoding, and Gmail account-resolution reuse in multi-account
    cases.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_read`
  - `cargo test -p nils-google-cli --test account_resolution_shared`

### Task 3.2: Implement Gmail Search And Get

- **Location**:
  - `crates/google-cli/src/gmail/read.rs`
  - `crates/google-cli/tests/gmail_read.rs`
- **Description**: Implement native `gmail search` and `gmail get` behavior, including query flags, metadata/header
  selection, and stable local output mapping.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Search/get commands produce stable local JSON/plain output without calling `gog`.
  - Query forwarding and metadata/header selection match the native contract.
  - Tests cover query forwarding, metadata selection, and error mapping for missing messages.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_read`
  - `cargo run -p nils-google-cli -- gmail search --help`

### Task 3.3: Implement Gmail Thread Get And Modify

- **Location**:
  - `crates/google-cli/src/gmail/thread.rs`
  - `crates/google-cli/tests/gmail_thread.rs`
- **Description**: Implement native `gmail thread get` and `gmail thread modify` behavior, including thread fetches and
  label add/remove semantics across messages in a thread.
- **Dependencies**:
  - Task 3.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Thread commands support at least the currently exposed get/modify surface.
  - Thread label mutation supports the current add/remove behavior.
  - Tests cover thread fetches, label mutation, and thread-not-found failures.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_thread`

### Task 3.4: Implement Gmail Send And MIME Assembly

- **Location**:
  - `crates/google-cli/src/gmail/send.rs`
  - `crates/google-cli/src/gmail/mime.rs`
  - `crates/google-cli/tests/gmail_send.rs`
- **Description**: Implement native `gmail send`, including MIME message assembly, attachment handling, reply/thread
  wiring, and Gmail API submission.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 4
- **Acceptance criteria**:
  - `gmail send` supports current scoped flags for body, subject, attachments, and thread targeting.
  - MIME generation uses a maintained crate rather than hand-built raw message assembly.
  - Tests cover attachment encoding, thread reply metadata, and failure mapping.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_send`

### Task 3.5: Finalize Gmail CLI Contracts, Docs, And Smoke Commands

- **Location**:
  - `crates/google-cli/src/cmd/gmail.rs`
  - `crates/google-cli/tests/gmail_cli_contract.rs`
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/gmail.md`
- **Description**: Align native Gmail help/output semantics, add CLI contract coverage, and update docs with runnable
  smoke commands.
- **Dependencies**:
  - Task 3.3
  - Task 3.2
  - Task 3.4
- **Complexity**: 2
- **Acceptance criteria**:
  - Gmail help and output semantics are native-specific and no longer describe pass-through behavior.
  - Contract tests cover both JSON and plain output modes for the supported Gmail surface.
  - Docs contain manual smoke commands for search, get, send, and thread modify, including how multi-account/default
    resolution affects Gmail commands.
- **Validation**:
  - `cargo test -p nils-google-cli --test gmail_cli_contract`
  - `cargo test -p nils-google-cli --test account_resolution_shared`
  - `rg -n "gmail search|gmail get|gmail send|thread modify|gog" crates/google-cli/README.md crates/google-cli/docs/gmail.md`

## Sprint 4: Native Drive Commands

**Goal**: Deliver native Drive list/search/get/download/upload behavior over the chosen Google API client stack.
**Parallelization**: Task 4.2 and Task 4.4 run in parallel after Task 4.1; Task 4.3 extends the read lane; Task 4.5
then integrates CLI tests and docs.
**PR grouping intent**: group
**Execution Profile**: parallel-x2
**Scorecard**:

- Execution Profile: parallel-x2
- TotalComplexity: 17
- CriticalPathComplexity: 13
- MaxBatchWidth: 2
- OverlapHotspots: `crates/google-cli/src/cmd/drive.rs` and `crates/google-cli/src/drive/mod.rs` are shared between the
  read/download and upload pipelines.
**Merge order**: Task 4.1; then Task 4.2 and Task 4.4 in parallel; then Task 4.3; then Task 4.5.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli --test drive_read`,
  `cargo test -p nils-google-cli --test drive_download`,
  `cargo test -p nils-google-cli --test drive_upload`,
  `cargo test -p nils-google-cli --test account_resolution_shared`,
  `cargo test -p nils-google-cli --test drive_cli_contract`,
  `cargo run -p nils-google-cli -- drive --help`
- Verify: Drive commands execute through native API clients and support both metadata and file-transfer operations.

### Task 4.1: Build Shared Drive Client And Metadata Types

- **Location**:
  - `crates/google-cli/src/drive/mod.rs`
  - `crates/google-cli/src/drive/client.rs`
  - `crates/google-cli/src/client.rs`
  - `crates/google-cli/tests/drive_read.rs`
  - `crates/google-cli/tests/account_resolution_shared.rs`
- **Description**: Establish the native Drive client wrapper, auth wiring, and reusable metadata adapters for list,
  search, get, download, and upload flows.
- **Dependencies**:
  - Task 3.5
- **Complexity**: 4
- **Acceptance criteria**:
  - Drive service initialization uses the native auth/token store.
  - Shared metadata adapters normalize Drive file responses into stable local types.
  - Tests cover client bootstrap, representative metadata decoding, and Drive account-resolution reuse in multi-account
    cases.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_read`
  - `cargo test -p nils-google-cli --test account_resolution_shared`

### Task 4.2: Implement Drive List, Search, And Get

- **Location**:
  - `crates/google-cli/src/drive/read.rs`
  - `crates/google-cli/tests/drive_read.rs`
- **Description**: Implement native read-oriented Drive commands, including query handling and metadata retrieval for
  `drive ls`, `drive search`, and `drive get`.
- **Dependencies**:
  - Task 4.1
- **Complexity**: 4
- **Acceptance criteria**:
  - `drive ls`, `drive search`, and `drive get` execute natively.
  - Read output modes match the local contract instead of wrapper passthrough behavior.
  - Tests cover paging/query arguments, metadata lookup, and file-not-found handling.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_read`
  - `cargo run -p nils-google-cli -- drive ls --help`

### Task 4.3: Implement Drive Download And Export Paths

- **Location**:
  - `crates/google-cli/src/drive/download.rs`
  - `crates/google-cli/tests/drive_download.rs`
- **Description**: Implement native download/export behavior, including destination handling, export formats, and error
  mapping for download flows.
- **Dependencies**:
  - Task 4.2
- **Complexity**: 3
- **Acceptance criteria**:
  - `drive download` executes natively for supported download/export cases.
  - Download paths, overwrite behavior, and export formats follow the native contract.
  - Tests cover destination-path handling, export/download behavior, and missing-file failures.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_download`

### Task 4.4: Implement Drive Upload And MIME Handling

- **Location**:
  - `crates/google-cli/src/drive/upload.rs`
  - `crates/google-cli/src/drive/mime.rs`
  - `crates/google-cli/tests/drive_upload.rs`
- **Description**: Implement native upload behavior, including parent selection, name overrides, MIME inference, and
  replace/convert flags where supported by the chosen API client.
- **Dependencies**:
  - Task 4.1
- **Complexity**: 4
- **Acceptance criteria**:
  - `drive upload` supports the scoped flag surface committed by the native contract.
  - MIME inference and explicit overrides behave deterministically.
  - Tests cover upload metadata, MIME selection, and replace-path behavior.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_upload`

### Task 4.5: Finalize Drive CLI Contracts, Docs, And Smoke Commands

- **Location**:
  - `crates/google-cli/src/cmd/drive.rs`
  - `crates/google-cli/tests/drive_cli_contract.rs`
  - `crates/google-cli/README.md`
  - `crates/google-cli/docs/drive.md`
- **Description**: Align native Drive help/output semantics, add CLI contract coverage, and update docs with runnable
  smoke commands.
- **Dependencies**:
  - Task 4.3
  - Task 4.2
  - Task 4.4
- **Complexity**: 2
- **Acceptance criteria**:
  - Drive help and output semantics are native-specific and no longer describe pass-through behavior.
  - Contract tests cover JSON/plain output for the supported Drive surface.
  - Docs contain manual smoke commands for list, search, get, upload, and download, including how multi-account/default
    resolution affects Drive commands.
- **Validation**:
  - `cargo test -p nils-google-cli --test drive_cli_contract`
  - `cargo test -p nils-google-cli --test account_resolution_shared`
  - `rg -n "drive ls|drive search|drive upload|drive download|gog" crates/google-cli/README.md crates/google-cli/docs/drive.md`

## Sprint 5: Native Integration, De-Wrapper Cleanup, And Release Gates

**Goal**: Remove the remaining wrapper assumptions, run live validation, and leave `google-cli` as a native crate with
no runtime dependency on `gog`.
**Parallelization**: Final integration gate stays serial to avoid churn while removing wrapper-era behavior and
collecting validation evidence.
**PR grouping intent**: per-sprint
**Execution Profile**: serial
**Scorecard**:

- Execution Profile: serial
- TotalComplexity: 15
- CriticalPathComplexity: 15
- MaxBatchWidth: 1
- OverlapHotspots: `crates/google-cli/src/runtime.rs`, `docs/specs/*google-cli*`, crate README/docs, and validation
  reports all move together when the wrapper model is deleted.
**Merge order**: single-lane sprint; merge tasks in listed order.
**Demo/Validation**:
- Command(s): `cargo test -p nils-google-cli`,
  `cargo test --workspace`,
  `scripts/workflow-lint.sh`,
  `scripts/workflow-test.sh`,
  `scripts/cli-standards-audit.sh --strict`,
  `bash scripts/docs-placement-audit.sh --strict`,
  `cargo run -p nils-google-cli -- auth --help`,
  `cargo run -p nils-google-cli -- gmail --help`,
  `cargo run -p nils-google-cli -- drive --help`
- Verify: the crate passes repo gates without `gog` runtime assumptions, and live smoke coverage is documented.

### Task 5.1: Remove Wrapper Runtime And `gog`-Specific Flags

- **Location**:
  - `crates/google-cli/src/runtime.rs`
  - `crates/google-cli/src/cmd/common.rs`
  - `crates/google-cli/src/main.rs`
  - `crates/google-cli/src/lib.rs`
  - `crates/google-cli/tests/native_no_gog.rs`
- **Description**: Delete the wrapper runtime and remaining `gog`-specific config/flags, replacing them with native
  runtime abstractions and help text.
- **Dependencies**:
  - Task 4.5
- **Complexity**: 4
- **Acceptance criteria**:
  - `GOOGLE_CLI_GOG_BIN` and wrapper-only pass-through flags are removed or replaced with native equivalents.
  - No production code path shells out to `gog`.
  - PATH-scrubbed and override-clean tests prove native commands run without a `gog` binary.
  - Help text and command metadata describe a native Rust implementation.
- **Validation**:
  - `rg -n "std::process::Command|tokio::process::Command|GOOGLE_CLI_GOG_BIN|gog" crates/google-cli/src`
  - `cargo test -p nils-google-cli --test native_no_gog`
  - `cargo test -p nils-google-cli --lib`

### Task 5.2: Execute Full Native Test Matrix And Live Smoke Validation

- **Location**:
  - `crates/google-cli/tests/auth_storage.rs`
  - `crates/google-cli/tests/auth_oauth_flow.rs`
  - `crates/google-cli/tests/auth_cli_contract.rs`
  - `crates/google-cli/tests/auth_account_resolution.rs`
  - `crates/google-cli/tests/gmail_read.rs`
  - `crates/google-cli/tests/gmail_thread.rs`
  - `crates/google-cli/tests/gmail_send.rs`
  - `crates/google-cli/tests/gmail_cli_contract.rs`
  - `crates/google-cli/tests/drive_read.rs`
  - `crates/google-cli/tests/drive_download.rs`
  - `crates/google-cli/tests/drive_upload.rs`
  - `crates/google-cli/tests/drive_cli_contract.rs`
  - `crates/google-cli/tests/account_resolution_shared.rs`
  - `crates/google-cli/tests/native_no_gog.rs`
  - `docs/reports/google-cli-native-validation-report.md`
- **Description**: Run the full native test matrix and a documented live smoke pass for auth, Gmail, and Drive using the
  loopback callback flow and cleanup rules for sent mail / uploaded files.
- **Dependencies**:
  - Task 5.1
- **Complexity**: 5
- **Acceptance criteria**:
  - Native tests pass without relying on `fake_gog` or a real `gog` binary.
  - Validation report records both automated test results and live smoke results.
  - Validation includes Gmail and Drive coverage for multi-account/no-default versus default-present resolution paths.
  - Live validation includes account restore/cleanup steps so the test leaves no stray Drive files and no broken auth
    state.
- **Validation**:
  - `cargo test -p nils-google-cli`
  - `rg -n "Live smoke|auth add|gmail send|drive upload|cleanup" docs/reports/google-cli-native-validation-report.md`

### Task 5.3: Migrate Specs And Docs From Wrapper Language To Native Language

- **Location**:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/specs/cli-standards-mapping.md`
  - `docs/specs/cli-error-code-registry.md`
  - `crates/google-cli/README.md`
- **Description**: Update repo-wide docs, architecture notes, and CLI standards mappings so `google-cli` is documented
  as a native client rather than a `gog` wrapper.
- **Dependencies**:
  - Task 5.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Root and crate docs no longer advertise `google-cli` as a wrapper.
  - Specs describe native error/output/runtime behavior and the no-UI auth-management stance.
  - Architecture docs explain the selected native dependency stack and why `gog` is no longer needed.
- **Validation**:

  ```bash
  rg -n "Rust wrapper over gog|shells out to gog|GOOGLE_CLI_GOG_BIN" \
    README.md docs/ARCHITECTURE.md docs/specs/cli-standards-mapping.md \
    docs/specs/cli-error-code-registry.md crates/google-cli/README.md
  ```

### Task 5.4: Finalize Dependency Audit, Rollback Notes, And Release Readiness

- **Location**:
  - `docs/reports/google-cli-native-crate-survey.md`
  - `docs/reports/google-cli-native-validation-report.md`
  - `docs/reports/cli-command-inventory.md`
  - `Cargo.toml`
  - `Cargo.lock`
- **Description**: Freeze the dependency decision record, record rollback instructions, and confirm the release/packaging
  docs match the new native runtime.
- **Dependencies**:
  - Task 5.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Survey report marks the final chosen crates and rejected alternatives.
  - Rollback notes describe how to revert to the last wrapper release if native rollout fails.
  - Command inventory and dependency files are consistent with the native implementation.
- **Validation**:
  - `cargo tree -p nils-google-cli`

  ```bash
  rg -n "rollback|final choice|rejected|native" \
    docs/reports/google-cli-native-crate-survey.md \
    docs/reports/google-cli-native-validation-report.md \
    docs/reports/cli-command-inventory.md
  ```

## Testing Strategy

- Unit: config/account-resolution logic, OAuth state/callback parsing, MIME assembly, and request/response adapters.
- Integration: mock HTTP or loopback-server tests for OAuth exchange and Google API calls using a chosen test server
  crate from Sprint 1.
- CLI contract: native command tests for auth, Gmail, and Drive help/output/error behavior.
- Live E2E/manual: real Google account validation for loopback auth, Gmail send/search/get/thread operations, and Drive
  upload/search/get/download with cleanup.
- Repo gates: `scripts/workflow-lint.sh`, `bash scripts/docs-placement-audit.sh --strict`, `cargo test --workspace`,
  and `scripts/workflow-test.sh`.

## Risks & gotchas

- The generated Google API crates may not cover every edge of the currently exposed CLI surface cleanly; Sprint 1 must
  keep a documented `reqwest` fallback path for blocked operations.
- OAuth state handling is easy to get subtly wrong; remote/manual flows need explicit tests because the wrapper-era live
  bug proved this path can fail even when Google auth itself succeeds.
- Keyring behavior differs across platforms, so config/keyring fallback and error messages must be deterministic.
- Multi-account semantics are a product decision as much as an implementation detail; if left vague, `auth status` and
  account-targeted commands will regress again.
- Removing `gog` pass-through flags is a breaking UX change unless help text and docs clearly map old behavior to new
  native commands.

## Rollback plan

1. Keep the last known-good wrapper release tagged before native migration starts and record that version in the native
   validation report.
2. Revert `crates/google-cli` to the wrapper-era module layout and restore the wrapper contract/spec files in one
   rollback changeset.
3. Re-pin `Cargo.toml`/`Cargo.lock` to the wrapper-era dependency set and remove the native Google client crates.
4. Restore root/crate docs that describe `google-cli` as a `gog` wrapper and remove native-only reports/specs.
5. Re-run `cargo test --workspace`, `scripts/workflow-lint.sh`, `scripts/workflow-test.sh`, and the wrapper help
   commands to confirm the repo is back on the previous runtime model.

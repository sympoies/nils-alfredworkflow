# Plan: Migrate generic dot-namespaced error codes to `NILS_<DOMAIN>_NNN`

## Overview

`workflow-cli`, `market-cli`, `weather-cli`, and `workflow-readme-cli` still
emit legacy dot-namespaced error codes (e.g. `user.invalid_input`,
`runtime.serialize_failed`) defined as `const ERROR_CODE_*` strings. PR2
([#149](https://github.com/sympoies/nils-alfredworkflow/pull/149)) migrated the
12 script-filter CLIs but skipped these four because the codes are
generic/cross-crate rather than per-crate `<crate>.<kind>` strings, and because
`workflow-readme-cli` has no reserved range yet in
`docs/specs/cli-error-code-registry.md`. This plan finishes the migration:
extends the registry, picks the correct `NILS_*` slot for every dot-code,
deletes the now-dead `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` constants left over
from PR1's `select_output_mode` removal, updates contract tests, and ships the
result as a single breaking-change PR via the standard `create-feature-pr`
flow.

## Scope

- In scope:
  - Add `NILS_WORKFLOW_README_*` reserved range (`001-099`) and 12 seed rows
    to `docs/specs/cli-error-code-registry.md`.
  - Add `NILS_WORKFLOW_003` seed row for `usage log persistence failure`.
  - Migrate every active dot-namespaced `ERROR_CODE_*` constant in
    `crates/workflow-cli`, `crates/market-cli`, `crates/weather-cli`,
    `crates/workflow-readme-cli` to its registered `NILS_<DOMAIN>_NNN` value
    (or to a `NILS_COMMON_*` slot when the semantic is generic).
  - Delete the `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` constants (dead code
    once PR1 lands; no producers remain).
  - Update unit + integration contract tests (`Some("…")`, `assert_eq!(err.code, …)`)
    for every renamed code.
  - Audit `workflows/*/scripts/*.sh` and `workflows/*/tests/smoke.sh` for any
    `error.code` consumer; confirm there are none, otherwise patch.
  - Run `scripts/local-pre-commit.sh` and ship via `create-feature-pr`.
- Out of scope:
  - Migrating `google-cli` (already on `NILS_GOOGLE_*`).
  - Migrating the 12 script-filter CLIs (already done in PR2 #149).
  - Refactoring `ErrorKind` enums to gain finer-grained variants beyond what
    the existing dot-codes already encoded.

## Assumptions

1. PR1 ([#148](https://github.com/sympoies/nils-alfredworkflow/pull/148)) and
   PR2 ([#149](https://github.com/sympoies/nils-alfredworkflow/pull/149)) are
   merged into `main` before this plan starts. If they are not, branch from
   `main` after they land, or rebase the working branch once they do — both
   touch `crates/{workflow,market,weather,workflow-readme}-cli/src/main.rs`
   and would conflict with Sprint 2 here.
2. No external consumer scripts (outside `workflows/*/scripts/*.sh`)
   pattern-match these codes. The audit task in Sprint 2 verifies this; if
   surprises appear, escalate before shipping.
3. `cli-shared-runtime-contract.md` Section "Forbidden Legacy Compatibility
   Aliases" already forbids dot-format codes for new code; this plan brings
   the four lagging crates into compliance.
4. `_001` = canonical user-input bucket and `_002` = canonical runtime/upstream
   bucket conventions (established by PR2) carry into `NILS_WORKFLOW_README_*`,
   except where the existing dot-codes encode finer granularity worth
   preserving as `_003+`.
5. `scripts/local-pre-commit.sh` is the gate stack of record (workflow-lint,
   script-filter-policy, cambridge scraper test, third-party artifacts, every
   workflow smoke test). `cargo test --workspace` is implicitly covered by it.
6. `plan-tooling` 0.7.3+ is on `PATH` for `validate` / `to-json` / `batches` /
   `split-prs` checks.

## Code → registry mapping

This is the canonical mapping that Sprint 2 implements. Every row must land
exactly as written; any deviation requires updating both the spec and the code
in lockstep.

| Crate | Current constant (value) | New `NILS_*` slot | Notes |
| --- | --- | --- | --- |
| `workflow-cli` | `ERROR_CODE_USER_INVALID_PATH` (`user.invalid_path`) | `NILS_WORKFLOW_001` | Spec already lists this seed (`project path not found/not directory`). |
| `workflow-cli` | `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` (`user.output_mode_conflict`) | *(delete constant)* | Dead after PR1 removed `select_output_mode`. No producer remains. |
| `workflow-cli` | `ERROR_CODE_RUNTIME_GIT` (`runtime.git_failed`) | `NILS_WORKFLOW_002` | Spec already lists this seed (`git origin/command failure`). |
| `workflow-cli` | `ERROR_CODE_RUNTIME_USAGE_WRITE` (`runtime.usage_persist_failed`) | `NILS_WORKFLOW_003` | New seed; add to spec in Sprint 1. |
| `workflow-cli` | `ERROR_CODE_RUNTIME_SERIALIZE` (`runtime.serialize_failed`) | `NILS_COMMON_005` | Generic "internal serialization/runtime failure" already in spec. |
| `market-cli` | `ERROR_CODE_USER_INVALID_INPUT` (`user.invalid_input`) | `NILS_MARKET_001` | Spec seed reads `invalid symbol/amount expression`; extend description if needed. |
| `market-cli` | `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` | *(delete constant)* | Dead after PR1. |
| `market-cli` | `ERROR_CODE_RUNTIME_PROVIDER_INIT` (`runtime.provider_init_failed`) | `NILS_MARKET_002` | Spec seed `provider unavailable/rate-limited`; same upstream bucket. |
| `market-cli` | `ERROR_CODE_RUNTIME_PROVIDER_FAILED` (`runtime.provider_failed`) | `NILS_MARKET_002` | Same bucket; no new seed needed. |
| `market-cli` | `ERROR_CODE_RUNTIME_SERIALIZE` (`runtime.serialize_failed`) | `NILS_COMMON_005` | Generic. |
| `weather-cli` | `ERROR_CODE_USER_INVALID_INPUT` (`user.invalid_input`) | `NILS_WEATHER_001` | Spec seed `invalid location arguments`. |
| `weather-cli` | `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` | *(delete constant)* | Dead after PR1. |
| `weather-cli` | `ERROR_CODE_RUNTIME_PROVIDER_INIT` (`runtime.provider_init_failed`) | `NILS_WEATHER_002` | Spec seed `weather provider unavailable`. |
| `weather-cli` | `ERROR_CODE_RUNTIME_SERIALIZE` (`runtime.serialize_failed`) | `NILS_COMMON_005` | Generic. |
| `workflow-readme-cli` | `ERROR_CODE_USER_INVALID_WORKFLOW_ROOT` (`user.invalid_workflow_root`) | `NILS_WORKFLOW_README_001` | New range; add to spec in Sprint 1. |
| `workflow-readme-cli` | `ERROR_CODE_USER_INVALID_README_SOURCE` (`user.invalid_readme_source`) | `NILS_WORKFLOW_README_002` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_README_NOT_FOUND` (`user.readme_not_found`) | `NILS_WORKFLOW_README_003` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_PLIST_NOT_FOUND` (`user.plist_not_found`) | `NILS_WORKFLOW_README_004` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_REMOTE_IMAGE_NOT_ALLOWED` (`user.remote_image_not_allowed`) | `NILS_WORKFLOW_README_005` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_INVALID_IMAGE_PATH` (`user.invalid_image_path`) | `NILS_WORKFLOW_README_006` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_IMAGE_NOT_FOUND` (`user.image_not_found`) | `NILS_WORKFLOW_README_007` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_USER_PLIST_README_KEY_MISSING` (`user.plist_readme_key_missing`) | `NILS_WORKFLOW_README_008` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_RUNTIME_READ_FAILED` (`runtime.read_failed`) | `NILS_WORKFLOW_README_009` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_RUNTIME_WRITE_FAILED` (`runtime.write_failed`) | `NILS_WORKFLOW_README_010` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_RUNTIME_CREATE_DIR_FAILED` (`runtime.create_dir_failed`) | `NILS_WORKFLOW_README_011` | New seed. |
| `workflow-readme-cli` | `ERROR_CODE_RUNTIME_COPY_FAILED` (`runtime.copy_failed`) | `NILS_WORKFLOW_README_012` | New seed. |

## Sprint 1: Spec extension and dead-code cleanup

**Goal**: Update `docs/specs/cli-error-code-registry.md` with the
`NILS_WORKFLOW_README_*` reserved range, the new `NILS_WORKFLOW_003` seed, and
remove the dead `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` constants. Workspace
must still build because the dead constants have no remaining callers.
**Demo/Validation**:

- Command(s):
  - `cargo check --workspace --tests`
  - `rg -nE 'ERROR_CODE_USER_OUTPUT_MODE_CONFLICT' crates/`
- Verify:
  - Spec contains `NILS_WORKFLOW_README_` row in the Domain Allocation table
    and 12 seed rows; `NILS_WORKFLOW_003` seed row is added.
  - `rg` returns no remaining references to the deleted constant in any of
    the four crates.
  - `cargo check` succeeds with no `unused_const` warnings on the touched
    crates.

### Task 1.1: Extend `docs/specs/cli-error-code-registry.md`

- **Location**:
  - `docs/specs/cli-error-code-registry.md`
- **Description**: Add `NILS_WORKFLOW_README_` row (range `001-099`) to the
  `Domain Allocation (Unique Ranges)` table in alphabetical position. In the
  `Seed Registry` table, insert the 12 `NILS_WORKFLOW_README_001`..`_012`
  rows from the mapping table above and the new `NILS_WORKFLOW_003` row
  (`usage log persistence failure`). Preserve the 80-char per-line lint
  posture; if a description overflows, fence the table or wrap the row per
  the existing Markdown style.
- **Dependencies**:
  - none
- **Acceptance criteria**:
  - Domain Allocation table contains `workflow-readme-cli` row.
  - Seed Registry contains all 13 new rows in stable sort order.
  - `markdownlint` (run via `scripts/workflow-lint.sh` later) passes.
- **Validation**:
  - `rg -n 'NILS_WORKFLOW_README_|NILS_WORKFLOW_003' docs/specs/cli-error-code-registry.md`
    returns 13 hits.

### Task 1.2: Remove dead `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` constants

- **Location**:
  - `crates/workflow-cli/src/main.rs`
  - `crates/market-cli/src/main.rs`
  - `crates/weather-cli/src/main.rs`
- **Description**: Delete the `const ERROR_CODE_USER_OUTPUT_MODE_CONFLICT: &str = "user.output_mode_conflict";`
  line from each of the three crates. Confirm no remaining producer uses it
  (PR1 removed the `select_output_mode` helper and the conflict tests). Do
  not touch `workflow-readme-cli` — it never had this constant.
- **Dependencies**:
  - none
- **Acceptance criteria**:
  - `rg -n 'ERROR_CODE_USER_OUTPUT_MODE_CONFLICT' crates/` returns zero hits.
  - `cargo check --workspace --tests` succeeds without warnings about an
    unused constant.
- **Validation**:
  - `cargo check --workspace --tests 2>&1 | rg 'ERROR_CODE_USER_OUTPUT_MODE_CONFLICT|unused_const'`
    returns nothing.

## Sprint 2: Code migration in four crates

**Goal**: Replace every dot-namespaced `ERROR_CODE_*` value in the four crates
with the registry-aligned `NILS_*` constant per the mapping table. Each crate
is one independently testable task; ordering inside the sprint is not
significant because the four crates do not share modules.
**Demo/Validation**:

- Command(s):
  - `cargo test --workspace`
  - `rg -nE '"(user|runtime)\.[a-z_]+"' crates/{workflow,market,weather,workflow-readme}-cli/src/`
- Verify:
  - Workspace tests pass (no contract test left asserting the old dot-codes
    after Sprint 3 updates the test fixtures).
  - The `rg` audit returns zero matches inside the four target crates.

**PR grouping intent**: per-sprint
**Execution Profile**: serial

### Task 2.1: Migrate `workflow-cli` constants

- **Location**:
  - `crates/workflow-cli/src/main.rs`
- **Description**: Update the surviving four constants to their `NILS_*`
  values per the mapping table:
  - `ERROR_CODE_USER_INVALID_PATH` → `"NILS_WORKFLOW_001"`
  - `ERROR_CODE_RUNTIME_GIT` → `"NILS_WORKFLOW_002"`
  - `ERROR_CODE_RUNTIME_USAGE_WRITE` → `"NILS_WORKFLOW_003"`
  - `ERROR_CODE_RUNTIME_SERIALIZE` → `"NILS_COMMON_005"`
- **Dependencies**:
  - Task 1.1
  - Task 1.2
- **Acceptance criteria**:
  - All four constants emit canonical `NILS_*` values.
  - No dot-code string survives in `src/main.rs`.
- **Validation**:
  - `rg -nE '"user\.|"runtime\.' crates/workflow-cli/src/main.rs` returns
    zero hits.
  - `cargo check -p nils-workflow-cli --tests` passes.

### Task 2.2: Migrate `market-cli` constants

- **Location**:
  - `crates/market-cli/src/main.rs`
- **Description**: Update the surviving four constants:
  - `ERROR_CODE_USER_INVALID_INPUT` → `"NILS_MARKET_001"`
  - `ERROR_CODE_RUNTIME_PROVIDER_INIT` → `"NILS_MARKET_002"`
  - `ERROR_CODE_RUNTIME_PROVIDER_FAILED` → `"NILS_MARKET_002"`
  - `ERROR_CODE_RUNTIME_SERIALIZE` → `"NILS_COMMON_005"`
- **Dependencies**:
  - Task 1.1
  - Task 1.2
- **Acceptance criteria**:
  - All four constants emit canonical `NILS_*` values.
  - `PROVIDER_INIT` and `PROVIDER_FAILED` legitimately collapse onto the
    same `NILS_MARKET_002` slot per spec; preserve both Rust constants for
    code readability even though they share the wire value.
- **Validation**:
  - `rg -nE '"user\.|"runtime\.' crates/market-cli/src/main.rs` returns zero
    hits.
  - `cargo check -p nils-market-cli --tests` passes.

### Task 2.3: Migrate `weather-cli` constants

- **Location**:
  - `crates/weather-cli/src/main.rs`
- **Description**: Update the surviving three constants:
  - `ERROR_CODE_USER_INVALID_INPUT` → `"NILS_WEATHER_001"`
  - `ERROR_CODE_RUNTIME_PROVIDER_INIT` → `"NILS_WEATHER_002"`
  - `ERROR_CODE_RUNTIME_SERIALIZE` → `"NILS_COMMON_005"`
- **Dependencies**:
  - Task 1.1
  - Task 1.2
- **Acceptance criteria**:
  - All three constants emit canonical `NILS_*` values.
- **Validation**:
  - `rg -nE '"user\.|"runtime\.' crates/weather-cli/src/main.rs` returns zero
    hits.
  - `cargo check -p nils-weather-cli --tests` passes.

### Task 2.4: Migrate `workflow-readme-cli` constants

- **Location**:
  - `crates/workflow-readme-cli/src/lib.rs`
- **Description**: Replace the 12 `ERROR_CODE_*` constant values with their
  new `NILS_WORKFLOW_README_001`..`_012` strings per the mapping table.
  Constant identifiers stay the same; only the string literals change.
- **Dependencies**:
  - Task 1.1
- **Acceptance criteria**:
  - All 12 constants emit `NILS_WORKFLOW_README_NNN` values.
  - No dot-code string survives anywhere in `src/lib.rs` or `src/main.rs`.
- **Validation**:
  - `rg -nE '"user\.|"runtime\.' crates/workflow-readme-cli/` returns zero
    hits.
  - `cargo check -p nils-workflow-readme-cli --tests` passes.

## Sprint 3: Test alignment, audit, and ship

**Goal**: Update every test that asserts the old code literals, audit
workflow shell consumers, run the full local pre-commit gate, then commit and
open a draft PR via `create-feature-pr`.
**Demo/Validation**:

- Command(s):
  - `scripts/local-pre-commit.sh`
  - `gh pr view --json url,isDraft`
- Verify:
  - `local-pre-commit.sh` exits zero (workflow-lint, script-filter-policy,
    cambridge scraper, third-party artifact tests, every smoke test pass).
  - PR is open as draft via `create-feature-pr`.

### Task 3.1: Update integration and unit test asserts

- **Location**:
  - `crates/workflow-cli/tests/integration/cli_contract.rs`
  - `crates/workflow-cli/src/main.rs` (in-file `#[cfg(test)] mod tests`)
  - `crates/market-cli/tests/integration/cli_contract.rs`
  - `crates/market-cli/src/main.rs` (in-file tests)
  - `crates/weather-cli/tests/integration/cli_contract.rs`
  - `crates/weather-cli/src/main.rs` (in-file tests)
  - `crates/workflow-readme-cli/tests/integration/cli_contract.rs`
  - `crates/workflow-readme-cli/src/lib.rs` (in-file tests, if any)
- **Description**: Replace every `Some("user.X")` / `Some("runtime.X")`
  literal and every `assert_eq!(err.code, "…")` with the new `NILS_*`
  value. Use the mapping table — do not re-derive. Watch for the
  `unknown_output_value_is_rejected_by_clap` tests added in PR1; they no
  longer assert any `error.code` and should be left alone.
- **Dependencies**:
  - Task 2.1
  - Task 2.2
  - Task 2.3
  - Task 2.4
- **Acceptance criteria**:
  - The audit command
    `rg -nE '"user\.|"runtime\.' crates/workflow-cli/ crates/market-cli/ crates/weather-cli/ crates/workflow-readme-cli/`
    returns zero hits across both `src/` and `tests/`.
  - `cargo test --workspace` passes.
- **Validation**:
  - `cargo test --workspace 2>&1 | rg 'FAILED|test result: FAILED'` returns
    nothing.

### Task 3.2: Audit workflow shell consumers

- **Location**:
  - `workflows/google-service/scripts/script_filter.sh`
  - `workflows/google-service/tests/smoke.sh`
  - `workflows/open-project/scripts/script_filter.sh`
  - `workflows/weather/TROUBLESHOOTING.md`
- **Description**: Run the audit `rg` (see Acceptance criteria) over the
  whole `workflows/` tree to find any consumer that pattern-matches the old
  dot-codes. Listed locations are representative entry points to inspect by
  hand if `rg` returns hits. Expected outcome: zero hits, because the four
  crates' codes are surfaced only inside the JSON envelope and current shell
  scripts ignore `error.code`. If any hit appears, patch the offending file
  in this same task.
- **Dependencies**:
  - none
- **Acceptance criteria**:
  - The following audit returns either zero hits, or a list that this task
    explicitly addresses:

    ```sh
    rg -nE 'user\.invalid_path|user\.invalid_input|user\.invalid_workflow_root|user\.invalid_readme_source|user\.readme_not_found|user\.plist_not_found|user\.remote_image_not_allowed|user\.invalid_image_path|user\.image_not_found|user\.plist_readme_key_missing|runtime\.git_failed|runtime\.usage_persist_failed|runtime\.provider_init_failed|runtime\.provider_failed|runtime\.serialize_failed|runtime\.read_failed|runtime\.write_failed|runtime\.create_dir_failed|runtime\.copy_failed' workflows/
    ```

  - If hits are found, the audit log notes which scripts were updated and
    why; otherwise note "no shell consumers, audit clean" in the PR body.
- **Validation**:
  - `scripts/local-pre-commit.sh` passes (smoke tests would notice any
    consumer that was actually using these codes).

### Task 3.3: Run `local-pre-commit.sh` to green

- **Location**:
  - n/a (whole-repo gate)
- **Description**: Execute `scripts/local-pre-commit.sh`. If it fails, fix
  the underlying issue (do not skip hooks). Common failure modes:
  cargo-fmt drift after manual edits, `markdownlint` complaint on the spec
  table, or a stray missed test assertion.
- **Dependencies**:
  - Task 3.1
  - Task 3.2
- **Acceptance criteria**:
  - Final invocation exits 0 with the line `ok: local pre-commit checks
    passed (mode=default, package_smoke=0, skip_node_scraper_tests=0)`.
- **Validation**:
  - Re-run is idempotent — second invocation also exits 0.

### Task 3.4: Commit via `semantic-commit` and open draft PR

- **Location**:
  - n/a (git + GitHub)
- **Description**: Stage all changes, write a `feat(cli):` commit explaining
  the migration. The commit body must enumerate the renamed constants and
  call out the breaking change. Then invoke the `create-feature-pr` skill;
  PR body uses the standard four-section template
  (`Summary` / `Changes` / `Testing` / `Risk / Notes`). Title:
  "Migrate generic dot-namespaced error codes in workflow/market/weather/
  workflow-readme CLIs to NILS_<DOMAIN>_NNN".
  Branch suggestion: `feat/cli-generic-error-codes-nils`.
- **Dependencies**:
  - Task 3.3
- **Acceptance criteria**:
  - `git log -1 --pretty=%H` returns the new feature commit.
  - `gh pr view --json url,isDraft` shows `"isDraft": true` and the URL.
- **Validation**:
  - Capture the PR number from `gh pr view --json number -q .number` and
    confirm `gh pr checks "$pr"` reports a non-empty check list (queued,
    running, or completed).

## Testing Strategy

- Unit: each crate has in-file `#[cfg(test)] mod tests`; assertions on
  `err.code` and `Some(...)` envelope literals exercise the new `NILS_*`
  values.
- Integration: every `crates/*-cli/tests/integration/cli_contract.rs` runs
  the built binary and asserts the envelope `error.code`. Sprint 3 updates
  these literal expectations.
- Smoke: `scripts/local-pre-commit.sh` runs every workflow's
  `tests/smoke.sh`; these do not pattern-match `error.code` today, so they
  serve as a regression net rather than an active assertion (Sprint 3.2
  audit reconfirms this).
- Manual: not required. If you want a sanity check, run
  `cargo run -q -p nils-weather-cli -- today --output json` (intentionally
  missing `--city`) and confirm the emitted envelope contains
  `"code": "NILS_WEATHER_001"` instead of `"user.invalid_input"`.

## Risks & gotchas

- **Sequencing with PR1 / PR2**: this plan touches four files
  (`crates/{workflow,market,weather,workflow-readme}-cli/src/main.rs`) that
  PR1 [#148](https://github.com/sympoies/nils-alfredworkflow/pull/148) also
  modified. Wait for PR1 to merge into `main` before opening this PR's
  branch, or rebase as soon as it lands.
- **PROVIDER_INIT and PROVIDER_FAILED collapsing**: in `market-cli` two Rust
  constants legitimately point at the same wire value (`NILS_MARKET_002`).
  Future maintainers may interpret this as a bug — Sprint 2.2 keeps both
  identifiers for code-site readability and adds an inline comment if
  ambiguity is plausible.
- **`workflow-readme-cli` seed sprawl**: 12 fine-grained codes is a large
  registry block. If review pushback wants only `_001` (user input) and
  `_002` (runtime), the migration shrinks but loses the existing fine
  granularity that the dot-codes already encode. Default position: keep all
  12 because the spec already supports `_003+` reserved space and the
  fine-grained codes carry real diagnostic value.
- **Hidden shell consumers**: Sprint 3.2 audits `workflows/`, but a third
  party (Alfred user script, monitoring tool) outside the repo may parse
  `error.code`. The release notes / PR body must call out the breaking
  change in the same way PR1 / PR2 did.
- **Dead constant removal triggering compile error**: if PR1's
  `select_output_mode` removal somehow did not fully land,
  `ERROR_CODE_USER_OUTPUT_MODE_CONFLICT` may still have a producer. Sprint
  1.2's `cargo check` is the gate; if it fails, restore the constant for
  that crate and add a follow-up note in the PR.
- **Missed test fixtures**: integration tests in some crates assert the
  exact envelope JSON via `serde_json::Value::get("code")`. If Sprint 3.1
  misses one, `cargo test` will fail loudly — pay attention to which crate
  the failure comes from before retrying.
- **Spec markdown lint**: the seed table is long. If `markdownlint` MD013
  trips on a description line, switch the offending row's description to a
  shorter phrasing rather than fence-blocking the whole table.

## Rollback plan

- Single PR; revert the merge commit on `main` to undo every change at once
  (`git revert -m 1 <merge-sha>`).
- No data migration, schema change, or external state to roll back.
- If only the registry change must be kept (because consumers already
  upgraded), revert just the four crate `src/` commits; the registry rows
  and the dead-constant deletions are safe to leave in place.

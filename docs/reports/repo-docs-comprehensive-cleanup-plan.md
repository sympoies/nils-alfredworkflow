# Plan: `Repo Docs Comprehensive Cleanup`

## Overview

Audit every markdown doc in `nils-alfredworkflow` against the current code, scripts, Cargo manifests, workflow manifests,
and shared specs; then realign root files, `docs/`, `docs/specs/`, all 21 crate docs, and all 21 workflow docs to match
reality. Layered top-down so canonical contracts (specs) are stable before crate / workflow docs are revised. Each
sprint produces one PR; tasks inside a sprint are serial so the doc-cleanup PR stays coherent and review diffs cluster
by ownership boundary.

Two sprint shapes appear:

- **Real-dep sprints** (Sprint 1, 3, 4, 8): tasks share files or contract surfaces. Serial ordering is required by
  file-level write conflict (e.g., Sprint 1 tasks 1.1/1.2 both edit `TROUBLESHOOTING.md`).
- **Batch sprints** (Sprint 5, 6, 7): tasks operate on disjoint crate trees with no file-level overlap. Serial
  ordering is for PR diff coherence, not write contention. Sprint 5/6/7 carry 5 tasks each, which is one over the
  rigorous serial-target 2-4. The trade-off is intentional: collapsing into more sprints would split a single
  domain (search/media, utility, apps) across PR boundaries; using parallel-xN modes is incompatible with the
  `per-sprint` PR grouping intent. Total and critical-path complexity stay within the 8-16 serial ceiling.

## Scope

- In scope:
  - Root markdown: `README.md`, `AGENTS.md`, `DEVELOPMENT.md`, `ALFRED_WORKFLOW_DEVELOPMENT.md`,
    `BINARY_DEPENDENCIES.md`, `TROUBLESHOOTING.md`.
  - `docs/`: `ARCHITECTURE.md`, `PACKAGING.md`, `RELEASE.md`.
  - `docs/specs/`: 12 specs (active / frozen / drift classification + content fixes).
  - `crates/<name>/README.md` and `crates/<name>/docs/*` across 21 crates (including 1 crate that is missing its
    `docs/` tree).
  - `workflows/<id>/README.md` and `workflows/<id>/TROUBLESHOOTING.md` across 21 workflows.
  - Cross-doc reference integrity (links, command paths, env vars).
- Out of scope:
  - Rewriting the canonical content of stable specs (only drift is fixed; spec design changes are deferred).
  - Generated artifacts (`THIRD_PARTY_LICENSES.md`, `THIRD_PARTY_NOTICES.md`) — refreshed only via existing
    generator script.
  - Code or behavior changes outside markdown; the only allowed code edits are doc-related script tweaks (e.g.,
    `scripts/docs-placement-audit.sh` allowlists if a new doc location is added).
  - Translating docs (zh/en mixing, terminology re-standardization).
  - New feature docs for unshipped work.

## Assumptions

1. The 12 `docs/specs/*.md` files are the canonical contracts; drift is corrected inside the spec, not by
   working around it elsewhere.
2. `crate-docs-placement-policy.md` is binding; every publishable crate needs `README.md` plus `docs/README.md`.
3. `ALFRED_WORKFLOW_DEVELOPMENT.md` line 133 four-section TROUBLESHOOTING contract is binding for every
   non-template workflow; the `_template` workflow is allowed to carry an extra `## Placeholder checklist`
   guidance section.
4. `_template` placeholder section stays; `bilibili-search` first-release support window section is removed as
   bilibili is now post-D2.
5. The package version stamp (`workflows/<id>/workflow.toml::version` and `Cargo.toml::workspace.package.version`)
   is **not** updated by this plan.
6. No new spec is introduced; only navigation / drift / dedup edits inside existing specs.
7. `scripts/docs-placement-audit.sh --strict` and `bash scripts/ci/markdownlint-audit.sh --strict` are the
   ground-truth gates after each sprint.
8. Per-sprint serial execution is intentional: each task within a sprint targets a different file family but
   reviewers expect to read the whole PR linearly. No cross-sprint execution parallelism is implied; sprints are
   sequential integration gates.

## Sprint 1: `Root-Layer Doc Alignment`

**Goal**: Realign root markdown so navigation, inventories, and tooling references reflect the current scripts,
crate names, and workflow set.

**Demo/Validation**:

- Command(s):
  - `bash scripts/docs-placement-audit.sh --strict`
  - `bash scripts/ci/markdownlint-audit.sh --strict`
  - `rg -n "TROUBLESHOOTING\\.md" ALFRED_WORKFLOW_DEVELOPMENT.md TROUBLESHOOTING.md`
- Verify: workflow inventory in `ALFRED_WORKFLOW_DEVELOPMENT.md` lists all 21 workflows; root
  `TROUBLESHOOTING.md` declares its routing policy; lint gates pass.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 14
**CriticalPathComplexity**: 14
**MaxBatchWidth**: 1
**OverlapHotspots**: T1.1 and T1.2 both edit `TROUBLESHOOTING.md`; T1.1 must finish first so routing edits in T1.2
build on the corrected inventory. T1.4 reads `workflows/*/workflow.toml` (no write); no overlap with T1.1-3.

### Task 1.1: `Reconcile workflow inventory in standards doc`

- **Location**:
  - `ALFRED_WORKFLOW_DEVELOPMENT.md` (lines 322-348 inventory + line 133 section contract)
- **Description**: Add the 5 missing workflows (`bangumi-search`, `bilibili-search`, `google-service`,
  `netflix-search`, `steam-search`) to the `### Workflow-local runbooks` list. Sort alphabetically. Confirm the
  required-section list under `### Required sections for each workflow troubleshooting file` still matches the
  4-section contract; do not relax it.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - All 21 workflow IDs from `ls workflows/` (excluding `_template`-only filtering decisions) appear under the
    runbooks list, alphabetized.
  - The four required section names remain exactly: `## Quick operator checks`, `## Common failures and actions`,
    `## Validation`, `## Rollback guidance`.
- **Validation**:
  - `diff <(ls workflows | sort) <(rg -oP 'workflows/\\K[a-z0-9-]+(?=/TROUBLESHOOTING\\.md)' ALFRED_WORKFLOW_DEVELOPMENT.md | sort -u)`
    returns no diff (or only `_template` if intentionally excluded — record the decision in PR body).

### Task 1.2: `Decide and implement root TROUBLESHOOTING routing policy`

- **Location**:
  - `TROUBLESHOOTING.md`
- **Description**: Convert the root file into a documented routing index. Pick one policy and state it in the
  intro: either (a) curated high-traffic subset with an explicit "full list lives in
  `ALFRED_WORKFLOW_DEVELOPMENT.md`" pointer, or (b) full mirror of all 21 workflows. Default recommendation: (a)
  curated, because root file currently lists 6 workflows and explicit curation is the lower-churn option.
  Document the inclusion rule (e.g., "workflows that have called maintainers in the last release cycle").
- **Dependencies**:
  - Task 1.1
- **Complexity**: 3
- **Acceptance criteria**:
  - `TROUBLESHOOTING.md` opens with a 1-2 sentence routing policy block stating subset vs. full mirror.
  - Inventory in `TROUBLESHOOTING.md` matches the policy: every listed workflow has a real
    matching `workflows/<workflow-id>/TROUBLESHOOTING.md` file on disk, and the policy's explicit pointer to
    `ALFRED_WORKFLOW_DEVELOPMENT.md` handles the rest.
  - No section name conflict with `ALFRED_WORKFLOW_DEVELOPMENT.md` (root file is routing, not standards).
- **Validation**:
  - `rg -n '^- ' TROUBLESHOOTING.md` shows the curated list.
  - Workflow runbook back-reference check returns no `MISSING:` lines:

    ```bash
    for w in $(rg -oP 'workflows/\K[a-z0-9-]+(?=/TROUBLESHOOTING\.md)' TROUBLESHOOTING.md); do
      test -f "workflows/$w/TROUBLESHOOTING.md" || echo "MISSING: $w"
    done
    ```

### Task 1.3: `Verify DEVELOPMENT / BINARY_DEPENDENCIES / AGENTS tooling references`

- **Location**:
  - `DEVELOPMENT.md`
  - `BINARY_DEPENDENCIES.md`
  - `AGENTS.md`
  - `AGENT_DOCS.toml`
- **Description**: Cross-check every script path, env var, and tool name claimed by these files against
  `scripts/`, `scripts/ci/`, `scripts/lib/`, `package.json`, `Cargo.toml`, `.github/workflows/*.yml`. Update or
  remove any reference that no longer exists. Specifically: confirm the `sccache` removal from
  `scripts/setup-rust-tooling.sh` (commit `b60c87f`) is reflected in any doc that previously implied sccache was
  installed; reconcile `setup-node-playwright.sh` trigger conditions; align CI baseline mentions with
  `.github/workflows/ci.yml`.
- **Dependencies**:
  - Task 1.2
- **Complexity**: 4
- **Acceptance criteria**:
  - Every `scripts/...sh` path mentioned in the 3 root files plus `AGENT_DOCS.toml` resolves to an existing file.
  - Every cargo tool (`cargo-nextest`, `cargo-llvm-cov`, etc.) referenced is actually installed by
    `scripts/setup-rust-tooling.sh` or noted as optional.
  - No mention of removed tooling (e.g., sccache) remains.
  - `AGENT_DOCS.toml` `[[document]]` paths exist on disk.
- **Validation**:
  - Confirm every `scripts/...sh` reference resolves on disk:

    ```bash
    for p in $(rg -oP 'scripts/[a-zA-Z0-9_./-]+\.sh' DEVELOPMENT.md BINARY_DEPENDENCIES.md AGENTS.md AGENT_DOCS.toml | sort -u); do
      test -e "$p" || echo "MISSING: $p"
    done
    ```

  - Confirm `agent-docs` reports `present=$expected missing=0` (count derived dynamically so adding a future
    required doc does not break this check):

    ```bash
    expected=$(rg -c '^\[\[document\]\]' AGENT_DOCS.toml)
    agent-docs resolve --context project-dev --strict --format checklist \
      | rg -q "present=$expected missing=0" && echo OK
    ```

### Task 1.4: `Polish root README.md workflow table and links`

- **Location**:
  - `README.md`
- **Description**: For each row in the README workflow table, validate keyword(s) match the matching
  `workflows/<workflow-id>/workflow.toml`, env var names match the corresponding crate's `src/config*.rs` or the
  workflow README, and the link target file exists. Cross-checks against `workflows/<workflow-id>/workflow.toml`
  and `crates/<crate-name>/Cargo.toml` are read-only — no edits land outside `README.md`. Add the 1 missing
  workflow row if any (for example a recently shipped workflow). Confirm Gatekeeper/quarantine standalone script
  section still references the real artifact name.
- **Dependencies**:
  - Task 1.3
- **Complexity**: 4
- **Acceptance criteria**:
  - All 21 workflows present in `workflows/` are represented in the README table (or explicitly excluded as
    template/non-shippable).
  - Every keyword shown matches the keyword(s) declared in the workflow's `workflow.toml`.
  - Every env var listed in the "Requires setup" column matches the live config parser for that crate / workflow
    helper.
  - All in-table relative links resolve.
- **Validation**:
  - `rg -n 'workflows/[a-z0-9-]+/README\\.md' README.md` lists 21 entries.
  - For each row, `awk` extract the workflow id and confirm the keyword(s) appear in the matching
    `workflows/<id>/workflow.toml` `[script_filter] keyword=` field (manual verification step recorded in PR
    body).

## Sprint 2: `docs/ Root Realignment`

**Goal**: `ARCHITECTURE.md`, `PACKAGING.md`, `RELEASE.md` correctly index every active spec, every live script,
and every supported release / publish path. This sprint sets up the spec-navigation surface that Sprint 3 will
deepen.

**Demo/Validation**:

- Command(s):
  - `bash scripts/docs-placement-audit.sh --strict`
  - `rg -n 'docs/specs/' docs/ARCHITECTURE.md docs/PACKAGING.md docs/RELEASE.md`
  - `for p in $(rg -oP 'scripts/[a-zA-Z0-9_./-]+\\.sh' docs/PACKAGING.md docs/RELEASE.md); do test -e "$p" || echo "MISSING: $p"; done`
- Verify: spec navigation section in `ARCHITECTURE.md` covers all 12 specs by category; PACKAGING / RELEASE
  commands all resolve.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 8
**CriticalPathComplexity**: 8
**MaxBatchWidth**: 1
**OverlapHotspots**: All three tasks live under `docs/`; the navigation block in `ARCHITECTURE.md` written by
T2.1 is referenced by T2.2 and T2.3 link backs, so T2.1 must land first.

### Task 2.1: `Add full spec navigation to ARCHITECTURE.md`

- **Location**:
  - `docs/ARCHITECTURE.md`
- **Description**: Add a structured navigation section listing all 12 `docs/specs/*.md` files, grouped by
  domain: CLI runtime contracts (`cli-shared-runtime-contract`, `cli-json-envelope-v1`, `cli-error-code-registry`),
  workflow / shared foundation policies (`workflow-shared-foundations-policy`, `workflow-script-refactor-contract`,
  `script-filter-input-policy`, `crate-docs-placement-policy`), CI / release contracts (`ci-refactor-contract`,
  `third-party-artifacts-contract-v1`, `third-party-license-artifact-contract-v1`), per-domain contracts
  (`google-cli-native-contract`, `steam-search-workflow-contract`). Include a one-line description per spec.
- **Dependencies**:
  - none
- **Complexity**: 3
- **Acceptance criteria**:
  - All 12 specs appear in the navigation, grouped by category, each with a one-line description.
  - Existing inline links to `cli-shared-runtime-contract.md` and `google-cli-native-contract.md` remain.
  - No spec is silently omitted.
- **Validation**:
  - `rg -oP 'docs/specs/\\K[a-z0-9-]+\\.md' docs/ARCHITECTURE.md | sort -u | wc -l` equals `12`.
  - `diff <(ls docs/specs/*.md | xargs -n1 basename | sort) <(rg -oP 'docs/specs/\\K[a-z0-9-]+\\.md' docs/ARCHITECTURE.md | sort -u)`
    is empty.

### Task 2.2: `Verify and reclassify PACKAGING.md commands`

- **Location**:
  - `docs/PACKAGING.md`
- **Description**: Walk every `bash scripts/...` and `scripts/...sh` line and confirm the script exists and the
  flag set is current. Reclassify `scripts/workflow-cli-resolver-audit.sh` from "macOS acceptance" to a
  "validation-only check" callout if it has no `--apply` mode (already verified read-only). Update the
  packaging command section to clarify when the resolver audit runs.
- **Dependencies**:
  - Task 2.1
- **Complexity**: 2
- **Acceptance criteria**:
  - Every script path resolves on disk.
  - Resolver audit reclassification is explicit (one-line note saying "read-only check, no apply mode").
  - Internal links to `RELEASE.md`, `BINARY_DEPENDENCIES.md`, `ALFRED_WORKFLOW_DEVELOPMENT.md` still resolve.
- **Validation**:
  - `for p in $(rg -oP 'scripts/[a-zA-Z0-9_./-]+\\.sh' docs/PACKAGING.md | sort -u); do test -e "$p" || echo "MISSING: $p"; done`
    returns no `MISSING:` lines.
  - `bash scripts/workflow-cli-resolver-audit.sh --help 2>&1 | grep -E '\\-\\-apply'` returns nothing
    (confirms read-only).

### Task 2.3: `Verify RELEASE.md publish flow + spec cross-refs`

- **Location**:
  - `docs/RELEASE.md`
- **Description**: Confirm `scripts/publish-crates.sh`, `release/crates-io-publish-order.txt`, and the
  third-party gate scripts mentioned all exist. Reconcile the `Sprint/Task` numbering breadcrumbs (if any) with
  `ci-refactor-contract.md`; either align labels or remove the breadcrumbs that no longer mean anything. Confirm
  release-bundle audit script flag set matches current implementation.
- **Dependencies**:
  - Task 2.2
- **Complexity**: 3
- **Acceptance criteria**:
  - All script paths in `RELEASE.md` resolve.
  - `release/crates-io-publish-order.txt` exists and is non-empty.
  - Sprint/task labels align with `ci-refactor-contract.md` numbering or are removed.
- **Validation**:
  - `test -f release/crates-io-publish-order.txt && wc -l release/crates-io-publish-order.txt`.
  - `for p in $(rg -oP 'scripts/[a-zA-Z0-9_./-]+\\.sh' docs/RELEASE.md | sort -u); do test -e "$p" || echo "MISSING: $p"; done`
    returns no `MISSING:` lines.

## Sprint 3: `docs/specs/ Deep Audit and Drift Fixes`

**Goal**: Mark every spec's lifecycle state (active / frozen / drift) and fix the drift segments inside active
specs. Resolve duplication between `third-party-artifacts-contract-v1.md` and
`third-party-license-artifact-contract-v1.md`.

**Demo/Validation**:

- Command(s):
  - `bash scripts/docs-placement-audit.sh --strict`
  - `bash scripts/ci/markdownlint-audit.sh --strict`
  - `rg -n '^# ' docs/specs/*.md`
- Verify: every spec opens with a status banner (`Status: active|frozen|superseded-by`); duplicate
  third-party spec is either merged or marked superseded with a forwarding note; cross-spec links resolve.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 14
**CriticalPathComplexity**: 14
**MaxBatchWidth**: 1
**OverlapHotspots**: T3.1 and T3.4 both touch the cli-runtime / third-party error-code areas; serial ordering
ensures error-code references stay coherent across the trio.

### Task 3.1: `Add status banners + audit drift in CLI contract trio`

- **Location**:
  - `docs/specs/cli-shared-runtime-contract.md`
  - `docs/specs/cli-json-envelope-v1.md`
  - `docs/specs/cli-error-code-registry.md`
- **Description**: Add a `> Status: active` (or `frozen`) banner under the title of each spec. Walk every
  example `error_code` and JSON envelope key against `crates/workflow-common/src/` and any `_cli` crate's
  `src/error.rs` to confirm names match. Fix any drift (rename, removed key, new key undocumented).
- **Dependencies**:
  - none
- **Complexity**: 4
- **Acceptance criteria**:
  - Each of the 3 specs has a status banner immediately after the H1.
  - Every `error_code` example in `cli-error-code-registry.md` is reachable in code (`rg -n` in
    `crates/*/src/`).
  - Cross-spec links between the trio resolve.
- **Validation**:
  - `rg -n '^> Status: ' docs/specs/cli-shared-runtime-contract.md docs/specs/cli-json-envelope-v1.md docs/specs/cli-error-code-registry.md`
    returns 3 matches.
  - Walk every `error_code` example and confirm it appears in `crates/` (no `ORPHAN:` lines, or each orphan has
    a documented historical reason):

    ```bash
    for code in $(rg -oP '"error_code"\s*:\s*"\K[a-zA-Z0-9_]+' docs/specs/cli-error-code-registry.md | sort -u); do
      rg -q "$code" crates/ || echo "ORPHAN: $code"
    done
    ```

### Task 3.2: `Audit shared-foundation / script-refactor / CI specs`

- **Location**:
  - `docs/specs/workflow-shared-foundations-policy.md`
  - `docs/specs/workflow-script-refactor-contract.md`
  - `docs/specs/ci-refactor-contract.md`
- **Description**: Add status banner; verify every helper file referenced under `scripts/lib/` exists
  (`workflow_helper_loader.sh`, `script_filter_cli_driver.sh`, `script_filter_query_policy.sh`,
  `script_filter_async_coalesce.sh`, `workflow_smoke_helpers.sh`, `workflow_cli_resolver.sh`); confirm
  CI-refactor sprint/task references map to actual `.github/workflows/*.yml` jobs; remove unreachable rollback
  notes referencing deleted infrastructure.
- **Dependencies**:
  - Task 3.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Each spec has a status banner.
  - Every `scripts/lib/*.sh` reference resolves.
  - Every `.github/workflows/*.yml` reference resolves.
  - No reference to a removed helper or removed CI job remains.
- **Validation**:
  - Walk every `scripts/lib/...sh` reference and confirm the helper exists (returns no `MISSING:` lines):

    ```bash
    for p in $(rg -oP 'scripts/lib/\K[a-zA-Z0-9_./-]+\.sh' \
        docs/specs/workflow-shared-foundations-policy.md \
        docs/specs/workflow-script-refactor-contract.md | sort -u); do
      test -f "scripts/lib/$p" || echo "MISSING: $p"
    done
    ```

  - `rg -n '\\.github/workflows/' docs/specs/ci-refactor-contract.md` results all resolve under the actual
    workflows directory.

### Task 3.3: `Audit policy specs (script-filter / placement / google / steam)`

- **Location**:
  - `docs/specs/script-filter-input-policy.md`
  - `docs/specs/crate-docs-placement-policy.md`
  - `docs/specs/google-cli-native-contract.md`
  - `docs/specs/steam-search-workflow-contract.md`
- **Description**: Add status banners. Verify `script-filter-input-policy.md` queue settings still match the
  template Alfred plist (the spec might claim TOML when actual location is plist — fix wording). Confirm
  `crate-docs-placement-policy.md` allowed-paths list still matches reality (e.g., `docs/plans/` is allowed).
  Verify `google-cli-native-contract.md` command tree matches `crates/google-cli/src/` clap definitions.
  Confirm `steam-search-workflow-contract.md` still applies (workflow exists; spec status active).
- **Dependencies**:
  - Task 3.2
- **Complexity**: 3
- **Acceptance criteria**:
  - All 4 specs have status banners.
  - `script-filter-input-policy.md` correctly identifies plist vs TOML for queue knobs.
  - `crate-docs-placement-policy.md` allowed root markdown list and `docs/` category list match the live tree.
  - `google-cli-native-contract.md` `auth|gmail|drive` subcommands match the clap tree in
    `crates/google-cli/src/`.
- **Validation**:
  - Confirm all four policy specs have status banners (4 matches):

    ```bash
    rg -n '^> Status: ' \
      docs/specs/script-filter-input-policy.md \
      docs/specs/crate-docs-placement-policy.md \
      docs/specs/google-cli-native-contract.md \
      docs/specs/steam-search-workflow-contract.md
    ```

  - `cargo run -p nils-google-cli -- --help` subcommand list matches the spec's command tree.

### Task 3.4: `Resolve third-party contract duplication`

- **Location**:
  - `docs/specs/third-party-artifacts-contract-v1.md`
  - `docs/specs/third-party-license-artifact-contract-v1.md`
  - `docs/RELEASE.md`
  - `TROUBLESHOOTING.md`
- **Description**: Decide whether to keep both specs (one for licenses, one for notices) or merge them. If
  keeping both, add explicit scope banners (`Status: active — covers <X>`) and a "see also" cross-link in each.
  If merging, mark the redundant file `Status: superseded-by <other>.md`, replace its body with a forwarding
  notice, and update all references. Update `docs/RELEASE.md` and root `TROUBLESHOOTING.md` to point at the
  surviving canonical spec. Note: if Sprint 1 already landed a routing-policy block in `TROUBLESHOOTING.md`
  (Task 1.2), this task only appends/replaces the third-party route entry — it does not rewrite the routing
  intro.
- **Dependencies**:
  - Task 3.3
- **Complexity**: 3
- **Acceptance criteria**:
  - No paragraph-level content duplication remains across the two specs.
  - Both specs have status banners (or the superseded one has only a forwarding notice).
  - Every doc that referenced either spec now points at the canonical one (no dead link).
- **Validation**:
  - `rg -n 'third-party-(artifacts|license-artifact)-contract' docs/ TROUBLESHOOTING.md README.md crates/ workflows/`
    shows references only to the surviving canonical path (or both, if both kept and rationale documented).
  - `bash scripts/generate-third-party-artifacts.sh --check` still succeeds (sanity check that nothing in the
    generator depends on the removed prose).

## Sprint 4: `Foundation Crate Doc Build-Out`

**Goal**: Fix the foundational crates whose docs are referenced by all other crates and workflow READMEs:
create the missing `workflow-readme-cli/docs/` tree, add `workflow-contract.md` for `workflow-cli` and
`google-cli`, and refresh the library-only foundation crates.

**Demo/Validation**:

- Command(s):
  - `bash scripts/docs-placement-audit.sh --strict`
  - `bash scripts/cli-standards-audit.sh`
  - `cargo run -p nils-workflow-cli -- --help`
  - `cargo run -p nils-google-cli -- --help`
- Verify: every publishable crate has both `README.md` and `docs/README.md`; new contract docs match
  `--help` output.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 16
**CriticalPathComplexity**: 16
**MaxBatchWidth**: 1
**OverlapHotspots**: All four tasks edit different crates so file-level overlap is zero. Standards-document
overlap (links into `docs/specs/*` from each new contract doc) is the integration hotspot — Sprint 3 must land
first so links are stable.

### Task 4.1: `Create workflow-readme-cli/docs/ tree`

- **Location**:
  - `crates/workflow-readme-cli/docs/README.md` (NEW)
  - `crates/workflow-readme-cli/README.md` (link update)
- **Description**: Create the missing `docs/` directory and `docs/README.md` per
  `crate-docs-placement-policy.md`. Document the crate's purpose (workflow README → packaged `info.plist`
  conversion), input / output contract, and the `convert` subcommand surface as currently implemented in
  `src/main.rs`. Cross-link from `crates/workflow-readme-cli/README.md`.
- **Dependencies**:
  - Task 3.3
- **Complexity**: 4
- **Acceptance criteria**:
  - `crates/workflow-readme-cli/docs/README.md` exists.
  - Subcommands documented match `cargo run -p nils-workflow-readme-cli -- --help`.
  - `bash scripts/docs-placement-audit.sh --strict` no longer flags `workflow-readme-cli` as missing
    `docs/README.md`.
- **Validation**:
  - `test -f crates/workflow-readme-cli/docs/README.md`.
  - `bash scripts/docs-placement-audit.sh --strict` exits 0.

### Task 4.2: `Author workflow-cli/docs/workflow-contract.md`

- **Location**:
  - `crates/workflow-cli/docs/workflow-contract.md` (NEW)
  - `crates/workflow-cli/docs/README.md` (link update)
- **Description**: Create the missing contract doc for `nils-workflow-cli`. Document the per-subcommand JSON
  envelope shape (referencing `cli-json-envelope-v1.md`), error codes (referencing
  `cli-error-code-registry.md`), and the `open-project` host-domain behavior captured in
  `crates/workflow-cli/docs/open-project-port-parity.md`. Note: `open-project` recently widened from
  github.com-only to any host (commit `a469e74`); the contract should reflect the current behavior.
- **Dependencies**:
  - Task 4.1
- **Complexity**: 4
- **Acceptance criteria**:
  - `crates/workflow-cli/docs/workflow-contract.md` exists, opens with `> Status: active` banner.
  - JSON envelope and error-code references resolve.
  - `open-project` section reflects host-agnostic behavior.
- **Validation**:
  - `test -f crates/workflow-cli/docs/workflow-contract.md`.
  - `rg -n 'cli-json-envelope-v1|cli-error-code-registry' crates/workflow-cli/docs/workflow-contract.md`
    returns matches.
  - `cargo run -p nils-workflow-cli -- open-project --help` content matches contract description.

### Task 4.3: `Author google-cli/docs/workflow-contract.md`

- **Location**:
  - `crates/google-cli/docs/workflow-contract.md` (NEW)
  - `crates/google-cli/docs/README.md` (link update)
- **Description**: Create the workflow-contract doc for `nils-google-cli` covering all three sub-namespaces
  (`auth`, `gmail`, `drive`). Document the JSON envelope per subcommand, error codes, env vars
  (`GOOGLE_CLI_CONFIG_DIR`, `GOOGLE_DRIVE_DOWNLOAD_DIR`, `GOOGLE_GS_SHOW_ALL_ACCOUNTS_UNREAD`), and account
  resolution rules. Reference `google-cli-native-contract.md` for the native module contract.
- **Dependencies**:
  - Task 4.2
- **Complexity**: 5
- **Acceptance criteria**:
  - `crates/google-cli/docs/workflow-contract.md` exists with active banner.
  - All three sub-namespaces documented; subcommands match `cargo run -p nils-google-cli -- --help` output.
  - Cross-links to `cli-json-envelope-v1.md`, `cli-error-code-registry.md`, `google-cli-native-contract.md`
    all resolve.
- **Validation**:
  - `test -f crates/google-cli/docs/workflow-contract.md`.
  - `cargo run -p nils-google-cli -- auth --help`, `... gmail --help`, `... drive --help` subcommand list
    matches the contract.
  - `bash scripts/cli-standards-audit.sh` passes for `google-cli`.

### Task 4.4: `Refresh library-foundation crate docs (alfred-core, alfred-plist, workflow-common)`

- **Location**:
  - `crates/alfred-core/README.md`
  - `crates/alfred-core/docs/README.md`
  - `crates/alfred-plist/README.md`
  - `crates/alfred-plist/docs/README.md`
  - `crates/workflow-common/README.md`
  - `crates/workflow-common/docs/README.md`
- **Description**: For each library-only crate, verify the README and `docs/README.md` describe the public
  surface (modules / functions) actually exported in `src/lib.rs`. Add a one-line note explaining why no
  `workflow-contract.md` exists (library-only — no CLI envelope to document). Note `workflow-common` recently
  gained host-agnostic git remote helpers (commit `a469e74`); doc should mention if missing.
- **Dependencies**:
  - Task 4.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Each crate's README + `docs/README.md` accurately describes the exported public API at module level.
  - The "why no workflow-contract" note is present in all three `docs/README.md` files.
  - `workflow-common` docs reflect the host-agnostic git remote helpers (post-`a469e74`).
- **Validation**:
  - `cargo doc -p nils-alfred-core -p nils-alfred-plist -p nils-workflow-common --no-deps` succeeds.
  - `rg -n 'workflow-contract|library-only' crates/alfred-core/docs/README.md crates/alfred-plist/docs/README.md crates/workflow-common/docs/README.md`
    shows the rationale note.

## Sprint 5: `CLI Crate Docs Batch 1 — Search/Media`

**Goal**: Realign docs for the search and media CLI crates against current clap definitions and shared specs.

**Demo/Validation**:

- Command(s):
  - `bash scripts/cli-standards-audit.sh`
  - `bash scripts/docs-placement-audit.sh --strict`
- Verify: each crate's README + `docs/README.md` + `docs/workflow-contract.md` cite real subcommands, real
  flags, and real error codes; envelope and error-code references match Sprint 3 canonical specs.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 15
**CriticalPathComplexity**: 15
**MaxBatchWidth**: 1
**OverlapHotspots**: No file overlap between tasks (different crate dirs). Shared spec references (envelope,
error codes) are the integration surface — Sprint 3 + 4 must precede.

### Task 5.1: `Sync brave-cli docs`

- **Location**:
  - `crates/brave-cli/README.md`
  - `crates/brave-cli/docs/README.md`
  - `crates/brave-cli/docs/workflow-contract.md`
- **Description**: Verify command tree (`search`, etc.), flags, env vars (`BRAVE_API_KEY`, `BRAVE_COUNTRY`,
  `BRAVE_SAFESEARCH`), and error codes against `src/`. Add status banner. Cross-link to canonical specs.
- **Dependencies**:
  - Task 4.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Subcommand list and flag list match `--help` output.
  - All env vars documented match the live config parser.
  - `workflow-contract.md` opens with `> Status: active` banner.
- **Validation**:
  - `cargo run -p nils-brave-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes for the crate.

### Task 5.2: `Sync wiki-cli docs`

- **Location**:
  - `crates/wiki-cli/README.md`
  - `crates/wiki-cli/docs/README.md`
  - `crates/wiki-cli/docs/workflow-contract.md`
- **Description**: Verify language code handling, multi-language env vars (`WIKI_LANGUAGE`,
  `WIKI_LANGUAGE_OPTIONS`, `WIKI_MAX_RESULTS`), and the ordered-list parsing standard reference (per
  `ALFRED_WORKFLOW_DEVELOPMENT.md` line 222). Add status banner.
- **Dependencies**:
  - Task 5.1
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Ordered-list parsing standard explicitly referenced.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-wiki-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 5.3: `Sync youtube-cli docs`

- **Location**:
  - `crates/youtube-cli/README.md`
  - `crates/youtube-cli/docs/README.md`
  - `crates/youtube-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`YOUTUBE_API_KEY`, `YOUTUBE_REGION_CODE`, `YOUTUBE_MAX_RESULTS`),
  subcommand list, and result rendering shape. Add status banner.
- **Dependencies**:
  - Task 5.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Subcommand list matches `--help`.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-youtube-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 5.4: `Sync bangumi-cli docs`

- **Location**:
  - `crates/bangumi-cli/README.md`
  - `crates/bangumi-cli/docs/README.md`
  - `crates/bangumi-cli/docs/workflow-contract.md`
  - `crates/bangumi-cli/docs/playwright-bridge-design.md`
- **Description**: Verify subcommands, env vars (`BANGUMI_API_KEY`, `BANGUMI_MAX_RESULTS`,
  `BANGUMI_API_FALLBACK`), and the playwright-bridge-design status (mark active or frozen if obsolete). Add
  status banner.
- **Dependencies**:
  - Task 5.3
- **Complexity**: 3
- **Acceptance criteria**:
  - All four crate docs reflect live behavior.
  - `playwright-bridge-design.md` has a status banner reflecting current usage.
- **Validation**:
  - `cargo run -p nils-bangumi-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 5.5: `Sync bilibili-cli docs`

- **Location**:
  - `crates/bilibili-cli/README.md`
  - `crates/bilibili-cli/docs/README.md`
  - `crates/bilibili-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`BILIBILI_UID`, `BILIBILI_MAX_RESULTS`, `BILIBILI_TIMEOUT_MS`), subcommand
  list, and search-suggestions endpoint policy. Add status banner.
- **Dependencies**:
  - Task 5.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Subcommand list matches `--help`.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-bilibili-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

## Sprint 6: `CLI Crate Docs Batch 2 — Utility/Reference`

**Goal**: Realign docs for utility / reference CLI crates (cambridge, market, weather, randomer, epoch).

**Demo/Validation**:

- Command(s):
  - `bash scripts/cli-standards-audit.sh`
  - `bash scripts/docs-placement-audit.sh --strict`
- Verify: each crate's docs match clap output and shared specs.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 15
**CriticalPathComplexity**: 15
**MaxBatchWidth**: 1
**OverlapHotspots**: No file overlap. `market-cli` carries an extra `expression-rules.md` doc — extra check
beyond the standard contract.

### Task 6.1: `Sync cambridge-cli docs`

- **Location**:
  - `crates/cambridge-cli/README.md`
  - `crates/cambridge-cli/docs/README.md`
  - `crates/cambridge-cli/docs/workflow-contract.md`
- **Description**: Verify modes (`CAMBRIDGE_DICT_MODE`, `CAMBRIDGE_MAX_RESULTS`, `CAMBRIDGE_TIMEOUT_MS`,
  `CAMBRIDGE_HEADLESS`), `cds` suggestion path behavior, and Playwright-bridge runtime self-heal flow
  (commit `6f99c6c`). Add status banner.
- **Dependencies**:
  - Task 4.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Self-heal flow described or referenced.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-cambridge-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 6.2: `Sync market-cli docs (incl. expression-rules.md)`

- **Location**:
  - `crates/market-cli/README.md`
  - `crates/market-cli/docs/README.md`
  - `crates/market-cli/docs/workflow-contract.md`
  - `crates/market-cli/docs/expression-rules.md`
- **Description**: Verify env vars (`MARKET_DEFAULT_FIAT`, `MARKET_FX_CACHE_TTL`, `MARKET_CRYPTO_CACHE_TTL`,
  `MARKET_FAVORITES_ENABLED`, `MARKET_FAVORITE_LIST`), expression-rules grammar (numeric vs asset operations),
  and FX/crypto provider lookup behavior. Add status banner to all four crate docs.
- **Dependencies**:
  - Task 6.1
- **Complexity**: 4
- **Acceptance criteria**:
  - Expression-rules grammar matches the implementation in `src/`.
  - All four crate doc files have status banners.
- **Validation**:
  - `cargo run -p nils-market-cli -- --help` output matches doc.
  - Expression-rules examples parse correctly through unit tests in `crates/market-cli/tests/`.

### Task 6.3: `Sync weather-cli docs`

- **Location**:
  - `crates/weather-cli/README.md`
  - `crates/weather-cli/docs/README.md`
  - `crates/weather-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`WEATHER_CLI_BIN`, `WEATHER_LOCALE`, `WEATHER_DEFAULT_CITIES`,
  `WEATHER_CACHE_TTL_SECS`), single-city vs multi-city flow, and forecast horizon flags. Add status banner.
- **Dependencies**:
  - Task 6.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Subcommand list and forecast modes match `--help`.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-weather-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 6.4: `Sync randomer-cli docs`

- **Location**:
  - `crates/randomer-cli/README.md`
  - `crates/randomer-cli/docs/README.md`
  - `crates/randomer-cli/docs/workflow-contract.md`
- **Description**: Verify format options, output shapes, and any rand 0.8 behavior changes (commit `ff17237`
  bumped to 0.8.6 for RUSTSEC-2026-0097). Add status banner.
- **Dependencies**:
  - Task 6.3
- **Complexity**: 2
- **Acceptance criteria**:
  - Format list matches live implementation.
  - Status banner present.
  - Any rand-API surface change post-0.8.6 reflected.
- **Validation**:
  - `cargo run -p nils-randomer-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 6.5: `Sync epoch-cli docs`

- **Location**:
  - `crates/epoch-cli/README.md`
  - `crates/epoch-cli/docs/README.md`
  - `crates/epoch-cli/docs/workflow-contract.md`
- **Description**: Verify epoch / datetime conversion behaviors, supported input formats, and copy-output
  shape. Add status banner.
- **Dependencies**:
  - Task 6.4
- **Complexity**: 3
- **Acceptance criteria**:
  - Input/output format list matches implementation.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-epoch-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

## Sprint 7: `CLI Crate Docs Batch 3 — Apps and Data`

**Goal**: Realign docs for the remaining CLI crates (timezone, spotify, steam, quote).

**Demo/Validation**:

- Command(s):
  - `bash scripts/cli-standards-audit.sh`
  - `bash scripts/docs-placement-audit.sh --strict`
- Verify: each crate's docs match clap output and shared specs.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 12
**CriticalPathComplexity**: 12
**MaxBatchWidth**: 1
**OverlapHotspots**: No file overlap.

### Task 7.1: `Sync timezone-cli docs`

- **Location**:
  - `crates/timezone-cli/README.md`
  - `crates/timezone-cli/docs/README.md`
  - `crates/timezone-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`MULTI_TZ_ZONES`, `MULTI_TZ_LOCAL_OVERRIDE`), IANA validation, and
  ordered-list parsing reference. Add status banner.
- **Dependencies**:
  - Task 4.4
- **Complexity**: 3
- **Acceptance criteria**:
  - IANA validation behavior described.
  - Ordered-list parsing standard referenced.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-timezone-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 7.2: `Sync spotify-cli docs`

- **Location**:
  - `crates/spotify-cli/README.md`
  - `crates/spotify-cli/docs/README.md`
  - `crates/spotify-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET`, `SPOTIFY_MARKET`), OAuth /
  token-cache behavior, and `open spotify:` URI handling. Add status banner.
- **Dependencies**:
  - Task 7.1
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Token-cache path documented.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-spotify-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 7.3: `Sync steam-cli docs`

- **Location**:
  - `crates/steam-cli/README.md`
  - `crates/steam-cli/docs/README.md`
  - `crates/steam-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`STEAM_REGION`, `STEAM_SHOW_REGION_OPTIONS`, `STEAM_LANGUAGE`), region
  switching rows, and cross-link to `steam-search-workflow-contract.md`. Add status banner.
- **Dependencies**:
  - Task 7.2
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Region switching behavior described.
  - Cross-link to `steam-search-workflow-contract.md` present.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-steam-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

### Task 7.4: `Sync quote-cli docs`

- **Location**:
  - `crates/quote-cli/README.md`
  - `crates/quote-cli/docs/README.md`
  - `crates/quote-cli/docs/workflow-contract.md`
- **Description**: Verify env vars (`QUOTE_DISPLAY_COUNT`, `QUOTE_REFRESH_INTERVAL`, `QUOTE_FETCH_COUNT`),
  cache directory layout, and refresh trigger mechanics. Add status banner.
- **Dependencies**:
  - Task 7.3
- **Complexity**: 3
- **Acceptance criteria**:
  - Env var list matches live config parser.
  - Cache layout described.
  - Status banner present.
- **Validation**:
  - `cargo run -p nils-quote-cli -- --help` output matches doc.
  - `bash scripts/cli-standards-audit.sh` passes.

## Sprint 8: `Workflow Standards Compliance and Cross-Check Sweep`

**Goal**: Bring all 21 workflow READMEs and TROUBLESHOOTING files into compliance with
`ALFRED_WORKFLOW_DEVELOPMENT.md` standards and cross-check workflow.toml ↔ scripts ↔ crate envelope.

**Demo/Validation**:

- Command(s):
  - `bash scripts/workflow-shared-foundation-audit.sh --check`
  - `bash scripts/workflow-sync-script-filter-policy.sh --check`
  - Compliance check (correctly handles missing sections):

    ```bash
    for d in workflows/*/; do
      f="$d/TROUBLESHOOTING.md"
      [ -f "$f" ] || continue
      missing=$(comm -23 \
        <(printf 'Quick operator checks\nCommon failures and actions\nValidation\nRollback guidance\n' | sort) \
        <(rg -oP '^## \K.+' "$f" | sort))
      [ -n "$missing" ] && printf '%s missing: %s\n' "$f" "$missing"
    done
    ```

- Verify: every non-template TROUBLESHOOTING file declares the four canonical sections; READMEs match
  workflow.toml + scripts.

**PR grouping intent**: per-sprint
**Execution Profile**: serial
**TotalComplexity**: 11
**CriticalPathComplexity**: 11
**MaxBatchWidth**: 1
**OverlapHotspots**: T8.1-T8.3 each touch a different workflow's TROUBLESHOOTING; T8.4 sweeps all 21
workflow READMEs (broadest blast radius). Serial ordering ensures standards-compliance fixes land before the
sweep so the sweep can assert compliance.

### Task 8.1: `Fix _template TROUBLESHOOTING extra section`

- **Location**:
  - `workflows/_template/TROUBLESHOOTING.md`
- **Description**: `_template` carries a `## Placeholder checklist` section beyond the standard four. Decide
  whether to (a) keep it as authoring guidance for new workflow creators (recommended — note this exception in
  `ALFRED_WORKFLOW_DEVELOPMENT.md`), or (b) move it inline as comments. If (a), update standards doc to allow
  the template-only exception. If (b), delete the section.
- **Dependencies**:
  - none
- **Complexity**: 2
- **Acceptance criteria**:
  - `_template` either follows the four-section contract exactly OR has documented exception in standards doc.
  - Choice is recorded in PR body.
- **Validation**:
  - `rg -n '^## ' workflows/_template/TROUBLESHOOTING.md` returns either exactly 4 sections or 5 with explicit
    standards-doc exception.
  - `rg -n '_template' ALFRED_WORKFLOW_DEVELOPMENT.md` reflects the chosen policy.

### Task 8.2: `Fix bilibili-search TROUBLESHOOTING extra section`

- **Location**:
  - `workflows/bilibili-search/TROUBLESHOOTING.md`
- **Description**: Remove the `## First-release support window (D0-D2)` section (workflow has shipped past
  D2; release-notes content belongs in CHANGELOG or release PR body, not the operational runbook). If any
  unique operational content lives in that section, fold it into `Common failures and actions` or
  `Rollback guidance`.
- **Dependencies**:
  - Task 8.1
- **Complexity**: 2
- **Acceptance criteria**:
  - `bilibili-search/TROUBLESHOOTING.md` has exactly the four canonical sections.
  - No operational content lost (folded into the right section if it was useful).
- **Validation**:
  - `rg -n '^## ' workflows/bilibili-search/TROUBLESHOOTING.md` returns exactly 4 lines matching the canonical
    four section names.

### Task 8.3: `Fix google-service TROUBLESHOOTING section names`

- **Location**:
  - `workflows/google-service/TROUBLESHOOTING.md`
- **Description**: Rename non-compliant section titles to canonical names: `Quick checks` →
  `Quick operator checks`, `Common failures` → `Common failures and actions`, `Validation commands` →
  `Validation`, `Rollback` → `Rollback guidance`. Keep section content; only titles change.
- **Dependencies**:
  - Task 8.2
- **Complexity**: 2
- **Acceptance criteria**:
  - All four section titles match the canonical contract.
  - Section bodies unchanged (verified via diff).
- **Validation**:
  - `rg -n '^## (Quick operator checks|Common failures and actions|Validation|Rollback guidance)$' workflows/google-service/TROUBLESHOOTING.md`
    returns 4 matches.
  - `git diff` body sections show no semantic content change.

### Task 8.4: `Sweep all 21 workflow READMEs vs workflow.toml + scripts`

- **Location**:
  - `workflows/_template/README.md`
  - `workflows/bangumi-search/README.md`
  - `workflows/bilibili-search/README.md`
  - `workflows/cambridge-dict/README.md`
  - `workflows/codex-cli/README.md`
  - `workflows/epoch-converter/README.md`
  - `workflows/google-search/README.md`
  - `workflows/google-service/README.md`
  - `workflows/imdb-search/README.md`
  - `workflows/market-expression/README.md`
  - `workflows/multi-timezone/README.md`
  - `workflows/netflix-search/README.md`
  - `workflows/open-project/README.md`
  - `workflows/quote-feed/README.md`
  - `workflows/randomer/README.md`
  - `workflows/spotify-search/README.md`
  - `workflows/steam-search/README.md`
  - `workflows/weather/README.md`
  - `workflows/wiki-search/README.md`
  - `workflows/youtube-search/README.md`
- **Description**: For each workflow, verify (a) keyword(s) match the matching workflow's `workflow.toml`,
  (b) env vars match the workflow's `scripts/` adapters AND the matching crate config parser, (c) referenced
  binaries / scripts / helpers actually exist. Cross-checks against `workflow.toml` and `scripts/` are
  read-only — only the listed README files are edited. Each README change is restricted to in-place value
  refresh (keyword, env var name, link target, removed-tool reference). Narrative rewrites are out of scope;
  if a README needs structural rework, file a follow-up issue and skip it here. Record drift findings in the
  PR body grouped by workflow.
- **Dependencies**:
  - Task 8.3
- **Complexity**: 5
- **Acceptance criteria**:
  - All 21 workflow READMEs scanned.
  - Drift either fixed inline (value-level only) or filed as follow-up issue (recorded in PR body).
  - No README references a removed script / helper / binary.
  - PR body includes a one-line-per-workflow status (`fixed-inline | follow-up-issue#NNN | clean`).
  - Per-README diff size stays small (no narrative rewrites); reviewer can scan each workflow's diff in
    under one minute.
- **Validation**:
  - Walk every workflow README's `scripts/...sh` references and confirm each script exists (returns no
    `MISSING:` lines):

    ```bash
    for w in workflows/*/; do
      for s in $(rg -oP 'scripts/[a-zA-Z0-9_./-]+\.sh' "$w/README.md" 2>/dev/null | sort -u); do
        test -e "$s" || echo "MISSING: $w -> $s"
      done
    done
    ```

  - `bash scripts/workflow-shared-foundation-audit.sh --check` passes.
  - `bash scripts/workflow-sync-script-filter-policy.sh --check` passes.

## Testing Strategy

- Unit: each task that touches a crate doc verifies via `cargo run -p <crate> -- --help` (or `cargo doc` for
  library-only crates). No new unit tests needed.
- Integration: Sprint validation gates rely on `scripts/cli-standards-audit.sh`,
  `scripts/docs-placement-audit.sh --strict`, `scripts/workflow-shared-foundation-audit.sh --check`,
  `scripts/workflow-sync-script-filter-policy.sh --check`, `scripts/ci/markdownlint-audit.sh --strict`.
- E2E/manual: `agent-docs resolve --context startup --strict --format checklist` and
  `agent-docs resolve --context project-dev --strict --format checklist` run after each sprint to confirm the
  preflight contract is intact.

## Risks & gotchas

- **Spec drift cascade**: editing a canonical spec in Sprint 3 ripples to many crate / workflow docs in
  Sprints 4-8. Mitigation: Sprint 3 lands first; if a Sprint 3 fix is deferred, hold the dependent crate doc
  task rather than copy-pasting interim wording.
- **Generated artifact churn**: cleaning up `THIRD_PARTY_*.md` references can collide with dependabot PRs that
  regenerate those artifacts. Mitigation: do not edit the generated files; only adjust prose around them.
- **`_template` policy choice (T8.1)**: keeping the placeholder section requires a standards-doc exception
  edit; removing it changes the new-workflow scaffold flow. Pick (a) keep + document, unless team prefers
  uniform 4-section template.
- **Inventory list churn**: if a new workflow is added between Sprint 1 and Sprint 8, the inventory list
  edited in T1.1 will go stale. Mitigation: add a one-line "ground truth" comment pointing maintainers at
  `ls workflows/` to enumerate.
- **clap subcommand drift between draft and merge**: `--help` output is the source of truth for crate docs;
  if a PR lands changing clap during the doc PR review window, rerun validation before merge.
- **Cross-spec link breakage**: if a status banner or path fix renames an anchor, all back-references must be
  updated. Mitigation: per-sprint validation runs `rg -n '<old-ref>'` to ensure no stale references remain.
- **Markdown lint surprise**: `rumdl` (replaces older `markdownlint-cli2`) may flag style differences; run
  `bash scripts/ci/markdownlint-audit.sh --strict` per sprint and fix style issues alongside content.
- **Volatile workflow.toml fields (`version`)**: do not edit version stamps as part of doc cleanup.
- **Validation tool availability**: several Validation lines depend on `agent-docs`, `rg`, `cargo`,
  `plan-tooling`, `rumdl`, and `comm` being on `PATH`. If running on a fresh checkout, run
  `scripts/setup-rust-tooling.sh` and `npm ci` first; otherwise validation will false-fail. List included
  in `BINARY_DEPENDENCIES.md`.

## Rollback plan

- Each sprint = one PR. To roll back, revert the merge commit; no shared-state migrations or stamps to undo.
- **Reverting Sprint 3 (specs)** after Sprint 4-8 land:
  1. Hold any in-flight downstream PRs before the revert lands.
  2. Revert the Sprint 3 merge commit.
  3. Run `bash scripts/docs-placement-audit.sh --strict` and `rg -n '> Status: ' docs/specs/` to surface
     dangling banner references.
  4. For each downstream sprint already merged (4-8), open a hotfix doc-PR that drops or reroutes the
     stale spec references introduced by that sprint; do not blanket-revert downstream sprints.
  5. Re-land Sprint 3 with corrected content; downstream hotfix PRs can then be reverted to restore the
     original cross-links.
- **Reverting Sprint 1 (inventory + routing)** after Sprint 8 lands:
  1. Revert Sprint 1.
  2. Re-check `ALFRED_WORKFLOW_DEVELOPMENT.md` workflow inventory: the 5 missing entries reappear; Sprint 8
     cross-checks may fail.
  3. Re-land Sprint 1 with the original 5-entry addition; Sprint 8 should auto-recover.
- **Validation rollback**: `agent-docs resolve --context project-dev --strict --format checklist` verifies
  the preflight set after any revert; if it fails, re-add the missing doc references in a hotfix doc-PR
  before merging anything else.

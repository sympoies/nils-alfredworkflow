# Development Guide

## Platform scope

- Alfred runtime checks (workflow install, keyword execution, Gatekeeper/quarantine fixes) are macOS-only.
- Development and CI quality gates (`lint`, `test`, `pack`) are expected to run on Linux as well.
- CI baseline uses Ubuntu (`.github/workflows/ci.yml`), and tooling bootstrap supports Debian/Ubuntu
  (`scripts/setup-rust-tooling.sh`).

## Setup

- If Rust/cargo (or required cargo tools) are not installed yet, run:
  - `scripts/setup-rust-tooling.sh`
- For workflows that use Node + Playwright tooling, run:
  - `scripts/setup-node-playwright.sh`
  - Add `--install-browser` only when you need live Playwright scraping checks.
- Manual setup fallback:
  - Install Rust via rustup (stable toolchain).
  - Ensure `rustfmt` and `clippy` components are installed:
    - `rustup component add rustfmt clippy`
  - Install Node.js (>=20) and run:
    - `npm ci`

## Build and run

- Build workspace: `cargo build`
- Run shared workflow CLI: `cargo run -p nils-workflow-cli -- --help`
- List workflows: `scripts/workflow-pack.sh --list`

## Formatting and linting

- Format check: `cargo fmt --all -- --check`
- Format fix: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- CLI standards audit: `scripts/cli-standards-audit.sh`
- Markdown lint audit: `bash scripts/ci/markdownlint-audit.sh --strict`
- Full lint entrypoint (includes `cli-standards-audit`, `docs-placement-audit`, and `markdownlint-audit`):
  `scripts/workflow-lint.sh`
- Shared foundation audit (also included in full lint entrypoint):
  `bash scripts/workflow-shared-foundation-audit.sh --check`
- Script Filter policy check (queue + shared foundation wiring):
  `bash scripts/workflow-sync-script-filter-policy.sh --check`

### CLI standards audit

- Hard-fail checks (must pass in CI): required standards docs, crate README presence, crate `description` metadata, and
  standards gate wiring.
- Warning checks (migration tracking): explicit json-mode indicators, envelope key assertions, and README standards
  sections.
- To enforce warnings as failures: `scripts/cli-standards-audit.sh --strict`

### Documentation placement

- Canonical policy: `docs/specs/crate-docs-placement-policy.md`
- Architecture/runtime ownership boundaries: `docs/ARCHITECTURE.md`
- Required placement gate before commit: `bash scripts/docs-placement-audit.sh --strict`
- Placement rule: crate-owned docs belong in `crates/<crate-name>/docs/`; workspace-level docs belong in allowed root
  `docs/` categories.

#### Contributor checklist (required before commit)

- [ ] For every new publishable crate, required docs exist: `crates/<crate-name>/README.md` and
      `crates/<crate-name>/docs/README.md`.
- [ ] For every new markdown file, ownership/path classification is complete (`workspace-level` vs `crate-specific`) and
      the file path follows the policy.
- [ ] Documentation placement audit passes: `bash scripts/docs-placement-audit.sh --strict`.

## Testing

### Required before committing

- Default local pre-commit entrypoint (recommended): `scripts/local-pre-commit.sh`
  - Runs `scripts/workflow-lint.sh`, `scripts/workflow-sync-script-filter-policy.sh --check`,
    `npm run test:cambridge-scraper`, and `scripts/workflow-test.sh --skip-third-party-audit`.
- CI-parity local sequence (same gate order as `.github/workflows/ci.yml`):
  - `scripts/local-pre-commit.sh --mode ci`
- Add package smoke gate when you want release-style package validation locally:
  - `scripts/local-pre-commit.sh --with-package-smoke`
- Why this replaces the old manual sequence:
  - `scripts/workflow-test.sh` already runs strict `third-party-artifacts-audit`, `cargo test --workspace`, and
    script-level shell tests (`scripts/script-tests.sh`) by default, so manually prepending those checks caused
    redundant runs.
- Run script-level shell tests directly (without full workflow-test flow):
  - `bash scripts/script-tests.sh`
- For workflow-specific or CLI-specific checks (for example live smoke or probe scripts), run the validation steps
  documented in the corresponding `workflows/<workflow-id>/README.md`.

### Local iteration shortcuts (optional)

- Smoke only for one workflow:
  - `scripts/workflow-test.sh --id <workflow-id> --skip-third-party-audit --skip-workspace-tests`
- Skip script-level shell tests temporarily during focused Rust/workflow iteration:
  - `scripts/workflow-test.sh --skip-script-tests`
  - Do not use this mode as a final pre-commit check.
- Skip Node scraper tests temporarily during non-scraper iteration:
  - `scripts/local-pre-commit.sh --skip-node-scraper-tests`
  - Do not use this mode as a final pre-commit check.

### Third-party artifacts generator workflow

- One-command regeneration:
  - `bash scripts/generate-third-party-artifacts.sh --write`
- Freshness check (fails when either artifact drifts from source metadata):
  - `bash scripts/generate-third-party-artifacts.sh --check`
- Regression tests for generator behavior:
  - `bash tests/third-party-artifacts/generator.test.sh`
- Expected failure modes:
  - `--check` exits non-zero with `FAIL [check] ... is stale` and a remediation command.
  - Missing required input files (for example `Cargo.lock`, `package-lock.json`, or
    `scripts/lib/codex_cli_version.sh`) exit non-zero with `required input missing: <path>`.

### Alfred Script Filter guardrail

- For workflows where Script Filter output is already fully controlled by our CLI/script JSON, keep
  `alfredfiltersresults=false` in `info.plist.template`.
- Do not set `alfredfiltersresults=true` unless you explicitly need Alfred-side secondary filtering.
- Reason: `alfredfiltersresults=true` can hide valid workflow items when Alfred query propagation falls back to
  null/empty argument paths, making the workflow appear broken even though script output is correct.
- Validation checklist for any workflow plist change:
  - `scripts/workflow-pack.sh --id <workflow-id>`
  - `plutil -convert json -o - build/workflows/<workflow-id>/pkg/info.plist \`
    `| jq -e '(.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.alfredfiltersresults) == false'`

### CI-style test reporting (optional)

- If `cargo nextest` is missing, run `scripts/setup-rust-tooling.sh`
- Run CI-style tests + generate JUnit:
  - `cargo nextest run --profile ci --workspace`

### Workflow-specific optional manual checks

- Workflow/CLI-specific optional checks (for example live endpoint smoke tests and probe scripts) are maintained in each
  workflow README.
- Reference workflow docs under `workflows/<workflow-id>/README.md`.

## Coverage (optional)

- Install tools:

  ```bash
  scripts/setup-rust-tooling.sh
  ```

- Generate coverage artifacts:

  ```bash
  mkdir -p target/coverage
  cargo llvm-cov nextest --profile ci --workspace --lcov --output-path target/coverage/lcov.info
  cargo llvm-cov report --html --output-dir target/coverage
  ```

## Packaging

- Pack one workflow:
  - `scripts/workflow-pack.sh --id <workflow-id>`
- Pack and install:
  - `scripts/workflow-pack.sh --id <workflow-id> --install --mode ui`
- Pack and background-install one already-installed workflow:
  - `scripts/workflow-pack.sh --id <workflow-id> --install --mode background`
  - Preserves `prefs.plist` plus installed hotkey and keyword customizations by default.
  - Add `--no-preserve-customizations` to reset hotkeys and keywords to packaged defaults during background install.
- Install latest already-built artifact only (skip rebuild):
  - `scripts/workflow-pack.sh --id <workflow-id> --install-only`
- Pack all workflows and background-install already-installed ones:
  - `scripts/workflow-pack.sh --all --install`
  - Add `--no-preserve-customizations` to reset installed hotkeys and keywords for every updated workflow.
- Pack all workflows:
  - `scripts/workflow-pack.sh --all`

### Crates.io runtime packaging policy

- When a workflow bundles a runtime binary published on crates.io, packaging scripts must follow this order:
  1. Prefer explicit local override (for example `*_PACK_BIN`).
  2. Then use local PATH binary.
  3. If binary is missing or not the pinned version, auto-install the pinned crate version from crates.io via
     `cargo install --locked --root <cache-root>` and bundle that installed binary.
- This policy avoids accidental version drift while keeping packaging reproducible across machines.

### External crate exact-pin policy

- Third-party crates used by workspace crates must be exact-pinned (for example `foo = "=1.2.3"`), not loose semver
  ranges.
- Add or update external crates with exact version syntax:
  - `cargo add <crate>@=<version>`
- For reproducibility, commit both `Cargo.toml` and `Cargo.lock` updates together after the pin change.

## Rust crate publishing (crates.io)

- Dry-run publish checks (all crates from `release/crates-io-publish-order.txt`):
  - `scripts/publish-crates.sh --dry-run`
- Dry-run publish checks (single crate):
  - `scripts/publish-crates.sh --dry-run --crates "<crate-name>"`
- Publish all crates in dependency order:
  - `CARGO_REGISTRY_TOKEN=... scripts/publish-crates.sh --publish`
- Publish a subset:
  - `scripts/publish-crates.sh --publish --crates "nils-alfred-core nils-workflow-common"`

## macOS acceptance (Gatekeeper / quarantine)

- For workflows that bundle executables, include a quarantine check during final acceptance on macOS.
- If Gatekeeper blocks execution, start with `ALFRED_WORKFLOW_DEVELOPMENT.md` and then follow the matching
  workflow-local troubleshooting file (`workflows/<workflow-id>/TROUBLESHOOTING.md`) and README acceptance steps.

### Gatekeeper startup auto-clear policy

- Common helper owner: `scripts/lib/workflow_cli_resolver.sh`.
- Bundled runtime entrypoints must:
  - source `workflow_cli_resolver.sh`
  - resolve package/release/debug candidates via `wfcr_resolve_binary`
- On macOS, workflow startup should try package-level quarantine cleanup once (per installed workflow directory) before
  resolving runtime binaries:
  - `wfcr_clear_workflow_quarantine_once_if_needed`
  - marker path: `${TMPDIR:-/tmp}/nils-workflow-quarantine-markers/<fingerprint>.marker`
- Candidate-level cleanup remains enabled for resolved runtime binaries:
  - `wfcr_clear_quarantine_if_needed`
- Standalone script remains fallback-only for locked-down environments:
  - `scripts/workflow-clear-quarantine-standalone.sh`

### Workflow inventory requiring Gatekeeper helper

Required (bundled runtime; must use helper-based startup cleanup):

- `bangumi-search`
- `bilibili-search`
- `cambridge-dict`
- `codex-cli` (custom resolver in `workflows/codex-cli/scripts/action_open.sh`, explicit helper call)
- `epoch-converter`
- `google-search`
- `market-expression`
- `memo-add`
- `multi-timezone`
- `netflix-search`
- `open-project`
- `quote-feed`
- `randomer`
- `spotify-search`
- `weather`
- `wiki-search`
- `youtube-search`

Not required (no bundled runtime binary in `workflow.toml`):

- `imdb-search`

Quick audit commands:

```bash
# Required policy gate (also runs in scripts/workflow-lint.sh and CI).
bash scripts/workflow-cli-resolver-audit.sh --check

# Workflows that reference helper-based binary resolution.
rg -n "wfcr_resolve_binary|wfcr_clear_workflow_quarantine_once_if_needed" \
  workflows/*/scripts/*.sh workflows/*/scripts/*/*.sh

# workflow.toml inventory (id + rust_binary)
for manifest in workflows/*/workflow.toml; do
  wf="$(basename "$(dirname "$manifest")")"
  rb="$(awk -F'=' '/^[[:space:]]*rust_binary[[:space:]]*=/{gsub(/^[[:space:]]*"|"[[:space:]]*$/, "", $2); gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2); print $2; exit}' "$manifest")"
  printf '%s\trust_binary=%s\n' "$wf" "${rb:-<none>}"
done | sort
```

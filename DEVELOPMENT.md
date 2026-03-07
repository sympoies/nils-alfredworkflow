# Development Guide

Use this file for day-to-day development, quality gates, and local validation flow.

- Toolchain/bootstrap prerequisites: `BINARY_DEPENDENCIES.md`
- Packaging/install/macOS acceptance: `docs/PACKAGING.md`
- Release and publish flow: `docs/RELEASE.md`
- Workflow runtime and troubleshooting standards: `ALFRED_WORKFLOW_DEVELOPMENT.md`

## Platform scope

- Alfred runtime checks and install acceptance are macOS-only.
- Development and CI quality gates are expected to run on Linux as well.
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
- For the full local tool list and install commands, use `BINARY_DEPENDENCIES.md`.

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
- For Script Filter/runtime behavior standards, use `ALFRED_WORKFLOW_DEVELOPMENT.md`.

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

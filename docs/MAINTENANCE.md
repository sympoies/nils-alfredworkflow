# Maintenance Reference

Workspace-level command reference for contributors maintaining
`nils-alfredworkflow`. Start with the principles and routine gate in
[`DEVELOPMENT.md`](../DEVELOPMENT.md); use this document when a change needs
specific setup, build, lint, test, or coverage commands.

- Toolchain/bootstrap prerequisites:
  [`BINARY_DEPENDENCIES.md`](../BINARY_DEPENDENCIES.md)
- Packaging/install/macOS acceptance: [`PACKAGING.md`](PACKAGING.md)
- Release and publish flow: [`RELEASE.md`](RELEASE.md)
- Workflow runtime and troubleshooting standards:
  [`ALFRED_WORKFLOW_DEVELOPMENT.md`](../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Platform scope

- Alfred runtime checks and install acceptance are macOS-only.
- Development and CI quality gates are expected to run on Linux as well.
- CI baseline uses Ubuntu (`.github/workflows/ci.yml`), and tooling bootstrap
  supports Debian/Ubuntu (`scripts/setup-rust-tooling.sh`).

## Setup

- If Rust/cargo or required cargo tools are not installed, run
  `scripts/setup-rust-tooling.sh`.
- For Python helper isolation, `.envrc` creates a repository `.venv` with
  `uv venv` when direnv loads; run `direnv allow`.
- For one-off isolated Python helper runs, use
  `uv run --python python3 --isolated python <script-or-module>`.
- For workflows that use Node and Playwright tooling, run
  `scripts/setup-node-playwright.sh`; add `--install-browser` only for live
  Playwright scraping checks.
- Manual fallback: install stable Rust, run
  `rustup component add rustfmt clippy`, install Node.js 24 or newer
  (`fnm use` reads `.node-version`), and run `npm ci`.
- For the complete local tool list, use
  [`BINARY_DEPENDENCIES.md`](../BINARY_DEPENDENCIES.md).

## Build and run

- Build workspace: `cargo build`
- Run shared workflow CLI: `cargo run -p nils-workflow-cli -- --help`
- List workflows: `scripts/workflow-pack.sh --list`

## Formatting and linting

- Format check: `cargo fmt --all -- --check`
- Format fix: `cargo fmt --all`
- Lint:
  `cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used`
- CLI standards audit: `scripts/cli-standards-audit.sh`
- Markdown lint audit: `bash scripts/ci/markdownlint-audit.sh --strict`
- Full lint entrypoint: `scripts/workflow-lint.sh`
- Shared foundation audit:
  `bash scripts/workflow-shared-foundation-audit.sh --check`
- Script Filter policy check:
  `bash scripts/workflow-sync-script-filter-policy.sh --check`

`clippy::unwrap_used` and `clippy::expect_used` remain warn-only at workspace
level while the CI summary tracks their per-target count. A crate may switch
them to `deny` after production paths route failures through `?` and a
`NILS_<DOMAIN>_NNN` code from
[`cli-error-code-registry.md`](specs/cli-error-code-registry.md).

### CLI standards audit

- Hard failures cover required standards docs, crate README presence, crate
  description metadata, and standards-gate wiring.
- Warnings track explicit JSON-mode indicators, envelope assertions, and README
  standards sections.
- Use `scripts/cli-standards-audit.sh --strict` to enforce warnings.

### Documentation placement

- Canonical policy:
  [`crate-docs-placement-policy.md`](specs/crate-docs-placement-policy.md)
- Architecture and runtime ownership: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Required placement gate: `bash scripts/docs-placement-audit.sh --strict`
- Crate-owned docs belong in `crates/<crate-name>/docs/`; workspace-level docs
  belong in the governed root or `docs/` categories.

### Development log

- Canonical contract: [`devlog/README.md`](devlog/README.md)
- Month files live at `docs/devlog/YYYY-MM.md`, newest entry first.
- Search past entries: `bash scripts/devlog-search.sh <term> [YYYY-MM]`
- Add an entry when work produces a durable outcome worth future lookup; keep
  the canonical contract, policy, or runbook document current first.

Before committing a documentation change, confirm that every publishable crate
has `crates/<crate-name>/README.md` and
`crates/<crate-name>/docs/README.md`, every new Markdown file has an explicit
owner and valid path, and the strict placement audit passes.

## Testing

### Required before committing

The default local gate is:

```bash
scripts/local-pre-commit.sh
```

It verifies Node.js, installs the locked npm tree, and runs the owned validation
phases. Its early
`scripts/ci/third-party-artifacts-change-gate.sh` selects strict freshness
checks when relevant inputs change. The remaining phases run
`scripts/workflow-lint.sh --skip-third-party-audit`,
`scripts/workflow-sync-script-filter-policy.sh --check`,
`npm run test:cambridge-scraper`, and
`scripts/workflow-test.sh --skip-third-party-audit`; the workflow test owns
workspace Rust tests and script-level shell tests.

- CI-parity order: `scripts/local-pre-commit.sh --mode ci`
- Add release-style package smoke:
  `scripts/local-pre-commit.sh --with-package-smoke`
- Run shell tests directly: `bash scripts/script-tests.sh`
- Workflow-specific checks live in
  `workflows/<workflow-id>/README.md`.

Do not prepend the old manual test sequence to the default gate:
`scripts/workflow-test.sh` already runs strict third-party artifact auditing,
`cargo test --workspace`, and the shell tests.

### Local iteration shortcuts

- One workflow smoke:
  `scripts/workflow-test.sh --id <workflow-id> --skip-third-party-audit --skip-workspace-tests`
- Temporarily skip shell tests:
  `scripts/workflow-test.sh --skip-script-tests`
- Temporarily skip Node scraper tests:
  `scripts/local-pre-commit.sh --skip-node-scraper-tests`

The skip modes are iteration aids, not final validation.

### Third-party artifact generation

- Regenerate: `bash scripts/generate-third-party-artifacts.sh --write`
- Check freshness: `bash scripts/generate-third-party-artifacts.sh --check`
- Regression tests:
  `bash tests/third-party-artifacts/generator.test.sh`

The check exits non-zero with `FAIL [check] ... is stale` and a remediation
command when an artifact drifts. Missing required inputs such as `Cargo.lock`,
`package-lock.json`, or `scripts/lib/codex_cli_version.sh` fail with
`required input missing: <path>`.

### Dependabot bumps

Every Dependabot cargo bump rewrites `Cargo.lock`, which drifts both artifacts
and fails the strict third-party audit inside `validate`. Two workflows handle
that automatically, split so the privileged half never touches pull-request
content:

- `.github/workflows/dependabot-third-party-artifacts.yml` runs unprivileged on
  `pull_request` — no secrets, read-only token. It is the only place that
  executes anything from the bump, and it publishes the regenerated files as
  the `third-party-refresh` artifact. Its refresh step carries
  `GITHUB_TOKEN: ${{ github.token }}` because the generator reads bundled
  runtime release metadata from the GitHub Releases API;
  `scripts/ci/ci-workflow-audit.sh` enforces that.
- `.github/workflows/dependabot-third-party-apply.yml` runs privileged on
  `workflow_run`. It checks out nothing, commits the two artifact paths onto the
  Dependabot branch through the Git Data API, and squash merges
  `dependabot/cargo/cargo-minor-patch-*` once **both** blocking workflows, `CI`
  and `cargo-deny`, have a completed successful run for that exact commit.
  Either completion wakes the job and it re-checks both, so whichever finishes
  last performs the merge. Major-version and single-crate bumps are refreshed
  but never auto-merged.

The artifact is untrusted input, so the applying workflow never executes it: it
writes only the two known paths, only onto the branch the run belongs to, and
only while that branch head still matches the commit the refresh was generated
for. CI then re-runs the strict audit on the resulting commit, and the merge
gate requires that run to pass.

The applying workflow requires two repository secrets, `BOT_APP_ID` and
`BOT_APP_PRIVATE_KEY`, for a GitHub App installed on this repository with
`Contents: read and write`, `Pull requests: read and write`, and
`Actions: read`. Both are configured here for the `sympoies-bot` App
(id `4665910`), which is installed org-wide and already carries exactly those
permissions. Note that the `sympoies-reviewer` App used to publish review
outcomes is a different identity and cannot stand in: it holds only
`Pull requests: write` and `Metadata: read`, so it fails at the commit step.
The App token is mandatory rather than a convenience: a commit
made with the default `GITHUB_TOKEN` does not start a new workflow run, so the
refreshed commit would carry no CI results and could never be shown green.
Without the secrets the applying workflow fails fast with that instruction, and
refreshing the bump by hand
(`bash scripts/generate-third-party-artifacts.sh --write`) stays the fallback.

Dependabot stops maintaining a pull request once anything else pushes to its
branch, so a bump that has received a refresh commit is no longer rebased
automatically. If `main` later moves `Cargo.lock` and the bump re-drifts,
recover it by commenting `@dependabot rebase` on the pull request, or close it
and let Dependabot recreate it.

### CI-style test reporting

If `cargo nextest` is missing, run `scripts/setup-rust-tooling.sh`. Then run:

```bash
cargo nextest run --profile ci --workspace
```

Workflow-specific live smoke and probe commands remain optional and are owned
by each workflow README.

## Coverage

Install the coverage tools with `scripts/setup-rust-tooling.sh`, then run:

```bash
mkdir -p target/coverage
cargo llvm-cov nextest --profile ci --workspace --lcov --output-path target/coverage/lcov.info
cargo llvm-cov report --html --output-dir target/coverage
```

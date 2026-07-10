# CI Refactor Contract (Sprint 1 Tasks 1.1-1.2)

> Status: frozen — post-Sprint-1 contract; canonical entrypoints are not bypassed without an explicit
> successor spec.

This document freezes the post-refactor CI routing contract for:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/publish-crates.yml`

All bootstrap and gate routing in those workflows must use the shared scripts in `scripts/ci/`.

## canonical entrypoint set

| Layer | canonical entrypoint | Purpose |
| --- | --- | --- |
| Bootstrap | `scripts/ci/ci-bootstrap.sh` | Shared bootstrap wrapper for CI/release/publish runtime setup. |
| Gates | `scripts/ci/ci-run-gates.sh` | Shared gate invoker for lint/test/package/release/publish command routing. |

## workflow step mapping

### `.github/workflows/ci.yml` (`validate` job)

| Workflow step | Route type | canonical entrypoint | Decision |
| --- | --- | --- | --- |
| Checkout | GitHub Action | `actions/checkout@v6` | Keep |
| Set up Node.js | GitHub Action | `actions/setup-node@v6` | Keep |
| Install Node dependencies | Inline shell | `npm ci` | Keep |
| Set up Rust | GitHub Action | `dtolnay/rust-toolchain@stable` | Keep |
| Cache cargo | GitHub Action | `Swatinem/rust-cache@v2` | Keep |
| Install cargo-nextest | GitHub Action | `taiki-e/install-action@v2` | Keep |
| Bootstrap CI runtime | Script | `bash scripts/ci/ci-bootstrap.sh --context ci --install-codex-cli` | Canonical |
| Lint | Script | `bash scripts/ci/ci-run-gates.sh lint` | Canonical |
| Third-party artifacts audit (strict) | Script | `bash scripts/ci/ci-run-gates.sh third-party-artifacts-audit` | Canonical |
| Node scraper tests | Script | `bash scripts/ci/ci-run-gates.sh node-scraper-tests` | Canonical |
| Shell script tests | Script | `bash scripts/ci/ci-run-gates.sh script-tests` | Canonical |
| Test | Script | `bash scripts/ci/ci-run-gates.sh test` | Canonical |
| Package smoke | Script | `bash scripts/ci/ci-run-gates.sh package-smoke --skip-arch-check` | Canonical |

### `.github/workflows/release.yml` (`package` job)

| Workflow step | Route type | canonical entrypoint | Decision |
| --- | --- | --- | --- |
| Checkout | GitHub Action | `actions/checkout@v6` | Keep |
| Set up Rust | GitHub Action | `dtolnay/rust-toolchain@stable` | Keep |
| Cache cargo | GitHub Action | `Swatinem/rust-cache@v2` | Keep |
| Bootstrap release runtime | Script | `bash scripts/ci/ci-bootstrap.sh --context release --install-codex-cli` | Canonical |
| Run release package gates | Script | `bash scripts/ci/ci-run-gates.sh release-package --tag "$GITHUB_REF_NAME"` | Canonical |
| Upload release assets | GitHub Action | `softprops/action-gh-release@v2` | Keep |

### `.github/workflows/publish-crates.yml` (`publish` job)

| Workflow step | Route type | canonical entrypoint | Decision |
| --- | --- | --- | --- |
| Checkout | GitHub Action | `actions/checkout@v6` | Keep |
| Set up Rust | GitHub Action | `dtolnay/rust-toolchain@stable` | Keep |
| Cache cargo | GitHub Action | `Swatinem/rust-cache@v2` | Keep |
| Bootstrap publish runtime | Script | `bash scripts/ci/ci-bootstrap.sh --context publish-crates` | Canonical |
| Publish selected crates | Script | `bash scripts/ci/ci-run-gates.sh publish-crates --mode "$PUBLISH_MODE" --crates "$PUBLISH_CRATES" --registry "$PUBLISH_REGISTRY"` | Canonical |

## deletion matrix

| Legacy or duplicate path | Replacement canonical entrypoint | Status | Rationale |
| --- | --- | --- | --- |
| `ci.yml` inline codex-cli install block | `ci-bootstrap.sh --context ci --install-codex-cli` | Deleted in Task 1.2 | CI/release install the host-compatible nils-cli release asset against repository-pinned SHA-256 metadata. |
| `release.yml` inline codex-cli install block | `ci-bootstrap.sh --context release --install-codex-cli` | Deleted in Task 1.2 | Removes duplicate pin/version wiring. |
| `publish-crates.yml` inline shell for mode/token/arg assembly | `ci-run-gates.sh publish-crates ...` | Deleted in Task 1.2 | Reuses one gate invoker instead of workflow-local branching logic. |
| `release.yml` multi-step inline shell gate orchestration (artifact regenerate/check, pack, bundle, audit) | `ci-run-gates.sh release-package --tag ...` | Deleted in Task 1.2 | Keeps release gate order in one script entrypoint. |
| Any workflow-local fallback branch that bypasses `ci-bootstrap.sh`/`ci-run-gates.sh` for the same gate | None (forbidden) | Marked for rejection in future drift audit (Task 1.4) | Prevents reintroducing duplicated pre-refactor gate paths. |

## no legacy compatibility policy

- CI routing is strict: no legacy compatibility fallback is allowed for gate execution once a gate has a canonical entrypoint.
- New or modified workflow gate steps must call `ci-bootstrap.sh` and/or `ci-run-gates.sh` instead of adding new inline shell branches.
- If a prior inline branch is still required temporarily, it must be called out in this contract with explicit removal ownership.

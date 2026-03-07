# Binary Dependencies

This document lists required local tools for development, linting, testing, and packaging.

## Platform intent

- Alfred runtime/interactive acceptance is macOS-only.
- Linux dependencies are intentionally kept for CI and headless test/lint/package flows.
- This repository's CI runs on Ubuntu (`.github/workflows/ci.yml`), and `scripts/setup-rust-tooling.sh` includes
  Debian/Ubuntu handling.

## Required tools

- Rust toolchain (`rustup`, `cargo`, `rustc`)
- Rust components: `rustfmt`, `clippy`, `llvm-tools-preview`
- Cargo tools: `cargo-nextest`, `cargo-llvm-cov`
- Core CLI/runtime: `git`, `jq`, `rg` (ripgrep), `curl`
- Shell tooling: `shellcheck`, `shfmt`
- Node runtime: `node`, `npm`
- Node dependency: `playwright` package (managed via root `package.json`)
- SHA-256 provider (at least one): `shasum` or `sha256sum` or `openssl`
- Packaging/runtime helpers: `zip`, `unzip`, `open` (macOS install/runtime), `xdg-open` (Linux CI/local smoke
  compatibility)
- Optional live scraper runtime: Playwright Chromium browser (`npx playwright install chromium`)

## Third-party artifacts generator prerequisites

- Regenerate artifacts:
  - `bash scripts/generate-third-party-artifacts.sh --write`
- Verify artifact freshness:
  - `bash scripts/generate-third-party-artifacts.sh --check`
- Run regression tests:
  - `bash tests/third-party-artifacts/generator.test.sh`
- Generator/runtime prerequisites:
  - `cargo`, `jq`, and `curl`
  - one SHA-256 provider (`shasum`, `sha256sum`, or `openssl`)
  - standard shell utilities used by the script (`awk`, `cmp`, `mktemp`, `sed`, `sort`)

## Related maintainer docs

- Packaging/install/macOS acceptance: `docs/PACKAGING.md`
- Release/tag/publish flow: `docs/RELEASE.md`
- Cross-workflow runtime and troubleshooting standards: `ALFRED_WORKFLOW_DEVELOPMENT.md`

## Install (macOS)

```bash
# Rust + cargo tools used by this repo
scripts/setup-rust-tooling.sh

# Shell tools
brew install shellcheck shfmt

# Packaging helpers
brew install zip unzip

# Node + Playwright deps for cambridge-dict scraper tests
npm ci
# Optional (only for live scraping checks)
npx playwright install chromium
```

## Install (Ubuntu/Debian)

```bash
# Base build + shell tools
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev git jq ripgrep shellcheck shfmt zip unzip

# Rust + cargo tools used by this repo
scripts/setup-rust-tooling.sh

# Node + Playwright deps for cambridge-dict scraper tests
npm ci
# Optional (only for live scraping checks)
npx playwright install chromium
```

## Verify

```bash
rustc --version
cargo --version
cargo fmt --version
cargo clippy --version
cargo nextest --version
cargo llvm-cov --version
git --version
jq --version
rg --version
shellcheck --version
shfmt --version
node --version
npm --version
npx playwright --version
zip -v | head -n 1
```

# Packaging Guide

Use this file for maintainer-side packaging, install, and macOS acceptance flows.

- Development/build/lint/test gates: [../DEVELOPMENT.md](../DEVELOPMENT.md)
- Local tool and runtime prerequisites: [../BINARY_DEPENDENCIES.md](../BINARY_DEPENDENCIES.md)
- Release/tag/publish flow: [RELEASE.md](RELEASE.md)
- Cross-workflow runtime and troubleshooting standards:
  [../ALFRED_WORKFLOW_DEVELOPMENT.md](../ALFRED_WORKFLOW_DEVELOPMENT.md)

## Packaging commands

- List workflow ids:
  - `scripts/workflow-pack.sh --list`
- Pack one workflow:
  - `scripts/workflow-pack.sh --id <workflow-id>`
- Pack and install with Alfred UI import flow:
  - `scripts/workflow-pack.sh --id <workflow-id> --install --mode ui`
- Pack and background-install one already-installed workflow:
  - `scripts/workflow-pack.sh --id <workflow-id> --install --mode background`
  - Preserves `prefs.plist` plus installed hotkey and keyword customizations by default.
  - Add `--no-preserve-customizations` to reset hotkeys and keywords to packaged defaults during background install.
- Install the latest already-built artifact only (skip rebuild):
  - `scripts/workflow-pack.sh --id <workflow-id> --install-only`
- Pack all workflows and background-install already-installed ones:
  - `scripts/workflow-pack.sh --all --install`
  - Add `--no-preserve-customizations` to reset installed hotkeys and keywords for every updated workflow.
- Pack all workflows:
  - `scripts/workflow-pack.sh --all`

## Packaging policy

### Bundled runtime packaging policy

- When a workflow bundles a runtime binary, packaging resolves it from the
  official pinned release, not from a local build or crates.io.
- Production packaging always downloads the official pinned target archive and
  its checksum, then verifies both against the repository-pinned SHA-256 before
  extracting the binary. The install root is a download/extraction workspace,
  not a trusted binary cache.
- Repository-pinned per-target digests are the source of truth: every downloaded
  candidate is verified against them, so a mutable co-published checksum is never
  trusted on its own. This keeps packaging reproducible across machines without
  relying on a local binary's self-reported version.
- Local binary overrides, alternate release base URLs, and checksum overrides are
  explicit test-mode-only inputs for hermetic fixtures. Production packaging
  rejects them. For the Codex workflow these are `CODEX_CLI_PACK_BIN`,
  `CODEX_CLI_RELEASE_BASE_URL`, and the checksum override, all gated behind
  `CODEX_CLI_RELEASE_TEST_MODE=1`.
- The Codex workflow is the reference implementation of this policy: it consumes
  the release-coupled `codex-cli` binary from the `sympoies/nils-cli` GitHub
  release because current nils-cli bundle versions can lead the independently
  published crates.io package. Restoring a crates.io `cargo install` packaging
  path is disallowed and enforced by `scripts/ci/ci-workflow-audit.sh`.

### External crate exact-pin policy

- Third-party crates used by workspace crates must be exact-pinned (for example `foo = "=1.2.3"`), not loose semver
  ranges.
- Add or update external crates with exact version syntax:
  - `cargo add <crate>@=<version>`
- For reproducibility, commit both `Cargo.toml` and `Cargo.lock` updates together after the pin change.

## macOS acceptance

- For workflows that bundle executables, include a quarantine/Gatekeeper check during final acceptance on macOS.
- Rebuild and install the target workflow before acceptance:
  - `scripts/workflow-pack.sh --id <workflow-id> --install`
- For workflows that use helper-based runtime resolution, verify the shared resolver policy:
  - `bash scripts/workflow-cli-resolver-audit.sh --check`
  - Read-only check, no `--apply` mode. The audit reports drift only; remediation is manual (or via the
    workflow-specific runbook).
- If Gatekeeper blocks execution, start from:
  - `ALFRED_WORKFLOW_DEVELOPMENT.md`
  - `workflows/<workflow-id>/TROUBLESHOOTING.md`
  - `workflows/<workflow-id>/README.md`
- Standalone quarantine-clear script remains fallback-only for locked-down environments:
  - `scripts/workflow-clear-quarantine-standalone.sh`

### Runtime inventory audit commands

Use commands instead of a hard-coded workflow list when validating which workflows bundle runtimes or helper wiring.

```bash
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

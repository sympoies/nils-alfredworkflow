# Packaging Guide

Use this file for maintainer-side packaging, install, and macOS acceptance flows.

- Development/build/lint/test gates: `../DEVELOPMENT.md`
- Local tool and runtime prerequisites: `../BINARY_DEPENDENCIES.md`
- Release/tag/publish flow: `docs/RELEASE.md`
- Cross-workflow runtime and troubleshooting standards: `../ALFRED_WORKFLOW_DEVELOPMENT.md`

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

### Crates.io runtime packaging policy

- When a workflow bundles a runtime binary published on crates.io, packaging scripts must follow this order:
  1. Prefer explicit local override (for example `*_PACK_BIN`).
  2. Then use local PATH binary.
  3. If the binary is missing or not the pinned version, auto-install the pinned crate version from crates.io via
     `cargo install --locked --root <cache-root>` and bundle that installed binary.
- This policy avoids accidental version drift while keeping packaging reproducible across machines.

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

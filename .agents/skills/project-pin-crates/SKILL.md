---
name: project-pin-crates
description: Pin managed crates and workflow runtime crate versions to an exact release.
argument-hint: "--version <x.y.z> [--targets <t,…>] [--dry-run]"
allowed-tools: Bash, Read
---

# Pin Crates

## Contract

Prereqs:

- Run inside this repository work tree.
- `bash`, `git`, `perl`, and `python3` available on `PATH`.
- Managed target files must exist in the project layout.
- Network access to the target's release source for preflight verification:
  GitHub Releases for `codex-cli`, crates.io for cargo-managed targets.
- `semantic-commit` required when `--auto-commit` or `--auto-push` is enabled.

Inputs:

- Required:
  - `--version <x.y.z|pre-release>`
- Optional:
  - `--targets <target[,target...]>`
  - `--all` (pin all managed targets; default when `--targets` omitted)
  - `--dry-run` (validate and print planned updates without writing files)
  - `--update-lock` (run `cargo update -p <crate> --precise <version>` for cargo targets)
  - `--auto-commit` (stage touched files and create a semantic commit)
  - `--auto-push` (push current branch after auto-commit; implies `--auto-commit`)
  - `--push-remote <remote>` (remote name for auto-push; default `origin`)
  - `--list-targets`

Managed targets:

- `codex-cli`
  - aliases: `codex-cli`, `codex`, `nils-codex-cli`
  - release source: `sympoies/nils-cli` GitHub release bundle
  - updates: codex workflow release-binary pin + related docs text
- `memo`
  - aliases: `memo`, `nils-memo`
  - published crate: `nils-memo`
  - updates: cargo dependency pin + related docs text

Outputs:

- Prints deterministic change summary:
  - selected targets
  - resolved release source or published crate names
  - GitHub release/crates.io version verification status
  - touched files
  - lockfile update status
  - auto-commit and auto-push status
- On non-dry-run mode, writes version pins to mapped files.
- When auto flags are enabled, creates semantic commit and optionally pushes branch.

Exit codes:

- `0`: success
- `1`: runtime failure
- `2`: usage error

Failure modes:

- `--version` missing (except `--list-targets`).
- Unknown target alias in `--targets`.
- Invalid semver-like version format.
- Required file missing.
- Pattern replacement expected by mapping not found.
- Target version not found in its release source.
- GitHub release or crates.io lookup failed before pinning.
- `cargo update` failed when `--update-lock` is enabled.
- `semantic-commit` missing when `--auto-commit`/`--auto-push` is enabled.
- Staged index not empty when `--auto-commit` is enabled.
- `git push` failed when `--auto-push` is enabled.

## Scripts (only entrypoints)

- `<PROJECT_ROOT>/.agents/skills/project-pin-crates/scripts/project-pin-crates.sh`

## Workflow

1. Resolve repo root and parse flags.
2. Resolve user-provided target aliases to managed targets.
3. Verify the selected Codex nils-cli release bundle on GitHub Releases and
   cargo-managed target versions on crates.io.
4. Apply file updates for each target:
   - `codex-cli`: runtime script and docs pin strings.
   - `memo`: cargo dependency and docs pin strings.
5. Optionally run lock sync (`--update-lock`) for cargo targets.
6. Optionally run semantic auto-commit and auto-push.
7. Print summary with touched files and crates.

## Boundary

This is a project-local skill scoped to version pinning for this repository's
managed crates and workflow runtime versions. It only touches the documented
target mappings (pin strings, related docs text, and the Cargo lockfile under
`--update-lock`) and — when explicitly requested — creates a `semantic-commit`
and pushes the current branch. It must not mutate runtime-kit manifests,
rendered product output, global runtime homes, credentials, sessions, or files
outside the declared managed targets.

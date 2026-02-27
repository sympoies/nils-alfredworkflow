---
name: nils-alfredworkflow-release-workflow
description: Create and push a release tag to trigger GitHub Release workflow.
---

# Release Tag

## Contract

Prereqs:

- Run inside this repository git work tree.
- `git` available on `PATH`.
- `semantic-commit` available on `PATH` (used for automated version-bump commit when needed).
- If tracked `package*.json` files are present: `node` available on `PATH` (used for JSON version sync).
- Remote `origin` configured and reachable.
- Release workflow listens on `v*` tags:
  - `.github/workflows/release.yml`
  - `on.push.tags: ["v*"]`

Inputs:

- Required:
  - `<version>` (for example `v0.1.0`)
- Optional:
  - `--remote <name>` (default `origin`)
  - `--dry-run` (validate and print planned actions only)

Outputs:

- Syncs version values to the provided input version (`vX.Y.Z` -> `X.Y.Z`) for:
  - explicit `version = "..."` entries in tracked `Cargo.toml` files
  - tracked `workflows/*/workflow.toml` manifests (excluding `_template`)
  - tracked root `package.json` and `package-lock.json` version fields
  - `workflows/bangumi-search/src/info.plist.template` `BANGUMI_USER_AGENT` placeholder (`nils-bangumi-cli/X.Y.Z`)
- Refreshes tracked `Cargo.lock` workspace package versions when present.
- Creates a version-bump commit when sync changes are needed.
- Pushes the version-bump commit to the current upstream branch.
- Creates an annotated git tag (`Release <version>`).
- Pushes tag to remote (`git push <remote> refs/tags/<version>`).
- Prints release URL when remote is GitHub-compatible.

Exit codes:

- `0`: success
- `1`: operational failure (`git`/remote/tag push error)
- `2`: usage error
- `3`: precondition failure (not git repo, dirty tree, missing remote, duplicate tag)

Failure modes:

- Invalid version format (must start with `v`).
- Working tree not clean.
- Current branch has no upstream or is behind upstream.
- Tag already exists locally or on remote.
- `origin` (or provided remote) not configured.
- Push failed due to auth/permissions/network.

## Scripts (only entrypoints)

- `<PROJECT_ROOT>/.agents/skills/nils-alfredworkflow-release-workflow/scripts/nils-alfredworkflow-release-workflow.sh`

## Workflow

1. Validate repository state (`git` repo, clean tree, remote exists, upstream branch ready).
2. Validate version format and tag uniqueness (local + remote).
3. Sync versions (`Cargo.toml` + workflow manifests + root `package*.json` + Bangumi User-Agent placeholder) to input
   semver and commit/push when needed.
4. Create annotated tag `Release <version>`.
5. Push tag to remote.
6. Print success summary and release URL.

---
name: nils-alfredworkflow-release-workflow
description: Create and push a release tag to trigger GitHub Release workflow.
---

# Release Tag

## Contract

Prereqs:

- Run inside this repository git work tree.
- `git` available on `PATH`.
- `gh` available on `PATH` and authenticated (`gh auth status`) for GitHub remotes.
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
  - `--force-tag` (overwrite existing local/remote tag with same version)
  - `--poll-seconds <n>` (default `15`)
  - `--max-wait-seconds <n>` (default `1800`)

Outputs:

- Syncs version values to the provided input version (`vX.Y.Z` -> `X.Y.Z`) for:
  - explicit `version = "..."` entries in tracked `Cargo.toml` files
  - tracked `workflows/*/workflow.toml` manifests (excluding `_template`)
  - tracked root `package.json` and `package-lock.json` version fields
  - `workflows/bangumi-search/src/info.plist.template` `BANGUMI_USER_AGENT` placeholder (`nils-bangumi-cli/X.Y.Z`)
- Refreshes tracked `Cargo.lock` workspace package versions when present.
- Regenerates tracked root `THIRD_PARTY_LICENSES.md` when its inputs changed.
- Creates a version-bump commit when sync changes are needed.
- Pushes the version-bump commit to the current upstream branch.
- Waits until CI workflow (`.github/workflows/ci.yml`) for the release head commit is `success` (GitHub remotes).
- Creates an annotated git tag (`Release <version>`).
- Pushes tag to remote (`git push <remote> refs/tags/<version>`).
- Waits until the release workflow run for the pushed tag is `success`.
- Waits until release page exists for the pushed tag (GitHub remotes).
- Success is reported only after both checks above pass.

Exit codes:

- `0`: success
- `1`: operational failure (`git`/remote/tag push error)
- `2`: usage error
- `3`: precondition failure (not git repo, dirty tree, missing remote, duplicate tag)
- `124`: timeout while waiting for release workflow/release page

Failure modes:

- Invalid version format (must start with `v`).
- Working tree not clean.
- Current branch has no upstream or is behind upstream.
- Tag already exists locally or on remote.
  - Use `--force-tag` when re-tagging an existing release version is intentional.
- `origin` (or provided remote) not configured.
- Push failed due to auth/permissions/network.
- CI workflow run failed for the release head commit.
- Release workflow run failed for the pushed tag.
- Release page did not appear before timeout.

## Scripts (only entrypoints)

- `<PROJECT_ROOT>/.agents/skills/nils-alfredworkflow-release-workflow/scripts/nils-alfredworkflow-release-workflow.sh`

## Workflow

1. Validate repository state (`git` repo, clean tree, remote exists, upstream branch ready).
2. Validate version format and tag uniqueness (local + remote).
3. Sync versions (`Cargo.toml` + workflow manifests + root `package*.json` + Bangumi User-Agent placeholder), refresh
   `Cargo.lock` and tracked `THIRD_PARTY_LICENSES.md` as needed, then commit/push when changes exist.
4. Wait CI workflow (`.github/workflows/ci.yml`) for the release head commit to complete with `success`.
5. Create annotated tag `Release <version>`.
6. Push tag to remote.
7. Wait release workflow for the tag to complete with `success`.
8. Wait release page to appear for the tag.
9. If CI/release workflow fails, fix root cause on branch, update/push fix commit, then recreate/re-push tag and repeat until green.

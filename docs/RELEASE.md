# Release

## Version source of truth

- Workflow package versions live in `workflows/<id>/workflow.toml`.
- Rust crate versions are workspace-driven via `Cargo.toml` (`[workspace.package].version`).
- Release tag input (`vX.Y.Z`) is treated as the source for both (`X.Y.Z`).

When using `.agents/skills/nils-alfredworkflow-release-workflow/scripts/nils-alfredworkflow-release-workflow.sh`, the script will:

1. Sync explicit `version = "..."` entries in tracked `Cargo.toml` files.
2. Sync tracked `workflows/*/workflow.toml` versions (excluding `_template`).
3. Refresh tracked `Cargo.lock` workspace package versions (when present).
4. Commit/push version bumps (when needed), then create/push the release tag.

## Local release dry run

1. `scripts/workflow-lint.sh`
2. `scripts/workflow-test.sh`
3. `scripts/workflow-pack.sh --all`

Artifacts are written to `dist/<workflow-id>/<version>/`.

## Documentation freshness checks (before tag)

1. Confirm documentation ownership/retention decisions are current:
   - `docs/reports/docs-ownership-matrix.md`
2. For native Google CLI scope changes, rerun and refresh:
   - `docs/reports/google-cli-native-validation-report.md`
3. For workflow behavior changes, rerun validation commands documented in:
   - `workflows/<workflow-id>/README.md`

## CI release

Tag push (`v*`) triggers `.github/workflows/release.yml` and uploads built `.alfredworkflow` artifacts and checksums.

## Third-party artifacts release assets

Release uploads include third-party compliance artifacts under `dist/release-bundles/`:

- `THIRD_PARTY_LICENSES.md`
- `THIRD_PARTY_LICENSES.md.sha256`
- `THIRD_PARTY_NOTICES.md`
- `THIRD_PARTY_NOTICES.md.sha256`

Before upload, release CI runs:

1. `bash scripts/generate-third-party-artifacts.sh --write`
2. `bash scripts/generate-third-party-artifacts.sh --check`
3. `bash scripts/ci/release-bundle-third-party-audit.sh --tag <tag> --dist-dir dist/release-bundles`

To validate locally after packaging:

1. `bash scripts/generate-third-party-artifacts.sh --write`
2. `bash scripts/generate-third-party-artifacts.sh --check`
3. `bash scripts/workflow-pack.sh --all`
4. `bash scripts/ci/release-bundle-third-party-audit.sh --tag v0.0.0-test --dist-dir dist/release-bundles`

## Third-party artifacts gate remediation

If release-time checks fail on third-party artifact freshness or audit gate commands, run:

1. Regenerate and verify:
   - `bash scripts/generate-third-party-artifacts.sh --write`
   - `bash scripts/generate-third-party-artifacts.sh --check`
2. Re-run strict audit gate:
   - `bash scripts/ci/third-party-artifacts-audit.sh --strict`
3. Re-run release bundle audit gate:
   - `bash scripts/ci/release-bundle-third-party-audit.sh --tag <tag> --dist-dir dist/release-bundles`
4. Retry release flow after the checks pass.

For detailed troubleshooting (including crates.io lookup failures), use:

- `TROUBLESHOOTING.md` -> `Third-party artifacts route`

# Release

## Version source of truth

- Workflow package versions live in `workflows/<id>/workflow.toml`.
- Rust crate versions are workspace-driven via `Cargo.toml` (`[workspace.package].version`).
- Release tag input (`vX.Y.Z`) is treated as the source for both (`X.Y.Z`).

When using `.agents/skills/project-release-workflow/scripts/project-release-workflow.sh`, the script will:

1. Sync explicit `version = "..."` entries in tracked `Cargo.toml` files.
2. Sync tracked `workflows/*/workflow.toml` versions (excluding `_template`).
3. Refresh tracked `Cargo.lock` workspace package versions (when present).
4. Commit/push version bumps (when needed), then create/push the release tag.

## Local release dry run

1. `scripts/workflow-lint.sh`
2. `scripts/workflow-test.sh`
3. `scripts/workflow-pack.sh --all`

Artifacts are written to `dist/<workflow-id>/<version>/`.

## Rust crate publishing (crates.io)

1. Dry-run publish checks for all crates from `release/crates-io-publish-order.txt`:
   - `scripts/publish-crates.sh --dry-run`
2. Dry-run publish checks for a single crate:
   - `scripts/publish-crates.sh --dry-run --crates "<crate-name>"`
3. Publish all crates in dependency order:
   - `CARGO_REGISTRY_TOKEN=... scripts/publish-crates.sh --publish`
4. Publish a subset:
   - `scripts/publish-crates.sh --publish --crates "nils-alfred-core nils-workflow-common"`

## Documentation freshness checks (before tag)

1. For CLI contract changes, refresh canonical specs:
   - `docs/specs/cli-shared-runtime-contract.md`
   - `docs/specs/cli-json-envelope-v1.md`
   - `docs/specs/cli-error-code-registry.md`
2. For native Google CLI scope changes, rerun validation commands documented in:
   - `crates/google-cli/README.md`
3. For workflow behavior changes, rerun validation commands documented in:
   - `workflows/<workflow-id>/README.md`

## CI release

Tag push (`v*`) triggers `.github/workflows/release.yml`.

The workflow now publishes two release surfaces on the same GitHub Release:

- Alfred workflow assets:
  - built `.alfredworkflow` files
  - `workflows-vX.Y.Z.zip`
  - `workflow-clear-quarantine-standalone.sh`
  - third-party compliance artifacts and checksums
- Standalone CLI bundle assets:
  - `nils-alfred-cli-vX.Y.Z-<target>.tar.gz`
  - `nils-alfred-cli-vX.Y.Z-<target>.tar.gz.sha256`

Standalone CLI targets match the `nils-cli` binary release flow:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

After the GitHub Release is published, release CI dispatches
`nils-alfred-cli-release` to `sympoies/homebrew-tap`. The tap workflow updates
the `nils-alfred-cli` formula, validates it on macOS and Linux, commits the
formula through the GitHub Contents API, and creates a tap-side
`nils-alfred-cli-vX.Y.Z` release.

The release workflow requires the tagged commit to have a green `validate` CI
check before packaging starts. Tag from a commit that has already passed CI on
`main`.

## Standalone CLI bundle

The bundle manifest is [release/standalone-cli-bins.txt](../release/standalone-cli-bins.txt).
It is the source of truth for release packaging, tarball audits, and CI build
selection.

Current bundled binaries:

- `bangumi-cli`
- `bilibili-cli`
- `epoch-cli`
- `market-cli`
- `quote-cli`
- `randomer-cli`
- `steam-cli`
- `timezone-cli`
- `weather-cli`
- `wiki-cli`
- `workflow-cli`
- `workflow-readme-cli`

Excluded from the standalone bundle for now:

- `brave-cli`: requires a Brave API key and is still legacy Alfred JSON-first.
- `spotify-cli`: requires Spotify credentials and is still legacy Alfred JSON-first.
- `youtube-cli`: requires a YouTube API key and is still legacy Alfred JSON-first.
- `cambridge-cli`: requires the external Node/Playwright scraper runtime.
- `google-cli`: native auth/keyring support currently pulls native Turso/simsimd dependencies that are not yet stable
  across the four standalone release targets.
- `memo-workflow-cli`: local memo DB operations currently pull native Turso/simsimd dependencies that are not yet
  stable across the four standalone release targets.

To validate the bundle packaging locally without building release binaries:

1. `bash scripts/tests/standalone_cli_release_tarball.test.sh`
2. `bash scripts/ci/release-standalone-cli-tarball-audit.sh --tag v9.9.9 --target x86_64-unknown-linux-gnu --dist-dir <test-dist-dir>`

To build a real local tarball for the current host target, first build the
manifest binaries, then run the package script. Example:

```bash
bins=()
while IFS= read -r bin; do
  bins+=("$bin")
done < <(awk -F'|' '/^[[:space:]]*($|#)/ { next } { gsub(/^[[:space:]]+|[[:space:]]+$/, "", $1); print $1 }' release/standalone-cli-bins.txt)
args=(--release --locked)
for bin in "${bins[@]}"; do
  args+=(--bin "$bin")
done
cargo build "${args[@]}"
bash scripts/ci/build-standalone-cli-release-tarball.sh --tag v0.0.0-test --target "$(rustc -vV | awk '/host:/ { print $2 }')"
```

## Third-party artifacts release assets

Release uploads include third-party compliance artifacts under `dist/release-bundles/`:

- [THIRD_PARTY_LICENSES.md](../THIRD_PARTY_LICENSES.md)
- `THIRD_PARTY_LICENSES.md.sha256`
- [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)
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

- [TROUBLESHOOTING.md](../TROUBLESHOOTING.md) -> `Third-party artifacts route`

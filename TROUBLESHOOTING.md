# Troubleshooting Index

Use this file as a quick routing index. Operational standards remain in
`ALFRED_WORKFLOW_DEVELOPMENT.md`; workflow-specific runbooks live under
`workflows/<workflow-id>/TROUBLESHOOTING.md`.

## Global checks

- `scripts/workflow-lint.sh`
- `scripts/workflow-test.sh`
- `scripts/workflow-pack.sh --all`

## Third-party license route

Use this route for `THIRD_PARTY_LICENSES.md` drift, runtime crates.io metadata lookup failures, or CI/release license
gate failures.

1. Regenerate and verify the artifact:
   - `bash scripts/generate-third-party-licenses.sh --write`
   - `bash scripts/generate-third-party-licenses.sh --check`
2. If generator output includes `failed to fetch runtime crate metadata from crates.io`:
   - Verify network access and retry:
     - `bash scripts/generate-third-party-licenses.sh --write`
   - Confirm runtime crate pin source:
     - `sed -n '1,120p' scripts/lib/codex_cli_version.sh`
3. Re-run CI/release gate checks locally:
   - `bash scripts/ci/third-party-licenses-audit.sh --strict`
   - `bash scripts/ci/release-bundle-third-party-audit.sh --tag <tag> --dist-dir dist/release-bundles`
4. If failures persist, follow release-specific guidance:
   - `docs/RELEASE.md` (`Third-party license gate remediation`)

## Workflow-local runbooks

- `workflows/bilibili-search/TROUBLESHOOTING.md`
- `workflows/wiki-search/TROUBLESHOOTING.md`
- `workflows/google-search/TROUBLESHOOTING.md`
- `workflows/youtube-search/TROUBLESHOOTING.md`
- `workflows/bangumi-search/TROUBLESHOOTING.md`

## Bilibili quick route

- Runtime checks: `bash workflows/bilibili-search/tests/smoke.sh`
- Packaging check: `scripts/workflow-pack.sh --id bilibili-search`
- If failures persist, follow rollback steps in
  `workflows/bilibili-search/TROUBLESHOOTING.md`.

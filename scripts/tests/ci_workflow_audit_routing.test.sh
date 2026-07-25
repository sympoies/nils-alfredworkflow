#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT

mkdir -p \
  "$fixture_root/scripts/ci" \
  "$fixture_root/.github/workflows" \
  "$fixture_root/docs"
cp "$repo_root/scripts/ci/ci-workflow-audit.sh" \
  "$fixture_root/scripts/ci/ci-workflow-audit.sh"
# shellcheck disable=SC2016
printf '%s\n' \
  'codex_cli_release_install "$target" "$install_root" "$destination"' \
  >"$fixture_root/scripts/ci/ci-bootstrap.sh"
printf '%s\n' \
  'Production packaging always downloads the official pinned target archive' \
  >"$fixture_root/docs/PACKAGING.md"

write_workflows() {
  local validation_run="$1"
  local extra_run="${2:-}"
  # shellcheck disable=SC2016
  printf '%s\n' \
    "jobs:" \
    "  validate:" \
    "    steps:" \
    "      - run: bash scripts/ci/ci-bootstrap.sh --context ci --install-codex-cli" \
    "      - name: Validate project" \
    "        env:" \
    '          GITHUB_TOKEN: ${{ github.token }}' \
    "$validation_run" \
    "      - run: bash scripts/ci/ci-run-gates.sh package-smoke --skip-arch-check" \
    "$extra_run" \
    >"$fixture_root/.github/workflows/ci.yml"
  # shellcheck disable=SC2016
  printf '%s\n' \
    "jobs:" \
    "  package:" \
    "    steps:" \
    '      - run: bash scripts/ci/ci-bootstrap.sh --context release --install-codex-cli' \
    "      - name: Run release package gates" \
    "        env:" \
    '          GITHUB_TOKEN: ${{ github.token }}' \
    '        run: bash scripts/ci/ci-run-gates.sh release-package --tag "${GITHUB_REF_NAME}"' \
    "      - name: Regenerate third-party artifacts" \
    "        env:" \
    '          GITHUB_TOKEN: ${{ github.token }}' \
    "        run: echo regenerate" \
    >"$fixture_root/.github/workflows/release.yml"
  printf '%s\n' \
    "jobs:" \
    "  publish:" \
    "    steps:" \
    "      - run: bash scripts/ci/ci-bootstrap.sh --context publish-crates" \
    "      - run: bash scripts/ci/ci-run-gates.sh publish-crates" \
    >"$fixture_root/.github/workflows/publish-crates.yml"
}

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null

write_workflows \
  "        # run: bash scripts/local-pre-commit.sh --mode ci"
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: commented validation route satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci" \
  $'      - run: |\n          echo before\n          bash scripts/ci/ci-run-gates.sh lint'
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: multiline duplicate ordinary gate satisfied the audit" >&2
  exit 1
fi

echo "ok: CI workflow routing audit tests passed"

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

# The Dependabot refresh pair is written separately from the canonical gate
# workflows so a case can break one of its two invariants without disturbing
# the rest of the fixture.
write_dependabot_workflows() {
  local generate_env="${1:-token}"
  local apply_checkout="${2:-}"
  local generate_permissions="${3:-read}"
  local apply_cargo_deny_gate="${4:-gated}"
  local apply_pr_lookup="${5:-scoped}"
  local apply_fork_guard="${6:-guarded}"
  local apply_author_guard="${7:-guarded}"

  {
    printf '%s\n' "permissions:"
    if [[ "$generate_permissions" == "read" ]]; then
      printf '%s\n' "  contents: read"
    else
      printf '%s\n' "  contents: write"
    fi
    printf '%s\n' \
      "jobs:" \
      "  generate:" \
      "    steps:" \
      "      - name: Regenerate third-party artifacts"
    if [[ "$generate_env" == "token" ]]; then
      # shellcheck disable=SC2016
      printf '%s\n' \
        "        env:" \
        '          GITHUB_TOKEN: ${{ github.token }}'
    fi
    printf '%s\n' \
      "        run: bash scripts/generate-third-party-artifacts.sh --check"
  } >"$fixture_root/.github/workflows/dependabot-third-party-artifacts.yml"

  {
    printf '%s\n' \
      "on:" \
      "  workflow_run:" \
      "    workflows:" \
      "      - CI"
    if [[ "$apply_cargo_deny_gate" == "gated" ]]; then
      printf '%s\n' "      - cargo-deny"
    fi
    printf '%s\n' \
      "jobs:" \
      "  commit:" \
      "    steps:"
    if [[ -n "$apply_checkout" ]]; then
      printf '%s\n' "      - uses: actions/checkout@v7"
    fi
    printf '%s\n' \
      "      - name: Commit the refresh" \
      "        run: |"
    # shellcheck disable=SC2016
    if [[ "$apply_pr_lookup" == "scoped" ]]; then
      printf '%s\n' \
        '          gh pr list --head "${HEAD_BRANCH}" --state open'
    else
      printf '%s\n' \
        '          gh pr list --head "${GITHUB_REPOSITORY_OWNER}:${HEAD_BRANCH}" --state open'
    fi
    if [[ "$apply_fork_guard" == "guarded" ]]; then
      printf '%s\n' \
        "          # --jq '[.[] | select(.isCrossRepository == false)][0] // empty'"
    else
      printf '%s\n' \
        "          # --jq '.[0] // empty'"
    fi
    if [[ "$apply_author_guard" == "guarded" ]]; then
      # shellcheck disable=SC2016
      printf '%s\n' \
        '          case "${author}" in' \
        '            "dependabot[bot]"|"app/dependabot") ;;' \
        '          esac'
    else
      # shellcheck disable=SC2016
      printf '%s\n' '          test "${author}" = "dependabot[bot]"'
    fi
    printf '%s\n' \
      "  merge:"
    if [[ "$apply_cargo_deny_gate" == "gated" ]]; then
      # shellcheck disable=SC2016
      printf '%s\n' \
        "    if: github.event.workflow_run.name == 'cargo-deny'" \
        "    steps:" \
        "      - name: Squash merge the bump" \
        "        run: |" \
        '          case "${author}" in' \
        '            "dependabot[bot]"|"app/dependabot") ;;' \
        '          esac' \
        "          for workflow in ci.yml cargo-deny.yml; do" \
        "            echo \"\$workflow\"" \
        "          done"
    else
      printf '%s\n' \
        "    if: github.event.workflow_run.name == 'CI'" \
        "    steps:" \
        "      - name: Squash merge the bump" \
        "        run: echo merge"
    fi
  } >"$fixture_root/.github/workflows/dependabot-third-party-apply.yml"

  printf '%s\n' \
    "name: cargo-deny" \
    "jobs:" \
    "  cargo-deny:" \
    "    steps:" \
    "      - run: echo deny" \
    >"$fixture_root/.github/workflows/cargo-deny.yml"
}

write_workflows() {
  local validation_run="$1"
  local extra_run="${2:-}"
  # shellcheck disable=SC2016
  printf '%s\n' \
    "name: CI" \
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
  write_dependabot_workflows
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

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
rm "$fixture_root/.github/workflows/dependabot-third-party-artifacts.yml"
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: missing dependabot refresh workflow satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows no-token
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: unauthenticated dependabot refresh step satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token checkout
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: privileged apply workflow checkout satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
rm "$fixture_root/.github/workflows/dependabot-third-party-apply.yml"
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: missing dependabot apply workflow satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token "" write
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: write permission on the unprivileged refresh workflow satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token "" read ungated
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: auto-merge without a cargo-deny gate satisfied the audit" >&2
  exit 1
fi

write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
rm "$fixture_root/.github/workflows/cargo-deny.yml"
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: missing cargo-deny workflow satisfied the audit" >&2
  exit 1
fi

# `gh pr list --head` documents that `<owner>:<branch>` is unsupported: it
# matches nothing, so the automation would silently skip every bump.
write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token "" read gated owner-qualified
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: owner-qualified gh pr list --head satisfied the audit" >&2
  exit 1
fi

# Without the cross-repository filter a fork branch of the same name can be
# selected, which is the finding the owner-qualified form was meant to close.
write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token "" read gated scoped unguarded
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: missing cross-repository pull request guard satisfied the audit" >&2
  exit 1
fi

# GitHub's GraphQL-backed PR response can render Dependabot as app/dependabot,
# while event payloads use dependabot[bot]. Both privileged jobs must accept it.
write_workflows \
  "        run: bash scripts/local-pre-commit.sh --mode ci"
write_dependabot_workflows token "" read gated scoped guarded unguarded
if bash "$fixture_root/scripts/ci/ci-workflow-audit.sh" --check >/dev/null 2>&1; then
  echo "error: single-form Dependabot author guard satisfied the audit" >&2
  exit 1
fi

echo "ok: CI workflow routing audit tests passed"

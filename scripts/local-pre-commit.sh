#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

mode="default"
with_package_smoke=0
skip_node_scraper_tests=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/local-pre-commit.sh [--mode <default|ci>] [--with-package-smoke] [--skip-node-scraper-tests]

Modes:
  default  Local baseline without duplicated third-party audit.
           Runs: workflow-lint -> script-filter-policy-check -> node-scraper-tests -> workflow-test --skip-third-party-audit
  ci       Exact CI gate order.
           Runs: ci-run-gates lint -> third-party-artifacts-audit -> node-scraper-tests -> test

Options:
  --with-package-smoke     Append package smoke gate after selected mode flow.
  --skip-node-scraper-tests Skip npm scraper tests (iteration-only shortcut; not CI parity).
USAGE
}

die() {
  echo "error: $*" >&2
  exit 2
}

run() {
  local -a run_cmd=("$@")
  echo "+ ${run_cmd[*]}"
  "${run_cmd[@]}"
}

run_default_mode() {
  run bash scripts/workflow-lint.sh
  run bash scripts/workflow-sync-script-filter-policy.sh --check

  if [[ "$skip_node_scraper_tests" -eq 0 ]]; then
    run npm run test:cambridge-scraper
  else
    echo "skip: node scraper tests disabled (--skip-node-scraper-tests)"
  fi

  run bash scripts/workflow-test.sh --skip-third-party-audit
}

run_ci_mode() {
  run bash scripts/ci/ci-run-gates.sh lint
  run bash scripts/ci/ci-run-gates.sh third-party-artifacts-audit

  if [[ "$skip_node_scraper_tests" -eq 0 ]]; then
    run bash scripts/ci/ci-run-gates.sh node-scraper-tests
  else
    echo "skip: node scraper tests disabled (--skip-node-scraper-tests)"
  fi

  run bash scripts/ci/ci-run-gates.sh test
}

while [[ $# -gt 0 ]]; do
  case "${1:-}" in
  --mode)
    [[ $# -ge 2 ]] || die "--mode requires a value"
    mode="${2:-}"
    shift 2
    ;;
  --with-package-smoke)
    with_package_smoke=1
    shift
    ;;
  --skip-node-scraper-tests)
    skip_node_scraper_tests=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    die "unknown argument: ${1:-}"
    ;;
  esac
done

if [[ "$mode" != "default" && "$mode" != "ci" ]]; then
  die "--mode must be one of: default, ci"
fi

cd "$repo_root"

if [[ "$mode" == "default" ]]; then
  run_default_mode
else
  run_ci_mode
fi

if [[ "$with_package_smoke" -eq 1 ]]; then
  run bash scripts/ci/ci-run-gates.sh package-smoke --skip-arch-check
fi

echo "ok: local pre-commit checks passed (mode=$mode, package_smoke=$with_package_smoke, skip_node_scraper_tests=$skip_node_scraper_tests)"

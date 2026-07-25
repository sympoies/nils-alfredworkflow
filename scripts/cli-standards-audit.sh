#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

strict_warnings=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/cli-standards-audit.sh [--strict]

Options:
  --strict   Treat warnings as failures.
  -h, --help Show help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --strict)
    strict_warnings=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown argument: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done

hard_failures=0
warnings=0

repo_pass() {
  local message="$1"
  printf 'PASS [repo] %s\n' "$message"
}

repo_fail() {
  local message="$1"
  printf 'FAIL [repo] %s\n' "$message"
  hard_failures=$((hard_failures + 1))
}

echo "== CLI standards audit =="

required_docs=(
  "docs/specs/cli-shared-runtime-contract.md"
  "docs/specs/cli-json-envelope-v1.md"
  "docs/specs/cli-error-code-registry.md"
)

for rel in "${required_docs[@]}"; do
  if [[ -f "$repo_root/$rel" ]]; then
    repo_pass "required doc present: $rel"
  else
    repo_fail "required doc missing: $rel (create via Sprint 1 tasks)"
  fi
done

if rg -n '^nils-weather-cli$' "$repo_root/release/crates-io-publish-order.txt" >/dev/null; then
  repo_pass "publish order includes nils-weather-cli"
else
  repo_fail "release/crates-io-publish-order.txt must include nils-weather-cli"
fi

if rg -n 'cli-standards-audit' "$repo_root/scripts/workflow-lint.sh" >/dev/null; then
  repo_pass "workflow-lint includes cli-standards-audit gate"
else
  repo_fail "scripts/workflow-lint.sh must run scripts/cli-standards-audit.sh"
fi

if rg -n 'cli-standards-audit' "$repo_root/.github/workflows/ci.yml" >/dev/null ||
  {
    rg -n 'scripts/local-pre-commit\.sh --mode ci' \
      "$repo_root/.github/workflows/ci.yml" >/dev/null &&
      rg -n 'scripts/ci/ci-run-gates\.sh lint' \
        "$repo_root/scripts/local-pre-commit.sh" >/dev/null
  }; then
  repo_pass "CI routes to the cli-standards-audit owner"
else
  repo_fail ".github/workflows/ci.yml must route to the cli-standards-audit gate"
fi

echo
echo "== Per-crate compliance =="

shopt -s nullglob
crate_dirs=("$repo_root"/crates/*-cli)
shopt -u nullglob

if [[ ${#crate_dirs[@]} -eq 0 ]]; then
  repo_fail "no crates/*-cli found"
fi

for crate_dir in "${crate_dirs[@]}"; do
  crate_name="$(basename "$crate_dir")"
  cargo_toml="$crate_dir/Cargo.toml"
  readme="$crate_dir/README.md"
  main_rs="$crate_dir/src/main.rs"
  contract_test="$crate_dir/tests/integration/cli_contract.rs"
  if [[ ! -f "$contract_test" && -f "$crate_dir/tests/integration/cli_contract.rs" ]]; then
    contract_test="$crate_dir/tests/integration/cli_contract.rs"
  fi

  crate_failures=0
  crate_warnings=0

  echo
  printf '[%s]\n' "$crate_name"

  if [[ -f "$cargo_toml" ]]; then
    echo "  PASS: Cargo.toml present"
  else
    echo "  FAIL: missing Cargo.toml"
    crate_failures=$((crate_failures + 1))
    hard_failures=$((hard_failures + 1))
  fi

  if [[ -f "$cargo_toml" ]]; then
    if rg -n '^description\s*=\s*".+"' "$cargo_toml" >/dev/null; then
      echo "  PASS: description metadata present"
    else
      echo "  FAIL: missing Cargo.toml description (add package description for publish/readability)"
      crate_failures=$((crate_failures + 1))
      hard_failures=$((hard_failures + 1))
    fi
  fi

  if [[ -f "$readme" ]]; then
    echo "  PASS: README present"

    if rg -n '^##\s+Output Contract' "$readme" >/dev/null; then
      echo "  PASS: README Output Contract section present"
    else
      echo "  WARN: README missing '## Output Contract' section"
      crate_warnings=$((crate_warnings + 1))
      warnings=$((warnings + 1))
    fi

    if rg -n '^##\s+Standards Status' "$readme" >/dev/null; then
      echo "  PASS: README Standards Status section present"
    else
      echo "  WARN: README missing '## Standards Status' section"
      crate_warnings=$((crate_warnings + 1))
      warnings=$((warnings + 1))
    fi
  else
    echo "  FAIL: missing README.md"
    crate_failures=$((crate_failures + 1))
    hard_failures=$((hard_failures + 1))
  fi

  if [[ -f "$main_rs" ]]; then
    echo "  PASS: src/main.rs present"
  else
    echo "  FAIL: missing src/main.rs"
    crate_failures=$((crate_failures + 1))
    hard_failures=$((hard_failures + 1))
  fi

  if [[ -f "$main_rs" ]]; then
    if rg -n 'to_json\(|serde_json::to_string\(' "$main_rs" >/dev/null; then
      if rg -n '\bjson\s*:\s*bool\b|\bmode\s*:\s*[A-Za-z_]|OutputMode::Json|--json|--format' "$main_rs" >/dev/null; then
        echo "  PASS: explicit output mode indicator detected"
      else
        echo "  WARN: JSON output detected without explicit mode flag (target: --json/--format/compat mode)"
        crate_warnings=$((crate_warnings + 1))
        warnings=$((warnings + 1))
      fi
    else
      echo "  WARN: no JSON output path detected in main.rs"
      crate_warnings=$((crate_warnings + 1))
      warnings=$((warnings + 1))
    fi
  fi

  if [[ -f "$contract_test" ]]; then
    missing_keys=()
    for key in schema_version command ok; do
      if ! rg -n "$key" "$contract_test" >/dev/null; then
        missing_keys+=("$key")
      fi
    done

    if [[ ${#missing_keys[@]} -eq 0 ]]; then
      echo "  PASS: envelope contract key assertions present in tests/integration/cli_contract.rs"
    else
      echo "  WARN: tests/integration/cli_contract.rs missing envelope assertions for: ${missing_keys[*]}"
      crate_warnings=$((crate_warnings + 1))
      warnings=$((warnings + 1))
    fi
  else
    echo "  WARN: missing tests/integration/cli_contract.rs (add envelope success/failure contract tests)"
    crate_warnings=$((crate_warnings + 1))
    warnings=$((warnings + 1))
  fi

  if [[ $crate_failures -gt 0 ]]; then
    printf '  STATUS: FAIL (failures=%d warnings=%d)\n' "$crate_failures" "$crate_warnings"
  elif [[ $crate_warnings -gt 0 ]]; then
    printf '  STATUS: WARN (failures=%d warnings=%d)\n' "$crate_failures" "$crate_warnings"
  else
    printf '  STATUS: PASS (failures=%d warnings=%d)\n' "$crate_failures" "$crate_warnings"
  fi
done

echo
printf 'Summary: hard_failures=%d warnings=%d strict=%s\n' \
  "$hard_failures" \
  "$warnings" \
  "$([[ $strict_warnings -eq 1 ]] && echo true || echo false)"

if [[ $hard_failures -gt 0 ]]; then
  echo "Result: FAIL (hard failures detected)"
  exit 1
fi

if [[ $strict_warnings -eq 1 && $warnings -gt 0 ]]; then
  echo "Result: FAIL (strict mode treats warnings as failures)"
  exit 1
fi

if [[ $warnings -gt 0 ]]; then
  echo "Result: PASS with warnings (run with --strict to enforce warnings)"
else
  echo "Result: PASS"
fi

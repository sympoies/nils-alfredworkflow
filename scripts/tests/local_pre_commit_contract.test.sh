#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT

mkdir -p \
  "$fixture_root/scripts/ci" \
  "$fixture_root/selected-bin" \
  "$fixture_root/shadow-bin"
cp "$repo_root/scripts/local-pre-commit.sh" \
  "$fixture_root/scripts/local-pre-commit.sh"

write_executable() {
  local path="$1"
  local body="$2"
  printf '#!/usr/bin/env bash\nset -euo pipefail\n%s\n' "$body" >"$path"
  chmod +x "$path"
}

write_executable "$fixture_root/selected-bin/node" \
  'printf "22\n"'
# shellcheck disable=SC2016
write_executable "$fixture_root/selected-bin/npm" '
  printf "npm %s\n" "$*" >>"${CONTRACT_LOG:?}"
  if [[ "npm $*" == "${CONTRACT_FAIL:-}" ]]; then
    exit 29
  fi
'
# shellcheck disable=SC2016
write_executable "$fixture_root/shadow-bin/npm" '
  printf "shadow npm %s\n" "$*" >>"${CONTRACT_LOG:?}"
  exit 91
'
# shellcheck disable=SC2016
write_executable "$fixture_root/scripts/ci/ci-run-gates.sh" '
  printf "gate %s\n" "$*" >>"${CONTRACT_LOG:?}"
  if [[ "gate $*" == "${CONTRACT_FAIL:-}" ]]; then
    exit 37
  fi
'

for script in \
  scripts/ci/third-party-artifacts-change-gate.sh \
  scripts/workflow-lint.sh \
  scripts/workflow-sync-script-filter-policy.sh \
  scripts/workflow-test.sh; do
  mkdir -p "$fixture_root/$(dirname "$script")"
  write_executable "$fixture_root/$script" "
    printf 'default ${script#scripts/} %s\\n' \"\$*\" >>\"\${CONTRACT_LOG:?}\"
    if [[ \"default ${script#scripts/} \$*\" == \"\${CONTRACT_FAIL:-}\" ]]; then
      exit 41
    fi
  "
done

run_wrapper() {
  local log="$1"
  local fail="${2:-}"
  shift 2 || true
  env \
    PATH="$fixture_root/shadow-bin:$fixture_root/selected-bin:/usr/bin:/bin" \
    CONTRACT_LOG="$log" \
    CONTRACT_FAIL="$fail" \
    bash "$fixture_root/scripts/local-pre-commit.sh" "$@"
}

assert_log() {
  local log="$1"
  local expected="$2"
  local actual=""
  actual="$(<"$log")"
  if [[ "$actual" != "$expected" ]]; then
    printf 'expected log:\n%s\nactual log:\n%s\n' "$expected" "$actual" >&2
    exit 1
  fi
}

ci_expected=$'npm ci\ngate lint\ngate third-party-artifacts-audit\ngate node-scraper-tests\ngate script-tests\ngate test'
ci_log="$fixture_root/ci.log"
run_wrapper "$ci_log" "" --mode ci >/dev/null
assert_log "$ci_log" "$ci_expected"

default_expected=$'npm ci\ndefault ci/third-party-artifacts-change-gate.sh \ndefault workflow-lint.sh --skip-third-party-audit\ndefault workflow-sync-script-filter-policy.sh --check\nnpm run test:cambridge-scraper\ndefault workflow-test.sh --skip-third-party-audit'
default_log="$fixture_root/default.log"
run_wrapper "$default_log" "" --mode default >/dev/null
assert_log "$default_log" "$default_expected"

for fail in \
  "npm ci" \
  "gate lint" \
  "gate third-party-artifacts-audit" \
  "gate node-scraper-tests" \
  "gate script-tests" \
  "gate test"; do
  fail_log="$fixture_root/fail-${fail// /-}.log"
  if run_wrapper "$fail_log" "$fail" --mode ci >/dev/null 2>&1; then
    echo "error: injected failure unexpectedly passed: $fail" >&2
    exit 1
  else
    status=$?
  fi
  if [[ "$fail" == "npm ci" ]]; then
    [[ "$status" -eq 29 ]] || {
      echo "error: npm failure status changed: $status" >&2
      exit 1
    }
  else
    [[ "$status" -eq 37 ]] || {
      echo "error: gate failure status changed: $status" >&2
      exit 1
    }
  fi
  [[ "$(<"$fail_log")" == *"$fail" ]] || {
    echo "error: injected failure was not reached: $fail" >&2
    exit 1
  }
  [[ "$(<"$fail_log")" != *"shadow npm"* ]] || {
    echo "error: wrapper used the PATH-shadowing npm" >&2
    exit 1
  }
done

echo "ok: local pre-commit contract tests passed"

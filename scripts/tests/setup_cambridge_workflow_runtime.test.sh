#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/setup-cambridge-runtime.test.XXXXXX")"
fake_bin_dir="$test_root/bin"
workflow_dir="$test_root/workflow"
node_log="$test_root/node.log"

trap 'rm -rf "$test_root"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

mkdir -p "$fake_bin_dir" "$workflow_dir"

cat >"$fake_bin_dir/npm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF

cat >"$fake_bin_dir/node" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

code=""
while [[ $# -gt 0 ]]; do
  case "$1" in
  -e)
    code="${2:-}"
    shift 2
    ;;
  *)
    shift
    ;;
  esac
done

printf '%s\n' "$code" >>"${SETUP_CAMBRIDGE_NODE_LOG:?}"

if [[ "$code" == *"chromium.launch"* ]]; then
  printf 'ok: headless chromium launch checked\n'
  exit 0
fi

if [[ "$code" == *"require(process.argv[1])"* ]]; then
  exit 0
fi

if [[ "$code" == *"import('playwright')"* && "$code" != *"chromium.executablePath"* ]]; then
  printf 'ok: playwright package resolved\n'
  exit 0
fi

printf 'error: runtime check did not launch headless chromium\n' >&2
exit 42
EOF

chmod +x "$fake_bin_dir/npm" "$fake_bin_dir/node"

SETUP_CAMBRIDGE_NODE_LOG="$node_log" \
  PATH="$fake_bin_dir:/usr/bin:/bin" \
  "$repo_root/scripts/setup-cambridge-workflow-runtime.sh" \
  --workflow-dir "$workflow_dir" \
  --check-only

if ! grep -q 'chromium.launch' "$node_log"; then
  fail "check-only runtime verification must launch headless chromium"
fi

printf 'ok: setup-cambridge-workflow-runtime tests passed\n'

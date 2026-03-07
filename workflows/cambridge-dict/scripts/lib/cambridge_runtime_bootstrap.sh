#!/usr/bin/env bash
set -euo pipefail

workflow_dir=""
state_file=""
log_file=""
result_file=""

append_path_entry() {
  local dir="${1:-}"
  [[ -n "$dir" && -d "$dir" ]] || return 0

  case ":${PATH:-}:" in
  *":$dir:"*) ;;
  *)
    if [[ -n "${PATH:-}" ]]; then
      PATH="$dir:$PATH"
    else
      PATH="$dir"
    fi
    ;;
  esac
}

ensure_common_runtime_path() {
  append_path_entry "/opt/homebrew/bin"
  append_path_entry "/usr/local/bin"
  append_path_entry "/opt/local/bin"
  append_path_entry "/usr/bin"
  append_path_entry "/bin"
  export PATH
}

usage() {
  cat <<'USAGE'
Usage:
  cambridge_runtime_bootstrap.sh --workflow-dir <path> --state-file <path> --log-file <path> --result-file <path>
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --workflow-dir)
    workflow_dir="${2:-}"
    shift 2
    ;;
  --state-file)
    state_file="${2:-}"
    shift 2
    ;;
  --log-file)
    log_file="${2:-}"
    shift 2
    ;;
  --result-file)
    result_file="${2:-}"
    shift 2
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

[[ -n "$workflow_dir" && -d "$workflow_dir" ]] || {
  echo "error: workflow dir is required" >&2
  exit 2
}

[[ -n "$state_file" ]] || {
  echo "error: state file is required" >&2
  exit 2
}

[[ -n "$log_file" ]] || {
  echo "error: log file is required" >&2
  exit 2
}

[[ -n "$result_file" ]] || {
  echo "error: result file is required" >&2
  exit 2
}

mkdir -p "$(dirname "$state_file")" "$(dirname "$log_file")" "$(dirname "$result_file")"
: >"$log_file"
rm -f "$result_file"

cleanup() {
  printf '%s\t%s\n' "$bootstrap_status" "$(date +%s)" >"$result_file"
  rm -f "$state_file"
}
trap cleanup EXIT

ensure_common_runtime_path
bootstrap_status="err"
printf '%s\n' "$$" >"$state_file"

command -v node >/dev/null 2>&1 || {
  echo "error: node is required" >>"$log_file"
  exit 1
}

command -v npm >/dev/null 2>&1 || {
  echo "error: npm is required" >>"$log_file"
  exit 1
}

command -v npx >/dev/null 2>&1 || {
  echo "error: npx is required" >>"$log_file"
  exit 1
}

if [[ ! -f "$workflow_dir/package.json" ]]; then
  cat >"$workflow_dir/package.json" <<'JSON'
{
  "name": "cambridge-dict-runtime",
  "private": true,
  "version": "1.0.0",
  "dependencies": {
    "playwright": "^1.54.0"
  }
}
JSON
fi

{
  echo "info: bootstrapping Cambridge runtime at $workflow_dir"
  echo "info: PATH=$PATH"
  npm --prefix "$workflow_dir" install --omit=dev --no-audit --no-fund
  (
    cd "$workflow_dir"
    node --input-type=module -e "import('playwright').then(() => process.stdout.write('ok: playwright package resolved\n'))"
  )
  npx --prefix "$workflow_dir" playwright --version
  npx --prefix "$workflow_dir" playwright install chromium
  echo "ok: cambridge runtime ready"
} >>"$log_file" 2>&1

bootstrap_status="ok"

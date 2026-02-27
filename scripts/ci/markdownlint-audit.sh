#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  markdownlint-audit.sh [--strict]

Run workspace Markdown lint checks using markdownlint-cli2 and the repo baseline config.

Options:
  --strict   Treat lint failures as hard failures (exit 1)
  -h, --help Show this help
USAGE
}

strict=0
while [[ $# -gt 0 ]]; do
  case "${1:-}" in
  --strict)
    strict=1
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown argument: ${1:-}" >&2
    usage >&2
    exit 2
    ;;
  esac
done

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$repo_root" || ! -d "$repo_root" ]]; then
  echo "error: must run inside a git work tree" >&2
  exit 2
fi
cd "$repo_root"

if ! command -v npx >/dev/null 2>&1; then
  echo "error: missing required tool on PATH: npx" >&2
  echo "hint: install Node.js (includes npx)" >&2
  exit 2
fi

config_file="$repo_root/.markdownlint-cli2.jsonc"
if [[ ! -f "$config_file" ]]; then
  echo "error: missing markdownlint config: $config_file" >&2
  exit 2
fi

md_files=()
while IFS= read -r -d '' path; do
  md_files+=("$path")
done < <(git -C "$repo_root" ls-files -z '*.md')

if [[ ${#md_files[@]} -eq 0 ]]; then
  echo "PASS: markdown lint audit (strict=$strict, files=0)"
  exit 0
fi

lint_cmd=(
  npx --yes markdownlint-cli2@0.21.0
  --config "$config_file"
)
lint_cmd+=("${md_files[@]}")

echo "+ ${lint_cmd[*]}"
if "${lint_cmd[@]}"; then
  echo "PASS: markdown lint audit (strict=$strict, files=${#md_files[@]})"
  exit 0
fi

if [[ "$strict" -eq 1 ]]; then
  echo "FAIL: markdown lint audit (strict=$strict, files=${#md_files[@]})" >&2
  exit 1
fi

echo "WARN: markdown lint audit found issues (strict=$strict)" >&2
echo "PASS: markdown lint audit (warning mode)"

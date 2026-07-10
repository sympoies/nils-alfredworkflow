#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skill_root="$(cd "${script_dir}/.." && pwd)"
repo_root="$(cd "${skill_root}/../../.." && pwd)"
entrypoint="${skill_root}/scripts/project-pin-crates.sh"

if [[ ! -f "${skill_root}/SKILL.md" ]]; then
  echo "error: missing SKILL.md" >&2
  exit 1
fi
if [[ ! -f "${entrypoint}" ]]; then
  echo "error: missing scripts/project-pin-crates.sh" >&2
  exit 1
fi

if [[ ! -d "${repo_root}/workflows/codex-cli" ]]; then
  echo "error: missing expected workflow path in repo" >&2
  exit 1
fi

targets_out="$("${entrypoint}" --list-targets)"
echo "$targets_out" | rg -n "^codex-cli$" >/dev/null
echo "$targets_out" | rg -n "nils-codex-cli" >/dev/null
echo "$targets_out" | rg -n "^memo$" >/dev/null
echo "$targets_out" | rg -n "nils-memo" >/dev/null

"${entrypoint}" --version 1.0.0 --dry-run >/dev/null
"${entrypoint}" --version 1.21.6 --targets codex-cli --dry-run >/dev/null
"${entrypoint}" --version 1.0.0 --targets nils-codex-cli,nils-memo --dry-run >/dev/null
"${entrypoint}" --version 1.0.0 --dry-run --auto-commit >/dev/null
"${entrypoint}" --version 1.0.0 --dry-run --auto-push >/dev/null
"${entrypoint}" --version 1.0.0 --dry-run --auto-push --push-remote origin >/dev/null

if "${entrypoint}" --version 1.0.0 --targets unknown-target --dry-run >/dev/null 2>&1; then
  echo "error: unknown target must fail" >&2
  exit 1
fi

if "${entrypoint}" --version 1.0.0 --dry-run --push-remote origin >/dev/null 2>&1; then
  echo "error: --push-remote without --auto-push must fail" >&2
  exit 1
fi

if "${entrypoint}" --version 0.0.0-doesnotexist.pincrates --targets memo --dry-run >/dev/null 2>&1; then
  echo "error: unavailable crates.io version must fail before updates" >&2
  exit 1
fi

echo "ok: project skill smoke checks passed"

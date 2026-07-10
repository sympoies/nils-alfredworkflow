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
help_out="$("${entrypoint}" --help)"
echo "$help_out" | rg -n "GitHub Releases" >/dev/null
echo "$help_out" | rg -n "crates.io" >/dev/null
echo "$targets_out" | rg -n "^codex-cli$" >/dev/null
echo "$targets_out" | rg -n "nils-codex-cli" >/dev/null
echo "$targets_out" | rg -n "^memo$" >/dev/null
echo "$targets_out" | rg -n "nils-memo" >/dev/null

"${entrypoint}" --version 1.0.0 --dry-run >/dev/null
"${entrypoint}" --version 1.21.6 --targets codex-cli --dry-run >/dev/null

release_fixture="$(mktemp)"
trap 'rm -f "$release_fixture"' EXIT
cat >"$release_fixture" <<'JSON'
{"tag_name":"v1.21.6","assets":[{"name":"nils-cli-v1.21.6-aarch64-apple-darwin.tar.gz"}]}
JSON
if PROJECT_PIN_CRATES_NILS_CLI_RELEASE_JSON="$release_fixture" \
  "${entrypoint}" --version 1.21.6 --targets codex-cli --dry-run >/dev/null 2>&1; then
  echo "error: codex-cli pin preflight must reject a release missing its checksum asset" >&2
  exit 1
fi

cat >"$release_fixture" <<'JSON'
{"tag_name":"v1.21.6","assets":[
  {"name":"nils-cli-v1.21.6-aarch64-apple-darwin.tar.gz"},
  {"name":"nils-cli-v1.21.6-aarch64-apple-darwin.tar.gz.sha256"},
  {"name":"nils-cli-v1.21.6-x86_64-apple-darwin.tar.gz"},
  {"name":"nils-cli-v1.21.6-x86_64-apple-darwin.tar.gz.sha256"},
  {"name":"nils-cli-v1.21.6-x86_64-unknown-linux-gnu.tar.gz.sha256"},
  {"name":"nils-cli-v1.21.6-aarch64-unknown-linux-gnu.tar.gz"},
  {"name":"nils-cli-v1.21.6-aarch64-unknown-linux-gnu.tar.gz.sha256"}
]}
JSON
if PROJECT_PIN_CRATES_NILS_CLI_RELEASE_JSON="$release_fixture" \
  "${entrypoint}" --version 1.21.6 --targets codex-cli --dry-run >/dev/null 2>&1; then
  echo "error: codex-cli pin preflight must reject a missing Linux archive" >&2
  exit 1
fi
rg -n "sympoies/nils-cli v.*release" "${entrypoint}" >/dev/null || {
  echo "error: codex-cli pin helper must update the README release-version reference" >&2
  exit 1
}
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

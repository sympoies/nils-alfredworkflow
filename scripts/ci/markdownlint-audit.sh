#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  markdownlint-audit.sh [--strict]

Run workspace Markdown lint checks using rumdl and the repo baseline config.

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

config_file="$repo_root/.rumdl.toml"
if [[ ! -f "$config_file" ]]; then
  echo "error: missing rumdl config: $config_file" >&2
  exit 2
fi

md_files=()
while IFS= read -r -d '' path; do
  [[ -f "$path" ]] || continue
  md_files+=("$path")
done < <(git -C "$repo_root" ls-files -z '*.md')

if [[ ${#md_files[@]} -eq 0 ]]; then
  echo "PASS: markdown lint audit (strict=$strict, files=0)"
  exit 0
fi

lint_cmd=(
  npx --no-install rumdl
  check
  --config "$config_file"
)
lint_cmd+=("${md_files[@]}")

collect_broken_local_markdown_links() {
  python3 - "$repo_root" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

repo_root = Path(sys.argv[1]).resolve()
link_pattern = re.compile(r"\[[^\]]+\]\(([^)]+)\)")


def parse_target(raw_target: str) -> str:
    target = raw_target.strip()
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1].strip()
    match = re.match(r"^(\S+)\s+['\"(].*$", target)
    if match:
        target = match.group(1)
    return target


def resolve_local_target(source_file: Path, target: str) -> Path | None:
    lowered = target.lower()
    if (
        not target
        or target.startswith("#")
        or lowered.startswith("http://")
        or lowered.startswith("https://")
        or lowered.startswith("mailto:")
        or lowered.startswith("tel:")
        or lowered.startswith("data:")
        or lowered.startswith("ftp://")
        or "://" in target
    ):
        return None

    clean_target = target.split("#", 1)[0].split("?", 1)[0].strip()
    if not clean_target:
        return None

    if clean_target.startswith("/"):
        return (repo_root / clean_target.lstrip("/")).resolve()
    return (source_file.parent / clean_target).resolve()


def to_repo_relative(path: Path) -> str | None:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return None


def main() -> None:
    output = subprocess.check_output(
        ["git", "-C", str(repo_root), "ls-files", "*.md"],
        text=True,
    )
    all_md_files = sorted(
        path.strip()
        for path in output.splitlines()
        if path.strip() and (repo_root / path.strip()).is_file()
    )

    audited_root_docs = {
        "AGENTS.md",
        "ALFRED_WORKFLOW_DEVELOPMENT.md",
        "BINARY_DEPENDENCIES.md",
        "DEVELOPMENT.md",
        "README.md",
        "THIRD_PARTY_LICENSES.md",
        "THIRD_PARTY_NOTICES.md",
        "TROUBLESHOOTING.md",
    }

    def should_audit(path: str) -> bool:
        if path in audited_root_docs:
            return True
        if path.startswith("docs/"):
            return True
        if re.match(r"^crates/[^/]+/README\.md$", path):
            return True
        if re.match(r"^crates/[^/]+/docs/.*\.md$", path):
            return True
        if re.match(r"^workflows/[^/]+/[^/]+\.md$", path):
            return True
        return False

    md_files = [path for path in all_md_files if should_audit(path)]
    findings: list[tuple[str, str, str]] = []

    for rel_path in md_files:
        source_file = repo_root / rel_path
        try:
            lines = source_file.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            lines = source_file.read_text(errors="ignore").splitlines()

        for line_no, line in enumerate(lines, start=1):
            for match in link_pattern.finditer(line):
                target = parse_target(match.group(1))
                resolved = resolve_local_target(source_file, target)
                if resolved is None:
                    continue

                rel_target = to_repo_relative(resolved)
                if rel_target is None:
                    continue

                if not resolved.exists():
                    findings.append((f"{rel_path}:{line_no}", target, rel_target))

    for finding in sorted(set(findings)):
        print("\t".join(finding))


if __name__ == "__main__":
    main()
PY
}

linter_failed=0
echo "+ ${lint_cmd[*]}"
if ! "${lint_cmd[@]}"; then
  linter_failed=1
fi

broken_local_links=0
broken_link_findings="$(collect_broken_local_markdown_links)"
if [[ -n "$broken_link_findings" ]]; then
  while IFS=$'\t' read -r source_ref raw_target resolved_target; do
    [[ -n "$source_ref" ]] || continue
    broken_local_links=$((broken_local_links + 1))
    echo "FAIL [local-link] $source_ref -> $raw_target (missing: $resolved_target)" >&2
  done <<<"$broken_link_findings"
  echo "hint: update markdown links to existing local paths or remove stale references." >&2
else
  echo "PASS [local-link] local markdown references resolved"
fi

if ((strict == 1 && (linter_failed != 0 || broken_local_links != 0))); then
  echo "FAIL: markdown lint audit (strict=$strict, files=${#md_files[@]}, markdownlint_failed=$linter_failed, broken_local_refs=$broken_local_links)" >&2
  exit 1
fi

if ((linter_failed != 0 || broken_local_links != 0)); then
  echo "WARN: markdown lint audit found issues (strict=$strict, markdownlint_failed=$linter_failed, broken_local_refs=$broken_local_links)" >&2
  echo "PASS: markdown lint audit (warning mode)"
  exit 0
fi

echo "PASS: markdown lint audit (strict=$strict, files=${#md_files[@]}, broken_local_refs=$broken_local_links)"

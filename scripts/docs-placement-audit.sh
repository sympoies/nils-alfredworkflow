#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

strict_warnings=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/docs-placement-audit.sh [--strict]

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

repo_warn() {
  local message="$1"
  printf 'WARN [repo] %s\n' "$message"
  warnings=$((warnings + 1))
}

repo_fail() {
  local message="$1"
  printf 'FAIL [repo] %s\n' "$message"
  hard_failures=$((hard_failures + 1))
}

crate_pass() {
  local crate="$1"
  local message="$2"
  printf 'PASS [%s] %s\n' "$crate" "$message"
}

crate_fail() {
  local crate="$1"
  local message="$2"
  printf 'FAIL [%s] %s\n' "$crate" "$message"
  hard_failures=$((hard_failures + 1))
}

collect_docs_freshness_findings() {
  python3 - "$repo_root" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

repo_root = Path(sys.argv[1]).resolve()
link_pattern = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
stale_root_doc_pattern = re.compile(
    r"^docs/[^/]*(?:contract|expression-rules|port-parity)[^/]*\.md$",
    re.IGNORECASE,
)


def list_markdown_files() -> list[str]:
    output = subprocess.check_output(
        ["git", "-C", str(repo_root), "ls-files", "*.md"],
        text=True,
    )
    return sorted(path.strip() for path in output.splitlines() if path.strip())


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


def collect_links_by_file(md_files: list[str]) -> dict[str, set[str]]:
    links_by_file: dict[str, set[str]] = {}
    for rel_path in md_files:
        source_file = repo_root / rel_path
        try:
            lines = source_file.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            lines = source_file.read_text(errors="ignore").splitlines()

        resolved_targets: set[str] = set()
        for line in lines:
            for match in link_pattern.finditer(line):
                target = parse_target(match.group(1))
                resolved = resolve_local_target(source_file, target)
                if resolved is None:
                    continue
                rel_target = to_repo_relative(resolved)
                if rel_target is not None:
                    resolved_targets.add(rel_target)
        links_by_file[rel_path] = resolved_targets
    return links_by_file


def main() -> None:
    md_files = list_markdown_files()
    md_set = set(md_files)
    links_by_file = collect_links_by_file(md_files)

    findings: list[tuple[str, str, str]] = []

    allowed_root_docs = {"ARCHITECTURE.md", "RELEASE.md"}
    allowed_docs_categories = {"plans", "reports", "specs"}
    for rel_path in (path for path in md_files if path.startswith("docs/")):
        parts = rel_path.split("/")
        if len(parts) == 2 and parts[1] in allowed_root_docs:
            continue
        if len(parts) == 3 and parts[1] in allowed_docs_categories:
            continue
        findings.append(("orphan_root", rel_path, "docs-ownership"))

    for rel_path in md_files:
        match = re.match(r"^crates/([^/]+)/docs/[^/]+\.md$", rel_path)
        if not match or rel_path.endswith("/docs/README.md"):
            continue
        crate_name = match.group(1)
        crate_readme = f"crates/{crate_name}/README.md"
        docs_readme = f"crates/{crate_name}/docs/README.md"
        references = set()
        references.update(links_by_file.get(crate_readme, set()))
        references.update(links_by_file.get(docs_readme, set()))
        if rel_path not in references:
            findings.append(("orphan_crate", rel_path, crate_name))

    for rel_path in md_files:
        match = re.match(r"^workflows/([^/]+)/[^/]+\.md$", rel_path)
        if not match or rel_path.endswith("/README.md"):
            continue
        workflow_id = match.group(1)
        workflow_readme = f"workflows/{workflow_id}/README.md"
        if workflow_readme not in md_set:
            findings.append(("orphan_workflow", rel_path, workflow_id))
            continue
        if rel_path not in links_by_file.get(workflow_readme, set()):
            findings.append(("orphan_workflow", rel_path, workflow_id))

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
                if stale_root_doc_pattern.match(rel_target):
                    findings.append(("stale_link", f"{rel_path}:{line_no}", rel_target))

    for finding in sorted(set(findings)):
        print("\t".join(finding))


if __name__ == "__main__":
    main()
PY
}

package_name_from_cargo() {
  local cargo_toml="$1"

  awk '
    /^\[package\][[:space:]]*$/ { in_package=1; next }
    in_package && /^\[/ { in_package=0 }
    in_package && /^[[:space:]]*name[[:space:]]*=/ {
      line=$0
      sub(/^[[:space:]]*name[[:space:]]*=[[:space:]]*"/, "", line)
      sub(/".*$/, "", line)
      print line
      exit
    }
  ' "$cargo_toml"
}

is_crate_specific_root_doc() {
  local filename="$1"

  case "$filename" in
  *contract*.md | *expression-rules*.md | *port-parity*.md)
    return 0
    ;;
  *)
    return 1
    ;;
  esac
}

echo "== Docs placement audit =="

publish_order_file="$repo_root/release/crates-io-publish-order.txt"
if [[ -f "$publish_order_file" ]]; then
  repo_pass "publish order file present: release/crates-io-publish-order.txt"
else
  repo_fail "missing release/crates-io-publish-order.txt"
fi

echo
echo "== Publishable crate required docs =="

declare -A package_to_crate_dir=()
mapfile -t cargo_tomls < <(find "$repo_root/crates" -mindepth 2 -maxdepth 2 -type f -name 'Cargo.toml' | sort)

if [[ ${#cargo_tomls[@]} -eq 0 ]]; then
  repo_fail "no crates/*/Cargo.toml found"
fi

for cargo_toml in "${cargo_tomls[@]}"; do
  package_name="$(package_name_from_cargo "$cargo_toml")"
  crate_dir="$(dirname "$cargo_toml")"

  if [[ -z "$package_name" ]]; then
    repo_fail "unable to resolve [package].name from ${cargo_toml#"$repo_root"/}"
    continue
  fi

  if [[ -n "${package_to_crate_dir[$package_name]:-}" ]]; then
    repo_fail "duplicate package.name '$package_name' across crates/"
    continue
  fi

  package_to_crate_dir["$package_name"]="$crate_dir"
done

publishable_packages=()
if [[ -f "$publish_order_file" ]]; then
  mapfile -t publishable_packages < <(awk '/^[[:space:]]*#/ { next } /^[[:space:]]*$/ { next } { print $1 }' "$publish_order_file")
fi

if [[ ${#publishable_packages[@]} -eq 0 ]]; then
  repo_fail "publish order is empty: release/crates-io-publish-order.txt"
fi

declare -A seen_publishable=()
for package_name in "${publishable_packages[@]}"; do
  if [[ -n "${seen_publishable[$package_name]:-}" ]]; then
    repo_fail "duplicate package in publish order: $package_name"
    continue
  fi
  seen_publishable["$package_name"]=1

  crate_dir="${package_to_crate_dir[$package_name]:-}"
  if [[ -z "$crate_dir" ]]; then
    crate_fail "$package_name" "missing crate directory for publishable package (check Cargo.toml name)"
    continue
  fi

  readme_path="$crate_dir/README.md"
  docs_index_path="$crate_dir/docs/README.md"

  if [[ -f "$readme_path" ]]; then
    crate_pass "$package_name" "required doc present: ${readme_path#"$repo_root"/}"
  else
    crate_fail "$package_name" "required doc missing: ${readme_path#"$repo_root"/}"
  fi

  if [[ -f "$docs_index_path" ]]; then
    crate_pass "$package_name" "required doc present: ${docs_index_path#"$repo_root"/}"
  else
    crate_fail "$package_name" "required doc missing: ${docs_index_path#"$repo_root"/}"
  fi
done

echo
echo "== Root docs placement =="

mapfile -t root_doc_paths < <(find "$repo_root/docs" -mindepth 1 -maxdepth 1 -type f -name '*.md' | sort)
crate_specific_root_detected=0

for root_doc_path in "${root_doc_paths[@]}"; do
  filename="$(basename "$root_doc_path")"
  rel_path="${root_doc_path#"$repo_root"/}"

  if ! is_crate_specific_root_doc "$filename"; then
    continue
  fi

  crate_specific_root_detected=1
  repo_fail "crate-specific root docs file is not allowed: $rel_path (move under crates/<crate>/docs/)"
done

if [[ $crate_specific_root_detected -eq 0 ]]; then
  repo_pass "no crate-specific root docs detected"
fi

echo
echo "== Docs freshness and reference drift =="

docs_freshness_findings="$(collect_docs_freshness_findings)"
orphan_docs_detected=0
stale_reference_detected=0

if [[ -n "$docs_freshness_findings" ]]; then
  while IFS=$'\t' read -r finding_type finding_arg1 finding_arg2; do
    [[ -n "$finding_type" ]] || continue
    case "$finding_type" in
    orphan_root)
      orphan_docs_detected=1
      repo_fail "orphan docs file path is outside canonical ownership paths: $finding_arg1 (allowed: docs/ARCHITECTURE.md, docs/RELEASE.md, docs/{plans,reports,specs}/*.md)"
      ;;
    orphan_crate)
      orphan_docs_detected=1
      crate_fail "$finding_arg2" "orphan crate docs file is not linked from crate entry docs: $finding_arg1 (link it from crates/$finding_arg2/README.md or crates/$finding_arg2/docs/README.md)"
      ;;
    orphan_workflow)
      orphan_docs_detected=1
      repo_fail "orphan workflow docs file is not linked from workflow README: $finding_arg1 (link it from workflows/$finding_arg2/README.md)"
      ;;
    stale_link)
      stale_reference_detected=1
      repo_fail "stale-to-canonical docs reference detected at $finding_arg1 -> $finding_arg2 (use canonical crates/<crate>/docs/... or docs/specs/... paths)"
      ;;
    *)
      repo_fail "unknown docs freshness finding type '$finding_type'"
      ;;
    esac
  done <<<"$docs_freshness_findings"
fi

if [[ $orphan_docs_detected -eq 0 ]]; then
  repo_pass "no orphan docs detected in enforced ownership paths"
fi

if [[ $stale_reference_detected -eq 0 ]]; then
  repo_pass "no stale-to-canonical docs reference drift detected"
fi

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

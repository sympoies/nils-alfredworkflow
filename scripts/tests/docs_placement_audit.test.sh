#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
audit_script="$repo_root/scripts/docs-placement-audit.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/docs-placement-audit.test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fail() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "$label (missing '$needle')"
  fi
}

create_fixture_repo() {
  local fixture_repo="$1"
  mkdir -p \
    "$fixture_repo/scripts" \
    "$fixture_repo/release" \
    "$fixture_repo/crates/demo/docs" \
    "$fixture_repo/docs"

  cp "$audit_script" "$fixture_repo/scripts/docs-placement-audit.sh"
  chmod +x "$fixture_repo/scripts/docs-placement-audit.sh"

  cat >"$fixture_repo/release/crates-io-publish-order.txt" <<'EOF'
nils-demo-cli
EOF

  cat >"$fixture_repo/crates/demo/Cargo.toml" <<'EOF'
[package]
name = "nils-demo-cli"
version = "0.1.0"
edition = "2021"
EOF

  cat >"$fixture_repo/crates/demo/README.md" <<'EOF'
# Demo Crate

- [Docs index](docs/README.md)
EOF

  cat >"$fixture_repo/crates/demo/docs/README.md" <<'EOF'
# Demo Docs
EOF

  cat >"$fixture_repo/README.md" <<'EOF'
# Fixture Repo

- [Development](DEVELOPMENT.md)
- [Alfred Workflow Development](ALFRED_WORKFLOW_DEVELOPMENT.md)
- [Binary Dependencies](BINARY_DEPENDENCIES.md)
- [Packaging](docs/PACKAGING.md)
- [Troubleshooting](TROUBLESHOOTING.md)
EOF

  cat >"$fixture_repo/DEVELOPMENT.md" <<'EOF'
# Development
EOF

  cat >"$fixture_repo/ALFRED_WORKFLOW_DEVELOPMENT.md" <<'EOF'
# Alfred Workflow Development
EOF

  cat >"$fixture_repo/BINARY_DEPENDENCIES.md" <<'EOF'
# Binary Dependencies
EOF

  cat >"$fixture_repo/docs/PACKAGING.md" <<'EOF'
# Packaging
EOF

  cat >"$fixture_repo/TROUBLESHOOTING.md" <<'EOF'
# Troubleshooting
EOF

  cat >"$fixture_repo/THIRD_PARTY_LICENSES.md" <<'EOF'
# Third Party Licenses
EOF

  cat >"$fixture_repo/THIRD_PARTY_NOTICES.md" <<'EOF'
# Third Party Notices
EOF

  cat >"$fixture_repo/docs/RELEASE.md" <<'EOF'
# Release

- [THIRD_PARTY_LICENSES.md](../THIRD_PARTY_LICENSES.md)
- [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)
EOF

  cat >"$fixture_repo/docs/ARCHITECTURE.md" <<'EOF'
# Architecture
EOF

  (
    cd "$fixture_repo"
    git init -q
    git config user.name "Docs Audit Test"
    git config user.email "docs-audit@example.com"
    git add .
  )
}

run_audit() {
  local fixture_repo="$1"
  (
    cd "$fixture_repo"
    bash scripts/docs-placement-audit.sh --strict
  )
}

test_repo_root_allowlist_accepts_governed_docs() {
  local fixture_repo="$test_root/pass-fixture"
  create_fixture_repo "$fixture_repo"

  local output
  output="$(run_audit "$fixture_repo")"
  assert_contains "$output" "PASS [repo] no unexpected repository-root markdown files detected" "root allowlist pass message"
  assert_contains "$output" "PASS [repo] all governed repository-root markdown files are linked from canonical entry docs" "root link pass message"
}

test_unexpected_repo_root_markdown_fails() {
  local fixture_repo="$test_root/unexpected-root-fixture"
  create_fixture_repo "$fixture_repo"

  cat >"$fixture_repo/NOTES.md" <<'EOF'
# Notes
EOF

  (
    cd "$fixture_repo"
    git add NOTES.md
  )

  local output=""
  set +e
  output="$(run_audit "$fixture_repo" 2>&1)"
  local rc=$?
  set -e

  [[ "$rc" -ne 0 ]] || fail "unexpected repo-root markdown should fail audit"
  assert_contains "$output" "repository-root markdown file is outside canonical ownership paths: NOTES.md" "unexpected root failure message"
}

test_missing_root_doc_entry_link_fails() {
  local fixture_repo="$test_root/missing-link-fixture"
  create_fixture_repo "$fixture_repo"

  cat >"$fixture_repo/README.md" <<'EOF'
# Fixture Repo

- [Development](DEVELOPMENT.md)
- [Alfred Workflow Development](ALFRED_WORKFLOW_DEVELOPMENT.md)
- [Binary Dependencies](BINARY_DEPENDENCIES.md)
- [Packaging](docs/PACKAGING.md)
EOF

  (
    cd "$fixture_repo"
    git add README.md
  )

  local output=""
  set +e
  output="$(run_audit "$fixture_repo" 2>&1)"
  local rc=$?
  set -e

  [[ "$rc" -ne 0 ]] || fail "missing README link for root troubleshooting doc should fail audit"
  assert_contains "$output" "repository-root markdown file is not linked from its canonical entry doc(s): TROUBLESHOOTING.md" "missing root link failure message"
}

main() {
  test_repo_root_allowlist_accepts_governed_docs
  test_unexpected_repo_root_markdown_fails
  test_missing_root_doc_entry_link_fails
  printf 'ok: docs placement audit tests passed\n'
}

main "$@"

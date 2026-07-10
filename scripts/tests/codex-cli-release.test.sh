#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf "$test_root"' EXIT

fixture="$test_root/release"
tree="$test_root/tree"
fake_bin="$test_root/fake-bin"
cargo_log="$test_root/cargo.log"
target="test-unknown-linux-gnu"
version="1.21.6"
archive="nils-cli-v${version}-${target}.tar.gz"
mkdir -p "$fixture" "$tree/bin" "$fake_bin"

cat >"$tree/bin/codex-cli" <<'SH'
#!/usr/bin/env bash
if [[ "${1:-}" == --version ]]; then
  echo "codex-cli 1.21.6"
  exit 0
fi
exit 0
SH
chmod +x "$tree/bin/codex-cli"
tar -czf "$fixture/$archive" -C "$tree" bin/codex-cli
if command -v shasum >/dev/null 2>&1; then
  fixture_sha="$(shasum -a 256 "$fixture/$archive" | awk '{print $1}')"
else
  fixture_sha="$(sha256sum "$fixture/$archive" | awk '{print $1}')"
fi
printf '%s  %s\n' "$fixture_sha" "$archive" >"$fixture/$archive.sha256"

cat >"$fake_bin/cargo" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${CARGO_STUB_LOG:?}"
if [[ "${1:-}" == install ]]; then
  exit 88
fi
exit 89
SH
chmod +x "$fake_bin/cargo"
: >"$cargo_log"

HOME="$test_root/home" CARGO_HOME="$test_root/cargo-home" CARGO_STUB_LOG="$cargo_log" \
  PATH="$fake_bin:$PATH" CODEX_CLI_RELEASE_TEST_MODE=1 \
  CODEX_CLI_RELEASE_BASE_URL="file://$fixture" CODEX_CLI_RELEASE_TARGET="$target" \
  CODEX_CLI_RELEASE_TEST_SHA256="$fixture_sha" \
  bash "$repo_root/scripts/ci/ci-bootstrap.sh" --context ci --install-codex-cli >/dev/null

[[ -x "$test_root/cargo-home/bin/codex-cli" ]] || {
  echo "error: ci-bootstrap did not install codex-cli from the release fixture" >&2
  exit 1
}
[[ "$("$test_root/cargo-home/bin/codex-cli" --version)" == "codex-cli 1.21.6" ]] || {
  echo "error: installed codex-cli version mismatch" >&2
  exit 1
}
if grep -Fq 'install nils-codex-cli' "$cargo_log"; then
  echo "error: ci-bootstrap invoked the retired crates.io install" >&2
  exit 1
fi

echo "ok: codex-cli release bootstrap tests passed"

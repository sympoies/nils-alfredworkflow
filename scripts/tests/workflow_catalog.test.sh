#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

# shellcheck disable=SC1091
source "$repo_root/scripts/lib/workflow_catalog.sh"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/workflow-catalog.test.XXXXXX")"
fake_bin_dir="$test_root/bin"
trap 'rm -rf "$test_root"' EXIT

mkdir -p "$fake_bin_dir"

: >"$fake_bin_dir/plutil"
cat >"$fake_bin_dir/plutil" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

cat "${!#}"
EOF
chmod +x "$fake_bin_dir/plutil"

fail() {
  local message="$1"
  printf 'FAIL: %s\n' "$message" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [[ "$actual" != "$expected" ]]; then
    fail "$label (expected='$expected', actual='$actual')"
  fi
}

test_toml_string_parses_expected_key() {
  local toml_file="$test_root/workflow.toml"
  cat >"$toml_file" <<'TOML'
id = "weather"
name = "Weather Forecast"
bundle_id = "com.sympoies.weather"
TOML

  assert_eq "Weather Forecast" "$(wfc_toml_string "$toml_file" name)" "toml name parse"
  assert_eq "com.sympoies.weather" "$(wfc_toml_string "$toml_file" bundle_id)" "toml bundle_id parse"
}

test_list_workflow_ids_skips_template_and_sorts() {
  local fixture_root="$test_root/list-fixture"
  mkdir -p \
    "$fixture_root/workflows/_template" \
    "$fixture_root/workflows/weather" \
    "$fixture_root/workflows/bangumi-search"

  local listed
  listed="$(wfc_list_workflow_ids "$fixture_root" | tr '\n' ' ' | sed -E 's/[[:space:]]+$//')"
  assert_eq "bangumi-search weather" "$listed" "workflow listing order"
}

test_manifest_path_and_bundle_lookup() {
  local fixture_root="$test_root/manifest-fixture"
  local manifest
  mkdir -p "$fixture_root/workflows/weather"
  manifest="$fixture_root/workflows/weather/workflow.toml"

  cat >"$manifest" <<'TOML'
id = "weather"
bundle_id = "com.sympoies.weather"
TOML

  assert_eq \
    "$manifest" \
    "$(wfc_manifest_path "$fixture_root" weather)" \
    "workflow manifest path"
  assert_eq \
    "com.sympoies.weather" \
    "$(wfc_bundle_id_for_workflow_id "$fixture_root" weather)" \
    "bundle id lookup"
}

test_find_installed_workflow_dir_by_bundle_id() {
  local fixture_root="$test_root/prefs-fixture"
  local prefs_root="$fixture_root/prefs"
  local resolved=""
  mkdir -p \
    "$prefs_root/user.workflow.a" \
    "$prefs_root/user.workflow.b"

  printf '%s\n' "com.sympoies.weather" >"$prefs_root/user.workflow.a/info.plist"
  printf '%s\n' "com.sympoies.open-project" >"$prefs_root/user.workflow.b/info.plist"

  resolved="$(PATH="$fake_bin_dir:$PATH" \
    wfc_find_installed_workflow_dir_by_bundle_id "$prefs_root" "com.sympoies.open-project")"
  assert_eq \
    "$prefs_root/user.workflow.b" \
    "$resolved" \
    "installed workflow dir lookup"

  set +e
  PATH="$fake_bin_dir:$PATH" \
    wfc_find_installed_workflow_dir_by_bundle_id "$prefs_root" "com.sympoies.missing" >/dev/null 2>&1
  local rc=$?
  set -e

  assert_eq "1" "$rc" "missing installed workflow exit code"
}

test_dist_latest_artifact_resolves_latest_path() {
  local fixture_root="$test_root/dist-fixture"
  mkdir -p \
    "$fixture_root/dist/weather/1.0.0" \
    "$fixture_root/dist/weather/1.1.0"

  touch "$fixture_root/dist/weather/1.0.0/Weather Forecast.alfredworkflow"
  touch "$fixture_root/dist/weather/1.1.0/Weather Forecast.alfredworkflow"

  local resolved
  resolved="$(wfc_dist_latest_artifact "$fixture_root" weather)"
  assert_eq \
    "$fixture_root/dist/weather/1.1.0/Weather Forecast.alfredworkflow" \
    "$resolved" \
    "latest dist artifact path"
}

test_dist_latest_artifact_fails_when_missing() {
  local fixture_root="$test_root/missing-dist-fixture"
  mkdir -p "$fixture_root/dist"

  set +e
  wfc_dist_latest_artifact "$fixture_root" weather >/dev/null 2>&1
  local rc=$?
  set -e

  assert_eq "1" "$rc" "missing dist artifact exit code"
}

main() {
  test_toml_string_parses_expected_key
  test_list_workflow_ids_skips_template_and_sorts
  test_manifest_path_and_bundle_lookup
  test_find_installed_workflow_dir_by_bundle_id
  test_dist_latest_artifact_resolves_latest_path
  test_dist_latest_artifact_fails_when_missing
  printf 'ok: workflow_catalog tests passed\n'
}

main "$@"

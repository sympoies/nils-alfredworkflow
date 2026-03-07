#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workflow_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$workflow_dir/../.." && pwd)"

smoke_helper="$repo_root/scripts/lib/workflow_smoke_helpers.sh"

if [[ ! -f "$smoke_helper" ]]; then
  echo "missing required helper: $smoke_helper" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$smoke_helper"

for required in \
  workflow.toml \
  README.md \
  TROUBLESHOOTING.md \
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg
require_bin python3

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "imdb-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in IMDB_SEARCH_SECTION IMDB_MAX_RESULTS; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

tmp_dir="$(mktemp -d)"
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"

cleanup() {
  artifact_restore_file "$artifact_path" "$artifact_backup"
  artifact_restore_file "$artifact_sha_path" "$artifact_sha_backup"
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin"
workflow_smoke_write_open_stub "$tmp_dir/bin/open"

cat >"$tmp_dir/suggest-ok.json" <<'EOF_JSON'
{
  "d": [
    {
      "id": "tt0133093",
      "l": "The Matrix",
      "q": "Movie",
      "y": 1999,
      "s": "Keanu Reeves"
    },
    {
      "id": "nm0000206",
      "l": "Keanu Reeves",
      "q": "Actor",
      "s": "John Wick, The Matrix"
    }
  ]
}
EOF_JSON

cat >"$tmp_dir/suggest-invalid.json" <<'EOF_BAD'
{"broken":
EOF_BAD

workflow_smoke_assert_action_requires_arg "$workflow_dir/scripts/action_open.sh"

action_arg="https://www.imdb.com/title/tt0133093/"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

empty_query_json="$({ "$workflow_dir/scripts/script_filter.sh" "   "; })"
assert_jq_json "$empty_query_json" '.items[0].title == "Enter a title keyword"' "empty query guidance title mismatch"
assert_jq_json "$empty_query_json" '.items[0].valid == false' "empty query item must be invalid"

short_query_json="$({ "$workflow_dir/scripts/script_filter.sh" "x"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guidance title mismatch"
assert_jq_json "$short_query_json" '.items[0].valid == false' "short query item must be invalid"

normal_json="$({ IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" "$workflow_dir/scripts/script_filter.sh" "matrix"; })"
assert_jq_json "$normal_json" '.items | length == 3' "normal query should output two suggestions + fallback"
assert_jq_json "$normal_json" '.items[0].title == "The Matrix"' "first suggestion title mismatch"
assert_jq_json "$normal_json" '.items[0].arg == "https://www.imdb.com/title/tt0133093/"' "title suggestion URL mismatch"
assert_jq_json "$normal_json" '.items[1].arg == "https://www.imdb.com/name/nm0000206/"' "name suggestion URL mismatch"
assert_jq_json "$normal_json" '.items[2].title == "Search IMDb: matrix"' "fallback row title mismatch"
assert_jq_json "$normal_json" '.items[2].arg == "https://www.imdb.com/find/?q=matrix&s=tt&ref_=fn_tt"' "fallback search URL mismatch"

max_results_json="$({ IMDB_MAX_RESULTS=1 IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" "$workflow_dir/scripts/script_filter.sh" "matrix"; })"
assert_jq_json "$max_results_json" '.items | length == 2' "IMDB_MAX_RESULTS should limit suggestion rows"
assert_jq_json "$max_results_json" '.items[0].title == "The Matrix"' "IMDB_MAX_RESULTS first row mismatch"
assert_jq_json "$max_results_json" '.items[1].title == "Search IMDb: matrix"' "IMDB_MAX_RESULTS fallback row missing"

env_query_json="$({ IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" alfred_workflow_query="neo" "$workflow_dir/scripts/script_filter.sh"; })"
assert_jq_json "$env_query_json" '.items[-1].title == "Search IMDb: neo"' "env query fallback mismatch"

stdin_query_json="$(printf 'morpheus' | IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" "$workflow_dir/scripts/script_filter.sh")"
assert_jq_json "$stdin_query_json" '.items[-1].title == "Search IMDb: morpheus"' "stdin query fallback mismatch"

section_json="$({ IMDB_SEARCH_SECTION=nm IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" "$workflow_dir/scripts/script_filter.sh" "keanu"; })"
assert_jq_json "$section_json" '.items[-1].arg == "https://www.imdb.com/find/?q=keanu&s=nm&ref_=fn_nm"' "section override fallback URL mismatch"

invalid_section_json="$({ IMDB_SEARCH_SECTION="??" IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-ok.json" "$workflow_dir/scripts/script_filter.sh" "alien"; })"
assert_jq_json "$invalid_section_json" '.items[-1].arg == "https://www.imdb.com/find/?q=alien&s=tt&ref_=fn_tt"' "invalid section should fallback to tt"

invalid_payload_json="$({ IMDB_SUGGEST_PAYLOAD_FILE="$tmp_dir/suggest-invalid.json" "$workflow_dir/scripts/script_filter.sh" "matrix"; })"
assert_jq_json "$invalid_payload_json" '.items | length == 1' "invalid payload should fallback to one search row"
assert_jq_json "$invalid_payload_json" '.items[0].subtitle | contains("Suggestions parse failed")' "invalid payload fallback subtitle mismatch"

"$repo_root/scripts/workflow-pack.sh" --id imdb-search >/dev/null
[[ -f "$artifact_path" ]] || fail "packaging did not produce artifact"
[[ -f "$artifact_sha_path" ]] || fail "packaging did not produce checksum"

plist_json="$(plist_to_json "$repo_root/build/workflows/imdb-search/pkg/info.plist")"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.keyword == "im||imdb"' "keyword wiring mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.alfredfiltersresults == false' "alfredfiltersresults must remain false"
assert_jq_json "$plist_json" '[.userconfigurationconfig[].variable] | index("IMDB_MAX_RESULTS") != null' "plist must expose IMDB_MAX_RESULTS"

echo "ok: imdb-search smoke test passed"

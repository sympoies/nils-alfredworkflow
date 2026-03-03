#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workflow_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$workflow_dir/../.." && pwd)"

fail() {
  echo "error: $*" >&2
  exit 1
}

require_bin() {
  local binary="$1"
  command -v "$binary" >/dev/null 2>&1 || fail "missing required binary: $binary"
}

require_bin shellcheck
mapfile -t shellcheck_targets < <(find "$workflow_dir" -type f -name '*.sh' | sort)
if [[ "${#shellcheck_targets[@]}" -gt 0 ]]; then
  shellcheck -e SC1091 "${shellcheck_targets[@]}"
fi

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || fail "missing required file: $path"
}

assert_exec() {
  local path="$1"
  [[ -x "$path" ]] || fail "script must be executable: $path"
}

toml_string() {
  local file="$1"
  local key="$2"
  awk -F'=' -v key="$key" '
    $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      value=$2
      sub(/^[[:space:]]*/, "", value)
      sub(/[[:space:]]*$/, "", value)
      gsub(/^"|"$/, "", value)
      print value
      exit
    }
  ' "$file"
}

plist_to_json() {
  local plist_file="$1"
  if command -v plutil >/dev/null 2>&1; then
    plutil -convert json -o - "$plist_file"
    return
  fi

  python3 - "$plist_file" <<'PY'
import json
import plistlib
import sys

with open(sys.argv[1], 'rb') as f:
    payload = plistlib.load(f)
print(json.dumps(payload))
PY
}

assert_jq_json() {
  local json_payload="$1"
  local filter="$2"
  local message="$3"
  if ! jq -e "$filter" >/dev/null <<<"$json_payload"; then
    fail "$message (jq: $filter)"
  fi
}

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

artifact_backup=""
if [[ -f "$artifact_path" ]]; then
  artifact_backup="$tmp_dir/$(basename "$artifact_path").backup"
  cp "$artifact_path" "$artifact_backup"
fi

artifact_sha_backup=""
if [[ -f "$artifact_sha_path" ]]; then
  artifact_sha_backup="$tmp_dir/$(basename "$artifact_sha_path").backup"
  cp "$artifact_sha_path" "$artifact_sha_backup"
fi

cleanup() {
  if [[ -n "$artifact_backup" && -f "$artifact_backup" ]]; then
    mkdir -p "$(dirname "$artifact_path")"
    cp "$artifact_backup" "$artifact_path"
  else
    rm -f "$artifact_path"
  fi

  if [[ -n "$artifact_sha_backup" && -f "$artifact_sha_backup" ]]; then
    mkdir -p "$(dirname "$artifact_sha_path")"
    cp "$artifact_sha_backup" "$artifact_sha_path"
  else
    rm -f "$artifact_sha_path"
  fi

  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin"

cat >"$tmp_dir/bin/open" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$OPEN_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/open"

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

set +e
"$workflow_dir/scripts/action_open.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_open.sh without args must exit 2"

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

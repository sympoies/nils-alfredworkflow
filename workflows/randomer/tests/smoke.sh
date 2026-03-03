#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workflow_dir="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$workflow_dir/../.." && pwd)"

PRIMARY_UID="70EEA820-E77B-42F3-A8D2-1A4D9E8E4A10"
TYPE_UID="C2F5E113-7D3B-49CC-8F95-5E8B0A9BB5C1"
VALUES_UID="A5F5D9EC-6344-47B3-9A0A-5F7DFB0D5132"
ACTION_UID="D7E624DB-D4AB-4D53-8C03-D051A1A97A4A"

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

assert_jq_file() {
  local file="$1"
  local filter="$2"
  local message="$3"
  if ! jq -e "$filter" "$file" >/dev/null; then
    fail "$message (jq: $filter)"
  fi
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
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter.sh \
  scripts/script_filter_types.sh \
  scripts/script_filter_expand.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

for format_icon in email imei unit uuid int decimal percent currency hex otp phone; do
  assert_file "$workflow_dir/src/assets/icons/${format_icon}.png"
done

for executable in \
  scripts/script_filter.sh \
  scripts/script_filter_types.sh \
  scripts/script_filter_expand.sh \
  scripts/action_open.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "randomer" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "randomer-cli" ]] || fail "rust_binary must be randomer-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

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

release_cli="$repo_root/target/release/randomer-cli"
release_backup=""
if [[ -f "$release_cli" ]]; then
  release_backup="$tmp_dir/randomer-cli.release.backup"
  cp "$release_cli" "$release_backup"
fi

cleanup() {
  if [[ -n "$release_backup" && -f "$release_backup" ]]; then
    mkdir -p "$(dirname "$release_cli")"
    cp "$release_backup" "$release_cli"
  elif [[ -f "$release_cli" ]]; then
    rm -f "$release_cli"
  fi

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

mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"

cat >"$tmp_dir/bin/pbcopy" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
cat >"$PBCOPY_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/pbcopy"

set +e
"$workflow_dir/scripts/action_open.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_open.sh without args must exit 2"

action_arg='550e8400-e29b-41d4-a716-446655440000'
printf '%s' "$action_arg" >"$tmp_dir/expected-copy.txt"
PBCOPY_STUB_OUT="$tmp_dir/copied.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
if ! cmp -s "$tmp_dir/expected-copy.txt" "$tmp_dir/copied.txt"; then
  fail "action_open.sh must copy exact argument bytes through pbcopy"
fi

cat >"$tmp_dir/stubs/randomer-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

command_name="${1:-}"
case "$command_name" in
list-formats)
  [[ "${2:-}" == "--query" ]] || exit 9
  query="${3:-}"
  printf '{"items":[{"title":"uuid","subtitle":"sample from %s","arg":"550e8400-e29b-41d4-a716-446655440000","mods":{"cmd":{"arg":"uuid","subtitle":"Generate 10 values","variables":{"RANDOMER_FORMAT":"uuid"}}}}]}' "$query"
  printf '\n'
  ;;
list-types)
  [[ "${2:-}" == "--query" ]] || exit 9
  query="${3:-}"
  printf '{"items":[{"title":"int","subtitle":"sample: 42 · Enter: show 10 values (%s)","arg":"int","icon":{"path":"assets/icons/int.png"},"valid":true,"variables":{"RANDOMER_FORMAT":"int"}}]}' "$query"
  printf '\n'
  ;;
generate)
  [[ "${2:-}" == "--format" ]] || exit 9
  format="${3:-}"
  [[ "${4:-}" == "--count" ]] || exit 9
  count="${5:-}"
  [[ "$count" == "10" ]] || exit 9
  printf '{"items":['
  for ((i = 1; i <= 10; i++)); do
    printf '{"title":"%s-%02d","subtitle":"%s","arg":"%s-%02d","valid":true}' "$format" "$i" "$format" "$format" "$i"
    if [[ "$i" -lt 10 ]]; then
      printf ','
    fi
  done
  printf ']}\n'
  ;;
*)
  exit 9
  ;;
esac
EOS
chmod +x "$tmp_dir/stubs/randomer-cli-ok"

cat >"$tmp_dir/stubs/randomer-cli-fail" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "unexpected failure" >&2
exit 7
EOS
chmod +x "$tmp_dir/stubs/randomer-cli-fail"

primary_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" "$workflow_dir/scripts/script_filter.sh" "uuid"; })"
assert_jq_json "$primary_json" '.items | type == "array" and length == 1' "primary script output must be Alfred items array"
assert_jq_json "$primary_json" '.items[0].mods.cmd.arg == "uuid"' "primary script item must provide cmd modifier arg for expand"
assert_jq_json "$primary_json" '.items[0].mods.cmd.variables.RANDOMER_FORMAT == "uuid"' "primary cmd modifier must include RANDOMER_FORMAT variable"

types_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" "$workflow_dir/scripts/script_filter_types.sh" "in"; })"
assert_jq_json "$types_json" '.items | type == "array" and length == 1' "types script output must be Alfred items array"
assert_jq_json "$types_json" '.items[0].title == "int" and .items[0].arg == "int"' "types script must provide format key arg"
assert_jq_json "$types_json" '.items[0].variables.RANDOMER_FORMAT == "int"' "types script must provide RANDOMER_FORMAT variable"

expanded_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" "$workflow_dir/scripts/script_filter_expand.sh" "uuid"; })"
assert_jq_json "$expanded_json" '.items | type == "array" and length == 10' "expanded script must output 10 values"
assert_jq_json "$expanded_json" 'all(.items[]; .subtitle == "uuid")' "expanded script subtitles must match selected format key"

expanded_env_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" RANDOMER_FORMAT="uuid" "$workflow_dir/scripts/script_filter_expand.sh"; })"
assert_jq_json "$expanded_env_json" '.items | type == "array" and length == 10' "expanded script must support RANDOMER_FORMAT env fallback"
assert_jq_json "$expanded_env_json" 'all(.items[]; .subtitle == "uuid")' "RANDOMER_FORMAT fallback subtitles must match format key"

expanded_query_env_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" alfred_workflow_query="uuid" "$workflow_dir/scripts/script_filter_expand.sh"; })"
assert_jq_json "$expanded_query_env_json" '.items | type == "array" and length == 10' "expanded script must support alfred_workflow_query fallback"
assert_jq_json "$expanded_query_env_json" 'all(.items[]; .subtitle == "uuid")' "alfred_workflow_query fallback subtitles must match format key"

expanded_stdin_json="$({ printf 'uuid'; } | RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" "$workflow_dir/scripts/script_filter_expand.sh")"
assert_jq_json "$expanded_stdin_json" '.items | type == "array" and length == 10' "expanded script must support stdin format fallback"
assert_jq_json "$expanded_stdin_json" 'all(.items[]; .subtitle == "uuid")' "stdin format fallback subtitles must match format key"

empty_expand_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-ok" "$workflow_dir/scripts/script_filter_expand.sh" " "; })"
assert_jq_json "$empty_expand_json" '.items[0].title == "Select a format first"' "empty expand query should return guidance item"

primary_failure_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-fail" "$workflow_dir/scripts/script_filter.sh" "uuid"; })"
assert_jq_json "$primary_failure_json" '.items | type == "array" and length == 1' "primary failure fallback must output one item"
assert_jq_json "$primary_failure_json" '.items[0].valid == false' "primary failure fallback item must be invalid"

types_failure_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-fail" "$workflow_dir/scripts/script_filter_types.sh" "uuid"; })"
assert_jq_json "$types_failure_json" '.items | type == "array" and length == 1' "types failure fallback must output one item"
assert_jq_json "$types_failure_json" '.items[0].valid == false' "types failure fallback item must be invalid"

expand_failure_json="$({ RANDOMER_CLI_BIN="$tmp_dir/stubs/randomer-cli-fail" "$workflow_dir/scripts/script_filter_expand.sh" "uuid"; })"
assert_jq_json "$expand_failure_json" '.items | type == "array" and length == 1' "expand failure fallback must output one item"
assert_jq_json "$expand_failure_json" '.items[0].valid == false' "expand failure fallback item must be invalid"

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-randomer-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/randomer-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/randomer-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id randomer >/dev/null

packaged_dir="$repo_root/build/workflows/randomer/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/randomer-cli"
assert_file "$artifact_path"
assert_file "$artifact_sha_path"

for format_icon in email imei unit uuid int decimal percent currency hex otp phone; do
  assert_file "$packaged_dir/assets/icons/${format_icon}.png"
done

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length >= 4' "packaged plist missing object graph"
assert_jq_file "$packaged_json_file" '.connections | length >= 3' "packaged plist missing connections"

assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$PRIMARY_UID\") | .config.scriptfile == \"./scripts/script_filter.sh\"" "primary script filter scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$PRIMARY_UID\") | .config.scriptargtype == 1" "primary script filter must pass query via argv"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TYPE_UID\") | .config.scriptfile == \"./scripts/script_filter_types.sh\"" "type selector scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TYPE_UID\") | .config.keyword == \"rrv\"" "type selector keyword must be rrv"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TYPE_UID\") | .config.scriptargtype == 1" "type selector must pass query via argv"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$VALUES_UID\") | .config.scriptfile == \"./scripts/script_filter_expand.sh\"" "values script filter scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$VALUES_UID\") | .config.scriptargtype == 1" "values script filter must pass query via argv"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$VALUES_UID\") | .config.alfredfiltersresults == false" "values script filter must disable Alfred-side result filtering"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$ACTION_UID\") | .config.scriptfile == \"./scripts/action_open.sh\"" "copy action scriptfile mismatch"

assert_jq_file "$packaged_json_file" ".connections[\"$PRIMARY_UID\"] | any(.destinationuid == \"$ACTION_UID\" and .modifiers == 0)" "missing primary->copy enter connection"
assert_jq_file "$packaged_json_file" ".connections[\"$PRIMARY_UID\"] | any(.destinationuid == \"$VALUES_UID\" and .modifiers == 1048576)" "missing primary->values cmd-enter connection"
assert_jq_file "$packaged_json_file" ".connections[\"$TYPE_UID\"] | any(.destinationuid == \"$VALUES_UID\" and .modifiers == 0)" "missing type->values enter connection"
assert_jq_file "$packaged_json_file" ".connections[\"$VALUES_UID\"] | any(.destinationuid == \"$ACTION_UID\" and .modifiers == 0)" "missing values->copy enter connection"

echo "ok: randomer smoke test"

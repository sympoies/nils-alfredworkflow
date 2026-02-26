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

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "steam-search" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_open.sh" ]] || fail "action mismatch"

for variable in STEAM_REGION STEAM_REGION_OPTIONS STEAM_MAX_RESULTS; do
  if ! rg -n "^${variable}[[:space:]]*=" "$manifest" >/dev/null; then
    fail "missing env var in workflow.toml: $variable"
  fi
done

plist_json="$(plist_to_json "$workflow_dir/src/info.plist.template")"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.keyword == "st||steam"' "plist keyword wiring mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaycustom == 1' "queue delay custom mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelaymode == 0' "queue delay mode mismatch"
assert_jq_json "$plist_json" '.objects[] | select(.type == "alfred.workflow.input.scriptfilter") | .config.queuedelayimmediatelyinitially == false' "queue immediate policy mismatch"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"

cat >"$tmp_dir/bin/open" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$OPEN_STUB_OUT"
EOS
chmod +x "$tmp_dir/bin/open"

set +e
"$workflow_dir/scripts/action_open.sh" >/dev/null 2>&1
action_rc=$?
set -e
[[ "$action_rc" -eq 2 ]] || fail "action_open.sh without args must exit 2"

action_arg="https://store.steampowered.com/app/620/Portal_2/"
OPEN_STUB_OUT="$tmp_dir/open-arg.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_open.sh" "$action_arg"
[[ "$(cat "$tmp_dir/open-arg.txt")" == "$action_arg" ]] || fail "action_open.sh must pass URL to open"

cat >"$tmp_dir/stubs/requery" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$STEAM_REQUERY_OUT"
EOS
chmod +x "$tmp_dir/stubs/requery"

ALFRED_WORKFLOW_CACHE="$tmp_dir/cache" \
  STEAM_REQUERY_OUT="$tmp_dir/requery.txt" \
  STEAM_REQUERY_COMMAND="$tmp_dir/stubs/requery" \
  "$workflow_dir/scripts/action_open.sh" "steam-requery:us:helldivers"

[[ "$(cat "$tmp_dir/requery.txt")" == "st helldivers" ]] || fail "steam requery text mismatch"
[[ "$(sed -n '1p' "$tmp_dir/cache/steam-region-override.state")" == "US" ]] || fail "steam region override state mismatch"

cat >"$tmp_dir/stubs/steam-cli" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "search" ]] || exit 9
[[ "${2:-}" == "--query" ]] || exit 9
query="${3:-}"
printf '{"items":[{"title":"Steam stub","subtitle":"query=%s","arg":"https://store.steampowered.com/app/620/","valid":true}]}' "$query"
printf '\n'
EOS
chmod +x "$tmp_dir/stubs/steam-cli"

result_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli" "$workflow_dir/scripts/script_filter.sh" "portal"; })"
assert_jq_json "$result_json" '.items[0].title == "Steam stub"' "script_filter success pass-through mismatch"

short_query_json="$({ STEAM_CLI_BIN="$tmp_dir/stubs/steam-cli" "$workflow_dir/scripts/script_filter.sh" "p"; })"
assert_jq_json "$short_query_json" '.items[0].title == "Keep typing (2+ chars)"' "short query guard mismatch"

echo "ok: steam-search smoke test passed"

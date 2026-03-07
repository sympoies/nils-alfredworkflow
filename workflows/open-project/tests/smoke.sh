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
  scripts/script_filter.sh \
  scripts/script_filter_github.sh \
  scripts/action_open.sh \
  scripts/action_record_usage.sh \
  scripts/action_open_github.sh; do
  assert_file "$workflow_dir/$required"
done

for executable in \
  scripts/script_filter.sh \
  scripts/script_filter_github.sh \
  scripts/action_open.sh \
  scripts/action_record_usage.sh \
  scripts/action_open_github.sh; do
  assert_exec "$workflow_dir/$executable"
done

for required_bin in jq git; do
  require_bin "$required_bin"
done

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cargo build -p nils-workflow-cli >/dev/null

project_root="$tmp_dir/projects"
repo_path="$project_root/alpha-repo"
usage_file="$tmp_dir/usage.log"
mkdir -p "$project_root"
git init -q "$repo_path"
printf '%s | %s\n' "$repo_path" "2025-01-02 03:04:05" >"$usage_file"

script_filter_output="$({
  PROJECT_DIRS="$project_root" \
    USAGE_FILE="$usage_file" \
    WORKFLOW_CLI_BIN="$repo_root/target/debug/workflow-cli" \
    "$workflow_dir/scripts/script_filter.sh" ""
})"

echo "$script_filter_output" | jq -e '.items | length > 0' >/dev/null
echo "$script_filter_output" | jq -e '.items[0].title == "alpha-repo"' >/dev/null
echo "$script_filter_output" | jq -e '.items[0].arg == $path' --arg path "$repo_path" >/dev/null
echo "$script_filter_output" | jq -e '.items[0].mods.shift.icon.path == "assets/icon-github.png"' >/dev/null

resolver_home="$tmp_dir/home-resolver"
mkdir -p "$resolver_home/.local/bin"
ln -s "$repo_root/target/debug/workflow-cli" "$resolver_home/.local/bin/workflow-cli"
script_filter_tilde_bin_output="$({
  HOME="$resolver_home" \
    PROJECT_DIRS="$project_root" \
    USAGE_FILE="$usage_file" \
    WORKFLOW_CLI_BIN=\~/.local/bin/workflow-cli \
    "$workflow_dir/scripts/script_filter.sh" ""
})"
echo "$script_filter_tilde_bin_output" | jq -e '.items[0].title == "alpha-repo"' >/dev/null

github_filter_output="$({
  PROJECT_DIRS="$project_root" \
    USAGE_FILE="$usage_file" \
    WORKFLOW_CLI_BIN="$repo_root/target/debug/workflow-cli" \
    "$workflow_dir/scripts/script_filter_github.sh" ""
})"
echo "$github_filter_output" | jq -e '.items[0].icon.path == "assets/icon-github.png"' >/dev/null

env VSCODE_PATH=/usr/bin/true "$workflow_dir/scripts/action_open.sh" "$repo_path" >/dev/null
vscode_home="$tmp_dir/home-vscode"
mkdir -p "$vscode_home/.local/bin"
cat >"$vscode_home/.local/bin/code" <<'EOS'
#!/usr/bin/env bash
exit 0
EOS
chmod +x "$vscode_home/.local/bin/code"
env HOME="$vscode_home" VSCODE_PATH=\~/.local/bin/code \
  "$workflow_dir/scripts/action_open.sh" "$repo_path" >/dev/null
recorded_path="$("$workflow_dir/scripts/action_record_usage.sh" "$repo_path")"
[[ "$recorded_path" == "$repo_path" ]]

path_with_newline="${repo_path}"$'\n'
env VSCODE_PATH=/usr/bin/true "$workflow_dir/scripts/action_open.sh" "$path_with_newline" >/dev/null
recorded_trimmed="$("$workflow_dir/scripts/action_record_usage.sh" "$path_with_newline")"
[[ "$recorded_trimmed" == "$repo_path" ]]

"$repo_root/scripts/workflow-pack.sh" --id open-project >/dev/null
packaged_plist="$repo_root/build/workflows/open-project/pkg/info.plist"
if [[ ! -f "$packaged_plist" ]]; then
  echo "packaged plist not found: $packaged_plist" >&2
  exit 1
fi
if [[ ! -f "$repo_root/build/workflows/open-project/pkg/icon.png" ]]; then
  echo "packaged root icon not found: $repo_root/build/workflows/open-project/pkg/icon.png" >&2
  exit 1
fi
if [[ ! -f "$repo_root/build/workflows/open-project/pkg/8F3399E3-951A-4DC0-BC7D-CFA83C1E1F76.png" ]]; then
  echo "packaged github script filter icon not found: $repo_root/build/workflows/open-project/pkg/8F3399E3-951A-4DC0-BC7D-CFA83C1E1F76.png" >&2
  exit 1
fi
if [[ ! -f "$repo_root/build/workflows/open-project/pkg/assets/icon-github.png" ]]; then
  echo "packaged GitHub icon not found: $repo_root/build/workflows/open-project/pkg/assets/icon-github.png" >&2
  exit 1
fi

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null
fi

packaged_json="$(plist_to_json "$packaged_plist")"

echo "$packaged_json" | jq -e '.objects | length > 0' >/dev/null
echo "$packaged_json" | jq -e '.connections | length > 0' >/dev/null
echo "$packaged_json" | jq -e '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.type] | all(. == 8)' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="6F5EB7A5-CDCD-4FDD-A04B-5FACC38B2F94") | .config.scriptfile == "./scripts/script_filter.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="E6B67FD5-5462-46F2-BB39-75F015526AA6") | .config.scriptfile == "./scripts/script_filter.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="8F3399E3-951A-4DC0-BC7D-CFA83C1E1F76") | .config.scriptfile == "./scripts/script_filter_github.sh"' >/dev/null
echo "$packaged_json" | jq -e '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.scriptargtype] | all(. == 1)' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="F718886C-3E7F-45D4-BCDF-0167EFCBA0E8") | .config.scriptfile == "./scripts/action_open.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="F718886C-3E7F-45D4-BCDF-0167EFCBA0E8") | .config.type == 8' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="05AA5EAC-4638-4A25-B975-FE35FBEA8FA0") | .config.scriptfile == "./scripts/action_record_usage.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="05AA5EAC-4638-4A25-B975-FE35FBEA8FA0") | .config.type == 8' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="FD59A9AB-0760-49CC-98D9-8B6A7CE43210") | .config.scriptfile == "./scripts/action_record_usage.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="FD59A9AB-0760-49CC-98D9-8B6A7CE43210") | .config.type == 8' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="C74C273E-BE92-4960-9054-3577DC7878B4") | .config.scriptfile == "./scripts/action_open_github.sh"' >/dev/null
echo "$packaged_json" | jq -e '.objects[] | select(.uid=="C74C273E-BE92-4960-9054-3577DC7878B4") | .config.type == 8' >/dev/null
echo "$packaged_json" | jq -e '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.keyword] | map(select(. != null)) | index("c") != null' >/dev/null
echo "$packaged_json" | jq -e '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.keyword] | map(select(. != null)) | index("code") != null' >/dev/null
echo "$packaged_json" | jq -e '[.objects[] | select(.type=="alfred.workflow.input.scriptfilter") | .config.keyword] | map(select(. != null)) | index("github") != null' >/dev/null
echo "$packaged_json" | jq -e '.connections["6F5EB7A5-CDCD-4FDD-A04B-5FACC38B2F94"] | any(.modifiers == 1048576 and .destinationuid == "FD59A9AB-0760-49CC-98D9-8B6A7CE43210")' >/dev/null
echo "$packaged_json" | jq -e '[.userconfigurationconfig[] | .variable] | sort == ["OPEN_PROJECT_MAX_RESULTS", "PROJECT_DIRS", "USAGE_FILE", "VSCODE_PATH"]' >/dev/null
echo "$packaged_json" | jq -e '.userconfigurationconfig[] | select(.variable=="OPEN_PROJECT_MAX_RESULTS") | .config.default == "30"' >/dev/null

echo "ok: open-project smoke test"

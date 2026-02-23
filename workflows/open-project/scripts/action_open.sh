#!/usr/bin/env bash
set -euo pipefail

script_dir=$(
  CDPATH=
  cd -- "$(dirname -- "$0")" && pwd
)
helper_loader=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [ -f "$candidate" ]; then
    helper_loader="$candidate"
    break
  fi
done

if [ "$#" -lt 1 ] || [ -z "$1" ]; then
  echo "usage: action_open.sh <project-path>" >&2
  exit 2
fi

project_path="$(printf '%s' "$1")"
if [ -z "$project_path" ] || [ ! -d "$project_path" ]; then
  echo "error: project path is not a directory: $project_path" >&2
  exit 2
fi

if [ -z "$helper_loader" ]; then
  echo "error: workflow helper missing: workflow_helper_loader.sh" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" off; then
  echo "error: workflow helper missing: workflow_cli_resolver.sh" >&2
  exit 1
fi

vscode_bin_raw="${VSCODE_PATH:-/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code}"
vscode_bin="$(wfcr_expand_home_path "$vscode_bin_raw")"

if [ -x "$vscode_bin" ]; then
  exec "$vscode_bin" "$project_path"
fi

resolved_bin="$(command -v "$vscode_bin" 2>/dev/null || true)"
if [ -n "$resolved_bin" ] && [ -x "$resolved_bin" ]; then
  exec "$resolved_bin" "$project_path"
fi

echo "error: unable to execute VSCODE_PATH: $vscode_bin" >&2
exit 1

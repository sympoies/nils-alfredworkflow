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

resolve_workflow_cli() {
  repo_root=$(
    CDPATH=
    cd -- "$script_dir/../../.." && pwd
  )

  wfcr_resolve_binary \
    "WORKFLOW_CLI_BIN" \
    "$script_dir/../bin/workflow-cli" \
    "$repo_root/target/release/workflow-cli" \
    "$repo_root/target/debug/workflow-cli" \
    "error: workflow-cli binary not found (checked package/release/debug paths)"
}

if [ "$#" -lt 1 ] || [ -z "$1" ]; then
  echo "usage: action_record_usage.sh <project-path>" >&2
  exit 2
fi

project_path="$(printf '%s' "$1")"
if [ -z "$project_path" ]; then
  echo "usage: action_record_usage.sh <project-path>" >&2
  exit 2
fi

workflow_cli="$(resolve_workflow_cli)"
recorded_path="$("$workflow_cli" record-usage --path "$project_path")"
printf '%s' "$recorded_path"

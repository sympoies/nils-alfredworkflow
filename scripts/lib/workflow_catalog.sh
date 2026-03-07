#!/usr/bin/env bash

if [[ -n "${WORKFLOW_CATALOG_HELPERS_LOADED:-}" ]]; then
  return 0
fi
WORKFLOW_CATALOG_HELPERS_LOADED=1

wfc_toml_string() {
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

wfc_list_workflow_ids() {
  local repo_root="$1"

  find "$repo_root/workflows" -mindepth 1 -maxdepth 1 -type d \
    ! -name '_template' -exec basename {} \; | sort
}

wfc_manifest_path() {
  local repo_root="$1"
  local workflow_id="$2"
  printf '%s\n' "$repo_root/workflows/$workflow_id/workflow.toml"
}

wfc_bundle_id_for_workflow_id() {
  local repo_root="$1"
  local workflow_id="$2"
  local manifest

  manifest="$(wfc_manifest_path "$repo_root" "$workflow_id")"
  [[ -f "$manifest" ]] || return 1

  wfc_toml_string "$manifest" bundle_id
}

wfc_find_installed_workflow_dir_by_bundle_id() {
  local prefs_root="$1"
  local bundle_id="$2"
  local info bid

  command -v plutil >/dev/null 2>&1 || return 1
  [[ -d "$prefs_root" ]] || return 1

  for info in "$prefs_root"/*/info.plist; do
    [[ -f "$info" ]] || continue
    bid="$(plutil -extract bundleid raw -o - "$info" 2>/dev/null || true)"
    if [[ "$bid" == "$bundle_id" ]]; then
      dirname "$info"
      return 0
    fi
  done

  return 1
}

wfc_dist_latest_artifact() {
  local repo_root="$1"
  local workflow_id="$2"
  local workflow_dist="$repo_root/dist/$workflow_id"

  [[ -d "$workflow_dist" ]] || return 1

  find "$workflow_dist" -type f -name '*.alfredworkflow' | sort | tail -n 1
}

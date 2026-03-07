#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
workflow_catalog_lib="$repo_root/scripts/lib/workflow_catalog.sh"

prefs_root_default="$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows"
prefs_root="${ALFRED_PREFS_ROOT:-$prefs_root_default}"

[[ -f "$workflow_catalog_lib" ]] || {
  echo "error: missing helper library: $workflow_catalog_lib" >&2
  exit 1
}
# shellcheck disable=SC1090
source "$workflow_catalog_lib"

list_only=0
pack_all=0
install_after=0
install_only=0
install_mode="ui"
install_mode_explicit=0
preserve_customizations=1
workflow_id=""

usage() {
  cat <<USAGE
Usage:
  scripts/workflow-pack.sh --list
  scripts/workflow-pack.sh --id <workflow-id>
  scripts/workflow-pack.sh --id <workflow-id> --install --mode <ui|background>
  scripts/workflow-pack.sh --id <workflow-id> --install --mode background --no-preserve-customizations
  scripts/workflow-pack.sh --id <workflow-id> --install-only [--mode <ui|background>]
  scripts/workflow-pack.sh --all
  scripts/workflow-pack.sh --all --install
  scripts/workflow-pack.sh --all --install --no-preserve-customizations

Notes:
  - Single-workflow install defaults to --mode ui when --mode is omitted.
  - --all --install always uses background install because Alfred UI sheets block batch imports.
  - Background install only updates workflows that are already installed in Alfred.
  - Background install preserves prefs.plist and installed hotkey/keyword customizations by default.
  - Use --no-preserve-customizations with background install to reset installed hotkeys/keywords to packaged defaults.
USAGE
}

sha256_write() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" >"$file.sha256"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" >"$file.sha256"
  else
    echo "warn: sha256sum/shasum not found; skip checksum" >&2
  fi
}

render_plist() {
  local template="$1"
  local output="$2"
  local bundle_id="$3"
  local name="$4"
  local version="$5"

  sed \
    -e "s|{{bundle_id}}|$bundle_id|g" \
    -e "s|{{name}}|$name|g" \
    -e "s|{{version}}|$version|g" \
    "$template" >"$output"
}

install_workflow_artifact() {
  local id="$1"
  local artifact="$2"

  if ! command -v open >/dev/null 2>&1; then
    echo "error: install requires macOS 'open' command" >&2
    return 1
  fi

  open "$artifact"
  echo "ok: installed $artifact"

  if [[ "$id" == "cambridge-dict" ]]; then
    # Alfred import can replace the workflow directory shortly after `open`.
    # Delay and retry runtime setup so node_modules is not immediately wiped.
    local runtime_ready=0
    for _ in $(seq 1 3); do
      sleep 2
      if "$repo_root/scripts/setup-cambridge-workflow-runtime.sh" \
        --wait-for-install \
        --skip-browser \
        --quiet; then
        runtime_ready=1
        break
      fi
    done
    if [[ "$runtime_ready" -ne 1 ]]; then
      echo "warn: cambridge runtime setup failed; run scripts/setup-cambridge-workflow-runtime.sh manually" >&2
    fi
  fi
}

dist_artifact_for_id() {
  local id="$1"
  local artifact=""

  artifact="$(wfc_dist_latest_artifact "$repo_root" "$id" || true)"
  [[ -n "$artifact" && -f "$artifact" ]] || {
    echo "error: no .alfredworkflow artifact found under dist/$id (run scripts/workflow-pack.sh --id $id first)" >&2
    return 1
  }

  printf '%s\n' "$artifact"
}

install_only_one_ui() {
  local id="$1"
  local artifact=""

  artifact="$(dist_artifact_for_id "$id")" || return 1
  install_workflow_artifact "$id" "$artifact"
}

resolve_effective_install_mode() {
  local mode="$install_mode"

  if [[ "$pack_all" -eq 1 && ("$install_after" -eq 1 || "$install_only" -eq 1) ]]; then
    if [[ "$install_mode_explicit" -eq 1 && "$install_mode" != "background" ]]; then
      echo "error: --all install only supports --mode background" >&2
      return 1
    fi
    mode="background"
  fi

  printf '%s\n' "$mode"
}

ensure_background_install_prereqs() {
  local required_cmd

  for required_cmd in ditto osascript plutil python3 rsync; do
    if ! command -v "$required_cmd" >/dev/null 2>&1; then
      echo "error: background install requires command: $required_cmd" >&2
      return 1
    fi
  done

  if [[ ! -d "$prefs_root" ]]; then
    echo "warn: Alfred workflows directory not found: $prefs_root"
    return 10
  fi
}

unpack_workflow_artifact() {
  local artifact="$1"
  local stage_dir="$2"

  rm -rf "$stage_dir"
  mkdir -p "$stage_dir"
  ditto -x -k "$artifact" "$stage_dir"
}

sync_stage_to_installed() {
  local stage_dir="$1"
  local installed_dir="$2"

  mkdir -p "$installed_dir"
  rsync \
    -a \
    --delete \
    --exclude 'prefs.plist' \
    --exclude 'node_modules/' \
    "$stage_dir"/ "$installed_dir"/
}

reload_workflow() {
  local bundle_id="$1"
  osascript -e "tell application id \"com.runningwithcrayons.Alfred\" to reload workflow \"$bundle_id\"" >/dev/null
}

merge_installed_customizations_into_stage() {
  local installed_plist="$1"
  local staged_plist="$2"

  python3 - "$installed_plist" "$staged_plist" <<'PY'
import plistlib
import sys
from pathlib import Path

installed_path = Path(sys.argv[1])
staged_path = Path(sys.argv[2])

with installed_path.open("rb") as fh:
    installed = plistlib.load(fh)
with staged_path.open("rb") as fh:
    staged = plistlib.load(fh)

installed_hotkeys = {}
installed_keywords = {}
for obj in installed.get("objects", []):
    if not isinstance(obj, dict):
        continue
    uid = obj.get("uid")
    config = obj.get("config")
    if not uid or not isinstance(config, dict):
        continue
    if obj.get("type") == "alfred.workflow.trigger.hotkey":
        installed_hotkeys[uid] = config
    if "keyword" in config:
        installed_keywords[uid] = config.get("keyword")

updated = False
for obj in staged.get("objects", []):
    if not isinstance(obj, dict):
        continue
    uid = obj.get("uid")
    config = obj.get("config")
    if not uid or not isinstance(config, dict):
        continue

    if obj.get("type") == "alfred.workflow.trigger.hotkey":
        installed_config = installed_hotkeys.get(uid)
        if isinstance(installed_config, dict):
            merged_config = dict(config)
            merged_config.update(installed_config)
            if merged_config != config:
                obj["config"] = merged_config
                config = merged_config
                updated = True

    if "keyword" in config and uid in installed_keywords:
        installed_keyword = installed_keywords[uid]
        if installed_keyword != config.get("keyword"):
            config["keyword"] = installed_keyword
            updated = True

if updated:
    with staged_path.open("wb") as fh:
        plistlib.dump(staged, fh, fmt=plistlib.FMT_XML, sort_keys=False)
PY
}

background_install_one_from_artifact() {
  local id="$1"
  local artifact="$2"
  local manifest bundle_id installed_dir tmp_root stage_dir stage_bundle

  manifest="$(wfc_manifest_path "$repo_root" "$id")"
  bundle_id="$(wfc_bundle_id_for_workflow_id "$repo_root" "$id" || true)"
  [[ -n "$bundle_id" ]] || {
    echo "error: missing bundle_id in $manifest" >&2
    return 1
  }

  installed_dir="$(wfc_find_installed_workflow_dir_by_bundle_id "$prefs_root" "$bundle_id" || true)"
  if [[ -z "$installed_dir" ]]; then
    echo "skip: not installed ($id, $bundle_id)"
    return 10
  fi

  tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/workflow-pack.background.${id}.XXXXXX")"
  stage_dir="$tmp_root/pkg"
  unpack_workflow_artifact "$artifact" "$stage_dir"

  stage_bundle="$(plutil -extract bundleid raw -o - "$stage_dir/info.plist" 2>/dev/null || true)"
  if [[ "$stage_bundle" != "$bundle_id" ]]; then
    echo "error: staged bundle_id mismatch for $id (expected $bundle_id, got ${stage_bundle:-<empty>})" >&2
    rm -rf "$tmp_root"
    return 1
  fi

  if [[ "$preserve_customizations" -eq 1 ]]; then
    merge_installed_customizations_into_stage "$installed_dir/info.plist" "$stage_dir/info.plist"
  fi
  sync_stage_to_installed "$stage_dir" "$installed_dir"
  reload_workflow "$bundle_id"
  rm -rf "$tmp_root"

  echo "ok: background installed $id -> $installed_dir"
}

background_install_one_from_dist() {
  local id="$1"
  local artifact=""

  artifact="$(dist_artifact_for_id "$id")" || return 1
  background_install_one_from_artifact "$id" "$artifact"
}

background_install_all_from_dist() {
  local id
  local success_count=0
  local skip_count=0
  local fail_count=0

  echo "info: --all install uses background mode because Alfred UI sheets block batch imports"
  while IFS= read -r id; do
    [[ -n "$id" ]] || continue
    if background_install_one_from_dist "$id"; then
      success_count=$((success_count + 1))
    else
      case "$?" in
      10)
        skip_count=$((skip_count + 1))
        ;;
      *)
        fail_count=$((fail_count + 1))
        ;;
      esac
    fi
  done < <(wfc_list_workflow_ids "$repo_root")

  echo "summary: updated=$success_count skipped=$skip_count failed=$fail_count"
  if [[ "$fail_count" -gt 0 ]]; then
    return 1
  fi
}

package_one() {
  local id="$1"
  local manifest="$repo_root/workflows/$id/workflow.toml"
  local workflow_root="$repo_root/workflows/$id"

  [[ -f "$manifest" ]] || {
    echo "error: missing manifest: $manifest" >&2
    return 1
  }

  local name bundle_id version rust_binary rust_package readme_source effective_readme_source
  name="$(wfc_toml_string "$manifest" name)"
  bundle_id="$(wfc_toml_string "$manifest" bundle_id)"
  version="$(wfc_toml_string "$manifest" version)"
  rust_binary="$(wfc_toml_string "$manifest" rust_binary)"
  readme_source="$(wfc_toml_string "$manifest" readme_source)"

  [[ -n "$name" ]] || {
    echo "error: missing name in $manifest" >&2
    return 1
  }
  [[ -n "$bundle_id" ]] || {
    echo "error: missing bundle_id in $manifest" >&2
    return 1
  }
  [[ -n "$version" ]] || {
    echo "error: missing version in $manifest" >&2
    return 1
  }

  if [[ -n "$rust_binary" ]]; then
    rust_package="$rust_binary"
    if [[ "$rust_package" != nils-* ]]; then
      rust_package="nils-$rust_package"
    fi
    cargo build --release -p "$rust_package"
  fi

  local stage_dir="$repo_root/build/workflows/$id/pkg"
  rm -rf "$stage_dir"
  mkdir -p "$stage_dir"

  cp -R "$workflow_root/scripts" "$stage_dir/"
  if [[ -d "$stage_dir/scripts" ]]; then
    find "$stage_dir/scripts" -type f -name '*.sh' -exec chmod +x {} +
  fi

  local shared_lib_dir
  shared_lib_dir="$repo_root/scripts/lib"
  if [[ -d "$stage_dir/scripts" && -d "$shared_lib_dir" ]]; then
    mkdir -p "$stage_dir/scripts/lib"
    if compgen -G "$shared_lib_dir/*.sh" >/dev/null; then
      cp "$shared_lib_dir"/*.sh "$stage_dir/scripts/lib/"
      find "$stage_dir/scripts/lib" -type f -name '*.sh' -exec chmod +x {} +
    fi
  fi

  if [[ -d "$workflow_root/src/assets" ]]; then
    cp -R "$workflow_root/src/assets" "$stage_dir/"
  fi

  if compgen -G "$workflow_root/src/*.png" >/dev/null; then
    cp "$workflow_root"/src/*.png "$stage_dir/"
  fi

  if [[ -f "$workflow_root/src/assets/icon.png" ]]; then
    cp "$workflow_root/src/assets/icon.png" "$stage_dir/icon.png"
  fi

  local prepare_package_hook
  prepare_package_hook="$workflow_root/scripts/prepare_package.sh"
  if [[ -x "$prepare_package_hook" ]]; then
    "$prepare_package_hook" --stage-dir "$stage_dir" --workflow-root "$workflow_root"
  fi

  render_plist \
    "$workflow_root/src/info.plist.template" \
    "$stage_dir/info.plist" \
    "$bundle_id" \
    "$name" \
    "$version"

  effective_readme_source="$readme_source"
  if [[ -z "$effective_readme_source" && -f "$workflow_root/README.md" ]]; then
    effective_readme_source="README.md"
  fi

  if [[ -n "$effective_readme_source" ]]; then
    cargo run -p nils-workflow-readme-cli -- \
      convert \
      --workflow-root "$workflow_root" \
      --readme-source "$effective_readme_source" \
      --stage-dir "$stage_dir" \
      --plist "$stage_dir/info.plist"
  fi

  if [[ -n "$rust_binary" && -f "$repo_root/target/release/$rust_binary" ]]; then
    mkdir -p "$stage_dir/bin"
    cp "$repo_root/target/release/$rust_binary" "$stage_dir/bin/$rust_binary"
  fi

  if command -v plutil >/dev/null 2>&1; then
    plutil -lint "$stage_dir/info.plist" >/dev/null
  fi

  local out_dir="$repo_root/dist/$id/$version"
  mkdir -p "$out_dir"
  local artifact="$out_dir/${name}.alfredworkflow"

  rm -f "$artifact" "$artifact.sha256"
  (cd "$stage_dir" && zip -rq "$artifact" .)
  sha256_write "$artifact"

  echo "ok: packaged $artifact"

  if [[ "$install_after" -eq 1 ]]; then
    install_workflow_artifact "$id" "$artifact"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --list)
    list_only=1
    shift
    ;;
  --id)
    workflow_id="${2:-}"
    [[ -n "$workflow_id" ]] || {
      echo "error: --id requires a value" >&2
      exit 2
    }
    shift 2
    ;;
  --all)
    pack_all=1
    shift
    ;;
  --install)
    install_after=1
    shift
    ;;
  --install-only)
    install_only=1
    shift
    ;;
  --mode)
    install_mode="${2:-}"
    [[ -n "$install_mode" ]] || {
      echo "error: --mode requires a value" >&2
      exit 2
    }
    case "$install_mode" in
    ui | background) ;;
    *)
      echo "error: --mode must be ui or background" >&2
      exit 2
      ;;
    esac
    install_mode_explicit=1
    shift 2
    ;;
  --no-preserve-customizations)
    preserve_customizations=0
    shift
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown argument: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done

if [[ "$list_only" -eq 1 ]]; then
  wfc_list_workflow_ids "$repo_root"
  exit 0
fi

if [[ "$install_only" -eq 1 && "$install_after" -eq 1 ]]; then
  echo "error: --install and --install-only cannot be used together" >&2
  usage >&2
  exit 2
fi

if [[ "$pack_all" -eq 1 && -n "$workflow_id" ]]; then
  echo "error: --all cannot be used with --id" >&2
  usage >&2
  exit 2
fi

if [[ "$install_mode_explicit" -eq 1 && "$install_after" -eq 0 && "$install_only" -eq 0 ]]; then
  echo "error: --mode requires --install or --install-only" >&2
  usage >&2
  exit 2
fi

effective_install_mode="$(resolve_effective_install_mode)" || exit $?

if [[ "$preserve_customizations" -eq 0 && "$effective_install_mode" == "ui" ]]; then
  echo "error: --no-preserve-customizations is only supported with --mode background" >&2
  usage >&2
  exit 2
fi

if [[ "$install_only" -eq 1 ]]; then
  if [[ -n "$workflow_id" ]]; then
    if [[ "$effective_install_mode" == "ui" ]]; then
      install_only_one_ui "$workflow_id"
      exit $?
    fi
    ensure_background_install_prereqs || {
      rc=$?
      [[ "$rc" -eq 10 ]] && exit 0
      exit "$rc"
    }
    if background_install_one_from_dist "$workflow_id"; then
      exit 0
    fi
    rc=$?
    [[ "$rc" -eq 10 ]] && exit 0
    exit "$rc"
  fi

  if [[ "$pack_all" -eq 1 ]]; then
    ensure_background_install_prereqs || {
      rc=$?
      [[ "$rc" -eq 10 ]] && exit 0
      exit "$rc"
    }
    background_install_all_from_dist
    exit $?
  fi

  echo "error: --install-only requires --id <workflow-id> or --all" >&2
  usage >&2
  exit 2
fi

if [[ "$pack_all" -eq 1 ]]; then
  background_after_pack_all=0
  if [[ "$install_after" -eq 1 ]]; then
    background_after_pack_all=1
    install_after=0
  fi

  while IFS= read -r id; do
    [[ -n "$id" ]] || continue
    package_one "$id"
  done < <(wfc_list_workflow_ids "$repo_root")

  if [[ "$background_after_pack_all" -eq 1 ]]; then
    ensure_background_install_prereqs || {
      rc=$?
      [[ "$rc" -eq 10 ]] && exit 0
      exit "$rc"
    }
    background_install_all_from_dist
    exit $?
  fi

  exit 0
fi

if [[ -n "$workflow_id" ]]; then
  if [[ "$install_after" -eq 1 && "$effective_install_mode" == "ui" ]]; then
    package_one "$workflow_id"
    exit $?
  fi

  background_after_pack_one=0
  if [[ "$install_after" -eq 1 && "$effective_install_mode" == "background" ]]; then
    background_after_pack_one=1
    install_after=0
  fi

  package_one "$workflow_id"

  if [[ "$background_after_pack_one" -eq 1 ]]; then
    ensure_background_install_prereqs || {
      rc=$?
      [[ "$rc" -eq 10 ]] && exit 0
      exit "$rc"
    }
    if background_install_one_from_dist "$workflow_id"; then
      exit 0
    fi
    rc=$?
    [[ "$rc" -eq 10 ]] && exit 0
    exit "$rc"
  fi

  exit 0
fi

usage >&2
exit 2

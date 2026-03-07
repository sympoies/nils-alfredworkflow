#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/workflow-pack.test.XXXXXX")"
fake_bin_dir="$test_root/bin"
mkdir -p "$fake_bin_dir"

trap 'rm -rf "$test_root"' EXIT

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

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "$label (missing '$needle')"
  fi
}

assert_file_exists() {
  local path="$1"
  local label="$2"
  [[ -e "$path" ]] || fail "$label (missing path: $path)"
}

assert_file_not_exists() {
  local path="$1"
  local label="$2"
  [[ ! -e "$path" ]] || fail "$label (unexpected path: $path)"
}

make_fake_tools() {
  cat >"$fake_bin_dir/open" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >>"$WORKFLOW_PACK_OPEN_LOG"
EOF

  cat >"$fake_bin_dir/osascript" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$WORKFLOW_PACK_OSASCRIPT_LOG"
EOF

  cat >"$fake_bin_dir/plutil" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "-lint" ]]; then
  exit 0
fi

file="${!#}"
content="$(tr -d '\n' <"$file")"
if [[ "$content" == *"<key>bundleid</key>"* ]]; then
  printf '%s\n' "$content" | sed -E 's/.*<key>bundleid<\/key>[[:space:]]*<string>([^<]+)<\/string>.*/\1/'
else
  cat "$file"
fi
EOF

  cat >"$fake_bin_dir/ditto" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "-x" || "${2:-}" != "-k" ]]; then
  printf 'unsupported ditto args: %s\n' "$*" >&2
  exit 2
fi

unzip -qq "$3" -d "$4"
EOF

  chmod +x "$fake_bin_dir/open" "$fake_bin_dir/osascript" "$fake_bin_dir/plutil" "$fake_bin_dir/ditto"
}

copy_pack_scripts() {
  local fixture_repo="$1"
  mkdir -p "$fixture_repo/scripts/lib"
  cp "$repo_root/scripts/workflow-pack.sh" "$fixture_repo/scripts/workflow-pack.sh"
  cp "$repo_root/scripts/lib/workflow_catalog.sh" "$fixture_repo/scripts/lib/workflow_catalog.sh"
  chmod +x "$fixture_repo/scripts/workflow-pack.sh"
}

write_workflow_fixture() {
  local fixture_repo="$1"
  local id="$2"
  local name="$3"

  mkdir -p \
    "$fixture_repo/workflows/$id/src" \
    "$fixture_repo/workflows/$id/scripts"

  cat >"$fixture_repo/workflows/$id/workflow.toml" <<EOF
id = "$id"
name = "$name"
bundle_id = "com.sympoies.$id"
version = "1.0.0"
EOF

  cat >"$fixture_repo/workflows/$id/src/info.plist.template" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>bundleid</key>
  <string>{{bundle_id}}</string>
  <key>name</key>
  <string>{{name}}</string>
  <key>objects</key>
  <array>
    <dict>
      <key>config</key>
      <dict>
        <key>action</key>
        <integer>0</integer>
        <key>argument</key>
        <integer>0</integer>
        <key>focusedappvariable</key>
        <false/>
        <key>focusedappvariablename</key>
        <string></string>
        <key>hotkey</key>
        <integer>0</integer>
        <key>hotmod</key>
        <integer>0</integer>
        <key>leftcursor</key>
        <false/>
        <key>modsmode</key>
        <integer>0</integer>
        <key>relatedAppsMode</key>
        <integer>0</integer>
      </dict>
      <key>type</key>
      <string>alfred.workflow.trigger.hotkey</string>
      <key>uid</key>
      <string>HOTKEY-UID</string>
      <key>version</key>
      <integer>2</integer>
    </dict>
    <dict>
      <key>config</key>
      <dict>
        <key>keyword</key>
        <string>alpha</string>
      </dict>
      <key>type</key>
      <string>alfred.workflow.input.scriptfilter</string>
      <key>uid</key>
      <string>KEYWORD-UID</string>
      <key>version</key>
      <integer>3</integer>
    </dict>
  </array>
  <key>version</key>
  <string>{{version}}</string>
</dict>
</plist>
EOF

  cat >"$fixture_repo/workflows/$id/scripts/main.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'ok\n'
EOF
  chmod +x "$fixture_repo/workflows/$id/scripts/main.sh"
}

write_installed_info_plist() {
  local target="$1"
  local bundle_id="$2"
  local hotkey="$3"
  local hotmod="$4"
  local hotstring="$5"
  local keyword="$6"

  cat >"$target" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>bundleid</key>
  <string>$bundle_id</string>
  <key>objects</key>
  <array>
    <dict>
      <key>config</key>
      <dict>
        <key>action</key>
        <integer>0</integer>
        <key>argument</key>
        <integer>0</integer>
        <key>focusedappvariable</key>
        <false/>
        <key>focusedappvariablename</key>
        <string></string>
        <key>hotkey</key>
        <integer>$hotkey</integer>
        <key>hotmod</key>
        <integer>$hotmod</integer>
        <key>hotstring</key>
        <string>$hotstring</string>
        <key>leftcursor</key>
        <false/>
        <key>modsmode</key>
        <integer>0</integer>
        <key>relatedAppsMode</key>
        <integer>0</integer>
      </dict>
      <key>type</key>
      <string>alfred.workflow.trigger.hotkey</string>
      <key>uid</key>
      <string>HOTKEY-UID</string>
      <key>version</key>
      <integer>2</integer>
    </dict>
    <dict>
      <key>config</key>
      <dict>
        <key>keyword</key>
        <string>$keyword</string>
      </dict>
      <key>type</key>
      <string>alfred.workflow.input.scriptfilter</string>
      <key>uid</key>
      <string>KEYWORD-UID</string>
      <key>version</key>
      <integer>3</integer>
    </dict>
  </array>
</dict>
</plist>
EOF
}

read_hotkey_value() {
  local plist_path="$1"
  python3 - "$plist_path" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as fh:
    payload = plistlib.load(fh)

for obj in payload.get("objects", []):
    if obj.get("type") != "alfred.workflow.trigger.hotkey":
        continue
    print(obj.get("config", {}).get("hotkey", ""))
    break
PY
}

read_hotkey_string() {
  local plist_path="$1"
  python3 - "$plist_path" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as fh:
    payload = plistlib.load(fh)

for obj in payload.get("objects", []):
    if obj.get("type") != "alfred.workflow.trigger.hotkey":
        continue
    print(obj.get("config", {}).get("hotstring", ""))
    break
PY
}

read_keyword_value() {
  local plist_path="$1"
  python3 - "$plist_path" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as fh:
    payload = plistlib.load(fh)

for obj in payload.get("objects", []):
    if obj.get("uid") != "KEYWORD-UID":
        continue
    print(obj.get("config", {}).get("keyword", ""))
    break
PY
}

test_single_ui_install_uses_open() {
  local fixture_root="$test_root/single-ui"
  local fixture_repo="$fixture_root/repo"
  local open_log="$fixture_root/open.log"
  local osascript_log="$fixture_root/osascript.log"

  mkdir -p "$fixture_repo"
  copy_pack_scripts "$fixture_repo"
  write_workflow_fixture "$fixture_repo" "alpha" "Alpha Workflow"
  : >"$open_log"
  : >"$osascript_log"

  PATH="$fake_bin_dir:$PATH" \
    WORKFLOW_PACK_OPEN_LOG="$open_log" \
    WORKFLOW_PACK_OSASCRIPT_LOG="$osascript_log" \
    bash "$fixture_repo/scripts/workflow-pack.sh" --id alpha --install --mode ui >/dev/null

  assert_contains \
    "$(cat "$open_log")" \
    "dist/alpha/1.0.0/Alpha Workflow.alfredworkflow" \
    "ui install should open packaged artifact"
  assert_eq "" "$(cat "$osascript_log")" "ui install should not reload via osascript"
}

test_single_background_install_updates_installed_copy() {
  local fixture_root="$test_root/single-background"
  local fixture_repo="$fixture_root/repo"
  local prefs_root="$fixture_root/prefs"
  local installed_dir="$prefs_root/user.workflow.alpha"
  local open_log="$fixture_root/open.log"
  local osascript_log="$fixture_root/osascript.log"

  mkdir -p "$fixture_repo" "$installed_dir"
  copy_pack_scripts "$fixture_repo"
  write_workflow_fixture "$fixture_repo" "alpha" "Alpha Workflow"
  write_installed_info_plist "$installed_dir/info.plist" "com.sympoies.alpha" "35" "524288" "P" "ap"
  printf '%s\n' "keep prefs" >"$installed_dir/prefs.plist"
  printf '%s\n' "stale" >"$installed_dir/old.txt"
  : >"$open_log"
  : >"$osascript_log"

  PATH="$fake_bin_dir:$PATH" \
    ALFRED_PREFS_ROOT="$prefs_root" \
    WORKFLOW_PACK_OPEN_LOG="$open_log" \
    WORKFLOW_PACK_OSASCRIPT_LOG="$osascript_log" \
    bash "$fixture_repo/scripts/workflow-pack.sh" --id alpha --install --mode background >/dev/null

  assert_eq "keep prefs" "$(cat "$installed_dir/prefs.plist")" "background install preserves prefs"
  assert_file_not_exists "$installed_dir/old.txt" "background install removes stale packaged files"
  assert_file_exists "$installed_dir/scripts/main.sh" "background install syncs packaged files"
  assert_eq "35" "$(read_hotkey_value "$installed_dir/info.plist")" "background install preserves hotkey keycode"
  assert_eq "P" "$(read_hotkey_string "$installed_dir/info.plist")" "background install preserves hotkey string"
  assert_eq "ap" "$(read_keyword_value "$installed_dir/info.plist")" "background install preserves keyword"
  assert_contains \
    "$(cat "$osascript_log")" \
    'reload workflow "com.sympoies.alpha"' \
    "background install should reload workflow"
  assert_eq "" "$(cat "$open_log")" "background install should not use open"
}

test_single_background_install_can_reset_customizations() {
  local fixture_root="$test_root/single-background-reset"
  local fixture_repo="$fixture_root/repo"
  local prefs_root="$fixture_root/prefs"
  local installed_dir="$prefs_root/user.workflow.alpha"
  local open_log="$fixture_root/open.log"
  local osascript_log="$fixture_root/osascript.log"

  mkdir -p "$fixture_repo" "$installed_dir"
  copy_pack_scripts "$fixture_repo"
  write_workflow_fixture "$fixture_repo" "alpha" "Alpha Workflow"
  write_installed_info_plist "$installed_dir/info.plist" "com.sympoies.alpha" "35" "524288" "P" "ap"
  printf '%s\n' "keep prefs" >"$installed_dir/prefs.plist"
  : >"$open_log"
  : >"$osascript_log"

  PATH="$fake_bin_dir:$PATH" \
    ALFRED_PREFS_ROOT="$prefs_root" \
    WORKFLOW_PACK_OPEN_LOG="$open_log" \
    WORKFLOW_PACK_OSASCRIPT_LOG="$osascript_log" \
    bash "$fixture_repo/scripts/workflow-pack.sh" --id alpha --install --mode background --no-preserve-customizations >/dev/null

  assert_eq "0" "$(read_hotkey_value "$installed_dir/info.plist")" "reset mode restores packaged hotkey keycode"
  assert_eq "" "$(read_hotkey_string "$installed_dir/info.plist")" "reset mode restores packaged hotkey string"
  assert_eq "alpha" "$(read_keyword_value "$installed_dir/info.plist")" "reset mode restores packaged keyword"
}

test_all_install_forces_background_mode() {
  local fixture_root="$test_root/all-install"
  local fixture_repo="$fixture_root/repo"
  local prefs_root="$fixture_root/prefs"
  local installed_alpha="$prefs_root/user.workflow.alpha"
  local open_log="$fixture_root/open.log"
  local osascript_log="$fixture_root/osascript.log"

  mkdir -p "$fixture_repo" "$installed_alpha"
  copy_pack_scripts "$fixture_repo"
  write_workflow_fixture "$fixture_repo" "alpha" "Alpha Workflow"
  write_workflow_fixture "$fixture_repo" "beta" "Beta Workflow"
  write_installed_info_plist "$installed_alpha/info.plist" "com.sympoies.alpha" "35" "524288" "P" "ap"
  printf '%s\n' "keep prefs" >"$installed_alpha/prefs.plist"
  : >"$open_log"
  : >"$osascript_log"

  PATH="$fake_bin_dir:$PATH" \
    ALFRED_PREFS_ROOT="$prefs_root" \
    WORKFLOW_PACK_OPEN_LOG="$open_log" \
    WORKFLOW_PACK_OSASCRIPT_LOG="$osascript_log" \
    bash "$fixture_repo/scripts/workflow-pack.sh" --all --install >/dev/null

  assert_file_exists \
    "$fixture_repo/dist/alpha/1.0.0/Alpha Workflow.alfredworkflow" \
    "all install should package alpha"
  assert_file_exists \
    "$fixture_repo/dist/beta/1.0.0/Beta Workflow.alfredworkflow" \
    "all install should package beta"
  assert_file_exists "$installed_alpha/scripts/main.sh" "all install should background update installed workflow"
  assert_eq "35" "$(read_hotkey_value "$installed_alpha/info.plist")" "all install preserves hotkey keycode"
  assert_eq "P" "$(read_hotkey_string "$installed_alpha/info.plist")" "all install preserves hotkey string"
  assert_eq "ap" "$(read_keyword_value "$installed_alpha/info.plist")" "all install preserves keyword"
  assert_contains \
    "$(cat "$osascript_log")" \
    'reload workflow "com.sympoies.alpha"' \
    "all install should reload installed workflow"
  assert_eq "" "$(cat "$open_log")" "all install should not use ui open"
  assert_file_not_exists "$prefs_root/user.workflow.beta" "all install should not create new installed workflow dirs"
}

test_all_install_can_reset_customizations() {
  local fixture_root="$test_root/all-install-reset"
  local fixture_repo="$fixture_root/repo"
  local prefs_root="$fixture_root/prefs"
  local installed_alpha="$prefs_root/user.workflow.alpha"
  local open_log="$fixture_root/open.log"
  local osascript_log="$fixture_root/osascript.log"

  mkdir -p "$fixture_repo" "$installed_alpha"
  copy_pack_scripts "$fixture_repo"
  write_workflow_fixture "$fixture_repo" "alpha" "Alpha Workflow"
  write_workflow_fixture "$fixture_repo" "beta" "Beta Workflow"
  write_installed_info_plist "$installed_alpha/info.plist" "com.sympoies.alpha" "35" "524288" "P" "ap"
  printf '%s\n' "keep prefs" >"$installed_alpha/prefs.plist"
  : >"$open_log"
  : >"$osascript_log"

  PATH="$fake_bin_dir:$PATH" \
    ALFRED_PREFS_ROOT="$prefs_root" \
    WORKFLOW_PACK_OPEN_LOG="$open_log" \
    WORKFLOW_PACK_OSASCRIPT_LOG="$osascript_log" \
    bash "$fixture_repo/scripts/workflow-pack.sh" --all --install --no-preserve-customizations >/dev/null

  assert_eq "keep prefs" "$(cat "$installed_alpha/prefs.plist")" "all reset install still preserves prefs"
  assert_eq "0" "$(read_hotkey_value "$installed_alpha/info.plist")" "all reset install restores packaged hotkey keycode"
  assert_eq "" "$(read_hotkey_string "$installed_alpha/info.plist")" "all reset install restores packaged hotkey string"
  assert_eq "alpha" "$(read_keyword_value "$installed_alpha/info.plist")" "all reset install restores packaged keyword"
}

main() {
  make_fake_tools
  test_single_ui_install_uses_open
  test_single_background_install_updates_installed_copy
  test_single_background_install_can_reset_customizations
  test_all_install_forces_background_mode
  test_all_install_can_reset_customizations
  printf 'ok: workflow_pack tests passed\n'
}

main "$@"

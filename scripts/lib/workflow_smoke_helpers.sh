#!/usr/bin/env bash

if [[ -n "${WORKFLOW_SMOKE_HELPERS_LOADED:-}" ]]; then
  return 0
fi
WORKFLOW_SMOKE_HELPERS_LOADED=1

fail() {
  echo "error: $*" >&2
  exit 1
}

require_bin() {
  local binary="$1"
  command -v "$binary" >/dev/null 2>&1 || fail "missing required binary: $binary"
}

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

workflow_smoke_assert_action_requires_arg() {
  local action_script="$1"
  local expected_rc="${2:-2}"

  set +e
  "$action_script" >/dev/null 2>&1
  local action_rc=$?
  set -e

  if [[ "$action_rc" -ne "$expected_rc" ]]; then
    fail "$(basename "$action_script") without args must exit $expected_rc"
  fi
}

workflow_smoke_write_open_stub() {
  local stub_path="$1"
  mkdir -p "$(dirname "$stub_path")"

  cat >"$stub_path" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >"$OPEN_STUB_OUT"
EOS
  chmod +x "$stub_path"
}

workflow_smoke_write_pbcopy_stub() {
  local stub_path="$1"
  mkdir -p "$(dirname "$stub_path")"

  cat >"$stub_path" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
cat >"$PBCOPY_STUB_OUT"
EOS
  chmod +x "$stub_path"
}

artifact_backup_file() {
  local target_path="$1"
  local backup_dir="$2"
  local backup_label="${3:-$(basename "$target_path")}"
  local backup_path=""

  if [[ -f "$target_path" ]]; then
    mkdir -p "$backup_dir"
    backup_path="$backup_dir/${backup_label}.backup"
    cp "$target_path" "$backup_path"
  fi

  printf '%s\n' "$backup_path"
}

artifact_restore_file() {
  local target_path="$1"
  local backup_path="$2"

  if [[ -n "$backup_path" && -f "$backup_path" ]]; then
    mkdir -p "$(dirname "$target_path")"
    cp "$backup_path" "$target_path"
    return 0
  fi

  rm -f "$target_path"
}

workflow_smoke_auto_shellcheck() {
  if [[ "${WORKFLOW_SMOKE_SKIP_SHELLCHECK:-0}" == "1" ]]; then
    return 0
  fi

  local caller_script="${BASH_SOURCE[1]:-}"
  case "$caller_script" in
  */workflows/*/tests/smoke.sh) ;;
  *)
    return 0
    ;;
  esac

  local workflow_dir
  workflow_dir="$(cd "$(dirname "$caller_script")/.." && pwd)"
  [[ -d "$workflow_dir" ]] || return 0

  require_bin shellcheck

  local -a shellcheck_targets=()
  mapfile -t shellcheck_targets < <(find "$workflow_dir" -type f -name '*.sh' | sort)
  if [[ "${#shellcheck_targets[@]}" -eq 0 ]]; then
    return 0
  fi

  # Some scripts source environment-specific paths dynamically (for example ~/.cargo/env).
  shellcheck -e SC1091 "${shellcheck_targets[@]}"
}

workflow_smoke_auto_shellcheck

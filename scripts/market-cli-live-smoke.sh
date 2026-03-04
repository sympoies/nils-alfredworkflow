#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/market-cli-live-smoke.XXXXXX")"
cache_dir="$tmp_root/cache"
mkdir -p "$cache_dir"

cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT

pass_count=0
fail_count=0
skip_count=0
total_count=0

results=()
last_favorites_icon_path=""

is_network_unavailable() {
  local message="${1,,}"
  local needles=(
    "network is unreachable"
    "no route to host"
    "temporary failure in name resolution"
    "name or service not known"
    "could not resolve"
    "timed out"
    "timeout"
    "connection refused"
    "connection reset"
    "tls handshake eof"
    "failed to lookup address information"
  )

  local needle
  for needle in "${needles[@]}"; do
    if [[ "$message" == *"$needle"* ]]; then
      return 0
    fi
  done
  return 1
}

compact_text() {
  tr '\n' ' ' <"$1" | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//'
}

validate_contract() {
  local expected_kind="$1"
  local json_file="$2"

  python3 - "$expected_kind" "$json_file" <<'PY'
import json
import sys
from pathlib import Path

expected_kind = sys.argv[1]
payload_path = Path(sys.argv[2])
raw = payload_path.read_text(encoding="utf-8").strip()
if not raw:
    raise SystemExit("empty output")

try:
    payload = json.loads(raw)
except json.JSONDecodeError as exc:
    raise SystemExit(f"invalid json: {exc.msg} at pos {exc.pos}") from exc

if not isinstance(payload, dict):
    raise SystemExit("top-level json must be an object")

if "result" in payload:
    if payload.get("ok") is not True:
        raise SystemExit("service envelope reported ok=false")
    result = payload.get("result")
    if not isinstance(result, dict):
        raise SystemExit("service envelope result must be an object")
    payload = result

required_fields = [
    "kind",
    "base",
    "quote",
    "amount",
    "unit_price",
    "converted",
    "provider",
    "fetched_at",
    "cache",
]
missing = [name for name in required_fields if name not in payload]
if missing:
    raise SystemExit("missing fields: " + ",".join(missing))

kind = payload.get("kind")
if kind != expected_kind:
    raise SystemExit(f"kind mismatch: expected {expected_kind}, got {kind!r}")

provider = payload.get("provider")
if not isinstance(provider, str) or not provider.strip():
    raise SystemExit("provider must be a non-empty string")

freshness = None
cache_block = payload.get("cache")
if isinstance(cache_block, dict):
    for key in ("freshness", "status", "state"):
        value = cache_block.get(key)
        if isinstance(value, str) and value.strip():
            freshness = value
            break

for key in ("freshness", "cache_status"):
    value = payload.get(key)
    if isinstance(value, str) and value.strip():
        freshness = freshness or value

print(f"provider={provider} freshness={freshness or 'unknown'}")
PY
}

validate_favorites_icon_contract() {
  local json_file="$1"
  local cache_root="$2"

  python3 - "$json_file" "$cache_root" <<'PY'
import json
import sys
from pathlib import Path

payload_path = Path(sys.argv[1])
cache_root = Path(sys.argv[2]).resolve()
raw = payload_path.read_text(encoding="utf-8").strip()
if not raw:
    raise SystemExit("empty output")

try:
    payload = json.loads(raw)
except json.JSONDecodeError as exc:
    raise SystemExit(f"invalid json: {exc.msg} at pos {exc.pos}") from exc

items = payload.get("items")
if not isinstance(items, list) or len(items) < 2:
    raise SystemExit("favorites output must contain prompt row plus at least one quote row")

quote_row = items[1]
icon_path = (
    quote_row.get("icon", {}).get("path")
    if isinstance(quote_row, dict)
    else None
)
if not isinstance(icon_path, str) or not icon_path.strip():
    raise SystemExit("favorite quote row missing icon.path")

icon_file = Path(icon_path).resolve()
if not icon_file.exists():
    raise SystemExit(f"icon file missing: {icon_file}")
if cache_root not in icon_file.parents:
    raise SystemExit(f"icon file not rooted under cache dir: {icon_file}")

print(f"icon={icon_file}")
PY
}

run_check() {
  local label="$1"
  local expected_kind="$2"
  shift 2

  total_count=$((total_count + 1))

  local stdout_file="$tmp_root/${label}.stdout"
  local stderr_file="$tmp_root/${label}.stderr"

  local rc=0
  if (
    cd "$repo_root"
    MARKET_CACHE_DIR="$cache_dir" \
      ALFRED_WORKFLOW_CACHE="$cache_dir" \
      ALFRED_WORKFLOW_DATA="$cache_dir" \
      "$@"
  ) >"$stdout_file" 2>"$stderr_file"; then
    local validation
    if validation="$(validate_contract "$expected_kind" "$stdout_file" 2>&1)"; then
      pass_count=$((pass_count + 1))
      results+=("PASS ${label}: ${validation}")
    else
      fail_count=$((fail_count + 1))
      local reason
      reason="$(printf '%s' "$validation" | tr '\n' ' ' | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//')"
      results+=("FAIL ${label}: invalid contract (${reason})")
    fi
    return
  else
    rc=$?
  fi

  local combined_log="$tmp_root/${label}.combined"
  cat "$stderr_file" "$stdout_file" >"$combined_log" || true

  if is_network_unavailable "$(compact_text "$combined_log")"; then
    skip_count=$((skip_count + 1))
    local note
    note="$(compact_text "$combined_log" | cut -c1-200)"
    if [[ -z "$note" ]]; then
      note="network error without message"
    fi
    results+=("SKIP ${label}: network unavailable (${note})")
    return
  fi

  fail_count=$((fail_count + 1))
  local note
  note="$(compact_text "$combined_log" | cut -c1-200)"
  if [[ -z "$note" ]]; then
    note="command failed without output"
  fi
  results+=("FAIL ${label}: command failed (exit=${rc}; ${note})")
}

run_favorites_icon_check() {
  local label="$1"
  total_count=$((total_count + 1))

  local stdout_file="$tmp_root/${label}.stdout"
  local stderr_file="$tmp_root/${label}.stderr"
  local rc=0

  if (
    cd "$repo_root"
    MARKET_CACHE_DIR="$cache_dir" \
      ALFRED_WORKFLOW_CACHE="$cache_dir" \
      ALFRED_WORKFLOW_DATA="$cache_dir" \
      cargo run -p nils-market-cli -- favorites --list BTC --default-fiat USD --output alfred-json
  ) >"$stdout_file" 2>"$stderr_file"; then
    local validation
    if validation="$(validate_favorites_icon_contract "$stdout_file" "$cache_dir" 2>&1)"; then
      local icon_path
      icon_path="$(printf '%s' "$validation" | sed -n 's/^icon=//p')"
      if [[ "$label" == "favorites-warm" && -n "$last_favorites_icon_path" && "$icon_path" != "$last_favorites_icon_path" ]]; then
        fail_count=$((fail_count + 1))
        results+=("FAIL ${label}: icon path changed between cold and warm runs (${last_favorites_icon_path} -> ${icon_path})")
        return
      fi
      last_favorites_icon_path="$icon_path"
      pass_count=$((pass_count + 1))
      results+=("PASS ${label}: ${validation}")
      return
    fi

    fail_count=$((fail_count + 1))
    local reason
    reason="$(printf '%s' "$validation" | tr '\n' ' ' | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//')"
    results+=("FAIL ${label}: invalid favorites icon contract (${reason})")
    return
  else
    rc=$?
  fi

  local combined_log="$tmp_root/${label}.combined"
  cat "$stderr_file" "$stdout_file" >"$combined_log" || true

  if is_network_unavailable "$(compact_text "$combined_log")"; then
    skip_count=$((skip_count + 1))
    local note
    note="$(compact_text "$combined_log" | cut -c1-200)"
    if [[ -z "$note" ]]; then
      note="network error without message"
    fi
    results+=("SKIP ${label}: network unavailable (${note})")
    return
  fi

  fail_count=$((fail_count + 1))
  local note
  note="$(compact_text "$combined_log" | cut -c1-200)"
  if [[ -z "$note" ]]; then
    note="command failed without output"
  fi
  results+=("FAIL ${label}: command failed (exit=${rc}; ${note})")
}

run_favorites_icon_check "favorites-cold"
run_favorites_icon_check "favorites-warm"
run_check "fx" "fx" cargo run -p nils-market-cli -- fx --base USD --quote JPY --amount 100 --json
run_check "crypto" "crypto" cargo run -p nils-market-cli -- crypto --base BTC --quote USD --amount 0.5 --json

for line in "${results[@]}"; do
  echo "$line"
done
echo "Summary: pass=${pass_count} fail=${fail_count} skip=${skip_count} total=${total_count}"

if ((fail_count > 0)); then
  echo "Result: FAIL"
  exit 1
fi

if ((skip_count > 0)); then
  echo "Result: SKIP (network unavailable; non-blocking live smoke)."
  exit 0
fi

echo "Result: PASS"

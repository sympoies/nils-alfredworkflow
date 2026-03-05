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

TODAY_UID="B1A11A4C-5F5D-4E8D-8E68-2AD5A95E95E1"
WEEK_UID="8A72E2AF-189E-4A13-9A0E-B30FEAF37F9A"
ACTION_UID="E7A2F2B8-9BB0-4F7A-A2A9-9074CBF90AA0"

for required in \
  workflow.toml \
  README.md \
  src/info.plist.template \
  src/assets/icon.png \
  scripts/script_filter_common.sh \
  scripts/generate_weather_icons.sh \
  scripts/script_filter_today.sh \
  scripts/script_filter_week.sh \
  scripts/action_copy.sh \
  tests/smoke.sh; do
  assert_file "$workflow_dir/$required"
done

weather_icons=(
  clear
  mainly-clear
  partly-cloudy
  cloudy
  fog
  drizzle
  rain
  snow
  rain-showers
  snow-showers
  thunderstorm
  unknown
)

for icon in "${weather_icons[@]}"; do
  assert_file "$workflow_dir/src/assets/icons/weather/${icon}.png"
done

for executable in \
  scripts/generate_weather_icons.sh \
  scripts/script_filter_common.sh \
  scripts/script_filter_today.sh \
  scripts/script_filter_week.sh \
  scripts/action_copy.sh \
  tests/smoke.sh; do
  assert_exec "$workflow_dir/$executable"
done

require_bin jq
require_bin rg

manifest="$workflow_dir/workflow.toml"
[[ "$(toml_string "$manifest" id)" == "weather" ]] || fail "workflow id mismatch"
[[ "$(toml_string "$manifest" rust_binary)" == "weather-cli" ]] || fail "rust_binary must be weather-cli"
[[ "$(toml_string "$manifest" script_filter)" == "script_filter_today.sh" ]] || fail "script_filter mismatch"
[[ "$(toml_string "$manifest" action)" == "action_copy.sh" ]] || fail "action mismatch"

if ! rg -n '^WEATHER_CLI_BIN[[:space:]]*=[[:space:]]*""' "$manifest" >/dev/null; then
  fail "WEATHER_CLI_BIN default must be empty"
fi

if ! rg -n '^WEATHER_LOCALE[[:space:]]*=[[:space:]]*"en"' "$manifest" >/dev/null; then
  fail "WEATHER_LOCALE default must be en"
fi

if ! rg -n '^WEATHER_DEFAULT_CITIES[[:space:]]*=[[:space:]]*"Tokyo"' "$manifest" >/dev/null; then
  fail "WEATHER_DEFAULT_CITIES default must be Tokyo"
fi

if ! rg -n '^WEATHER_CACHE_TTL_SECS[[:space:]]*=[[:space:]]*"900"' "$manifest" >/dev/null; then
  fail "WEATHER_CACHE_TTL_SECS default must be 900"
fi

tmp_dir="$(mktemp -d)"
artifact_id="$(toml_string "$manifest" id)"
artifact_version="$(toml_string "$manifest" version)"
artifact_name="$(toml_string "$manifest" name)"
artifact_path="$repo_root/dist/$artifact_id/$artifact_version/${artifact_name}.alfredworkflow"
artifact_sha_path="${artifact_path}.sha256"

release_cli="$repo_root/target/release/weather-cli"
artifact_backup="$(artifact_backup_file "$artifact_path" "$tmp_dir" "$(basename "$artifact_path")")"
artifact_sha_backup="$(artifact_backup_file "$artifact_sha_path" "$tmp_dir" "$(basename "$artifact_sha_path")")"
release_backup="$(artifact_backup_file "$release_cli" "$tmp_dir" "weather-cli.release")"

cleanup() {
  artifact_restore_file "$release_cli" "$release_backup"
  artifact_restore_file "$artifact_path" "$artifact_backup"
  artifact_restore_file "$artifact_sha_path" "$artifact_sha_backup"
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mkdir -p "$tmp_dir/bin" "$tmp_dir/stubs"
workflow_smoke_write_pbcopy_stub "$tmp_dir/bin/pbcopy"
workflow_smoke_assert_action_requires_arg "$workflow_dir/scripts/action_copy.sh"

copy_arg="2026-02-12 Sunny 12.0~18.0C rain:10%"
PBCOPY_STUB_OUT="$tmp_dir/pbcopy-out.txt" PATH="$tmp_dir/bin:$PATH" \
  "$workflow_dir/scripts/action_copy.sh" "$copy_arg"
[[ "$(cat "$tmp_dir/pbcopy-out.txt")" == "$copy_arg" ]] || fail "action_copy.sh must pass exact arg to pbcopy"

cat >"$tmp_dir/stubs/weather-cli-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

period="${1:-}"
shift || true

mode=""
lang=""
city=""
lat=""
lon=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --output)
    mode="${2:-}"
    shift 2
    ;;
  --city)
    city="${2:-}"
    shift 2
    ;;
  --lang)
    lang="${2:-}"
    shift 2
    ;;
  --lat)
    lat="${2:-}"
    shift 2
    ;;
  --lon)
    lon="${2:-}"
    shift 2
    ;;
  *)
    shift
    ;;
  esac
done

[[ "$mode" == "alfred-json" ]] || exit 9

summary="Cloudy"
rain_label="rain"
if [[ "$lang" == "zh" ]]; then
  summary="陰天"
  rain_label="降雨"
fi

location="city:${city}"
timezone="Asia/Tokyo"
lat_out="35.6762"
lon_out="139.6503"
if [[ -n "$lat" || -n "$lon" ]]; then
  location="${lat},${lon}"
  timezone="UTC"
  lat_out="${lat}"
  lon_out="${lon}"
elif [[ -n "$city" ]]; then
  location="$city"
  case "$(printf '%s' "$city" | tr '[:upper:]' '[:lower:]')" in
  tokyo)
    timezone="Asia/Tokyo"
    lat_out="35.6762"
    lon_out="139.6503"
    ;;
  osaka)
    timezone="Asia/Tokyo"
    lat_out="34.6937"
    lon_out="135.5023"
    ;;
  taipei)
    timezone="Asia/Taipei"
    lat_out="25.0330"
    lon_out="121.5654"
    ;;
  *)
    timezone="Asia/Tokyo"
    lat_out="35.0000"
    lon_out="139.0000"
    ;;
  esac
fi

if [[ "$period" == "hourly" ]]; then
  jq -nc \
    --arg location "$location" \
    --arg timezone "$timezone" \
    --arg lat "$lat_out" \
    --arg lon "$lon_out" \
    --arg summary "$summary" \
    --arg rain_label "$rain_label" \
    '{
      items: (
        [
          {
            title: ($location + " (" + $timezone + ")"),
            subtitle: ("source=open_meteo freshness=live lat=" + $lat + " lon=" + $lon),
            arg: $location,
            valid: false
          }
        ]
        + [
          range(0; 4) | {
            title: ("2026-02-12 " + ((if . < 10 then "0" else "" end) + (tostring) + ":00") + " " + $summary + " 12.0°C"),
            subtitle: ($rain_label + ":10%"),
            arg: ("2026-02-12T" + (if . < 10 then "0" else "" end) + (tostring) + ":00"),
            valid: false
          }
        ]
      )
    }'
  exit 0
fi

if [[ "$period" == "week" ]]; then
  jq -nc \
    --arg location "$location" \
    --arg timezone "$timezone" \
    --arg lat "$lat_out" \
    --arg lon "$lon_out" \
    --arg summary "$summary" \
    --arg rain_label "$rain_label" \
    --arg period "$period" \
    '{
      items: (
        [
          {
            title: ($location + " (" + $timezone + ")"),
            subtitle: ("source=open_meteo freshness=live lat=" + $lat + " lon=" + $lon),
            arg: $location,
            valid: false
          }
        ]
        + [
          range(0; 7) | {
            title: ("2026-02-" + ((12 + .) | tostring) + " " + $summary + " 12.0~18.0°C"),
            subtitle: ($rain_label + ":10%"),
            arg: ($period + " forecast"),
            valid: false
          }
        ]
      )
    }'
  exit 0
fi

if [[ "$period" == "today" ]]; then
  printf '{"items":[{"title":"%s (%s)","subtitle":"source=open_meteo freshness=live lat=%s lon=%s","arg":"%s","valid":false},{"title":"2026-02-12 %s 12.0~18.0°C","subtitle":"%s:10%%","arg":"%s forecast","valid":false}]}' "$location" "$timezone" "$lat_out" "$lon_out" "$location" "$summary" "$rain_label" "$period"
  printf '\n'
  exit 0
fi

exit 9
EOS
chmod +x "$tmp_dir/stubs/weather-cli-ok"

cat >"$tmp_dir/stubs/weather-cli-invalid" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "invalid value: city or lat/lon must be provided" >&2
exit 2
EOS
chmod +x "$tmp_dir/stubs/weather-cli-invalid"

cat >"$tmp_dir/stubs/weather-cli-runtime" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
echo "provider timeout" >&2
exit 1
EOS
chmod +x "$tmp_dir/stubs/weather-cli-runtime"

cat >"$tmp_dir/stubs/weather-cli-malformed" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
printf '{"bad":"shape"}\n'
EOS
chmod +x "$tmp_dir/stubs/weather-cli-malformed"

today_stage_one_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "Taipei"; })"
assert_jq_json "$today_stage_one_json" '.items | type == "array" and length == 1' "today stage one should keep original single-row display"
assert_jq_json "$today_stage_one_json" '.items[0].title == "Taipei 12.0~18.0°C cloudy 10%"' "today stage one should keep original today row title"
assert_jq_json "$today_stage_one_json" '.items[0].subtitle == "2026-02-12 Asia/Taipei 25.0330,121.5654"' "today stage one should keep original subtitle format"
assert_jq_json "$today_stage_one_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "today stage one should keep weather icon mapping"
assert_jq_json "$today_stage_one_json" '.items[0].valid == false' "today stage one row must be non-actionable for stage two transition"
assert_jq_json "$today_stage_one_json" '.items[0].autocomplete == "city::Taipei"' "today stage one should add city token for stage two"

today_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$today_stage_two_json" '.items | type == "array" and length == 4' "today stage two must return hourly rows"
assert_jq_json "$today_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C cloudy 10%"' "today stage two should render normalized hourly row"
assert_jq_json "$today_stage_two_json" '.items[0].subtitle == "2026-02-12 Asia/Taipei 25.0330,121.5654"' "today stage two subtitle should show date timezone and coordinates"
assert_jq_json "$today_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "today hourly row should map to weather icon"
assert_jq_json "$today_stage_two_json" '.items[3].title == "Taipei 03:00 12.0°C cloudy 10%"' "today stage two should keep later hourly rows"

today_zh_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_LOCALE="zh" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$today_zh_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C 陰天 10%"' "zh locale should use chinese summary for hourly rows"
assert_jq_json "$today_zh_stage_two_json" '.items[0].subtitle == "2026-02-12 Asia/Taipei 25.0330,121.5654"' "zh locale hourly subtitle should show date timezone and coordinates"
assert_jq_json "$today_zh_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "zh hourly row should map to same cloudy icon"

today_zh_stage_one_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_LOCALE="zh" "$workflow_dir/scripts/script_filter_today.sh" "Taipei"; })"
assert_jq_json "$today_zh_stage_one_json" '.items[0].title == "Taipei 12.0~18.0°C 陰天 10%"' "today stage one zh should keep original localized title"
assert_jq_json "$today_zh_stage_one_json" '.items[0].autocomplete == "city::Taipei"' "today stage one zh should still allow stage two"

week_city_picker_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_week.sh" "Taipei"; })"
assert_jq_json "$week_city_picker_json" '.items | type == "array" and length >= 1' "week stage one should list city candidates"
assert_jq_json "$week_city_picker_json" '.items[0].title == "Taipei"' "week stage one should prioritize typed city"
assert_jq_json "$week_city_picker_json" '.items[0].valid == false' "week stage one item must be non-actionable city picker row"
assert_jq_json "$week_city_picker_json" '.items[0].autocomplete == "city::Taipei"' "week stage one should emit city token autocomplete"

week_env_query_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" alfred_workflow_query="Taipei" "$workflow_dir/scripts/script_filter_week.sh"; })"
assert_jq_json "$week_env_query_json" '.items[0].title == "Taipei"' "week stage one should support Alfred query via env fallback"

week_stdin_query_json="$(printf 'Taipei' | WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_week.sh")"
assert_jq_json "$week_stdin_query_json" '.items[0].title == "Taipei"' "week stage one should support query via stdin fallback"

week_default_picker_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_DEFAULT_CITIES="Tokyo,Osaka" "$workflow_dir/scripts/script_filter_week.sh" "  "; })"
assert_jq_json "$week_default_picker_json" '.items | type == "array" and length == 2' "week empty query should list default cities"
assert_jq_json "$week_default_picker_json" '.items[0].title == "Tokyo"' "week default picker should include Tokyo"
assert_jq_json "$week_default_picker_json" '.items[1].title == "Osaka"' "week default picker should include Osaka"

week_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_week.sh" "city::Taipei"; })"
assert_jq_json "$week_stage_two_json" '.items | type == "array" and length == 7' "week stage two must return fixed 7 rows"
assert_jq_json "$week_stage_two_json" '.items[0].title == "Taipei 12.0~18.0°C cloudy 10%"' "week stage two should render normalized weather row"
assert_jq_json "$week_stage_two_json" '.items[0].subtitle == "2026-02-12 Asia/Taipei 25.0330,121.5654"' "week stage two subtitle should show date timezone and coordinates"
assert_jq_json "$week_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "week stage two should map to weather icon"
assert_jq_json "$week_stage_two_json" '.items[6].subtitle == "2026-02-18 Asia/Taipei 25.0330,121.5654"' "week stage two should keep 7th day row"

week_coordinate_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_week.sh" "city::25.03,121.56"; })"
assert_jq_json "$week_coordinate_stage_two_json" '.items | type == "array" and length == 7' "week coordinate city token should still produce 7 rows"
assert_jq_json "$week_coordinate_stage_two_json" '.items[0].title == "25.03,121.56 12.0~18.0°C cloudy 10%"' "week coordinate stage two should include coordinate location in title"
assert_jq_json "$week_coordinate_stage_two_json" '.items[0].subtitle == "2026-02-12 UTC 25.03,121.56"' "week coordinate stage two subtitle should show UTC coordinates"

empty_today_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "  "; })"
assert_jq_json "$empty_today_json" '.items | type == "array" and length == 1' "empty today query should keep original today result count"
assert_jq_json "$empty_today_json" '.items[0].title == "Tokyo 12.0~18.0°C cloudy 10%"' "empty today query should keep original Tokyo row"
assert_jq_json "$empty_today_json" '.items[0].autocomplete == "city::Tokyo"' "empty today query should add city token for stage two"

empty_today_multi_default_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_DEFAULT_CITIES="Tokyo,Osaka" "$workflow_dir/scripts/script_filter_today.sh" "  "; })"
assert_jq_json "$empty_today_multi_default_json" '.items | type == "array" and length == 2' "multi default cities should keep original row-per-city today display"
assert_jq_json "$empty_today_multi_default_json" 'any(.items[]; .title == "Tokyo 12.0~18.0°C cloudy 10%")' "multi default should include Tokyo today row"
assert_jq_json "$empty_today_multi_default_json" 'any(.items[]; .title == "Osaka 12.0~18.0°C cloudy 10%")' "multi default should include Osaka today row"

multi_city_query_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "Taipei,Tokyo"; })"
assert_jq_json "$multi_city_query_json" '.items | type == "array" and length == 2' "multi-city query should keep original row-per-city today display"
assert_jq_json "$multi_city_query_json" 'any(.items[]; .title == "Taipei 12.0~18.0°C cloudy 10%")' "multi-city query should include Taipei today row"
assert_jq_json "$multi_city_query_json" 'any(.items[]; .title == "Tokyo 12.0~18.0°C cloudy 10%")' "multi-city query should include Tokyo today row"

today_coordinate_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "city::25.03,121.56"; })"
assert_jq_json "$today_coordinate_stage_two_json" '.items | type == "array" and length == 4' "today coordinate city token should still produce hourly rows"
assert_jq_json "$today_coordinate_stage_two_json" '.items[0].title == "25.03,121.56 00:00 12.0°C cloudy 10%"' "today coordinate stage two should include coordinate location in title"
assert_jq_json "$today_coordinate_stage_two_json" '.items[0].subtitle == "2026-02-12 UTC 25.03,121.56"' "today coordinate stage two subtitle should show UTC coordinates"

invalid_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-invalid" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$invalid_json" '.items[0].title == "Invalid location input"' "invalid input title mapping mismatch"
assert_jq_json "$invalid_json" '.items[0].valid == false' "invalid fallback item must be invalid"

runtime_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-runtime" "$workflow_dir/scripts/script_filter_week.sh" "city::Taipei"; })"
assert_jq_json "$runtime_json" '.items[0].title == "Weather provider unavailable"' "runtime failure title mapping mismatch"

malformed_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-malformed" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$malformed_json" '.items[0].title == "Weather output format error"' "malformed output title mapping mismatch"

missing_layout="$tmp_dir/layout-missing"
mkdir -p "$missing_layout/workflows/weather/scripts"
cp "$workflow_dir/scripts/script_filter_common.sh" "$missing_layout/workflows/weather/scripts/script_filter_common.sh"
cp "$workflow_dir/scripts/script_filter_today.sh" "$missing_layout/workflows/weather/scripts/script_filter_today.sh"
cp "$workflow_dir/scripts/script_filter_week.sh" "$missing_layout/workflows/weather/scripts/script_filter_week.sh"
chmod +x "$missing_layout/workflows/weather/scripts/script_filter_common.sh"
chmod +x "$missing_layout/workflows/weather/scripts/script_filter_today.sh"
chmod +x "$missing_layout/workflows/weather/scripts/script_filter_week.sh"
missing_binary_json="$({ WEATHER_CLI_BIN="$missing_layout/does-not-exist/weather-cli" "$missing_layout/workflows/weather/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$missing_binary_json" '.items[0].title == "weather-cli binary not found"' "missing binary title mismatch"

make_layout_cli() {
  local target="$1"
  local marker="$2"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOS
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[{"title":"${marker}","subtitle":"source=open_meteo freshness=live","arg":"layout","valid":false}]}'
printf '\\n'
EOS
  chmod +x "$target"
}

run_layout_check() {
  local mode="$1"
  local marker="$2"
  local layout="$tmp_dir/layout-$mode"

  mkdir -p "$layout/workflows/weather/scripts"
  cp "$workflow_dir/scripts/script_filter_common.sh" "$layout/workflows/weather/scripts/script_filter_common.sh"
  cp "$workflow_dir/scripts/script_filter_today.sh" "$layout/workflows/weather/scripts/script_filter_today.sh"
  cp "$workflow_dir/scripts/script_filter_week.sh" "$layout/workflows/weather/scripts/script_filter_week.sh"
  chmod +x "$layout/workflows/weather/scripts/script_filter_common.sh"
  chmod +x "$layout/workflows/weather/scripts/script_filter_today.sh"
  chmod +x "$layout/workflows/weather/scripts/script_filter_week.sh"

  case "$mode" in
  packaged)
    make_layout_cli "$layout/workflows/weather/bin/weather-cli" "$marker"
    ;;
  release)
    make_layout_cli "$layout/target/release/weather-cli" "$marker"
    ;;
  debug)
    make_layout_cli "$layout/target/debug/weather-cli" "$marker"
    ;;
  *)
    fail "unsupported layout mode: $mode"
    ;;
  esac

  local output
  output="$("$layout"/workflows/weather/scripts/script_filter_today.sh "city::Taipei")"
  assert_jq_json "$output" ".items[0].title == \"$marker\"" "script_filter failed to resolve $mode weather-cli path"
}

run_layout_check packaged packaged-cli
run_layout_check release release-cli
run_layout_check debug debug-cli

cat >"$tmp_dir/bin/cargo" <<EOS
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$#" -eq 4 && "\$1" == "build" && "\$2" == "--release" && "\$3" == "-p" && "\$4" == "nils-weather-cli" ]]; then
  mkdir -p "$repo_root/target/release"
  cat >"$repo_root/target/release/weather-cli" <<'EOCLI'
#!/usr/bin/env bash
set -euo pipefail
printf '{"items":[]}\n'
EOCLI
  chmod +x "$repo_root/target/release/weather-cli"
  exit 0
fi

if [[ "\$#" -ge 4 && "\$1" == "run" && "\$2" == "-p" && "\$3" == "nils-workflow-readme-cli" && "\$4" == "--" ]]; then
  exit 0
fi

echo "unexpected cargo invocation: \$*" >&2
exit 1
EOS
chmod +x "$tmp_dir/bin/cargo"

PATH="$tmp_dir/bin:$PATH" "$repo_root/scripts/workflow-pack.sh" --id weather >/dev/null

packaged_dir="$repo_root/build/workflows/weather/pkg"
packaged_plist="$packaged_dir/info.plist"
assert_file "$packaged_plist"
assert_file "$packaged_dir/icon.png"
assert_file "$packaged_dir/assets/icon.png"
assert_file "$packaged_dir/bin/weather-cli"
assert_file "$artifact_path"
assert_file "$artifact_sha_path"

for icon in "${weather_icons[@]}"; do
  assert_file "$packaged_dir/assets/icons/weather/${icon}.png"
done

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$packaged_plist" >/dev/null || fail "packaged plist lint failed"
fi

packaged_json_file="$tmp_dir/packaged.json"
plist_to_json "$packaged_plist" >"$packaged_json_file"

assert_jq_file "$packaged_json_file" '.objects | length >= 3' "packaged plist missing object graph"
assert_jq_file "$packaged_json_file" '.connections | length >= 2' "packaged plist missing connections"

assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TODAY_UID\") | .config.scriptfile == \"./scripts/script_filter_today.sh\"" "today script filter scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TODAY_UID\") | .config.keyword == \"wt||weather\"" "today keyword must be wt||weather"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$TODAY_UID\") | .config.scriptargtype == 1" "today script filter must pass query via argv"

assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$WEEK_UID\") | .config.scriptfile == \"./scripts/script_filter_week.sh\"" "week script filter scriptfile mismatch"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$WEEK_UID\") | .config.keyword == \"ww\"" "week keyword must be ww"
assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$WEEK_UID\") | .config.scriptargtype == 1" "week script filter must pass query via argv"

assert_jq_file "$packaged_json_file" ".objects[] | select(.uid==\"$ACTION_UID\") | .config.scriptfile == \"./scripts/action_copy.sh\"" "copy action scriptfile mismatch"

assert_jq_file "$packaged_json_file" ".connections[\"$TODAY_UID\"] | any(.destinationuid == \"$ACTION_UID\" and .modifiers == 0)" "missing today->copy enter connection"
assert_jq_file "$packaged_json_file" ".connections[\"$WEEK_UID\"] | any(.destinationuid == \"$ACTION_UID\" and .modifiers == 0)" "missing week->copy enter connection"

assert_jq_file "$packaged_json_file" '[.userconfigurationconfig[] | .variable] | sort == ["WEATHER_CACHE_TTL_SECS", "WEATHER_CLI_BIN", "WEATHER_DEFAULT_CITIES", "WEATHER_LOCALE"]' "user configuration variables mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="WEATHER_CLI_BIN") | .config.required == false' "WEATHER_CLI_BIN must be optional"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="WEATHER_LOCALE") | .config.default == "en"' "WEATHER_LOCALE default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="WEATHER_DEFAULT_CITIES") | .config.default == "Tokyo"' "WEATHER_DEFAULT_CITIES default mismatch"
assert_jq_file "$packaged_json_file" '.userconfigurationconfig[] | select(.variable=="WEATHER_CACHE_TTL_SECS") | .config.default == "900"' "WEATHER_CACHE_TTL_SECS default mismatch"

echo "ok: weather workflow smoke test"

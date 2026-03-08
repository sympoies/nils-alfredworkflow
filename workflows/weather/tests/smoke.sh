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
  TROUBLESHOOTING.md \
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

for required in \
  README.md \
  crates/weather-cli/README.md \
  crates/weather-cli/docs/README.md \
  crates/weather-cli/docs/workflow-contract.md; do
  assert_file "$repo_root/$required"
done

weather_icons=(
  clear-day
  clear-night
  mainly-clear-day
  mainly-clear-night
  partly-cloudy-day
  partly-cloudy-night
  cloudy
  cloudy-night
  fog
  fog-night
  drizzle
  drizzle-night
  rain
  rain-night
  snow
  snow-night
  rain-showers
  rain-showers-night
  snow-showers
  snow-showers-night
  thunderstorm
  thunderstorm-night
  unknown
  unknown-night
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
require_bin npx

markdown_docs=(
  "$repo_root/README.md"
  "$repo_root/crates/weather-cli/README.md"
  "$repo_root/crates/weather-cli/docs/README.md"
  "$repo_root/crates/weather-cli/docs/workflow-contract.md"
  "$workflow_dir/README.md"
  "$workflow_dir/TROUBLESHOOTING.md"
)

npx --yes markdownlint-cli2@0.21.0 \
  --config "$repo_root/.markdownlint-cli2.jsonc" \
  "${markdown_docs[@]}"

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
cities=()
lat=""
lon=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --output)
    mode="${2:-}"
    shift 2
    ;;
  --city)
    cities+=("${2:-}")
    shift 2
    ;;
  --lang)
    lang="${2:-}"
    shift 2
    ;;
  --lang=*)
    lang="${1#--lang=}"
    shift
    ;;
  --lat)
    lat="${2:-}"
    shift 2
    ;;
  --lat=*)
    lat="${1#--lat=}"
    shift
    ;;
  --lon)
    lon="${2:-}"
    shift 2
    ;;
  --lon=*)
    lon="${1#--lon=}"
    shift
    ;;
  *)
    shift
    ;;
  esac
done

[[ "$mode" == "alfred-json" ]] || exit 9

summary="Cloudy"
summary_en="Cloudy"
rain_label="rain"
timezone_display="Asia/Tokyo (UTC+9)"
utc_offset_label="UTC+9"

resolve_stub_context() {
  local city="${1:-}"

  summary_en="Cloudy"
  location="city:${city}"
  timezone="Asia/Tokyo"
  timezone_display="Asia/Tokyo (UTC+9)"
  utc_offset_label="UTC+9"
  lat_out="35.6762"
  lon_out="139.6503"

  if [[ -n "$lat" || -n "$lon" ]]; then
    location="${lat},${lon}"
    lat_out="${lat}"
    lon_out="${lon}"
    case "${lat},${lon}" in
    35.6762,139.6503 | 35.68,139.69)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      ;;
    34.6937,135.5023)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      ;;
    25.0330,121.5654 | 25.03,121.56)
      timezone="Asia/Taipei"
      timezone_display="Asia/Taipei (UTC+8)"
      utc_offset_label="UTC+8"
      ;;
    *)
      timezone="UTC"
      timezone_display="UTC (UTC+0)"
      utc_offset_label="UTC+0"
      ;;
    esac
  elif [[ -n "$city" ]]; then
    location="$city"
    case "$(printf '%s' "$city" | tr '[:upper:]' '[:lower:]')" in
    tokyo)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      lat_out="35.6762"
      lon_out="139.6503"
      ;;
    osaka)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      lat_out="34.6937"
      lon_out="135.5023"
      ;;
    kyoto)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      lat_out="35.0116"
      lon_out="135.7681"
      summary_en="Mainly clear"
      ;;
    taipei)
      timezone="Asia/Taipei"
      timezone_display="Asia/Taipei (UTC+8)"
      utc_offset_label="UTC+8"
      lat_out="25.0330"
      lon_out="121.5654"
      ;;
    taichung)
      timezone="Asia/Taipei"
      timezone_display="Asia/Taipei (UTC+8)"
      utc_offset_label="UTC+8"
      lat_out="24.1477"
      lon_out="120.6736"
      summary_en="Partly cloudy"
      ;;
    "los angeles")
      timezone="America/Los_Angeles"
      timezone_display="America/Los_Angeles (UTC-8)"
      utc_offset_label="UTC-8"
      lat_out="34.0522"
      lon_out="-118.2437"
      summary_en="Clear sky"
      ;;
    *)
      timezone="Asia/Tokyo"
      timezone_display="Asia/Tokyo (UTC+9)"
      utc_offset_label="UTC+9"
      lat_out="35.0000"
      lon_out="139.0000"
      ;;
    esac
  fi
}

weather_code_for_summary() {
  case "$1" in
  "Clear sky")
    printf '0'
    ;;
  "Mainly clear")
    printf '1'
    ;;
  "Partly cloudy")
    printf '2'
    ;;
  *)
    printf '3'
    ;;
  esac
}

summary_zh_from_en() {
  case "$1" in
  "Clear sky")
    printf '晴朗'
    ;;
  "Mainly clear")
    printf '大致晴朗'
    ;;
  "Partly cloudy")
    printf '晴時多雲'
    ;;
  *)
    printf '陰天'
    ;;
  esac
}

day_icon_key_for_summary() {
  case "$1" in
  "Clear sky")
    printf 'clear-day'
    ;;
  "Mainly clear")
    printf 'mainly-clear-day'
    ;;
  "Partly cloudy")
    printf 'partly-cloudy-day'
    ;;
  *)
    printf 'cloudy'
    ;;
  esac
}

night_icon_key_for_summary() {
  case "$1" in
  "Clear sky")
    printf 'clear-night'
    ;;
  "Mainly clear")
    printf 'mainly-clear-night'
    ;;
  "Partly cloudy")
    printf 'partly-cloudy-night'
    ;;
  *)
    printf 'cloudy-night'
    ;;
  esac
}

is_night_hour() {
  local hour="$1"
  ((10#$hour < 6 || 10#$hour >= 18))
}

current_icon_key_for_summary() {
  local summary="$1"
  local override="${WEATHER_ICON_LOCAL_HOUR_OVERRIDE:-}"

  override="$(printf '%s' "$override" | tr -d '[:space:]')"
  if [[ "$override" =~ ^[0-9]{1,2}$ ]] && ((10#$override >= 0 && 10#$override <= 23)) && is_night_hour "$override"; then
    night_icon_key_for_summary "$summary"
    return 0
  fi

  day_icon_key_for_summary "$summary"
}

hourly_icon_key_for_summary() {
  local summary="$1"
  local hour="$2"

  if is_night_hour "$hour"; then
    night_icon_key_for_summary "$summary"
    return 0
  fi

  day_icon_key_for_summary "$summary"
}

city="${cities[0]:-}"
resolve_stub_context "$city"

weather_code="$(weather_code_for_summary "$summary_en")"
summary="$summary_en"
if [[ "$lang" == "zh" ]]; then
  summary="$(summary_zh_from_en "$summary_en")"
  rain_label="降雨"
fi

if [[ "$period" == "today" && ${#cities[@]} -gt 1 ]]; then
  items=()
  for city in "${cities[@]}"; do
    resolve_stub_context "$city"
    weather_code="$(weather_code_for_summary "$summary_en")"
    summary="$summary_en"
    if [[ "$lang" == "zh" ]]; then
      summary="$(summary_zh_from_en "$summary_en")"
      display_summary="$summary"
      rain_label="降雨"
    else
      display_summary="$(printf '%s' "$summary" | tr '[:upper:]' '[:lower:]')"
      rain_label="rain"
    fi

    current_icon_key="$(current_icon_key_for_summary "$summary_en")"
    item="$(jq -nc \
      --arg location "$location" \
      --arg timezone "$timezone" \
      --arg timezone_display "$timezone_display" \
      --arg utc_offset_label "$utc_offset_label" \
      --arg lat "$lat_out" \
      --arg lon "$lon_out" \
      --arg lang "$lang" \
      --arg summary "$summary" \
      --arg display_summary "$display_summary" \
      --arg icon_key "$current_icon_key" \
      --arg rain_label "$rain_label" \
      --argjson weather_code "$weather_code" \
      '{
        title: ($location + " " + (if $lang == "zh" then "週四" else "Thu" end) + " 12.0~18.0°C " + $display_summary + " 10%"),
        subtitle: ("2026-02-12 " + (if $lang == "zh" then "週四" else "Thu" end) + " " + $timezone + " " + $lat + "," + $lon),
        arg: "2026-02-12",
        valid: true,
        icon: {
          path: ("assets/icons/weather/" + $icon_key + ".png")
        },
        weather_meta: {
          item_kind: "daily",
          date: "2026-02-12",
          date_with_weekday: ("2026-02-12 " + (if $lang == "zh" then "週四" else "Thu" end)),
          weekday_label: (if $lang == "zh" then "週四" else "Thu" end),
          summary: $summary,
          weather_code: $weather_code,
          icon_key: $icon_key,
          is_night: ($icon_key | endswith("-night")),
          temp_min_c_label: "12.0",
          temp_max_c_label: "18.0",
          precip_prob_max_pct_label: "10",
          location_name: $location,
          timezone: $timezone,
          timezone_display: $timezone_display,
          utc_offset_label: $utc_offset_label,
          latitude_label: $lat,
          longitude_label: $lon
        }
      }')"
    items+=("$item")
  done

  printf '%s\n' "${items[@]}" | jq -sc '{items: .}'
  exit 0
fi

if [[ "$period" == "hourly" ]]; then
  jq -nc \
    --arg location "$location" \
    --arg timezone "$timezone" \
    --arg timezone_display "$timezone_display" \
    --arg utc_offset_label "$utc_offset_label" \
    --arg lat "$lat_out" \
    --arg lon "$lon_out" \
    --arg lang "$lang" \
    --arg summary "$summary" \
    --arg summary_en "$summary_en" \
    --arg rain_label "$rain_label" \
    --argjson weather_code "$weather_code" \
    '
      def hourly_icon($summary_en; $hour):
        if $summary_en == "Clear sky" then
          (if $hour < 6 or $hour >= 18 then "clear-night" else "clear-day" end)
        elif $summary_en == "Mainly clear" then
          (if $hour < 6 or $hour >= 18 then "mainly-clear-night" else "mainly-clear-day" end)
        elif $summary_en == "Partly cloudy" then
          (if $hour < 6 or $hour >= 18 then "partly-cloudy-night" else "partly-cloudy-day" end)
        else
          (if $hour < 6 or $hour >= 18 then "cloudy-night" else "cloudy" end)
        end;
      {
      items: (
        [
          {
            title: ($location + " (" + $timezone + ")"),
            subtitle: ("source=open_meteo freshness=live lat=" + $lat + " lon=" + $lon),
            arg: $location,
            valid: false,
            weather_meta: {
              item_kind: "header",
              location_name: $location,
              timezone: $timezone,
              timezone_display: $timezone_display,
              latitude_label: $lat,
              longitude_label: $lon
            }
          }
        ]
        + [
          range(0; 4) as $hour_index
          | ((if $hour_index < 10 then "0" else "" end) + ($hour_index | tostring)) as $hour
          | (hourly_icon($summary_en; ($hour | tonumber))) as $icon_key
          | {
              title: ("2026-02-12 " + $hour + ":00 " + $summary + " 12.0°C"),
              subtitle: ($rain_label + ":10%"),
              arg: ("2026-02-12T" + $hour + ":00"),
              valid: false,
              icon: {
                path: ("assets/icons/weather/" + $icon_key + ".png")
              },
              weather_meta: {
                item_kind: "hourly",
                date: "2026-02-12",
                date_with_weekday: ("2026-02-12 " + (if $lang == "zh" then "週四" else "Thu" end)),
                weekday_label: (if $lang == "zh" then "週四" else "Thu" end),
                timezone_display: $timezone_display,
                utc_offset_label: $utc_offset_label,
                time: ($hour + ":00"),
                datetime: ("2026-02-12T" + $hour + ":00"),
                summary: $summary,
                weather_code: $weather_code,
                icon_key: $icon_key,
                is_night: ($icon_key | endswith("-night")),
                temp_c_label: "12.0",
                precip_prob_pct_label: "10"
              }
            }
        ]
      )
      }
    '
  exit 0
fi

if [[ "$period" == "week" ]]; then
  jq -nc \
    --arg location "$location" \
    --arg timezone "$timezone" \
    --arg timezone_display "$timezone_display" \
    --arg utc_offset_label "$utc_offset_label" \
    --arg lat "$lat_out" \
    --arg lon "$lon_out" \
    --arg lang "$lang" \
    --arg summary "$summary" \
    --arg summary_en "$summary_en" \
    --arg rain_label "$rain_label" \
    --arg period "$period" \
    --argjson weather_code "$weather_code" \
    '
      def daily_icon($summary_en):
        if $summary_en == "Clear sky" then "clear-day"
        elif $summary_en == "Mainly clear" then "mainly-clear-day"
        elif $summary_en == "Partly cloudy" then "partly-cloudy-day"
        else "cloudy"
        end;
      def weekday_label($day_index; $lang):
        if $lang == "zh" then
          ["週四", "週五", "週六", "週日", "週一", "週二", "週三"][$day_index]
        else
          ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][$day_index]
        end;
      {
      items: (
        [
          {
            title: ($location + " (" + $timezone + ")"),
            subtitle: ("source=open_meteo freshness=live lat=" + $lat + " lon=" + $lon),
            arg: $location,
            valid: false,
            weather_meta: {
              item_kind: "header",
              location_name: $location,
              timezone: $timezone,
              timezone_display: $timezone_display,
              latitude_label: $lat,
              longitude_label: $lon
            }
          }
        ]
        + [
          range(0; 7) as $day_index
          | (daily_icon($summary_en)) as $icon_key
          | ("2026-02-" + ((12 + $day_index) | tostring)) as $date
          | {
              title: ($date + " " + $summary + " 12.0~18.0°C"),
              subtitle: ($rain_label + ":10%"),
              arg: ($period + " forecast"),
              valid: false,
              icon: {
                path: ("assets/icons/weather/" + $icon_key + ".png")
              },
              weather_meta: {
                item_kind: "daily",
                date: $date,
                date_with_weekday: ($date + " " + weekday_label($day_index; $lang)),
                weekday_label: weekday_label($day_index; $lang),
                timezone_display: $timezone_display,
                utc_offset_label: $utc_offset_label,
                summary: $summary,
                weather_code: $weather_code,
                icon_key: $icon_key,
                is_night: false,
                temp_min_c_label: "12.0",
                temp_max_c_label: "18.0",
                precip_prob_max_pct_label: "10"
              }
            }
        ]
      )
      }
    '
  exit 0
fi

if [[ "$period" == "today" ]]; then
  current_icon_key="$(current_icon_key_for_summary "$summary_en")"
  jq -nc \
    --arg location "$location" \
    --arg timezone "$timezone" \
    --arg timezone_display "$timezone_display" \
    --arg utc_offset_label "$utc_offset_label" \
    --arg lat "$lat_out" \
    --arg lon "$lon_out" \
    --arg lang "$lang" \
    --arg summary "$summary" \
    --arg rain_label "$rain_label" \
    --arg period "$period" \
    --arg icon_key "$current_icon_key" \
    --argjson weather_code "$weather_code" \
    '{
      items: [
        {
          title: ($location + " (" + $timezone + ")"),
          subtitle: ("source=open_meteo freshness=live lat=" + $lat + " lon=" + $lon),
          arg: $location,
          valid: false,
          weather_meta: {
            item_kind: "header",
            location_name: $location,
            timezone: $timezone,
            timezone_display: $timezone_display,
            latitude_label: $lat,
            longitude_label: $lon
          }
        },
        {
          title: ("2026-02-12 " + $summary + " 12.0~18.0°C"),
          subtitle: ($rain_label + ":10%"),
          arg: ($period + " forecast"),
          valid: false,
          icon: {
            path: ("assets/icons/weather/" + $icon_key + ".png")
          },
          weather_meta: {
            item_kind: "daily",
            date: "2026-02-12",
            date_with_weekday: ("2026-02-12 " + (if $lang == "zh" then "週四" else "Thu" end)),
            weekday_label: (if $lang == "zh" then "週四" else "Thu" end),
            timezone_display: $timezone_display,
            utc_offset_label: $utc_offset_label,
            summary: $summary,
            weather_code: $weather_code,
            icon_key: $icon_key,
            is_night: ($icon_key | endswith("-night")),
            temp_min_c_label: "12.0",
            temp_max_c_label: "18.0",
            precip_prob_max_pct_label: "10"
          }
        }
      ]
    }'
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

cat >"$tmp_dir/stubs/weather-cli-city-fails-latlon-ok" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

period="${1:-}"
shift || true

city=""
lat=""
lon=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --city)
    city="${2:-}"
    shift 2
    ;;
  --lat=*)
    lat="${1#--lat=}"
    shift
    ;;
  --lon=*)
    lon="${1#--lon=}"
    shift
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

if [[ "$period" != "hourly" ]]; then
  echo "unsupported period" >&2
  exit 9
fi

if [[ -n "$city" ]]; then
  echo "geocode network lookup should not happen" >&2
  exit 1
fi

if [[ "$lat" != "25.033" || "$lon" != "121.5654" ]]; then
  echo "missing cached coordinates" >&2
  exit 1
fi

jq -nc '
  {
    items: [
      {
        title: "25.0330,121.5654 (Asia/Taipei)",
        subtitle: "source=open_meteo freshness=live lat=25.0330 lon=121.5654",
        arg: "25.0330,121.5654",
        valid: false,
        weather_meta: {
          item_kind: "header",
          location_name: "25.0330,121.5654",
          timezone: "Asia/Taipei",
          timezone_display: "Asia/Taipei (UTC+8)",
          latitude_label: "25.0330",
          longitude_label: "121.5654"
        }
      },
      {
        title: "2026-02-12 00:00 Cloudy 12.0°C",
        subtitle: "rain:10%",
        arg: "2026-02-12T00:00",
        valid: false,
        icon: { path: "assets/icons/weather/cloudy-night.png" },
        weather_meta: {
          item_kind: "hourly",
          date: "2026-02-12",
          date_with_weekday: "2026-02-12 Thu",
          weekday_label: "Thu",
          timezone_display: "Asia/Taipei (UTC+8)",
          utc_offset_label: "UTC+8",
          time: "00:00",
          datetime: "2026-02-12T00:00",
          summary: "Cloudy",
          weather_code: 3,
          icon_key: "cloudy-night",
          is_night: true,
          temp_c_label: "12.0",
          precip_prob_pct_label: "10"
        }
      }
    ]
  }
'
EOS
chmod +x "$tmp_dir/stubs/weather-cli-city-fails-latlon-ok"

mkdir -p "$tmp_dir/empty-cache"
export ALFRED_WORKFLOW_CACHE="$tmp_dir/empty-cache"

today_stage_one_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "Taipei"; })"
assert_jq_json "$today_stage_one_json" '.items | type == "array" and length == 1' "today stage one should keep original single-row display"
assert_jq_json "$today_stage_one_json" '.items[0].title == "Taipei 12.0~18.0°C cloudy 10%"' "today stage one title should not include weekday"
assert_jq_json "$today_stage_one_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "today stage one subtitle should show timezone with UTC offset"
assert_jq_json "$today_stage_one_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "today stage one should keep weather icon mapping"
assert_jq_json "$today_stage_one_json" '.items[0].valid == false' "today stage one row must be non-actionable for stage two transition"
assert_jq_json "$today_stage_one_json" '.items[0].autocomplete == "city::Taipei"' "today stage one should add city token for stage two"

today_stage_two_query="$(jq -r '.items[0].autocomplete' <<<"$today_stage_one_json")"
today_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "$today_stage_two_query"; })"
assert_jq_json "$today_stage_two_json" '.items | type == "array" and length == 4' "today stage two must return hourly rows"
assert_jq_json "$today_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C cloudy 10%"' "today stage two title should not include weekday"
assert_jq_json "$today_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "today stage two subtitle should show timezone with UTC offset"
assert_jq_json "$today_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy-night.png"' "today hourly row should map to night weather icon after dark"
assert_jq_json "$today_stage_two_json" '.items[3].title == "Taipei 03:00 12.0°C cloudy 10%"' "today stage two should keep later hourly rows without weekday"

today_legacy_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$today_legacy_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C cloudy 10%"' "today stage two should keep legacy city token compatibility"

mkdir -p "$tmp_dir/cache/weather-cli/geocode"
cat >"$tmp_dir/cache/weather-cli/geocode/city-taipei.json" <<'EOS'
{"name":"Taipei","latitude":25.033,"longitude":121.5654,"timezone":"Asia/Taipei"}
EOS
today_cached_city_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-city-fails-latlon-ok" ALFRED_WORKFLOW_CACHE="$tmp_dir/cache" "$workflow_dir/scripts/script_filter_today.sh" "city::Taipei"; })"
assert_jq_json "$today_cached_city_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C cloudy 10%"' "today stage two should resolve cached city coordinates before hourly fetch"
assert_jq_json "$today_cached_city_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "cached city coordinates should preserve timezone offset subtitle output"

today_clear_day_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_ICON_LOCAL_HOUR_OVERRIDE=10 "$workflow_dir/scripts/script_filter_today.sh" "Los Angeles"; })"
assert_jq_json "$today_clear_day_json" '.items[0].icon.path == "assets/icons/weather/clear-day.png"' "today stage one should keep day clear icon during daytime"

today_clear_night_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_ICON_LOCAL_HOUR_OVERRIDE=22 "$workflow_dir/scripts/script_filter_today.sh" "Los Angeles"; })"
assert_jq_json "$today_clear_night_json" '.items[0].icon.path == "assets/icons/weather/clear-night.png"' "today stage one should use clear night icon after dark"

today_mainly_clear_night_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_ICON_LOCAL_HOUR_OVERRIDE=22 "$workflow_dir/scripts/script_filter_today.sh" "Kyoto"; })"
assert_jq_json "$today_mainly_clear_night_json" '.items[0].icon.path == "assets/icons/weather/mainly-clear-night.png"' "today stage one should use mainly clear night icon after dark"

today_partly_cloudy_night_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_ICON_LOCAL_HOUR_OVERRIDE=22 "$workflow_dir/scripts/script_filter_today.sh" "Taichung"; })"
assert_jq_json "$today_partly_cloudy_night_json" '.items[0].icon.path == "assets/icons/weather/partly-cloudy-night.png"' "today stage one should use partly cloudy night icon after dark"

today_hourly_clear_night_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "city::Los Angeles"; })"
assert_jq_json "$today_hourly_clear_night_json" '.items[0].icon.path == "assets/icons/weather/clear-night.png"' "hourly rows should use night icon for overnight clear weather"

today_la_stage_one_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "Los Angeles"; })"
today_la_stage_two_query="$(jq -r '.items[0].autocomplete' <<<"$today_la_stage_one_json")"
today_la_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "$today_la_stage_two_query"; })"
assert_jq_json "$today_la_stage_two_json" '.items | type == "array" and length == 4' "today stage two should support negative longitude coordinate tokens"
assert_jq_json "$today_la_stage_two_json" '.items[0].title == "Los Angeles 00:00 12.0°C clear sky 10%"' "negative longitude city token stage two should preserve the display location without weekday in title"
assert_jq_json "$today_la_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • America/Los_Angeles (UTC-8) • 34.0522,-118.2437"' "negative longitude city token stage two should keep timezone offset and negative longitude"

today_zh_stage_one_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_LOCALE="zh" "$workflow_dir/scripts/script_filter_today.sh" "Taipei"; })"
assert_jq_json "$today_zh_stage_one_json" '.items[0].title == "Taipei 12.0~18.0°C 陰天 10%"' "today stage one zh title should not include weekday"
assert_jq_json "$today_zh_stage_one_json" '.items[0].autocomplete == "city::Taipei"' "today stage one zh should emit city token for stage two"

today_zh_stage_two_query="$(jq -r '.items[0].autocomplete' <<<"$today_zh_stage_one_json")"
today_zh_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_LOCALE="zh" "$workflow_dir/scripts/script_filter_today.sh" "$today_zh_stage_two_query"; })"
assert_jq_json "$today_zh_stage_two_json" '.items[0].title == "Taipei 00:00 12.0°C 陰天 10%"' "zh locale should use chinese summary without weekday in title"
assert_jq_json "$today_zh_stage_two_json" '.items[0].subtitle == "2026-02-12 週四 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "zh locale hourly subtitle should show timezone with UTC offset"
assert_jq_json "$today_zh_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy-night.png"' "zh hourly row should map to same night cloudy icon"

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
assert_jq_json "$week_stage_two_json" '.items[0].title == "Taipei 12.0~18.0°C cloudy 10%"' "week stage two should render normalized weather row without weekday in title"
assert_jq_json "$week_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "week stage two subtitle should show timezone with UTC offset"
assert_jq_json "$week_stage_two_json" '.items[0].icon.path == "assets/icons/weather/cloudy.png"' "week stage two should map to weather icon"
assert_jq_json "$week_stage_two_json" '.items[6].subtitle == "Wed, Feb 18 • Asia/Taipei (UTC+8) • 25.0330,121.5654"' "week stage two should keep timezone offset on 7th day row"

week_coordinate_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_week.sh" "city::25.03,121.56"; })"
assert_jq_json "$week_coordinate_stage_two_json" '.items | type == "array" and length == 7' "week coordinate city token should still produce 7 rows"
assert_jq_json "$week_coordinate_stage_two_json" '.items[0].title == "25.03,121.56 12.0~18.0°C cloudy 10%"' "week coordinate stage two should include coordinate location without weekday in title"
assert_jq_json "$week_coordinate_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.03,121.56"' "week coordinate stage two subtitle should show timezone with UTC offset"

empty_today_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "  "; })"
assert_jq_json "$empty_today_json" '.items | type == "array" and length == 1' "empty today query should keep original today result count"
assert_jq_json "$empty_today_json" '.items[0].title == "Tokyo 12.0~18.0°C cloudy 10%"' "empty today query should keep default Tokyo row title without weekday"
assert_jq_json "$empty_today_json" '.items[0].autocomplete == "city::Tokyo"' "empty today query should add city token for stage two"

empty_today_multi_default_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" WEATHER_DEFAULT_CITIES="Tokyo,Osaka" "$workflow_dir/scripts/script_filter_today.sh" "  "; })"
assert_jq_json "$empty_today_multi_default_json" '.items | type == "array" and length == 2' "multi default cities should keep original row-per-city today display"
assert_jq_json "$empty_today_multi_default_json" 'any(.items[]; .title == "Tokyo 12.0~18.0°C cloudy 10%")' "multi default should include Tokyo today row"
assert_jq_json "$empty_today_multi_default_json" 'any(.items[]; .title == "Osaka 12.0~18.0°C cloudy 10%")' "multi default should include Osaka today row"

multi_city_query_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "Taipei,Tokyo"; })"
assert_jq_json "$multi_city_query_json" '.items | type == "array" and length == 2' "multi-city query should keep original row-per-city today display"
assert_jq_json "$multi_city_query_json" 'any(.items[]; .title == "Taipei 12.0~18.0°C cloudy 10%")' "multi-city query should include Taipei today row"
assert_jq_json "$multi_city_query_json" 'any(.items[]; .title == "Tokyo 12.0~18.0°C cloudy 10%")' "multi-city query should include Tokyo today row"
assert_jq_json "$multi_city_query_json" 'any(.items[]; .subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.0330,121.5654")' "multi-city query should include timezone offset subtitle"

today_coordinate_stage_two_json="$({ WEATHER_CLI_BIN="$tmp_dir/stubs/weather-cli-ok" "$workflow_dir/scripts/script_filter_today.sh" "city::25.03,121.56"; })"
assert_jq_json "$today_coordinate_stage_two_json" '.items | type == "array" and length == 4' "today coordinate city token should still produce hourly rows"
assert_jq_json "$today_coordinate_stage_two_json" '.items[0].title == "25.03,121.56 00:00 12.0°C cloudy 10%"' "today coordinate stage two should include coordinate location without weekday in title"
assert_jq_json "$today_coordinate_stage_two_json" '.items[0].subtitle == "Thu, Feb 12 • Asia/Taipei (UTC+8) • 25.03,121.56"' "today coordinate stage two subtitle should show timezone with UTC offset"

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

#!/usr/bin/env bash
set -euo pipefail

DEFAULT_CITY_FALLBACK="Tokyo"
DEFAULT_LOCALE_FALLBACK="en"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
helper_loader=""
for candidate in \
  "$script_dir/lib/workflow_helper_loader.sh" \
  "$script_dir/../../../scripts/lib/workflow_helper_loader.sh"; do
  if [[ -f "$candidate" ]]; then
    helper_loader="$candidate"
    break
  fi
done

if [[ -z "$helper_loader" ]]; then
  cwd_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$cwd_repo_root" && -f "$cwd_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    helper_loader="$cwd_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi

if [[ -z "$helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$helper_loader"

if ! wfhl_source_helper "$script_dir" "script_filter_error_json.sh" auto; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate script_filter_error_json.sh runtime helper.","valid":false}]}\n'
  exit 0
fi

if ! wfhl_source_helper "$script_dir" "workflow_cli_resolver.sh" auto; then
  sfej_emit_error_item_json "Workflow helper missing" "Cannot locate workflow_cli_resolver.sh runtime helper."
  exit 0
fi

trim_query() {
  local value="${1-}"
  printf '%s' "$value" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

period_title() {
  case "$1" in
  today)
    printf 'Today'
    ;;
  week)
    printf '7-Day'
    ;;
  hourly)
    printf 'Hourly'
    ;;
  *)
    printf 'Weather'
    ;;
  esac
}

emit_single_item() {
  local title="$1"
  local subtitle="$2"
  local valid="$3"
  sfej_emit_single_item_json "$title" "$subtitle" "$valid"
}

print_error_item() {
  local period="$1"
  local raw_message="${2:-weather-cli failed}"
  local message
  local prefix

  prefix="$(period_title "$period")"
  message="$(sfej_normalize_error_message "$raw_message")"
  [[ -n "$message" ]] || message="weather-cli failed"

  local title="${prefix} forecast error"
  local subtitle="$message"
  local lower
  lower="$(printf '%s' "$message" | tr '[:upper:]' '[:lower:]')"

  if [[ "$lower" == *"binary not found"* ]]; then
    title="weather-cli binary not found"
    subtitle="Package workflow or set WEATHER_CLI_BIN to a weather-cli executable."
  elif [[ "$lower" == *"city or lat/lon"* || "$lower" == *"requires either city"* || "$lower" == *"invalid value"* || "$lower" == *"invalid float"* || "$lower" == *"must be provided together"* ]]; then
    title="Invalid location input"
    subtitle="Use city name or lat,lon coordinates (for example 25.03,121.56)."
  elif [[ "$lower" == *"geocod"* || "$lower" == *"city not found"* || "$lower" == *"no locations found"* ]]; then
    title="Location not found"
    subtitle="Try a more specific city name, or use lat,lon coordinates."
  elif [[ "$lower" == *"provider"* || "$lower" == *"upstream"* || "$lower" == *"429"* || "$lower" == *"503"* ]]; then
    title="Weather provider unavailable"
    subtitle="Upstream weather provider failed. Retry shortly."
  elif [[ "$lower" == *"timeout"* || "$lower" == *"timed out"* || "$lower" == *"io error"* || "$lower" == *"internal error"* || "$lower" == *"panic"* ]]; then
    title="Weather runtime failure"
    subtitle="weather-cli failed while fetching forecast. Retry shortly."
  elif [[ "$lower" == *"malformed alfred json"* ]]; then
    title="Weather output format error"
    subtitle="weather-cli returned malformed Alfred JSON."
  fi

  emit_single_item "$title" "$subtitle" false
}

resolve_weather_cli() {
  wfcr_resolve_binary \
    "WEATHER_CLI_BIN" \
    "$script_dir/../bin/weather-cli" \
    "$repo_root/target/release/weather-cli" \
    "$repo_root/target/debug/weather-cli" \
    "weather-cli binary not found (checked WEATHER_CLI_BIN/package/release/debug paths)"
}

parse_lat_lon() {
  local query="$1"
  if [[ "$query" =~ ^[[:space:]]*([+-]?[0-9]+([.][0-9]+)?)[[:space:]]*,[[:space:]]*([+-]?[0-9]+([.][0-9]+)?)[[:space:]]*$ ]]; then
    printf '%s\n%s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

split_city_csv() {
  local value="${1-}"
  printf '%s' "$value" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | sed '/^$/d'
}

resolve_locale() {
  local raw="${1-}"
  local lowered

  lowered="$(printf '%s' "$raw" | tr '[:upper:]' '[:lower:]' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
  case "$lowered" in
  "" | en | english)
    printf 'en'
    ;;
  zh | zh-tw | zh-cn | zh-hant | zh-hans | chinese)
    printf 'zh'
    ;;
  *)
    printf 'en'
    ;;
  esac
}

normalize_alfred_items() {
  local json_output="$1"

  if ! command -v jq >/dev/null 2>&1; then
    printf '%s\n' "$json_output"
    return 0
  fi

  jq -ce '
    def icon_name($summary):
      if $summary == "clear sky" or $summary == "晴朗" then
        "clear"
      elif $summary == "mainly clear" or $summary == "大致晴朗" then
        "mainly-clear"
      elif $summary == "partly cloudy" or $summary == "晴時多雲" then
        "partly-cloudy"
      elif $summary == "cloudy" or $summary == "陰天" then
        "cloudy"
      elif $summary == "fog" or $summary == "有霧" then
        "fog"
      elif $summary == "drizzle" or $summary == "毛毛雨" then
        "drizzle"
      elif $summary == "rain" or $summary == "降雨" then
        "rain"
      elif $summary == "snow" or $summary == "降雪" then
        "snow"
      elif $summary == "rain showers" or $summary == "陣雨" then
        "rain-showers"
      elif $summary == "snow showers" or $summary == "陣雪" then
        "snow-showers"
      elif $summary == "thunderstorm" or $summary == "雷雨" then
        "thunderstorm"
      elif $summary == "unknown weather" or $summary == "天氣狀態未知" then
        "unknown"
      else
        "unknown"
      end;

    if (.items | type != "array") then
      error("missing items array")
    else
      if (.items | length) < 2 then
        .items |= map(
          . + {
            "valid": false,
            "arg": (if ((.arg // "") | length) == 0 then (.title // "") else .arg end)
          }
        )
      else
        (if ((.items[0].title // "") | test("^.+ \\([^)]+\\)$"))
         then ((.items[0].title // "") | capture("^(?<location>.+) \\((?<timezone>[^)]+)\\)$"))
         else null
         end) as $header
        | ($header.location // ((.items[0].title // "") | sub(" \\([^)]*\\)$"; ""))) as $location
        | ($header.timezone // "UTC") as $timezone
        | (if ((.items[0].subtitle // "") | test("lat=-?[0-9]+(?:\\.[0-9]+)? lon=-?[0-9]+(?:\\.[0-9]+)?"))
           then ((.items[0].subtitle // "") | capture("lat=(?<lat>-?[0-9]+(?:\\.[0-9]+)?) lon=(?<lon>-?[0-9]+(?:\\.[0-9]+)?)"))
           else null
           end) as $coords
        | ($coords.lat // "?") as $lat
        | ($coords.lon // "?") as $lon
        | .items = (
            .items[1:]
            | map(
                (.title // "") as $title
                | (.subtitle // "") as $subtitle
                | (if ($title | test("^(?<date>[^ ]+) (?<summary>.+) (?<min>-?[0-9]+(?:\\.[0-9]+)?)~(?<max>-?[0-9]+(?:\\.[0-9]+)?)°C$"))
                   then ($title | capture("^(?<date>[^ ]+) (?<summary>.+) (?<min>-?[0-9]+(?:\\.[0-9]+)?)~(?<max>-?[0-9]+(?:\\.[0-9]+)?)°C$"))
                   else null
                   end) as $daily
                | (if ($title | test("^(?<date>[^ ]+)[ T](?<time>[0-9]{2}:[0-9]{2}) (?<summary>.+) (?<temp>-?[0-9]+(?:\\.[0-9]+)?)°C$"))
                   then ($title | capture("^(?<date>[^ ]+)[ T](?<time>[0-9]{2}:[0-9]{2}) (?<summary>.+) (?<temp>-?[0-9]+(?:\\.[0-9]+)?)°C$"))
                   else null
                   end) as $hourly
                | (if ($subtitle | test("[0-9]+%"))
                   then ($subtitle | capture("(?<rain>[0-9]+)%"))
                   else null
                   end) as $rain
                | if $daily != null then
                    ($daily.summary | if test("^[A-Za-z ]+$") then ascii_downcase else . end) as $summary
                    | {
                        "title": ($location + " " + $daily.min + "~" + $daily.max + "°C " + $summary + " " + (($rain.rain // "?") + "%")),
                        "subtitle": ($daily.date + " " + $timezone + " " + $lat + "," + $lon),
                        "arg": (if ((.arg // "") | length) == 0 then $daily.date else .arg end),
                        "valid": true,
                        "icon": {
                          "path": ("assets/icons/weather/" + icon_name($summary) + ".png")
                        }
                      }
                  elif $hourly != null then
                    ($hourly.summary | if test("^[A-Za-z ]+$") then ascii_downcase else . end) as $summary
                    | {
                        "title": ($location + " " + $hourly.time + " " + $hourly.temp + "°C " + $summary + " " + (($rain.rain // "?") + "%")),
                        "subtitle": ($hourly.date + " " + $timezone + " " + $lat + "," + $lon),
                        "arg": (if ((.arg // "") | length) == 0 then ($hourly.date + " " + $hourly.time) else .arg end),
                        "valid": true,
                        "icon": {
                          "path": ("assets/icons/weather/" + icon_name($summary) + ".png")
                        }
                      }
                  else
                    {
                      "title": ($location + " " + $title),
                      "subtitle": ($timezone + " " + $lat + "," + $lon),
                      "arg": (if ((.arg // "") | length) == 0 then $title else .arg end),
                      "valid": true,
                      "icon": {
                        "path": "assets/icons/weather/unknown.png"
                      }
                    }
                  end
              )
          )
      end
    end
  ' <<<"$json_output"
}

period="${1:-}"
query="${2:-}"

case "$period" in
today | week | hourly) ;;
*)
  emit_single_item "Weather workflow error" "Invalid period: $period" false
  exit 0
  ;;
esac

trimmed_query="$(trim_query "$query")"
output_locale="$(resolve_locale "${WEATHER_LOCALE:-$DEFAULT_LOCALE_FALLBACK}")"

err_file="${TMPDIR:-/tmp}/weather-script-filter.err.$$"
trap 'rm -f "$err_file"' EXIT

weather_cli=""
if ! weather_cli="$(resolve_weather_cli 2>"$err_file")"; then
  err_msg="$(cat "$err_file")"
  print_error_item "$period" "$err_msg"
  exit 0
fi

if [[ -n "$trimmed_query" ]] && lat_lon="$(parse_lat_lon "$trimmed_query")"; then
  lat="$(printf '%s\n' "$lat_lon" | sed -n '1p')"
  lon="$(printf '%s\n' "$lat_lon" | sed -n '2p')"

  if json_output="$("$weather_cli" "$period" --output alfred-json --lang "$output_locale" --lat "$lat" --lon "$lon" 2>"$err_file")"; then
    if [[ -z "$json_output" ]]; then
      print_error_item "$period" "weather-cli returned empty response"
      exit 0
    fi

    if ! normalized_output="$(normalize_alfred_items "$json_output" 2>/dev/null)"; then
      print_error_item "$period" "weather-cli returned malformed Alfred JSON"
      exit 0
    fi

    printf '%s\n' "$normalized_output"
    exit 0
  fi

  err_msg="$(cat "$err_file")"
  print_error_item "$period" "$err_msg"
  exit 0
fi

city_csv="$trimmed_query"
if [[ -z "$city_csv" ]]; then
  city_csv="$(trim_query "${WEATHER_DEFAULT_CITIES:-$DEFAULT_CITY_FALLBACK}")"
  [[ -n "$city_csv" ]] || city_csv="$DEFAULT_CITY_FALLBACK"
fi

mapfile -t city_targets < <(split_city_csv "$city_csv")
if [[ ${#city_targets[@]} -eq 0 ]]; then
  city_targets=("$DEFAULT_CITY_FALLBACK")
fi

if [[ ${#city_targets[@]} -gt 1 ]] && ! command -v jq >/dev/null 2>&1; then
  emit_single_item "Missing jq for multi-city mode" "Install jq or query a single city." false
  exit 0
fi

if [[ ${#city_targets[@]} -eq 1 ]]; then
  city="${city_targets[0]}"
  if json_output="$("$weather_cli" "$period" --output alfred-json --lang "$output_locale" --city "$city" 2>"$err_file")"; then
    if [[ -z "$json_output" ]]; then
      print_error_item "$period" "weather-cli returned empty response"
      exit 0
    fi

    if ! normalized_output="$(normalize_alfred_items "$json_output" 2>/dev/null)"; then
      print_error_item "$period" "weather-cli returned malformed Alfred JSON"
      exit 0
    fi

    printf '%s\n' "$normalized_output"
    exit 0
  fi

  err_msg="$(cat "$err_file")"
  print_error_item "$period" "$err_msg"
  exit 0
fi

item_arrays=()
for city in "${city_targets[@]}"; do
  if json_output="$("$weather_cli" "$period" --output alfred-json --lang "$output_locale" --city "$city" 2>"$err_file")"; then
    if [[ -z "$json_output" ]]; then
      message="weather-cli returned empty response"
      error_item="$(jq -nc --arg city "$city" --arg message "$message" '{title: ($city + ": forecast error"), subtitle: $message, valid: false}')"
      item_arrays+=("[$error_item]")
      continue
    fi

    if ! normalized_output="$(normalize_alfred_items "$json_output" 2>/dev/null)"; then
      message="weather-cli returned malformed Alfred JSON"
      error_item="$(jq -nc --arg city "$city" --arg message "$message" '{title: ($city + ": forecast error"), subtitle: $message, valid: false}')"
      item_arrays+=("[$error_item]")
      continue
    fi

    city_items="$(jq -ce '.items' <<<"$normalized_output" 2>/dev/null || true)"
    if [[ -n "$city_items" ]]; then
      item_arrays+=("$city_items")
      continue
    fi

    message="weather-cli returned malformed Alfred JSON"
    error_item="$(jq -nc --arg city "$city" --arg message "$message" '{title: ($city + ": forecast error"), subtitle: $message, valid: false}')"
    item_arrays+=("[$error_item]")
    continue
  fi

  err_msg="$(cat "$err_file")"
  message="$(sfej_normalize_error_message "$err_msg")"
  [[ -n "$message" ]] || message="weather-cli failed"
  error_item="$(jq -nc --arg city "$city" --arg message "$message" '{title: ($city + ": forecast error"), subtitle: $message, valid: false}')"
  item_arrays+=("[$error_item]")
done

if [[ ${#item_arrays[@]} -eq 0 ]]; then
  print_error_item "$period" "weather-cli returned no city outputs"
  exit 0
fi

combined_output="$(printf '%s\n' "${item_arrays[@]}" | jq -sc '{items: map(.[]) }')"
printf '%s\n' "$combined_output"

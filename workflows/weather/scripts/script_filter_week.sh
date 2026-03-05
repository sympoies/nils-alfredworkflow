#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_CITY_FALLBACK="Tokyo"
CITY_TOKEN_PREFIX="city::"

workflow_helper_loader="$script_dir/lib/workflow_helper_loader.sh"
if [[ ! -f "$workflow_helper_loader" ]]; then
  workflow_helper_loader="$script_dir/../../../scripts/lib/workflow_helper_loader.sh"
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  git_repo_root="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
  if [[ -n "$git_repo_root" && -f "$git_repo_root/scripts/lib/workflow_helper_loader.sh" ]]; then
    workflow_helper_loader="$git_repo_root/scripts/lib/workflow_helper_loader.sh"
  fi
fi
if [[ ! -f "$workflow_helper_loader" ]]; then
  printf '{"items":[{"title":"Workflow helper missing","subtitle":"Cannot locate workflow_helper_loader.sh runtime helper.","valid":false}]}\n'
  exit 0
fi
# shellcheck disable=SC1090
source "$workflow_helper_loader"

if ! wfhl_source_required_helper "$script_dir" "script_filter_query_policy.sh" auto "json"; then
  exit 0
fi

trim_query() {
  local value="${1-}"
  printf '%s' "$value" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

json_escape() {
  local value="${1-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

split_city_csv() {
  local value="${1-}"
  printf '%s' "$value" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | sed '/^$/d'
}

is_lat_lon_query() {
  local value="${1-}"
  [[ "$value" =~ ^[[:space:]]*[+-]?[0-9]+([.][0-9]+)?[[:space:]]*,[[:space:]]*[+-]?[0-9]+([.][0-9]+)?[[:space:]]*$ ]]
}

contains_lower() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    if [[ "$item" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

emit_city_picker_items() {
  local raw_query="${1-}"
  local trimmed_query
  local defaults_csv
  local lowered_query
  local item

  trimmed_query="$(trim_query "$raw_query")"
  defaults_csv="$(trim_query "${WEATHER_DEFAULT_CITIES:-$DEFAULT_CITY_FALLBACK}")"
  [[ -n "$defaults_csv" ]] || defaults_csv="$DEFAULT_CITY_FALLBACK"

  mapfile -t default_cities < <(split_city_csv "$defaults_csv")
  if [[ ${#default_cities[@]} -eq 0 ]]; then
    default_cities=("$DEFAULT_CITY_FALLBACK")
  fi

  city_candidates=()
  if [[ -z "$trimmed_query" ]]; then
    city_candidates=("${default_cities[@]}")
  elif is_lat_lon_query "$trimmed_query"; then
    city_candidates=("$trimmed_query")
  elif [[ "$trimmed_query" == *","* ]]; then
    mapfile -t city_candidates < <(split_city_csv "$trimmed_query")
    if [[ ${#city_candidates[@]} -eq 0 ]]; then
      city_candidates=("$trimmed_query")
    fi
  else
    lowered_query="$(printf '%s' "$trimmed_query" | tr '[:upper:]' '[:lower:]')"
    for item in "${default_cities[@]}"; do
      if [[ "$(printf '%s' "$item" | tr '[:upper:]' '[:lower:]')" == *"$lowered_query"* ]]; then
        city_candidates+=("$item")
      fi
    done
    city_candidates=("$trimmed_query" "${city_candidates[@]}")
  fi

  unique_cities=()
  seen_lowers=()
  for item in "${city_candidates[@]}"; do
    item="$(trim_query "$item")"
    [[ -n "$item" ]] || continue

    local lowered_item
    lowered_item="$(printf '%s' "$item" | tr '[:upper:]' '[:lower:]')"
    if contains_lower "$lowered_item" "${seen_lowers[@]}"; then
      continue
    fi
    seen_lowers+=("$lowered_item")
    unique_cities+=("$item")
  done

  if [[ ${#unique_cities[@]} -eq 0 ]]; then
    unique_cities=("$DEFAULT_CITY_FALLBACK")
  fi

  if command -v jq >/dev/null 2>&1; then
    printf '%s\n' "${unique_cities[@]}" | jq -Rsc --arg token "$CITY_TOKEN_PREFIX" '
      split("\n")
      | map(select(length > 0))
      | {
          items: map({
            title: .,
            subtitle: "Select city to view 7-day forecast",
            autocomplete: ($token + .),
            arg: .,
            valid: false
          })
        }
    '
    return 0
  fi

  printf '{"items":['
  local first=1
  for item in "${unique_cities[@]}"; do
    if [[ "$first" -eq 0 ]]; then
      printf ','
    fi
    first=0
    printf '{"title":"%s","subtitle":"Select city to view 7-day forecast","autocomplete":"%s","arg":"%s","valid":false}' \
      "$(json_escape "$item")" \
      "$(json_escape "${CITY_TOKEN_PREFIX}${item}")" \
      "$(json_escape "$item")"
  done
  printf ']}\n'
}

query="$(sfqp_resolve_query_input "${1:-}")"
trimmed_query="$(trim_query "$query")"

if [[ "$trimmed_query" == "${CITY_TOKEN_PREFIX}"* ]]; then
  selected_city="$(trim_query "${trimmed_query#"${CITY_TOKEN_PREFIX}"}")"
  if [[ -z "$selected_city" ]]; then
    emit_city_picker_items ""
    exit 0
  fi

  if week_json="$("$script_dir/script_filter_common.sh" week "$selected_city")"; then
    if command -v jq >/dev/null 2>&1; then
      jq -ce 'if (.items | type) == "array" then .items |= .[:7] else . end' <<<"$week_json"
      exit 0
    fi
    printf '%s\n' "$week_json"
    exit 0
  fi

  # script_filter_common should always return JSON error rows; keep a fallback.
  emit_city_picker_items "$selected_city"
  exit 0
fi

emit_city_picker_items "$trimmed_query"

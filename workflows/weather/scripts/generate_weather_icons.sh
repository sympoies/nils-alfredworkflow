#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source_dir="$(cd "$script_dir/../assets-src/icons/weather" && pwd)"
out_dir="$(cd "$script_dir/../src/assets" && pwd)/icons/weather"

if ! command -v image-processing >/dev/null 2>&1; then
  echo "image-processing binary not found; cannot generate weather icons" >&2
  exit 1
fi

mkdir -p "$out_dir"

mapfile -t svg_files < <(find "$source_dir" -maxdepth 1 -type f -name '*.svg' | sort)
if [[ ${#svg_files[@]} -eq 0 ]]; then
  echo "no source SVG files found under $source_dir" >&2
  exit 1
fi

rm -f "$out_dir"/*.png

for svg_path in "${svg_files[@]}"; do
  icon_name="$(basename "$svg_path" .svg)"
  image-processing convert \
    --from-svg "$svg_path" \
    --to png \
    --out "$out_dir/${icon_name}.png" \
    --overwrite >/dev/null
done

printf 'ok: generated %s weather icons in %s\n' "${#svg_files[@]}" "$out_dir"

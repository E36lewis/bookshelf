#!/usr/bin/env bash
# Renders the macOS app icon (apple/icon/AppIcon.svg) into every size the
# asset catalog wants: Bookshelf/Assets.xcassets/AppIcon.appiconset.
# The PNGs are committed, so builds don't need this; run it after changing
# the SVG. Needs rsvg-convert (librsvg: `brew install librsvg`, or
# `apt install librsvg2-bin`) or Inkscape.
set -euo pipefail
apple="$(cd "$(dirname "$0")/.." && pwd)"
svg="$apple/icon/AppIcon.svg"
catalog="$apple/Bookshelf/Assets.xcassets"
out="$catalog/AppIcon.appiconset"

render() { # size file
  if command -v rsvg-convert >/dev/null; then
    rsvg-convert --width "$1" --height "$1" --output "$2" "$svg"
  elif command -v inkscape >/dev/null; then
    # Absolute paths: a sandboxed (snap) Inkscape resolves relative ones elsewhere.
    inkscape "$svg" --export-type=png --export-filename="$2" -w "$1" -h "$1" 2>/dev/null
  else
    echo "Needs rsvg-convert or inkscape to render $svg." >&2
    exit 1
  fi
}

rm -rf "$out"
mkdir -p "$out"
cat >"$catalog/Contents.json" <<'JSON'
{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

# The macOS sizes: 16, 32, 128, 256 and 512 points, at 1x and 2x.
images=""
for points in 16 32 128 256 512; do
  for scale in 1 2; do
    pixels=$((points * scale))
    suffix=""
    if [ "$scale" = 2 ]; then suffix="@2x"; fi
    file="icon_${points}x${points}${suffix}.png"
    render "$pixels" "$out/$file"
    images+="${images:+,
}    {
      \"filename\" : \"$file\",
      \"idiom\" : \"mac\",
      \"scale\" : \"${scale}x\",
      \"size\" : \"${points}x${points}\"
    }"
  done
done

cat >"$out/Contents.json" <<JSON
{
  "images" : [
$images
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

# Smaller files, if oxipng is around (lossless).
if command -v oxipng >/dev/null; then
  oxipng -q -o 4 --strip safe "$out"/*.png
fi
echo "Wrote $out"

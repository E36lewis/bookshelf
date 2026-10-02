#!/usr/bin/env bash
# Renders the Windows app icon, windows/Bookshelf/Assets/Bookshelf.ico,
# from the Linux icon (packaging/com.bookshelf.Bookshelf.svg): the same
# book on a tile that fills the square, which stays sharp at the 16 and
# 24 px the taskbar and title bar use. (The Mac icon sits on Apple's icon
# grid, with a margin and a shadow that blur at those sizes.)
# The .ico is committed, so builds don't need this; run it after changing
# the SVG. Needs rsvg-convert (`apt install librsvg2-bin`) or Inkscape,
# and python3 (standard library only).
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
svg="$repo/packaging/com.bookshelf.Bookshelf.svg"
ico="$repo/windows/Bookshelf/Assets/Bookshelf.ico"

# Under target/ (ignored by git), not /tmp: a sandboxed (snap) Inkscape
# can't write to the real /tmp.
mkdir -p "$repo/target"
work="$(mktemp -d "$repo/target/make-icon.XXXXXX")"
trap 'rm -rf "$work"' EXIT

render() { # size file
  if command -v rsvg-convert >/dev/null; then
    rsvg-convert --width "$1" --height "$1" --output "$2" "$svg"
  elif command -v inkscape >/dev/null; then
    inkscape "$svg" --export-type=png --export-filename="$2" -w "$1" -h "$1" 2>/dev/null
  else
    echo "Needs rsvg-convert or inkscape to render $svg." >&2
    exit 1
  fi
}

# Windows' sizes: 16-64 for 100-200% display scaling (title bar, taskbar,
# Explorer's lists), 256 for large icons. Biggest first, so a reader that
# only takes the first image takes the sharpest.
sizes="256 64 48 40 32 24 20 16"
for size in $sizes; do render "$size" "$work/$size.png"; done
if command -v oxipng >/dev/null; then
  oxipng -q -o 4 --strip safe "$work"/*.png
fi

# Every size as a PNG inside the .ico (Windows Vista and later read them),
# with 1 plane and 32 bits per pixel in the directory, as Windows expects.
mkdir -p "$(dirname "$ico")"
python3 - "$ico" "$work" $sizes <<'PY'
import struct
import sys

ico, work, sizes = sys.argv[1], sys.argv[2], [int(s) for s in sys.argv[3:]]
images = []
for size in sizes:
    with open(f"{work}/{size}.png", "rb") as f:
        png = f.read()
    if png[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", png[16:24]) != (size, size):
        sys.exit(f"{size}.png isn't a {size} x {size} PNG")
    images.append((size, png))

header = struct.pack("<HHH", 0, 1, len(images))  # reserved, type 1 (icon), count
offset = len(header) + 16 * len(images)
entries, data = b"", b""
for size, png in images:
    # width, height (0 means 256), colors, reserved, planes, bits, bytes, offset
    entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset + len(data))
    data += png
with open(ico, "wb") as f:
    f.write(header + entries + data)
PY
echo "Wrote $ico ($(wc -c <"$ico") bytes)"

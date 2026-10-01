#!/usr/bin/env bash
# Builds dist/Bookshelf-<version>-x86_64.AppImage
#
#   bash packaging/build-appimage.sh
#
# Build on Ubuntu 24.04 (needs libadwaita 1.5). The result runs on systems with
# an equal or newer glibc. See packaging/README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

ID=com.bookshelf.Bookshelf
BUILD="$PWD/target/appimage"
APPDIR="$BUILD/AppDir"
TOOLS="$BUILD/tools"
DIST="$PWD/dist"

# --- sanity checks ---------------------------------------------------------
if [ -n "${CONDA_PREFIX:-}" ]; then
  echo "A conda environment is active. It would leak its libraries into the AppImage." >&2
  echo "Run 'conda deactivate' and try again." >&2
  exit 1
fi

for cmd in cargo pkg-config curl file sha256sum; do
  command -v "$cmd" >/dev/null || {
    echo "Missing '$cmd'. Install the build packages first:" >&2
    echo "  sudo apt install build-essential pkg-config curl file libgtk-4-dev libadwaita-1-dev librsvg2-bin librsvg2-common" >&2
    exit 1
  }
done
pkg-config --exists gtk4 libadwaita-1 || {
  echo "GTK4 / libadwaita development files not found:" >&2
  echo "  sudo apt install libgtk-4-dev libadwaita-1-dev" >&2
  exit 1
}

VERSION=$(grep -m1 '^version' bookshelf-app/Cargo.toml | cut -d'"' -f2)
export VERSION

# --- 1. release build -------------------------------------------------------
echo "==> Building release binary ($VERSION)"
cargo build --release -p bookshelf-app

# --- 2. build tools (downloaded once, cached) -------------------------------
# Pinned to exact versions and checked against known sha256 sums, because
# these get executed. To upgrade: change the version, download the file,
# run sha256sum on it, and update the sum here.
LINUXDEPLOY_TAG=1-alpha-20251107-1
LINUXDEPLOY_SHA256=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
PLUGIN_GTK_COMMIT=7a3fbc31a9e5075073ff8790f26effbac5f84453
PLUGIN_GTK_SHA256=b0f4cbc684a0103a9651f0955b635eaea0096b3a66c0f5a2c2aa337960375171

echo "==> Getting linuxdeploy (cached in target/appimage/tools)"
mkdir -p "$TOOLS"
fetch() { # url dest sha256
  if ! { [ -f "$2" ] && echo "$3  $2" | sha256sum -c --status; }; then
    curl -fsSL "$1" -o "$2.part"
    if ! echo "$3  $2.part" | sha256sum -c --status; then
      echo "Checksum mismatch for $1; refusing to use it." >&2
      rm -f "$2.part"
      exit 1
    fi
    mv "$2.part" "$2"
  fi
  chmod +x "$2"
}
fetch "https://github.com/linuxdeploy/linuxdeploy/releases/download/$LINUXDEPLOY_TAG/linuxdeploy-x86_64.AppImage" \
      "$TOOLS/linuxdeploy-x86_64.AppImage" "$LINUXDEPLOY_SHA256"
fetch "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/$PLUGIN_GTK_COMMIT/linuxdeploy-plugin-gtk.sh" \
      "$TOOLS/linuxdeploy-plugin-gtk.sh" "$PLUGIN_GTK_SHA256"
export PATH="$TOOLS:$PATH"

# --- 3. assemble the AppDir --------------------------------------------------
echo "==> Assembling AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/fonts/bookshelf" \
         "$APPDIR/apprun-hooks"

install -m755 target/release/bookshelf-gtk "$APPDIR/usr/bin/bookshelf-gtk"
install -m644 "packaging/$ID.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/$ID.svg"
sed "s|@EXEC@|bookshelf-gtk|" "packaging/$ID.desktop.in" > "$APPDIR/usr/share/applications/$ID.desktop"
cp -r packaging/fonts/. "$APPDIR/usr/share/fonts/bookshelf/"
install -m755 packaging/apprun-hooks/bookshelf-fonts.sh "$APPDIR/apprun-hooks/bookshelf-fonts.sh"

# linuxdeploy wants a raster icon at the top level; make one if we can.
ICON="packaging/$ID.svg"
if command -v rsvg-convert >/dev/null; then
  rsvg-convert -w 256 -h 256 "packaging/$ID.svg" -o "$BUILD/$ID.png"
  ICON="$BUILD/$ID.png"
fi

# --- 4. bundle libraries and make the AppImage ------------------------------
echo "==> Bundling GTK4 / libadwaita and creating the AppImage"
export DEPLOY_GTK_VERSION=4
export APPIMAGE_EXTRACT_AND_RUN=1   # lets linuxdeploy run without FUSE
mkdir -p "$DIST"
(
  cd "$BUILD"
  linuxdeploy-x86_64.AppImage \
    --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/bookshelf-gtk" \
    --desktop-file "$APPDIR/usr/share/applications/$ID.desktop" \
    --icon-file "$ICON" \
    --plugin gtk \
    --output appimage
)
mv "$BUILD"/Bookshelf*.AppImage "$DIST"/

echo
echo "Done:"
ls -lh "$DIST"/Bookshelf*.AppImage
echo "Run it with:  chmod +x dist/Bookshelf*.AppImage && ./dist/Bookshelf*.AppImage"

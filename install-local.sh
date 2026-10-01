#!/usr/bin/env bash
# Builds Bookshelf and adds it to your app menu (no sudo; installs under ~/.local).
#   bash install-local.sh              install / update
#   bash install-local.sh --uninstall  remove
set -euo pipefail
cd "$(dirname "$0")"

ID=com.bookshelf.Bookshelf
BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
FONTS="$HOME/.local/share/fonts/bookshelf"

refresh_caches() {
  update-desktop-database "$APPS" 2>/dev/null || true
  gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$BIN/bookshelf-gtk" "$APPS/$ID.desktop" "$ICONS/$ID.svg"
  rm -rf "$FONTS"
  fc-cache -f 2>/dev/null || true
  refresh_caches
  echo "Removed Bookshelf launcher. Your data in ~/.local/share/bookshelf is untouched."
  exit 0
fi

if [ -n "${CONDA_PREFIX:-}" ]; then
  echo "Warning: a conda environment is active. If the build fails, run 'conda deactivate' and retry."
fi

cargo build --release -p bookshelf-app

mkdir -p "$BIN" "$APPS" "$ICONS" "$FONTS"
install -m755 target/release/bookshelf-gtk "$BIN/bookshelf-gtk"
install -m644 "packaging/$ID.svg" "$ICONS/$ID.svg"
# Exec= needs the path quoted (spaces etc.), with \ " ` $ escaped per the
# Desktop Entry spec. awk takes it from the environment, so nothing in the
# path is treated as a pattern.
exec_path="$BIN/bookshelf-gtk"
exec_path=${exec_path//\\/\\\\\\\\}  # a literal \ is \\\\ in a quoted Exec
exec_path=${exec_path//\"/\\\"}
exec_path=${exec_path//\`/\\\`}
exec_path=${exec_path//\$/\\\$}
EXEC="\"$exec_path\"" awk '{
  i = index($0, "@EXEC@")
  if (i) $0 = substr($0, 1, i - 1) ENVIRON["EXEC"] substr($0, i + 6)
  print
}' "packaging/$ID.desktop.in" > "$APPS/$ID.desktop"
cp -r packaging/fonts/. "$FONTS/"
fc-cache -f "$FONTS" 2>/dev/null || true
refresh_caches

echo "Done. Press the Super key and type 'Bookshelf' to launch it."
echo "(Right-click it there to 'Add to Favorites' and pin it to the dock.)"

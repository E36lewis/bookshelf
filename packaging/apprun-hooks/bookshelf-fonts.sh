#!/bin/sh
# Sourced by AppRun on every launch. Makes the fonts bundled inside the
# AppImage visible to this app only (nothing is installed on the system).
conf_dir="${XDG_CACHE_HOME:-$HOME/.cache}/bookshelf"
mkdir -p "$conf_dir" 2>/dev/null || true
cat > "$conf_dir/fonts.conf" <<CONF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <dir>$APPDIR/usr/share/fonts/bookshelf</dir>
  <cachedir prefix="xdg">bookshelf-fontconfig</cachedir>
</fontconfig>
CONF
export FONTCONFIG_FILE="$conf_dir/fonts.conf"

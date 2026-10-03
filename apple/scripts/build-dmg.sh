#!/usr/bin/env bash
# Builds Bookshelf.app (Release, universal: Apple silicon + Intel), checks
# it, and packs it into a disk image to drag into Applications:
#
#   apple/scripts/build-dmg.sh Stable    # apple/build/Bookshelf-<version>.dmg
#   apple/scripts/build-dmg.sh Preview   # apple/build/Bookshelf-Preview-<version>.dmg
#
# Stable is the release: "Bookshelf", with the real bundle ID and journal.
# Preview (the default) has its own, so trying it never touches real books.
# The app is ad-hoc signed with its sandbox entitlements, not with a
# Developer ID, so Gatekeeper asks once before opening it (MANUAL.md).
# Needs the xcframework (build-xcframework.sh) and the Xcode project
# (xcodegen generate in apple/). The app stays in build/dd-<channel>.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^ *MARKETING_VERSION: "\(.*\)"$/\1/p' project.yml)
test -n "$version"
channel=${1:-Preview}
case "$channel" in
  Stable) name=Bookshelf bundle_id=io.github.e36lewis.Bookshelf dmg=build/Bookshelf-$version.dmg ;;
  Preview) name="Bookshelf Preview" bundle_id=io.github.e36lewis.Bookshelf.Preview dmg=build/Bookshelf-Preview-$version.dmg ;;
  *) echo "usage: $0 [Stable|Preview]" >&2; exit 2 ;;
esac

derived=build/dd-$channel
xcodebuild build -quiet \
  -project Bookshelf.xcodeproj -scheme Bookshelf -configuration Release \
  -destination 'generic/platform=macOS' -derivedDataPath "$derived" \
  BOOKSHELF_CHANNEL="$channel" ONLY_ACTIVE_ARCH=NO
app=$derived/Build/Products/Release/Bookshelf.app
contents=$app/Contents
plist() { /usr/libexec/PlistBuddy -c "Print :$1" "$contents/Info.plist"; }

fail() { echo "build-dmg.sh: $*" >&2; exit 1; }
# The channel and its bundle ID (and so its sandbox container and journal).
[ "$(plist BookshelfChannel)" = "$channel" ] || fail "BookshelfChannel isn't $channel"
[ "$(plist CFBundleIdentifier)" = "$bundle_id" ] || fail "the bundle ID isn't $bundle_id"
# The version is project.yml's MARKETING_VERSION, not XcodeGen's 1.0.
[ "$(plist CFBundleShortVersionString)" = "$version" ] || fail "the version isn't $version"
# Universal.
lipo "$contents/MacOS/Bookshelf" -verify_arch arm64 x86_64 || fail "not universal"
# The bundled fonts and their licenses.
fonts=$contents/Resources/Fonts
[ "$(find "$fonts" -maxdepth 1 -name '*.ttf' | wc -l)" -eq 8 ] || fail "not all 8 fonts are in the app"
[ -f "$fonts/iA Writer Duo/LICENSE.md" ] && [ -f "$fonts/Source Serif 4/LICENSE.md" ] || fail "a font license is missing"
# The app icon, from the asset catalog.
[ "$(plist CFBundleIconName)" = AppIcon ] && [ -s "$contents/Resources/AppIcon.icns" ] || fail "no app icon"
# Ad-hoc signed (Apple silicon runs nothing unsigned), sealed, sandboxed.
codesign --verify --deep --strict --verbose=2 "$app"
signature=$(codesign -dv "$app" 2>&1)
grep -qx 'Signature=adhoc' <<<"$signature" || fail "not ad-hoc signed"
# PlistBuddy, not plutil: plutil's key paths split on the key's dots.
entitlements=$(mktemp)
codesign -d --entitlements - --xml "$app" >"$entitlements" 2>/dev/null
sandboxed=$(/usr/libexec/PlistBuddy -c "Print :com.apple.security.app-sandbox" "$entitlements" 2>/dev/null || true)
[ "$sandboxed" = true ] || { cat "$entitlements"; fail "not sandboxed"; }
rm -f "$entitlements"

# The disk image: the app (named for its channel, so a preview can sit
# beside the release in Applications) and a link to drop it on.
stage=build/dmg-$channel
rm -rf "$stage" "$dmg"
mkdir -p "$stage"
ditto "$app" "$stage/$name.app"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "$name $version" -srcfolder "$stage" -fs HFS+ -format UDZO "$dmg"
hdiutil verify -quiet "$dmg"
rm -rf "$stage"

echo "Built apple/$dmg from apple/$app"

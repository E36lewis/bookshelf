#!/usr/bin/env bash
# Checks that every part of Bookshelf says the same version, and that it's
# the release tag's (v0.9.0 for 0.9.0) when one is given. Prints
# "version=<x.y.z>" (release.yml adds it to the job's outputs).
#
#   scripts/check-version.sh [tag]
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "check-version.sh: $*" >&2; exit 1; }
cargo_version() { sed -n 's/^version = "\(.*\)"$/\1/p' "$1" | head -n1; }

version=$(cargo_version bookshelf-core/Cargo.toml)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "bookshelf-core/Cargo.toml has no x.y.z version"
same() { [ "$2" = "$version" ] || fail "$1 says '$2', not $version"; }

for crate in bookshelf-app bookshelf-cli bookshelf-ffi tools/uniffi-bindgen; do
  same "$crate/Cargo.toml" "$(cargo_version "$crate/Cargo.toml")"
done
for crate in bookshelf-core bookshelf-app bookshelf-cli bookshelf-ffi uniffi-bindgen; do
  same "Cargo.lock ($crate)" "$(grep -A1 -x "name = \"$crate\"" Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')"
done
same "apple/project.yml (MARKETING_VERSION)" "$(sed -n 's/^ *MARKETING_VERSION: "\(.*\)"$/\1/p' apple/project.yml)"
same "apple/project.yml (CURRENT_PROJECT_VERSION)" "$(sed -n 's/^ *CURRENT_PROJECT_VERSION: "\(.*\)"$/\1/p' apple/project.yml)"
csproj=windows/Bookshelf/Bookshelf.csproj
same "$csproj (Version)" "$(sed -n 's|^ *<Version>\(.*\)</Version>$|\1|p' "$csproj")"
same "$csproj (AssemblyVersion)" "$(sed -n 's|^ *<AssemblyVersion>\(.*\)\.0</AssemblyVersion>$|\1|p' "$csproj")"
same "$csproj (FileVersion)" "$(sed -n 's|^ *<FileVersion>\(.*\)\.0</FileVersion>$|\1|p' "$csproj")"
same windows/Bookshelf/app.manifest \
  "$(sed -n 's|^ *<assemblyIdentity version="\(.*\)\.0" name="Bookshelf.app" />$|\1|p' windows/Bookshelf/app.manifest)"

if [ -n "${1:-}" ] && [ "$1" != "v$version" ]; then
  fail "the tag $1 doesn't match the version, $version (the tag should be v$version)"
fi
echo "version=$version"

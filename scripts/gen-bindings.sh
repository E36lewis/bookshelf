#!/usr/bin/env bash
# Regenerates the Swift and C# bindings for bookshelf-ffi. Run it after
# changing anything exported in bookshelf-ffi/src/lib.rs, and commit the
# result; CI fails if the committed bindings are out of date.
#
# Works on Linux (and macOS): the generators read the compiled library,
# and their output doesn't depend on the platform.
set -euo pipefail
cd "$(dirname "$0")/.."

CS_GEN_REV=e10ce410eb3a10cc19c7928b93ea8d84e038c034 # uniffi-bindgen-cs v0.11.0+v0.31.0
TOOLS=target/tools

cargo build -q -p bookshelf-ffi
case "$(uname -s)" in
  Darwin) lib=target/debug/libbookshelf_ffi.dylib ;;
  *) lib=target/debug/libbookshelf_ffi.so ;;
esac

# Swift: one .swift file for the BookshelfFFI target; the C header and
# module map go into the xcframework (apple/scripts/build-xcframework.sh).
swift_tmp=$(mktemp -d)
cargo run -q -p uniffi-bindgen -- generate --library "$lib" --language swift \
  --out-dir "$swift_tmp" --no-format
cp "$swift_tmp/bookshelf_ffi.swift" apple/BookshelfKit/Sources/BookshelfFFI/
cp "$swift_tmp/bookshelf_ffiFFI.h" apple/BookshelfKit/Headers/
cp "$swift_tmp/bookshelf_ffiFFI.modulemap" apple/BookshelfKit/Headers/module.modulemap
rm -rf "$swift_tmp"

# C#: installed once, pinned to an exact commit, inside target/.
if [ ! -x "$TOOLS/bin/uniffi-bindgen-cs" ]; then
  cargo install -q --locked --git https://github.com/NordSecurity/uniffi-bindgen-cs \
    --rev "$CS_GEN_REV" --root "$TOOLS" uniffi-bindgen-cs
fi
"$TOOLS/bin/uniffi-bindgen-cs" --library "$lib" --out-dir windows/Bookshelf.Core/Generated --no-format

echo "Bindings regenerated."

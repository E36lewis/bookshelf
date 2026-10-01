#!/usr/bin/env bash
# Builds the Rust core as a universal (Apple silicon + Intel) static library
# and wraps it in apple/BookshelfKit/Frameworks/bookshelf_ffiFFI.xcframework.
# macOS only (needs lipo and xcodebuild).
set -euo pipefail
cd "$(dirname "$0")/../.."

export MACOSX_DEPLOYMENT_TARGET=14.0
# Leave symbols in the static library; Xcode strips the final app.
export CARGO_PROFILE_RELEASE_STRIP=false

targets=(aarch64-apple-darwin x86_64-apple-darwin)
for t in "${targets[@]}"; do
  cargo build --release -p bookshelf-ffi --target "$t"
done

work=target/apple
rm -rf "$work" && mkdir -p "$work/headers"
lipo -create \
  "target/aarch64-apple-darwin/release/libbookshelf_ffi.a" \
  "target/x86_64-apple-darwin/release/libbookshelf_ffi.a" \
  -output "$work/libbookshelf_ffi.a"
cp apple/BookshelfKit/Headers/bookshelf_ffiFFI.h apple/BookshelfKit/Headers/module.modulemap "$work/headers/"

out=apple/BookshelfKit/Frameworks/bookshelf_ffiFFI.xcframework
rm -rf "$out"
xcodebuild -create-xcframework -library "$work/libbookshelf_ffi.a" -headers "$work/headers" -output "$out"
echo "Built $out"

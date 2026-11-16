#!/bin/sh
# Builds platen-mobile for iPhone and writes the Swift bindings into Generated/. Xcode runs this before compiling.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/ios/Generated"
lib="$root/target/aarch64-apple-ios/release/libplaten_mobile.a"
mise="${MISE:-$HOME/.local/bin/mise}"
clean() {
    env -i HOME="$HOME" USER="${USER:-}" PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin" \
        sh -c "cd '$root' && $*"
}
clean "'$mise' exec -- cargo rustc -q -p platen-mobile --lib --release --target aarch64-apple-ios --crate-type staticlib"
mkdir -p "$out"
clean "'$mise' exec -- cargo run -q -p platen-mobile --bin uniffi-bindgen -- generate --library '$lib' --language swift --out-dir '$out'"
mv -f "$out/platen_mobileFFI.modulemap" "$out/module.modulemap"

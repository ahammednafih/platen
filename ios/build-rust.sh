#!/bin/sh
# Builds platen-mobile for iPhone (or the simulator, when Xcode builds for it) and writes the Swift bindings into Generated/. Xcode runs this before compiling.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/ios/Generated"
target=aarch64-apple-ios
[ "${PLATFORM_NAME:-}" = iphonesimulator ] && target=aarch64-apple-ios-sim
lib="$root/target/$target/release/libplaten_mobile.a"
mise="${MISE:-$HOME/.local/bin/mise}"
clean() {
    env -i HOME="$HOME" USER="${USER:-}" IPHONEOS_DEPLOYMENT_TARGET=26.0 PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin" \
        sh -c "cd '$root' && $*"
}
clean "'$mise' exec -- cargo rustc -q -p platen-mobile --lib --release --target $target --crate-type staticlib"
mkdir -p "$out"
clean "'$mise' exec -- cargo run -q -p platen-mobile --bin uniffi-bindgen -- generate --library '$lib' --language swift --out-dir '$out'"
mv -f "$out/platen_mobileFFI.modulemap" "$out/module.modulemap"

# Platen for Android

Kotlin and Jetpack Compose (Material 3 Expressive) over the shared Rust core in
[`crates/mobile`](../crates/mobile). The core does the scanning, page edits, export, copy, print and
shrink; this app is the UI plus the few things only Android can do.

## Building

You need JDK 21, the Android SDK with NDK 30, the Android Rust target and `cargo-ndk`:

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Gradle builds `platen-mobile` for arm64 with `cargo ndk` and generates the Kotlin bindings before
compiling, so a Rust change only needs another Gradle build. Test on a real phone on the same Wi-Fi as
the printer; emulators can't see the LAN printer.

The debug build installs as **Platen (Dev)** (`com.kanamlabs.platen.dev`, orange icon) beside the
release build (`com.kanamlabs.platen`).

## Where things are

| File | What |
| --- | --- |
| `AppModel.kt` | Screen state and calls into the core, on a background thread |
| `ScanTab.kt`, `PageTools.kt`, `CopyTab.kt`, `PrintTab.kt`, `FilesTab.kt`, `ShrinkScreen.kt`, `DeviceScreen.kt` | The screens |
| `Theme.kt`, `Ui.kt`, `Common.kt` | Colours, type, the glass and paper surfaces, shared controls, haptics |
| `Discovery.kt` | Finding printers with `NsdManager` (Android blocks the core's own mDNS) |
| `AndroidPdf.kt` | Rendering PDF pages with `PdfRenderer`, handed to the core for printing |
| `SaveFolder.kt` | Saving exports to Documents/Platen (MediaStore) or a folder the user picks |
| `Secrets.kt` | The printer's web password, encrypted with an Android Keystore key |

No extra libraries beyond AndroidX, Compose and JNA (which UniFFI needs). Prefer platform APIs.

## Releases

Pushing a `v*` tag builds a signed APK in the release workflow and attaches it to the GitHub release.
The version name comes from the tag and the version code from the commit count. Signing uses the
`ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD` and `ANDROID_KEY_ALIAS` repository secrets.
Every release must be signed with the same key, or phones can't update.

To build a signed APK locally, point `ANDROID_KEYSTORE`, `ANDROID_KEYSTORE_PASSWORD` and
`ANDROID_KEY_ALIAS` at the release key and run `./gradlew :app:assembleRelease -PversionName=X.Y.Z`.

## Licences

Inter is bundled under the SIL Open Font License; see [`licenses/Inter-OFL.txt`](licenses/Inter-OFL.txt).

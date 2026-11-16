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

### Google Play

The same tag also builds an app bundle. When the `PLAY_TRACK` repository variable is set (`internal`,
`alpha`, `beta` or `production`), the release workflow uploads it to that track, with the R8 mapping
and "What's new" taken from the Android and shared lines of the release notes. Internal and testing
tracks go live straight away; `production` is left as a draft to review and roll out in Play Console.

One-time setup:

1. Play Console → Create app, package `com.kanamlabs.platen`.
2. App integrity → App signing: choose to use your own key and upload the release key exported with
   Play's PEPK tool. Play then signs installs with the same key as the GitHub APKs, so people can move
   between the two without uninstalling. The same key stays the upload key; CI needs no change.
3. Upload the first bundle by hand (Play's API can't create the first release): with the release key
   env vars above, `./gradlew :app:bundleRelease -PversionName=X.Y.Z`, then upload
   `app/build/outputs/bundle/release/app-release.aab` to Internal testing.
4. A Google Cloud service account with the Google Play Android Developer API enabled and a JSON key,
   invited in Play Console → Users and permissions with release permissions for this app. Put the
   JSON in the `PLAY_SERVICE_ACCOUNT_JSON` repository secret.
5. Set the `PLAY_TRACK` repository variable, e.g. `internal`.

## Licences

Inter is bundled under the SIL Open Font License; see [`licenses/Inter-OFL.txt`](licenses/Inter-OFL.txt).

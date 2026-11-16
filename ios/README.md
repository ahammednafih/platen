# Platen for iPhone

SwiftUI over the shared Rust core in [`crates/mobile`](../crates/mobile). The core does the scanning,
page edits, export, copy, print and shrink; this app is the UI plus the few things only iOS can do.

## Building

You need Xcode, [XcodeGen](https://github.com/yonaskolb/XcodeGen) and the iOS Rust target:

```sh
rustup target add aarch64-apple-ios
cd ios
./build-rust.sh        # first time only, so XcodeGen sees the generated bindings
xcodegen generate
xcodebuild -project Platen.xcodeproj -scheme Platen -destination 'id=<iPhone UDID>' \
  -derivedDataPath build -allowProvisioningUpdates build
xcrun devicectl device install app --device <iPhone UDID> build/Build/Products/Debug-iphoneos/Platen.app
```

Xcode runs `build-rust.sh` before compiling: it builds `platen-mobile` as a static library for arm64
iPhones and writes the Swift bindings into `Generated/`. Test on a real iPhone on the printer's Wi-Fi;
there's no simulator build. The Debug build installs as **Platen (Dev)** (`com.kanamlabs.platen.dev`).

## Where things are

| File | What |
| --- | --- |
| `AppModel.swift` | Screen state and calls into the core, off the main thread |
| `ScanTab.swift`, `PageTools.swift`, `CopyTab.swift`, `PrintTab.swift`, `FilesTab.swift`, `DeviceView.swift` | The screens (Shrink is in `FilesTab.swift`) |
| `Common.swift`, `PageViewer.swift` | Shared controls, colours, the page and sheet viewers |
| `Discovery.swift` | Finding printers with `NWBrowser` (`_uscan._tcp`), resolved to IPv4 |
| `Platform.swift` | PDF rendering for the core, thumbnails, the Keychain password, saved files |

Exports go to the app's Documents folder, one folder per scan, which the Files app shows under On My
iPhone › Platen. No libraries beyond the system frameworks.

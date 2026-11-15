# platen-mobile

The phone apps' shared core: app state (the open scan, undo history, settings, the copy in progress)
and commands over `platen-core`, exposed to Kotlin, and later Swift, with
[UniFFI](https://mozilla.github.io/uniffi-rs/).

The Android app builds this with `cargo ndk` from Gradle and generates its bindings there; see
[`android/README.md`](../../android/README.md). Kotlin package: `com.kanamlabs.platen.core`
(`uniffi.toml`).

## Shape

- `Platen` is the one object the app holds. Its methods mirror the desktop app's Tauri commands, so
  both apps behave the same way. They block; call them off the main thread.
- `types.rs` has the records and enums the apps see, converted from the core's types.
- Errors arrive as `PlatenException.Failed(message)`. The messages `stopped` and `wrong-password` are
  meant for the app to handle, not show.

## What stays in the apps

- **Finding printers.** Phones need their own discovery APIs (Android `NsdManager`, iOS `NWBrowser`);
  the app passes the eSCL URL to `connect`.
- **Rendering PDF pages.** The app implements `PdfRenderer` (Android `PdfRenderer`, iOS PDFKit) and
  passes it to the constructor; the core calls it when printing a PDF.
- **The device password.** Each app keeps it in its own keystore and passes it to `device_details`.
- **Where files go for the user.** The core writes into app storage; the app copies exports out.

## Adding a command

1. Add a method on `Platen` in `lib.rs` (or a free function) with `#[uniffi::export]`.
2. Put any new records or enums in `types.rs`, with conversions from the core's types.
3. Rebuild the app; the bindings regenerate.

Keep platform code out of here. If something needs the platform, add a small `with_foreign` trait like
`PdfRenderer` and let each app implement it.

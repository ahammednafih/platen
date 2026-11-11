# Platen

Driverless scan, copy and print for AirScan + AirPrint printers. User-facing overview: `README.md`.
How the device protocols behave: `docs/protocols.md`. Local planning notes, if present: `docs/PLAN.md` (not in the repo).
Contribution rules: `CONTRIBUTING.md`.

## Commands

- Rust is pinned in `mise.toml`; run cargo as `mise exec -- cargo …`.
- Tests: `mise exec -- cargo test --workspace`. Frontend check: `cd app && pnpm check`.
- App: `cd app && pnpm tauri dev`. Package: `pnpm tauri build` → `target/release/bundle/{macos,dmg}`
  (ad-hoc signed, not notarized).
- CLI: `mise exec -- cargo run -p platen-cli -- --help`. Offline checks: `platen --stitch TOP BOTTOM OUT`,
  `platen --fill-file IN OUT`.
- The printer is on the LAN; network commands from a sandbox need the sandbox off.

## Layout

- `crates/core` — the library (crate `platen-core`): `escl`, `native`, `ipp`, `stitch`, `fill`, `pdf`,
  `ocr`, `session`, `copy`, `webui`, `pushscan`, `button`, `snmp`.
- `crates/tui` — the `platen` CLI.
- `app` — Tauri 2 + Svelte 5. Commands in `app/src-tauri/src/*.rs`, typed wrappers in
  `app/src/lib/api.ts`; keep the two in sync.

## Rules

- No vendor names or branding in the UI. Compatibility notes go in the README.
- Don't `cargo fmt` the app crate or reformat `tauri.conf.json`; neither is formatter-clean. Keep diffs
  to your change.
- Page edits write a new image file and never delete the old one; undo relies on that.
- Settings live in `<config>/Platen`, rebuildable files in `<cache>/Platen` (`core/src/device.rs`).

## Device quirks

- eSCL: send `pwg:Version 2.93` + `scan:Intent`, or the device ignores settings. Always fetch
  `NextDocument` until 404, or DELETE the job, or the device stays "Processing".
- eSCL clamps anything above 600 dpi to 300. 1200 dpi goes over the native protocol
  (`core/src/native.rs`, TCP 54921). Inquiry and scan must share one connection, else `-NG 401`.
  A full page at 1200 dpi takes about 2m20s and 40 MB.
- After a Scan-button press the device refuses eSCL (HTTP 500) until a scan starts; button scans use
  `native`. Details: `docs/protocols.md`.
- Web UI (`core/src/webui.rs`, `pushscan.rs`): HTTPS only, self-signed. The password field's name is read
  from the form; success sets `AuthCookie`. Settings forms need a `Referer` from the device's own page
  (403 without). A mode change returns a "restart now?" page whose hidden fields go to `?pc=2`.
  CSV export 403s.
- The device password is in the Keychain under `platen-device-web` (old name `scan-device-web` is read
  as a fallback), through the Keychain API, never the `security` command line.
- Searchable PDF shells out to Tesseract (`core/src/ocr.rs`): a text-only PDF per page at 300 dpi,
  merged over our pages with lopdf. Uses eng+mal when `mal.traineddata` is installed.

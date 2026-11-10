# scan

Driverless scan/copy/print for AirScan + AirPrint printers. Plan and milestone status: `docs/PLAN.md`
(read only the section you need).

- Rust pinned via `mise.toml`; run cargo as `mise exec -- cargo …`.
- App: `cd app && pnpm tauri dev`; frontend check `pnpm check`. Commands live in
  `app/src-tauri/src/commands.rs`, typed wrappers in `app/src/lib/api.ts` — keep them in sync.
- Scanner/printer is on the LAN; network commands from the Claude sandbox need it disabled.
- Test stitching offline with `scan --stitch TOP BOTTOM OUT`, fill with `scan --fill-file IN OUT`.
- eSCL quirks: send `pwg:Version 2.93` + `scan:Intent`, else the device ignores settings; always
  fetch `NextDocument` until 404 / DELETE the job or the device stays "Processing".
- Don't add vendor branding or names to the UI.
- eSCL clamps >600 dpi to 300; 1200 dpi goes over the native protocol (`core/src/native.rs`, TCP 54921).
  Inquiry + scan must share one connection, else `-NG 401`. Full page at 1200 ≈ 2m20s, ~40 MB.

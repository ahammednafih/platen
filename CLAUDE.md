# scan

Driverless scan/copy/print for AirScan + AirPrint printers. Plan and milestone status: `docs/PLAN.md`
(read only the section you need).

- Rust pinned via `mise.toml`; run cargo as `mise exec -- cargo …`.
- Scanner/printer is on the LAN; network commands from the Claude sandbox need it disabled.
- Test stitching offline with `scan --stitch TOP BOTTOM OUT`, fill with `scan --fill-file IN OUT`.
- eSCL quirks: send `pwg:Version 2.93` + `scan:Intent`, else the device ignores settings; always
  fetch `NextDocument` until 404 / DELETE the job or the device stays "Processing".
- Don't add vendor branding or names to the UI.

# Device protocols

What Platen speaks to the device, and the behaviour that isn't in any spec. Standard protocols come
first; the vendor-specific parts were worked out by observation on one inkjet all-in-one and may
differ on other models. Add what you find.

## Scanning: eSCL (AirScan)

Code: `crates/core/src/escl.rs`.

- Send `pwg:Version 2.93` and `scan:Intent` in the scan settings, or the device ignores the rest.
- After a scan, fetch `NextDocument` until it returns 404, or DELETE the job. Otherwise the device
  stays in "Processing" and refuses the next scan.
- Requests above 600 dpi are silently clamped to 300.
- 1-bit JPEG is rejected, so black and white is scanned as gray and thresholded locally.

## Scanning: the native protocol

Code: `crates/core/src/native.rs`. TCP port 54921. Used for 1200 dpi and for Scan-button scans.

- The device greets with `+OK 200`. `-NG 401` means another client still holds the scanner.
- `ESC I` asks what the device will grant for a resolution and mode; `ESC X` starts the scan. Both
  must go over the same connection.
- The reply is a stream of blocks: a type byte, a header, a little-endian length, then the payload.
  Type `0x64` is JPEG data, `0x82` ends a page, `0x80` ends the job.
- A full page at 1200 dpi takes over two minutes and about 40 MB.

## Printing: IPP (AirPrint)

Code: `crates/core/src/ipp.rs`. Pages are rendered locally and sent as JPEG, one job per sheet, which
is what makes page picking, several pages per sheet and manual two-sided printing possible on any
printer. Ink levels come from the `marker-levels` attribute.

Paper type and borderless printing go in a `media-col` collection instead of the plain `media`
keyword: `media-size` (`x-dimension`, `y-dimension` in hundredths of a millimetre), `media-type`
(from `media-type-supported`, e.g. `stationery`, `stationery-inkjet`, `photographic-glossy`, plus a
vendor keyword for its own photo paper) and, for borderless, all four `media-*-margin` members set to
0. The printer advertises borderless by listing 0 in `media-bottom-margin-supported`.

## The Scan button

Code: `crates/core/src/button.rs`, `snmp.rs`, `pushscan.rs`.

1. The device only accepts a registration if SNMP v1/v2c is read-write. The factory setting is
   read-only, and writes are then dropped without an answer.
2. The PC registers with an SNMP SET on `1.3.6.1.4.1.2435.2.3.9.2.11.1.1.0`, using the device's write
   community, with a string of the form
   `TYPE=BR;BUTTON=SCAN;USER="<name>";FUNC=<IMAGE|OCR|EMAIL|FILE>;HOST=<pc-ip>:54925;APPNUM=<1-4>;DURATION=360;BRID=;`.
   The registration expires after `DURATION` seconds, so it is refreshed while the app runs.
3. Panels without a screen send to the PC chosen on the web interface's Scan to PC page, so that is
   set to the registered name.
4. A press arrives as a UDP datagram on port 54925: a 4-byte header, then
   `TYPE=BR;BUTTON=SCAN;USER=…;FUNC=…;…;SEQ=n;`. It is sent twice with the same `SEQ`.
5. Until a scan starts, the device refuses eSCL with HTTP 500. The native protocol works.

## The device's web interface

Code: `crates/core/src/webui.rs`, `pushscan.rs`. HTTPS only, with a self-signed certificate.

- Log in by posting the password field of the status page; its name changes between firmware
  versions, so it is read from the form. Success sets an `AuthCookie`.
- Settings forms need a `Referer` from the device's own page, or they answer 403.
- A change that needs a restart returns a confirmation page; its hidden fields are posted back with
  `?pc=2`, and the device restarts about a minute later.

### Maintenance pages

Every form posts with the session cookie, its own `CSRFToken` and a `Referer` from the same page.
Tried on the test device: one black-only normal clean and one nozzle check sheet.

- `/general/head_cleaning.html` has one form per button. Each posts `pageid=21`, its own `CSRFToken`
  and `btn_def` to `/general/head_cleaning_confirm.html`. `btn_def`: 1–3 black only (normal, strong,
  strongest), 4–6 colour only (same), 7–9 all colours (same), 10 all colours "special" (the most ink).
  The page warns that cleaning with little ink left can damage the machine.
- `head_cleaning_confirm.html` only asks again ("Press [Yes] if you want to start head cleaning"). Its
  Yes posts `pageid=22`, that page's new `CSRFToken` and the same `btn_def` to
  `/general/head_cleaning_confirm.html?pc=12`, which starts cleaning. The reply auto-submits a form
  back to `head_cleaning.html`. IPP stays `idle` throughout; a normal black clean runs about a minute.
- `/general/checkprintquality.html` prints the nozzle check pattern on A4/Letter plain paper: post
  `pageid=19`, `CSRFToken`, `postif_registration_reject=1`, `B1f7=0` to the same page. Accepted, the
  reply's print state (between the `printstate_start` and `printstate_end` hidden inputs) reads
  "Please wait."; while the device is busy (cleaning) the form comes back instead and nothing prints.
  `checkprintqualitymonitor.html` serves the same print state for polling.
- `/general/alignment.html` step 1 prints the alignment sheet the same way (`pageid=20`, `B1f9=0`,
  about 2 minutes); step 2 needs the sheet on the glass and the panel's Mono Copy button, and
  `alignmentmonitor.html` reports progress. Not used by Platen.


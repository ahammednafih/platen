# Platen

Scan, copy and print on a network printer from your Mac, without the vendor's app.

Platen talks to the printer directly over the standard driverless protocols (AirScan and AirPrint), so
there is nothing to install on the printer side and no account to sign in to. It started as a
replacement for a slow, limited vendor app and grew to cover everything that app did, plus the things
it wouldn't.

> Platen is an independent project. It is not affiliated with, endorsed by, or supported by any
> printer manufacturer.

## What it does

**Scan**

- Multi-page scans that save as you go: close the app and pick a draft back up later.
- Every paper size that fits the glass, 100 to 1200 dpi, colour, gray or black and white.
- Long pages: scan the top and bottom halves of a page taller than the glass and Platen joins them.
- Page tools: rotate, straighten, crop, brightness and contrast, fill torn edges, reorder by dragging,
  retake, undo and redo.
- Export to PDF (colour, share-size, print-ready) or JPEG/PNG, with a searchable text layer.
- Shrink any PDF under a size limit, and compare it with the original before saving.
- The printer's own Scan button can scan straight into the app.

**Copy**

- One page, two pages on one sheet, both sides of an ID card on one sheet, or a long page shrunk to fit.
- Copies, colour or black and white, scale, density.

**Print**

- PDFs and images, with the options the system print dialog hides or lacks: pick pages by clicking
  them, odd or even pages, several pages per sheet, reverse order, two-sided printing by hand with a
  prompt to flip the stack.

**Device**

- Ink levels and status at a glance.
- Page counters, maintenance and network details, read from the printer's web page.

## Will it work with my printer?

| Feature | Needs |
| --- | --- |
| Scan, copy, long pages, page tools, export | A scanner that supports AirScan (eSCL). Most network all-in-ones from the last decade do. |
| Print, ink levels | A printer that supports AirPrint (IPP). |
| 1200 dpi scans, device details, Scan button | Brother inkjet all-in-ones. These use the vendor's own protocols, worked out by observation. |

Platen has been developed and tested against one Brother inkjet all-in-one. The standard features
should work elsewhere, but they are untested on other makes. Reports and fixes are welcome.

Platen runs on macOS 13 or later. The core is plain Rust, so Linux and Windows are possible, but
nobody has done that work yet.

## Install

1. Download the latest `.dmg` from [Releases](../../releases).
2. Drag Platen into Applications.
3. The app isn't notarized by Apple, so the first launch needs one extra step: right-click the app and
   choose **Open**, then **Open** again. If macOS still refuses, run:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Platen.app
   ```

For searchable PDFs, install Tesseract. Everything else works without it.

```sh
brew install tesseract          # English
brew install tesseract-lang     # other languages
```

## Using the printer's Scan button

Out of the box, pressing Scan on the printer does nothing useful without the vendor app running.
Platen can take its place:

1. Open **Device details** and tick **Scan into this app with the device's Scan button**.
2. If the printer's network settings access is off (it is by default), Platen shows the steps to turn
   it on, or does it for you. The printer restarts once.
3. Press Scan on the printer. On panels without a screen, press both Copy buttons together.

The page lands in the scan that's open, or a new one. This works while Platen is running.

## Command line

The release also includes `platen`, a terminal app and one-shot tool over the same core:

```sh
platen                               # interactive terminal app
platen --status                      # scanner and printer state, ink levels
platen --scan-to page.jpg --dpi 300  # scan one page to a file
platen --print file.pdf --copies 2   # print
platen --shrink in.pdf out.pdf 2     # shrink a PDF under 2 MB
```

Run `platen --help` for the rest.

## Security and privacy

- Everything stays on your network. Platen makes no connections other than to your printer.
- The printer's admin password, if you enter one for Device details, is stored in the macOS Keychain.
- The printer's web page uses a self-signed certificate, so Platen cannot verify it is really talking
  to your printer. On a network you don't trust, someone could pose as the printer and capture that
  password. Use Device details on your own network.
- The Scan button needs the printer's SNMP access set to read-write. That lets any device on your
  network change printer settings. Platen tells you this before it changes anything, and the setting
  can be turned back from the printer's web page.

Found a security problem? Please open a private security advisory on this repository rather than a
public issue.

## Build from source

You need Rust (the version in `mise.toml`), Node 22 and pnpm.

```sh
git clone <this repository>
cd platen/app
pnpm install
pnpm tauri dev      # run the app
pnpm tauri build    # build Platen.app and a .dmg into target/release/bundle
```

```sh
cargo test --workspace            # core tests
cargo run -p platen-cli -- --help # the CLI
cd app && pnpm check              # frontend type check
```

### Layout

| Path | What |
| --- | --- |
| `crates/core` | The library: scanning (eSCL and the native protocol), printing (IPP), stitching, PDF writing, OCR, sessions, the Scan button. |
| `crates/tui` | The `platen` command-line app. |
| `app` | The desktop app: Tauri 2 with a Svelte 5 frontend. Commands live in `app/src-tauri/src`. |
| `docs/PLAN.md` | Design notes, including how the vendor protocols work. |

### Releasing

Bump the version in `app/package.json` and `Cargo.toml`, then push a matching tag:

```sh
git tag v0.3.0 && git push origin v0.3.0
```

The release workflow tests, builds a universal app (Apple silicon and Intel), and publishes the `.dmg`
and the CLI to GitHub Releases.

## Contributing

Issues and pull requests are welcome, especially reports from other printers: what worked, what
didn't, and the model. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and the ground rules.

## Licence

MIT. See [LICENSE](LICENSE).

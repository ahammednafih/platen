# Security

## Reporting a problem

Please report security problems privately: open the **Security** tab of this repository and choose
**Report a vulnerability**. Don't open a public issue. You'll get a reply within a week.

Useful to include: what an attacker could do, the steps to reproduce it, and the version or commit.

## What Platen trusts

- **Your local network.** Platen talks only to your printer. The printer's web interface uses a
  self-signed certificate, so Platen cannot verify it; on an untrusted network someone could pose as
  the printer and capture its admin password.
- **The printer.** Data from the device is parsed defensively and scans are size-limited, but a device
  you don't control should not be treated as safe.
- **Nothing remote.** The app loads no remote content and makes no connections beyond the printer.

## Releases

Release files are built by the workflow in `.github/workflows/release.yml` from a tagged commit on
`main`, with every third-party action pinned to a commit. The app is not notarized by Apple, so
download it only from this repository's Releases page.

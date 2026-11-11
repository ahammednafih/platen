# Contributing to Platen

Thanks for helping. Reports from other printers are as useful as code: Platen has been developed
against a single device, so every other model is new ground.

## Reporting how it works on your printer

Open an issue with:

- the printer's make and model
- what worked and what didn't (scan, copy, print, ink levels, device details, Scan button)
- for a failure, the message the app showed and, if you can, the output of `platen --status`

## Setting up

You need Rust (the version in `mise.toml`; [mise](https://mise.jdx.dev) installs it), Node 22 and
pnpm. Tesseract is only needed for searchable PDFs.

```sh
cd app
pnpm install
pnpm tauri dev
```

Before opening a pull request:

```sh
cargo test --workspace
cd app && pnpm check
```

CI runs the same two checks.

## Where things go

| Change | Place |
| --- | --- |
| Talking to the device, image work, PDFs | `crates/core`, with a test beside the code |
| A new app command | `app/src-tauri/src`, registered in `lib.rs`, with a typed wrapper in `app/src/lib/api.ts` |
| Screens and dialogs | `app/src/lib` |
| How a device protocol behaves | `docs/protocols.md`, so the next person doesn't rediscover it |

## Ground rules

- **No vendor names in the app's interface.** Say "the device". Compatibility notes belong in the README.
- **Standard protocols first.** Use AirScan and AirPrint where they can do the job; add vendor-specific
  code only for what they can't, and keep it in its own module.
- **Test behaviour, not internals.** Parsers and encoders get tests with real captured input. Tests must
  not need a printer or the network.
- **Never delete a page image when editing.** Edits write a new file; undo depends on it.
- **Keep changes focused.** Don't reformat files you aren't changing. The app crate and
  `tauri.conf.json` are deliberately not run through a formatter.
- **Explain device-side changes to the user.** If a feature needs a printer setting changed, the app
  says why, shows the manual steps, and asks before doing it.

## Commits

One change per commit, with a short subject that starts with a [gitmoji](https://gitmoji.dev) and says
what the user gets, for example `✨ Rename a scan from its title`. Add a body only when the reason
isn't obvious from the diff.

## Security

Please don't open a public issue for a security problem. See [SECURITY.md](SECURITY.md) for how to
report one privately.

## Licence

By contributing you agree that your work is released under the MIT licence, the same as the project.

# Instructions for agents working on MX Shooting Calculator

## Ask for as little as possible

The app keeps the hunter's data on their own devices and asks nobody for
anything it doesn't strictly need. Keep it that way:

- **No personal data.** Don't ask for or store a user's name, e-mail
  address, phone number or account id. Don't add analytics, telemetry,
  crash reporting, tracking or advertising. If a feature seems to need
  personal data, find a design that doesn't, or ask the maintainer first.
- **The fewest permissions.** Google sign-in asks for
  `https://www.googleapis.com/auth/drive.appdata` only, and deliberately not
  `openid`, `email` or `profile`: the e-mail address was removed on purpose.
  Don't add scopes, OAuth providers or browser permissions (position,
  compass, camera, notifications) beyond what a feature needs. Ask for a
  browser permission only when the user presses the button that needs it.
- **No new outside services** without the maintainer's agreement. Today the
  app only talks to:
  - Open-Meteo, for the weather;
  - the app's own sync server;
  - Google Drive, for sync, if the user chooses it;
  - GitHub, for the desktop apps' update check and downloads, if the user
    turns updates on (off by default).

  Pages ship with no third-party scripts, fonts or cookies.
- **The sync server keeps nothing readable.** Sync copies are encrypted on
  the device. Sign-in tokens pass through and are gone within minutes. Don't
  make the server store or log user data.
- **Keep the privacy policy true.** Any change to what the app collects,
  sends or stores must update `site/privacy.html`, including its date, in
  the same change. Check `server/README.md` and the Google Cloud Console
  steps too.

## Keep the export format documented

The backup file (`PersonalData` in `crates/ballistics-store/src/transfer.rs`
and the types in it) is a documented, open format. When you change it:

- Regenerate the docs with
  `MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format`. A test
  fails until you do.
- Update the hand-written sections of `docs/export-format.md` yourself.
- Field doc comments become the published descriptions: write them for an
  outside reader.

## Keep the code healthy

- **Before committing** run what the Tests workflow runs: `cargo fmt --all`,
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `scripts/web.sh lint`, and `cd server && npm test` when the
  server changed. CI fails on any of them, and releases wait for CI.
- **Review bigger changes before committing them.** For a feature, a
  security-relevant change or anything touching sync, the server, the
  updater or the stored data, have an independent review agent read the
  diff (correctness first, then security and the rules above), check its
  findings against the code, fix the real ones, then commit. One-line fixes
  and text changes don't need it.
- **Dependencies:** Dependabot proposes updates weekly; `deny.toml` keeps out
  crates with known security advisories, unaccepted licences or unknown
  sources. If a check fails on an advisory, update the crate, or note in
  `deny.toml` why it can't affect the app.

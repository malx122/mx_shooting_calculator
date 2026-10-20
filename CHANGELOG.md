# Changelog

All notable changes to MX Shooting Calculator. Versions follow
[Semantic Versioning](https://semver.org): while the version starts with 0,
any release may still change things noticeably.

## [Unreleased]

### Added

- **Weather where you are:** from your position or a place search
  (Open-Meteo): temperature, pressure, the ground's height and humidity are
  filled in. The forecast wind becomes a clock direction from the direction
  of your shot, which a phone can take from its compass.
- **Truing:** enter what a long shot really took to hit centre and the
  calculation adjusts the load's velocity or drag to match it.
- **Cards:** printable dope cards (bank card to A4, several temperatures
  and wind speeds, the ethical limit and dead-on ranges marked, text sized
  to fit with a readability meter) and stand cards (a map of the stand with
  range rings and holds, the shooting sector, no-shoot directions and
  landmarks). Saved as PDF.
- **Turret labels:** a strip for the elevation turret with each range at
  the clicks it needs, worked out by the calculation, a band for each turn
  and a click ruler. Measure the turret once per rifle. Numbers every 10 to
  100 m, the zeroing temperature as a choice, a marker under each number,
  five fonts and colours per band and per range. The strip allows for the
  label's thickness (paper, sticker, photo paper, laminated or your own),
  so its ends meet round the turret.

## [0.5.0] - 2026-10-20

### Added

- **AI assistants:** ask an AI assistant (a local one in LM Studio or Jan,
  or Claude) for holdovers, dope tables and whether a shot is ethical and
  legal, computed by the app's own engine with your rifles and bullets. See
  "AI assistants" in the README for setting it up.
- **Hunting rules per country:** choose Sweden, Finland or none (the flag
  in the header; on a phone, in the menu). Finland's rifle ammunition rules
  (Metsästysasetus 666/1993 § 16 a) are new, with each species' level, its
  Finnish name and what else Finnish law says. Sweden's rules moved to a
  file of their own; more countries can follow. The app starts from the
  device's language and region.
- **Game of Finland:** wild forest reindeer, sika deer, wolf, lynx,
  wolverine, raccoon dog, otter, stoat, grey, ringed and harbour seal, red
  squirrel and muskrat, each with its own drawing (the ringed and harbour
  seal share one) and Sweden's and Finland's rules. Seals have a group of
  their own; wolf, lynx and wolverine join the bear.
- **What's new:** this changelog in the app. Click the version number by the
  logo, or open What's new in the phone's menu. After an update the app
  says so once.

### Fixed

- **Swedish ammunition class for bullets of unknown construction** (from a
  Strelok import, for instance): they were called not allowed. They now get
  the class they meet if they expand, with a note to set the construction.

## [0.4.0] - 2026-10-20

### Added

- **Sync between devices:** keep rifles, bullets, loads, scopes and targets
  the same on your phone and computer, either through your own Google Drive
  (sign in with Google on each device) or with a sync key and no account
  (scan the first device's QR code on the next). With a key the data is
  encrypted on the device; the sync server (a small Cloudflare Worker, in
  `server/`) can't read it. Changes sync on start, after each change and
  every few minutes, and catch up after being offline.
- **Deletions are remembered**, so they reach other devices and backups
  (export format 5).

### Changed

- When the same record was saved at the same moment on two devices, every
  device now keeps the same copy.
- Restoring a backup brings back records deleted since.

## [0.3.0] - 2026-10-19

### Added

- **All Swedish game species:** every species on Svenska Jägareförbundet's
  hunting-season list, from moose to fieldfare, with Swedish names,
  estimated sizes and vital zones.
- **A drawing for each species:** the males, with the features that tell
  them apart (the capercaillie's beard and fan tail, the black grouse's
  lyre tail, the mallard's green head). Species that look alike share a
  drawing. Moose, wild boar, bear and fox are redrawn in more detail.
- **Groups:** target lists fold into groups (deer, birds, ducks and geese
  and so on), and search also finds Swedish names.
- **Swedish ammunition classes:** each Swedish species shows the class
  required with a rifle, and the results say whether your load meets it.
  Your own targets can have a group and a class too.
- **Delta Optical scopes:** 42 scopes (Titanium HD, Titanium, Classic,
  Entry, Javelin, Stryker HD, Hornet) with 17 of their reticles drawn.
  Some reticle sizes are estimates, as each entry's notes say.

### Changed

- **Catalog files per maker:** bullets and scopes have one file per maker.
- **Reticles redrawn:** the 20 reticles that had been taken from the makers'
  vector drawings (Kahles SKMR4; Vortex EBR-2C, EBR-4, EBR-7C, EBR-7D,
  EBR-8, EBR-9, VMR-2, XLR-2, HSR-5i and G4i BDC) are now the project's own
  drawings, built from the published subtension figures and the makers'
  pictures. The licence notes say so: the project's own catalog drawings
  are under the code's licences.

## [0.2.0] - 2026-10-19

### Added

- **Phone layout:** one column with a bottom tab bar, bigger text and touch
  targets, and a menu sheet for Units, Import, Export and Install.
- **Calculate on a phone:** results first, with quick fields for range,
  shot angle and wind. Each opens a picker:
  - a range ruler with presets and recent ranges;
  - an inclinometer dial, which can measure the angle with the phone's tilt;
  - a wind clock with quick speeds.

  A target button sits beside the scope view, and all inputs are on a
  Conditions page.
- **Demo data:** "Load demo data" in the menu and on the empty screens. It
  loads three rifles with bullets, loads, a scope and a target, and loading
  it twice adds nothing.
- **No target:** just the reticle and the impact point, without a vital
  zone or energy limits. It is available on the desktop too.
- **Installable web app:** a manifest, home-screen icons and a service
  worker, so the web version opens offline after the first visit. It also
  asks the browser to keep its data.

### Fixed

- **Demo Browning X-Bolt twist:** now 1:10, as Browning makes the .308. With
  1:12 the 175 gr bullet showed as unstable.
- **Long list text:** cut with "…" instead of running under the arrow.

## [0.1.0] - 2026-10-19

The first public version.

- **Ballistics:** point-mass solver with G1/G7 BCs, BC bands by velocity
  and custom drag curves. It covers air density, wind, shot angle, spin
  drift and Coriolis.
- **Your data:** rifles (with dope table settings), bullets and loads, own
  scopes and own targets, kept in a local database. JSON export/import
  merges devices without duplicates. Strelok Pro backups (`.srl`) import
  too.
- **Catalog:** about 50 calibers, 225 bullets and factory loads, Kahles and
  Vortex scopes with drawn reticles, and game animals and sport targets.
- **Calculator:** holds in mil, MOA or clicks, ethical range from the
  target's energy and velocity limits, point-blank range, and lead for
  moving game. It also shows the view through the scope at any
  magnification, and the trajectory in 2D or 3D.
- **Units:** metric or imperial for each quantity.
- **Platforms:** desktop (macOS, Windows, Linux) and the web (WebAssembly,
  data kept in the browser).

[Unreleased]: https://github.com/malx122/mx_shooting_calculator/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/malx122/mx_shooting_calculator/releases/tag/v0.1.0

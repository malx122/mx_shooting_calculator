# Changelog

All notable changes to MX Shooting Calculator. Versions follow
[Semantic Versioning](https://semver.org): while the version starts with 0,
any release may still change things noticeably.

## [Unreleased]

## [0.8.1] - 2026-10-22

### Changed

- **The range table comes first** on a computer: right under the hold,
  with the trajectory graph and 3D view as tabs. The ethical range, hit
  chance and dead-on hold sit in a rail on the right, above the view
  through the scope, so the page has less empty space. What to dial or
  hold stands out in large clicks; the row for your shot is outlined (or,
  between two rows, an orange line marks where it falls), the dead-on
  ranges are marked green, past the ethical limit is red, and a
  hit-chance column (one of the choices under *Columns…*) shows how sure
  each range is. Click a row to shoot at that range. New rifles show the
  drop and the wind drift in cm by default.

- The view through the scope has a faint grid on the ground, in
  perspective, so the field reads as depth up to the target.
- The stand card (a range card) is there for target shooting too, without
  the ethical limits; it was hidden outside hunting.
- Calculate shows *Target limits* only for game: for a sport target, or in
  target shooting, it had nothing left in it.

## [0.8.0] - 2026-10-22

### Added

- **Hit chance and wind bracket.** Under *Your precision*: your group, the
  velocity spread, and how well you know the range and the wind. The
  results give the chance of landing in the vital zone or hit area, the
  scope view draws where 9 shots in 10 land, the ethical range for game
  also stops where the hit chance falls below 90 % (or what you choose),
  and the wind bracket says how far the bullet goes across for the lowest
  and highest likely wind and whether holding for the middle covers both.
- **A shot log.** **Log this shot** under the hold on Calculate keeps the
  shot with the hold the app gave (in mil or MOA and in clicks), the
  bullet's drop, drift, speed and energy at the range, the weather, the
  wind and the range. Add whether it hit or missed, where it hit and notes.
  The log lists the shots newest first; open it from the ••• menu (on a
  phone, the menu) under **Shot log**.
- **Find where the animal stood.** In a logged shot, **📍 Mark my
  position** (from where you shot) and **🧭 Take bearing** (hold the phone
  flat, its top edge towards the animal) work out the animal's spot from
  the range, with an area to search that allows for a compass 10° off and
  a range 5 % off. **Guide me there** then shows the distance and
  direction from where you are ("128 m, bearing 52°"), an arrow when the
  phone's compass knows which way it points, and when you are within the
  search area. There is no map: nothing is sent anywhere, and **Copy
  coordinates** puts the spot on the clipboard. On a computer, type the
  position and the bearing. The browser asks before it tells the app
  where you are, and only when you press one of those buttons.
- Backups and sync carry the shot log (format version 6, described in
  [docs/export-format.md](docs/export-format.md)). Older versions of the
  app can't read the new backups.
- **Svenska and Suomi:** the app in Swedish and Finnish beside English,
  with the hunter's own words: inskjutning, knäpp, vindavdrift, etiskt
  skjutavstånd; kohdistus, naksu, osumaenergia, ampumaetäisyys. Choose
  **Language** in the menu, or keep **System** to follow the device. The
  game species go by their Swedish or Finnish names, and the cards print in
  the chosen language. Numbers keep the decimal point for now; decimal
  commas come later.
- **Small drawings in the four input tiles** on the Calculate page, following
  the values: the range on a scale, the line of fire at its angle, the wind
  arrow on a clock face, and the chosen target's own silhouette. On a phone
  they sit under the value.
- **A new moose** with the hit area and rings of Svenska Jägareförbundet's
  moose figure (15, 25, 33 and 41.5 cm) instead of a circle. The results
  say where a dead-on hold lands: "in the hit area, ring 4".

### Changed

- The sync server keeps copies of up to 1 MiB (was 256 KiB): room for the
  shot log, about 800 shots beside everything else.
- On a phone, the view through the scope is centred and as wide as the
  screen allows, with its magnification and width written under the
  slider.
- The vital zone's size is no longer changed on the Calculate page; copy
  the target on the Targets page to use another size.

## [0.7.2] - 2026-10-20

### Fixed

- The ••• menu on wide screens: Units, Use for and Hunting rules open a
  window of their own instead of a submenu that covered the menu.
- The sync status's dot no longer overlaps its text.

## [0.7.1] - 2026-10-20

### Added

- **Swarovski Optik scopes:** 34 scopes (Z8i, Z8i+, Z6i, Z5i+, Z5+, Z5,
  Z3 and dS), with 13 of their reticles drawn. BRS, BRW, BRW2m-I and 4Wm-I
  come from Swarovski's published figures. 4A, 4A-I, 4A-IF, LD-I, D-I and
  Plex are drawn at an estimated size, which their notes say. The ballistic
  reticles Swarovski publishes no figures for (4W, BRX, BRH, BRT) show the
  plain crosshair rather than guessed hold marks.
- **Zeiss, Meopta and GPO scopes:** 36 Zeiss scopes (V8, Conquest V6/V4,
  V3, Victory HT, LRP), 89 Meopta scopes (MeoPro, MeoHunter, MeoStar,
  MeoTac, ZD and older Artemis models) and 34 GPO scopes (SPECTRA, PASSION,
  CENTURI, GPOTAC), with their reticles drawn from the makers' published
  figures. A few plain reticles are drawn at an estimated size, which their
  notes say; ballistic reticles with no published figures show the plain
  crosshair.
- **Desktop apps** for macOS (Apple silicon and Intel), Windows and Linux,
  attached to each release on GitHub, and a **download page** with a
  **privacy policy** beside the web version.
- **An open backup format:** an exported backup now also writes out the
  calibers, scopes and reticles its rifles name from the app's catalog, so
  other programs can read it without the app. The format is described in
  [docs/export-format.md](docs/export-format.md), with a JSON Schema
  published beside the download page.

### Changed

- **Google Drive sync asks for less:** only the app's own folder in your
  Drive, no longer your e-mail address. The Sync window reminds you to use
  the same Google account on every device.

### Fixed

- The legal class, season and light pills wrap instead of pushing the
  scope view off the edge.
- The estimated field of view, used when a maker publishes none, for
  scopes that start below 1×.

## [0.7.0] - 2026-10-20

### Added

- **Hunting or target shooting:** the app asks once what it is used for.
  Target shooting leaves out everything about hunting (ethical limits,
  legal ammunition, seasons, shooting light, stand cards), and every target
  is just a hit zone. **Use for** in the menu changes it.

- **Chronograph import:** measured velocities from a Garmin Xero
  (ShotView CSV), LabRadar or MagnetoSpeed file, with the string's shot
  count, SD and ES kept and shown. A warning if the average is far from what
  the bullet should do.
- **Hunting today:** the season for the chosen game in your county or
  region, and today's legal shooting light where you are, from Swedish and
  Finnish law (Jaktförordningen, Metsästysasetus and the ministry's
  decrees). Sunrise and sunset are worked out offline, midnight sun and
  polar night included. The Targets page shows each species' season table
  with the law's words and sources.
- **Zero as a height:** a rifle can be zeroed "4 cm high at 100 m". The
  calculator tells every zero that way, with where the bullet comes back to
  the line of sight: your own, and the best point-blank zero ("13 cm high
  at 100 m"), which **Make it my zero** saves as the rifle's (allowing for
  a bullet that hits higher or lower than the rifle's zero load). Turret
  labels start where the bullet comes down to the line of sight.

### Fixed

- A bullet or measurement with an impossible number (from a chronograph
  file, a backup or a very light, high-drag bullet) could crash the app and
  keep it from starting. Numbers must now be finite and within what rifles
  and bullets can be, on saving and on import, and the engine stops a
  flight that runs away instead of crashing.
- A Strelok backup with an infinite speed could make an export that
  wouldn't import; a damaged, deeply nested one crashed the import.
- A backup stamped far in the future made later edits lose to it.
- "Make it my zero" could halve the dead-on range it promised.
- Two measured velocities a fraction of a degree apart gave an absurd
  temperature sensitivity; it is now fitted only over at least 2 °C and
  kept believable.
- The shooting light in the far north in summer, which runs past midnight.
- A dope card from 0 m printed 1570.8 mil on its first row.
- Chronograph import errors weren't shown; caliber and scope pickers stayed
  open after a choice; a long name stretched the calculator's panel; the
  truing form kept another bullet's numbers; the turret label's ruler
  numbers ran together on small turrets; "near the sound barrier" showed
  far below it; the phone showed "beyond the ethical range" in green.

### Changed

- **A simpler Calculate.** Four tiles for what changes every shot (range,
  angle, wind, target) open the same pickers as on a phone. Weather, fine
  corrections (cant, spin drift, aerodynamic jump, Coriolis) and the
  target's own numbers are folded, each with a summary line. Dial and
  illumination sit under the scope view.
- **Go or no-go in one place:** the legal class, the season in your county
  and today's shooting light are pills under the hold, with the county to
  choose and **Use my position** next to them. On a phone they are one line.
- **Limits at a glance:** the ethical max and the dead-on hold, with the
  zeros, muzzle velocity and stability under Details.
- **The range table** (once the dope table) shows the clicks, angles, wind
  and energy for new rifles; **Columns…** on the table sets its ranges and
  columns, kept for the rifle. A phone shows three columns, or all.
- **Load setup moved to the Rifles page:** measured velocities, chronograph
  files, point of impact, zero weather and truing are under **Loads in this
  rifle**; Calculate keeps one line about the load with a **Load setup…**
  link. Truing is behind **Shot long and missed? True the calculation…**.
- **Rifle form:** the basics first, the clicks taken from the scope, the
  barrel, zero weather and notes folded.
- **One menu:** on the desktop the header's Units, Rules, Sync, Import and
  Export are one ⋯ menu (with What's new and About), next to the rules'
  flag and a sync dot. A first start asks where you hunt.
- **Bullets:** one **+ Add from the catalog** button; the caliber choice
  belongs to the catalog, which shows each bullet's legal class. The bullet
  form folds the drag model and the velocity details.
- **Turret label:** its settings are in three groups: your turret (folded
  once measured), the ranges, and the look.
- Cards say they are worked out level and in calm air (Calculate also
  counts today's wind, angle and cant). "Drop" in the dope table is
  "Height", the card's "Up" is "Elevation", and "1 click" is singular.
- Hunting today has a **Use my position** button; messages clear after a
  few seconds; the season table lists your county first; ranges up to
  2000 m on both phone and desktop.

## [0.6.0] - 2026-10-20

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
- **Aerodynamic jump:** a crosswind now lifts or lowers the bullet a little
  at the muzzle (Litz's fit), when the bullet's length is known. On by
  default; it can be switched off beside spin drift.
- **Cant:** enter how much the rifle is tilted, and the holds allow for it;
  the calculator says how far a dead-on hold lands from a level one. The AI
  tools take a cant too.

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

[Unreleased]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.1...HEAD
[0.8.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/malx122/mx_shooting_calculator/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/malx122/mx_shooting_calculator/releases/tag/v0.1.0

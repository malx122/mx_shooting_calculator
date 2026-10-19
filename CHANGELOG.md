# Changelog

All notable changes to MX Shooting Calculator. Versions follow
[Semantic Versioning](https://semver.org): while the version starts with 0,
any release may still change things noticeably.

## [Unreleased]

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

[Unreleased]: https://github.com/malx122/mx_shooting_calculator/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/malx122/mx_shooting_calculator/releases/tag/v0.1.0

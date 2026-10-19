# Changelog

All notable changes to MX Shooting Calculator. Versions follow
[Semantic Versioning](https://semver.org): while the version starts with 0,
any release may still change things noticeably.

## [Unreleased]

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

[Unreleased]: https://github.com/malx122/mx_shooting_calculator/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/malx122/mx_shooting_calculator/releases/tag/v0.1.0

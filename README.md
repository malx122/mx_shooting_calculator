<img src="branding/mxsc-logo.svg" alt="MX Shooting Calculator" width="420">

# MX Shooting Calculator (MXSC)

A Rust app (egui + wgpu) for rifle ballistics, for hunting and target
shooting. It runs on the desktop and in the browser. You keep your rifles
and the bullets you shoot in them in a local database, and the calculator
gives:

- a range table (dope), with holds in mil or MOA and scope clicks;
- energy and velocity at impact, and an ethical range limit set by the
  game you hunt (target shooters can leave everything about hunting out);
- point-blank range for your zero, and the zero that maximises it, told
  the way it is shot in ("13 cm high at 100 m") with a button to make it
  the rifle's zero;
- lead for moving game;
- the view through your scope and a 3D view of the shot, with game animals
  and sport targets at true size;
- a shot log of the shots you took, with the solution the app gave, what
  came of them and, after a shot at game, the way to where the animal
  stood.

It is in English, Swedish and Finnish.

## Install

Each release on [GitHub](https://github.com/malx122/mx_shooting_calculator/releases)
has the desktop app for macOS, Windows and Linux. The web version below runs
on phones and anywhere else.

- **macOS** (Apple silicon and Intel, macOS 11 or later): open the `.dmg`
  and drag the app to Applications. The app isn't notarized yet, so the
  first time macOS refuses to open it: open **System Settings → Privacy &
  Security** and choose **Open Anyway**.
- **Windows** (64-bit): unzip and run `MX Shooting Calculator.exe`.
  SmartScreen may warn about an unknown publisher: **More info → Run
  anyway**.
- **Linux** (x86-64): unpack, then `./install.sh` puts the program in
  `~/.local/bin` with a menu entry and icon; or run `./mxsc` from the folder.

**Updates** (desktop, from version 0.8.2): in the menu, **Updates…** turns
on a check for a new version when the app starts (off until you turn it
on), or checks now. **Update now** downloads the new version from GitHub,
checks that it is signed by the app's maker, puts it in place of the old one
and offers to restart. On a Mac the app must be in Applications (not run
from the disk image).

## Run

```sh
cargo run --release      # the app
cargo test --workspace   # all tests
```

On GitHub the `Tests` workflow (`.github/workflows/test.yml`) runs on every
push to main and on pull requests: formatting (`cargo fmt --all --check`),
the workspace's tests, clippy with warnings as errors (also for the browser
build: `scripts/web.sh lint`), the dependencies' security advisories,
licences and sources (`cargo deny check`, set in `deny.toml`), and the sync
server's tests (`cd server && npm test`). Each release workflow runs it
first and publishes nothing if it fails. Dependabot (`.github/dependabot.yml`)
opens weekly pull requests to update the crates, npm packages and pinned
actions.

The program is called `mxsc`. Its database is stored in the platform's
app-data folder: on macOS that is
`~/Library/Application Support/MX Shooting Calculator/mxsc.sqlite`. To use
another file, pass `--db /path/to/file.sqlite` or set `MXSC_DB`.

The app was called Malx Hunting Calculator, and before that Ballistic
Calculator. If no database exists yet but one from an earlier name does, it
is copied over on first start; the old file is left in place. Backups
exported under the earlier names still import.

### Web version

The same app runs in a browser (WebAssembly). Each release is published at
**https://malx122.github.io/mx_shooting_calculator/** by
`.github/workflows/pages.yml`. To build it yourself:

```sh
scripts/web.sh           # serve on http://127.0.0.1:8080, rebuilding on changes
scripts/web.sh build     # release build into crates/app/dist: static files to host
```

It needs rustup with the browser target, [trunk](https://trunkrs.dev) and
`llvm-ar`. On macOS:

```sh
brew install rustup trunk llvm
$(brew --prefix rustup)/bin/rustup toolchain install stable --target wasm32-unknown-unknown
```

The script puts rustup's toolchain first, because Homebrew's own Rust can't
build for the browser. It also uses `llvm-ar`, because macOS's `ar` can't
archive the WebAssembly build of SQLite. Plain `cargo` and the desktop app
are unaffected.

How the web version differs from the desktop one:

- **Storage:** SQLite runs in the page. The database is kept in the
  browser's IndexedDB, so it survives reloads and restarts, but it belongs
  to that browser and site. Changes are written back a moment after they are
  made. Use Export/Import to move data between browsers and devices.
- **Export/Import:** Export downloads `mxsc-backup.json`. Import opens the
  browser's file picker (JSON exports and Strelok `.srl` backups).
- **Drawing:** WebGPU where the browser has it, otherwise WebGL.
- **Size:** about 9 MB of WebAssembly, 3.6 MB gzipped or 2.6 MB with brotli.
- **Installable and offline:** a web app manifest and a service worker
  (`crates/app/manifest.webmanifest`, `crates/app/sw.js`, icons in
  `crates/app/assets/pwa/`). "Add to Home Screen" gives an app icon and a
  full-screen app; after the first visit it opens without a connection. The
  page is fetched from the network first, so a new release loads as soon as
  there is a connection. The app asks the browser to keep its data.

### On a phone

Below 640 points of width (any phone, and a narrow desktop window) the app
switches to a one-column layout with bigger text and touch targets
(`mobile.rs`, `calculator/phone.rs`):

- **Navigation:** a bottom tab bar (Calculate, Rifles, Bullets, Targets,
  Cards) and a compact header whose menu holds the Shot log, Units, Language,
  what the app is used for, Hunting rules, Sync, Import, Export, Load demo
  data, Install and the version.
- **Calculate, results first:**
  - The rifle and bullet line opens a page with the rest of the inputs,
    each group folded with a summary.
  - Under it are four tiles: **range**, **shot angle**, **wind** and
    **target**.
  - Then the holds with **Log this shot**, one line on the legal class, the
    season and the light (tap it for the details), and the scope view.
  - Limits, the range table (three columns, or all) and the trajectory
    (2D/3D) fold out below.
- **Pickers:** sheets that slide up from the bottom (on a desktop, the same
  pickers open as dialogs from the tiles).
  - **Range:** a drag ruler, ± steps, and distances from the range table
    plus recent ranges.
  - **Shot angle:** an inclinometer dial and presets. In the browser, "Measure
    with the phone" reads the phone's tilt. The rifle's **cant** is set here
    too.
  - **Wind:** a clock dial for the direction and quick speeds.
  - **Target:** search, recent targets, and **No target** (just the reticle
    and the impact point).
- **Rifles, Bullets, Targets and the Shot log:** a list, then the record full
  screen with a way back.

### Versions and releases

Versions follow [Semantic Versioning](https://semver.org), MAJOR.MINOR.PATCH:
fixes raise the patch number, new features the minor number. While the
version starts with 0, any release may still change things noticeably. The
version is set once, in the workspace `Cargo.toml`, and shared by all
crates. The app shows it next to the logo; hover for the commit it was built
from.

To make a release:

1. Move the notes under "Unreleased" in [CHANGELOG.md](CHANGELOG.md) to a new
   version heading, and set the same version in `Cargo.toml`. The changelog
   is compiled into the app and shown under **What's new** (click the
   version number; on a phone, in the menu), so write it for hunters. A
   test fails if `Cargo.toml`'s version has no heading in it.
2. Commit, then tag and push: `git tag v0.2.0 && git push origin main v0.2.0`.
3. The tag builds and publishes the web version, and builds the desktop apps
   and attaches them to the tag's GitHub release (created from the
   changelog). The workflows refuse a tag that doesn't match `Cargo.toml`.

The desktop packages are made by `scripts/package.sh` on each system (the
`Desktop apps` workflow runs it on GitHub's macOS, Windows and Linux
machines): a universal `.dmg` on macOS, a `.zip` on Windows and a `.tar.gz`
on Linux, in `dist-desktop/`. The icons are drawn from the SVG by
`cargo run -p mxsc --example icons`. The macOS app is signed without an
identity and the Windows program not at all; signing and notarizing need an
Apple Developer ID and a code-signing certificate.

Beside the packages each system also gets the file the app's updater
installs (`-update`: the app bundle as a `.tar.gz` on macOS, the bare
program elsewhere). The release job writes `update.json`, the version and
each update file's SHA-256, and signs it with `tools/sign-update` using the
minisign key in the `MXSC_UPDATE_KEY` secret; the app checks that signature
with the public key in `crates/app/src/updates.rs` before it installs
anything. The secret lives in the repository's `release` environment, which
only `v*` tags may use, so no branch or manual run from one can sign.
Without it a release gets no `update.json`, and the apps aren't offered it. To try the updater against a local server laid out like
GitHub's release downloads, set `MXSC_UPDATE_FROM` to its address.

The backup format (`transfer::VERSION`) and the database schema (the
migrations in `db.rs`) have their own numbers. The backup format is
documented for other programs in [docs/export-format.md](docs/export-format.md)
with a JSON Schema; both are generated from the code, and a test fails until
they are regenerated after a change
(`MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format`). Only a change that stops old
backups or databases from loading needs a new major version.

### Development data

`dev-data/sample.json` is a fixture in the app's export format with:

- a **CZ 457** in .17 HMR, with two CCI loads from the catalog and a Kahles
  HELIA 3 4-12x44i with the G4B reticle (second focal plane, ballistic bars);
- a **Tikka T3** in .223 Rem, with two factory loads from the catalog and a
  Kahles K318i with the SKMR4 (first focal plane, mrad);
- a **Browning X-Bolt** in .308 Win, with two factory loads, a handload, a
  velocity measured in that rifle, and a Kahles HELIA 2,4-12x56i with the
  4-Dot.

```sh
cargo seed   # (re)create target/dev.sqlite from the fixture
cargo dev    # run the app on target/dev.sqlite; your real database is untouched
```

Both shortcuts are defined in `.cargo/config.toml`; run them from the
repository root.

To change the sample data:

1. Edit it in the app (`cargo dev`).
2. Choose **Export backup** in the menu.
3. Save over `dev-data/sample.json`.

`cargo test` checks that the fixture still imports and that its catalog links
resolve. The seed command only replaces `target/dev.sqlite`; given any other
path that already exists, it refuses unless you pass `--force`.

## Brand

The vector source files are in `branding/`:

- **`mxsc-icon.svg`:** the app icon. It shows the view through a rifle
  scope: a German #4 reticle over a forest ridge, with a blaze-orange
  trajectory arcing into the aim point.
- **`mxsc-logo.svg`** (for light backgrounds) and **`mxsc-logo-dark.svg`:**
  the icon with the MX wordmark and the "Shooting Calculator" tagline.

The wordmark is drawn from straight strokes, so it needs no font; the X's
crossing is the orange aim point. Only the tagline uses a system font.

The app uses `crates/app/assets/`: the icon and a light wordmark for the dark
header. They are rendered from SVG at whatever size is needed, including the
window and dock icon, so no bitmaps are stored. The colors are defined in
`crates/app/src/theme.rs`:

| Name | Hex | Use |
|---|---|---|
| Forest | `#12261A` – `#2B5A3E` | backgrounds, panels |
| Bone | `#F1EADB` | text, the wordmark |
| Blaze | `#E8702A` | accent, selection, the trajectory |
| Meadow | `#7FC08C` | the vital zone in the plot |

## How it is used

1. **Rifles:** add each rifle with:
   - its caliber, from a searchable list of about 50 common calibers;
   - twist, sight height, scope and click value. The scope comes from the
     catalog or from **My scopes**: scopes you enter yourself, with
     magnification range, focal plane, the magnification the reticle is true
     at, clicks, elevation travel, field of view and illumination. A custom
     scope is drawn with a catalog reticle you choose, or a plain crosshair;
   - the zero and the weather it was made in: temperature, powder
     temperature, pressure and humidity. The zero can be a height at its
     range, the way many hunters zero ("4 cm high at 100 m"): the bullet
     then hits that high there instead of on the crosshair.

   The form starts with the basics (name, caliber, scope and reticle, the
   clicks, sight height, zero); the barrel, the zero's weather and notes are
   folded with a summary. A catalog scope brings its click value, shown as
   "from the scope" with **Change** for your own.

   Under the rifle, **Loads in this rifle** lists the bullets of its caliber.
   Each has what is specific to that combination:
   - **Measured velocities:** chronograph averages at different temperatures.
     Two or more give the rifle's own temperature sensitivity, fitted as a
     line. **From a chronograph file…** reads a Garmin Xero (the ShotView
     app's CSV, any language), LabRadar (`SR…Report.csv` from its card) or
     MagnetoSpeed (`LOG.CSV`, one series chosen) file: misreads are left
     out, and the average, SD and ES are worked out from the shots and kept
     with the measurement. Enter the cartridges' temperature that day.
   - **Point of impact:** where this load hits relative to the load the rifle
     was zeroed with, at the zero range. The engine tilts the bore by that
     angle.
   - **Zero weather:** the rifle's, the same as the shot's, or recorded
     values.
   - **Truing** (behind **Shot long and missed? True the calculation…**):
     after a long shot (300 m or more works best), enter the range and what
     it took to hit centre, in the weather set on Calculate. The engine
     finds the muzzle velocity (when you have no chronograph) or the drag
     (when the velocity is measured, so the BC is the doubt) that matches,
     and uses it from then on. The AI tools use it too.

   Calculate uses a load once it is saved.
2. **Bullets:** build your collection.
   - **From the catalog** (**+ Add from the catalog**): pick a caliber, then
     factory ammunition in that caliber or a component bullet that fits its
     bore. With hunting rules set, a column gives each one's legal class
     out of a typical barrel. Adding one makes your own editable copy.
     Handloads need your own muzzle velocity before they can be saved.
   - **Manually:** for anything not in the catalog. A bullet can have up to
     five **BCs by velocity band**, interpolated by speed through the air.
   - **Own drag curve:** a bullet can use its own measured drag curve (Mach
     against Cd, for example a maker's Doppler-radar data) instead of G1/G7
     and a BC. Enter points or paste a whole table: spaces, tabs, commas,
     semicolons and decimal commas are all read. With its own curve the
     bullet needs no BC; the engine uses its mass and diameter.

   The drag model (BCs, bands or a curve) and the velocity details are
   folded with a summary; construction and lead-free, which decide the
   legal class, stay in view for a hunter.
3. **Calculate:** choose a rifle. Only bullets *in your collection* loaded in
   that rifle's caliber can be chosen. A line under them tells the muzzle
   velocity today and where it comes from (the box, your measurements,
   trued); **Load setup…** opens the bullet's setup on the Rifles page.

   Four tiles hold what changes every shot: **range**, **angle** (and cant),
   **wind** and **target**, each opening a picker. The rest is folded, each
   with a summary line: **Weather** (temperature, with **Air details** for
   pressure, altitude, humidity and powder temperature), **Fine
   corrections** (cant, spin drift, aerodynamic jump, Coriolis) and the
   target's own numbers.

   The results lead with the hold, then (for game, with hunting rules set)
   pills for the **legal class**, the **season** in your county and today's
   **shooting light**. The **ethical max**, **hit chance** and **dead-on
   hold** stand beside the hold (on a computer, above the scope view), with
   the zeros, muzzle velocity and stability under **Details**. Under the
   scope view: dial or hold, and the illumination. Under the hold, the
   **range table** (or the trajectory as a graph or in 3D, one tab away)
   shows what to dial or hold in large clicks with the angles, drop and
   drift in cm and the rest smaller, the hit chance at each range, the row
   for this shot outlined (or a line between the rows it falls between),
   the dead-on ranges marked green and the ranges past the ethical limit
   red; click a row to shoot at that range. On a computer
   **Columns…** sets its ranges and columns, kept for the rifle.

   **Weather:** **📍 Weather here** (on a phone or in a browser that may
   know your position) or a place search fills in the temperature,
   sea-level pressure and the ground's height, humidity and the latitude,
   from [Open-Meteo](https://open-meteo.com) (MET Norway's model in the
   Nordics). The forecast wind (10 m above open ground) is turned into a
   clock direction from the direction you shoot: type it, or on a phone
   hold the phone flat with its top edge towards the target and press **🧭
   Point the phone**. **Use the wind** brings it down to shooting height
   (about 70 % over open ground; less in forest, so adjust it). On a phone
   the wind picker offers the forecast wind too.

   **Target and ethics:** choose what you shoot at, either game or a sport
   target. The target sets the vital zone (or, on a sport target, the hit
   zone) and, for game, the minimum energy. You can change the energy for
   this calculation; ↺ goes back to the target's value. To use another zone
   size, copy the target on the Targets page. Sport targets have no
   ethical limits. The moose shows the hit area and rings of Svenska
   Jägareförbundet's moose figure, and the results say where a dead-on hold
   lands ("in the hit area, ring 4"). The target is drawn at true size through the scope and in
   3D. When the target is game in your country, the results say whether
   your load meets the **legal rifle ammunition level** for it, worked out
   from the bullet's weight, whether it expands, lead-free or not, and its
   energy at the muzzle and at 100 m. A bullet whose construction isn't set
   (a Strelok import, say) is assumed to expand, and the result says so.

   **Hit chance:** under **Your precision** you say how well you shoot and
   how well you know the shot: your 5-shot group at 100 m from the
   position you shoot from, the muzzle velocity's spread (from your
   chronograph string when there is one), and how far off the range, the
   wind's speed and its direction may be (each about one standard
   deviation). The app flies the shot again with each of them changed, adds
   up how the hits spread at the target, and gives the chance of landing in
   the vital zone or the hit area when you hold as it tells you. The scope
   view draws the ellipse 9 shots in 10 land in. For game, the ethical
   range also stops where the hit chance falls below what you ask for (90 %
   unless you change it), and the **wind bracket** says how far the bullet
   goes across for the lowest and highest likely wind, and whether holding
   for the middle keeps both in the zone.

   **Hunting today:** for game, with your country set, the results show the
   season in your county (Sweden's län, Finland's maakunnat), from
   the weather place search or picked from the list: open until when, or when
   it opens, for which animals and with which method limits. It also shows
   today's legal shooting light where you are (sunrise and sunset worked out
   offline for your position), and whether it is allowed right now. In
   Sweden that is from an hour before sunrise to an hour after sunset for
   moose, deer, wolf, lynx and wolverine (the last hour only stalking or
   watching), and to two hours before sunset for bear, by Jaktförordningen
   9 §. In Finland only ducks and geese have such hours. The Targets page
   shows each species' whole season table, the law's own words with links,
   and the sources. The data is the law as of its date: check before you
   hunt. How it is made and updated: [tools/hunting](tools/hunting/README.md).

   **Hunting rules** (in the menu; the header's flag shows the choice)
   chooses the country: **Sweden** (Naturvårdsverket, NFS 2023:8, classes
   1–4), **Finland** (Metsästysasetus 666/1993 § 16 a: moose, deer, roe
   deer, fox and small game classes) or **none** for no legal checks. It
   starts from the device's language and region. Each country's rules are a
   file of their own in `crates/ballistics-store/src/rules/`; adding a
   country means adding a file and its levels to the targets.

   The **Trajectory** can be shown in 2D (a graph of drop and wind drift) or
   in 3D. The 3D view is a true-scale scene: a hunter at the muzzle, the
   target at the target range with its vital zone ringed, the
   bullet's path, the line of sight, the wind and distance marks. Drag to
   orbit, right-drag to pan, and ⌘/Ctrl + scroll or pinch to zoom. The
   buttons jump to set views: Overview, Behind the shooter, From the side,
   At the target and From above. Double-click to return to the overview.
   It is drawn with egui's painter, so it needs no 3D library. Because the
   scene is true to scale, the hunter and the target are small at long range
   in the wide views.
4. **Targets:** the catalog's game animals and sport targets, and **My
   targets**. The catalog has every species on Svenska Jägareförbundet's
   list of Swedish hunting seasons and Finland's game species (Metsästyslaki
   § 5), in folding groups (deer; wild boar and mouflon; bear, wolf, lynx and
   wolverine; fox, badger, martens and otter; hares, beaver and rodents;
   seals; grouse, ptarmigan and pheasant; ducks, geese and swans; other
   birds; sport). Search finds them by English, Swedish or Finnish name. Each species has
   its own drawing of the male (birds in the plumage they have in the
   hunting season); species that look alike share one, such as the grey
   geese or the gulls. Sizes and vital zones are estimates. Each species
   shows, with your country's flag, the least rifle ammunition its law
   allows for it, and what else the law says (a shotgun allowed too, a
   licence needed, protected). Copy a catalog target to adjust it, or make a new one:
   - an animal: drawn as any of the catalog's animals, at your size, with
     or without antlers or horns. Birds and small animals are measured to
     the top of the back, and birds from beak to tail;
   - a paper target with rings: innermost ring, ring spacing, number of
     rings and black;
   - a steel plate.

   Each target has a vital or hit zone. Game has a minimum energy and can
   have its own minimum velocity. That velocity replaces the bullet's
   expansion velocity and your general one; small game sets none, since
   it doesn't need expansion.
5. **Cards:** printable cards for the rifle, bullet and target chosen in
   Calculate, in today's weather. The preview is the card as it prints;
   **Save PDF** saves it (on the desktop it then opens in the PDF viewer;
   on a phone it goes to the downloads, ready to print or keep).
   - **Dope card:** what to dial or hold at each range. Pick the size
     (a bank card, A7, A6, A5 or A4, portrait or landscape; small cards
     can fill an A4 sheet with cut lines), the ranges (or a quick choice:
     to the ethical limit, driven hunt, stand, long), one or more
     temperatures (an elevation column each: autumn, winter, all year),
     elevation as clicks, mil or MOA, or cm, wind columns for chosen
     speeds, and speed, energy and time of flight. The text is made as
     large as the card allows, and a meter says whether it is easy to
     read, small, or too small. A red line marks the ethical limit and a
     green bar the ranges where a dead-on hold stays in the vital zone.
   - **Stand card:** a range card: a map of a hunting stand or a shooting
     position, facing its direction, over
     90°, 180° or all round: range rings with the hold at each, the
     dead-on zone in green, beyond the ethical limit in red, the shooting
     sector the hunt leader gave (outside it greyed) and the directions
     never to shoot (roads, houses, the next stand) hatched. Landmarks
     with their range and direction are marked on the map and listed with
     their hold; on a phone, 🧭 takes a direction from the compass. Spare
     room becomes lines for notes. Stands are kept on the device.
   - **Turret label:** a strip to wrap round the elevation turret, with
     the ranges at the clicks they need. Measure the turret once (diameter
     over the knurling and height with a caliper, clicks a turn, which way
     it turns when dialling up) and say what the label is printed on: it
     bends round its own middle, so the strip is made π × its thickness
     longer (0.3 mm for paper, nearly 1 mm laminated) and its ends meet. It
     is kept for the rifle. The marks come from the
     calculation at today's temperature, the zeroing temperature or a
     chosen one: a number every 10, 25, 50 or 100 m (as "1 2 3", "10 11 12"
     or "100 110 120"), lines and dots between, optionally a tick or a
     triangle under each number at its exact click, and a band for each
     turn of the turret. Under them is a ruler with a tick for every click
     and the mil (or MOA) numbered. Lettering: Helvetica or one of four
     bundled fonts (Barlow Condensed, Bebas Neue, Share Tech Mono, Black
     Ops One; SIL Open Font License, embedded in the PDF), and the
     numbers' size. Colours: a scheme to start from, then the background
     and marks of the label, the description, each turn's band and the
     ruler, a colour for beyond the ethical limit, and any single range a
     colour of its own. Marks that would overlap are left off and listed.
     The A4 sheet has copies of the strip with cut marks, a 50 mm bar to
     check the print scale, and the clicks for every range.

6. **Shot log** (in the ••• menu; on a phone, the menu): **Log this shot**
   under the hold on Calculate keeps the shot: the rifle, bullet and
   target, the range, shot angle, wind and weather, the hold the app gave
   (in mil or MOA and in clicks) and the bullet's drop, drift, speed,
   energy and time of flight at the range. The log lists them newest first
   with the date, rifle, target, range and outcome; open one to add whether
   it hit or missed, where it hit and notes, or to delete it.

   After a shot at game, the record helps find where the animal stood:
   - **📍 Mark my position** (from where you shot) reads the phone's GPS
     once, with its accuracy. **🧭 Take bearing** reads the compass while
     you hold the phone flat, its top edge towards the animal. Both can be
     typed instead, as on a computer, which has neither.
   - From the position, the bearing and the level distance (the range ×
     cos of the shot angle) the app works out the animal's spot on a
     sphere the size of the Earth, and a search radius that allows for a
     compass 10° off, a range 5 % off (at least 5 m) and the position's
     accuracy, and is at least 10 m.
   - **Guide me there** follows your position as you walk and shows the
     distance and direction to the spot ("128 m, bearing 52°"), an arrow
     when the compass knows where the phone points, and when you are
     within the search area. **Copy coordinates** copies the spot as
     latitude, longitude.

   There is no map, so nothing about the shot is sent anywhere; the log
   travels only in your sync copy and your backups. The browser asks before
   it tells the app where you are, and the app asks only when you press
   Mark my position or Guide me there.

**Hunting or target shooting:** the app asks once what it is used for (and a
hunter where they hunt); the menu's **Use for** changes it. **Target
shooting** leaves out everything about hunting: ethical limits, legal
ammunition, seasons and shooting light (stand cards stay, without the
limits). Every target is then just a hit zone.

**Units** (in the menu): metric or imperial for each quantity separately.
Distance, drop, sight height, velocity, wind speed, temperature, sensitivity,
pressure (hPa, mmHg or inHg), altitude, energy, bullet weight, length,
diameter, barrel length and twist can each be set. Values are always stored
in metric, so changing units never changes your data.

**Language** (in the menu): English, Svenska or Suomi, or **System** to
follow the device (the browser's language on the web, the system's on the
desktop; English when it is neither Swedish nor Finnish). The catalog's game
species then go by their Swedish or Finnish names, and the cards print in the
chosen language. Numbers keep the decimal point for now; decimal commas come
later. The catalog's notes, the hunting-season notes and What's new stay in
English.

**Export backup / Import backup…** (in the menu) save and merge your rifles,
bullets, rifle loads, custom scopes, custom targets and shot log as JSON. **Import
backup…** also reads **Strelok Pro backups
(`.srl`)**. It shows what it will add, with each rifle's inferred caliber to
check, before writing anything, and importing the same backup again adds
nothing. See [docs/strelok-import.md](docs/strelok-import.md). Import merges record by record and keeps whichever copy was edited
last, so the same file can move data between devices. Backups from earlier
versions (formats 1 to 5) still import. Restoring a backup brings back
records deleted since.

**Sync** (in the menu; the header's dot shows how it is doing) keeps your rifles, bullets,
loads, scopes, targets and shot log the same on all your devices, in one of two ways:

- **With Google Drive:** **Sign in with Google** on each device, with the
  same account. The data is kept in a hidden folder in your own Drive that
  only this app can reach.
- **With a sync key, no account:** **Start syncing with a key** on the first
  device. It shows the key with a QR code; scan it on the next device (it
  opens the app with the key) or type the key under **Join**. The data is
  encrypted on the device before it is sent, so the sync server
  ([server/](server/README.md), a small Cloudflare Worker) can't read it.
  Keep the key to yourself: anyone with it can read and change the synced
  data.

A device that joins has its own data merged in. From then on the app syncs
when it starts, a moment after each change, when it comes back to the front,
and every few minutes; changes made offline go up later. When the same
record is changed on two devices, the later change wins; a deletion and a
later edit work the same way. **Stop syncing here** leaves the device's data
as it is; **Delete the synced copy** turns sync off on every device. Units
and calculator inputs stay per device.

## AI assistants

`mxsc-mcp` lets an AI assistant answer questions with the app's own engine
and your own rifles: "what's the holdover for my Tikka at 280 m, 4 m/s from
the right?", "make me a dope card for the X-Bolt", "is my .243 load legal for
roe deer?". It is an [MCP](https://modelcontextprotocol.io) server, so it
works with any assistant that supports MCP: LM Studio or Jan with a local
model, Claude Desktop, Claude Code and others.

The assistant gets these tools (read-only for now):

| Tool | What it does |
|---|---|
| `list_rifles`, `list_bullets`, `list_targets` | Your rifles (with scope, zero, clicks and measured bullets), your bullets, and the game and sport targets (search by English, Swedish or Finnish name) |
| `holdover` | Elevation and windage to dial or hold at a range, in the scope's units and clicks, with drop, drift, velocity and energy; with a target, whether the shot is inside the ethical range and meets Swedish ammunition rules |
| `dope_table` | The same over a span of ranges |
| `search_catalog` | The bullet catalog |

Names can be loose ("tikka", ".308", "rådjur"); when a name fits several
records the tool says so instead of guessing. Without a bullet it takes the
one last used with the rifle. The weather is the one last entered in the
app's calculator unless the assistant gives its own, wind and angle are none
unless given, and every answer says which conditions it used.

It reads the desktop app's database. With sync on in the desktop app, what
you add on your phone reaches it whenever the desktop app runs.

**Set up.** Build and install it once:

```sh
cargo install --path crates/mxsc-mcp      # installs ~/.cargo/bin/mxsc-mcp
```

Then tell your assistant to start it (use the full path):

- **LM Studio:** Program → Install → *Edit mcp.json*, then add
  `{"mcpServers": {"mxsc": {"command": "/Users/you/.cargo/bin/mxsc-mcp"}}}`.
  Pick a model that is good at tool calls (Qwen 3, Llama 3.1 or newer, 7B
  or larger).
- **Claude Desktop:** Settings → Developer → *Edit config*, and add the same
  `mcpServers` entry to `claude_desktop_config.json`.
- **Claude Code:** `claude mcp add mxsc -- ~/.cargo/bin/mxsc-mcp`.

Add `"args": ["--db", "/path/to/file.sqlite"]` to use another database, for
example `target/dev.sqlite` for the development data.

**While developing,** register `scripts/mxsc-mcp.sh` instead of the installed
copy. It rebuilds the server when the code has changed and then starts it,
so the assistant always gets the current version (from its next session, or
after it restarts the server):

```sh
claude mcp add --scope user mxsc -- "$PWD/scripts/mxsc-mcp.sh" --db "$PWD/target/dev.sqlite"
```

## Code layout

| Crate | What it does |
|---|---|
| `crates/ballistics-core` | The physics engine: a 3-DOF point-mass solver with G1/G7 drag, atmosphere, wind, shot angle, cant, spin drift, aerodynamic jump, Coriolis and the hunting analyses. No dependencies, no I/O. |
| `crates/ballistics-store` | The base catalog, the SQLite database (with schema migrations), JSON export/import, the sync merge and encryption (`sync.rs`), and the shot log with its great-circle sums for the animal's spot (`shots.rs`). No UI, no networking. |
| `crates/app` | The egui screens, for desktop and the web (`web.rs`, `index.html`). Pages return actions instead of writing to the store, so the logic can be reused by another front end. `sync.rs` and `http.rs` run sync rounds in the background. `cards/` lays out the printable cards once, in millimetres, for both the preview and a hand-written PDF (Helvetica, nothing embedded). `i18n.rs` shows the texts in Swedish and Finnish from `i18n/*.json` (see Translating). |
| `crates/ballistics-shot` | A shot from the hunter's records: the engine's scenario from a rifle, bullet, measured velocities and conditions, holds in the rifle's clicks, and the legal ammunition check for the hunter's country. Shared by the app and `mxsc-mcp`, so both give the same answers. |
| `crates/mxsc-mcp` | The MCP server for AI assistants (see above). |
| `server` | The sync server: a Cloudflare Worker with a D1 database. See [server/README.md](server/README.md). |

`ballistics-core` and `ballistics-store` build on desktop and mobile targets
(SQLite is compiled in), and in the browser, where SQLite is compiled to
WebAssembly. The app crate builds for desktop and the web. Its few
platform-specific parts (data folder, file dialogs, start-up) are behind
`cfg(target_arch = "wasm32")`.

### How sync merges

Every record has a UUID and an `updated_at` time; deletions are kept too
(the `deleted` table), so they reach the other devices instead of the record
coming back. A merge keeps the later of two copies, and on equal times
compares them, so every device settles on the same one. Times come from a
clock that never runs behind anything the store has seen, so a device whose
clock is slow still stamps its edits after the copies they were made from.
The shared copy is the usual export (format 6): encrypted on the sync
server, which checks versions so two devices writing at once can't overwrite
each other; plain in Google Drive, which can't check versions, but a device
whose write is overwritten still has its records and puts them back on its
next round.

## Translating

The app's texts are looked up by their English (`crates/app/src/i18n.rs`):

```rust
ui.label(tr("Range"));
ui.label(trf!("Saved {}", rifle.name));
ui.label(trf!("{count} rifles", count = rifles.len()));
```

`trf!` is `format!` for texts the hunter reads: `{}`, `{0}` and `{name}`
placeholders, with a sign and decimals (`{:+.1}`) and nothing else. The
English must be a literal inside `tr(`/`trf!(` so the checks can find it.
Texts from the data go through `tr_data` (target groups, rules levels) and
`target_name` (a catalog target's Swedish or Finnish name).

The translations are data: `crates/app/i18n/sv.json` and `fi.json` map the
English to the translation, sorted. A text with no translation (or an empty
one) shows in English, so new code works before anyone translates it.

1. Wrap the new texts in `tr` or `trf!`.
2. `cargo test -p mxsc i18n -- --nocapture` lists the texts each language
   still lacks. `MXSC_I18N_UPDATE=1 cargo test -p mxsc i18n` adds them to the
   files with an empty translation and takes out the ones the app no longer
   has.
3. Fill in the empty translations. A translation keeps the English's
   placeholders: the `{}` ones in the same order (give them names if a
   language needs another order), the named ones anywhere.

The tests fail when a file has a text the app no longer shows, or a
translation that lost a placeholder. Numbers keep the decimal point in every
language for now; decimal commas come later.

## Scopes and reticles

`crates/ballistics-store/catalog/scopes/` (one file per maker) and
`reticles.json` hold scope specifications and reticle geometry. Reticles
stay in one file because makers' scopes can share a reticle. They cover seven makers:

- **Kahles:** 8 scopes with their 4-Dot, G4B, 4-DH and SKMR4 reticles drawn.
  Kahles publishes dimensioned drawings for every hunting reticle.
- **Vortex:** 43 scopes from Crossfire HD, Diamondback (and Tactical),
  Strike Eagle, Venom, Viper HD, Viper PST Gen II, Razor HD Gen III and
  Razor HD LHT.
  - **Reticles:** 26 Vortex reticles are drawn, with one entry per scope
    where Vortex publishes per-model values. Each reticle's notes say what
    was published and what was measured from the picture or assumed; line
    widths were often assumed.
  - **Gaps.** Vortex loads many of its spec tables dynamically, so some
    fields are missing. Where the field of view is missing, the scope view
    estimates it from the magnification. Missing click values on scopes
    from 2× up are set to Vortex's standard ¼ MOA or 0.1 MRAD, which the
    notes say.
  - **Not cross-checked.** This data was collected in one quick pass.
- **Delta Optical:** 42 scopes from Titanium HD, Titanium, Classic, Entry,
  Javelin, Stryker HD and Hornet, with 17 of their reticles drawn (4A S,
  4A SB, 2D, Mil-Dot, Multi-X, SMR-1/2, SDOG-1, DLR-1, LRD-1P, DLS-1/2/3,
  DSMR, DGMR, D²BR, D²MR).
  - **Sources.** Delta's own sites (deltaoptical.pl and the older
    deltaoptical.eu with its dimensioned reticle sheets), Delta's user
    manuals, and optics-trade.eu for a few gaps; each entry's notes say
    which.
  - **Estimates.** Several Delta pictures carry no dimensions (4A S, 2D,
    Mil-Dot, Multi-X, SDOG-1): their angular size is estimated and the
    notes say so. Where Delta doesn't say at which magnification a
    second-focal-plane reticle is true, the top magnification is assumed.
  - **Not drawn.** Delta publishes nothing to draw SKR-1/2, DLR-3, Di MD,
    2D FFP or the Javelin's 4A SB from, so those scopes offer the plain
    crosshair.
- **Swarovski Optik:** 34 scopes, all second focal plane:
  - **Current:** Z8i, Z8i+, Z6i, Z5i+ (MOA and MRAD versions) and dS Gen. III.
  - **Older ones Swarovski still documents:** Z3, Z5 2.4-12x50 and 3.5-18x44, and Z5+.
  - **Sources.** Swarovski's product pages (the data each page carries
    for its variants) and its technical data sheets; each entry's notes
    say which.
  - **Reticles.** 13 drawn:
    - from Swarovski's published figures: BRS(-I), BRW(-I), BRW2m-I and 4Wm-I;
    - with an estimated size, from Swarovski's pictures (the notes say so):
      4A, 4A-I, 4A-IF, LD-I, D-I and Plex(-I). These have no hold marks,
      so only how the posts look is uncertain.
  - **Not drawn.** Swarovski's subtension program is gone and nothing else
    gives the figures for 4W(-I), BRX(-I), BRX Heavy, BRH and BRT-I.
    Their hold and wind marks shouldn't be guessed, so those scopes offer
    the plain crosshair until the figures turn up.
- **Zeiss:** 36 scopes: V8, Conquest V6 and V4, V3, Victory HT and the LRP
  S3/S5 long-range scopes.
  - **Sources.** Zeiss's product pages, catalogues and illuminated and
    non-illuminated reticle data sheets.
  - **Reticles.** 52 entries from Zeiss's per-scope tables: #60, #54, #43,
    #6, #20 Z-Plex, #90 AHR, ZMOA, ZBR, ZBi, ZMOA-T, XRR, SHR(i) and the
    LRP's ZF-MRi and ZF-MOAi. Only the #54 on the V8 1.8-14x50 is drawn at
    an estimated size, because Zeiss publishes it for the 1.1-8x24 only.
  - **Not drawn.** The older Victory HT 1.1-4x24 and 1.5-6x42 don't name
    their reticle, so they offer the plain crosshair.
- **Meopta:** 89 scopes: MeoPro Optika6, R6 and R5, MeoHunter R5, MeoStar R1,
  R1r and R2, MeoTac, the ZD scopes, and older MeoPro, Optika5 and
  Artemis 2000/2100/3000 models that Meopta still documents in its reticle
  sheets.
  - **Sources.** Meopta's catalogues, manuals and one reticle sheet per
    reticle. The older scopes are known only from the reticle sheets, so
    their click values are "not published".
  - **Reticles.** 106 entries from those sheets: the 1, 4A–4K, ZPlex,
    ZPlus and KDot hunting reticles, the Dichro versions, BDC, MMD,
    MilDot, MRad, McWhorter, Windmax 8, MT and the calibre-specific RD
    reticles. ZPlex on the Optika6 1-6x24 SFP and the R5, 4C on the
    Optika6 1-6x24 FFP and the "4" on the MeoPro 3-9x40 are drawn at an
    estimated size. Meopta's sheets leave the magnification blank only for
    first-focal-plane scopes, so the old Artemis variables, whose focal
    plane isn't stated, are taken as first focal plane.
  - **Not drawn.** Eleven offerings have no sheet (such as the
    non-illuminated .223, .308 and 6.5CM, Z-Plus Dichro, and McWhorter HV on
    the MeoPro), so those scopes also offer the plain crosshair.
- **GPO:** 34 scopes: SPECTRA, PASSION, CENTURI and GPOTAC.
  - **Sources.** GPO's technical information and gp-optics.com's reticle
    sheets, plus GPO USA's subtension pages.
  - **Reticles.** 28 entries: G4 and G4i (glass, Fiber and Drop), G3i
    Drive, BRi, BRWi, LR Pro, PLRi, MOAi and HSi-CQB. The CENTURI and
    PASSION G4i are estimates scaled from the SPECTRA tables.
  - **Not drawn.** MOAi on the CENTURI scopes, MIL-FFP and the GPOTAC 8X's
    HSi have no figures, so those scopes offer the plain crosshair.

**How reticles are drawn.** Every reticle is the project's own drawing:
code builds it from the subtension figures the maker publishes (spacings,
line widths, hold-point positions), and what the maker doesn't print is
measured from the maker's published picture. Nothing is traced or copied
from the makers' vector artwork.

**Reticle format.** A reticle is a list of vector shapes in angular units
around the aiming point (x to the right, y up):

- `line`, with its thickness, or `null` for a hairline;
- `polygon`, filled, for heavy posts;
- `circle`, as a dot or a ring.

The `unit` is whatever the maker publishes: `mrad`, `moa` or `cm_at_100m`.
Elements that reach the edge of the view extend to ±500 and are clipped to
the field of view. Parts that light up carry `"illuminated": true`.

**One entry per drawing.** Scopes that share a reticle point to the same
entry, which lists them in `for_scope`. Before drawing a new reticle, match
it against the ones already there. When entries are merged, `replaces`
keeps the old ids so rifles and backups that name them still work. A test
fails when two entries have the same drawing.

**Every name is searchable.** When one drawing is sold under several names
(for example the Illuminated Dead-Hold BDC and the Dead-Hold 2A), the extra
names go in `also_known_as`. Searching for a reticle finds it by its name,
`maker_reticle_name` or any of those names, ignoring case, spaces and
dashes. That works when choosing a reticle for your own scope, and in the
scope picker, which also finds catalog scopes by reticle name.

**Second focal plane.** On these scopes the marks are true only at one
magnification, stored as `reticle_reference_magnification`. Because the real
size of a reticle differs between scope models, Kahles publishes one value
table per model, so the catalog has one reticle entry per scope (for example
"4-Dot (HELIA 2,4-12x56i)"). A scope has one reference magnification, but
makers publish each reticle's figures at a magnification of their own (Meopta
gives one scope's reticles at 4×, 7× and "max"). The scope takes the
magnification most of its reticles hold at, and the others get a copy
rescaled to it, which its notes say.

**In the app:**

- A rifle has a scope and reticle (Rifles page). Choosing a scope fills in
  the click unit and value.
- The calculator shows **Through the scope**: the reticle at the chosen
  magnification, the impact point for a dead-on hold (the orange dot to hold
  on the target), and the target with its vital zone at true size.
- The view is the scope's real field of view at the magnification set on
  the slider under it. On second-focal-plane scopes the slider marks the
  magnification where the reticle's marks are true; double-click it to jump
  there. The elevation can be held or dialled, and illumination switched on
  or off.
- At other magnifications, second-focal-plane reticles are scaled
  correctly.

**Caveats in the data.** Most dimensions come from Kahles' tables. A few were
drawn rather than published, and each reticle's `notes` say which:

- the start of the 4-Dot bottom post;
- the SKMR4 holdover-tree widths, measured from Kahles' scale drawing.

The HELIA 1-5x24i's click value is not published.

## The base catalog

`crates/ballistics-store/catalog/` holds the catalog as JSON compiled into the
app:

- **`calibers.json`:** about 50 calibers, each with bullet diameter and a
  typical twist.
- **`bullets/`:** 225 bullets and loads, one file per maker (for example
  `bullets/norma.json`). These are factory loads from Norma, Lapua, Sako,
  Hornady, Barnes, Geco, Federal, CCI, Remington and Winchester, and
  component bullets from Hornady, Nosler, Sierra, Barnes, Lapua and Norma.
- **`scopes/`:** scopes, one file per maker; see below.

  A new maker only needs a new file: the build script compiles in every
  `.json` file in these folders, in name order. A file holds only its own
  maker's entries, named after the maker in lower case (a test checks it).
- **`reticles.json`:** reticle geometry, in one file since scopes from
  different makers can share a reticle.
- **`targets.json`:** every Swedish game species, a few others (elk,
  white-tailed deer) and sport targets (ISSF 300 m, 50 m and 10 m rifle
  targets; 10, 20 and 30 cm steel plates), each in a list group. Each
  species has its rifle level per country (`"ammunition": {"sweden": 2,
  "finland": 3}`) and what else each country's rules say (`rules_notes`).
  Older files with `swedish_class` still read.
- **`drawings.json`:** the animal drawings the targets use, as coloured
  polygons in metres. An animal can also have `zones` (polygons a shot can
  land in, most telling first, such as a test figure's hit area) and a
  training target's `rings` (true size, with what each scores); animals
  without zones use a circle the size of the target's vital zone.
  `tools/drawings/README.md` describes how a drawing is made: a flat vector
  picture, converted by `tools/drawings/svg2drawing.py`, with its zone drawn
  in `tools/hitzone-editor.html` over a picture of the figure.

  The animals' vital zones and minimum energies are common **guidelines**,
  not law. Legal rules differ by country and are often stated differently
  (for example as energy at 100 m). Check your own country's rules and adjust
  or copy the target. The ISSF ring sizes follow the ISSF rules.

Most entries come from the manufacturer's own pages. Hornady and Winchester
don't publish BCs for their .17 HMR loads, so those four entries take their
BC from retailer pages: at least two independent sources had to agree, and
each entry's notes list them.

Every entry carries the manufacturer page it came from (`source_url`) and the
date it was checked. Values that a manufacturer does not publish are left
empty rather than estimated: bullet length (often), G7 BC, and minimum
expansion velocity.

A test (`builtin_catalog_is_consistent`) checks every entry:

- known caliber keys;
- diameters that fit a caliber;
- plausible BCs and velocities;
- grams matching grains;
- https sources.

Edit the JSON directly and run the tests.

### Known caveats in the data

- **Sako, Geco, CCI, Remington and Federal (partly)** don't say whether
  their BC is G1 or G7. The values are stored as G1, which matches the
  published velocity tables and is the usual convention for rimfire.
- **Lapua Naturalis** has unusually low published G1 BCs (for example 0.201
  for the 6.5 mm 140 gr). Lapua's own velocity tables confirm them.
- **Bullet length** is missing for most factory loads. Without it the app
  cannot compute stability, spin drift or aerodynamic jump and says so;
  enter the length if you know it.
- **RWS is not included:** its website asks AI crawlers not to collect its
  pages.

## Physics validation

`crates/ballistics-core/src/tests.rs` checks analytic limits and physical
behaviour. It also checks agreement with py-ballisticcalc 3.1.1, an
independent solver: drop agrees within 0.1 cm at 1000 m on level shots.

Two effects are added to the point-mass flight:

- **Aerodynamic jump:** a crosswind throws a spinning bullet slightly up or
  down as it leaves the muzzle. The app uses Bryan Litz's fit,
  (0.01·Sg − 0.0024·L + 0.032) MOA per mph of crosswind, with Sg the
  stability and L the bullet's length in calibres: with a right-hand twist,
  wind from the left lifts the bullet and wind from the right lowers it.
  It needs the bullet's length, and can be switched off.
- **Cant:** a rifle tilted about the line of sight turns the bore's own
  geometry (its height under the scope and its angle up to the line of
  sight) with it, while gravity and wind stay put. A dead-on hold then
  lands to the side of the tilt and a little low, by the bore line's height
  over the line of sight times the sine of the cant (5° at 300 m: a few
  centimetres). The holds are worked out for the tilted reticle, and the
  calculator says how far a level shot would differ.

A point-mass model with one BC is very good at hunting ranges, but it is only
as good as the BC and muzzle velocity you give it. Chronograph your rifle and
confirm your dope on paper at real distances before hunting. The energy and
velocity limits are yours to set; check your bullet maker's expansion data and
your country's legal ammunition requirements.

## License

The source code is licensed under either of the
[Apache License, Version 2.0](LICENSE-APACHE) or the [MIT License](LICENSE-MIT),
at your option. This is the usual dual licence for Rust projects.

- **Third-party libraries** are listed with their licences in
  [THIRD-PARTY-LICENSES.txt](THIRD-PARTY-LICENSES.txt). Ship that file with
  any binary. All of them are permissive except `resvg`/`usvg` and
  `option-ext` (MPL-2.0: changes to those files must be shared) and the
  fonts egui bundles (SIL Open Font License and Ubuntu Font Licence). The
  desktop app's list of trusted certificate authorities (`webpki-roots`) is
  under CDLA-Permissive-2.0, which only asks that its licence text comes
  along. The sync server's tooling (`wrangler`) is only used to build and
  deploy it and doesn't ship. Regenerate the file after changing dependencies:
  `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.txt` (install
  with `cargo install cargo-about --locked --features cli`).
- **Chronograph test data:** the parser's tests quote a few lines of real
  files from MIT-licensed projects (useyournose/xeroconv and
  ammolytics/experiments), credited in `crates/ballistics-store/src/chrono.rs`.
- **The card fonts** in `crates/app/assets/fonts` (Barlow Condensed, Bebas
  Neue, Share Tech Mono, Black Ops One, from Google Fonts) are under the SIL
  Open Font License 1.1; each font's licence and copyright is next to it.
  They are embedded in the PDFs the app makes, which the licence allows.
- **The built-in catalog** (see [NOTICE](NOTICE)): bullet and scope figures
  are facts compiled from manufacturers' publications, each with its
  source. The reticle and animal drawings are the project's own: the
  reticles are built by the project's code from the subtension figures the
  makers publish, not traced from their artwork. The project's own parts
  are under the same licences as the code. Product and reticle names are
  their owners' trademarks.

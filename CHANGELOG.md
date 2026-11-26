# Changelog

All notable changes to MX Shooting Calculator. Versions follow
[Semantic Versioning](https://semver.org): while the version starts with 0,
any release may still change things noticeably.

## [Unreleased]

### Fixed

- **A shotgun keeps its own range on Calculate:** choosing a shotgun after
  a rifle no longer takes the rifle's 150 m or more, where shot shows no
  pattern. A shotgun starts at 30 m, and each kind of gun gets its own
  range back when you switch. Past 150 m the page says plainly that the
  range is too far for shot.

## [0.12.1] - 2026-11-26

### Fixed

- **0.12.0 is out at last:** its build stopped on a check that newer Rust
  added, so 0.12.0 never reached the desktop or the web. 0.12.1 is the same
  app.

## [0.12.0] - 2026-11-21

### Added

- **Groups:** a new tab for shot groups. Photograph the target (or choose a
  photo; on a computer open one or drop it on the window), set the scale,
  the aim point and the shots, and the app works out the group: extreme
  spread (also edge to edge), mean radius with its 95% range, σ, CEP and
  R90 (by Grubbs's method when the group is clearly elliptical), SD across
  and up, width × height, and the chance of a hit in a circle you size.
  Results come first: the impact from the aim and the correction to dial in
  the rifle's own clicks ("2 clicks left, 1 click up"). A shot that pulls the
  group is suggested as a flier, never left out until you call it; a
  cold-bore shot is kept apart with its distance from the aim and the group.
  Groups of fewer than 5 shots say their numbers are rough.
  - **The library** lists the groups newest first under their range session
    or day, each with a small drawing of the group and its size by mean
    radius or extreme spread (your choice), filtered by rifle, load, tags
    and notes. Archive a group, or delete it with Undo.
  - **Saving** asks what was shot: the rifle (and its fitted barrel), the
    bullet, the distance, the session (today's, or a new one by place and
    date), tags and notes, and whether to count the rounds on the barrel.
  - **Scale from the target's grid:** tap four grid crossings anywhere on
    the photo, say the square size (5 mm, 10 mm, ½ inch, 25 mm, 1 inch or
    your own) and how many squares across and down, and the photo is scaled
    and straightened without the whole sheet in view. A sheet's four corners
    and two points a known distance apart work too.
  - **Find the grid:** the app looks for the target's printed grid itself,
    once on a new photo and again on the part on screen when you press
    **Find the grid**: it places four crossings as far apart as it can vouch
    for and counts the squares between them, after checking the count on
    the pixels along every edge. The scale counts once you confirm the
    square size with one tap; the app remembers it. On paper with heavier
    lines every few (1 inch over ¼ inch, 10 mm over 1 mm) you choose the
    big squares or the small ones, each with its own size.
  - **Detect** (⌖, or D on a computer) on the shots step looks for bullet
    holes in the part of the photo on screen, sized by the scale and the
    calibre, dark on paper and light in the black, touching pairs split.
    They show as dashed rings to check, never added by themselves: tap a
    ring to tick or untick it, then **Add** the ticked ones as shots in one
    undo step, numbered from the top left. Holes already marked, and doubtful
    ones, start unticked.
  - **MX targets** are recognised on a photo by their corner marks: the
    scale and straightening are set by themselves, with the sheet's top up
    however the photo is turned, and a warning when a mark was only
    estimated or the sheet doesn't lie flat. The app asks once what the
    sheet's 100 mm ruler measures, to allow for the printer's scaling. The
    aim step offers the sheet's aim points; another group on the same
    photo starts at the next free one.
  - **Later photo:** photograph an MX target again after more shots and
    the holes not yet marked are suggested, found through the sheet's own
    marks however the photo was taken. A later photo that doesn't line up
    with the marked shots is flagged as maybe another sheet, and a hole
    that grew or split as a likely shot through an earlier one.
  - **Blank grid** for a target you didn't photograph, at true scale.
  - **Photos stay on the device:** reduced to 4096 pixels unless you keep
    the originals, with their location and other metadata taken out. Groups,
    sessions and photo records go into backups and sync; the photos don't.
  - **Units:** group sizes as mil, MOA or cm per 100 m, and on the target in
    mm, cm or inches, in the Units settings.
  - **Demo data** now includes groups, a zero series and a load ladder.
  - **Analyse** (from the Groups header on phones, Library | Analyse on
    computers):
    - **Zero check:** whether the zero is holding, drifting or can't tell
      yet from the groups shot since the last zero reset, net of what was
      dialled, with the correction in your clicks. Mark a zero reset, or
      write the load's point of impact for the calculator. It counts only
      groups shot at the rifle's zero range, measured from where the
      bullet flies at that exact distance, and offers the correction only
      with a verdict, showing its 95% range.
    - **Trend:** group size, σ, velocity or the cold-bore shot against date,
      charge, seating, temperature, pressure or barrel rounds, one line per
      load, with 95% bars and box plots. From velocity against temperature
      it fits the powder's temperature sensitivity and can write it to the
      bullet.
    - **Compare** groups side by side, best and worst, with an honest test
      of two groups ("can't tell yet: about 14 shots each would settle
      it"), and save the comparison.
    - **Composites** pool groups into one, about their aims, about their own
      centres or as a cold-bore series, in angle and in mm at any distance.
    - **Load ladders:** rungs by charge or seating, the flattest runs with
      their noise, and "Make 45.5 gr my load".
  - **Powder charges** show in grains by default, also with metric units;
    grams can be chosen in Units. Groups keep the handload's charge and CBTO
    as they were when saved, so a later recipe change doesn't move them on a
    trend.
  - **Library:** pick several groups to compare, pool, archive or delete
    them; saved analyses are listed with the groups.
  - **Group details:** part of a ladder, what was dialled for the group, and
    whether it counts in the zero check.
  - **Velocities on a group's page:** paste, type or read a chronograph file
    (Garmin Xero, LabRadar, MagnetoSpeed), pair the readings with the
    shots, see the average, SD with its range, ES and warnings, how the
    vertical follows velocity against what the solver expects, and save the
    string as the load's velocity.
  - **Share a group** as a picture (PNG), and save groups or shots as CSV.
- **Cards:** MX targets, printable targets for Groups: A4, A3 or Letter,
  portrait or landscape, with 1, 2, 4 or 6 diamond or square aim marks sized
  for 100–300 m, a light 10 mm or 1-inch grid and a 100 mm check ruler. A
  coded mark near each corner tells the app the sheet's layout, so a photo
  of the sheet can set its own scale, straighten itself and place the aim
  points, even when one mark is covered or holed. The format is open and
  documented in docs/mx-targets.md.
- **Calculator:** "Use my groups" sets "Your precision" from your measured
  groups.
- **Rifles** show their round counts (action, barrel, since clean), turning
  amber when a clean is due at your own "clean after" count; log rounds and
  cleans, see and delete recent rounds, and set the rounds fired before
  logging.
  - **Barrels:** add one (it is fitted at once), fit an earlier one, retire
    or remove one; when the fitted barrel's length or twist differ from the
    rifle's ballistics, copy them across.
  - Each barrel and load shows the precision measured from its groups.
- **Handloads** keep their recipe: powder, charge (grains or grams), primer,
  brass, COAL, CBTO and notes.
- **Sessions,** in the menu: days at the range with place, time and weather
  (filled in from Open-Meteo when you ask), and the groups shot there.
  Deleting a session leaves its weather on its groups.
- **Backup with photos:** one ZIP with the backup and your groups' photos;
  importing it brings the photos back.
- **Phone tabs:** Calculate · Groups · Guns · Ammunition · Targets. Cards
  moved into the ••• menu beside the shot log.
- **Sun theme:** a light, high-contrast look for reading the app in bright
  sunlight at the range, on paper-coloured pages with dark text and deeper
  orange, green and red. Choose **Theme** in the menu: Dark (as before),
  Sun, or Follow the system, which switches with the device's light or dark
  setting. The choice is kept on each device and takes effect at once; in
  the browser the loading screen follows it too.
- **Backups and sync** use export format 9 (docs/export-format.md), which
  adds the Groups records; earlier versions of the app refuse its files and
  sync copies rather than lose them.

### Fixed

- **Dark on light-mode devices:** a computer set to light mode could show
  the app with egui's light defaults mixed in with the forest-green
  headers. Dark now looks the same everywhere, and on the desktop the
  window's title bar follows the chosen theme.
- **Target editor on phones:** the fields no longer overlap the animal
  preview.

## [0.11.1] - 2026-11-21

### Fixed

- **The Mac app is built again:** 0.11.0's Mac disk image failed to build,
  so 0.11.0 came out only on the web. 0.11.1 is the same app for all
  systems.

## [0.11.0] - 2026-11-21

### Added

- **Shotguns, treated as rifles are:** a shotgun is a gun among your rifles
  (the Rifles page is now Guns), its shells are ammunition beside your
  bullets (the Bullets page is now Ammunition), and Calculate works with it.
  - **Shotgun calibers in the catalog:** 20 of them, from 10/89 to .410/76,
    with C.I.P.'s bore, chamber, pressures and steel-shot limits. A gun and a
    shell each have a caliber; a shell fits a gun of its gauge with as long a
    chamber or longer (a 12/70 shell in a 12/76), never the other way round.
  - **Shells in the catalog:** Gamebore's range (186 loads and sizes) and
    Norma's 12/70 range, formerly Gyttorp (27), from the makers' pages, on
    the Ammunition page's catalog under Shotgun, with **Add** as for
    bullets.
  - **The gun:** each barrel's choke, by name or constriction (each name
    with its stars, fraction, notches, letters and the names other countries
    use, so you can find it by what is stamped on the tube), and for each
    shell the pattern boards you counted with it in this gun, which replace
    the choke's nominal pattern (as measured velocities do for a rifle).
  - **The shell:** shot or a slug, with the velocity as the box gives it (V0,
    V2.5, 3 ft or the British observed velocity).
  - **On Calculate:** pick the shotgun, its shell and barrel; the range,
    wind, target and weather are the same as for a rifle (and the angle, for
    slugs; shot is worked out level). The shell
    range by range: velocity, each pellet's energy, the pattern's spread,
    the share in the Nordic test's 75 cm circle, pellets per dm², drop and
    wind drift, with your range highlighted. Slugs get their path against
    the sights.
  - **The pattern on the target at true size:** a shot's pellets on the
    animal's 3D body at your range, with those on the hit area counted. In
    Sweden, the hunting test's circle is drawn beside it for reference.
  - **What the law says,** per species, for Sweden (NFS 2023:8) and Finland
    (metsästysasetus): shot or slug, gauge, Sweden's 4.1 mm limit, lead in
    wetlands, Finland's lead ban for waterfowl and the EU's ban from 2033.
  - **Advice, each named with its source:** Svenska Jägareförbundet's shot
    sizes per species, Riistakeskus' and Metsästäjäliitto's ranges, and
    the step from lead to steel. The app shows what a pattern does; it
    doesn't judge how many pellets an animal needs.
  - **How far shot carries,** worked out for the load, with the Swedish
    range-safety figures beside it.
  - **The swarm in three dimensions:** each pellet flies on its own, some
    starting slower and the dented ones dragging more, so the shot arrives
    as a string 1–4 m long, as measured. The table gives the string's length
    and the energy at its back; the pattern on the target varies from shot
    to shot as real patterns do; a side view shows the swarm as it reaches
    the target; and with **Animal crossing at** the pattern lands on a
    moving animal, with the lead to give it.
  - **Shells in 3D, with their measurements:** every shotgun caliber has a
    3D shell with a star crimp and a drawing of its C.I.P. measurements, in
    the shotgun editor and the shell catalog, and saves as an STL for a
    3D-printed dummy like a rifle cartridge.
  - Researched and tested against published pellet, pattern and slug tables
    (docs/shotgun.md). Backups, sync and the export format (version 8) carry
    the shotguns.
- **AI assistants (MCP)** can look up a caliber's cartridge measurements,
  tolerances and how common it is, list the most popular calibers for a kind
  of hunting or competition, and save a cartridge as an STL for a
  3D-printed dummy.
- **Start over** when the database is damaged or from a newer version: the
  desktop app can begin with an empty database, behind a dialog with a box
  to tick. Nothing is deleted; the old file is kept under a new name.
- **Cartridge measurements:** beside the 3D cartridge in the bullet catalog
  and the rifle editor, a drawing of the cartridge with its measurements:
  the C.I.P. drawing's letters, each value the largest the standard allows,
  and the range down to the smallest where the drawing gives a tolerance.
  Hover over it for what each letter means. Where there isn't room beside
  it, it folds under **Measurements**.
- **3D animals:** the moose, red deer, fallow deer, roe deer, sika deer,
  white-tailed deer, wapiti, wild forest reindeer, wild boar, mouflon, brown bear,
  wolf, lynx, wolverine, raccoon dog, mink, beaver, muskrat, red squirrel,
  brown hare, mountain hare, rabbit, grey seal, ringed seal (also shown for
  the harbour seal), fox, black grouse, hazel grouse, willow ptarmigan and
  rock ptarmigan have a 3D body,
  to scale. The target's page shows it straight from the side (drag to
  turn it, **From the side** to go back), and it stands in the 3D view and
  through the scope. Its hit area and rings show through the body, sized
  as on the moose test figure.

### Changed

- **Main pages fill the window:** the empty space beside a page's content
  now scrolls with it.
- **Wapiti:** its own drawing instead of the red deer's, a bull's size
  (1.5 m at the shoulder, 2.4 m long, was 1.37 and 2.3 m) and a 40 cm vital
  zone (was 30 cm), so its ethical range is longer.
- **Red deer vital zone:** 28 cm across (was 25 cm), measured on the 3D stag
  at full size; its ethical range is a little longer.
- **Fox vital zone:** now 10.5 cm across (was 8 cm), measured on the 3D fox
  at full size. The ethical range for the fox is longer to match.
- **More vital zones measured on the 3D bodies at full size**, so these
  ethical ranges are longer:
  - raccoon dog 11 cm (was 9);
  - beaver 11 cm (was 10);
  - muskrat 5 cm (was 4);
  - red squirrel 4 cm (was 3);
  - rabbit and willow ptarmigan 6 cm (was 5);
  - grey seal 28 cm (was 20), on a 2.3 m bull;
  - ringed seal 21 cm and harbour seal 24 cm (both were 12).

### Fixed

- **Updating when the data won't open:** the desktop app's page for a
  database it can't open now checks for updates (at once if you turned
  updates on) and can install one, so data saved by a newer version can be
  opened by updating. It also explains when that is the cause, and shows in
  your language.

## [0.10.1] - 2026-11-13

### Security

- **SQLite 3.54.1** in the desktop and web apps, for a flaw in older
  versions (CVE-2026-119375) that a crafted database file could use to read
  stale memory. The app only ever opens its own database, so it wasn't
  exposed, but it now runs the fixed version. The web app keeps its data in
  the browser exactly as before; nothing moves.

## [0.10.0] - 2026-11-13

### Added

- **Garmin watch:** every rifle and load on a Garmin watch (the epix (Gen 2)
  first), through a new watch app. Pair once with a code; after that the app
  keeps the watch up to date while it is open, encrypted for that watch
  only. On the watch you choose the rifle, set the range, the wind and the
  temperature, and read the clicks and the centimetres; the watch reads the
  air pressure itself, and counts the wind's drift, aerodynamic jump and
  head- or tailwind.
- **Cartridges in 3D:** the bullet catalog shows the caliber's cartridge in
  3D, true to its C.I.P. drawing, with a typical bullet and a millimetre
  ruler; drag to turn it. The catalog keeps each drawing's printed
  tolerances too. The printed copy can have the caliber's name cut into
  its side, so dummies of similar calibers can be told apart.
  **Save for 3D printing (STL)** saves it in millimetres, standing on its
  head, to try magazines and feeding. The rifle editor shows it too. For
  every caliber in the catalog, each from its C.I.P. drawing.
- **54 more calibers** in the catalog, among them 9.3×66 Sako, 7.62×53R,
  the Blaser and Nosler magnums, .300 Norma Mag, 6.5×54 M-S, the
  benchrest and F-class sixes (6mm BR, Dasher, PPC, 6XC, 6mm GT), 6mm ARC,
  the PRS quarter-bores (25 Creedmoor, 25×47 Lapua, 25 GT) and 6mm BRA,
  7.5×55 Swiss, the CheyTacs, .416 Barrett and Hellfire, .50 BMG and the Nitro
  Expresses. Each caliber has rough popularity grades for hunting and
  competition.

### Fixed

- **Caliber catalog:** 6.5×52R takes .257 in bullets (6.53 mm) and 8×57 JR
  .318 in (8.08 mm); the catalog had 6.71 and 8.20 mm, so it offered
  component bullets of the wrong size for them.

## [0.9.2] - 2026-11-05

### Fixed

- **Number fields:** a decimal comma counts ("3,8" was read as 3), and a
  typed unit is converted ("140 gr" in a gram field, "50 F" in a °C field,
  "1,5\"" in a cm field). Text that can't be read, or a number outside the
  field's range, keeps the value from before with a note saying why,
  instead of quietly becoming the nearest allowed number. Escape puts the
  old value back.
- **Phones:** a swipe that starts on a number field scrolls the page; it
  used to change the number.
- **Delete:** a double click or double tap can't confirm a delete any more;
  "Really delete?" takes clicks after a moment, and Cancel takes the first
  button's place. Deleting the synced copy asks in a dialog of its own.
- **Back and reload in the web version:** on a phone, Back (and the swipe
  back) closes the open editor or sheet instead of leaving the app, and
  asks before dropping unsaved changes, as the ‹ links now do too.
  Leaving or reloading the page with unsaved changes asks first.
- The bullet form warns about a muzzle velocity no rifle load has (a
  number typed in fps in an m/s field, say). The rifle and bullet forms say
  that the barrel and test-barrel lengths don't change the velocity used.
- **Aerodynamic jump went the wrong way.** With a right-hand twist, wind
  from the right now lifts the bullet and wind from the left lowers it, as
  in Litz's work and Hornady's 4DOF calculator. Before, it was reversed:
  in a 4 m/s full crosswind the hold was about 7 cm off at 300 m and 14 cm
  at 600 m.
- **Forecast pressure:** a sea-level pressure is taken back to your
  altitude through air at the day's temperature, the way forecasts reduce
  it, instead of through the standard atmosphere. On cold days in the
  mountains the air pressure used was too high: about 1 % at 1000 m and
  2 % at 2000 m on a −20 °C day.
- **Air and gravity:** humid air now carries sound a little faster (it
  changes the Mach number slightly on hot, humid days), and with Coriolis
  on, gravity follows the latitude. Both are a couple of centimetres at
  1000 m.
- **BC by velocity:** the bullet form and the export format now say what
  a band's speed means: the BC applies at that speed and is blended in
  between, so a maker's band is best entered at its middle speed. The
  format used to describe steps, which the calculation never did.

## [0.9.1] - 2026-11-02

### Added

- **Scopes:** the first-generation Vortex Viper PST (1-4x24, 2.5-10x32 FFP,
  2.5-10x44, 4-16x50 FFP and SFP, 6-24x50 FFP and SFP, each in MOA and
  MRAD) with their EBR-1 and TMCQ reticles, drawn from Vortex's sheet for
  each model.

## [0.9.0] - 2026-10-31

### Added

- **Windage label:** a fourth card, a strip for the windage turret with a
  band for each of up to six ranges and the wind speeds at their clicks,
  right of the 0 for a wind from the right and left for a wind from the
  left. Full-value wind, without spin drift; numbers every 1 to 10 m/s (or
  your wind unit) with dots between. The windage turret is measured once
  per rifle; the look and label designs are shared with the turret label.
  The A4 sheet has a table of clicks for each range and wind.
- A line option under label numbers and intermediate ticks and dots, with
  a two-handle control and editable percentages for its start and end. Both
  positions are included in named designs; existing length settings are kept
  as the end position.
- Named label designs per rifle: save and apply appearance presets, make copies
  with a new name, update, rename or delete them. Fonts, colours and print
  choices are kept on the device, alongside the other card settings.

### Fixed

- Inset the line-position slider so its end handles stay clear of the panel scrollbar.
- Keep wrapped label marker choices fully visible, with line-position controls
  immediately below the choices and above the display options.
- Apply tick and triangle label markers beneath intermediate ticks and dots,
  as well as beneath the numbers.
- Keep mobile update messages fully visible above the bottom navigation from
  the first frame, without leaving space after the message expires.

## [0.8.8] - 2026-10-26

### Compatibility

- Existing bullets and settings are retained, and older backups still import.
  This version upgrades the local database to schema 10 and writes backup
  format 7 to preserve data attribution. Older app versions cannot open the
  updated database or import these new backups. Update all syncing devices.

### Fixed

- Reject non-finite core inputs and invalid maximum ranges, and prevent a NaN
  query from panicking in the BC-band lookup.
- Corrected selected Fiocchi and PPU catalog BCs after reviewing source conflicts.
  The Fiocchi 165 gr GameKing now uses G1 instead of its disputed G7 value.

### Added

- Added 1,034 ChamberSim component records as a separately attributed CC BY-SA
  4.0 data collection. Source uncertainty notes and record links are retained;
  existing catalog entries are unchanged. Data attribution survives personal
  copies, backups and sync. Backup format 7 and database schema 10 preserve the
  new field; older apps cannot open the newer database or import these backups.
- BC source labels for Aguila and the retailer-sourced Hornady and Winchester
  rimfire entries, identifying estimates, user-supplied values and provisional
  matches to other bullets. Catalog source notes are available in the bullet editor.

## [0.8.7] - 2026-10-25

### Added

- Added 260 Federal rifle and rimfire loads from its official product pages,
  bringing Federal to 275 profiles across 46 supported calibers. The
  unlabelled BCs are stored as G1; separately labeled G7 values are retained.
  Identical ballistic listings across part numbers and package sizes share a
  profile. Loads without a published BC and loads in unsupported calibers are
  not included.
- Added 163 Norma factory loads from its official product pages across 39
  supported calibers. The pages publish G1 BCs; no G7 BCs were found for
  these loads. Profiles without a published BC and duplicates of existing
  Norma loads are not included.
- Added 46 Fiocchi rifle loads from its official product pages, retaining
  published G1 coefficients and G7 values where available. The Fiocchi
  rimfire listings are not included because their product pages do not
  publish BCs.
- Added 17 Prvi Partizan standard rifle loads from the supported calibers in
  its 2023 catalog's first standard-ammunition tables, matching their Bullet
  Art. numbers to the rifle-bullet table, plus 20 Match loads and 18 rifle
  component bullets from its 2023 catalog. The Match loads publish G1 BCs;
  standard-bullet BCs are listed without a drag-model label and stored as G1
  by catalog convention. Load-specific Match BCs remain distinct where they
  differ from the component table. The .50 Browning Match entry is not
  included because the caliber is not yet defined here.
- Added 152 Sellier & Bellot rifle loads from its eXergy Edge and Blue, SP,
  SPCE, HPC, PTS, Sierra, FMJ and Target Match lines. Each profile includes
  both manufacturer-published G1 and G7 BCs. Added 12 calibers, including
  .17 Hornet, 5.6×50R Magnum, 6.5 Grendel, 6.5×52R, 6.8 Rem SPC,
  .30 Carbine, 7.62×54R, 7×65R, 8×57 JR/JRS, 8×64 S and 9.3×72R.
  7×65R remains distinct from 7×64.
- Added 11 lead-free Tarvas component bullets, including its target bullet,
  the .308 Win 165 gr Tarvas cartridge linked to the matching bullet, and
  GGG's .308 Win 165 gr TARVAS cartridge. Both cartridge records use the
  published muzzle velocity of 825 m/s.
- Added 109 Sierra rifle-hunting component bullets and the .350 Legend and
  .450 Bushmaster calibers. Sierra lists projectiles for reloading, not
  factory ammunition; listings without a published BC are omitted.
- Added 11 GGG factory loads listed in its brochure and three GGG FMJ
  component bullets, using the published weights, ballistic coefficients
  and muzzle velocities.
- Added all 41 remaining CCI single-projectile rimfire load profiles, bringing
  the catalog to 48 CCI profiles. Package-size variants share entries where
  their ballistic data match. Added the .17 Mach 2, .22 Long, .22 Short and
  .22 WRF calibers; .22 Long remains distinct from .22 LR. Multi-pellet
  shotshells and the noise blank are outside the single-projectile catalog.
- Added .300 AAC Blackout (.300 BLK / 7.62×35) with its .308-inch bullet
  diameter and a typical 1:8 twist.
- Added seven Barnes .300 AAC Blackout factory-load profiles, including
  VOR-TX, Suppressor Series, Precision Match and Multi-Purpose JHP loads.
- Added nine SK .22 LR factory-load profiles using SK's published ballistic
  chart data.
- Added Aguila's 19 listed rimfire loads and the .22 Win Auto and 5mm
  Remington Rimfire Magnum calibers. G1 BCs are labeled as estimates where
  Aguila does not publish them, except for its 20 gr 17 HMR JHP, which uses
  the matching Hornady 20 gr XTP BC. Aguila's internally inconsistent 17 HMR
  velocity table is not used.

## [0.8.6] - 2026-10-25

### Added

- Expanded the bullet catalog with Berger, CopperBear, Infitech, Speer,
  Swift and Woodleigh bullets, and added several calibers.
- Catalog ammunition without a published muzzle velocity can now be added;
  enter your load's velocity before saving it.

## [0.8.5] - 2026-10-24

### Fixed

- The Windows app builds again: 0.8.4 came out for the web only, without
  the desktop downloads, because of a mismatch between two of the graphics
  library's parts on Windows.

## [0.8.4] - 2026-10-24

### Changed

- The app runs on a newer version of its user-interface library (egui
  0.37). It should look and work as before. The hold's arrows are plain
  arrows, and on a computer, symbols and emoji the app's own fonts lack come
  from the system's fonts, as they do in a browser.

### Fixed

- The desktop app's secure connections (weather, sync, updates) now refuse
  certificates from an authority whose keys have leaked (an update of the
  TLS library, rustls-webpki).

## [0.8.3] - 2026-10-24

### Fixed

- **Google sign-in is safer.** The sign-in is handed only to the app that
  started it: a sign-in link someone else sends you can no longer give them
  your Drive data. A sign-in no app picks up is deleted within half an hour.
  On a phone's home-screen app, type in the code the sign-in page shows if
  the app doesn't continue by itself. Apps before this version can't sign in
  to Google any more (they still sync once signed in): update them.
- The sync server limits how many requests one sender can make, and stops
  taking new synced copies before it runs out of room, so nobody can fill
  it up and stop everyone's sync.
- After *Delete the synced copy*, signing in to Google again starts syncing
  again instead of stopping at once.
- The web app refuses to open in a second tab while it is open in another,
  which could damage the data kept in the browser.
- Importing a Strelok file with some non-English letters in a bullet's
  name no longer crashes the app.
- The tool for AI assistants only reads the app's database, and never
  upgrades it for a newer version.
- The desktop updater writes the new version beside the app, not in a
  shared temporary folder.

## [0.8.2] - 2026-10-22

### Added

- **Updates on a computer.** In the menu, *Updates…* can check for a new
  version when the app starts (off until you turn it on), or check now.
  *Update now* downloads it, checks that it is signed by the app's maker,
  puts it in place and offers to restart. The check asks GitHub, where the
  releases are kept; nothing else is sent. It works from this version on:
  this one still has to be installed by hand.

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

[Unreleased]: https://github.com/malx122/mx_shooting_calculator/compare/v0.12.1...HEAD
[0.12.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.11.1...v0.12.0
[0.11.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.10.1...v0.11.0
[0.10.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.9.2...v0.10.0
[0.9.2]: https://github.com/malx122/mx_shooting_calculator/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/malx122/mx_shooting_calculator/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.8...v0.9.0
[0.8.8]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.7...v0.8.8
[0.8.7]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.6...v0.8.7
[0.8.6]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/malx122/mx_shooting_calculator/compare/v0.8.1...v0.8.2
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

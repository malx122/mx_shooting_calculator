# The export file

MX Shooting Calculator saves the hunter's own data as one JSON file:
**Export backup** in the app's menu writes it, **Import backup…** reads it,
and the AI tools (`mxsc-mcp`) and sync use the same format (on the sync
server it is compressed with deflate and encrypted on the device; in
Google Drive it is the plain file). This page describes the file so other
programs can read and write it.

- **Schema:** [`export-format.schema.json`](export-format.schema.json)
  (JSON Schema 2020-12), also published at
  <https://malx122.github.io/mx_shooting_calculator/download/export-format.schema.json>.
- **Field reference:** [below](#field-reference). It and the schema are
  generated from the app's code, and a test fails when the code changes
  without them, so they always describe what the app writes.

The file is UTF-8 JSON, usually named `mxsc-backup.json`.

Device-local settings are not included: calculator inputs, card layouts,
stands and named label designs stay on the device and are not synced.

```json
{
  "format": "mxsc/personal-data",
  "version": 9,
  "exported_at": 1792473562000,
  "rifles": [ { "id": "…", "name": "Browning X-Bolt", "caliber": "308-win", "twist_in": 10.0, … } ],
  "bullets": [ { "id": "…", "manufacturer": "Federal", "name": "Terminal Ascent", "mass_g": 11.34, … } ],
  "rifle_loads": [ { "id": "…", "rifle_id": "…", "bullet_id": "…", "velocities": [ … ], … } ],
  "scopes": [],
  "targets": [],
  "shots": [ { "id": "…", "taken_at": 1792473562000, "rifle_id": "…", "range_m": 150.0, "outcome": "hit", … } ],
  "shotguns": [ { "id": "…", "name": "Beretta Silver Pigeon", "caliber": "12-76", "barrels": [ … ], … } ],
  "shells": [ { "id": "…", "caliber": "12-70", "manufacturer": "Gyttorp", "name": "Jakt 36 g US 3", "payload": { … }, … } ],
  "shotgun_loads": [ { "id": "…", "shotgun_id": "…", "shell_id": "…", "patterns": [ … ], … } ],
  "barrels": [ { "id": "…", "rifle_id": "…", "name": "Factory", "length_mm": 510.0, … } ],
  "round_log": [ { "id": "…", "rifle_id": "…", "barrel_id": "…", "rounds": 5, "at": 1792473562000, … } ],
  "cleans": [],
  "zero_resets": [],
  "sessions": [ { "id": "…", "place": "Boden", "started_at": 1792470000000, "temperature_c": 6.0, … } ],
  "photos": [ { "id": "9f86d0…", "width_px": 4032, "height_px": 3024, "mime": "image/jpeg", "calibration": { "kind": "two_point", … }, … } ],
  "groups": [ { "id": "…", "taken_at": 1792473562000, "marks": { "on": "photo", "photo_id": "9f86d0…", "shots": [ … ] }, "distance_m": 100.0, … } ],
  "ladders": [],
  "analyses": [],
  "deleted": [],
  "catalog": { "calibers": [ … ], "scopes": [ … ], "reticles": [ … ] }
}
```

## Conventions

- **Licensed catalog data:** a copied bullet may contain `data_attribution`.
  Preserve it when sharing, adapting, importing or re-exporting the record,
  including when its catalog link or personal notes change. It identifies the
  source, revision, credit, license and initial adaptation. These terms apply to
  the copied data, not the user's independent notes or the app's source code.
  The app writes format 7 so older versions reject such backups rather than
  silently discarding attribution. Older backups without the field still import.
  Both attribution URLs must use HTTPS; unsafe schemes are rejected on import.

- **Units are in the field names**, always metric:
  - lengths: `_mm`, `_cm`, `_m`;
  - speeds: `_mps` (metres per second);
  - temperatures: `_c` (°C);
  - pressures: `_hpa`;
  - energies: `_j`;
  - masses: `_g`;
  - angles: `_mrad`, `_degrees`;
  - places: `latitude_degrees` and `longitude_degrees` (WGS84, as GPS gives
    them; north and east positive);
  - times: `_at` and `_ms` (milliseconds since 1 January 1970, UTC), and
    `_s` for seconds of flight;
  - points in a photo: `_px`, pixels of the photo as the app keeps it.

  Twist is the one exception: `twist_in` is the inches of barrel per turn
  of the rifling, as the trade gives it.
- **Signs:** heights are positive above the line of sight; angles and
  offsets are positive up and to the right; a shot angle is positive uphill.
  Points on a target are the same: in millimetres, x to the right and y up.
  Points in a photo are as images are stored: x to the right and y down
  from the photo's top-left corner.
- **Points** are arrays of two numbers, `[x, y]`.
- **Ids** are strings, unique within each kind of record. Records refer to
  each other by id: a rifle load names its `rifle_id` and `bullet_id`, and a
  shotgun load its `shotgun_id` and `shell_id`; a rifle's `scope_id` names
  one of the file's `scopes`, or else a scope in the app's catalog. A
  photo's id is the SHA-256 of its bytes, in lowercase hexadecimal. Apart
  from a rifle load's and a shotgun load's, a reference may name a record
  that has since been deleted: a group keeps naming its rifle, and a reader
  should skip what it can't find.
- **References to the app's catalog:** a `caliber` is a key such as
  `308-win` (for a shotgun or a shell, such as `12-76`), a `scope_id` may name a catalog scope, a `reticle_id` a catalog
  reticle, and an animal target's `silhouette` one of the app's drawings. The
  `catalog` section of an exported file writes out the calibers, scopes and
  reticles the records name, so a reader doesn't need the app. Sync copies
  leave it out.
- **`updated_at`** is when a record was last changed. Importing merges
  record by record: a new record is added, an identical one is left alone,
  and otherwise the copy changed last wins.
- **`deleted`** lists records deleted since, so a merge deletes them on the
  other side too. Records that belong to another aren't listed when it is
  deleted: they follow it. A rifle's or bullet's `rifle_loads`, and a
  rifle's `barrels`, `round_log`, `cleans` and `zero_resets`, go when the
  rifle is listed as deleted and isn't in the data (round log entries,
  cleans and zero resets naming a barrel that is still there stay with the
  barrel); a barrel's `cleans` and `zero_resets` go when the barrel is
  listed and isn't there. So if a rifle is deleted on one device and edited
  later on another, it comes back with everything that belongs to it.
  Photo records are never listed: see [Shot groups](#shot-groups).

## Calculating with it

A trajectory needs, for a rifle and a bullet of the same `caliber`:

- **From the rifle:**
  - the sight: `sight_height_cm`;
  - the zero: `zero_range_m` and `zero_height_cm`, made in the weather `zero_temperature_c`, `zero_pressure_hpa` (a barometer reading at the range), `zero_humidity_percent` and `zero_powder_temperature_c`;
  - the barrel: `twist_in` and `right_hand_twist`, for spin drift and stability;
  - the turret: `click_unit` and `click_value`, for holds in clicks.
- **From the bullet:**
  - `mass_g`, `diameter_mm`, and `length_mm` (for stability);
  - the drag model `drag_model`, used as follows:
    - `G1` or `G7`: the BC `bc_g1` or `bc_g7`, or `bc_bands` when there are any;
    - `Custom`: the `drag_curve`;
  - the velocity: `muzzle_velocity_mps` at `velocity_temperature_c`, changing by `velocity_per_degree` for each °C of powder temperature.
- **From the rifle load for the pair, if there is one:**
  - `velocities` replace the bullet's velocity: with two or more, fit a line through them for the temperature effect;
  - `impact_offset_mrad` tilts the bore;
  - `zero` says which weather the zero was made in;
  - `truing` multiplies the velocity or the drag by its `factor`.

## The shot log

Each of `shots` is a shot the hunter logged from the calculator. It keeps
what the app said at the time, so it reads the same after the rifle, the
bullet or the target is changed or deleted:

- what was shot with and at: `rifle_id`, `bullet_id` and `target_id`, with
  their names then (`rifle_name`, `bullet_name`, `target_name`);
- the shot: `range_m` along the line of sight, `inclination_degrees`, the
  wind and the `weather`;
- the `hold` the app gave, as an angle (`elevation_mrad`, `windage_mrad`)
  and in the rifle's clicks, and the bullet's path at the range
  (`solution`);
- what came of it: `outcome`, `hit_where` and `notes`.

After a shot at game the hunter can mark where they stood (`position`, with
the device's `accuracy_m`) and take the `bearing_degrees` to the animal,
from true north. From these the app works out `animal_spot`: the point at
the level distance (`range_m` × cos `inclination_degrees`) along that
bearing, on a sphere of the Earth's mean radius (6 371 008.8 m), and a
`search_radius_m` around it. The radius adds up, as the root of the sum of
the squares, a compass error of ±10° (about 17 % of the distance
sideways), a range error of 5 % (at least 5 m), and the position's
accuracy (10 m when not known); it is at least 10 m. A reader may work the
spot out again from the other fields; the app does whenever it saves the
record.

Positions are personal: they say where the hunter hunts. The app keeps them
on the hunter's devices, in their sync copy (encrypted on the sync server,
or in the app's own folder of their Google Drive) and in the files they
export, and sends them nowhere else.

## Shotguns

Shotguns are kept the way rifles are:

- each of `shotguns` is a gun: its `caliber`, a key in the app's catalog of
  shotgun calibers such as `12-76` (gauge 12, 76 mm chamber), and `barrels`,
  each with its
  `choke`, given either by name (`"by": "name"`, such as `modified` for half
  choke) or by its constriction in mm (`"by": "constriction"`). For slugs,
  `sight_height_cm` and `slug_zero_m` give its sights;
- each of `shells` is ammunition for a `caliber`, as a bullet is for a
  rifle caliber: a `12-70` shell fits guns of gauge 12 with a chamber as long
  or longer (12/70, 12/76, 12/89), never shorter. It is loaded with shot (`material`, `pellet_mm`, `load_g`) or a slug
  (`mass_g`, `diameter_mm`, `bc_g1`), with `velocity_mps` as the box gives
  it and `velocity_at`, where it was measured (`muzzle`, `v2_5` at 2.5 m,
  `three_feet`, or `observed`, the British average over the first 20
  yards);
- each of `shotgun_loads` is a shell in a gun (`shotgun_id`, `shell_id`), as
  a rifle load is a bullet in a rifle, with the `patterns` the hunter shot
  and counted: the `barrel` (its index in the gun's `barrels`), the
  `distance_m`, the `circle_cm` and `pellets_inside`. The share inside is
  `pellets_inside` over `pellets_in_load`, or over the pellets the shell's
  `load_g` of `pellet_mm` shot holds when that is left out. A board with
  `pellets_inside` 0, or more than that worked-out count, is kept but not
  used; one with more than its own `pellets_in_load` is refused. A board for
  a barrel the gun no longer has is kept but not used.
  There is one record for each gun and shell: a file with two is merged
  into one, the later winning, as rifle loads are by their rifle and bullet.

Pellet sizes are kept in mm, since a number on a box stands for different
sizes in different countries (US 5 is 3.05 mm, English 5 is 2.79 mm).

## Shot groups

Each of `groups` is a group of shots on a target, with what was shot and
where. Its `marks` say where the shots are:

- **On a photo** (`"on": "photo"`): the photo is one of `photos`, named by
  `photo_id`. Shots, the aim point and the calibration's points are in the
  photo's pixels (`x_px`, `y_px`), so a better scale later never moves a
  shot. The photo's `calibration`, one for every group marked on it, turns
  pixels into millimetres on the target:
  - `two_point`: two points `distance_mm` apart give the scale; the photo is
    taken to face the target squarely, its x to the right and y down;
  - `four_corner`: the corners of a rectangle `width_mm` × `height_mm`,
    from its top left clockwise, map the photo onto the target's plane (a
    homography), which also straightens a photo taken at an angle. Tapped
    corners are taken as the photo shows them, so the target's up is the
    photo's; with `ordered: true` (corners found from a printed MX target's
    marks, see [mx-targets.md](mx-targets.md)) they are the rectangle's own
    top left, top right, bottom right and bottom left, and its up is the
    rectangle's whichever way the photo is turned.
  Until its photo has a calibration a group has no sizes. Several groups
  may be marked on one photo, each with its own aim point.
- **On a grid** (`"on": "grid"`): a blank grid at true scale, for targets
  that weren't photographed. Shots and the aim point are in millimetres from
  the grid's centre (`x_mm`, `y_mm`), x to the right and y up.

Sizes as angles need `distance_m`. Without an aim point (`aim_px` or
`aim_mm`) a group has no impact or correction. `dialled_offset_mrad` is what
was dialled on the scope compared with the zero; zero checks subtract it
from the group's impact. Shots carry their own `velocity_mps`;
`extra_velocities_mps` are readings that match no marked shot, such as
foulers. How a group was last zoomed and panned is kept on each device and
is not in the file.

A group may name the session it was shot in (`session_id`); the session's
weather applies unless the group's own `conditions` give a value. A ladder's rungs
are groups naming the `ladder_id`, each at its `rung_value`: grams of
powder for a `charge` ladder, the cartridge's base-to-ogive length (CBTO)
in millimetres for a `seating` one. Other groups of a handload keep the
recipe's `charge_g` and `cbto_mm` as they were when the group was saved,
so a later change to the recipe doesn't move them on a trend; where a
group has none, readers may fall back to its bullet's recipe.
`analyses` are saved comparisons and composites; the app works them out
again from their groups.

**Photos** are not in the file: `photos` describes them (size, type, when
taken, calibration). The app keeps the photos themselves, and a thumbnail
of each, on the device, as JPEG named by their id: on a computer in a
folder beside the database file named after it (`mxsc-photos` beside
`mxsc.sqlite`), in the browser in the site's storage. The kept JPEGs carry
no camera metadata: no time or place, no camera or owner.

A photo record is never listed in `deleted`. A device drops its record
when no group there is marked on it any more, and a merge brings in a
photo record only for a group that is marked on it (or when the device has
the record already). So deleting a photo on one device never takes it from
a group made on another.

**Round counts** are worked out, never stored:
- the action's count is the rifle's `rounds_before_logging` plus every
  entry of `round_log` for the rifle, whatever its barrel;
- a barrel's count is its own `rounds_before_logging` plus the entries
  naming it, on whichever rifle they were fired, so a barrel keeps its
  count when it moves to another rifle;
- the count since the last clean adds the entries for the barrel made
  after its latest entry in `cleans` naming it; with no clean logged it is
  unknown. Entries and cleans with `barrel_id` `null` count only for a
  rifle without barrels: where a rifle has barrels, the app always names
  the one fitted.

The round log entry the app makes when a group is saved names the group
(`group_id`) and has an id made from it (a version 5 UUID of the group's
id, in the namespace `6d787363-726f-756e-6473-8f2a41c6d903`), so a group
saved on two devices is counted once.

## A backup with photos

**Export backup with photos** writes one `.zip` file,
`mxsc-backup-with-photos.zip`, that also holds the photos the backup's
groups are marked on:

```
mxsc-backup-with-photos.zip
├── mxsc-backup.json          the backup file described here
└── photos/
    ├── 9f86d081…0a08.jpg     one JPEG per photo this device has,
    └── …                     named by its id (the SHA-256 of its bytes)
```

- `mxsc-backup.json` is exactly what **Export backup** writes, deflated.
- Each photo is stored as it is kept on the device (JPEG, without camera
  metadata), uncompressed. A photo kept only on another device is left
  out; its record is still in the backup.
- Other files in the zip are ignored.

**Import backup…** reads such a zip as it reads the plain file: the records
are merged as from `mxsc-backup.json`, and each photo one of its `photos`
records names is put back in the device's photo store, with a new
thumbnail; other photos are left in the zip. Before anything is unpacked,
a zip is refused if it holds two files of one name, more than 20 000
files, more than 2 GiB unpacked (512 MiB in the browser), a backup file
over 256 MiB, a photo over 64 MiB, or a file over 1 MiB that unpacked is
more than 100 times its packed size. A photo that isn't a JPEG or whose
bytes don't match its name is refused, and with it the whole file.

## Groups as CSV

**Groups as CSV** writes a spreadsheet file in metric units, comma
separated, with a header row; times are UTC (ISO 8601), decimals use a
point. A text that starts with `=`, `+`, `-`, `@`, a tab or a carriage
return (a name, a tag or a note) gets a `'` in front, so a spreadsheet
shows it rather than running it as a formula.

- `mxsc-groups.csv`, one row per group: `group_id`, `taken_at_utc`, the
  `rifle`, `barrel` and `bullet` by name, the recipe's `powder`,
  `charge_g` and `cbto_mm`, `distance_m`, the `session`'s place, `shots`
  and `counted_shots` (without cold-bore shots and called fliers), the
  sizes in mm and mrad (`extreme_spread_*`, `mean_radius_*`, `sigma_*`,
  `cep_*`, `width_mm`, `height_mm`), the impact from the aim
  (`impact_right_*`, `impact_up_*`), the conditions (`temperature_c`,
  `powder_temperature_c`, `pressure_hpa`, `humidity_percent`,
  `wind_speed_mps`, `wind_clock`; the group's own, else its session's), the
  velocities (`velocity_count`, `velocity_avg_mps`, `velocity_sd_mps`,
  `velocity_es_mps`), `tags`, `notes` and `archived`.
- `mxsc-shots.csv`, one row per shot: `group_id`, `taken_at_utc`, `rifle`,
  `bullet`, `distance_m`, `shot` (its number in marking order),
  `cold_bore`, `called_flier`, `counted`, where it is from the aim
  (`right_of_aim_mm`, `up_from_aim_mm`, `right_of_aim_mrad`,
  `up_from_aim_mrad`) and from the centre of the shots that count
  (`right_of_centre_*`, `up_from_centre_*`), and its `velocity_mps`.

A group's page saves its own shots the same way (**Shots (CSV)**), and the
group as a picture (**Picture (PNG)**).

## Writing a file for the app

The app reads files written by other programs:
- `format` must be `mxsc/personal-data`, and `version` no higher than the app knows (9).
- Fields marked "may be left out" can be omitted.
- Unknown fields are ignored, and the `catalog` section is never imported.
- Every record is checked before anything is saved. Numbers must be finite and within what guns and their ammunition can be. If any record fails, nothing from the file is imported, and the app says which record failed.

## Versions

| Version | What changed |
|---|---|
| 1 | Rifles, bullets and loads with one measured velocity. |
| 2 | Loads hold a list of velocities, an impact offset and their zero weather. |
| 3 | The hunter's own scopes; bullets with their own drag curve. |
| 4 | The hunter's own targets. |
| 5 | Deleted records. Later, still version 5: the `catalog` section (ignored by imports), truing, shot statistics on velocities, a zero height. |
| 6 | The shot log (`shots`). |
| 7 | Attribution for separately licensed bullet data (`data_attribution`). |
| 8 | Shotguns (`shotguns`) with their chokes, shotgun shells (`shells`), and the pattern boards of a shell in a gun (`shotgun_loads`). |
| 9 | Shot groups (`groups`) on photos (`photos`) or grids, with barrels, the round log, cleans, zero resets, range sessions, ladders and analyses; rifles name their fitted barrel and rounds fired before logging; handloads keep a `recipe`. |

## Field reference

<!-- field reference: generated from the code, see schema.rs -->

### The file

A file of the hunter's own records: an export, a backup or a sync copy.

| Field | Type | Meaning |
|---|---|---|
| `format` | string | Always `mxsc/personal-data`. Files from the app's earlier names say `mhc/personal-data` or `ballistic-calculator/personal-data`. |
| `version` | integer | The format's version; this description is of version 9. A reader should refuse versions newer than it knows. |
| `exported_at` | integer | When the file was written, in milliseconds since 1970 (UTC). |
| `rifles` | array of [Rifle](#rifle) | The hunter's rifles. |
| `bullets` | array of [Bullet](#bullet) | The hunter's bullets: factory ammunition and handloads. |
| `rifle_loads` | array of [RifleLoad](#rifleload) | What is particular to a bullet in a rifle: measured velocities, point of impact, zero weather and truing. |
| `scopes` | array of [UserScope](#userscope) | Scopes the hunter entered (catalog scopes are only named by id). |
| `targets` | array of [Target](#target) | Targets the hunter made or copied (catalog targets aren't included). |
| `shots` | array of [Shot](#shot) | The shot log: shots the hunter took, with the solution the app gave and, after a shot at game, where the hunter and the animal stood. |
| `shotguns` | array of [Shotgun](#shotgun) | The hunter's shotguns, each with its barrels' chokes. |
| `shells` | array of [Shell](#shell) | The hunter's shotgun shells: shot and slugs, by caliber. |
| `shotgun_loads` | array of [ShotgunLoad](#shotgunload) | What is particular to a shell in a shotgun: the pattern boards counted with it. |
| `barrels` | array of [RifleBarrel](#riflebarrel) | Barrels of the hunter's rifles, fitted or not. |
| `round_log` | array of [RoundLogEntry](#roundlogentry) | Rounds fired, for the round counts of actions and barrels. |
| `cleans` | array of [Clean](#clean) | When barrels were cleaned. |
| `zero_resets` | array of [ZeroReset](#zeroreset) | When rifles' zeros were set again. |
| `sessions` | array of [Session](#session) | Days at the range, with their weather. |
| `photos` | array of [Photo](#photo) | What is known about the target photos groups are marked on. The photos themselves are not in the file. |
| `groups` | array of [Group](#group) | Shot groups: shots marked on a photo or a grid, with what, where and how they were shot. |
| `ladders` | array of [Ladder](#ladder) | Load developments over several groups. |
| `analyses` | array of [Analysis](#analysis) | Saved comparisons and composites of groups. |
| `deleted` | array of [Deletion](#deletion) | Records deleted, so merging the file deletes them elsewhere too. |
| `catalog` | [CatalogDetails](#catalogdetails) | The catalog entries the records refer to by id, written out so the file can be read without the app. Only in exported files; importing ignores it. May be left out. |

### Rifle

A rifle: its caliber, barrel, scope and zero.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The rifle's id, unique among rifles. |
| `name` | string | The hunter's name for the rifle. |
| `caliber` | string | The caliber's key in the app's catalog, e.g. `6-5x55-se` (see `catalog`). |
| `barrel_length_mm` | number or null | Barrel length; `null` when not known. |
| `twist_in` | number | Barrel length for one turn of the rifling, in inches. |
| `right_hand_twist` | boolean | Whether the rifling turns to the right (clockwise seen from behind), as most do. |
| `sight_height_cm` | number | Height of the scope's axis above the bore's axis. |
| `zero_range_m` | number | The range the rifle is zeroed at. |
| `zero_height_cm` | number | Where the zero puts the bullet at the zero range: above (positive) or below the line of sight, cm. Hunters often zero "4 cm high at 100 m"; zero for a plain zero. |
| `zero_temperature_c` | number | Air temperature when the rifle was zeroed. |
| `zero_pressure_hpa` | number | Air pressure when the rifle was zeroed, as a barometer at the range reads it (not reduced to sea level). |
| `zero_powder_temperature_c` | number or null | Powder temperature at the zero; `null` means the air temperature. |
| `zero_humidity_percent` | number | Relative humidity when the rifle was zeroed. |
| `table` | [TableSettings](#tablesettings) | The range table's ranges and columns. |
| `click_unit` | [ClickUnit](#clickunit) | The unit of the scope's clicks. |
| `click_value` | number | One click of the turrets, in `click_unit`: 0.1 for 0.1 mil, 0.25 for 1/4 MOA. |
| `scope_id` | string or null | The mounted scope: the id of one of the file's `scopes`, or of a scope in the app's catalog (see `catalog`); `null` for none. |
| `reticle_id` | string or null | The scope's reticle: the id of a reticle in the app's catalog. |
| `fitted_barrel_id` | string or null | The barrel on the rifle now: one of the file's `barrels`; `null` when the hunter keeps no barrels for it. |
| `rounds_before_logging` | integer | Rounds fired through the action before the hunter began the round log (`round_log`); the logged rounds add to it. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). Merging keeps the copy changed last. |

### TableSettings

The rifle's range table: which ranges and which columns.

| Field | Type | Meaning |
|---|---|---|
| `start_m` | number | The first range. |
| `end_m` | number | The last range. |
| `step_m` | number | The distance between ranges. |
| `columns` | [TableColumns](#tablecolumns) | Which columns to show. |

### TableColumns

Which range table columns to show besides the range. Elevation and wind are corrections: what to dial or hold, up and right positive.

| Field | Type | Meaning |
|---|---|---|
| `drop_length` | boolean | Where a dead-on hold hits, above (+) or below the aim, as a length. |
| `drop_mil` | boolean | Elevation in mil. |
| `drop_moa` | boolean | Elevation in MOA. |
| `drop_clicks` | boolean | Elevation in the rifle's clicks. |
| `wind_length` | boolean | Wind drift as a length. |
| `wind_mil` | boolean | Wind correction in mil. |
| `wind_moa` | boolean | Wind correction in MOA. |
| `wind_clicks` | boolean | Wind correction in clicks. |
| `velocity` | boolean | The bullet's speed. |
| `energy` | boolean | The bullet's energy. |
| `time` | boolean | Time of flight. |
| `hit_chance` | boolean | The chance of hitting the target's vital zone or hit area at each range, from the shooter's precision. Shown only for a target that has one. |

### ClickUnit

The unit of a scope's clicks as the rifle stores it.

One of:

- `"mil"`
- `"moa"`

### Bullet

A bullet in the hunter's own collection: either factory ammunition or a handload. Only these can be chosen when calculating.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The bullet's id, unique among bullets. |
| `kind` | [BulletKind](#bulletkind) | Factory ammunition or a handload. |
| `manufacturer` | string | The maker, e.g. `Federal`. |
| `name` | string | The product, e.g. `Terminal Ascent`. |
| `caliber` | string | Caliber key of the cartridge this is loaded in. |
| `diameter_mm` | number | Bullet diameter. |
| `mass_g` | number | Bullet weight in grams (7000 grains a pound: 1 gr = 0.0648 g). |
| `length_mm` | number or null | Bullet length, for stability, spin drift and aerodynamic jump; `null` when not known. |
| `bc_g1` | number or null | G1 ballistic coefficient (lb/in²), if known. |
| `bc_g7` | number or null | G7 ballistic coefficient (lb/in²), if known. |
| `drag_model` | [BcModel](#bcmodel) | Which of the two BCs the calculation uses. |
| `bc_bands` | array of [BcBand](#bcband) | BCs for velocity bands under `drag_model`; when present they replace the single BC in the calculation. |
| `drag_curve` | array of [DragPoint](#dragpoint) | Drag coefficient by Mach number, used when `drag_model` is `Custom`. |
| `muzzle_velocity_mps` | number | Muzzle velocity: the maker's for factory ammunition, the hunter's for a handload. |
| `velocity_temperature_c` | number | Temperature the muzzle velocity was measured at. |
| `velocity_per_degree` | number | Change in muzzle velocity per °C of powder temperature, in m/s. |
| `test_barrel_mm` | number or null | Length of the barrel the maker's velocity was measured in, if known. |
| `construction` | [Construction](#construction) | How the bullet is made. |
| `lead_free` | boolean | Whether the bullet is lead-free. |
| `min_expansion_mps` | number or null | The least impact speed at which it expands, as the maker gives it. |
| `catalog_id` | string or null | The catalog entry this was copied from, if any. |
| `data_attribution` | [DataAttribution](#dataattribution) or null | Attribution for copied data. Preserve it when sharing or adapting the record, even if the catalog entry is no longer available. May be left out. |
| `recipe` | [Recipe](#recipe) or null | How a handload is loaded, if the hunter wrote it down. May be left out. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). Merging keeps the copy changed last. |

### BulletKind

Factory ammunition or the hunter's own load.

One of:

- `"factory"`: Factory-loaded cartridge: the velocity comes from the box.
- `"handload"`: A component bullet loaded by the hunter: the velocity is their own.

### BcModel

How the bullet's drag is described.

One of:

- `"G1"`: A G1 ballistic coefficient (`bc_g1`).
- `"G7"`: A G7 ballistic coefficient (`bc_g7`).
- `"Custom"`: The bullet's own drag curve (`drag_curve`) instead of a BC.

### BcBand

One point of a velocity-dependent BC: the BC that applies at a velocity. Between points the BC is interpolated linearly by the bullet's speed through the air; below the slowest point and above the fastest it stays at that point's BC.

| Field | Type | Meaning |
|---|---|---|
| `velocity_mps` | number | The speed this BC applies at, in m/s. A maker's band that covers a range of speeds (for example "above 2600 ft/s") is best given at the middle of that range. |
| `bc` | number | The BC (of the bullet's `drag_model`) at that speed. |

### DragPoint

One point of a bullet's own drag curve.

| Field | Type | Meaning |
|---|---|---|
| `mach` | number | Speed as a Mach number. |
| `cd` | number | Drag coefficient at that speed. |

### Construction

How the bullet is made, which decides whether it expands (and so its legal class for game): `fmj` and `match` don't, `other` is unknown, and the rest do.

One of:

- `"cup_and_core"`
- `"bonded"`
- `"partition"`
- `"monolithic"`
- `"polymer_tip"`
- `"polymer_tip_bonded"`
- `"polymer_tip_monolithic"`
- `"fmj"`
- `"match"`
- `"other"`

### DataAttribution

Attribution and license terms for data copied from another collection. These describe the copied data, not the user's independent notes or the app's code.

| Field | Type | Meaning |
|---|---|---|
| `credit` | string | The creator or collection that must receive credit. |
| `source_url` | string | HTTPS URL where the copied record can be found. Other URL schemes are rejected when storing or importing the record. |
| `source_revision` | string | Revision of the source collection used for the import. |
| `license` | string | SPDX license identifier for the copied data, e.g. `CC-BY-SA-4.0`. |
| `license_url` | string | HTTPS URL of the license text or canonical license page. Other URL schemes are rejected when storing or importing the record. |
| `changes` | string | Changes made in the initial catalog adaptation. Personal copies may contain later user edits. |

### Recipe

How a handload is put together. Every part may be left blank.

| Field | Type | Meaning |
|---|---|---|
| `powder` | string | The powder, e.g. `Vihtavuori N150`. |
| `charge_g` | number or null | The powder charge, in grams (1 grain = 0.0648 g). |
| `primer` | string | The primer, e.g. `CCI BR-2`. |
| `brass` | string | The cases, e.g. `Lapua, 3rd firing`. |
| `coal_mm` | number or null | Cartridge overall length (COAL), mm. |
| `cbto_mm` | number or null | Cartridge base to ogive (CBTO), mm: the length that sets how far the bullet jumps to the rifling. |
| `notes` | string | The hunter's notes on the recipe. |

### RifleLoad

A bullet as shot from one particular rifle: chronographed velocities, where it hits relative to the rifle's zero, and the weather it was zeroed in. Everything here overrides the bullet's general data for this rifle.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The load's id, unique among loads. |
| `rifle_id` | string | The rifle (one of the file's `rifles`). |
| `bullet_id` | string | The bullet (one of the file's `bullets`). |
| `velocities` | array of [VelocityPoint](#velocitypoint) | Measured velocities; empty means the bullet's own velocity is used. |
| `impact_offset_mrad` | array of 2 × number | Where this load hits relative to the rifle's zero, as an angle in mrad: [right, up]. |
| `zero` | [LoadZero](#loadzero) | Which weather this load's zero was made in. |
| `truing` | [Truing](#truing) or null | A long shot the calculation was trued to, if any. May be left out. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). Merging keeps the copy changed last. |

### VelocityPoint

One chronograph result: the average muzzle velocity at a temperature.

| Field | Type | Meaning |
|---|---|---|
| `temperature_c` | number | The cartridges' (powder's) temperature when they were shot. |
| `velocity_mps` | number | The average muzzle velocity. |
| `shots` | [ShotStats](#shotstats) or null | The string it averages, when imported from a chronograph. May be left out. |

### ShotStats

How a string of shots spread.

| Field | Type | Meaning |
|---|---|---|
| `count` | integer | How many shots the average is of. |
| `sd_mps` | number | Standard deviation (of a sample: n − 1), m/s. |
| `es_mps` | number | Extreme spread, fastest less slowest, m/s. |

### LoadZero

Which weather a load's zero was made in.

One of:

- `mode`: `"rifle"`: The rifle's zero weather.
- `mode`: `"same_as_shot"`: The same weather as the shot, so the weather needs no correction.
- `mode`: `"recorded"`, with the fields of [ZeroWeather](#zeroweather)

### ZeroWeather

Weather when a rifle was zeroed.

| Field | Type | Meaning |
|---|---|---|
| `temperature_c` | number | Air temperature. |
| `powder_temperature_c` | number | The cartridges' (powder's) temperature. |
| `pressure_hpa` | number | Air pressure as a barometer there reads it (not reduced to sea level). |
| `humidity_percent` | number | Relative humidity. |

### Truing

A long shot that showed where the calculation was off, and the factor found to make it match.

| Field | Type | Meaning |
|---|---|---|
| `method` | [TruingMethod](#truingmethod) | What was adjusted. |
| `range_m` | number | The range of the shot. |
| `elevation_mrad` | number | What it took to hit centre: elevation in mrad, up positive. |
| `inclination_degrees` | number | Shot angle then, degrees. |
| `weather` | [ZeroWeather](#zeroweather) | The weather then. |
| `factor` | number | The velocity or drag multiplier that makes the calculation match. |

### TruingMethod

What truing adjusts to make the calculation match a real shot.

One of:

- `"velocity"`: Scale the muzzle velocity: for loads without a chronograph.
- `"drag"`: Scale the bullet's drag (its BC the other way): for loads whose velocity was measured.

### UserScope

A scope the hunter entered: one that isn't in the catalog, or one set up differently. Part of the personal data, exported and imported with it.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The scope's id, unique among scopes; rifles name it in `scope_id`. |
| `manufacturer` | string | The maker. |
| `model` | string | The model. |
| `magnification_min` | number | Lowest magnification. |
| `magnification_max` | number | Highest magnification. |
| `objective_mm` | number or null | Objective lens diameter, if known. |
| `focal_plane` | [FocalPlane](#focalplane) | Where the reticle sits. |
| `reticle_reference_magnification` | number or null | For second-focal-plane scopes: the magnification at which the reticle's marks are true. |
| `click_unit` | [AngleUnit](#angleunit) | The unit of its clicks. |
| `click_value` | number | One click, in `click_unit`. |
| `elevation_travel` | number or null | Total elevation adjustment, in `click_unit`. |
| `fov_m_at_100m` | array of 2 × number or null | Field of view in metres at 100 m, at the lowest and the highest magnification. |
| `illuminated` | boolean | Whether the reticle can be lit. |
| `reticle_id` | string or null | A catalog reticle to draw; `null` draws a plain crosshair. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). Merging keeps the copy changed last. |

### FocalPlane

One of:

- `"ffp"`: First focal plane: the reticle grows with the image, so its marks are true at every magnification.
- `"sfp"`: Second focal plane: the reticle stays the same size, so its marks are true only at one magnification.

### AngleUnit

One of:

- `"mrad"`
- `"moa"`
- `"cm_at_100m"`: Centimetres at 100 metres, common on European hunting scopes; 1 cm at 100 m is 0.1 mrad.

### Target

What is shot at: a game animal, whose vital zone and least energy set the ethical range, or a sport target and its hit zone. Drawn at true size by its `shape`.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The target's id, unique among targets. |
| `name` | string | Its name. |
| `kind` | [TargetKind](#targetkind) | Game, with ethical limits, or a sport target. |
| `vital_diameter_cm` | number | The zone a shot must land in: the heart–lung area of an animal, or the part of a sport target that counts as a hit. |
| `min_energy_j` | number or null | Least energy the bullet must carry at the target. Only for game. |
| `min_velocity_mps` | number or null | Least impact velocity this game asks for, instead of the hunter's general one (zero: no expansion needed, as for small game). |
| `notes` | string | Notes about it. |
| `group` | [TargetGroup](#targetgroup) or null | Where it is listed; `null` for the hunter's own until they choose. |
| `other_names` | array of string | Other names to find it by, such as the Swedish and Finnish ones. |
| `ammunition` | object with any of `finland`, `sweden` | The least rifle ammunition level (class) each country's rules ask for, where it is game there; 1 is the strictest. May be left out. |
| `rules_notes` | object with any of `finland`, `sweden` | What else each country's rules say about it ("a shotgun is allowed too"). May be left out. |
| `catalog_id` | string or null | The catalog target a custom one was copied from. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC); zero for catalog targets. |

And one of these, by its tag:

- `shape`: `"animal"`, with `silhouette` (string), `shoulder_height_cm` (number), `body_length_cm` (number), `antlers` (boolean): An animal standing broadside.
- `shape`: `"rings"`, with `inner_mm` (number), `step_mm` (number), `count` (integer), `black_mm` (number): A paper target with evenly spaced scoring rings and a black aiming area.
- `shape`: `"plate"`, with `diameter_mm` (number): A round steel plate.

### TargetKind

Game, with ethical limits, or a sport target.

One of:

- `"game"`
- `"sport"`

### TargetGroup

How targets are grouped in lists, the way hunters think of the game.

One of:

- `"deer"`
- `"boar_and_mouflon"`
- `"bear"`
- `"small_predators"`
- `"hares_and_rodents"`
- `"seals"`
- `"forest_birds"`
- `"waterfowl"`
- `"other_birds"`
- `"sport"`
- `"other"`

### Shot

A shot taken, with the solution the app gave and what came of it.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The shot's id, unique among shots. |
| `taken_at` | integer | When it was logged, in milliseconds since 1970 (UTC). |
| `rifle_id` | string | The rifle shot with; it may have been deleted since. |
| `rifle_name` | string | The rifle's name then, so the log reads after it is renamed or deleted. |
| `bullet_id` | string | The bullet shot. |
| `bullet_name` | string | The bullet's name then. |
| `target_id` | string or null | What was shot at: one of the hunter's targets or the catalog's, or `null` for no target. |
| `target_name` | string | The target's name then. |
| `range_m` | number | The range along the line of sight. |
| `inclination_degrees` | number | The shot angle: up (positive) or down the line of sight. |
| `wind_speed_mps` | number | The wind's speed. |
| `wind_clock` | number | Where the wind came from, on a clock: 12 head-on, 3 from the right. |
| `weather` | [ShotWeather](#shotweather) | The weather the solution was worked out for. |
| `hold` | [ShotHold](#shothold) | The hold the app gave. |
| `solution` | [ShotSolution](#shotsolution) | The bullet's path at the range. |
| `outcome` | [Outcome](#outcome) | Hit, miss or not written down. |
| `hit_where` | string | Where a hit landed, in the hunter's words ("heart–lung"). |
| `position` | [ShooterPosition](#shooterposition) or null | Where the hunter stood, if they marked it. |
| `bearing_degrees` | number or null | The direction from the hunter to the animal, degrees from true north, if taken. |
| `animal_spot` | [AnimalSpot](#animalspot) or null | Where the animal stood, from `position`, `bearing_degrees`, the range and the shot angle; `null` without a position and a bearing. |
| `notes` | string | Notes about it. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### ShotWeather

The weather the solution was worked out for.

| Field | Type | Meaning |
|---|---|---|
| `temperature_c` | number | Air temperature. |
| `powder_temperature_c` | number | The cartridges' (powder's) temperature. |
| `pressure_hpa` | number | Air pressure where the hunter stood, as a barometer there reads it (not reduced to sea level). |
| `humidity_percent` | number | Relative humidity. |

### ShotHold

The hold the app gave: the angle to come up and right by, and the same in the rifle's clicks.

| Field | Type | Meaning |
|---|---|---|
| `elevation_mrad` | number | Up (positive) or down. |
| `windage_mrad` | number | Right (positive) or left. |
| `click_unit` | [ClickUnit](#clickunit) | The unit of the rifle's clicks then. |
| `click_value` | number | One click, in `click_unit`. |
| `elevation_clicks` | integer | Clicks up (positive) or down, rounded to whole clicks. |
| `windage_clicks` | integer | Clicks right (positive) or left. |

### ShotSolution

Where the bullet was worked out to be at the range, aimed dead on.

| Field | Type | Meaning |
|---|---|---|
| `drop_cm` | number | Above (positive) or below the line of sight. |
| `windage_cm` | number | Right (positive) or left of the line of sight: wind and spin drift. |
| `velocity_mps` | number | Speed at the range. |
| `energy_j` | number | Energy at the range. |
| `time_of_flight_s` | number | Seconds from the muzzle to the range. |

### Outcome

What came of a shot.

One of:

- `"not_recorded"`: Not written down (yet).
- `"hit"`: The shot hit.
- `"miss"`: The shot missed.

### ShooterPosition

Where the hunter stood when shooting.

| Field | Type | Meaning |
|---|---|---|
| `latitude_degrees` | number | North positive, south negative. |
| `longitude_degrees` | number | East positive, west negative. |
| `accuracy_m` | number or null | How far off the position may be, as the device said; `null` when it was typed in. |

### AnimalSpot

Where the animal stood, worked out from the hunter's position, the bearing and the range.

| Field | Type | Meaning |
|---|---|---|
| `latitude_degrees` | number | North positive, south negative. |
| `longitude_degrees` | number | East positive, west negative. |
| `search_radius_m` | number | How far around the spot to search: the bearing's, the range's and the position's possible errors together. |

### Shotgun

A shotgun: its caliber and barrels with their chokes.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The gun's id, unique among shotguns. |
| `name` | string | The hunter's name for it. |
| `caliber` | string | Its caliber's key in the app's catalog: "12-76" for a 12/76. |
| `barrels` | array of [Barrel](#barrel) | Its barrels, each with a choke: one for a single-barrel gun, two for an over-and-under or side-by-side. |
| `sight_height_cm` | number or null | For slugs: the height of the sights (or scope) above the bore, cm; `null` for a bead. |
| `slug_zero_m` | number or null | For slugs: the range the sights are set for, m. |
| `notes` | string | Notes about it. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Barrel

One of the gun's barrels.

| Field | Type | Meaning |
|---|---|---|
| `name` | string | What the hunter calls it: "Under", "Over", "Right"; empty for a gun with one barrel. |
| `choke` | [Choke](#choke) | Its choke, fixed or the tube in it. |

### Choke

A barrel's choke: by its name, or by its constriction as measured or stamped.

One of:

- `by`: `"name"`, with `name` ([ChokeName](#chokename)): By its name.
- `by`: `"constriction"`, with `mm` (number): By its constriction: how much narrower the choke is than the bore.

### ChokeName

A choke by its name, from open to tight.

One of:

- `"cylinder"`: No constriction (★★★★★).
- `"skeet"`: About 0.13 mm (⅛).
- `"improved_cylinder"`: American improved cylinder, about 0.25 mm.
- `"light_modified"`: Quarter choke (¼, ★★★★), often called improved cylinder in Europe.
- `"modified"`: Half choke (½, ★★★).
- `"improved_modified"`: Three-quarter choke (¾, ★★).
- `"light_full"`: Light full (⅞).
- `"full"`: Full choke (1/1, ★).
- `"extra_full"`: Tighter than full, as turkey chokes are.

### Shell

A shotgun shell in the hunter's collection: shot or a slug, for one shotgun caliber, as rifle bullets are for one rifle caliber.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The shell's id, unique among shells. |
| `caliber` | string | Its caliber's key in the app's catalog: "12-70" for a 12/70 shell (see the export's `catalog`, or docs/shotgun-research/calibers.md). It fits a gun of its gauge with a chamber as long or longer. |
| `catalog_id` | string or null | The catalog shell it was copied from, if any. |
| `manufacturer` | string | Its maker: "Gyttorp". |
| `name` | string | Its name, as the box gives it: "Röda Papp 32 g US 5". |
| `payload` | [Payload](#payload) | Shot or a slug. |
| `velocity_mps` | number | The velocity the box gives (or the hunter measured), m/s. |
| `velocity_at` | [VelocityAt](#velocityat) | Where that velocity is measured. |
| `notes` | string | Notes about it. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Payload

What a shell is loaded with.

One of:

- `kind`: `"shot"`, with `material` ([ShotMaterial](#shotmaterial)), `pellet_mm` (number), `load_g` (number): Shot: many round pellets.
- `kind`: `"slug"`, with `mass_g` (number), `diameter_mm` (number), `bc_g1` (number), `lead_free` (boolean): A slug: one projectile.

### ShotMaterial

What shot is made of.

One of:

- `"lead"`: Lead (hardened with antimony).
- `"steel"`: Soft iron, called steel.
- `"bismuth"`: Bismuth with a little tin.
- `"tungsten_iron"`: Tungsten, nickel and iron, such as HEVI-Shot.
- `"tungsten_matrix"`: Tungsten powder in a polymer, such as Tungsten-Matrix.
- `"tss"`: Tungsten super shot (TSS).
- `"zinc"`: Zinc.

### VelocityAt

Where the velocity on the box was measured.

One of:

- `"muzzle"`: At the muzzle ("V0").
- `"v1"`: 1 m from the muzzle ("V1", as Gamebore prints it).
- `"v2_5"`: 2.5 m from the muzzle ("V2.5", as C.I.P. makers print it).
- `"three_feet"`: 3 feet from the muzzle, as American makers print it.
- `"observed"`: The British "observed velocity": the average over the first 20 yards.

### ShotgunLoad

What is particular to a shell in one shotgun: the pattern boards the hunter shot with it, as a rifle load holds the velocities measured in one rifle.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The record's id, unique among shotgun loads. |
| `shotgun_id` | string | The shotgun (`shotguns`). |
| `shell_id` | string | The shell (`shells`). |
| `patterns` | array of [PatternTest](#patterntest) | Pattern boards counted with this shell in this gun, which replace its choke's nominal pattern for that barrel. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### PatternTest

A pattern the hunter shot and counted: how many pellets landed inside a circle at a distance. The Nordic test is a 75 cm circle at 35 m.

| Field | Type | Meaning |
|---|---|---|
| `barrel` | integer | Which of the gun's barrels shot it, counted from 0 in `barrels`. |
| `distance_m` | number | From the muzzle to the board, m. |
| `circle_cm` | number | The circle's diameter, cm, drawn round the pattern's densest part. |
| `pellets_inside` | integer | Pellets counted inside the circle; 0 until counted, and a board with none is left out. |
| `pellets_in_load` | integer or null | Pellets in the whole load, if counted (or taken from the box); `null` to work them out from the load and pellet size. |

### RifleBarrel

A barrel that is or was on one of the hunter's rifles.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The barrel's id, unique among barrels. |
| `rifle_id` | string | The rifle (one of the file's `rifles`) the barrel belongs to. |
| `name` | string | The hunter's name for it, e.g. `Factory` or `Bartlein 26"`. |
| `length_mm` | number or null | Its length; `null` when not known. |
| `twist_in` | number or null | Barrel length for one turn of the rifling, in inches; `null` when not known. |
| `right_hand_twist` | boolean | Whether the rifling turns to the right, as most do. |
| `maker` | string | Who made it. |
| `fitted_at` | integer or null | When it was fitted to the rifle, in milliseconds since 1970 (UTC), if known. |
| `retired_at` | integer or null | When it came off for good; `null` while it is still in use. |
| `rounds_before_logging` | integer | Rounds fired through it before the hunter began the round log; the logged rounds add to it. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### RoundLogEntry

Rounds fired through a rifle at one time, for the round counts.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The entry's id, unique among round log entries. |
| `rifle_id` | string | The rifle fired (one of the file's `rifles`). |
| `barrel_id` | string or null | The barrel they went through (one of the file's `barrels`); `null` when the hunter keeps no barrels for the rifle or didn't say: then they count for the action only. |
| `rounds` | integer | How many rounds, 1 to 10 000. |
| `at` | integer | When they were fired, in milliseconds since 1970 (UTC). |
| `note` | string | The hunter's note. |
| `group_id` | string or null | The group (one of the file's `groups`) these rounds were, when the entry was made by saving one. Such an entry's id is the UUID (version 5) of the group's id in the namespace `6d787363-726f-756e-6473-8f2a41c6d903`, so every device makes the same one. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Clean

A cleaning of a rifle's barrel.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The clean's id, unique among cleans. |
| `rifle_id` | string | The rifle cleaned (one of the file's `rifles`). |
| `barrel_id` | string or null | The barrel cleaned (one of the file's `barrels`); `null` for a rifle whose barrels the hunter doesn't keep apart. |
| `at` | integer | When, in milliseconds since 1970 (UTC). |
| `note` | string | The hunter's note, e.g. what was used. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### ZeroReset

The moment a rifle's zero was set again (a new scope, a new mount, a new zero): zero checks pool only the groups shot since.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The reset's id, unique among zero resets. |
| `rifle_id` | string | The rifle (one of the file's `rifles`). |
| `barrel_id` | string or null | The barrel (one of the file's `barrels`) the zero is for; `null` for the rifle whatever its barrel. |
| `at` | integer | When, in milliseconds since 1970 (UTC). |
| `note` | string | The hunter's note, e.g. why. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Session

A day or an outing at the range. Groups name the session they were shot in and take its conditions unless they have their own.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The session's id, unique among sessions. |
| `place` | string | Where, in the hunter's words (a range's name, not a position). |
| `started_at` | integer | When it began, in milliseconds since 1970 (UTC). |
| `temperature_c` | number or null | Air temperature. May be left out. |
| `powder_temperature_c` | number or null | The cartridges' (powder's) temperature, when it differs from the air's. May be left out. |
| `pressure_hpa` | number or null | Air pressure as a barometer at the range reads it (not reduced to sea level). May be left out. |
| `humidity_percent` | number or null | Relative humidity. May be left out. |
| `wind_speed_mps` | number or null | The wind's speed. May be left out. |
| `wind_clock` | number or null | Where the wind came from, on a clock: 12 head-on, 3 from the right. May be left out. |
| `notes` | string | The hunter's notes. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Photo

A photo of a target that groups are marked on. Only its description is in the file; the photo itself is kept on the device beside the data.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The lowercase hexadecimal SHA-256 of the kept photo's bytes, which also names it where it is kept. |
| `width_px` | integer | Its width as kept: upright, after any reduction. |
| `height_px` | integer | Its height as kept. |
| `mime` | string | The type of the kept bytes, `image/jpeg`. |
| `taken_at` | integer or null | When the camera says it was taken, in milliseconds since 1970 (UTC); `null` when it doesn't say. |
| `calibration` | [Calibration](#calibration) or null | How its pixels map to millimetres on the target, for every group marked on it; `null` until the scale is set. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Calibration

How a photo's pixels map to millimetres on the target. Points are in the photo's pixels: x to the right and y down from the photo's top-left corner, so the middle of the top-left pixel is `[0.5, 0.5]`.

One of:

- `kind`: `"two_point"`, with `a_px` (array of 2 × number), `b_px` (array of 2 × number), `distance_mm` (number): Two points a known distance apart on the target, such as lines of its grid: gives the scale, the photo taken to face the target squarely.
- `kind`: `"four_corner"`, with `corners_px` (array of 4 × array of 2 × number), `width_mm` (number), `height_mm` (number), `ordered` (boolean): The corners of a rectangle of known size on the target, such as the sheet's edges: gives the scale and straightens a photo taken at an angle.

### Group

A group of shots: where they went, and what, where and how they were shot.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The group's id, unique among groups. |
| `taken_at` | integer | When it was shot, in milliseconds since 1970 (UTC). |
| `marks` | [Marks](#marks) | Where its shots are marked. |
| `rifle_id` | string or null | The rifle shot (one of the file's `rifles`), if given. |
| `barrel_id` | string or null | The rifle's barrel (one of the file's `barrels`), if given. |
| `bullet_id` | string or null | The bullet or load shot (one of the file's `bullets`), if given. |
| `ladder_id` | string or null | The ladder (one of the file's `ladders`) this group is a rung of. |
| `rung_value` | number or null | The rung's value, in the unit the ladder's `variable` names: the charge in grams, or the seating's CBTO in mm. |
| `charge_g` | number or null | The handload's powder charge when the group was saved, in grams, as its bullet's `recipe` gave it; `null` when it gave none (a factory load, or a group saved before this was kept). |
| `cbto_mm` | number or null | The handload's cartridge base to ogive (CBTO) when the group was saved, mm, as its bullet's `recipe` gave it; `null` when it gave none. |
| `distance_m` | number or null | The distance to the target; `null` when not given, which leaves out sizes as angles. |
| `session_id` | string or null | The session (one of the file's `sessions`) it was shot in. |
| `conditions` | [Conditions](#conditions) | Conditions that differ from the session's; what is `null` here is the session's. |
| `extra_velocities_mps` | array of number | Muzzle velocities measured for rounds that match no marked shot (foulers, shots off the paper), in the order they came. |
| `dialled_offset_mrad` | array of 2 × number | What was dialled on the scope, right and up positive, compared with the zero. Zero checks take it off the group's impact. |
| `include_in_zero` | boolean | Whether zero checks count this group. |
| `count_rounds` | boolean | Whether saving the group logs its shots as rounds fired (an entry of `round_log` naming the group). |
| `tags` | array of string | The hunter's tags, e.g. `practice`. |
| `notes` | string | The hunter's notes. |
| `archived` | boolean | Kept, but out of the library's lists. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### Marks

Where a group's shots are marked.

One of:

- `on`: `"photo"`, with the fields of [PhotoMarks](#photomarks): On a photo of the target.
- `on`: `"grid"`, with the fields of [GridMarks](#gridmarks): On a blank grid.

### PhotoShot

A shot marked on a photo.

| Field | Type | Meaning |
|---|---|---|
| `x_px` | number | The hole's centre: pixels from the photo's left edge. |
| `y_px` | number | The hole's centre: pixels down from the photo's top edge. |
| `cold_bore` | boolean | The first shot from a cold, clean barrel. |
| `called_flier` | boolean | The shooter knew the shot was bad as it broke; statistics can leave it out. |
| `velocity_mps` | number or null | Its muzzle velocity, if measured. |

### PhotoMarks

Shots marked on a photo of the target, in its pixels.

| Field | Type | Meaning |
|---|---|---|
| `photo_id` | string | The photo (one of the file's `photos`), whose `calibration` gives the scale. |
| `aim_px` | array of 2 × number or null | The point aimed at, `[x, y]` in pixels; `null` without one, which leaves out the group's correction and zero checks. |
| `shots` | array of [PhotoShot](#photoshot) | The shots, in the order they were marked. |

### GridShot

A shot placed on a blank grid.

| Field | Type | Meaning |
|---|---|---|
| `x_mm` | number | Millimetres right (positive) or left of the grid's centre. |
| `y_mm` | number | Millimetres up (positive) or down from the grid's centre. |
| `cold_bore` | boolean | The first shot from a cold, clean barrel. |
| `called_flier` | boolean | The shooter knew the shot was bad as it broke; statistics can leave it out. |
| `velocity_mps` | number or null | Its muzzle velocity, if measured. |

### GridMarks

Shots placed on a blank grid at true scale, in millimetres from its centre: x to the right, y up. For targets that weren't photographed: scored paper, or positions from an electronic target.

| Field | Type | Meaning |
|---|---|---|
| `width_mm` | number | The grid's width. |
| `height_mm` | number | The grid's height. |
| `aim_mm` | array of 2 × number or null | The point aimed at, `[x, y]` in mm; `null` without one. Usually the centre, `[0, 0]`. |
| `shots` | array of [GridShot](#gridshot) | The shots, in the order they were placed. |

### Conditions

The weather at the range. Every part may be unknown, and is then left out.

| Field | Type | Meaning |
|---|---|---|
| `temperature_c` | number or null | Air temperature. May be left out. |
| `powder_temperature_c` | number or null | The cartridges' (powder's) temperature, when it differs from the air's. May be left out. |
| `pressure_hpa` | number or null | Air pressure as a barometer at the range reads it (not reduced to sea level). May be left out. |
| `humidity_percent` | number or null | Relative humidity. May be left out. |
| `wind_speed_mps` | number or null | The wind's speed. May be left out. |
| `wind_clock` | number or null | Where the wind came from, on a clock: 12 head-on, 3 from the right. May be left out. |

### Ladder

A load development: groups of one bullet in one rifle, each a rung at a different charge or seating depth (the groups' `ladder_id` and `rung_value`). Zero checks and trends leave its groups out unless the hunter adds them.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The ladder's id, unique among ladders. |
| `name` | string | The hunter's name for it; may be empty. |
| `rifle_id` | string | The rifle (one of the file's `rifles`). |
| `bullet_id` | string | The bullet loaded (one of the file's `bullets`), whose recipe the rungs vary. |
| `variable` | [LadderVariable](#laddervariable) | What changes from rung to rung. |
| `notes` | string | The hunter's notes. |
| `promoted_bullet_id` | string or null | The load (one of the file's `bullets`) made from the rung the hunter kept, once there is one. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### LadderVariable

What a ladder varies from rung to rung.

One of:

- `"charge"`: The powder charge; rung values are grams.
- `"seating"`: The seating depth; rung values are the cartridge's base-to-ogive length (CBTO), mm.

### Analysis

An analysis of several groups the hunter saved, worked out again from the groups whenever it is opened.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The analysis's id, unique among analyses. |
| `kind` | [AnalysisKind](#analysiskind) | What it does. |
| `name` | string | The hunter's name for it. |
| `group_ids` | array of string | Its groups (of the file's `groups`), in the hunter's order; ones deleted since are skipped. |
| `settings` |  | How the app shows it, as the app's own JSON object; a reader may ignore it. |
| `updated_at` | integer | When the record was last changed, in milliseconds since 1970 (UTC). |

### AnalysisKind

What a saved analysis does with its groups.

One of:

- `"compare"`: Groups side by side, tested against each other.
- `"composite"`: The groups' shots pooled into one group.
- `"ladder"`: A ladder's rungs, ranked.

### Deletion

A record that was deleted.

| Field | Type | Meaning |
|---|---|---|
| `kind` | [RecordKind](#recordkind) | What kind of record it was. |
| `id` | string | The deleted record's id. |
| `deleted_at` | integer | When it was deleted, in milliseconds since 1970 (UTC). |

### RecordKind

The kinds of the hunter's records.

One of:

- `"rifle"`
- `"bullet"`
- `"rifle_load"`
- `"scope"`
- `"target"`
- `"shot"`
- `"shotgun"`
- `"shell"`
- `"shotgun_load"`
- `"barrel"`
- `"round_log_entry"`
- `"clean"`
- `"zero_reset"`
- `"session"`
- `"photo"`
- `"group"`
- `"ladder"`
- `"analysis"`

### CatalogDetails

What the records name from the app's built-in catalog.

| Field | Type | Meaning |
|---|---|---|
| `calibers` | array of [CaliberDetails](#caliberdetails) | The calibers named by rifles and bullets (their `caliber`). |
| `shotgun_calibers` | array of [ShotgunCaliberDetails](#shotguncaliberdetails) | The shotgun calibers named by shotguns and shells (their `caliber`). |
| `scopes` | array of [ScopeDetails](#scopedetails) | The catalog scopes named by rifles (their `scope_id`). |
| `reticles` | array of [ReticleDetails](#reticledetails) | The catalog reticles named by rifles and scopes (their `reticle_id`). |

### CaliberDetails

A caliber, as a rifle's or bullet's `caliber` names it.

| Field | Type | Meaning |
|---|---|---|
| `key` | string | The key records use, e.g. `308-win`. |
| `name` | string | Its name, e.g. `.308 Win`. |
| `bullet_diameter_mm` | number | The bullet diameter the caliber takes, mm. |

### ShotgunCaliberDetails

A shotgun caliber, as a shotgun's or shell's `caliber` names it.

| Field | Type | Meaning |
|---|---|---|
| `key` | string | The key records use, e.g. `12-76`. |
| `name` | string | Its name, e.g. `12/76`. |
| `gauge` | [Gauge](#gauge) | Its gauge: `"12"`, `".410"`. |
| `length_mm` | number | The length in its name, mm: a shell fits a gun of its gauge whose caliber's length is the same or longer. |
| `bore_mm` | number | The bore's nominal (least) diameter, mm. |

### Gauge

A shotgun's bore size.

One of:

- `"10"`
- `"12"`
- `"16"`
- `"20"`
- `"24"`
- `"28"`
- `"32"`
- `".410"`: .410 bore, also called 36 gauge.

### ScopeDetails

A catalog scope, as a rifle's `scope_id` names it.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The id a rifle's `scope_id` uses. |
| `manufacturer` | string | The maker. |
| `model` | string | The model. |
| `magnification_min` | number | Lowest magnification. |
| `magnification_max` | number | Highest magnification. |
| `objective_mm` | number | Objective lens diameter, mm. |
| `tube_mm` | number or null | Tube diameter, mm. |
| `focal_plane` | [FocalPlane](#focalplane) | Where the reticle sits. |
| `reticle_reference_magnification` | number or null | For second-focal-plane scopes: the magnification at which the reticle's published dimensions are true. |
| `click_unit` | [AngleUnit](#angleunit) | The unit of its clicks. |
| `click_value` | number or null | Per click, in `click_unit`; `null` when the maker doesn't publish it. |
| `elevation_travel` | number or null | Total elevation adjustment, in `click_unit`. |
| `fov_m_at_100m` | array of 2 × number or null | Field of view in metres at 100 m, at the lowest and the highest magnification. |
| `illuminated` | boolean | Whether the reticle can be lit. |
| `source_url` | string | Where the data was taken from. |

### ReticleDetails

A catalog reticle, as a `reticle_id` names it.

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The id the record uses. |
| `manufacturer` | string | The maker. |
| `name` | string | The app's name for it, with scope and unit. |
| `maker_reticle_name` | string | The maker's own name for the reticle. |
| `unit` | [AngleUnit](#angleunit) | The unit its marks are drawn in. |
| `source_url` | string | Where its dimensions were taken from. |

<!-- end of the field reference -->

# The export file

MX Shooting Calculator saves the hunter's own data as one JSON file:
**Export backup** in the app's menu writes it, **Import backup…** reads it,
and the AI tools (`mxsc-mcp`) and sync use the same format. This page
describes the file so other programs can read and write it.

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
  "version": 7,
  "exported_at": 1792473562000,
  "rifles": [ { "id": "…", "name": "Browning X-Bolt", "caliber": "308-win", "twist_in": 10.0, … } ],
  "bullets": [ { "id": "…", "manufacturer": "Federal", "name": "Terminal Ascent", "mass_g": 11.34, … } ],
  "rifle_loads": [ { "id": "…", "rifle_id": "…", "bullet_id": "…", "velocities": [ … ], … } ],
  "scopes": [],
  "targets": [],
  "shots": [ { "id": "…", "taken_at": 1792473562000, "rifle_id": "…", "range_m": 150.0, "outcome": "hit", … } ],
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
  - times: `_ms` (milliseconds since 1 January 1970, UTC), and `_s` for
    seconds of flight.

  Twist is the one exception: `twist_in` is the inches of barrel per turn
  of the rifling, as the trade gives it.
- **Signs:** heights are positive above the line of sight; angles and
  offsets are positive up and to the right; a shot angle is positive uphill.
- **Ids** are strings, unique within each kind of record. Records refer to
  each other by id: a rifle load names its `rifle_id` and `bullet_id`; a
  rifle's `scope_id` names one of the file's `scopes`, or else a scope in the
  app's catalog.
- **References to the app's catalog:** a `caliber` is a key such as
  `308-win`, a `scope_id` may name a catalog scope, a `reticle_id` a catalog
  reticle, and an animal target's `silhouette` one of the app's drawings. The
  `catalog` section of an exported file writes out the calibers, scopes and
  reticles the records name, so a reader doesn't need the app. Sync copies
  leave it out.
- **`updated_at`** is when a record was last changed. Importing merges
  record by record: a new record is added, an identical one is left alone,
  and otherwise the copy changed last wins.
- **`deleted`** lists records deleted since, so a merge deletes them on the
  other side too.

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

## Writing a file for the app

The app reads files written by other programs:
- `format` must be `mxsc/personal-data`, and `version` no higher than the app knows (7).
- Fields marked "may be left out" can be omitted.
- Unknown fields are ignored, and the `catalog` section is never imported.
- Every record is checked before anything is saved. Numbers must be finite and within what rifles and bullets can be. If any record fails, nothing from the file is imported, and the app says which record failed.

## Versions

| Version | What changed |
|---|---|
| 1 | Rifles, bullets and loads with one measured velocity. |
| 2 | Loads hold a list of velocities, an impact offset and their zero weather. |
| 3 | The hunter's own scopes; bullets with their own drag curve. |
| 4 | The hunter's own targets. |
| 5 | Deleted records. Later, still version 5: the `catalog` section (ignored by imports), truing, shot statistics on velocities, a zero height. |
| 6 | The shot log (`shots`). |

## Field reference

<!-- field reference: generated from the code, see schema.rs -->

### The file

A file of the hunter's own records: an export, a backup or a sync copy.

| Field | Type | Meaning |
|---|---|---|
| `format` | string | Always `mxsc/personal-data`. Files from the app's earlier names say `mhc/personal-data` or `ballistic-calculator/personal-data`. |
| `version` | integer | The format's version; this description is of version 7. A reader should refuse versions newer than it knows. |
| `exported_at` | integer | When the file was written, in milliseconds since 1970 (UTC). |
| `rifles` | array of [Rifle](#rifle) | The hunter's rifles. |
| `bullets` | array of [Bullet](#bullet) | The hunter's bullets: factory ammunition and handloads. |
| `rifle_loads` | array of [RifleLoad](#rifleload) | What is particular to a bullet in a rifle: measured velocities, point of impact, zero weather and truing. |
| `scopes` | array of [UserScope](#userscope) | Scopes the hunter entered (catalog scopes are only named by id). |
| `targets` | array of [Target](#target) | Targets the hunter made or copied (catalog targets aren't included). |
| `shots` | array of [Shot](#shot) | The shot log: shots the hunter took, with the solution the app gave and, after a shot at game, where the hunter and the animal stood. |
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

One BC band: the BC that applies at a velocity.

| Field | Type | Meaning |
|---|---|---|
| `velocity_mps` | number | From this speed down to the next band's. |
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

### CatalogDetails

What the records name from the app's built-in catalog.

| Field | Type | Meaning |
|---|---|---|
| `calibers` | array of [CaliberDetails](#caliberdetails) | The calibers named by rifles and bullets (their `caliber`). |
| `scopes` | array of [ScopeDetails](#scopedetails) | The catalog scopes named by rifles (their `scope_id`). |
| `reticles` | array of [ReticleDetails](#reticledetails) | The catalog reticles named by rifles and scopes (their `reticle_id`). |

### CaliberDetails

A caliber, as a rifle's or bullet's `caliber` names it.

| Field | Type | Meaning |
|---|---|---|
| `key` | string | The key records use, e.g. `308-win`. |
| `name` | string | Its name, e.g. `.308 Win`. |
| `bullet_diameter_mm` | number | The bullet diameter the caliber takes, mm. |

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

# Strelok Pro backups (`.srl`)

MXSC imports Strelok Pro backups through **Import backup…** in the menu. The code is in
`crates/ballistics-store/src/strelok.rs` and its tests use the made-up backup
`crates/ballistics-store/testdata/strelok-sample.srl`. To see what a real
file would become, without touching any database:

```sh
cargo run -p ballistics-store --example strelok -- backup.srl [database.sqlite [--apply]]
```

## How importing works

1. **Reading.** The file is XML (`<StrelokPro>`), but it is **not
   well-formed**: names such as "Sellier & Bellot" contain a bare `&`. Every
   `&` that does not start an entity is escaped before parsing. Only backups
   made with metric units are accepted; imperial ones are refused rather than
   guessed at.
2. **Planning.** The backup becomes rifles, bullets, rifle loads and one
   custom scope per rifle, plus warnings. Each rifle's caliber is shown with
   how it was found:
   - **from the names:** the rifle's name counts most, cartridge names less,
     and longer matches more. Parenthesised item numbers are ignored, since
     their digits could look like a caliber;
   - **from the bullet diameter**, when only one caliber fits;
   - **kept from your rifle**, when it was imported before;
   - **chosen**, when you pick it in the import window;
   - **a guess**, which is flagged so you check it.
3. **Previewing.** The plan is merged inside a transaction that is rolled
   back, so the window shows exact counts of new, updated, unchanged and kept
   records. Changing a caliber re-plans the import.
4. **Importing** uses the same merge as JSON backups.

## Importing more than once

Every imported record gets an id derived from what it is (UUID v5), so the
same backup always gives the same ids:

| Record | Identified by |
|---|---|
| Rifle | its Strelok name |
| Bullet | the caliber inferred from the file, maker, product, item number and weight. The same cartridge in two Strelok rifles of a caliber becomes one bullet. |
| Rifle load | rifle and bullet |
| Scope | the rifle |

All records get the backup file's modification time as their `updated_at`.
The merge then decides record by record:

- **identical** to what is already here: left alone ("already here");
- **edited here** after the backup was made: kept as edited;
- the backup is **newer** and the record wasn't touched: updated.

Bullet ids use the caliber inferred from the file alone, never the caliber
you chose, so correcting a rifle's caliber does not create new bullets on
the next import.

**Renaming a rifle in Strelok** gives it a new identity, so it is imported as
another rifle. The same goes for renaming a cartridge.

## Structure

```
StrelokPro
├─ Metric_units_on, CurrentRifle (index)
├─ Rifle*            rifle and its scope, inline
│  └─ Cartridge*     loads belong to one rifle
│     └─ TermoSensitivity/TermoRow*   (TS_Temperature, TS_Speed)
└─ Units             display units per quantity
```

## Mapping (all implemented)

| Strelok | MXSC | Notes |
|---|---|---|
| `RifleName` | `Rifle.name` | Caliber is not a field; it is inferred (see above). |
| `ZeroDistance` | `Rifle.zero_range_m` | Metres when `Metric_units_on` is true. |
| `ScopeHeight` | `Rifle.sight_height_cm` | cm in metric mode. |
| `ScopeClickVert/Hor` | `Rifle.click_value` | Always stored in MOA. `click_units` 1 shows them as mrad or cm/100 m, so 0.34377 MOA = 0.1 mil and 0.85944 MOA = 0.25 mil. 0 means MOA. |
| `Reticle` (number) | the custom scope's notes | Strelok's own reticle library can't be read. The scope gets a plain crosshair; pick a catalog reticle under My scopes. |
| `min/max/true_magnification`, `first_focal` | a custom scope (`UserScope`) | Magnification range, focal plane, reference magnification (`true_magnification`) and clicks. Mount it on the rifle with a plain crosshair, or let the user pick a catalog reticle. |
| `TwistRate`, `TwistLeft` | `twist_in`, `right_hand_twist` | Inches. |
| `m_StartDistance/EndDistance/StepDistance` | `Rifle.table` start, end, step | |
| `m_show_path_cm/moa/td/click` | `TableColumns.drop_length/drop_moa/drop_mil/drop_clicks` | `td` (thousandths) is mil. |
| `m_show_wind_*` | `TableColumns.wind_*` | |
| `m_show_speed/energy/time` | `velocity/energy/time` | |
| `CartridgeName` | `Bullet.name`, `manufacturer` | Usually "150gr. VOR-TX (21540), 308 Win, (Barnes)": the leading weight is dropped, the maker comes from the last part (or a known maker at the start of hand-typed names), and the item number stays in the name. |
| `BulletWeight_gr` | `Bullet.mass_g` | Grains. |
| `BulletSpeed`, `BulletTemperature` | `muzzle_velocity_mps`, `velocity_temperature_c` | |
| `BulletBC`, `BulletBC2…5` + `…Speed` | `bc_g1/bc_g7`, `bc_bands` | A speed of 0 means a single BC. |
| `DragFunctionName` | `drag_model` | G1 and G7. Other standard functions (G2, G5, …) are not supported. |
| `DragFunctionCategory` ≠ 0 | (skipped, with a note) | Custom drag curve. MXSC supports these, but how Strelok stores the points is not known yet: no real backup with one has been seen. |
| `TempModifyer` | `velocity_per_degree` | Percent per 15 °C: v × % / 100 / 15. |
| `TermoSensitivity/TermoRow` | `RifleLoad.velocities` | Rows with speed 0 are skipped. Without rows, the cartridge's own velocity and temperature are used. |
| `bullet_length_inch`, `bullet_diam_inch` | `length_mm`, `diameter_mm` | Strelok's default length 1.200 in is often a placeholder. |
| `ShiftVerticalMOA/HorizontalMOA` | `RifleLoad.impact_offset_mrad` | MOA to mrad, positive taken as up and right. **The sign is not yet confirmed against Strelok.** |
| `ZeroTemperature`, `ZeroPowderTemperature`, `ZeroPressure`, `ZeroHumidity`, `same_atm` | `RifleLoad.zero`; the rifle's zero weather from the selected cartridge | Pressure is in mmHg. `same_atm=true` means `SameAsShot`; otherwise `Recorded`. |
| `CartridgeNotes`, `RifleNotes` | `notes` | The text "null" means empty. |
| `CurrentRifle`, `CurrentCartridge` | calculator selection | |
| `Units` | not imported | Display preferences; set them under Units. |

## Handled on purpose

- **Strelok's placeholder** cartridge "Sample. Replace with your data" is
  skipped.
- **A cartridge that appears twice in one rifle** is imported once, with a
  note.
- **Records that fail validation** (no BC, no velocity, a bad table range)
  are skipped or replaced by defaults, with a note. They never stop the rest
  of the import.

## Open points

- **Custom drag curves** (`DragFunctionCategory` ≠ 0): need a sample backup
  that has one.
- **The sign of `ShiftVertical/HorizontalMOA`:** set a known shift in Strelok
  and compare.
- **Imperial backups** (`Metric_units_on=false`): need a sample to see
  whether Strelok stores values in imperial units.

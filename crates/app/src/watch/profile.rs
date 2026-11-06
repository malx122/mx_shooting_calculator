//! A watch profile (docs/watch.md, "The watch profile"): one rifle and load
//! in any weather the hunter is likely to meet, row by row out to a range,
//! for the watch to read the holds from.
//!
//! The phone solves the flight on a grid of air temperatures and pressures;
//! the watch fits a curve through the grid for its own weather, and through
//! the rows for the range. Against solving each shot that loses about
//! 0.01 mil out to 600 m (0.2 % in strong winds; the tests below check it).

use ballistics_shot::Pressure;
use ballistics_store::ClickUnit;
use sha2::{Digest, Sha256};

use crate::cards::Subject;
use crate::i18n::tr;
use crate::units::{Quantity, UnitPrefs};

pub const FORMAT: u8 = 2;
/// Metres between rows.
pub const STEP_M: u16 = 25;
/// The weather grid: air (and cartridge) temperatures, °C…
pub const TEMPERATURES_C: [i8; 4] = [-30, -8, 13, 35];
/// …and air pressures where the hunter stands, hPa (sea level to about
/// 3000 m).
pub const PRESSURES_HPA: [u16; 3] = [700, 875, 1050];
const CELLS: usize = TEMPERATURES_C.len() * PRESSURES_HPA.len();
/// Humidity barely matters; the grid takes a middling one.
const HUMIDITY_PERCENT: f64 = 50.0;
/// The interpolation through the rows takes four of them.
pub const MIN_ROWS: usize = 4;
pub const MAX_ROWS: usize = 120;
/// The longest name the watch shows, bytes.
const MAX_NAME: usize = 64;
/// The wind the wind columns are measured over, m/s (a hunting wind: the
/// drift grows nearly in proportion to it, so this sets the slope).
const REFERENCE_WIND_MPS: f64 = 4.0;
/// How far to look for the ethical limit, m; a limit past it is given as it.
pub const ETHICAL_SEARCH_M: f64 = 3000.0;
const COLUMNS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct WatchProfile {
    /// Tells the watch which profile the hunter had chosen, across updates.
    pub id: [u8; 8],
    pub name: String,
    pub click_unit: ClickUnit,
    /// In the click unit.
    pub click_value: f64,
    /// The hunter's units, for the watch: 0 m, 1 yd.
    pub range_unit: u8,
    /// 0 m/s, 1 km/h, 2 mph.
    pub wind_unit: u8,
    /// 0 cm, 1 in.
    pub length_unit: u8,
    /// 0 °C, 1 °F.
    pub temperature_unit: u8,
    /// 0 hPa, 1 mmHg, 2 inHg.
    pub pressure_unit: u8,
    pub step_m: u16,
    pub rows: usize,
    /// Per grid cell (temperatures outer, pressures inner): the ethical
    /// limit, m, `ETHICAL_SEARCH_M` when it is past that; `None` when the
    /// target asks no minimum energy.
    pub ethical_m: Option<[u16; CELLS]>,
    /// Milliradians per step of each column's numbers.
    pub scales: [f32; COLUMNS],
    /// Per cell, per row, per column, in steps of the column's scale:
    /// elevation up and windage right in calm air, then the elevation and
    /// windage more per 1 m/s of wind from the right (the aerodynamic jump
    /// and the drift), then per 1 m/s of wind head-on, then per 1 m/s of
    /// crosswind times 1 m/s of headwind (how the two together differ from
    /// each alone). Row `i` is at `(i + 1) × step_m`.
    pub values: Vec<i16>,
}

/// What the watch shows for one shot (the same sums, for the tests).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WatchHold {
    pub elevation_mrad: f64,
    pub windage_mrad: f64,
}

/// The subject in the grid's first weather, without wind, level, without
/// cant or Coriolis, the cartridges at the air's temperature: all a profile
/// takes from the day is whether spin drift and the jump are counted.
pub fn on_grid(subject: &Subject) -> Subject {
    let mut subject = subject.clone();
    subject.conditions = ballistics_shot::Conditions {
        temperature_c: f64::from(TEMPERATURES_C[0]),
        pressure: Pressure::Station(f64::from(PRESSURES_HPA[0])),
        humidity_percent: HUMIDITY_PERCENT,
        powder_temperature_c: None,
        spin_drift: subject.conditions.spin_drift,
        aerodynamic_jump: subject.conditions.aerodynamic_jump,
        ..ballistics_shot::Conditions::default()
    };
    subject
}

impl WatchProfile {
    /// The profile out to `farthest_m`.
    pub fn compute(subject: &Subject, u: &UnitPrefs, farthest_m: f64) -> Result<Self, String> {
        let step = f64::from(STEP_M);
        let rows = if farthest_m.is_finite() {
            ((farthest_m / step).floor() as usize).clamp(MIN_ROWS, MAX_ROWS)
        } else {
            MIN_ROWS
        };
        let last = rows as f64 * step;
        let mut cells = Vec::with_capacity(CELLS * rows);
        let mut ethical = [0; CELLS];
        for (t, &temperature) in TEMPERATURES_C.iter().enumerate() {
            for (p, &pressure) in PRESSURES_HPA.iter().enumerate() {
                let subject = in_weather(subject, f64::from(temperature), f64::from(pressure));
                let solve = |wind_mps: f64, clock: f64, max_range: f64| {
                    solve_in_wind(&subject, wind_mps, clock, max_range)
                        .map_err(|error| error.to_string())
                };
                let calm = solve(0.0, 3.0, last + step)?;
                let cross = solve(REFERENCE_WIND_MPS, 3.0, last + step)?;
                let head = solve(REFERENCE_WIND_MPS, 12.0, last + step)?;
                // From half past one: as much across as head-on.
                let both = solve(REFERENCE_WIND_MPS, 1.5, last + step)?;
                let part = REFERENCE_WIND_MPS * std::f64::consts::FRAC_1_SQRT_2;
                for index in 1..=rows {
                    let range = index as f64 * step;
                    let (Some(calm_row), Some(cross_row), Some(head_row), Some(both_row)) = (
                        calm.at(range),
                        cross.at(range),
                        head.at(range),
                        both.at(range),
                    ) else {
                        return Err(tr(
                            "The bullet doesn't reach the last range in the coldest air. \
                             Choose a shorter range.",
                        )
                        .to_owned());
                    };
                    let per_mps =
                        |windy: f64, calm: f64| (windy - calm) / REFERENCE_WIND_MPS * 1000.0;
                    let elevation = [
                        calm_row.elevation_correction,
                        cross_row.elevation_correction,
                        head_row.elevation_correction,
                        both_row.elevation_correction,
                    ];
                    let windage = [
                        calm_row.windage_correction,
                        cross_row.windage_correction,
                        head_row.windage_correction,
                        both_row.windage_correction,
                    ];
                    // What the half-past-one wind does beyond its two parts.
                    let mixed = |[calm, cross, head, both]: [f64; 4]| {
                        let alone = per_mps(cross, calm) * part + per_mps(head, calm) * part;
                        ((both - calm) * 1000.0 - alone) / (part * part)
                    };
                    cells.push([
                        calm_row.elevation_correction * 1000.0,
                        calm_row.windage_correction * 1000.0,
                        per_mps(elevation[1], elevation[0]),
                        per_mps(windage[1], windage[0]),
                        per_mps(elevation[2], elevation[0]),
                        per_mps(windage[2], windage[0]),
                        mixed(elevation),
                        mixed(windage),
                    ]);
                }
                ethical[t * PRESSURES_HPA.len() + p] = match subject.ethical_limit(&calm) {
                    Some(limit) => limit,
                    // Not within the table: look further, so the watch can
                    // tell "past the table" from "no limit".
                    None => solve(0.0, 3.0, ETHICAL_SEARCH_M)
                        .ok()
                        .and_then(|far| subject.ethical_limit(&far))
                        .unwrap_or(ETHICAL_SEARCH_M),
                }
                .round()
                .clamp(1.0, ETHICAL_SEARCH_M)
                    as u16;
            }
        }
        let mut scales = [0f32; COLUMNS];
        for (column, scale) in scales.iter_mut().enumerate() {
            let most = cells
                .iter()
                .map(|row| row[column].abs())
                .fold(0.0, f64::max);
            // A column of zeros (no spin drift, say) still has a scale.
            *scale = (most / f64::from(i16::MAX)).max(1e-7) as f32;
        }
        let values = cells
            .iter()
            .flat_map(|row| {
                (0..COLUMNS).map(move |column| {
                    (row[column] / f64::from(scales[column]))
                        .round()
                        .clamp(-f64::from(i16::MAX), f64::from(i16::MAX)) as i16
                })
            })
            .collect();
        let index_of = |quantity: Quantity| {
            let label = u.label(quantity);
            quantity
                .units()
                .iter()
                .position(|unit| unit.label == label)
                .unwrap_or(0) as u8
        };
        let rifle = &subject.rifle;
        Ok(WatchProfile {
            id: profile_id(subject),
            name: watch_name(&rifle.name, &subject.bullet.display_name()),
            click_unit: rifle.click_unit,
            click_value: rifle.click_value,
            range_unit: index_of(Quantity::Distance),
            wind_unit: index_of(Quantity::WindSpeed),
            length_unit: index_of(Quantity::ShortLength),
            temperature_unit: index_of(Quantity::Temperature),
            pressure_unit: index_of(Quantity::Pressure),
            step_m: STEP_M,
            rows,
            ethical_m: subject.min_energy_j.map(|_| ethical),
            scales,
            values,
        })
    }

    /// The farthest range, m.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_m(&self) -> f64 {
        self.rows as f64 * f64::from(self.step_m)
    }

    /// The bytes the watch reads: profile `index` of `count` in the bundle
    /// `version`.
    pub fn encode(&self, version: u32, index: u8, count: u8) -> Vec<u8> {
        let name = shorten(&self.name);
        let mut bytes = Vec::with_capacity(80 + name.len() + 2 * self.values.len());
        bytes.push(FORMAT);
        bytes.extend(version.to_le_bytes());
        bytes.push(index);
        bytes.push(count);
        bytes.extend(self.id);
        bytes.push(match self.click_unit {
            ClickUnit::Mil => 0,
            ClickUnit::Moa => 1,
        });
        bytes.extend((self.click_value as f32).to_le_bytes());
        bytes.extend([
            self.range_unit,
            self.wind_unit,
            self.length_unit,
            self.temperature_unit,
            self.pressure_unit,
        ]);
        bytes.extend(self.step_m.to_le_bytes());
        bytes.extend((self.rows as u16).to_le_bytes());
        bytes.push(TEMPERATURES_C.len() as u8);
        bytes.extend(TEMPERATURES_C.map(|t| t as u8));
        bytes.push(PRESSURES_HPA.len() as u8);
        for pressure in PRESSURES_HPA {
            bytes.extend(pressure.to_le_bytes());
        }
        for scale in self.scales {
            bytes.extend(scale.to_le_bytes());
        }
        for limit in self.ethical_m.unwrap_or([0; CELLS]) {
            bytes.extend(limit.to_le_bytes());
        }
        bytes.push(name.len() as u8);
        bytes.extend(name.as_bytes());
        for value in &self.values {
            bytes.extend(value.to_le_bytes());
        }
        bytes
    }

    /// Column `column` of row `row` in cell (`t`, `p`), mrad.
    fn value(&self, t: usize, p: usize, row: usize, column: usize) -> f64 {
        let cell = t * PRESSURES_HPA.len() + p;
        f64::from(self.values[(cell * self.rows + row) * COLUMNS + column])
            * f64::from(self.scales[column])
    }

    /// The watch's sums (docs/watch.md, "On the watch"): `None` before the
    /// first row or past the last.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn hold(
        &self,
        range_m: f64,
        wind_mps: f64,
        clock: f64,
        temperature_c: f64,
        pressure_hpa: f64,
    ) -> Option<WatchHold> {
        let step = f64::from(self.step_m);
        let position = range_m / step - 1.0;
        if !(-1e-9..=(self.rows - 1) as f64 + 1e-9).contains(&position) {
            return None;
        }
        let first = (position.floor() as isize - 1).clamp(0, self.rows as isize - 4) as usize;
        let nodes: Vec<f64> = (first..first + 4).map(|row| row as f64).collect();
        let along = weights(&nodes, position);
        let temperatures = TEMPERATURES_C.map(f64::from);
        let pressures = PRESSURES_HPA.map(f64::from);
        let by_t = weights(&temperatures, clamp_to(&temperatures, temperature_c));
        let by_p = weights(&pressures, clamp_to(&pressures, pressure_hpa));
        let at = |column: usize| {
            let mut sum = 0.0;
            for (t, wt) in by_t.iter().enumerate() {
                for (p, wp) in by_p.iter().enumerate() {
                    for (k, wr) in along.iter().enumerate() {
                        let row = first + k;
                        let row_m = (row + 1) as f64 * step;
                        sum += wt * wp * wr * self.value(t, p, row, column) * row_m;
                    }
                }
            }
            sum / range_m
        };
        let angle = clock * std::f64::consts::PI / 6.0;
        let cross = wind_mps * angle.sin();
        let head = wind_mps * angle.cos();
        Some(WatchHold {
            elevation_mrad: at(0) + at(2) * cross + at(4) * head + at(6) * cross * head,
            windage_mrad: at(1) + at(3) * cross + at(5) * head + at(7) * cross * head,
        })
    }

    /// The ethical limit in this weather, m (straight lines between the
    /// grid's cells); `None` when there is none.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ethical_limit(&self, temperature_c: f64, pressure_hpa: f64) -> Option<f64> {
        let limits = self.ethical_m?;
        let temperatures = TEMPERATURES_C.map(f64::from);
        let pressures = PRESSURES_HPA.map(f64::from);
        let (t, ft) = bracket(&temperatures, temperature_c);
        let (p, fp) = bracket(&pressures, pressure_hpa);
        let at = |t: usize, p: usize| f64::from(limits[t * PRESSURES_HPA.len() + p]);
        let low = at(t, p) + (at(t, p + 1) - at(t, p)) * fp;
        let high = at(t + 1, p) + (at(t + 1, p + 1) - at(t + 1, p)) * fp;
        Some(low + (high - low) * ft)
    }
}

/// The subject on the grid in one of its weathers.
fn in_weather(subject: &Subject, temperature_c: f64, pressure_hpa: f64) -> Subject {
    let mut subject = on_grid(subject);
    subject.conditions.temperature_c = temperature_c;
    subject.conditions.pressure = Pressure::Station(pressure_hpa);
    subject
}

/// The flight in the subject's weather in a wind from `clock` o'clock.
fn solve_in_wind(
    subject: &Subject,
    wind_mps: f64,
    clock: f64,
    max_range: f64,
) -> Result<ballistics_core::solver::Solution, ballistics_core::solver::SolveError> {
    let conditions = ballistics_shot::Conditions {
        wind_speed_mps: wind_mps,
        wind_clock: clock,
        ..subject.conditions.clone()
    };
    let scenario = ballistics_shot::scenario(
        &subject.rifle,
        &subject.bullet,
        subject.load.as_ref(),
        &conditions,
    );
    ballistics_core::solver::solve(&scenario, max_range)
}

/// The weight of each node in the polynomial through all of them, at `x`
/// (Lagrange).
pub fn weights(nodes: &[f64], x: f64) -> Vec<f64> {
    (0..nodes.len())
        .map(|i| {
            (0..nodes.len())
                .filter(|&j| j != i)
                .map(|j| (x - nodes[j]) / (nodes[i] - nodes[j]))
                .product()
        })
        .collect()
}

/// `x` held within the nodes' span: the watch doesn't reach past the grid.
pub fn clamp_to(nodes: &[f64], x: f64) -> f64 {
    x.clamp(nodes[0], nodes[nodes.len() - 1])
}

/// The node at or below `x` (never the last) and how far on to the next.
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let x = clamp_to(nodes, x);
    let below = nodes
        .windows(2)
        .position(|pair| x <= pair[1])
        .unwrap_or(nodes.len() - 2);
    (
        below,
        (x - nodes[below]) / (nodes[below + 1] - nodes[below]),
    )
}

/// The first 8 bytes of SHA-256 of the rifle's and bullet's ids.
fn profile_id(subject: &Subject) -> [u8; 8] {
    let digest = Sha256::new()
        .chain_update(subject.rifle.id.as_bytes())
        .chain_update(b"/")
        .chain_update(subject.bullet.id.as_bytes())
        .finalize();
    digest[..8].try_into().expect("8 of 32 bytes")
}

/// "Rifle · bullet" in what the watch takes: the rifle's name is cut first,
/// so two loads of one rifle still differ.
fn watch_name(rifle: &str, bullet: &str) -> String {
    let full = format!("{rifle} · {bullet}");
    let room = MAX_NAME.saturating_sub(" · ".len() + bullet.len());
    if full.len() <= MAX_NAME || room < 8 {
        return shorten(&full);
    }
    format!("{} · {bullet}", shorten_to(rifle, room).trim_end())
}

/// The name cut to what the watch takes, on a character boundary.
fn shorten(name: &str) -> String {
    shorten_to(name, MAX_NAME)
}

fn shorten_to(name: &str, most: usize) -> String {
    let mut end = name.len().min(most);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::tests::demo_subject;

    /// Every demo rifle with a bullet that fits it.
    fn demo_subjects() -> Vec<Subject> {
        let mut store = ballistics_store::Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        let bullets = store.bullets().unwrap();
        let base = demo_subject();
        store
            .rifles()
            .unwrap()
            .into_iter()
            .filter_map(|rifle| {
                let bullet = bullets
                    .iter()
                    .find(|bullet| bullet.caliber == rifle.caliber)?
                    .clone();
                Some(Subject {
                    rifle,
                    bullet,
                    ..base.clone()
                })
            })
            .collect()
    }

    /// The truth: that very shot solved, the wind from its own clock.
    fn solved(subject: &Subject, range: f64, wind: f64, clock: f64, t: f64, p: f64) -> (f64, f64) {
        let row = solve_in_wind(&in_weather(subject, t, p), wind, clock, range + 50.0)
            .unwrap()
            .at(range)
            .unwrap();
        (
            row.elevation_correction * 1000.0,
            row.windage_correction * 1000.0,
        )
    }

    #[test]
    fn the_watch_matches_the_solver() {
        let mut checked = 0;
        for subject in demo_subjects() {
            let profile = WatchProfile::compute(&subject, &UnitPrefs::metric(), 600.0).unwrap();
            assert_eq!(profile.rows, 24);
            assert_eq!(profile.last_m(), 600.0);
            // Ranges between rows, winds and weather the grid wasn't made
            // for, from 2 and 10 o'clock.
            for (range, wind, clock, t, p) in [
                (347.0, 7.0, 2.0, -17.0, 1003.0),
                (512.0, 3.5, 10.0, 24.0, 931.0),
                (90.0, 9.0, 2.0, 3.0, 812.0),
                (600.0, 5.0, 2.0, -29.0, 1048.0),
                (420.0, 6.0, 7.0, 31.0, 745.0),
                (275.0, 12.0, 4.0, 8.0, 1013.0),
            ] {
                let (elevation, windage) = solved(&subject, range, wind, clock, t, p);
                // 0.01 mil, or 0.2 % in a strong wind (the wind columns are
                // slopes measured over a 4 m/s wind).
                let close = |truth: f64| 0.01 + 0.002 * truth.abs();
                let hold = profile.hold(range, wind, clock, t, p).unwrap();
                assert!(
                    (hold.elevation_mrad - elevation).abs() < close(elevation),
                    "{} at {range} m: {hold:?} {elevation}",
                    profile.name
                );
                assert!(
                    (hold.windage_mrad - windage).abs() < close(windage),
                    "{} at {range} m: {hold:?} {windage}",
                    profile.name
                );
                checked += 1;
            }
        }
        assert!(checked >= 8, "{checked}");
    }

    #[test]
    fn the_ethical_limit_follows_the_weather() {
        let subject = demo_subject();
        let profile = WatchProfile::compute(&subject, &UnitPrefs::metric(), 200.0).unwrap();
        let limit = |t: f64, p: f64| {
            let subject = in_weather(&subject, t, p);
            let far = solve_in_wind(&subject, 0.0, 3.0, ETHICAL_SEARCH_M).unwrap();
            subject.ethical_limit(&far).unwrap()
        };
        // Past the table's end too.
        let watch = profile.ethical_limit(13.0, 1050.0).unwrap();
        assert!(watch > 200.0);
        assert!((watch - limit(13.0, 1050.0)).abs() < 1.0, "{watch}");
        // Between cells, within a few metres.
        let watch = profile.ethical_limit(0.0, 990.0).unwrap();
        assert!((watch - limit(0.0, 990.0)).abs() < 10.0, "{watch}");
        // No minimum energy, no limit.
        let sport = Subject {
            min_energy_j: None,
            ..subject
        };
        let profile = WatchProfile::compute(&sport, &UnitPrefs::metric(), 200.0).unwrap();
        assert_eq!(profile.ethical_limit(13.0, 1013.0), None);
    }

    #[test]
    fn nothing_before_the_first_row_or_past_the_last() {
        let profile = WatchProfile::compute(&demo_subject(), &UnitPrefs::metric(), 600.0).unwrap();
        assert_eq!(profile.hold(20.0, 0.0, 3.0, 15.0, 1013.0), None);
        assert_eq!(profile.hold(601.0, 0.0, 3.0, 15.0, 1013.0), None);
        assert!(profile.hold(25.0, 0.0, 3.0, 15.0, 1013.0).is_some());
        assert!(profile.hold(600.0, 0.0, 3.0, 15.0, 1013.0).is_some());
        // Weather past the grid is held at its edge.
        assert_eq!(
            profile.hold(300.0, 0.0, 3.0, -45.0, 600.0),
            profile.hold(300.0, 0.0, 3.0, -30.0, 700.0)
        );
    }

    #[test]
    fn the_bytes_follow_the_format() {
        let subject = demo_subject();
        let profile = WatchProfile::compute(&subject, &UnitPrefs::imperial(), 300.0).unwrap();
        let bytes = profile.encode(1_793_491_200, 1, 3);
        assert_eq!(bytes[0], FORMAT);
        assert_eq!(
            u32::from_le_bytes(bytes[1..5].try_into().unwrap()),
            1_793_491_200
        );
        assert_eq!(&bytes[5..7], &[1, 3]);
        assert_eq!(&bytes[7..15], &profile.id);
        assert_eq!(
            f32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            profile.click_value as f32
        );
        // Yards, mph, inches, °F, inHg.
        assert_eq!(&bytes[20..25], &[1, 2, 1, 1, 2]);
        assert_eq!(u16::from_le_bytes([bytes[25], bytes[26]]), STEP_M);
        assert_eq!(u16::from_le_bytes([bytes[27], bytes[28]]), 12);
        assert_eq!(bytes[29], 4);
        assert_eq!(bytes[30] as i8, -30);
        assert_eq!(bytes[34], 3);
        assert_eq!(u16::from_le_bytes([bytes[35], bytes[36]]), 700);
        let name_at = 41 + 4 * COLUMNS + 2 * CELLS;
        let name = profile.name.as_bytes();
        assert_eq!(usize::from(bytes[name_at]), name.len());
        assert_eq!(&bytes[name_at + 1..name_at + 1 + name.len()], name);
        assert_eq!(
            bytes.len(),
            name_at + 1 + name.len() + 2 * COLUMNS * 12 * CELLS
        );
    }

    #[test]
    fn long_names_keep_the_bullet() {
        let bullet = "Federal Terminal Ascent 11.3 g (175 gr)";
        let name = watch_name("Browning X-Bolt Hunter Long Range McMillan", bullet);
        assert!(name.len() <= MAX_NAME, "{name}");
        assert!(name.ends_with(bullet), "{name}");
        assert!(name.starts_with("Browning X-Bolt"), "{name}");
        assert_eq!(watch_name("Tikka", "Lapua"), "Tikka · Lapua");
    }

    #[test]
    fn long_names_are_cut_between_characters() {
        let long = "Sako 85 Finnlight ".repeat(3) + "ääää";
        let short = shorten(&long);
        assert!(short.len() <= MAX_NAME);
        assert!(long.starts_with(&short));
    }
}

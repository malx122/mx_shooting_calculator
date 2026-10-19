//! A shot from the hunter's records. Turns a rifle, a bullet (with the
//! velocities measured in that rifle) and the day's conditions into the
//! physics engine's scenario, and the engine's answer into holds in the
//! rifle's clicks. The app and the AI tools (`mxsc-mcp`) both use it, so
//! they give the same answers.

use std::f64::consts::PI;
use std::sync::Arc;

use ballistics_core::{
    Atmosphere, BcBands, Coriolis, DragCurve, DragModel, Load, RangeRow, Scenario, Solution, Wind,
};
use ballistics_store::{BcModel, Bullet, ClickUnit, LoadZero, Rifle, RifleLoad, ZeroWeather};
use serde::{Deserialize, Serialize};

const METRES_PER_INCH: f64 = 0.0254;
const RAD_PER_MOA: f64 = PI / (180.0 * 60.0);

/// How the air pressure is known.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pressure {
    /// Measured where the hunter stands (a barometer), hPa.
    Station(f64),
    /// The weather report's sea-level pressure (hPa) and the altitude (m).
    SeaLevel { hpa: f64, altitude_m: f64 },
}

impl Pressure {
    pub fn station_hpa(self) -> f64 {
        match self {
            Pressure::Station(hpa) => hpa,
            Pressure::SeaLevel { hpa, altitude_m } => Atmosphere::station_pressure(hpa, altitude_m),
        }
    }
}

/// The day's weather and the shot's geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Conditions {
    pub temperature_c: f64,
    pub pressure: Pressure,
    pub humidity_percent: f64,
    /// The cartridges' temperature; `None` when it follows the air's.
    pub powder_temperature_c: Option<f64>,
    pub wind_speed_mps: f64,
    /// Where the wind comes from, on a clock: 12 head-on, 3 from the right.
    pub wind_clock: f64,
    /// Up (positive) or down the line of sight.
    pub inclination_degrees: f64,
    /// Latitude and the shot's direction (degrees from north), for Coriolis.
    pub coriolis: Option<Coriolis>,
    pub spin_drift: bool,
}

impl Default for Conditions {
    /// A standard day without wind: 15 °C, 1013.25 hPa at sea level, 50 %.
    fn default() -> Self {
        Conditions {
            temperature_c: 15.0,
            pressure: Pressure::SeaLevel {
                hpa: 1013.25,
                altitude_m: 0.0,
            },
            humidity_percent: 50.0,
            powder_temperature_c: None,
            wind_speed_mps: 0.0,
            wind_clock: 3.0,
            inclination_degrees: 0.0,
            coriolis: None,
            spin_drift: true,
        }
    }
}

impl Conditions {
    pub fn powder_temperature_c(&self) -> f64 {
        self.powder_temperature_c.unwrap_or(self.temperature_c)
    }

    pub fn atmosphere(&self) -> Atmosphere {
        Atmosphere {
            temperature_c: self.temperature_c,
            pressure_hpa: self.pressure.station_hpa(),
            humidity: self.humidity_percent / 100.0,
        }
    }
}

/// The engine's scenario for this rifle and bullet. `load` is what was
/// measured with the bullet in this rifle (velocities, point of impact,
/// zero weather), if anything.
pub fn scenario(
    rifle: &Rifle,
    bullet: &Bullet,
    load: Option<&RifleLoad>,
    conditions: &Conditions,
) -> Scenario {
    let atmosphere = conditions.atmosphere();
    let powder_temperature_c = conditions.powder_temperature_c();
    let velocity = match load {
        Some(load) => load.velocity_model(bullet),
        None => RifleLoad::new(&rifle.id, &bullet.id).velocity_model(bullet),
    };
    let recorded = |weather: ZeroWeather| {
        let atmosphere = Atmosphere {
            temperature_c: weather.temperature_c,
            pressure_hpa: weather.pressure_hpa,
            humidity: weather.humidity_percent / 100.0,
        };
        (atmosphere, weather.powder_temperature_c)
    };
    let (zero_atmosphere, zero_powder_temperature_c) =
        match load.map_or(LoadZero::Rifle, |load| load.zero) {
            LoadZero::Rifle => recorded(rifle.zero_weather()),
            LoadZero::SameAsShot => (atmosphere, powder_temperature_c),
            LoadZero::Recorded(weather) => recorded(weather),
        };
    let bands: Vec<(f64, f64)> = bullet
        .bc_bands
        .iter()
        .map(|band| (band.velocity_mps, band.bc))
        .collect();
    let offset = load.map_or([0.0, 0.0], |load| load.impact_offset_mrad);
    Scenario {
        bullet: ballistics_core::Bullet {
            mass_kg: bullet.mass_g / 1000.0,
            diameter_m: bullet.diameter_mm / 1000.0,
            length_m: bullet.length_mm.map(|mm| mm / 1000.0),
            ballistic_coefficient: bullet.ballistic_coefficient().unwrap_or(0.0),
            drag_model: match bullet.drag_model {
                BcModel::G7 => DragModel::G7,
                BcModel::G1 | BcModel::Custom => DragModel::G1,
            },
            // The store validates bands and curves, so these only fall back
            // on bad data.
            bc_bands: BcBands::new(&bands).unwrap_or(BcBands::NONE),
            drag_curve: (bullet.drag_model == BcModel::Custom)
                .then(|| {
                    DragCurve::new(
                        bullet
                            .drag_curve
                            .iter()
                            .map(|point| (point.mach, point.cd))
                            .collect(),
                    )
                    .ok()
                })
                .flatten()
                .map(Arc::new),
        },
        rifle: ballistics_core::Rifle {
            sight_height_m: rifle.sight_height_cm / 100.0,
            zero_range_m: rifle.zero_range_m,
            twist_m: rifle.twist_in * METRES_PER_INCH,
            right_hand_twist: rifle.right_hand_twist,
        },
        load: Load {
            muzzle_velocity: velocity.velocity_mps,
            reference_temperature_c: velocity.reference_temperature_c,
            temperature_sensitivity: velocity.per_degree,
        },
        zero_atmosphere,
        zero_powder_temperature_c,
        impact_offset: [offset[0] / 1000.0, offset[1] / 1000.0],
        shot: ballistics_core::Conditions {
            atmosphere,
            powder_temperature_c,
            wind: Wind {
                speed: conditions.wind_speed_mps,
                from_degrees: conditions.wind_clock * 30.0,
            },
            inclination_degrees: conditions.inclination_degrees,
            coriolis: conditions.coriolis,
        },
        spin_drift: conditions.spin_drift,
    }
}

/// An angle in the rifle's click unit (mil or MOA).
pub fn from_radians(unit: ClickUnit, radians: f64) -> f64 {
    match unit {
        ClickUnit::Mil => radians * 1000.0,
        ClickUnit::Moa => radians / RAD_PER_MOA,
    }
}

pub fn to_radians(unit: ClickUnit, value: f64) -> f64 {
    match unit {
        ClickUnit::Mil => value / 1000.0,
        ClickUnit::Moa => value * RAD_PER_MOA,
    }
}

pub fn unit_label(unit: ClickUnit) -> &'static str {
    match unit {
        ClickUnit::Mil => "mil",
        ClickUnit::Moa => "MOA",
    }
}

/// What to dial or hold at one range, in the rifle's scope units.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hold {
    pub range_m: f64,
    pub unit: ClickUnit,
    /// Up (positive) or down, in `unit`.
    pub elevation: f64,
    /// Right (positive) or left, in `unit`.
    pub windage: f64,
    pub elevation_clicks: i64,
    pub windage_clicks: i64,
    /// Where a dead-on hold hits: above (positive) or below the aim, m.
    pub drop_m: f64,
    /// Right (positive) or left of the aim, m.
    pub drift_m: f64,
    pub velocity_mps: f64,
    pub energy_j: f64,
    pub time_s: f64,
    pub mach: f64,
}

impl Hold {
    pub fn new(rifle: &Rifle, row: &RangeRow) -> Hold {
        let unit = rifle.click_unit;
        let elevation = from_radians(unit, row.elevation_correction);
        let windage = from_radians(unit, row.windage_correction);
        let clicks = |angle: f64| (angle / rifle.click_value).round() as i64;
        Hold {
            range_m: row.range,
            unit,
            elevation,
            windage,
            elevation_clicks: clicks(elevation),
            windage_clicks: clicks(windage),
            drop_m: row.drop,
            drift_m: row.windage,
            velocity_mps: row.velocity,
            energy_j: row.energy_j,
            time_s: row.time,
            mach: row.mach,
        }
    }
}

/// The velocity the bullet needs to expand: its maker's figure when known,
/// else the hunter's general one.
pub fn min_velocity(bullet: &Bullet, general_mps: f64) -> f64 {
    bullet.min_expansion_mps.unwrap_or(general_mps)
}

/// The Swedish ammunition class a load meets (NFS 2023:8).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LoadClass {
    /// The strictest class met, 1 being the strictest; `None` for none.
    pub class: Option<u8>,
    /// The bullet's construction isn't known (a Strelok import, say), so
    /// `class` assumes it is made to expand.
    pub assumes_expanding: bool,
}

/// Whether a load may be used on a game species in Sweden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Allowed {
    Yes,
    /// Only if the bullet expands: its construction isn't set.
    IfExpanding,
    No,
}

impl LoadClass {
    /// Whether the load meets the class the species needs, and why, in a
    /// sentence for `species`.
    pub fn verdict(&self, needed: u8, species: &str) -> (Allowed, String) {
        let met = self.class.is_some_and(|class| class <= needed);
        let class = self.class.map_or("meets no class".to_owned(), |class| {
            format!("is class {class}")
        });
        match (met, self.assumes_expanding) {
            (true, false) => (
                Allowed::Yes,
                format!(
                    "Swedish rules: {species} needs class {needed} or stricter; this load {class}: allowed."
                ),
            ),
            (true, true) => (
                Allowed::IfExpanding,
                format!(
                    "Swedish rules: {species} needs class {needed} or stricter; this load {class} if the \
                     bullet is made to expand. Its construction isn't set in the app: set it on the \
                     bullet to be sure."
                ),
            ),
            (false, _) => (
                Allowed::No,
                format!(
                    "Swedish rules: {species} needs class {needed} or stricter; this load {class}: not allowed."
                ),
            ),
        }
    }
}

/// The Swedish ammunition class of the load: its weight, its energy at the
/// muzzle and at 100 m, and whether it expands. When the construction is
/// unknown it is assumed to expand, and the answer says so.
pub fn load_class(bullet: &Bullet, solution: &Solution) -> LoadClass {
    let energy_at = |range: f64| solution.at(range).map_or(0.0, |row| row.energy_j);
    let expanding = bullet.construction.expanding();
    LoadClass {
        class: ballistics_store::swedish_class(
            bullet.mass_g,
            energy_at(100.0),
            energy_at(0.0),
            Some(expanding.unwrap_or(true)),
            bullet.lead_free,
        ),
        assumes_expanding: expanding.is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_core::solver;

    fn rifle() -> Rifle {
        let mut rifle = Rifle::new("Test", "308-win", 10.0);
        rifle.click_unit = ClickUnit::Mil;
        rifle.click_value = 0.1;
        rifle
    }

    fn bullet() -> Bullet {
        let mut bullet = Bullet::new_manual("308-win", 7.82);
        bullet.mass_g = 11.7;
        bullet.bc_g1 = Some(0.45);
        bullet.muzzle_velocity_mps = 790.0;
        bullet
    }

    #[test]
    fn the_zero_range_needs_no_hold_and_further_out_holds_up() {
        let rifle = rifle();
        let solution = solver::solve(
            &scenario(&rifle, &bullet(), None, &Conditions::default()),
            400.0,
        )
        .unwrap();
        let zero = Hold::new(&rifle, &solution.at(rifle.zero_range_m).unwrap());
        assert!(zero.elevation.abs() < 0.05, "{zero:?}");
        assert_eq!(zero.elevation_clicks, 0);
        let far = Hold::new(&rifle, &solution.at(300.0).unwrap());
        assert!(far.elevation > 1.0 && far.drop_m < 0.0, "{far:?}");
        assert_eq!(far.elevation_clicks, (far.elevation / 0.1).round() as i64);
        assert!(far.windage.abs() < 0.2, "only spin drift");
    }

    #[test]
    fn wind_from_the_right_needs_a_hold_to_the_right() {
        let rifle = rifle();
        let conditions = Conditions {
            wind_speed_mps: 5.0,
            wind_clock: 3.0,
            spin_drift: false,
            ..Conditions::default()
        };
        let solution =
            solver::solve(&scenario(&rifle, &bullet(), None, &conditions), 400.0).unwrap();
        let hold = Hold::new(&rifle, &solution.at(300.0).unwrap());
        assert!(hold.windage > 0.0 && hold.drift_m < 0.0, "{hold:?}");
    }

    #[test]
    fn an_unknown_construction_is_assumed_to_expand_and_said_so() {
        let rifle = rifle();
        let mut bullet = bullet();
        bullet.construction = ballistics_store::Construction::Other;
        let solution = solver::solve(
            &scenario(&rifle, &bullet, None, &Conditions::default()),
            400.0,
        )
        .unwrap();
        let class = load_class(&bullet, &solution);
        assert_eq!(class.class, Some(1));
        assert!(class.assumes_expanding);
        assert_eq!(class.verdict(1, "Moose").0, Allowed::IfExpanding);
        bullet.construction = ballistics_store::Construction::Fmj;
        let fmj = load_class(&bullet, &solution);
        assert_eq!(fmj.class, Some(3));
        assert_eq!(fmj.verdict(2, "Roe deer").0, Allowed::No);
        bullet.construction = ballistics_store::Construction::Bonded;
        assert_eq!(
            load_class(&bullet, &solution).verdict(1, "Moose").0,
            Allowed::Yes
        );
    }

    #[test]
    fn angles_convert_both_ways() {
        for unit in [ClickUnit::Mil, ClickUnit::Moa] {
            assert!((to_radians(unit, from_radians(unit, 0.01)) - 0.01).abs() < 1e-12);
        }
        assert!(
            (from_radians(ClickUnit::Moa, to_radians(ClickUnit::Mil, 1.0)) - 3.4377).abs() < 1e-3
        );
    }
}

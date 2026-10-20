//! A shot from the hunter's records. Turns a rifle, a bullet (with the
//! velocities measured in that rifle) and the day's conditions into the
//! physics engine's scenario, and the engine's answer into holds in the
//! rifle's clicks. The app and the AI tools (`mxsc-mcp`) both use it, so
//! they give the same answers.

use std::f64::consts::PI;
use std::sync::Arc;

use ballistics_core::solver;
use ballistics_core::{
    Atmosphere, BcBands, Coriolis, DragCurve, DragModel, Load, RangeRow, Scenario, Solution, Wind,
};
use ballistics_store::{
    BcModel, Bullet, ClickUnit, Country, LoadZero, Rifle, RifleLoad, RulesLoad, Truing,
    TruingMethod, ZeroWeather,
};
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
    /// The jump a crosswind gives the bullet at the muzzle.
    pub aerodynamic_jump: bool,
    /// The rifle tilted: positive with the scope's top to the right.
    pub cant_degrees: f64,
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
            aerodynamic_jump: true,
            cant_degrees: 0.0,
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
    // A trued load scales its velocity, or its drag (the BC the other way).
    let (velocity_factor, drag_factor) = match load.and_then(|load| load.truing) {
        Some(Truing {
            method: TruingMethod::Velocity,
            factor,
            ..
        }) => (factor, 1.0),
        Some(Truing {
            method: TruingMethod::Drag,
            factor,
            ..
        }) => (1.0, factor),
        None => (1.0, 1.0),
    };
    let bands: Vec<(f64, f64)> = bullet
        .bc_bands
        .iter()
        .map(|band| (band.velocity_mps, band.bc / drag_factor))
        .collect();
    let offset = load.map_or([0.0, 0.0], |load| load.impact_offset_mrad);
    Scenario {
        bullet: ballistics_core::Bullet {
            mass_kg: bullet.mass_g / 1000.0,
            diameter_m: bullet.diameter_mm / 1000.0,
            length_m: bullet.length_mm.map(|mm| mm / 1000.0),
            ballistic_coefficient: bullet.ballistic_coefficient().unwrap_or(0.0) / drag_factor,
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
                            .map(|point| (point.mach, point.cd * drag_factor))
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
            zero_height_m: rifle.zero_height_cm / 100.0,
            twist_m: rifle.twist_in * METRES_PER_INCH,
            right_hand_twist: rifle.right_hand_twist,
        },
        load: Load {
            muzzle_velocity: velocity.velocity_mps * velocity_factor,
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
            cant_degrees: conditions.cant_degrees,
        },
        spin_drift: conditions.spin_drift,
        aerodynamic_jump: conditions.aerodynamic_jump,
    }
}

impl Conditions {
    /// The conditions recorded with a zero or a truing shot (no wind).
    pub fn recorded(weather: &ZeroWeather, inclination_degrees: f64) -> Conditions {
        Conditions {
            temperature_c: weather.temperature_c,
            pressure: Pressure::Station(weather.pressure_hpa),
            humidity_percent: weather.humidity_percent,
            powder_temperature_c: Some(weather.powder_temperature_c),
            inclination_degrees,
            wind_speed_mps: 0.0,
            ..Conditions::default()
        }
    }

    /// The weather part, to keep with a truing shot.
    pub fn weather(&self) -> ZeroWeather {
        ZeroWeather {
            temperature_c: self.temperature_c,
            pressure_hpa: self.pressure.station_hpa(),
            humidity_percent: self.humidity_percent,
            powder_temperature_c: self.powder_temperature_c(),
        }
    }
}

/// Why a load can't be trued to an observed shot.
#[derive(Clone, Debug, PartialEq)]
pub enum TruingError {
    /// The range is too close to the zero for the drop to say much.
    TooShort,
    /// No factor within reason explains the shot: check the zero, the
    /// range, the units or the bullet.
    NoMatch { predicted_mrad: f64 },
    /// The bullet doesn't reach the range.
    OutOfReach,
}

impl std::fmt::Display for TruingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TruingError::TooShort => f.write_str(
                "the drop at that range is too small to true from; use a longer range (300 m or more works best)",
            ),
            TruingError::NoMatch { predicted_mrad } => write!(
                f,
                "no sensible change explains that hold (the calculation says {predicted_mrad:.2} mrad); \
                 check the zero, the range and the units"
            ),
            TruingError::OutOfReach => f.write_str("the bullet doesn't reach that range"),
        }
    }
}

/// How far each method may move the load: velocity ±20 %, drag half to
/// double.
fn truing_bounds(method: TruingMethod) -> (f64, f64) {
    match method {
        TruingMethod::Velocity => (0.8, 1.2),
        TruingMethod::Drag => (0.5, 2.0),
    }
}

/// The elevation (mrad) the calculation gives at `range_m` with `truing`.
fn elevation_with(
    rifle: &Rifle,
    bullet: &Bullet,
    load: &RifleLoad,
    truing: Truing,
    range_m: f64,
) -> Option<f64> {
    let mut trued = load.clone();
    trued.truing = Some(truing);
    let conditions = Conditions::recorded(&truing.weather, truing.inclination_degrees);
    let solution = solver::solve(
        &scenario(rifle, bullet, Some(&trued), &conditions),
        range_m + 5.0,
    )
    .ok()?;
    Some(solution.at(range_m)?.elevation_correction * 1000.0)
}

/// Finds the velocity or drag factor that makes the calculation give the
/// elevation it took to hit centre at `range_m` (`elevation_mrad`, up
/// positive) in `conditions`. Any truing the load had is replaced.
pub fn true_load(
    rifle: &Rifle,
    bullet: &Bullet,
    load: &RifleLoad,
    method: TruingMethod,
    range_m: f64,
    elevation_mrad: f64,
    conditions: &Conditions,
) -> Result<Truing, TruingError> {
    let base = Truing {
        method,
        range_m,
        elevation_mrad,
        inclination_degrees: conditions.inclination_degrees,
        weather: conditions.weather(),
        factor: 1.0,
    };
    let at = |factor: f64| elevation_with(rifle, bullet, load, Truing { factor, ..base }, range_m);
    let predicted = at(1.0).ok_or(TruingError::OutOfReach)?;
    if predicted.abs() < 1.0 {
        return Err(TruingError::TooShort);
    }
    // More velocity means less to hold over; more drag means more.
    let (mut low, mut high) = truing_bounds(method);
    let miss = |factor: f64| at(factor).map(|elevation| elevation - elevation_mrad);
    let (Some(at_low), Some(at_high)) = (miss(low), miss(high)) else {
        return Err(TruingError::NoMatch {
            predicted_mrad: predicted,
        });
    };
    if at_low.signum() == at_high.signum() {
        return Err(TruingError::NoMatch {
            predicted_mrad: predicted,
        });
    }
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        let Some(at_middle) = miss(middle) else { break };
        if at_middle.signum() == at_low.signum() {
            low = middle;
        } else {
            high = middle;
        }
        if high - low < 1e-5 {
            break;
        }
    }
    Ok(Truing {
        factor: (low + high) / 2.0,
        ..base
    })
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

/// How a load fares under a country's rifle ammunition rules.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Assessment {
    pub country: Country,
    /// The strictest level met, 1 being the strictest; `None` for none.
    pub level: Option<u8>,
    /// The bullet's construction isn't known (a Strelok import, say), so
    /// `level` assumes it is made to expand.
    pub assumes_expanding: bool,
}

/// Whether a load may be used on a species.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Allowed {
    Yes,
    /// Only if the bullet expands: its construction isn't set.
    IfExpanding,
    No,
}

impl Assessment {
    /// Whether the load meets the level `needed` for `species`, and why, in
    /// a sentence.
    pub fn verdict(&self, needed: u8, species: &str) -> (Allowed, String) {
        let rules = self.country.rules();
        let met = self.level.is_some_and(|level| level <= needed);
        let rules_name = format!("{} rules", self.country.adjective());
        let wanted = rules.requirement_text(needed).to_lowercase();
        let load = self.level.map_or("meets no level".to_owned(), |level| {
            format!("is {}", rules.level_name(level).to_lowercase())
        });
        match (met, self.assumes_expanding) {
            (true, false) => (
                Allowed::Yes,
                format!("{rules_name}: {species} needs {wanted}; this load {load}: allowed."),
            ),
            (true, true) => (
                Allowed::IfExpanding,
                format!(
                    "{rules_name}: {species} needs {wanted}; this load {load} if the bullet is made \
                     to expand. Its construction isn't set in the app: set it on the bullet to be \
                     sure."
                ),
            ),
            (false, _) => (
                Allowed::No,
                format!("{rules_name}: {species} needs {wanted}; this load {load}: not allowed."),
            ),
        }
    }
}

/// The level of `country`'s rules the load meets: its weight, its energy at
/// the muzzle and at 100 m, whether it expands and whether it is lead-free.
/// When the construction is unknown it is assumed to expand, and the
/// assessment says so.
pub fn assess(bullet: &Bullet, solution: &Solution, country: Country) -> Assessment {
    let energy_at = |range: f64| solution.at(range).map_or(0.0, |row| row.energy_j);
    let expanding = bullet.construction.expanding();
    let load = RulesLoad {
        mass_g: bullet.mass_g,
        e0_j: energy_at(0.0),
        e100_j: energy_at(100.0),
        expanding: expanding.unwrap_or(true),
        lead_free: bullet.lead_free,
    };
    Assessment {
        country,
        level: country.rules().level_met(&load),
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
        let sweden = assess(&bullet, &solution, Country::Sweden);
        assert_eq!(sweden.level, Some(1));
        assert!(sweden.assumes_expanding);
        assert_eq!(sweden.verdict(1, "Moose").0, Allowed::IfExpanding);
        bullet.construction = ballistics_store::Construction::Fmj;
        let fmj = assess(&bullet, &solution, Country::Sweden);
        assert_eq!(fmj.level, Some(3));
        let (allowed, text) = fmj.verdict(2, "Roe deer");
        assert_eq!(allowed, Allowed::No);
        assert!(
            text.starts_with(
                "Swedish rules: Roe deer needs class 2 or stricter; this load is class 3"
            ),
            "{text}"
        );
        bullet.construction = ballistics_store::Construction::Bonded;
        let bonded = assess(&bullet, &solution, Country::Sweden);
        assert_eq!(bonded.verdict(1, "Moose").0, Allowed::Yes);
    }

    #[test]
    fn truing_finds_the_velocity_and_drag_that_match_a_long_shot() {
        let rifle = rifle();
        let bullet = bullet();
        let load = RifleLoad::new(&rifle.id, &bullet.id);
        let conditions = Conditions::default();
        // Pretend the real load is 3 % slower: what it took to hit at 400 m.
        let slower = Truing {
            method: TruingMethod::Velocity,
            range_m: 400.0,
            elevation_mrad: 0.0,
            inclination_degrees: 0.0,
            weather: conditions.weather(),
            factor: 0.97,
        };
        let observed = elevation_with(&rifle, &bullet, &load, slower, 400.0).unwrap();
        let trued = true_load(
            &rifle,
            &bullet,
            &load,
            TruingMethod::Velocity,
            400.0,
            observed,
            &conditions,
        )
        .unwrap();
        assert!((trued.factor - 0.97).abs() < 1e-3, "{trued:?}");
        // The same shot explained by drag instead: more drag than the BC says.
        let by_drag = true_load(
            &rifle,
            &bullet,
            &load,
            TruingMethod::Drag,
            400.0,
            observed,
            &conditions,
        )
        .unwrap();
        assert!(by_drag.factor > 1.0, "{by_drag:?}");
        // Applied, both give the observed hold at 400 m.
        for truing in [trued, by_drag] {
            let mut trued_load = load.clone();
            trued_load.truing = Some(truing);
            let solution = solver::solve(
                &scenario(&rifle, &bullet, Some(&trued_load), &conditions),
                450.0,
            )
            .unwrap();
            let elevation = solution.at(400.0).unwrap().elevation_correction * 1000.0;
            assert!(
                (elevation - observed).abs() < 0.01,
                "{elevation} vs {observed}"
            );
        }
        // Near the zero there is nothing to learn; far off, nothing fits.
        assert_eq!(
            true_load(
                &rifle,
                &bullet,
                &load,
                TruingMethod::Velocity,
                120.0,
                0.3,
                &conditions
            ),
            Err(TruingError::TooShort)
        );
        assert!(matches!(
            true_load(
                &rifle,
                &bullet,
                &load,
                TruingMethod::Velocity,
                400.0,
                observed * 3.0,
                &conditions
            ),
            Err(TruingError::NoMatch { .. })
        ));
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

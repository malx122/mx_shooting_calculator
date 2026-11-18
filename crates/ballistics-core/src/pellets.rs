//! Shotgun pellets: how many a load holds, one pellet's flight with a
//! sphere's drag, and the spread of a pattern from the choke.

use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use crate::drag::{DragCurve, DragModel};
use crate::solver::{self, BcBands, Bullet, Conditions, Sample, SolveError};

/// Density of pure lead; hardened (antimony) shot is a little lighter.
pub const LEAD_DENSITY_KG_M3: f64 = 11_340.0;
/// Density of steel shot.
pub const STEEL_DENSITY_KG_M3: f64 = 7_870.0;

/// Distance a choke's pattern percentage is measured at: 40 yards.
pub const PATTERN_DISTANCE_M: f64 = 36.576;
/// Radius of the circle a choke's pattern percentage is counted in: 30 inches across.
pub const PATTERN_RADIUS_M: f64 = 0.381;

/// The most pellets a load may hold or a pattern may have.
pub const MAX_PELLETS: usize = 100_000;

/// Drag coefficient of a sphere by Mach number, at the Reynolds numbers of
/// shot pellets (about 10⁴–10⁵). An approximate curve from published sphere
/// drag measurements, not yet checked against pellet velocity tables.
const SPHERE_DRAG: [(f64, f64); 16] = [
    (0.0, 0.47),
    (0.4, 0.47),
    (0.6, 0.50),
    (0.7, 0.54),
    (0.8, 0.60),
    (0.85, 0.65),
    (0.9, 0.71),
    (0.95, 0.78),
    (1.0, 0.84),
    (1.05, 0.89),
    (1.1, 0.92),
    (1.2, 0.96),
    (1.4, 0.99),
    (1.6, 1.0),
    (2.0, 1.0),
    (3.0, 0.98),
];

/// A path ends short of its range once the pellet falls more steeply than
/// this across the line of sight: its speed along the line below this share
/// of its whole speed (cos 60°).
const STEEPEST_FALL_ALONG_SHARE: f64 = 0.5;

/// A round shot pellet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pellet {
    pub diameter_m: f64,
    /// For example `LEAD_DENSITY_KG_M3` or `STEEL_DENSITY_KG_M3`.
    pub density_kg_m3: f64,
}

impl Pellet {
    /// One pellet's mass.
    pub fn mass_kg(&self) -> f64 {
        self.density_kg_m3 * PI / 6.0 * self.diameter_m.powi(3)
    }

    /// Whole pellets in a load of `load_kg`, or `None` for a pellet or load
    /// that isn't physical or a count above `MAX_PELLETS`.
    pub fn count_in(&self, load_kg: f64) -> Option<usize> {
        let mass = self.mass_kg();
        if !(mass.is_finite() && mass > 0.0 && load_kg.is_finite() && load_kg >= 0.0) {
            return None;
        }
        // The tolerance keeps a load of exactly n pellets from rounding to n − 1.
        let count = (load_kg / mass * (1.0 + 1e-12)).floor();
        (count <= MAX_PELLETS as f64).then_some(count as usize)
    }
}

pub(crate) fn sphere_drag() -> Arc<DragCurve> {
    static CURVE: OnceLock<Arc<DragCurve>> = OnceLock::new();
    CURVE
        .get_or_init(|| {
            Arc::new(DragCurve::new(SPHERE_DRAG.to_vec()).expect("the sphere drag curve is valid"))
        })
        .clone()
}

/// Flies one pellet along the bore, which is the line its samples' drop and
/// windage are measured from. The path ends at `max_range` (slant), or short
/// of it once the pellet falls more steeply than 60° across the line, so
/// check the last sample's range. The conditions' cant and powder
/// temperature don't apply.
pub fn fly(
    pellet: &Pellet,
    muzzle_velocity: f64,
    conditions: &Conditions,
    max_range: f64,
) -> Result<Vec<Sample>, SolveError> {
    solver::validate_max_range(max_range)?;
    validate(pellet, muzzle_velocity)?;
    solver::validate_conditions(conditions)?;
    let bullet = Bullet {
        mass_kg: pellet.mass_kg(),
        diameter_m: pellet.diameter_m,
        length_m: None,
        ballistic_coefficient: 0.0,
        drag_model: DragModel::G1,
        bc_bands: BcBands::NONE,
        drag_curve: Some(sphere_drag()),
    };
    let (mut last_range, mut last_time) = (0.0, 0.0);
    Ok(solver::fly_along_sight(
        bullet,
        conditions,
        muzzle_velocity,
        |sample| {
            let along = (sample.range - last_range) / (sample.time - last_time);
            (last_range, last_time) = (sample.range, sample.time);
            sample.range < max_range && along >= STEEPEST_FALL_ALONG_SHARE * sample.velocity
        },
    ))
}

fn validate(pellet: &Pellet, muzzle_velocity: f64) -> Result<(), SolveError> {
    if ![pellet.diameter_m, pellet.density_kg_m3, muzzle_velocity]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err(SolveError::InvalidInput(
            "a number is not a number (infinite or undefined)",
        ));
    }
    solver::first_failed(&[
        (
            (0.000_5..=0.025).contains(&pellet.diameter_m),
            "pellet diameter must be between 0.5 mm and 25 mm",
        ),
        (
            (1_000.0..=25_000.0).contains(&pellet.density_kg_m3),
            "pellet density must be between 1000 and 25 000 kg/m³",
        ),
        (
            muzzle_velocity > 0.0 && muzzle_velocity <= 1_000.0,
            "pellet muzzle velocity must be above 0 and at most 1000 m/s",
        ),
    ])
}

/// One standard deviation, per axis, of the angle (radians) of a round
/// normal pattern that puts `pattern_percent` of its pellets in the 30-inch
/// circle at 40 yards. Refuses a percentage outside 0–100, ends excluded.
pub fn pattern_sigma(pattern_percent: f64) -> Result<f64, SolveError> {
    if !(pattern_percent > 0.0 && pattern_percent < 100.0) {
        return Err(SolveError::InvalidInput(
            "pattern percentage must be above 0 and below 100",
        ));
    }
    let inside = pattern_percent / 100.0;
    let circle = (PATTERN_RADIUS_M / PATTERN_DISTANCE_M).atan();
    Ok(circle / (-2.0 * (-inside).ln_1p()).sqrt())
}

/// `count` pellets' angles off the bore, [right, up] in radians, for a choke
/// that puts `pattern_percent` in the 30-inch circle at 40 yards. Real
/// patterns are flatter in the middle than this normal spread. The same seed
/// gives the same pattern, also in later versions. Refuses what
/// `pattern_sigma` refuses and more than `MAX_PELLETS` pellets.
pub fn pattern(pattern_percent: f64, count: usize, seed: u64) -> Result<Vec<[f64; 2]>, SolveError> {
    let sigma = pattern_sigma(pattern_percent)?;
    if count > MAX_PELLETS {
        return Err(SolveError::InvalidInput("too many pellets in the pattern"));
    }
    let mut random = SplitMix64(seed);
    Ok((0..count)
        .map(|_| {
            let (right, up) = random.normal_pair();
            [right * sigma, up * sigma]
        })
        .collect())
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1].
    fn unit(&mut self) -> f64 {
        ((self.next() >> 11) + 1) as f64 / (1u64 << 53) as f64
    }

    /// Two independent standard normals (Box–Muller).
    fn normal_pair(&mut self) -> (f64, f64) {
        let radius = (-2.0 * self.unit().ln()).sqrt();
        let angle = 2.0 * PI * self.unit();
        (radius * angle.cos(), radius * angle.sin())
    }
}

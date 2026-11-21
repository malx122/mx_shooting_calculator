//! Shotgun pellets: how many a load holds, one pellet's flight with a
//! sphere's drag, how a choke's pattern spreads with range, how many
//! pellets reach the vitals, and how far shot can carry.

use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use crate::atmosphere::Atmosphere;
use crate::drag::{DragCurve, DragModel};
use crate::solver::{self, BcBands, Bullet, Conditions, Sample, SolveError, Wind};

mod hits;
mod sizes;
mod spread;
pub use hits::*;
pub use sizes::*;
pub use spread::*;

/// Density of pure lead. Hardened shot (2–4 % antimony) is about 11 100.
pub const LEAD_DENSITY_KG_M3: f64 = 11_340.0;
/// Density of hardened lead shot, 2–4 % antimony: Lowry's figure for
/// buffered lead, between SAAMI's pellet counts at 2 and 4 %.
pub const HARD_LEAD_DENSITY_KG_M3: f64 = 11_100.0;
/// Density of steel (soft iron) shot.
pub const STEEL_DENSITY_KG_M3: f64 = 7_860.0;
/// Density of bismuth shot (3 % tin).
pub const BISMUTH_DENSITY_KG_M3: f64 = 9_600.0;
/// Density of tungsten–nickel–iron shot such as HEVI-Shot.
pub const TUNGSTEN_IRON_DENSITY_KG_M3: f64 = 12_000.0;
/// Density of tungsten-polymer shot such as Kent's Tungsten-Matrix.
pub const TUNGSTEN_MATRIX_DENSITY_KG_M3: f64 = 10_800.0;
/// Density of tungsten super shot (TSS).
pub const TSS_DENSITY_KG_M3: f64 = 18_000.0;
/// Density of zinc shot.
pub const ZINC_DENSITY_KG_M3: f64 = 7_140.0;

/// What shot is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    /// Hardened lead, as game shot is made.
    Lead,
    /// Soft iron, called steel.
    Steel,
    /// Bismuth with about 3 % tin.
    Bismuth,
    /// Tungsten, nickel and iron, such as HEVI-Shot.
    TungstenIron,
    /// Tungsten powder in a polymer, such as Kent's Tungsten-Matrix.
    TungstenMatrix,
    /// Tungsten super shot, nearly pure tungsten alloy.
    Tss,
    Zinc,
}

impl Material {
    pub const ALL: [Material; 7] = [
        Material::Lead,
        Material::Steel,
        Material::Bismuth,
        Material::TungstenIron,
        Material::TungstenMatrix,
        Material::Tss,
        Material::Zinc,
    ];

    pub fn density_kg_m3(self) -> f64 {
        match self {
            Material::Lead => HARD_LEAD_DENSITY_KG_M3,
            Material::Steel => STEEL_DENSITY_KG_M3,
            Material::Bismuth => BISMUTH_DENSITY_KG_M3,
            Material::TungstenIron => TUNGSTEN_IRON_DENSITY_KG_M3,
            Material::TungstenMatrix => TUNGSTEN_MATRIX_DENSITY_KG_M3,
            Material::Tss => TSS_DENSITY_KG_M3,
            Material::Zinc => ZINC_DENSITY_KG_M3,
        }
    }

    /// Whether the shot contains lead, which the EU bans in and within 100 m
    /// of wetlands (Regulation 2021/57, since 2023) and for all hunting from
    /// 7 December 2033 (Regulation 2026/2530); slugs are not affected.
    pub fn is_lead(self) -> bool {
        self == Material::Lead
    }
}

/// Distance a choke's pattern percentage is measured at: 40 yards.
pub const PATTERN_DISTANCE_M: f64 = 36.576;
/// Radius of the circle a choke's pattern percentage is counted in: 30 inches across.
pub const PATTERN_RADIUS_M: f64 = 0.381;

/// The most pellets a load may hold or a pattern may have.
pub const MAX_PELLETS: usize = 100_000;

/// Drag coefficient of a smooth sphere by Mach number: the correlation of
/// Loth, Daspit, Jeong, Nagata and Nonomura, "Supersonic and hypersonic
/// drag coefficients for a sphere", AIAA Journal 59(8), 2021, fitted to the
/// ballistic-range sphere measurements, at a Reynolds number of 5·10⁴.
/// Pellets fly at about 2·10⁴–1.2·10⁵, below the drag crisis; there the
/// Reynolds number changes the velocity at 35–45 m by under 1 %. It matches
/// Lowry's pellet table (American Rifleman, 1989) within 5 % at 40 yards
/// and 6 % at 50 for steel BB–4 and lead 2–6, and Eley's striking
/// velocities within 2.5 % at 40 m for English BB–7. Fired lead is dented
/// and drags 0–8 % more than a sphere; tables for shot under 2.3 mm
/// disagree with each other by ±10 %.
const SPHERE_DRAG: [(f64, f64); 20] = [
    (0.0, 0.462),
    (0.3, 0.472),
    (0.5, 0.494),
    (0.6, 0.516),
    (0.7, 0.543),
    (0.8, 0.599),
    (0.85, 0.663),
    (0.9, 0.726),
    (0.95, 0.787),
    (1.0, 0.840),
    (1.05, 0.882),
    (1.1, 0.916),
    (1.2, 0.964),
    (1.3, 0.993),
    (1.4, 1.010),
    (1.5, 1.021),
    (1.75, 1.029),
    (2.0, 1.031),
    (2.5, 1.023),
    (3.0, 1.004),
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
    /// A pellet of `diameter_m` made of `material`.
    pub fn of(material: Material, diameter_m: f64) -> Self {
        Self {
            diameter_m,
            density_kg_m3: material.density_kg_m3(),
        }
    }

    /// One pellet's energy at `velocity`, joules.
    pub fn energy_j(&self, velocity: f64) -> f64 {
        0.5 * self.mass_kg() * velocity * velocity
    }

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
    let (mut last_range, mut last_time) = (0.0, 0.0);
    Ok(solver::fly_along_sight(
        bullet(pellet),
        conditions,
        muzzle_velocity,
        solver::Stepping::FINE,
        |sample| {
            let along = (sample.range - last_range) / (sample.time - last_time);
            (last_range, last_time) = (sample.range, sample.time);
            sample.range < max_range && along >= STEEPEST_FALL_ALONG_SHARE * sample.velocity
        },
    ))
}

/// Where a published shot velocity is measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VelocityAt {
    /// At the muzzle ("V0").
    Muzzle,
    /// At this distance from the muzzle: C.I.P. makers print V2.5 (2.5 m),
    /// American ones the velocity at 3 feet (0.914 m). Shot loses 2–4 % of
    /// its speed a metre at first, so V2.5 is about 20–35 m/s below the
    /// muzzle velocity.
    Distance(f64),
    /// The average over this distance, as British makers' "observed
    /// velocity" over the first 20 yards (18.288 m): about 80 m/s below the
    /// muzzle velocity.
    AverageOver(f64),
}

/// The muzzle velocity that gives `velocity` measured `at`, for this pellet
/// in these conditions. A coil chronograph reads the middle of the shot
/// column, about 6–10 % slower than the front pellets that lead the pattern
/// (Lowry).
pub fn muzzle_velocity(
    pellet: &Pellet,
    velocity: f64,
    at: VelocityAt,
    conditions: &Conditions,
) -> Result<f64, SolveError> {
    validate(pellet, velocity)?;
    solver::validate_conditions(conditions)?;
    let distance = match at {
        VelocityAt::Muzzle => return Ok(velocity),
        VelocityAt::Distance(distance) | VelocityAt::AverageOver(distance) => distance,
    };
    if !(distance.is_finite() && distance > 0.0 && distance <= 50.0) {
        return Err(SolveError::InvalidInput(
            "a velocity must be measured between 0 and 50 m from the muzzle",
        ));
    }
    let measured = |muzzle: f64| -> Option<f64> {
        let path = fly(pellet, muzzle, conditions, distance).ok()?;
        let there = solver::interpolate(&path, distance)?;
        Some(match at {
            VelocityAt::AverageOver(_) => distance / there.time,
            _ => there.velocity,
        })
    };
    // A tailwind can carry the shot faster than it left the muzzle, so the
    // search starts from almost nothing.
    let (mut low, mut high) = (0.1, 1_000.0);
    if !measured(high).is_some_and(|fastest| fastest >= velocity)
        || measured(low).is_some_and(|slowest| slowest > velocity)
    {
        return Err(SolveError::InvalidInput(
            "no muzzle velocity up to 1000 m/s gives that velocity there",
        ));
    }
    for _ in 0..60 {
        let middle = (low + high) / 2.0;
        if measured(middle).is_some_and(|got| got >= velocity) {
            high = middle;
        } else {
            low = middle;
        }
    }
    Ok((low + high) / 2.0)
}

/// How far shot can carry: the danger range behind the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaxRange {
    /// Along the ground, back to the height of the muzzle, m.
    pub distance_m: f64,
    /// The elevation that carries farthest, degrees.
    pub elevation_degrees: f64,
    /// The time of flight, s.
    pub time_s: f64,
    /// The pellet's speed where it comes down, m/s.
    pub velocity: f64,
}

/// The longest a pellet is flown to find where it comes down.
const MAX_RANGE_FLIGHT: solver::Stepping = solver::Stepping {
    time_step_s: 0.002,
    max_time_s: 300.0,
};

/// The farthest a pellet carries over level ground, at the elevation that
/// carries it farthest (about 23–28° for shot in still air), in the
/// conditions' air and wind; their shot angle doesn't apply. Journée's old
/// rule, 2200 yards per inch of pellet (79 m a millimetre), agrees within
/// 7 % for lead 2–4 mm; it overstates steel and buckshot. A tailwind
/// carries shot farther and higher. Refuses a pellet that doesn't come
/// down within 300 s.
pub fn max_range(
    pellet: &Pellet,
    muzzle_velocity: f64,
    conditions: &Conditions,
) -> Result<MaxRange, SolveError> {
    validate(pellet, muzzle_velocity)?;
    solver::validate_conditions(conditions)?;
    let at = |elevation_degrees: f64| -> Option<MaxRange> {
        let raised = Conditions {
            inclination_degrees: elevation_degrees,
            cant_degrees: 0.0,
            ..*conditions
        };
        let (sin, cos) = elevation_degrees.to_radians().sin_cos();
        let height = |sample: &Sample| sample.range * sin + sample.drop * cos;
        let mut path = solver::fly_along_sight(
            bullet(pellet),
            &raised,
            muzzle_velocity,
            MAX_RANGE_FLIGHT,
            |sample| height(sample) >= 0.0,
        );
        let last = path.pop().filter(|last| height(last) < 0.0)?;
        let before = path.last()?;
        let share = height(before) / (height(before) - height(&last));
        // Measured from the shooter, behind them (blown back) negative.
        let ground = |sample: &Sample| {
            let along = sample.range * cos - sample.drop * sin;
            along.hypot(sample.windage).copysign(along)
        };
        let lerp = |from: f64, to: f64| from + (to - from) * share;
        Some(MaxRange {
            distance_m: lerp(ground(before), ground(&last)),
            elevation_degrees,
            time_s: lerp(before.time, last.time),
            velocity: lerp(before.velocity, last.velocity),
        })
    };
    const NOT_DOWN: SolveError =
        SolveError::InvalidInput("the pellet doesn't come down within 300 s");
    let farther = |a: &MaxRange, b: &MaxRange| a.distance_m.total_cmp(&b.distance_m);
    // The distance has one peak in elevation: find it roughly, then by
    // golden section, which keeps one of its two points each step.
    let mut best: Option<MaxRange> = None;
    for step in 1..=30 {
        let tried = at(step as f64 * 2.5).ok_or(NOT_DOWN)?;
        if best.is_none_or(|best| farther(&tried, &best).is_gt()) {
            best = Some(tried);
        }
    }
    let best = best.ok_or(NOT_DOWN)?;
    let (mut low, mut high) = (
        (best.elevation_degrees - 2.5).max(0.5),
        (best.elevation_degrees + 2.5).min(80.0),
    );
    let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
    let mut a = at(high - ratio * (high - low)).ok_or(NOT_DOWN)?;
    let mut b = at(low + ratio * (high - low)).ok_or(NOT_DOWN)?;
    for _ in 0..14 {
        if farther(&a, &b).is_lt() {
            low = a.elevation_degrees;
            a = b;
            b = at(low + ratio * (high - low)).ok_or(NOT_DOWN)?;
        } else {
            high = b.elevation_degrees;
            b = a;
            a = at(high - ratio * (high - low)).ok_or(NOT_DOWN)?;
        }
    }
    Ok(if farther(&a, &b).is_lt() { b } else { a })
}

fn bullet(pellet: &Pellet) -> Bullet {
    Bullet {
        mass_kg: pellet.mass_kg(),
        diameter_m: pellet.diameter_m,
        length_m: None,
        ballistic_coefficient: 0.0,
        drag_model: DragModel::G1,
        bc_bands: BcBands::NONE,
        drag_curve: Some(sphere_drag()),
    }
}

/// Calm, level conditions in this air, for a shot's numbers that don't
/// depend on the wind.
pub fn calm(atmosphere: Atmosphere) -> Conditions {
    Conditions {
        atmosphere,
        powder_temperature_c: atmosphere.temperature_c,
        wind: Wind::CALM,
        inclination_degrees: 0.0,
        coriolis: None,
        cant_degrees: 0.0,
    }
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
    spread::check_percent(pattern_percent)?;
    let inside = pattern_percent / 100.0;
    let circle = (PATTERN_RADIUS_M / PATTERN_DISTANCE_M).atan();
    Ok(circle / (-2.0 * (-inside).ln_1p()).sqrt())
}

/// `count` pellets' angles off the bore, [right, up] in radians, for a choke
/// that puts `pattern_percent` in the 30-inch circle at 40 yards, at a
/// constant angle: right at 40 yards, but too wide nearer and too narrow
/// farther for a choked barrel (`pattern_at` has the spread at each range).
/// The same seed gives the same pattern, also in later versions. Refuses
/// what `pattern_sigma` refuses and more than `MAX_PELLETS` pellets.
pub fn pattern(pattern_percent: f64, count: usize, seed: u64) -> Result<Vec<[f64; 2]>, SolveError> {
    normal_pairs(count, seed, pattern_sigma(pattern_percent)?)
}

/// `count` pairs of independent normals of standard deviation `sigma`.
fn normal_pairs(count: usize, seed: u64, sigma: f64) -> Result<Vec<[f64; 2]>, SolveError> {
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

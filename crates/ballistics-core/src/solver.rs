//! Point-mass (3-DOF) trajectory solver.
//!
//! Frame: the muzzle is the origin, `x` points horizontally down range, `y`
//! up and `z` to the right. Drag acts against the bullet's velocity relative
//! to the moving air, scaled by the standard drag table and the BC. Results
//! are reported against the line of sight through the scope, the way a
//! hunter reads them: slant range from a rangefinder, and drop and windage
//! perpendicular to that line.

use std::fmt;
use std::ops::{Add, Mul, Sub};
use std::sync::Arc;

use super::atmosphere::Atmosphere;
use super::drag::{DragCurve, DragModel};
use super::stability;

pub const GRAVITY: f64 = 9.806_65;
const EARTH_ROTATION_RAD_PER_S: f64 = 7.292_115e-5;
/// BC in lb/in² (the number printed on ammunition boxes) to kg/m².
const BC_LB_PER_IN2_TO_KG_PER_M2: f64 = 703.069_58;
const TIME_STEP_S: f64 = 0.000_5;
const MPH_PER_MPS: f64 = 2.236_936;
const RAD_PER_MOA: f64 = std::f64::consts::PI / (180.0 * 60.0);
const MAX_FLIGHT_TIME_S: f64 = 15.0;
const MIN_SPEED: f64 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Bullet {
    pub mass_kg: f64,
    pub diameter_m: f64,
    /// Needed for stability and spin drift; often unpublished.
    pub length_m: Option<f64>,
    /// Ballistic coefficient in lb/in² against `drag_model`.
    pub ballistic_coefficient: f64,
    pub drag_model: DragModel,
    /// BCs for velocity bands; when present they replace
    /// `ballistic_coefficient`.
    pub bc_bands: BcBands,
    /// The bullet's own drag curve. When present it replaces the standard
    /// drag model and every BC.
    pub drag_curve: Option<Arc<DragCurve>>,
}

impl Bullet {
    /// The BC at a speed through the air, in lb/in².
    pub fn bc_at(&self, speed: f64) -> f64 {
        self.bc_bands
            .at(speed)
            .unwrap_or(self.ballistic_coefficient)
    }

    /// Ballistic coefficient in kg/m² at a speed through the air. With the
    /// bullet's own drag curve there is no reference projectile, so it is
    /// just mass over diameter squared.
    fn bc_si_at(&self, speed: f64) -> f64 {
        match self.drag_curve {
            Some(_) => self.mass_kg / (self.diameter_m * self.diameter_m),
            None => self.bc_at(speed) * BC_LB_PER_IN2_TO_KG_PER_M2,
        }
    }

    fn drag_coefficient(&self, mach: f64) -> f64 {
        match &self.drag_curve {
            Some(curve) => curve.drag_coefficient(mach),
            None => self.drag_model.drag_coefficient(mach),
        }
    }
}

pub const MAX_BC_BANDS: usize = 5;

/// A BC that changes with velocity, as some makers publish it: points of
/// (velocity in m/s, BC), interpolated linearly and held constant beyond the
/// first and last point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BcBands {
    points: [(f64, f64); MAX_BC_BANDS],
    len: usize,
}

impl BcBands {
    pub const NONE: BcBands = BcBands {
        points: [(0.0, 0.0); MAX_BC_BANDS],
        len: 0,
    };

    pub fn new(points: &[(f64, f64)]) -> Result<BcBands, &'static str> {
        if points.len() > MAX_BC_BANDS {
            return Err("at most five BC bands");
        }
        if points
            .iter()
            .any(|&(velocity, bc)| !(velocity > 0.0 && bc > 0.0))
        {
            return Err("BC bands need a positive velocity and BC");
        }
        let mut bands = BcBands::NONE;
        bands.points[..points.len()].copy_from_slice(points);
        bands.len = points.len();
        bands.points[..bands.len].sort_by(|a, b| a.0.total_cmp(&b.0));
        if bands.points[..bands.len]
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err("two BC bands have the same velocity");
        }
        Ok(bands)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn at(&self, speed: f64) -> Option<f64> {
        let points = &self.points[..self.len];
        let (first, last) = (points.first()?, points.last()?);
        if speed <= first.0 {
            return Some(first.1);
        }
        if speed >= last.0 {
            return Some(last.1);
        }
        let upper = points.partition_point(|&(velocity, _)| velocity < speed);
        let ((v0, bc0), (v1, bc1)) = (points[upper - 1], points[upper]);
        Some(bc0 + (bc1 - bc0) * (speed - v0) / (v1 - v0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rifle {
    /// Centre of the scope above the centre of the bore.
    pub sight_height_m: f64,
    pub zero_range_m: f64,
    /// Where the rifle is zeroed to hit at the zero range: above (positive)
    /// or below the line of sight. Zero for a plain zero.
    pub zero_height_m: f64,
    /// Barrel length for one full turn of the rifling.
    pub twist_m: f64,
    pub right_hand_twist: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    /// Muzzle velocity measured (or published) at `reference_temperature_c`.
    pub muzzle_velocity: f64,
    pub reference_temperature_c: f64,
    /// Change in muzzle velocity per °C of powder temperature, in m/s.
    pub temperature_sensitivity: f64,
}

impl Load {
    pub fn velocity_at(&self, powder_temperature_c: f64) -> f64 {
        self.muzzle_velocity
            + self.temperature_sensitivity * (powder_temperature_c - self.reference_temperature_c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub speed: f64,
    /// Direction the wind blows *from*, clockwise from the line of fire:
    /// 0° from the target (headwind), 90° from the right, 180° from behind.
    pub from_degrees: f64,
}

impl Wind {
    pub const CALM: Wind = Wind {
        speed: 0.0,
        from_degrees: 0.0,
    };

    fn velocity(&self) -> Vec3 {
        let angle = self.from_degrees.to_radians();
        Vec3::new(-angle.cos(), 0.0, -angle.sin()) * self.speed
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coriolis {
    pub latitude_degrees: f64,
    /// Compass direction of the shot: 0° north, 90° east.
    pub azimuth_degrees: f64,
}

/// Where and in what weather the shot is taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    pub atmosphere: Atmosphere,
    pub powder_temperature_c: f64,
    pub wind: Wind,
    /// Angle of the line of sight: positive uphill, negative downhill.
    pub inclination_degrees: f64,
    pub coriolis: Option<Coriolis>,
    /// The rifle tilted about the line of sight, as the shooter sees it:
    /// positive with the top of the scope to the right.
    pub cant_degrees: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub bullet: Bullet,
    pub rifle: Rifle,
    pub load: Load,
    /// Weather on the day the rifle was zeroed. The zero is taken level and
    /// in calm air.
    pub zero_atmosphere: Atmosphere,
    pub zero_powder_temperature_c: f64,
    /// Where this load hits relative to the rifle's zero, as an angle in
    /// radians: [right, up]. Used when the rifle was zeroed with another load.
    pub impact_offset: [f64; 2],
    pub shot: Conditions,
    pub spin_drift: bool,
    /// The small up or down jump a crosswind gives a spinning bullet as it
    /// leaves the muzzle.
    pub aerodynamic_jump: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SolveError {
    InvalidInput(&'static str),
    ZeroOutOfReach,
}

impl fmt::Display for SolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::InvalidInput(what) => write!(formatter, "invalid input: {what}"),
            SolveError::ZeroOutOfReach => {
                write!(formatter, "the bullet cannot reach the zero range")
            }
        }
    }
}

impl std::error::Error for SolveError {}

/// One integration step, relative to the line of sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub time: f64,
    /// Distance along the line of sight (what a rangefinder reads).
    pub range: f64,
    /// Height above the line of sight; negative is below the crosshair.
    pub drop: f64,
    /// Sideways offset including spin drift; positive is right.
    pub windage: f64,
    pub velocity: f64,
    pub mach: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeRow {
    pub range: f64,
    pub time: f64,
    pub drop: f64,
    pub windage: f64,
    /// Angle to dial or hold up, in radians (negative means hold low).
    pub elevation_correction: f64,
    /// Angle to dial or hold right, in radians (negative means hold left).
    pub windage_correction: f64,
    pub velocity: f64,
    pub mach: f64,
    pub energy_j: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    /// Muzzle velocity after powder-temperature correction.
    pub muzzle_velocity: f64,
    /// Miller stability factor at the muzzle in the shot conditions, when
    /// the bullet length is known.
    pub stability: Option<f64>,
    /// Bore elevation and windage relative to the line of sight that the
    /// zero produces, in radians.
    pub zero_elevation: f64,
    pub zero_windage: f64,
    pub samples: Vec<Sample>,
    pub mass_kg: f64,
    /// The cant, radians (positive: scope's top to the right).
    pub cant: f64,
    /// The scope's height above the bore.
    pub sight_height: f64,
    /// The bore's angles to the line of sight as the bullet leaves:
    /// [right, up], the zero's plus the load's impact offset.
    pub bore: [f64; 2],
}

impl Solution {
    /// Trajectory values at a slant range, or `None` beyond the computed path.
    /// With a cant, the drop and windage are where a dead-on hold lands and
    /// the corrections are what to dial or hold in the tilted reticle.
    pub fn at(&self, range: f64) -> Option<RangeRow> {
        let sample = interpolate(&self.samples, range)?;
        let (drop, windage, elevation_correction, windage_correction) = if self.cant == 0.0 {
            (
                sample.drop,
                sample.windage,
                (-sample.drop).atan2(range),
                (-sample.windage).atan2(range),
            )
        } else {
            self.canted(&sample)
        };
        Some(RangeRow {
            range,
            time: sample.time,
            drop,
            windage,
            elevation_correction,
            windage_correction,
            velocity: sample.velocity,
            mach: sample.mach,
            energy_j: kinetic_energy(self.mass_kg, sample.velocity),
        })
    }

    /// The canted shot. The bore's own geometry (its height under the
    /// scope and its angles to the line of sight) turns with the rifle,
    /// gravity and wind don't. In the rifle's frame the bore puts the
    /// bullet at `bore` = (r·right, r·up − sight height) before gravity and
    /// wind move it by `pushed`; canted, `bore` turns by the cant. The
    /// correction is the extra angle, in the rifle's own frame, that brings
    /// the turned bore and the push back onto the line of sight.
    fn canted(&self, sample: &Sample) -> (f64, f64, f64, f64) {
        let r = sample.range.max(1e-6);
        let (sin, cos) = self.cant.sin_cos();
        let [right, up] = self.bore;
        let bore = (r * right, r * up - self.sight_height);
        let pushed = (sample.windage - bore.0, sample.drop - bore.1);
        // Turning the rifle clockwise (as the shooter sees it) by the cant
        // takes its (right, up) to (x·cos + y·sin, −x·sin + y·cos).
        let windage = bore.0 * cos + bore.1 * sin + pushed.0;
        let drop = -bore.0 * sin + bore.1 * cos + pushed.1;
        // What the rifle's frame must hold: the push turned back, reversed.
        let need = (
            -(pushed.0 * cos - pushed.1 * sin),
            -(pushed.0 * sin + pushed.1 * cos),
        );
        let windage_correction = need.0 / r - right;
        let elevation_correction = (need.1 + self.sight_height) / r - up;
        (drop, windage, elevation_correction, windage_correction)
    }

    /// Where a canted dead-on hold lands compared with a level one at a
    /// range: (right, up), metres. Zero without a cant.
    pub fn cant_shift(&self, range: f64) -> Option<(f64, f64)> {
        let sample = interpolate(&self.samples, range)?;
        if self.cant == 0.0 {
            return Some((0.0, 0.0));
        }
        let (drop, windage, _, _) = self.canted(&sample);
        Some((windage - sample.windage, drop - sample.drop))
    }
}

pub fn kinetic_energy(mass_kg: f64, velocity: f64) -> f64 {
    0.5 * mass_kg * velocity * velocity
}

/// Zeroes the rifle, then flies the shot out to `max_range` (slant).
pub fn solve(scenario: &Scenario, max_range: f64) -> Result<Solution, SolveError> {
    validate(scenario)?;
    let zero = zero_angles(scenario)?;
    Ok(fly_from_zero(scenario, max_range, zero))
}

/// Like `solve`, with the sights set at `zero` (elevation, windage in
/// radians, as a `Solution` reports them) instead of zeroing the rifle
/// again: for a shot that differs from the expected one (another muzzle
/// velocity, another wind) with the same sight setting.
pub fn solve_with_zero(
    scenario: &Scenario,
    max_range: f64,
    zero: (f64, f64),
) -> Result<Solution, SolveError> {
    validate(scenario)?;
    Ok(fly_from_zero(scenario, max_range, zero))
}

fn fly_from_zero(scenario: &Scenario, max_range: f64, zero: (f64, f64)) -> Solution {
    let (zero_elevation, zero_windage) = zero;
    let shot = &scenario.shot;
    let muzzle_velocity = scenario.load.velocity_at(shot.powder_temperature_c);
    let stability = muzzle_stability(scenario, muzzle_velocity, &shot.atmosphere);
    let flight = Flight::new(scenario, shot, stability);
    let [offset_right, offset_up] = scenario.impact_offset;
    let launch = launch_velocity(
        muzzle_velocity,
        shot.inclination_degrees.to_radians() + zero_elevation + offset_up,
        zero_windage + offset_right,
    );
    let mut samples = flight.fly(launch, |sample| sample.range < max_range);
    if scenario.aerodynamic_jump
        && let Some(jump) = aerodynamic_jump(scenario, stability)
    {
        for sample in &mut samples {
            sample.drop += jump * sample.range;
        }
    }
    Solution {
        muzzle_velocity,
        stability,
        zero_elevation,
        zero_windage,
        samples,
        mass_kg: scenario.bullet.mass_kg,
        cant: shot.cant_degrees.to_radians(),
        sight_height: scenario.rifle.sight_height_m,
        bore: [zero_windage + offset_right, zero_elevation + offset_up],
    }
}

/// The vertical angle (radians, up positive) a crosswind gives the bullet
/// at the muzzle: Litz's fit, (0.01·Sg − 0.0024·L + 0.032) MOA per mph of
/// crosswind, L the bullet's length in calibres. With a right-hand twist,
/// wind from the left throws it up, from the right down. Needs the
/// stability, so the bullet's length.
pub fn aerodynamic_jump(scenario: &Scenario, stability: Option<f64>) -> Option<f64> {
    let sg = stability?;
    let bullet = &scenario.bullet;
    let calibres = bullet.length_m? / bullet.diameter_m;
    let wind = scenario.shot.wind;
    // Wind from the right (90°) is positive.
    let from_right_mph = wind.speed * wind.from_degrees.to_radians().sin() * MPH_PER_MPS;
    let moa_per_mph = 0.01 * sg - 0.0024 * calibres + 0.032;
    let twist = if scenario.rifle.right_hand_twist {
        1.0
    } else {
        -1.0
    };
    Some(-twist * moa_per_mph * from_right_mph * RAD_PER_MOA)
}

/// Flies a level, calm-air shot in the shot's atmosphere with the bore
/// raised `elevation` radians above the line of sight. Used by analyses that
/// pick their own zero, such as maximum point-blank range.
pub fn fly_level(
    scenario: &Scenario,
    elevation: f64,
    keep_going: impl FnMut(&Sample) -> bool,
) -> Vec<Sample> {
    let conditions = Conditions {
        wind: Wind::CALM,
        inclination_degrees: 0.0,
        coriolis: None,
        cant_degrees: 0.0,
        ..scenario.shot
    };
    let velocity = scenario.load.velocity_at(conditions.powder_temperature_c);
    let stability = muzzle_stability(scenario, velocity, &conditions.atmosphere);
    Flight::new(scenario, &conditions, stability)
        .fly(launch_velocity(velocity, elevation, 0.0), keep_going)
}

/// Linear interpolation of a sample path at a slant range.
pub fn interpolate(samples: &[Sample], range: f64) -> Option<Sample> {
    let after = samples.partition_point(|sample| sample.range < range);
    if after == 0 {
        return (samples.first()?.range == range).then(|| samples[0]);
    }
    if after == samples.len() {
        return None;
    }
    let (a, b) = (samples[after - 1], samples[after]);
    let t = (range - a.range) / (b.range - a.range);
    let lerp = |from: f64, to: f64| from + (to - from) * t;
    Some(Sample {
        time: lerp(a.time, b.time),
        range,
        drop: lerp(a.drop, b.drop),
        windage: lerp(a.windage, b.windage),
        velocity: lerp(a.velocity, b.velocity),
        mach: lerp(a.mach, b.mach),
    })
}

fn validate(scenario: &Scenario) -> Result<(), SolveError> {
    let Scenario {
        bullet,
        rifle,
        load,
        zero_atmosphere,
        shot,
        ..
    } = scenario;
    let muzzle = load.velocity_at(shot.powder_temperature_c);
    let zero_muzzle = load.velocity_at(scenario.zero_powder_temperature_c);
    // Every number finite and within what a rifle bullet can be: beyond
    // that the integration breaks down rather than giving an answer.
    let numbers = [
        bullet.mass_kg,
        bullet.diameter_m,
        bullet.length_m.unwrap_or(0.01),
        bullet.ballistic_coefficient,
        rifle.sight_height_m,
        rifle.zero_range_m,
        rifle.zero_height_m,
        rifle.twist_m,
        muzzle,
        zero_muzzle,
        shot.inclination_degrees,
        shot.cant_degrees,
        shot.wind.speed,
        shot.wind.from_degrees,
        shot.powder_temperature_c,
        scenario.zero_powder_temperature_c,
    ];
    if numbers.iter().any(|value| !value.is_finite()) {
        return Err(SolveError::InvalidInput(
            "a number is not a number (infinite or undefined)",
        ));
    }
    let checks = [
        (
            (0.000_1..=0.2).contains(&bullet.mass_kg),
            "bullet mass must be between 0.1 g and 200 g",
        ),
        (
            (0.001..=0.04).contains(&bullet.diameter_m),
            "bullet diameter must be between 1 mm and 40 mm",
        ),
        (
            bullet.ballistic_coefficient <= 3.0,
            "ballistic coefficient is implausibly high",
        ),
        (
            muzzle < 2500.0 && zero_muzzle < 2500.0,
            "muzzle velocity must be under 2500 m/s",
        ),
        (
            rifle.zero_range_m <= 3000.0,
            "zero range is implausibly long",
        ),
        (
            rifle.zero_height_m.abs() <= 1.0,
            "zero height is implausibly large",
        ),
        (
            bullet.length_m.is_none_or(|length| length > 0.0),
            "bullet length must be positive",
        ),
        (
            bullet.drag_curve.is_some() || bullet.ballistic_coefficient > 0.0,
            "ballistic coefficient must be positive",
        ),
        (
            rifle.sight_height_m >= 0.0,
            "sight height cannot be negative",
        ),
        (rifle.zero_range_m > 0.0, "zero range must be positive"),
        (rifle.twist_m > 0.0, "twist rate must be positive"),
        (
            load.velocity_at(shot.powder_temperature_c) > 0.0,
            "muzzle velocity must be positive",
        ),
        (
            load.velocity_at(scenario.zero_powder_temperature_c) > 0.0,
            "muzzle velocity must be positive",
        ),
        (
            scenario
                .impact_offset
                .iter()
                .all(|angle| angle.abs() < 0.05),
            "impact offset is implausibly large",
        ),
        (
            shot.inclination_degrees.abs() < 90.0,
            "shot angle must be between -90° and 90°",
        ),
        (shot.wind.speed >= 0.0, "wind speed cannot be negative"),
    ];
    for atmosphere in [zero_atmosphere, &shot.atmosphere] {
        if atmosphere.pressure_hpa <= 0.0 || atmosphere.temperature_c <= -273.15 {
            return Err(SolveError::InvalidInput("atmosphere is not physical"));
        }
    }
    match checks.iter().find(|(ok, _)| !ok) {
        Some((_, message)) => Err(SolveError::InvalidInput(message)),
        None => Ok(()),
    }
}

fn muzzle_stability(scenario: &Scenario, velocity: f64, atmosphere: &Atmosphere) -> Option<f64> {
    let length = scenario.bullet.length_m?;
    Some(stability::miller_stability(
        scenario.bullet.mass_kg,
        scenario.bullet.diameter_m,
        length,
        scenario.rifle.twist_m,
        velocity,
        atmosphere,
    ))
}

/// The zero day's flight: its weather, level and calm, and the muzzle
/// velocity it gives.
fn zero_day(scenario: &Scenario) -> (Flight, f64) {
    let conditions = Conditions {
        atmosphere: scenario.zero_atmosphere,
        powder_temperature_c: scenario.zero_powder_temperature_c,
        wind: Wind::CALM,
        inclination_degrees: 0.0,
        coriolis: None,
        cant_degrees: 0.0,
    };
    let velocity = scenario.load.velocity_at(conditions.powder_temperature_c);
    let stability = muzzle_stability(scenario, velocity, &conditions.atmosphere);
    (Flight::new(scenario, &conditions, stability), velocity)
}

/// The path the zero puts this load on (with its impact offset), as on
/// the zero day (level and in calm air), until `keep_going` rejects a
/// sample.
pub fn zero_day_path(
    scenario: &Scenario,
    keep_going: impl FnMut(&Sample) -> bool,
) -> Result<Vec<Sample>, SolveError> {
    validate(scenario)?;
    let (elevation, _) = zero_angles(scenario)?;
    let (flight, velocity) = zero_day(scenario);
    let launch = launch_velocity(velocity, elevation + scenario.impact_offset[1], 0.0);
    Ok(flight.fly(launch, keep_going))
}

/// Finds the bore angles that put the bullet at the zero height (on the
/// crosshair for a plain zero) at the zero range, in the zero-day weather.
fn zero_angles(scenario: &Scenario) -> Result<(f64, f64), SolveError> {
    let (flight, velocity) = zero_day(scenario);
    let zero_range = scenario.rifle.zero_range_m;
    let height = scenario.rifle.zero_height_m;
    let at_zero = |elevation: f64| {
        let samples = flight.fly(launch_velocity(velocity, elevation, 0.0), |s| {
            s.range < zero_range
        });
        interpolate(&samples, zero_range).ok_or(SolveError::ZeroOutOfReach)
    };

    // Drop at the zero range is almost linear in the bore angle, so the
    // secant method converges in a few shots.
    let (mut previous_angle, mut angle) = (0.0, 0.002);
    let mut previous_drop = at_zero(previous_angle)?.drop - height;
    for _ in 0..30 {
        let mut sample = at_zero(angle)?;
        sample.drop -= height;
        if sample.drop.abs() < 1e-6 {
            // In calm air the horizontal path is a straight line, so one
            // angle cancels the spin drift at the zero range.
            let windage = (-sample.windage).atan2(zero_range);
            return Ok((angle, windage));
        }
        let slope = (sample.drop - previous_drop) / (angle - previous_angle);
        if !slope.is_finite() || slope <= 0.0 {
            break;
        }
        (previous_angle, previous_drop) = (angle, sample.drop);
        angle -= sample.drop / slope;
        if angle.abs() > 0.5 {
            break;
        }
    }
    Err(SolveError::ZeroOutOfReach)
}

/// Bore direction rotated `elevation` up from horizontal and `windage` to the
/// right about the sight's vertical axis.
fn launch_velocity(speed: f64, elevation: f64, windage: f64) -> Vec3 {
    Vec3::new(
        windage.cos() * elevation.cos(),
        windage.cos() * elevation.sin(),
        windage.sin(),
    ) * speed
}

struct Flight {
    /// Air at the muzzle; thinned with height as the bullet climbs or falls.
    atmosphere: Atmosphere,
    bullet: Bullet,
    wind: Vec3,
    earth_rotation: Option<Vec3>,
    sight_position: Vec3,
    sight_direction: Vec3,
    sight_up: Vec3,
    spin: Option<(f64, bool)>,
}

impl Flight {
    fn new(scenario: &Scenario, conditions: &Conditions, stability: Option<f64>) -> Self {
        let inclination = conditions.inclination_degrees.to_radians();
        let sight_direction = Vec3::new(inclination.cos(), inclination.sin(), 0.0);
        let sight_up = Vec3::new(-inclination.sin(), inclination.cos(), 0.0);
        let earth_rotation = conditions.coriolis.map(|coriolis| {
            let latitude = coriolis.latitude_degrees.to_radians();
            let azimuth = coriolis.azimuth_degrees.to_radians();
            Vec3::new(
                latitude.cos() * azimuth.cos(),
                latitude.sin(),
                -latitude.cos() * azimuth.sin(),
            ) * EARTH_ROTATION_RAD_PER_S
        });
        Self {
            atmosphere: conditions.atmosphere,
            bullet: scenario.bullet.clone(),
            wind: conditions.wind.velocity(),
            earth_rotation,
            sight_position: sight_up * scenario.rifle.sight_height_m,
            sight_direction,
            sight_up,
            spin: stability
                .filter(|_| scenario.spin_drift)
                .map(|sg| (sg, scenario.rifle.right_hand_twist)),
        }
    }

    fn acceleration(&self, velocity: Vec3, air: (f64, f64)) -> Vec3 {
        let (density, speed_of_sound) = air;
        let airspeed = velocity - self.wind;
        let speed = airspeed.length();
        let bc = self.bullet.bc_si_at(speed);
        // ρ·π / (8·BC) turns Cd(Mach)·v² into a deceleration.
        let drag_scale = density * std::f64::consts::PI / (8.0 * bc);
        let drag_coefficient = self.bullet.drag_coefficient(speed / speed_of_sound);
        let mut acceleration = airspeed * (-drag_scale * drag_coefficient * speed);
        acceleration.y -= GRAVITY;
        if let Some(rotation) = self.earth_rotation {
            acceleration = acceleration - rotation.cross(velocity) * 2.0;
        }
        acceleration
    }

    /// Air density and speed of sound at a height above the muzzle.
    fn air_at(&self, height: f64) -> (f64, f64) {
        let air = self.atmosphere.at_height(height);
        (air.density(), air.speed_of_sound())
    }

    fn sample(&self, time: f64, position: Vec3, velocity: Vec3) -> Sample {
        let relative = position - self.sight_position;
        let spin_drift = self.spin.map_or(0.0, |(sg, right_hand)| {
            stability::spin_drift(sg, time, right_hand)
        });
        Sample {
            time,
            range: relative.dot(self.sight_direction),
            drop: relative.dot(self.sight_up),
            windage: relative.z + spin_drift,
            velocity: velocity.length(),
            mach: (velocity - self.wind).length() / self.air_at(position.y).1,
        }
    }

    /// Integrates with RK4 until `keep_going` rejects a sample (which is
    /// still recorded, so the path brackets the range asked for).
    fn fly(&self, launch: Vec3, mut keep_going: impl FnMut(&Sample) -> bool) -> Vec<Sample> {
        let dt = TIME_STEP_S;
        let (mut position, mut velocity, mut time) = (Vec3::ZERO, launch, 0.0);
        let mut samples = vec![self.sample(time, position, velocity)];
        while time < MAX_FLIGHT_TIME_S && velocity.length() > MIN_SPEED {
            // Density barely changes within one step, so it is evaluated once.
            let air = self.air_at(position.y);
            let k1 = self.acceleration(velocity, air);
            let v2 = velocity + k1 * (dt * 0.5);
            let k2 = self.acceleration(v2, air);
            let v3 = velocity + k2 * (dt * 0.5);
            let k3 = self.acceleration(v3, air);
            let v4 = velocity + k3 * dt;
            let k4 = self.acceleration(v4, air);
            position = position + (velocity + v2 * 2.0 + v3 * 2.0 + v4) * (dt / 6.0);
            velocity = velocity + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0);
            time += dt;
            // A step that blows up (a very light bullet with a lot of
            // drag) ends the path rather than filling it with nonsense.
            if !(position.x.is_finite()
                && position.y.is_finite()
                && position.z.is_finite()
                && velocity.length().is_finite())
                || velocity.length() > launch.length() * 1.5 + 100.0
            {
                break;
            }
            let sample = self.sample(time, position, velocity);
            samples.push(sample);
            if !keep_going(&sample) {
                break;
            }
        }
        samples
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, scale: f64) -> Vec3 {
        Vec3::new(self.x * scale, self.y * scale, self.z * scale)
    }
}

//! The shot as a cloud of pellets, each flown on its own.
//!
//! A shot's pellets don't arrive together. The front pellets leave the
//! muzzle at the load's velocity; the others start a little slower, and
//! those dented in the barrel drag more and fall back. At 35–40 m the cloud
//! is 2–4 m long and passes in 10–15 ms, lead longer than steel (Lowry,
//! "The Effect of a Shot String?", American Rifleman, November 1979;
//! Compton, "An experimental and theoretical investigation of shot cloud
//! ballistics", PhD thesis, UCL, 1996).
//!
//! Here each pellet `i` gets
//! - a start speed `V·(1 − a|z₁|)` and a drag `Cd·(1 + b|z₂|)`, with `z₁`,
//!   `z₂` independent standard normals: half-normal, so nothing outruns the
//!   front pellets and the string has a hard front and a long tail, as
//!   Lowry's and Compton's do. The string is not drawn; it comes from flying
//!   every pellet with its own speed and drag. `a` and `b` lie between the
//!   fits to Lowry's string lengths at 40–65 yards (table C) and to
//!   Compton's cloud lengths at 20–50 m, which pull a little apart; the
//!   tests hold them to both, and to Compton's arrival-time peaks;
//! - a direction off the pattern's centre, a round normal of the choke's
//!   spread (`sigma_at`) that it keeps for the whole flight. A tight choke's
//!   spread grows with each pellet's own flight time (Compton: σ ∝ t), so a
//!   slow, dented pellet lands later and wider, as Compton, Brister and
//!   Journée saw; a cylinder's opens at a fixed angle. Between them the time
//!   share is the choke law's delay over a full choke's;
//! - its own drop and wind drift, from its own flight.
//!
//! From shot to shot the front velocity varies by 1.1 % for lead and 1.7 %
//! for steel (Compton, table 3.2) and the pattern's size by about 4 % for
//! small shot, once the noise of counting a finite number of pellets is
//! taken out (Compton's raw data), and 15–20 % for BB and larger (BASC).
//!
//! Left out, for want of data: clumping (Compton's counts in 100 mm cells
//! are Poisson), a separate group of fliers (Compton's patterns are normal
//! to about 1.65σ; the drag spread's tail gives the few that trail far
//! behind), and how far the pattern's centre wanders from shot to shot.
//! The research is in docs/shotgun-research/swarm.md.

use super::{Material, Pellet, SplitMix64, fly, spread};
use crate::solver::{self, Conditions, Sample, SolveError};

/// How a load's pellets differ from each other, and its shots from each
/// other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwarmSpread {
    /// The scale `a` of the half-normal shortfall in start speed behind the
    /// front pellets, as a share of their speed.
    pub start_speed: f64,
    /// The scale `b` of the half-normal extra drag of dented pellets, as a
    /// share of a sphere's.
    pub drag: f64,
    /// One standard deviation, as a share, of the front pellets' velocity
    /// from shot to shot.
    pub shot_velocity: f64,
    /// One standard deviation of the log of the pattern's size from shot to
    /// shot.
    pub shot_pattern: f64,
}

impl SwarmSpread {
    /// Soft lead (under 0.5 % antimony) without buffer: between the fits to
    /// Lowry's standard 1¼ oz No. 4 (string 79 inches long at 40 yards
    /// holding 80 % of the pellets) and to Compton's 32 g No. 5.
    pub const SOFT_LEAD: Self = Self {
        start_speed: 0.035,
        drag: 0.12,
        shot_velocity: 0.011,
        shot_pattern: 0.04,
    };
    /// Hardened, plated or buffered lead, as game shot is made: about half
    /// soft lead's speed spread (Lowry's buffered 1½ oz No. 4: 56 inches).
    pub const HARD_LEAD: Self = Self {
        start_speed: 0.02,
        drag: 0.10,
        shot_velocity: 0.011,
        shot_pattern: 0.04,
    };
    /// Steel hardly dents, so it has little drag spread; its string comes
    /// from its start speeds. Lowry's (55 inches at 40 yards) and Compton's
    /// steel strings disagree on how fast they grow; this lies between.
    pub const STEEL: Self = Self {
        start_speed: 0.05,
        drag: 0.04,
        shot_velocity: 0.017,
        shot_pattern: 0.05,
    };
    /// Every pellet alike and every shot alike: the flat pattern.
    pub const NONE: Self = Self {
        start_speed: 0.0,
        drag: 0.0,
        shot_velocity: 0.0,
        shot_pattern: 0.0,
    };

    /// The spread for shot of `material` and `diameter_m`. No source
    /// measured bismuth, tungsten or zinc strings; they are taken as hard
    /// lead, with Compton's shot-to-shot velocity for bismuth and zinc.
    pub fn of(material: Material, diameter_m: f64) -> Self {
        let spread = match material {
            Material::Steel => Self::STEEL,
            Material::Bismuth => Self {
                shot_velocity: 0.016,
                ..Self::HARD_LEAD
            },
            Material::Zinc => Self {
                shot_velocity: 0.012,
                ..Self::HARD_LEAD
            },
            Material::Lead | Material::TungstenIron | Material::TungstenMatrix | Material::Tss => {
                Self::HARD_LEAD
            }
        };
        // Few large pellets pattern far less evenly from shot to shot.
        let large = ((diameter_m - SMALL_SHOT_M) / (LARGE_SHOT_M - SMALL_SHOT_M)).clamp(0.0, 1.0);
        Self {
            shot_pattern: spread.shot_pattern + (LARGE_SHOT_PATTERN - spread.shot_pattern) * large,
            ..spread
        }
    }
}

/// Up to US No. 4 (3.3 mm) a pattern's size varies as small shot's ...
const SMALL_SHOT_M: f64 = 0.0033;
/// ... and from BB (4.6 mm) up as large shot's.
const LARGE_SHOT_M: f64 = 0.0046;
const LARGE_SHOT_PATTERN: f64 = 0.175;

/// The farthest out a half-normal draw reaches, so one pellet in millions
/// can't stop dead.
const MOST_SIGMAS: f64 = 4.0;
/// Flights on each side of the grid that every pellet's flight is read
/// from.
const GRID: usize = 7;
/// Pellets drawn once to give the cloud's averages and string.
const TYPICAL_DRAWS: usize = 2_000;
const TYPICAL_SEED: u64 = 0x5717_1E55;

/// One pellet's draw for a shot: its direction and how it flies.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Draw {
    /// Standard normals [right, up]: its direction off the pattern's
    /// centre, in units of the spread.
    direction: [f64; 2],
    /// Its start speed over the load's front velocity.
    speed: f64,
    /// Its drag over a sphere's.
    drag: f64,
}

/// One shot's pellets, drawn once and read at any range.
#[derive(Clone, Debug, PartialEq)]
pub struct SwarmShot {
    /// This shot's pattern size over the choke's.
    pub pattern_scale: f64,
    /// This shot's front velocity over the load's.
    pub velocity_scale: f64,
    pellets: Vec<Draw>,
}

impl SwarmShot {
    pub fn len(&self) -> usize {
        self.pellets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pellets.is_empty()
    }
}

/// Where and when one pellet crosses the plane at a range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    /// [right, up] in metres from where the load's front pellets' path
    /// crosses the plane: the pattern's centre in the tables.
    pub place: [f64; 2],
    /// Seconds after leaving the muzzle.
    pub time_s: f64,
    pub velocity_mps: f64,
}

/// How long the cloud is at a range, and how fast its front and back are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotString {
    /// When the front pellets arrive, seconds.
    pub front_time_s: f64,
    /// When half the pellets have arrived, seconds.
    pub middle_time_s: f64,
    /// The shortest stretch of time holding 80 % of the pellets, seconds.
    pub time_80_s: f64,
    /// That stretch as a length, the front pellets' speed times it: Lowry's
    /// measure of a string, metres.
    pub length_80_m: f64,
    pub front_velocity_mps: f64,
    /// The speed of the pellet that arrives after 90 % of the others.
    pub back_velocity_mps: f64,
}

/// A load's pellets flown from the muzzle: a grid of flights by start speed
/// and drag that every pellet's own flight is read from.
#[derive(Clone, Debug)]
pub struct Swarm {
    spread: SwarmSpread,
    /// Start speed over the load's front velocity, at each grid line.
    speeds: Vec<f64>,
    /// Drag over a sphere's, at each grid line.
    drags: Vec<f64>,
    /// The flights, speed by speed, each over every drag.
    paths: Vec<Vec<Sample>>,
    /// The front pellets' flight: start speed and drag both 1.
    front: Vec<Sample>,
    typical: Vec<Draw>,
}

impl Swarm {
    /// Flies the grid for `pellet` at `muzzle_velocity` (the front
    /// pellets') in `conditions` out to `max_range` (slant). Refuses what
    /// `fly` refuses and a spread outside 0–20 % in speed, 0–50 % in drag, 0–5
    /// % in velocity and 0–50 % in pattern size from shot to shot.
    pub fn new(
        pellet: &Pellet,
        muzzle_velocity: f64,
        conditions: &Conditions,
        spread: SwarmSpread,
        max_range: f64,
    ) -> Result<Self, SolveError> {
        let SwarmSpread {
            start_speed,
            drag,
            shot_velocity,
            shot_pattern,
        } = spread;
        let within = |value: f64, most: f64| value.is_finite() && (0.0..=most).contains(&value);
        if !(within(start_speed, 0.2)
            && within(drag, 0.5)
            && within(shot_velocity, 0.05)
            && within(shot_pattern, 0.5))
        {
            return Err(SolveError::InvalidInput(
                "the swarm's spread must be 0–20 % in speed, 0–50 % in drag, and from shot to shot 0–5 % in velocity and 0–50 % in pattern size",
            ));
        }
        // A shot's faster front pellets can't pass the fastest a pellet is
        // flown at.
        let fastest = (1.0 + MOST_SIGMAS * shot_velocity)
            .min(super::MAX_MUZZLE_VELOCITY_MPS / muzzle_velocity)
            .max(1.0);
        let slowest = (1.0 - MOST_SIGMAS * start_speed) * (1.0 - MOST_SIGMAS * shot_velocity);
        let speeds = grid_lines(slowest, fastest);
        let drags = grid_lines(1.0, 1.0 + MOST_SIGMAS * drag);
        let front = fly(pellet, muzzle_velocity, conditions, max_range)?;
        let mut paths = Vec::with_capacity(speeds.len() * drags.len());
        for &speed in &speeds {
            for &drag in &drags {
                // More drag on the same pellet is less mass for the air to
                // slow: the same diameter, so the same Mach and Reynolds
                // numbers.
                let flown = Pellet {
                    density_kg_m3: pellet.density_kg_m3 / drag,
                    ..*pellet
                };
                paths.push(fly(
                    &flown,
                    (muzzle_velocity * speed).min(super::MAX_MUZZLE_VELOCITY_MPS),
                    conditions,
                    max_range,
                )?);
            }
        }
        let mut swarm = Self {
            spread,
            speeds,
            drags,
            paths,
            front,
            typical: Vec::new(),
        };
        swarm.typical = swarm.draws(TYPICAL_DRAWS, TYPICAL_SEED);
        Ok(swarm)
    }

    pub fn spread(&self) -> SwarmSpread {
        self.spread
    }

    /// One shot of `count` pellets. The same seed gives the same shot. With
    /// `shot_to_shot` off, the shot has the load's own pattern size and
    /// velocity, for averages. Refuses more than `MAX_PELLETS` pellets.
    pub fn shot(
        &self,
        count: usize,
        seed: u64,
        shot_to_shot: bool,
    ) -> Result<SwarmShot, SolveError> {
        if count > super::MAX_PELLETS {
            return Err(SolveError::InvalidInput("too many pellets in the pattern"));
        }
        let mut random = SplitMix64(seed);
        let (velocity, pattern) = random.normal_pair();
        let (velocity_scale, pattern_scale) = if shot_to_shot {
            (
                1.0 + self.spread.shot_velocity * velocity.clamp(-MOST_SIGMAS, MOST_SIGMAS),
                (self.spread.shot_pattern * pattern.clamp(-MOST_SIGMAS, MOST_SIGMAS)).exp(),
            )
        } else {
            (1.0, 1.0)
        };
        let mut pellets = self.draws(count, random.next());
        for pellet in &mut pellets {
            pellet.speed *= velocity_scale;
        }
        Ok(SwarmShot {
            pattern_scale,
            velocity_scale,
            pellets,
        })
    }

    fn draws(&self, count: usize, seed: u64) -> Vec<Draw> {
        let mut random = SplitMix64(seed);
        (0..count)
            .map(|_| {
                let (right, up) = random.normal_pair();
                let (slow, dented) = random.normal_pair();
                Draw {
                    direction: [right, up],
                    speed: 1.0 - self.spread.start_speed * slow.abs().min(MOST_SIGMAS),
                    drag: 1.0 + self.spread.drag * dented.abs().min(MOST_SIGMAS),
                }
            })
            .collect()
    }

    /// Where and when each of the shot's pellets crosses the plane at
    /// `range_m`, for a choke putting `pattern_percent` in the 30-inch circle
    /// at 40 yards. A pellet that doesn't reach the range is left out.
    pub fn arrivals(
        &self,
        shot: &SwarmShot,
        pattern_percent: f64,
        range_m: f64,
    ) -> Result<Vec<Arrival>, SolveError> {
        let sigma = spread::sigma_at(pattern_percent, range_m)? * shot.pattern_scale;
        let timed = spread::time_share(pattern_percent);
        let Some(at) = self.at(range_m) else {
            return Ok(Vec::new());
        };
        let reference = at.front;
        let mean_time = self.mean_time(&at);
        Ok(shot
            .pellets
            .iter()
            .filter_map(|pellet| {
                let sample = at.read(pellet.speed, pellet.drag)?;
                let grows = if mean_time > 0.0 {
                    (1.0 - timed) + timed * sample.time / mean_time
                } else {
                    1.0
                };
                let [right, up] = pellet.direction;
                Some(Arrival {
                    place: [
                        right * sigma * grows + sample.windage - reference.windage,
                        up * sigma * grows + sample.drop - reference.drop,
                    ],
                    time_s: sample.time,
                    velocity_mps: sample.velocity,
                })
            })
            .collect())
    }

    /// The load's string at `range_m`; `None` beyond where the front pellets
    /// reach.
    pub fn string_at(&self, range_m: f64) -> Option<ShotString> {
        let at = self.at(range_m)?;
        let mut times: Vec<(f64, f64)> = self
            .typical
            .iter()
            .filter_map(|pellet| at.read(pellet.speed, pellet.drag))
            .map(|sample| (sample.time, sample.velocity))
            .collect();
        if times.is_empty() {
            return None;
        }
        times.sort_by(|a, b| a.0.total_cmp(&b.0));
        let count = times.len();
        let inside = ((count as f64 * 0.8).ceil() as usize).clamp(1, count);
        let time_80_s = (0..=count - inside)
            .map(|first| times[first + inside - 1].0 - times[first].0)
            .fold(f64::INFINITY, f64::min);
        let front = at.front;
        Some(ShotString {
            front_time_s: front.time,
            middle_time_s: times[count / 2].0,
            time_80_s,
            length_80_m: time_80_s * front.velocity,
            front_velocity_mps: front.velocity,
            back_velocity_mps: times[((count * 9) / 10).min(count - 1)].1,
        })
    }

    fn mean_time(&self, at: &At) -> f64 {
        let times: Vec<f64> = self
            .typical
            .iter()
            .filter_map(|pellet| at.read(pellet.speed, pellet.drag))
            .map(|sample| sample.time)
            .collect();
        if times.is_empty() {
            at.front.time
        } else {
            times.iter().sum::<f64>() / times.len() as f64
        }
    }

    /// Every flight read at `range_m`.
    fn at(&self, range_m: f64) -> Option<At<'_>> {
        Some(At {
            swarm: self,
            front: solver::interpolate(&self.front, range_m)?,
            samples: self
                .paths
                .iter()
                .map(|path| solver::interpolate(path, range_m))
                .collect(),
        })
    }
}

/// The grid's flights at one range.
struct At<'a> {
    swarm: &'a Swarm,
    front: Sample,
    samples: Vec<Option<Sample>>,
}

impl At<'_> {
    /// A pellet's flight at this range, between the grid's four flights
    /// around it.
    fn read(&self, speed: f64, drag: f64) -> Option<Sample> {
        let (row, across) = place(&self.swarm.speeds, speed);
        let (column, down) = place(&self.swarm.drags, drag);
        let drags = self.swarm.drags.len();
        let corner = |i: usize, j: usize| self.samples[i * drags + j];
        let next_row = (row + 1).min(self.swarm.speeds.len() - 1);
        let next_column = (column + 1).min(drags - 1);
        // A flight next to the pellet's may have ended short of the range
        // (fallen steeply) when the pellet's own wouldn't: read between the
        // flights that reach, unless most of the pellet's weight is on those
        // that don't.
        let corners = [
            (corner(row, column), (1.0 - across) * (1.0 - down)),
            (corner(next_row, column), across * (1.0 - down)),
            (corner(row, next_column), (1.0 - across) * down),
            (corner(next_row, next_column), across * down),
        ];
        let reach: f64 = corners
            .iter()
            .filter(|(sample, _)| sample.is_some())
            .map(|(_, weight)| weight)
            .sum();
        if reach < 0.5 {
            return None;
        }
        let mix = |value: fn(&Sample) -> f64| {
            corners
                .iter()
                .filter_map(|(sample, weight)| sample.as_ref().map(|sample| value(sample) * weight))
                .sum::<f64>()
                / reach
        };
        let range = corners.iter().find_map(|(sample, _)| *sample)?.range;
        Some(Sample {
            time: mix(|s| s.time),
            range,
            drop: mix(|s| s.drop),
            windage: mix(|s| s.windage),
            velocity: mix(|s| s.velocity),
            mach: mix(|s| s.mach),
        })
    }
}

/// `GRID` lines from `low` to `high`, or one where they meet.
fn grid_lines(low: f64, high: f64) -> Vec<f64> {
    if high - low < 1e-12 {
        return vec![low];
    }
    (0..GRID)
        .map(|i| low + (high - low) * i as f64 / (GRID - 1) as f64)
        .collect()
}

/// The grid line at or below `value` and how far it is towards the next.
fn place(lines: &[f64], value: f64) -> (usize, f64) {
    if lines.len() == 1 {
        return (0, 0.0);
    }
    let step = lines[1] - lines[0];
    let position = ((value - lines[0]) / step).clamp(0.0, (lines.len() - 1) as f64);
    let index = (position.floor() as usize).min(lines.len() - 2);
    (index, position - index as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::Atmosphere;
    use crate::pellets::{LEAD_DENSITY_KG_M3, STEEL_DENSITY_KG_M3, calm, sigma_at};

    const YARD: f64 = 0.9144;
    const INCH: f64 = 0.0254;

    fn swarm(diameter_mm: f64, density: f64, velocity: f64, spread: SwarmSpread) -> Swarm {
        let pellet = Pellet {
            diameter_m: diameter_mm / 1000.0,
            density_kg_m3: density,
        };
        Swarm::new(
            &pellet,
            velocity,
            &calm(Atmosphere::standard()),
            spread,
            70.0,
        )
        .unwrap()
    }

    /// Lowry's standard 1¼ oz No. 4 soft lead: 227 m/s at 40 yards.
    fn lowry_standard() -> Swarm {
        swarm(3.30, LEAD_DENSITY_KG_M3, 433.0, SwarmSpread::SOFT_LEAD)
    }

    fn within(value: f64, expected: f64, share: f64) -> bool {
        (value / expected - 1.0).abs() <= share
    }

    /// Each pellet's distance behind the front pellets at `range_m`, as
    /// Lowry measured it: its lag in time times the front pellets' speed.
    fn lags(swarm: &Swarm, range_m: f64) -> Vec<f64> {
        let shot = swarm.shot(4000, 7, false).unwrap();
        let string = swarm.string_at(range_m).unwrap();
        let mut lags: Vec<f64> = swarm
            .arrivals(&shot, 60.0, range_m)
            .unwrap()
            .iter()
            .map(|pellet| (pellet.time_s - string.front_time_s) * string.front_velocity_mps)
            .collect();
        lags.sort_by(f64::total_cmp);
        lags
    }

    #[test]
    fn the_string_has_lowrys_shape_at_40_yards() {
        // Lowry, table A: from the front pellet, 50 / 80 / 90 / 95 % of the
        // pellets within 58 / 88 / 105 / 120 inches. The spread is a
        // compromise with Compton's lengths, which a closer fit to this
        // table overshoots by 20–25 % at 50 m: it falls 10–14 % short here.
        let lags = lags(&lowry_standard(), 40.0 * YARD);
        for (share, inches) in [(0.5, 58.0), (0.8, 88.0), (0.9, 105.0), (0.95, 120.0)] {
            let lag = lags[(share * lags.len() as f64) as usize - 1] / INCH;
            assert!(within(lag, inches, 0.15), "{share}: {lag:.0} in");
        }
        // No pellet outruns the front ones.
        assert!(lags[0] >= -1e-9);
    }

    #[test]
    fn strings_are_as_long_as_lowrys() {
        // Lowry, table C: 80 % of the pellets within 79 inches (standard
        // soft lead), 56 (buffered) and 55 (1⅛ oz No. 2 steel) at 40 yards.
        let buffered = swarm(3.30, LEAD_DENSITY_KG_M3, 452.0, SwarmSpread::HARD_LEAD);
        let steel = swarm(3.81, STEEL_DENSITY_KG_M3, 416.0, SwarmSpread::STEEL);
        for (swarm, inches) in [(lowry_standard(), 79.0), (buffered, 56.0), (steel, 55.0)] {
            let length = swarm.string_at(40.0 * YARD).unwrap().length_80_m / INCH;
            assert!(
                within(length, inches, 0.2),
                "{length:.0} in against {inches}"
            );
        }
    }

    #[test]
    fn a_soft_lead_string_grows_as_lowrys() {
        // Lowry, table C: 79 inches at 40 yards, 155 at 65 (1.96 times).
        let swarm = lowry_standard();
        let length = |yards: f64| swarm.string_at(yards * YARD).unwrap().length_80_m;
        let growth = length(65.0) / length(40.0);
        assert!((1.6..=2.2).contains(&growth), "{growth}");
    }

    #[test]
    fn strings_grow_as_comptons_from_20_to_50_m() {
        // Compton, figure 3.10, the main cloud's length (taken as 1.79 times
        // the shortest stretch holding 90 %): 32 g No. 5 lead 2.1 / 3.3 /
        // 4.5 / 5.7 m and steel 2.5 / 3.1 / 3.7 / 4.3 m at 20 / 30 / 40 /
        // 50 m.
        let lead = swarm(2.80, LEAD_DENSITY_KG_M3, 439.0, SwarmSpread::SOFT_LEAD);
        let steel = swarm(3.00, STEEL_DENSITY_KG_M3, 430.0, SwarmSpread::STEEL);
        for (swarm, lengths, tolerance) in [
            (lead, [2.1, 3.3, 4.5, 5.7], 0.2),
            (steel, [2.5, 3.1, 3.7, 4.3], 0.25),
        ] {
            for (range, expected) in [20.0, 30.0, 40.0, 50.0].into_iter().zip(lengths) {
                let lags = lags(&swarm, range);
                let inside = (lags.len() as f64 * 0.9) as usize;
                let shortest = (0..lags.len() - inside)
                    .map(|first| lags[first + inside] - lags[first])
                    .fold(f64::INFINITY, f64::min);
                let length = 1.79 * shortest;
                assert!(
                    within(length, expected, tolerance),
                    "{range} m: {length:.2} m against {expected}"
                );
            }
        }
    }

    #[test]
    fn the_back_of_the_string_is_slower_as_comptons() {
        // Compton, figure 4.4: at 20 m the trailing edge is 11–15 % slower
        // than the leading edge for small lead and 7 % for steel (read off
        // the plot). Taken here as the pellet that trails 95 in 100.
        let lead = swarm(2.80, LEAD_DENSITY_KG_M3, 439.0, SwarmSpread::SOFT_LEAD);
        let steel = swarm(3.00, STEEL_DENSITY_KG_M3, 430.0, SwarmSpread::STEEL);
        for (swarm, slower) in [(lead, 0.09..=0.15), (steel, 0.05..=0.10)] {
            let shot = swarm.shot(4000, 7, false).unwrap();
            let mut arrivals = swarm.arrivals(&shot, 60.0, 20.0).unwrap();
            arrivals.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
            let back = arrivals[arrivals.len() * 95 / 100].velocity_mps;
            let share = 1.0 - back / swarm.string_at(20.0).unwrap().front_velocity_mps;
            assert!(slower.contains(&share), "{share}");
        }
    }

    #[test]
    fn the_arrivals_peak_as_comptons() {
        // Compton, figure 3.16, No. 3 lead (3.3 mm, 287 m/s at 20 m): most
        // pellets arrive about 2, 4, 5 and 7 ms behind the front at 20, 30,
        // 40 and 50 m (read off the plot).
        let swarm = swarm(3.30, LEAD_DENSITY_KG_M3, 433.0, SwarmSpread::SOFT_LEAD);
        let shot = swarm.shot(20_000, 13, false).unwrap();
        for (range, peak_ms) in [(20.0, 2.0), (30.0, 4.0), (40.0, 5.0), (50.0, 7.0)] {
            let front = swarm.string_at(range).unwrap().front_time_s;
            let mut bins = [0usize; 60];
            for pellet in swarm.arrivals(&shot, 60.0, range).unwrap() {
                let bin = ((pellet.time_s - front) * 1000.0 / 0.5) as usize;
                if let Some(count) = bins.get_mut(bin) {
                    *count += 1;
                }
            }
            let most = (0..bins.len()).max_by_key(|&bin| bins[bin]).unwrap();
            let lag_ms = (most as f64 + 0.5) * 0.5;
            assert!(
                (lag_ms - peak_ms).abs() <= 1.5,
                "{range} m: {lag_ms} ms against {peak_ms}"
            );
        }
    }

    #[test]
    fn the_middle_of_the_swarm_takes_lowrys_and_bristers_lead() {
        // Lowry: 10 ft ahead of a bird crossing at 80 ft/s at 40 yards.
        // Brister: 7 ft at 40 mph (58.7 ft/s). A check of the flight time
        // more than of the string: the front pellets' alone would pass too.
        let middle = lowry_standard()
            .string_at(40.0 * YARD)
            .unwrap()
            .middle_time_s;
        assert!(within(middle * 80.0, 10.0, 0.1), "{} ft", middle * 80.0);
        assert!(within(middle * 58.67, 7.0, 0.1), "{} ft", middle * 58.67);
    }

    #[test]
    fn the_spread_stays_the_chokes() {
        // Growing with each pellet's flight time keeps the spread the choke
        // law gives, on average.
        let swarm = swarm(3.0, LEAD_DENSITY_KG_M3, 400.0, SwarmSpread::HARD_LEAD);
        let shot = swarm.shot(20_000, 3, false).unwrap();
        for percent in [40.0, 55.0, 70.0] {
            for range in [20.0, 36.576, 50.0] {
                let arrivals = swarm.arrivals(&shot, percent, range).unwrap();
                let square = arrivals
                    .iter()
                    .map(|pellet| pellet.place[0].powi(2) + pellet.place[1].powi(2))
                    .sum::<f64>()
                    / arrivals.len() as f64;
                let sigma = (square / 2.0).sqrt();
                let expected = sigma_at(percent, range).unwrap();
                assert!(
                    within(sigma, expected, 0.03),
                    "{percent} % at {range} m: {sigma} against {expected}"
                );
            }
        }
    }

    #[test]
    fn a_full_chokes_slow_pellets_land_wider() {
        let swarm = swarm(3.0, LEAD_DENSITY_KG_M3, 400.0, SwarmSpread::SOFT_LEAD);
        let shot = swarm.shot(20_000, 5, false).unwrap();
        let arrivals = swarm.arrivals(&shot, 70.0, 36.576).unwrap();
        let middle = swarm.string_at(36.576).unwrap().middle_time_s;
        let spread = |late: bool| {
            let chosen: Vec<f64> = arrivals
                .iter()
                .filter(|pellet| (pellet.time_s > middle) == late)
                .map(|pellet| pellet.place[0].hypot(pellet.place[1]))
                .collect();
            chosen.iter().sum::<f64>() / chosen.len() as f64
        };
        assert!(spread(true) > spread(false) * 1.01);
    }

    #[test]
    fn counts_vary_from_shot_to_shot_as_comptons() {
        // Compton, table 3.14: 36 g No. 4 lead (about 209 pellets) at 40
        // yards, 143.7 ± 9.6 in the 30-inch circle over ten shots.
        let swarm = swarm(
            3.1,
            LEAD_DENSITY_KG_M3,
            400.0,
            SwarmSpread::of(Material::Lead, 0.0031),
        );
        let percent = 143.7 / 209.0 * 100.0;
        let counts: Vec<f64> = (0..400)
            .map(|seed| {
                let shot = swarm.shot(209, seed, true).unwrap();
                let arrivals = swarm.arrivals(&shot, percent, 40.0 * YARD).unwrap();
                arrivals
                    .iter()
                    .filter(|pellet| pellet.place[0].hypot(pellet.place[1]) <= 15.0 * INCH)
                    .count() as f64
            })
            .collect();
        let mean = counts.iter().sum::<f64>() / counts.len() as f64;
        let sd = (counts
            .iter()
            .map(|count| (count - mean).powi(2))
            .sum::<f64>()
            / (counts.len() - 1) as f64)
            .sqrt();
        assert!(within(mean, 143.7, 0.03), "{mean}");
        assert!((7.5..=12.5).contains(&sd), "{sd}");
    }

    #[test]
    fn every_pellet_flies_as_its_own_flight() {
        // The grid reads a pellet's flight as flying it alone would.
        let pellet = Pellet {
            diameter_m: 0.003,
            density_kg_m3: LEAD_DENSITY_KG_M3,
        };
        let conditions = calm(Atmosphere::standard());
        let swarm = Swarm::new(&pellet, 400.0, &conditions, SwarmSpread::SOFT_LEAD, 60.0).unwrap();
        let shot = swarm.shot(50, 11, true).unwrap();
        let at = swarm.at(45.0).unwrap();
        for draw in &shot.pellets {
            let read = at.read(draw.speed, draw.drag).unwrap();
            let alone = Pellet {
                density_kg_m3: pellet.density_kg_m3 / draw.drag,
                ..pellet
            };
            let path = fly(&alone, 400.0 * draw.speed, &conditions, 60.0).unwrap();
            let flown = solver::interpolate(&path, 45.0).unwrap();
            assert!(
                (read.time - flown.time).abs() < 1e-4,
                "{} against {}",
                read.time,
                flown.time
            );
            assert!((read.velocity - flown.velocity).abs() < 0.3);
        }
    }

    #[test]
    fn without_spread_the_swarm_is_the_flat_pattern() {
        let swarm = swarm(3.0, LEAD_DENSITY_KG_M3, 400.0, SwarmSpread::NONE);
        let shot = swarm.shot(100, 1, true).unwrap();
        assert_eq!((shot.pattern_scale, shot.velocity_scale), (1.0, 1.0));
        let string = swarm.string_at(30.0).unwrap();
        assert!(string.time_80_s.abs() < 1e-12);
        let sigma = sigma_at(60.0, 30.0).unwrap();
        for (pellet, draw) in swarm
            .arrivals(&shot, 60.0, 30.0)
            .unwrap()
            .iter()
            .zip(&shot.pellets)
        {
            assert!((pellet.time_s - string.front_time_s).abs() < 1e-12);
            assert!((pellet.place[0] - draw.direction[0] * sigma).abs() < 1e-9);
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_shot() {
        let swarm = swarm(3.0, STEEL_DENSITY_KG_M3, 420.0, SwarmSpread::STEEL);
        assert_eq!(
            swarm.shot(80, 9, true).unwrap(),
            swarm.shot(80, 9, true).unwrap()
        );
        assert_ne!(
            swarm.shot(80, 9, true).unwrap(),
            swarm.shot(80, 10, true).unwrap()
        );
        assert!(
            swarm
                .shot(crate::pellets::MAX_PELLETS + 1, 1, true)
                .is_err()
        );
    }

    #[test]
    fn large_shot_varies_more_from_shot_to_shot() {
        assert_eq!(SwarmSpread::of(Material::Lead, 0.0025).shot_pattern, 0.04);
        assert_eq!(
            SwarmSpread::of(Material::Steel, 0.0050).shot_pattern,
            LARGE_SHOT_PATTERN
        );
        let between = SwarmSpread::of(Material::Lead, 0.0040).shot_pattern;
        assert!(between > 0.04 && between < LARGE_SHOT_PATTERN);
    }
}

#[cfg(test)]
mod edges {
    use super::*;
    use crate::atmosphere::Atmosphere;
    use crate::pellets::{STEEL_DENSITY_KG_M3, calm};

    #[test]
    fn a_fast_load_still_flies() {
        // The fastest grid line stays within what a pellet is flown at.
        let pellet = Pellet {
            diameter_m: 0.003,
            density_kg_m3: STEEL_DENSITY_KG_M3,
        };
        let swarm = Swarm::new(
            &pellet,
            990.0,
            &calm(Atmosphere::standard()),
            SwarmSpread::STEEL,
            60.0,
        );
        assert!(swarm.is_ok(), "{:?}", swarm.err());
    }

    #[test]
    fn at_the_muzzle_the_pellets_are_where_they_point() {
        let pellet = Pellet {
            diameter_m: 0.003,
            density_kg_m3: STEEL_DENSITY_KG_M3,
        };
        let swarm = Swarm::new(
            &pellet,
            400.0,
            &calm(Atmosphere::standard()),
            SwarmSpread::STEEL,
            60.0,
        )
        .unwrap();
        let shot = swarm.shot(20, 1, true).unwrap();
        for pellet in swarm.arrivals(&shot, 70.0, 0.0).unwrap() {
            assert!(pellet.place.iter().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn pellets_reach_as_far_as_their_own_flights() {
        // Small shot far out: a pellet isn't left out because a neighbouring
        // flight in the grid fell short.
        let pellet = Pellet::of(Material::Lead, 0.002);
        let conditions = calm(Atmosphere::standard());
        let swarm = Swarm::new(&pellet, 400.0, &conditions, SwarmSpread::HARD_LEAD, 150.0).unwrap();
        let shot = swarm.shot(400, 3, false).unwrap();
        let reach = |range: f64| swarm.arrivals(&shot, 60.0, range).unwrap().len();
        let own = shot
            .pellets
            .iter()
            .filter(|draw| {
                let alone = Pellet {
                    density_kg_m3: pellet.density_kg_m3 / draw.drag,
                    ..pellet
                };
                fly(&alone, 400.0 * draw.speed, &conditions, 150.0)
                    .ok()
                    .and_then(|path| solver::interpolate(&path, 140.0))
                    .is_some()
            })
            .count();
        let read = reach(140.0);
        assert!(read.abs_diff(own) <= 400 / 50, "{read} against {own}");
    }
}

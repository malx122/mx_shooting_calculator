//! Where the shots may land around the aiming point, from what the hunter
//! doesn't know exactly: their own spread, the muzzle velocity's
//! shot-to-shot variation, the range and the wind. Each is taken as a
//! normal distribution, independent of the others, and its effect at the
//! target is found by flying the shot again with it changed (a linear
//! sensitivity), so the result is two standard deviations, across and up.

use crate::solver::{self, Scenario, SolveError, Solution};

/// A 5-shot group's extreme spread is on average 3.07 standard deviations
/// (per axis) of a round normal spread; a 3-shot group's 2.41.
pub const FIVE_SHOT_SPREAD_IN_SIGMAS: f64 = 3.07;

/// What isn't known exactly, each as one standard deviation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uncertainty {
    /// The shooter's and rifle's own spread: one standard deviation of the
    /// angle, per axis, radians.
    pub aim: f64,
    /// Shot-to-shot muzzle velocity, m/s.
    pub muzzle_velocity: f64,
    /// The range, m.
    pub range: f64,
    /// The wind's speed, m/s.
    pub wind_speed: f64,
    /// The wind's direction, degrees.
    pub wind_direction: f64,
}

impl Uncertainty {
    /// The aim's standard deviation from a 5-shot group's extreme spread
    /// `group` (metres) at `range` (metres).
    pub fn aim_from_group(group: f64, range: f64) -> f64 {
        group / range / FIVE_SHOT_SPREAD_IN_SIGMAS
    }
}

/// The shot flown again with each uncertain input changed a little, for
/// the sensitivities at every range.
pub struct Sensitivities {
    base: Solution,
    faster: Solution,
    windier: Solution,
    veered: Solution,
    /// The expected shot's wind speed, m/s.
    wind_speed: f64,
}

/// Small changes to fly with: enough to stay clear of rounding, small
/// enough to stay linear.
const DELTA_VELOCITY: f64 = 5.0;
const DELTA_WIND: f64 = 1.0;
const DELTA_DEGREES: f64 = 5.0;

/// How the shots spread at one range, as standard deviations in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    /// Across, to the right positive.
    pub across: f64,
    /// Up and down.
    pub up: f64,
}

impl Sensitivities {
    /// Flies the shot three more times: faster, in more wind, and with the
    /// wind turned. The sights stay as zeroed for `base`.
    pub fn new(scenario: &Scenario, base: Solution, max_range: f64) -> Result<Self, SolveError> {
        let zero = (base.zero_elevation, base.zero_windage);
        let mut faster = scenario.clone();
        faster.load.muzzle_velocity += DELTA_VELOCITY;
        let mut windier = scenario.clone();
        windier.shot.wind.speed += DELTA_WIND;
        let mut veered = scenario.clone();
        veered.shot.wind.from_degrees += DELTA_DEGREES;
        Ok(Sensitivities {
            faster: solver::solve_with_zero(&faster, max_range, zero)?,
            windier: solver::solve_with_zero(&windier, max_range, zero)?,
            veered: solver::solve_with_zero(&veered, max_range, zero)?,
            base,
            wind_speed: scenario.shot.wind.speed,
        })
    }

    /// The spread at `range` for these uncertainties.
    pub fn spread_at(&self, range: f64, uncertainty: &Uncertainty) -> Option<Spread> {
        let base = self.base.at(range)?;
        let faster = self.faster.at(range)?;
        let windier = self.windier.at(range)?;
        let veered = self.veered.at(range)?;
        // Per unit of each input: metres at the target.
        let per_velocity = [
            (faster.windage - base.windage) / DELTA_VELOCITY,
            (faster.drop - base.drop) / DELTA_VELOCITY,
        ];
        let per_wind = [
            (windier.windage - base.windage) / DELTA_WIND,
            (windier.drop - base.drop) / DELTA_WIND,
        ];
        let per_degree = [
            (veered.windage - base.windage) / DELTA_DEGREES,
            (veered.drop - base.drop) / DELTA_DEGREES,
        ];
        // A range error moves the target, not the bullet: the hold was set
        // for `range`, so the bullet meets a target at range + d where the
        // path has gone on, less what the held angle gives there.
        let range_error = uncertainty.range.max(0.0);
        let per_range = if range_error > 0.0 {
            let step = range_error.min(range * 0.5).max(0.5);
            let further = self.base.at(range + step)?;
            [
                (further.windage - base.windage * (range + step) / range) / step,
                (further.drop - base.drop * (range + step) / range) / step,
            ]
        } else {
            [0.0, 0.0]
        };
        let aim = uncertainty.aim * range;
        let axis = |k: usize| {
            (aim * aim
                + (per_velocity[k] * uncertainty.muzzle_velocity).powi(2)
                + (per_wind[k] * uncertainty.wind_speed).powi(2)
                + (per_degree[k] * uncertainty.wind_direction).powi(2)
                + (per_range[k] * range_error).powi(2))
            .sqrt()
        };
        Some(Spread {
            across: axis(0),
            up: axis(1),
        })
    }

    /// Where the bullet goes across at `range` in a wind of `speed` (m/s)
    /// from the same direction, metres to the right: for a wind bracket.
    pub fn drift_at(&self, range: f64, speed: f64) -> Option<f64> {
        let base = self.base.at(range)?;
        let windier = self.windier.at(range)?;
        Some(base.windage + (windier.windage - base.windage) / DELTA_WIND * (speed - self.wind_speed))
    }

    /// The expected shot.
    pub fn base(&self) -> &Solution {
        &self.base
    }
}

/// The chance (0 to 1) that a shot lands inside `inside` when the shots
/// spread normally by `spread` around `centre` (all in metres, x across, y
/// up). Integrated on a grid over ±4 standard deviations.
pub fn hit_chance(spread: Spread, centre: [f64; 2], inside: impl Fn([f64; 2]) -> bool) -> f64 {
    const STEPS: usize = 120;
    let (sx, sy) = (spread.across.max(1e-4), spread.up.max(1e-4));
    let mut total = 0.0;
    let mut hit = 0.0;
    for i in 0..STEPS {
        let u = -4.0 + 8.0 * (i as f64 + 0.5) / STEPS as f64;
        let wx = (-0.5 * u * u).exp();
        for j in 0..STEPS {
            let v = -4.0 + 8.0 * (j as f64 + 0.5) / STEPS as f64;
            let w = wx * (-0.5 * v * v).exp();
            total += w;
            if inside([centre[0] + u * sx, centre[1] + v * sy]) {
                hit += w;
            }
        }
    }
    hit / total
}

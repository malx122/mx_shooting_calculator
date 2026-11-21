//! Muzzle velocities from a chronograph, and how they move the shots up
//! and down.

use crate::distributions::{chi_squared_quantile, t_quantile};
use crate::geometry::{Point, centroid};
use crate::group::{CONFIDENCE, Estimate, Interval, StatsError};

/// The slowest muzzle velocity (m/s) a chronograph can have read; slower
/// is a misread. The same bounds as the chronograph import.
pub const SLOWEST_MPS: f64 = 10.0;

/// The fastest muzzle velocity (m/s) a chronograph can have read.
pub const FASTEST_MPS: f64 = 2500.0;

/// Readings slower than this (m/s) are possible but unusual for a rifle,
/// so worth a second look.
pub const USUAL_SLOWEST_MPS: f64 = 100.0;

/// Readings faster than this (m/s) are possible but unusual.
pub const USUAL_FASTEST_MPS: f64 = 1500.0;

/// A reading at least this share away from the string's median is worth a
/// second look.
pub const FAR_FROM_MEDIAN: f64 = 0.10;

/// The fewest shots with both a place and a velocity that the velocity
/// against vertical fit needs.
pub const MIN_PAIRED_SHOTS: usize = 5;

/// A reading that is accepted but worth a second look; readings are
/// counted from 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VelocityWarning {
    /// Slower than [`USUAL_SLOWEST_MPS`] or faster than
    /// [`USUAL_FASTEST_MPS`].
    Unusual { index: usize, mps: f64 },
    /// [`FAR_FROM_MEDIAN`] or more from the median of three readings or
    /// more: feet per second typed as metres per second, or a slipped
    /// decimal point.
    FarFromMedian {
        index: usize,
        mps: f64,
        median_mps: f64,
    },
}

/// The warnings for `readings` (m/s), in order of the readings.
pub fn velocity_warnings(readings: &[f64]) -> Vec<VelocityWarning> {
    let mut sorted: Vec<f64> = readings.to_vec();
    sorted.sort_by(f64::total_cmp);
    let half = sorted.len() / 2;
    let median = match sorted.len() {
        0..=2 => None,
        n if n % 2 == 1 => Some(sorted[half]),
        _ => Some(0.5 * (sorted[half - 1] + sorted[half])),
    };
    let mut warnings = Vec::new();
    for (index, &mps) in readings.iter().enumerate() {
        if !(USUAL_SLOWEST_MPS..=USUAL_FASTEST_MPS).contains(&mps) {
            warnings.push(VelocityWarning::Unusual { index, mps });
        }
        if let Some(median_mps) = median
            && (mps - median_mps).abs() >= FAR_FROM_MEDIAN * median_mps
        {
            warnings.push(VelocityWarning::FarFromMedian {
                index,
                mps,
                median_mps,
            });
        }
    }
    warnings
}

/// Whether `mps` is a muzzle velocity a chronograph can have read.
pub fn is_plausible(mps: f64) -> bool {
    (SLOWEST_MPS..=FASTEST_MPS).contains(&mps)
}

fn check_readings(readings: &[f64]) -> Result<(), StatsError> {
    if let Some((index, &mps)) = readings
        .iter()
        .enumerate()
        .find(|&(_, &mps)| !is_plausible(mps))
    {
        if !mps.is_finite() {
            return Err(StatsError::NotFinite);
        }
        return Err(StatsError::ImplausibleVelocity { index, mps });
    }
    Ok(())
}

/// The spread of a string of velocity readings, m/s.
#[derive(Clone, Debug, PartialEq)]
pub struct VelocityStats {
    pub shots: usize,
    pub average: f64,
    pub slowest: f64,
    pub fastest: f64,
    /// Fastest − slowest.
    pub extreme_spread: f64,
    /// The sample standard deviation (n − 1) with its 95% range from
    /// χ²(n − 1); `None` for one reading.
    pub sd: Option<Estimate>,
    /// Readings worth a second look; see [`velocity_warnings`].
    pub warnings: Vec<VelocityWarning>,
}

impl VelocityStats {
    /// The statistics of `readings` (m/s). Refuses a reading no
    /// chronograph can have made instead of guessing what was meant, and
    /// lists the ones worth a second look.
    pub fn new(readings: &[f64]) -> Result<VelocityStats, StatsError> {
        if readings.is_empty() {
            return Err(StatsError::NoShots);
        }
        check_readings(readings)?;
        let n = readings.len();
        let average = readings.iter().sum::<f64>() / n as f64;
        let slowest = readings.iter().copied().fold(f64::INFINITY, f64::min);
        let fastest = readings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let sd = (n >= 2).then(|| {
            let degrees = (n - 1) as f64;
            let squares: f64 = readings.iter().map(|v| (v - average).powi(2)).sum();
            let tail = 0.5 * (1.0 - CONFIDENCE);
            Estimate {
                value: (squares / degrees).sqrt(),
                range: Interval {
                    low: (squares / chi_squared_quantile(1.0 - tail, degrees)).sqrt(),
                    high: (squares / chi_squared_quantile(tail, degrees)).sqrt(),
                },
            }
        });
        Ok(VelocityStats {
            shots: n,
            average,
            slowest,
            fastest,
            extreme_spread: fastest - slowest,
            sd,
            warnings: velocity_warnings(readings),
        })
    }
}

/// A straight line fitted by least squares to each shot's height (mm from
/// the group's centre, up positive) against its velocity (m/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityVertical {
    /// How many shots had both.
    pub shots: usize,
    /// mm higher per m/s faster, with its 95% range from Student's t
    /// with n − 2 degrees of freedom.
    pub slope: Estimate,
    /// The slope's standard error, mm per m/s.
    pub slope_standard_error: f64,
    /// The line's height at 0 m/s, mm; the line is best read through
    /// (`mean_velocity`, `mean_height`).
    pub intercept: f64,
    /// The mean velocity of these shots, m/s.
    pub mean_velocity: f64,
    /// The mean height of these shots, mm from the group's centre.
    pub mean_height: f64,
    /// The share of the shots' vertical spread the line explains, 0 to 1.
    pub r_squared: f64,
}

impl VelocityVertical {
    /// The fit for `shots` (mm) where `velocities` (m/s, one per shot)
    /// are known; heights are measured from the centre of all the shots.
    pub fn new(shots: &[Point], velocities: &[Option<f64>]) -> Result<Self, StatsError> {
        if shots.len() != velocities.len() {
            return Err(StatsError::LengthMismatch);
        }
        if shots.iter().any(|shot| !shot.is_finite()) {
            return Err(StatsError::NotFinite);
        }
        let centre = centroid(shots).ok_or(StatsError::NoShots)?;
        let (speeds, heights): (Vec<f64>, Vec<f64>) = shots
            .iter()
            .zip(velocities)
            .filter_map(|(shot, velocity)| velocity.map(|v| (v, shot.y - centre.y)))
            .unzip();
        VelocityVertical::fit(&speeds, &heights)
    }

    /// The fit of `heights` (mm) against `velocities` (m/s), pair by pair.
    pub fn fit(velocities: &[f64], heights: &[f64]) -> Result<Self, StatsError> {
        if velocities.len() != heights.len() {
            return Err(StatsError::LengthMismatch);
        }
        let n = velocities.len();
        if n < MIN_PAIRED_SHOTS {
            return Err(StatsError::TooFewShots {
                need: MIN_PAIRED_SHOTS,
                have: n,
            });
        }
        if heights.iter().any(|h| !h.is_finite()) {
            return Err(StatsError::NotFinite);
        }
        check_readings(velocities)?;
        let mean_velocity = velocities.iter().sum::<f64>() / n as f64;
        let mean_height = heights.iter().sum::<f64>() / n as f64;
        let (mut vv, mut vh, mut hh) = (0.0, 0.0, 0.0);
        for (&v, &h) in velocities.iter().zip(heights) {
            let (dv, dh) = (v - mean_velocity, h - mean_height);
            vv += dv * dv;
            vh += dv * dh;
            hh += dh * dh;
        }
        if !(mean_height.is_finite() && vv.is_finite() && vh.is_finite() && hh.is_finite()) {
            return Err(StatsError::NotFinite);
        }
        if vv <= 0.0 {
            return Err(StatsError::NoVelocitySpread);
        }
        let slope = vh / vv;
        // Past their bounds only by rounding.
        let residual = (hh - slope * vh).max(0.0);
        let degrees = (n - 2) as f64;
        let standard_error = (residual / degrees / vv).sqrt();
        let t = t_quantile(0.5 + 0.5 * CONFIDENCE, degrees);
        Ok(VelocityVertical {
            shots: n,
            slope: Estimate {
                value: slope,
                range: Interval {
                    low: slope - t * standard_error,
                    high: slope + t * standard_error,
                },
            },
            slope_standard_error: standard_error,
            intercept: mean_height - slope * mean_velocity,
            mean_velocity,
            mean_height,
            r_squared: if hh > 0.0 {
                (vh * vh / (vv * hh)).min(1.0)
            } else {
                0.0
            },
        })
    }
}

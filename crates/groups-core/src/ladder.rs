//! Load development: a ladder of charges or seating depths, looking for a
//! run of rungs where the vertical impact holds still.
//!
//! Groups at the same value (within a small tolerance) pool into a rung.
//! Every run of at least [`MIN_RUN`] neighbouring rungs is scored by how
//! far its vertical impacts spread, (max − min) ÷ noise, where the noise
//! is the RMS of the run's vertical standard errors; likewise across. A
//! run whose score is within [`flat_limit`] for its length is called
//! flat: a spread that noise alone gives [`FLAT_SHARE`] of the time, so
//! the run can't be told from one whose impact truly holds still.
//!
//! Each rung's standard errors are σ̂/√N with σ̂ pooled over the whole
//! ladder, not the rung's own σ̂: a rung's few shots give a noisy σ̂,
//! and runs of loose rungs would then score flatter than runs of tight
//! ones.

use std::sync::OnceLock;

use crate::distributions::{normal_cdf, normal_pdf};
use crate::error::{AnalysisError, check_sigma, pool_sigmas, t95};
use crate::geometry::Point;
use crate::group::{GroupStats, Interval, Sigma, StatsError};
use crate::velocity::is_plausible;

/// Values closer than this (g or mm) are one rung.
pub const RUNG_TOLERANCE: f64 = 0.001;

/// The fewest rungs in a run.
pub const MIN_RUN: usize = 3;

/// A run is flat when its score is within the spread that this share of
/// runs of the same length reach from noise alone (see [`flat_limit`]).
pub const FLAT_SHARE: f64 = 2.0 / 3.0;

/// The flat limits are worked out once for runs up to this long.
const CACHED_RUNS: usize = 40;

/// The score at and below which a run of `rungs` rungs is flat: the
/// [`FLAT_SHARE`] quantile of the range of `rungs` independent standard
/// normals, so a run whose impact truly holds still passes two times in
/// three whatever its length (1.37 for 2 rungs, 2.00 for 3, 2.38 for 4,
/// 2.65 for 5, 2.85 for 6). `NaN` for fewer than two rungs.
pub fn flat_limit(rungs: usize) -> f64 {
    static LIMITS: OnceLock<Vec<f64>> = OnceLock::new();
    if rungs < 2 {
        return f64::NAN;
    }
    if rungs > CACHED_RUNS {
        return range_quantile(rungs, FLAT_SHARE);
    }
    LIMITS.get_or_init(|| {
        (2..=CACHED_RUNS)
            .map(|m| range_quantile(m, FLAT_SHARE))
            .collect()
    })[rungs - 2]
}

/// The `p` quantile of the range of `m` independent standard normals:
/// P(W ≤ w) = m ∫ φ(x) (Φ(x + w) − Φ(x))^(m − 1) dx, by Simpson's rule
/// over ±8, solved by Newton's method kept inside a bracket.
fn range_quantile(m: usize, p: f64) -> f64 {
    const STEPS: usize = 800;
    const EDGE: f64 = 8.0;
    let h = 2.0 * EDGE / STEPS as f64;
    let xs: Vec<f64> = (0..=STEPS).map(|i| -EDGE + i as f64 * h).collect();
    let below: Vec<f64> = xs.iter().map(|&x| normal_cdf(x)).collect();
    let density: Vec<f64> = xs.iter().map(|&x| normal_pdf(x)).collect();
    let weight = |i: usize| match i {
        0 | STEPS => 1.0,
        i if i % 2 == 1 => 4.0,
        _ => 2.0,
    };
    let m = m as f64;
    // (P(W ≤ w), its density at w).
    let cdf = |w: f64| {
        let (mut total, mut slope) = (0.0, 0.0);
        for (i, &x) in xs.iter().enumerate() {
            let inside = (normal_cdf(x + w) - below[i]).max(0.0);
            total += weight(i) * density[i] * inside.powf(m - 1.0);
            slope += weight(i) * density[i] * normal_pdf(x + w) * inside.powf(m - 2.0);
        }
        (m * total * h / 3.0, m * (m - 1.0) * slope * h / 3.0)
    };
    let (mut low, mut high) = (0.0, 4.0 * EDGE);
    let mut w = 2.0 * (2.0 * m.ln()).sqrt();
    for _ in 0..100 {
        let (value, slope) = cdf(w);
        if value < p {
            low = w;
        } else {
            high = w;
        }
        let newton = w - (value - p) / slope;
        let next = if newton.is_finite() && newton > low && newton < high {
            newton
        } else {
            0.5 * (low + high)
        };
        if (next - w).abs() <= 1e-12 * w {
            return next;
        }
        w = next;
    }
    w
}

/// One group of a ladder.
#[derive(Clone, Debug, PartialEq)]
pub struct LadderGroup {
    /// The rung's value: the charge, g, or the seating depth, mm.
    pub value: f64,
    /// The group's centre less its aim point, mrad, [across, up].
    pub impact_mrad: Point,
    pub shots: usize,
    /// The group's radial σ̂, mrad; ignored for a single shot.
    pub sigma_mrad: f64,
    /// The chronograph's readings for the group, m/s, if any.
    pub velocities_mps: Vec<f64>,
}

/// How runs are ranked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LadderKey {
    /// Vertical, then horizontal, then σ̂, each deciding only between runs
    /// the one before can't tell apart: flat runs first (lowest σ̂ first
    /// among those also flat across, then the flattest across), then the
    /// rest by vertical score.
    #[default]
    Default,
    /// The vertical score alone.
    Vertical,
    /// The horizontal score alone.
    Horizontal,
    /// The run's pooled σ̂ alone; runs without one last.
    Sigma,
}

/// What a ladder may use besides its groups.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LadderOptions {
    /// Values this close or closer to a rung's first value join it (0
    /// for equal values only).
    pub value_tolerance: f64,
    pub key: LadderKey,
    /// σ̂ of the rifle and load from elsewhere, mrad, with its degrees of
    /// freedom: pooled with the ladder's own σ̂ by their degrees of
    /// freedom, and the only measure of the noise for a ladder of single
    /// shots.
    pub known_sigma: Option<Sigma>,
}

impl Default for LadderOptions {
    fn default() -> Self {
        LadderOptions {
            value_tolerance: RUNG_TOLERANCE,
            key: LadderKey::Default,
            known_sigma: None,
        }
    }
}

/// How velocity changes along a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityTrend {
    /// The change of velocity per unit of the rungs' value: m/s per g of
    /// charge, or per mm of seating depth.
    pub per_unit: f64,
    /// Its 95% range, from Student's t with readings − 2 degrees of
    /// freedom; `None` with fewer than three readings.
    pub range: Option<Interval>,
    /// How many readings the line was fitted to.
    pub readings: usize,
}

/// The chronograph readings of a rung.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocitySummary {
    pub readings: usize,
    pub mean_mps: f64,
    /// The sample SD (n − 1) of the readings about their mean; `None` for
    /// one reading.
    pub sd_mps: Option<f64>,
}

/// The groups at one value.
#[derive(Clone, Debug, PartialEq)]
pub struct LadderRung {
    /// The mean of its groups' values.
    pub value: f64,
    /// Its groups, by index in the input.
    pub groups: Vec<usize>,
    pub shots: usize,
    /// The per-shot mean impact up, mrad.
    pub vertical: f64,
    /// The per-shot mean impact across, mrad.
    pub horizontal: f64,
    /// σ̂ of the ladder ÷ √(shots), mrad.
    pub vertical_standard_error: f64,
    /// The same across (the round model has one σ for both).
    pub horizontal_standard_error: f64,
    /// σ̂ pooled within this rung's groups; `None` without a group of two
    /// shots or more.
    pub sigma: Option<Sigma>,
    pub velocity: Option<VelocitySummary>,
}

/// A run of neighbouring rungs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LadderRun {
    /// Its first rung, by index in [`Ladder::rungs`].
    pub first: usize,
    /// How many rungs.
    pub rungs: usize,
    /// Highest less lowest vertical impact, mrad.
    pub vertical_range: f64,
    /// The RMS of its rungs' vertical standard errors, mrad.
    pub vertical_noise: f64,
    /// `vertical_range` ÷ `vertical_noise`.
    pub vertical_score: f64,
    pub horizontal_range: f64,
    pub horizontal_noise: f64,
    pub horizontal_score: f64,
    /// The flat limit for a run this long (see [`flat_limit`]).
    pub flat_limit: f64,
    /// The vertical score is at most `flat_limit`.
    pub flat: bool,
    /// The horizontal score is at most `flat_limit`.
    pub flat_across: bool,
    /// σ̂ pooled within all its groups, mrad.
    pub sigma: Option<Sigma>,
    /// The change of velocity per unit of value (m/s per g or per mm), by
    /// least squares over every reading in the run; `None` with readings
    /// at fewer than two rungs.
    pub velocity_per_unit: Option<VelocityTrend>,
}

/// A ladder worked out.
#[derive(Clone, Debug, PartialEq)]
pub struct Ladder {
    /// By value, lowest first.
    pub rungs: Vec<LadderRung>,
    /// The σ̂ behind every standard error: pooled within all the groups,
    /// and with the known σ̂ if one was given.
    pub noise_sigma: Sigma,
    /// Every run of [`MIN_RUN`] rungs or more, shortest first, then by
    /// first rung.
    pub runs: Vec<LadderRun>,
    /// The best run of each length by `key`, shortest first.
    pub best: Vec<LadderRun>,
    pub key: LadderKey,
}

impl LadderGroup {
    /// The ladder group of a measured group, which needs an aim point and
    /// a distance to give its impact and σ̂ in mrad.
    pub fn from_stats(
        stats: &GroupStats,
        value: f64,
        velocities_mps: Vec<f64>,
    ) -> Result<LadderGroup, AnalysisError> {
        let impact = stats.impact.ok_or(AnalysisError::MissingAim)?;
        let distance = stats
            .options
            .distance_m
            .ok_or(AnalysisError::MissingDistance)?;
        Ok(LadderGroup {
            value,
            impact_mrad: impact * (1.0 / distance),
            shots: stats.shots,
            sigma_mrad: stats.sigma_mrad().map_or(0.0, |sigma| sigma.value),
            velocities_mps,
        })
    }
}

impl Ladder {
    pub fn new(groups: &[LadderGroup], options: LadderOptions) -> Result<Ladder, AnalysisError> {
        if !options.value_tolerance.is_finite() {
            return Err(AnalysisError::NotFinite);
        }
        if options.value_tolerance < 0.0 {
            return Err(AnalysisError::Negative("value tolerance"));
        }
        if let Some(known) = options.known_sigma
            && !(known.value.is_finite()
                && known.value >= 0.0
                && known.degrees_of_freedom.is_finite()
                && known.degrees_of_freedom > 0.0)
        {
            return Err(AnalysisError::NotFinite);
        }
        if groups.is_empty() {
            return Err(AnalysisError::Empty);
        }
        for (index, group) in groups.iter().enumerate() {
            check_sigma(index, group.shots, group.sigma_mrad)?;
            let error = |error: StatsError| AnalysisError::Input { index, error };
            if !group.value.is_finite() || !group.impact_mrad.is_finite() {
                return Err(error(StatsError::NotFinite));
            }
            if let Some((reading, &mps)) = group
                .velocities_mps
                .iter()
                .enumerate()
                .find(|&(_, &mps)| !is_plausible(mps))
            {
                return Err(error(if mps.is_finite() {
                    StatsError::ImplausibleVelocity {
                        index: reading,
                        mps,
                    }
                } else {
                    StatsError::NotFinite
                }));
            }
        }
        let own = pool_sigmas(groups.iter().map(|g| (g.shots, g.sigma_mrad)));
        let noise_sigma = match (own, options.known_sigma) {
            (Some(own), Some(known)) => {
                let degrees = own.degrees_of_freedom + known.degrees_of_freedom;
                let squares = own.degrees_of_freedom * own.value * own.value
                    + known.degrees_of_freedom * known.value * known.value;
                Sigma {
                    value: (squares / degrees).sqrt(),
                    degrees_of_freedom: degrees,
                }
            }
            (Some(sigma), None) | (None, Some(sigma)) => sigma,
            (None, None) => return Err(AnalysisError::NoSpreadEstimate),
        };
        if !noise_sigma.value.is_finite() {
            return Err(AnalysisError::NotFinite);
        }
        let rungs = rungs(groups, options.value_tolerance, noise_sigma.value);
        if rungs.len() < MIN_RUN {
            return Err(AnalysisError::TooFew {
                what: "rungs",
                need: MIN_RUN,
                have: rungs.len(),
            });
        }
        let mut runs = Vec::new();
        for length in MIN_RUN..=rungs.len() {
            for first in 0..=rungs.len() - length {
                runs.push(run(groups, &rungs, first, length));
            }
        }
        if runs.iter().any(|run| {
            !(run.vertical_range.is_finite()
                && run.horizontal_range.is_finite()
                && run.vertical_noise.is_finite())
        }) {
            return Err(AnalysisError::NotFinite);
        }
        let best = (MIN_RUN..=rungs.len())
            .filter_map(|length| {
                runs.iter()
                    .filter(|run| run.rungs == length)
                    .min_by(|a, b| order(options.key, a, b))
                    .copied()
            })
            .collect();
        Ok(Ladder {
            rungs,
            noise_sigma,
            runs,
            best,
            key: options.key,
        })
    }
}

fn rungs(groups: &[LadderGroup], tolerance: f64, sigma: f64) -> Vec<LadderRung> {
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| groups[a].value.total_cmp(&groups[b].value));
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut first_value = f64::NAN;
    for index in order {
        let value = groups[index].value;
        match members.last_mut() {
            Some(rung) if value - first_value <= tolerance => rung.push(index),
            _ => {
                members.push(vec![index]);
                first_value = value;
            }
        }
    }
    members
        .into_iter()
        .map(|indices| {
            let shots: usize = indices.iter().map(|&i| groups[i].shots).sum();
            let total = shots as f64;
            let mean = |coordinate: fn(&Point) -> f64| {
                indices
                    .iter()
                    .map(|&i| groups[i].shots as f64 * coordinate(&groups[i].impact_mrad))
                    .sum::<f64>()
                    / total
            };
            let readings: Vec<f64> = indices
                .iter()
                .flat_map(|&i| groups[i].velocities_mps.iter().copied())
                .collect();
            let standard_error = sigma / total.sqrt();
            LadderRung {
                value: indices.iter().map(|&i| groups[i].value).sum::<f64>() / indices.len() as f64,
                shots,
                vertical: mean(|p| p.y),
                horizontal: mean(|p| p.x),
                vertical_standard_error: standard_error,
                horizontal_standard_error: standard_error,
                sigma: pool_sigmas(
                    indices
                        .iter()
                        .map(|&i| (groups[i].shots, groups[i].sigma_mrad)),
                ),
                velocity: summary(&readings),
                groups: indices,
            }
        })
        .collect()
}

fn summary(readings: &[f64]) -> Option<VelocitySummary> {
    if readings.is_empty() {
        return None;
    }
    let n = readings.len();
    let mean_mps = readings.iter().sum::<f64>() / n as f64;
    let sd_mps = (n >= 2).then(|| {
        let squares: f64 = readings.iter().map(|v| (v - mean_mps).powi(2)).sum();
        (squares / (n - 1) as f64).sqrt()
    });
    Some(VelocitySummary {
        readings: n,
        mean_mps,
        sd_mps,
    })
}

/// `range / noise`, where no noise makes any range infinitely large.
fn score(range: f64, noise: f64) -> f64 {
    if range == 0.0 {
        0.0
    } else if noise == 0.0 {
        f64::INFINITY
    } else {
        range / noise
    }
}

fn run(groups: &[LadderGroup], rungs: &[LadderRung], first: usize, length: usize) -> LadderRun {
    let part = &rungs[first..first + length];
    let spread = |value: fn(&LadderRung) -> f64| {
        let (low, high) = part
            .iter()
            .map(value)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), v| {
                (low.min(v), high.max(v))
            });
        high - low
    };
    let rms = |error: fn(&LadderRung) -> f64| {
        (part.iter().map(|rung| error(rung).powi(2)).sum::<f64>() / length as f64).sqrt()
    };
    let vertical_range = spread(|rung| rung.vertical);
    let vertical_noise = rms(|rung| rung.vertical_standard_error);
    let horizontal_range = spread(|rung| rung.horizontal);
    let horizontal_noise = rms(|rung| rung.horizontal_standard_error);
    let vertical_score = score(vertical_range, vertical_noise);
    let horizontal_score = score(horizontal_range, horizontal_noise);
    let limit = flat_limit(length);
    let members = || {
        part.iter()
            .flat_map(|rung| rung.groups.iter().map(|&i| &groups[i]))
    };
    let readings: Vec<(f64, f64)> = part
        .iter()
        .flat_map(|rung| {
            rung.groups.iter().flat_map(move |&i| {
                groups[i]
                    .velocities_mps
                    .iter()
                    .map(move |&mps| (rung.value, mps))
            })
        })
        .collect();
    LadderRun {
        first,
        rungs: length,
        vertical_range,
        vertical_noise,
        vertical_score,
        horizontal_range,
        horizontal_noise,
        horizontal_score,
        flat_limit: limit,
        flat: vertical_score <= limit,
        flat_across: horizontal_score <= limit,
        sigma: pool_sigmas(members().map(|g| (g.shots, g.sigma_mrad))),
        velocity_per_unit: slope(&readings),
    }
}

/// The least-squares slope of y on x, with its 95% range from three
/// points on.
fn slope(points: &[(f64, f64)]) -> Option<VelocityTrend> {
    let n = points.len();
    if n < 2 {
        return None;
    }
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / n as f64;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / n as f64;
    let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
    for &(x, y) in points {
        let (dx, dy) = (x - mean_x, y - mean_y);
        xx += dx * dx;
        xy += dx * dy;
        yy += dy * dy;
    }
    // Readings at a single rung say nothing of a change.
    if xx.is_nan() || xx <= 0.0 || !xx.is_finite() {
        return None;
    }
    let per_unit = xy / xx;
    let range = (n > 2).then(|| {
        let degrees = (n - 2) as f64;
        let residual = (yy - per_unit * xy).max(0.0);
        let half = t95(degrees) * (residual / degrees / xx).sqrt();
        Interval {
            low: per_unit - half,
            high: per_unit + half,
        }
    });
    Some(VelocityTrend {
        per_unit,
        range,
        readings: n,
    })
}

fn order(key: LadderKey, a: &LadderRun, b: &LadderRun) -> std::cmp::Ordering {
    let sigma = |run: &LadderRun| run.sigma.map_or(f64::INFINITY, |s| s.value);
    let primary = match key {
        LadderKey::Vertical => a.vertical_score.total_cmp(&b.vertical_score),
        LadderKey::Horizontal => a.horizontal_score.total_cmp(&b.horizontal_score),
        LadderKey::Sigma => sigma(a).total_cmp(&sigma(b)),
        LadderKey::Default => {
            // false sorts before true, so negate "flat".
            (!a.flat).cmp(&!b.flat).then_with(|| {
                if !a.flat {
                    a.vertical_score.total_cmp(&b.vertical_score)
                } else {
                    (!a.flat_across).cmp(&!b.flat_across).then_with(|| {
                        if a.flat_across {
                            sigma(a).total_cmp(&sigma(b))
                        } else {
                            a.horizontal_score.total_cmp(&b.horizontal_score)
                        }
                    })
                }
            })
        }
    };
    primary
        .then_with(|| a.vertical_score.total_cmp(&b.vertical_score))
        .then_with(|| a.horizontal_score.total_cmp(&b.horizontal_score))
        .then_with(|| a.first.cmp(&b.first))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Rng;

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    fn group(value: f64, up: f64, across: f64, shots: usize, sigma: f64) -> LadderGroup {
        LadderGroup {
            value,
            impact_mrad: Point::new(across, up),
            shots,
            sigma_mrad: sigma,
            velocities_mps: Vec::new(),
        }
    }

    #[test]
    fn rungs_and_runs_by_hand() {
        // σ̂ 0.5 everywhere; 4 shots a rung, so every SE is 0.25.
        let mut groups = vec![
            group(2.70, 0.0, 0.0, 4, 0.5),
            group(2.75, 1.0, 0.75, 4, 0.5),
            group(2.80, 1.25, 0.0, 2, 0.5),
            group(2.8004, 1.25, 0.0, 2, 0.5),
            group(2.85, 1.375, 0.0, 4, 0.5),
            group(2.90, 3.0, 0.0, 4, 0.5),
        ];
        groups[2].velocities_mps = vec![800.0, 802.0];
        groups[3].velocities_mps = vec![801.0];
        groups[1].velocities_mps = vec![790.0];
        groups[4].velocities_mps = vec![811.0, 809.0];
        let ladder = Ladder::new(&groups, LadderOptions::default()).unwrap();
        assert_eq!(ladder.rungs.len(), 5);
        let pooled = &ladder.rungs[2];
        assert_eq!(pooled.groups, [2, 3]);
        assert_eq!(pooled.shots, 4);
        close(pooled.value, 2.8002, 1e-12);
        close(pooled.vertical_standard_error, 0.25, 1e-15);
        assert_eq!(pooled.sigma.unwrap().degrees_of_freedom, 4.0);
        let velocity = pooled.velocity.unwrap();
        assert_eq!(velocity.readings, 3);
        close(velocity.mean_mps, 801.0, 1e-12);
        close(velocity.sd_mps.unwrap(), 1.0, 1e-12);
        assert_eq!(
            ladder.noise_sigma.degrees_of_freedom,
            2.0 * 3.0 * 4.0 + 2.0 * 2.0
        );
        // 3 + 2 + 1 runs of 3, 4 and 5 rungs.
        assert_eq!(ladder.runs.len(), 6);
        // 2.75–2.85: vertical 1 to 1.375, range 0.375 = 1.5 SE, within
        // the 2.0 of three rungs: flat. Across 0.75 = 3 SE: not flat.
        let middle = ladder
            .runs
            .iter()
            .find(|r| r.rungs == 3 && r.first == 1)
            .unwrap();
        assert_eq!(middle.vertical_range, 0.375);
        assert_eq!(middle.vertical_noise, 0.25);
        assert_eq!(middle.vertical_score, 1.5);
        assert_eq!(middle.flat_limit, flat_limit(3));
        assert!(middle.flat);
        assert_eq!(middle.horizontal_score, 3.0);
        assert!(!middle.flat_across);
        assert_eq!(ladder.best[0].first, 1);
        assert_eq!(ladder.best.len(), 3);
        // Velocity over 2.75 → 2.85: six readings, so t with 4 degrees
        // of freedom.
        let readings = [
            (2.75, 790.0),
            (2.8002, 800.0),
            (2.8002, 802.0),
            (2.8002, 801.0),
            (2.85, 811.0),
            (2.85, 809.0),
        ];
        let mean_x = readings.iter().map(|r| r.0).sum::<f64>() / 6.0;
        let mean_y = readings.iter().map(|r| r.1).sum::<f64>() / 6.0;
        let xx: f64 = readings.iter().map(|r| (r.0 - mean_x).powi(2)).sum();
        let xy: f64 = readings
            .iter()
            .map(|r| (r.0 - mean_x) * (r.1 - mean_y))
            .sum();
        let residual: f64 = readings
            .iter()
            .map(|r| (r.1 - mean_y - xy / xx * (r.0 - mean_x)).powi(2))
            .sum();
        let half = t95(4.0) * (residual / 4.0 / xx).sqrt();
        let trend = middle.velocity_per_unit.unwrap();
        assert_eq!(trend.readings, 6);
        close(trend.per_unit, xy / xx, 1e-9);
        close(trend.range.unwrap().high, xy / xx + half, 1e-6);
        // Two readings give a slope with no range.
        let first_pair = slope(&[(1.0, 800.0), (2.0, 810.0)]).unwrap();
        assert_eq!((first_pair.per_unit, first_pair.range), (10.0, None));
        assert_eq!(slope(&[(1.0, 800.0), (1.0, 810.0)]), None);
        // The first run has no flat stretch; a one-key ranking by
        // horizontal prefers the run with no sideways change.
        let first = ladder
            .runs
            .iter()
            .find(|r| r.rungs == 3 && r.first == 0)
            .unwrap();
        assert!(!first.flat);
        let across = Ladder::new(
            &groups,
            LadderOptions {
                key: LadderKey::Horizontal,
                ..LadderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(across.best[0].first, 2);
        assert_eq!(across.key, LadderKey::Horizontal);
    }

    #[test]
    fn noise_is_the_rms_of_unequal_rungs() {
        // σ̂ 0.5; rungs of 4, 1 and 4 shots: SEs 0.25, 0.5, 0.25, whose RMS
        // is √0.125 (their mean would be 1/3).
        let groups = [
            group(1.0, 0.0, 0.0, 4, 0.5),
            group(2.0, 0.0, 0.0, 1, f64::NAN),
            group(3.0, 0.0, 0.0, 4, 0.5),
        ];
        let ladder = Ladder::new(&groups, LadderOptions::default()).unwrap();
        close(ladder.runs[0].vertical_noise, 0.125f64.sqrt(), 1e-15);
        close(ladder.runs[0].horizontal_noise, 0.125f64.sqrt(), 1e-15);
    }

    #[test]
    fn equal_values_pool_with_no_tolerance() {
        let groups = [
            group(1.0, 0.0, 0.0, 3, 0.1),
            group(1.0, 0.2, 0.0, 3, 0.1),
            group(2.0, 0.0, 0.0, 3, 0.1),
            group(3.0, 0.0, 0.0, 3, 0.1),
        ];
        let options = LadderOptions {
            value_tolerance: 0.0,
            ..LadderOptions::default()
        };
        let ladder = Ladder::new(&groups, options).unwrap();
        assert_eq!(ladder.rungs.len(), 3);
        assert_eq!(ladder.rungs[0].groups, [0, 1]);
        close(ladder.rungs[0].vertical, 0.1, 1e-15);
        let negative = LadderOptions {
            value_tolerance: -0.1,
            ..LadderOptions::default()
        };
        assert_eq!(
            Ladder::new(&groups, negative),
            Err(AnalysisError::Negative("value tolerance"))
        );
    }

    #[test]
    fn the_default_ranking_prefers_flat_across_among_flat_runs() {
        // SE 0.25. Both runs of 3 are flat up and down; the first is
        // flatter up but wanders 4 SE across, the second holds still across.
        let groups = [
            group(1.0, 0.0, 1.0, 4, 0.5),
            group(2.0, 0.0, 0.0, 4, 0.5),
            group(3.0, 0.1, 0.0, 4, 0.5),
            group(4.0, 0.4, 0.0, 4, 0.5),
        ];
        let ladder = Ladder::new(&groups, LadderOptions::default()).unwrap();
        let threes: Vec<&LadderRun> = ladder.runs.iter().filter(|r| r.rungs == 3).collect();
        assert!(threes.iter().all(|run| run.flat));
        assert!(!threes[0].flat_across && threes[1].flat_across);
        assert!(threes[0].vertical_score < threes[1].vertical_score);
        assert_eq!(ladder.best[0].first, 1);
        let vertical = LadderOptions {
            key: LadderKey::Vertical,
            ..LadderOptions::default()
        };
        assert_eq!(Ladder::new(&groups, vertical).unwrap().best[0].first, 0);
        // Among runs flat both ways, the lower σ̂ comes first.
        let mut tight = groups.clone();
        tight[0].impact_mrad = Point::new(0.0, 0.0);
        tight[3].sigma_mrad = 0.3;
        let ladder = Ladder::new(&tight, LadderOptions::default()).unwrap();
        assert!(
            ladder
                .runs
                .iter()
                .filter(|r| r.rungs == 3)
                .all(|r| r.flat_across)
        );
        assert_eq!(ladder.best[0].first, 1);
    }

    #[test]
    fn known_sigma_pools_with_the_ladders_own() {
        let groups: Vec<LadderGroup> = (0..3).map(|i| group(i as f64, 0.0, 0.0, 3, 0.1)).collect();
        // Own: 0.1 on 12 df; known 0.2 on 12 df: √((0.12 + 0.48) / 24).
        let known = Sigma {
            value: 0.2,
            degrees_of_freedom: 12.0,
        };
        let options = LadderOptions {
            known_sigma: Some(known),
            ..LadderOptions::default()
        };
        let ladder = Ladder::new(&groups, options).unwrap();
        assert_eq!(ladder.noise_sigma.degrees_of_freedom, 24.0);
        close(ladder.noise_sigma.value, 0.025f64.sqrt(), 1e-15);
        let bad = LadderOptions {
            known_sigma: Some(Sigma {
                value: 0.1,
                degrees_of_freedom: 0.0,
            }),
            ..LadderOptions::default()
        };
        assert_eq!(Ladder::new(&groups, bad), Err(AnalysisError::NotFinite));
    }

    #[test]
    fn groups_from_their_statistics() {
        let shots = [Point::new(-10.0, 20.0), Point::new(10.0, 20.0)];
        let options = crate::Options {
            aim: Some(Point::new(0.0, 0.0)),
            distance_m: Some(100.0),
            ..crate::Options::default()
        };
        let stats = crate::GroupStats::new(&shots, options).unwrap();
        let group = LadderGroup::from_stats(&stats, 2.8, vec![800.0]).unwrap();
        assert_eq!(group.impact_mrad, Point::new(0.0, 0.2));
        close(
            group.sigma_mrad,
            stats.sigma().unwrap().value / 100.0,
            1e-15,
        );
        assert_eq!((group.value, group.shots), (2.8, 2));
        let no_aim = crate::GroupStats::new(&shots, crate::Options::default()).unwrap();
        assert_eq!(
            LadderGroup::from_stats(&no_aim, 2.8, Vec::new()),
            Err(AnalysisError::MissingAim)
        );
        let no_distance = crate::GroupStats::new(
            &shots,
            crate::Options {
                distance_m: None,
                ..options
            },
        )
        .unwrap();
        assert_eq!(
            LadderGroup::from_stats(&no_distance, 2.8, Vec::new()),
            Err(AnalysisError::MissingDistance)
        );
    }

    #[test]
    fn sorting_does_not_depend_on_input_order() {
        let groups: Vec<LadderGroup> = (0..6)
            .map(|i| {
                group(
                    2.6 + 0.05 * i as f64,
                    (i as f64 * 0.7).sin() * 0.2,
                    0.0,
                    3,
                    0.12,
                )
            })
            .collect();
        let forward = Ladder::new(&groups, LadderOptions::default()).unwrap();
        let reversed: Vec<LadderGroup> = groups.iter().rev().cloned().collect();
        let backward = Ladder::new(&reversed, LadderOptions::default()).unwrap();
        for (a, b) in forward.runs.iter().zip(&backward.runs) {
            close(a.vertical_score, b.vertical_score, 1e-12);
        }
        assert_eq!(forward.best.len(), backward.best.len());
        for (a, b) in forward.best.iter().zip(&backward.best) {
            assert_eq!((a.first, a.rungs), (b.first, b.rungs));
        }
        // Moving every impact the same way changes no score.
        let moved: Vec<LadderGroup> = groups
            .iter()
            .map(|g| LadderGroup {
                impact_mrad: g.impact_mrad + Point::new(0.3, -1.0),
                ..g.clone()
            })
            .collect();
        let moved = Ladder::new(&moved, LadderOptions::default()).unwrap();
        for (a, b) in forward.runs.iter().zip(&moved.runs) {
            close(a.vertical_score, b.vertical_score, 1e-9);
        }
    }

    #[test]
    fn single_shot_ladders_need_a_known_sigma() {
        let groups: Vec<LadderGroup> = (0..4)
            .map(|i| group(2.6 + 0.05 * i as f64, 0.01 * i as f64, 0.0, 1, f64::NAN))
            .collect();
        assert_eq!(
            Ladder::new(&groups, LadderOptions::default()),
            Err(AnalysisError::NoSpreadEstimate)
        );
        let known = Sigma {
            value: 0.1,
            degrees_of_freedom: 20.0,
        };
        let ladder = Ladder::new(
            &groups,
            LadderOptions {
                known_sigma: Some(known),
                ..LadderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(ladder.noise_sigma, known);
        close(ladder.rungs[0].vertical_standard_error, 0.1, 1e-15);
        assert!(ladder.runs.iter().all(|run| run.sigma.is_none()));
        // A Sigma key with no σ̂ anywhere still ranks, without a panic.
        let by_sigma = Ladder::new(
            &groups,
            LadderOptions {
                known_sigma: Some(known),
                key: LadderKey::Sigma,
                ..LadderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(by_sigma.best.len(), 2);
    }

    #[test]
    fn bad_inputs_are_refused() {
        let good = |value: f64| group(value, 0.0, 0.0, 3, 0.1);
        assert_eq!(
            Ladder::new(&[], LadderOptions::default()),
            Err(AnalysisError::Empty)
        );
        assert_eq!(
            Ladder::new(
                &[good(1.0), good(1.0005), good(2.0)],
                LadderOptions::default()
            ),
            Err(AnalysisError::TooFew {
                what: "rungs",
                need: 3,
                have: 2
            })
        );
        let mut bad = good(f64::NAN);
        assert!(matches!(
            Ladder::new(&[good(1.0), bad.clone()], LadderOptions::default()),
            Err(AnalysisError::Input { index: 1, .. })
        ));
        bad = good(1.5);
        bad.velocities_mps = vec![800.0, 8000.0];
        assert!(matches!(
            Ladder::new(&[good(1.0), bad], LadderOptions::default()),
            Err(AnalysisError::Input {
                index: 1,
                error: StatsError::ImplausibleVelocity { index: 1, .. }
            })
        ));
        let options = LadderOptions {
            value_tolerance: f64::NAN,
            ..LadderOptions::default()
        };
        assert!(Ladder::new(&[good(1.0)], options).is_err());
        // Overflow is refused; zero noise gives infinite scores, no panic.
        let huge = [
            group(1.0, 1e308, 0.0, 3, 0.1),
            group(2.0, -1e308, 0.0, 3, 0.1),
            group(3.0, 0.0, 0.0, 3, 0.1),
        ];
        assert_eq!(
            Ladder::new(&huge, LadderOptions::default()),
            Err(AnalysisError::NotFinite)
        );
        let still = [
            group(1.0, 0.0, 0.0, 3, 0.0),
            group(2.0, 0.1, 0.0, 3, 0.0),
            group(3.0, 0.1, 0.0, 3, 0.0),
        ];
        let ladder = Ladder::new(&still, LadderOptions::default()).unwrap();
        assert_eq!(ladder.runs[0].vertical_score, f64::INFINITY);
        assert_eq!(ladder.runs[0].horizontal_score, 0.0);
    }

    /// The share of runs of `rungs` rungs of `shots` shots, with σ = 1 and
    /// a vertical trend of `trend` noise per step, called flat, with σ̂
    /// estimated from the shots.
    fn flat_rate(rng: &mut Rng, rungs: usize, shots: usize, trend: f64, trials: usize) -> f64 {
        let noise = 1.0 / (shots as f64).sqrt();
        let mut flat = 0;
        for _ in 0..trials {
            let groups: Vec<LadderGroup> = (0..rungs)
                .map(|i| {
                    let centre = Point::new(0.0, trend * noise * i as f64);
                    let points: Vec<Point> = (0..shots).map(|_| rng.shot(centre, 1.0)).collect();
                    let stats = crate::GroupStats::new(&points, crate::Options::default()).unwrap();
                    LadderGroup {
                        value: i as f64,
                        impact_mrad: stats.centre,
                        shots,
                        sigma_mrad: stats.sigma().unwrap().value,
                        velocities_mps: Vec::new(),
                    }
                })
                .collect();
            let ladder = Ladder::new(&groups, LadderOptions::default()).unwrap();
            if ladder.runs.last().unwrap().flat {
                flat += 1;
            }
        }
        flat as f64 / trials as f64
    }

    #[test]
    fn flat_limits() {
        // Two rungs: the range is |N(0, 2)|, so √2 Φ⁻¹(5/6).
        close(
            flat_limit(2),
            2f64.sqrt() * crate::distributions::normal_quantile(5.0 / 6.0),
            1e-9,
        );
        // As simulated with 400,000 sets each: 2.000, 2.380, 2.643, 2.850.
        for (rungs, limit) in [(3, 2.0005), (4, 2.3783), (5, 2.6453), (6, 2.8503)] {
            close(flat_limit(rungs), limit, 1e-4);
        }
        assert!(flat_limit(41) > flat_limit(40) && flat_limit(1).is_nan());
        // With the noise known, the range of truly flat runs is within the
        // limit two times in three, whatever the length.
        let mut rng = Rng::new(8);
        for rungs in [3, 5, 8] {
            let trials = 20_000;
            let passed = (0..trials)
                .filter(|_| {
                    let values: Vec<f64> = (0..rungs).map(|_| rng.normal()).collect();
                    let (low, high) = values
                        .iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| {
                            (l.min(v), h.max(v))
                        });
                    high - low <= flat_limit(rungs)
                })
                .count();
            let rate = passed as f64 / trials as f64;
            assert!((rate - FLAT_SHARE).abs() < 0.014, "{rungs} rungs: {rate}");
        }
    }

    /// The numbers in `docs/group-statistics.md`: `cargo test --release
    /// -p groups-core -- --ignored --nocapture flat_limit_table`.
    #[test]
    #[ignore]
    fn flat_limit_table() {
        let mut rng = Rng::new(2026);
        for shots in [3, 5] {
            for rungs in [3, 4, 5, 6] {
                let rates: Vec<String> = [0.0, 0.5, 1.0, 2.0]
                    .iter()
                    .map(|&trend| {
                        format!(
                            "{:.0}%",
                            100.0 * flat_rate(&mut rng, rungs, shots, trend, 10_000)
                        )
                    })
                    .collect();
                println!(
                    "{shots} shots, {rungs} rungs, limit {:.3}: flat at a climb of 0/0.5/1/2 noise per step: {}",
                    flat_limit(rungs),
                    rates.join(" / ")
                );
            }
        }
        // An 8-rung ladder with no node at all: a steady climb of `trend`
        // noise per step. How often does it show a flat 3-rung run?
        for shots in [3, 5] {
            for trend in [0.0, 0.5, 1.0, 1.5, 2.0] {
                let trials = 4_000;
                let noise = 1.0 / (shots as f64).sqrt();
                let mut found = 0;
                for _ in 0..trials {
                    let groups: Vec<LadderGroup> = (0..8)
                        .map(|i| {
                            let centre = Point::new(0.0, trend * noise * i as f64);
                            let points: Vec<Point> =
                                (0..shots).map(|_| rng.shot(centre, 1.0)).collect();
                            let stats =
                                crate::GroupStats::new(&points, crate::Options::default()).unwrap();
                            LadderGroup {
                                value: i as f64,
                                impact_mrad: stats.centre,
                                shots,
                                sigma_mrad: stats.sigma().unwrap().value,
                                velocities_mps: Vec::new(),
                            }
                        })
                        .collect();
                    let ladder = Ladder::new(&groups, LadderOptions::default()).unwrap();
                    if ladder.best[0].flat {
                        found += 1;
                    }
                }
                println!(
                    "8 rungs of {shots}, steady climb of {trend} noise/step: a flat 3-rung run {:.0}% of the time",
                    100.0 * found as f64 / trials as f64
                );
            }
        }
    }

    #[test]
    fn what_flat_means() {
        let mut rng = Rng::new(5);
        // Truly flat runs of 3-shot rungs, σ̂ estimated, pass about 64% of
        // the time at any length; a climb of 2 noise per step seldom does.
        for rungs in [3, 5] {
            let flat = flat_rate(&mut rng, rungs, 3, 0.0, 2_000);
            assert!((0.58..0.70).contains(&flat), "{rungs}: flat {flat}");
        }
        let two = flat_rate(&mut rng, 3, 3, 2.0, 2_000);
        assert!(two < 0.1, "two {two}");
    }
}

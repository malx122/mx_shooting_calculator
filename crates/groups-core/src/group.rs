//! The statistics of a group of shots on the target.
//!
//! The model behind every range and every derived radius is the usual one
//! for rifle groups: shots scatter round a centre as a normal
//! distribution with the same standard deviation σ across and up. All
//! lengths are millimetres on the target, x to the right and y up (see
//! [`geometry`](crate::geometry)).
//!
//! The impact is where the group's centre is from the aim point: centre −
//! aim, so a group above and right of the aim has positive x and y. The
//! correction is the opposite, aim − centre: how far to move the group to
//! put it on the aim, up and right positive.

use std::fmt;
use std::ops::Add;

use crate::distributions::{chi_squared_quantile, f_quantile, t_quantile};
use crate::geometry::{Point, centroid, farthest_pair};
use crate::hit::ShotModel;

/// The confidence of every range this module gives.
pub const CONFIDENCE: f64 = 0.95;

/// The mean distance from the centre of a round normal spread, in σ:
/// √(π/2).
pub const MEAN_RADIUS_IN_SIGMAS: f64 = 1.253_314_137_315_500_3;

/// Ellipticity needs at least this many degrees of freedom per axis (5
/// shots in one group) before a group can count as elliptical.
pub const ELLIPTICAL_MIN_AXIS_DEGREES: usize = 4;

/// A group never counts as elliptical below this ratio of the SDs along
/// its longest and shortest axes, however many shots it has.
pub const ELLIPTICAL_MIN_RATIO: f64 = 1.5;

/// How often a round group may count as elliptical by chance.
pub const ELLIPTICAL_FALSE_ALARM: f64 = 0.05;

/// The fewest shots the flier check looks at.
pub const FLIER_MIN_SHOTS: usize = 5;

/// A flier lies beyond the radius that would hold this share of further
/// shots from the same spread as the rest (their R99).
pub const FLIER_REST_SHARE: f64 = 0.99;

/// Leaving a flier out cuts σ̂ by at least this fraction.
pub const FLIER_MIN_SIGMA_CUT: f64 = 0.25;

/// The expected extreme spread of a group of n shots from a round normal
/// spread, in σ (per axis), for n = 2 to 50.
///
/// From a fixed-seed Monte Carlo of ten million strings of shots, good to
/// about ±0.001. A test checks it against 20,000 strings; `cargo test
/// --release -p groups-core -- --ignored --nocapture
/// expected_extreme_spread` checks it against the ten million and prints
/// it.
pub const EXPECTED_EXTREME_SPREAD_IN_SIGMAS: [f64; 49] = [
    1.772, 2.409, 2.794, 3.066, 3.275, 3.444, 3.585, 3.706, 3.811, 3.905, // 2–11
    3.988, 4.064, 4.133, 4.196, 4.254, 4.308, 4.359, 4.406, 4.451, 4.493, // 12–21
    4.533, 4.570, 4.606, 4.640, 4.673, 4.704, 4.734, 4.763, 4.790, 4.817, // 22–31
    4.842, 4.867, 4.890, 4.913, 4.935, 4.957, 4.978, 4.998, 5.018, 5.037, // 32–41
    5.056, 5.074, 5.091, 5.108, 5.125, 5.142, 5.157, 5.173, 5.188, // 42–50
];

/// The expected extreme spread of `shots` shots in σ, or `None` outside
/// 2 to 50 shots.
pub fn expected_extreme_spread_in_sigmas(shots: usize) -> Option<f64> {
    EXPECTED_EXTREME_SPREAD_IN_SIGMAS
        .get(shots.checked_sub(2)?)
        .copied()
}

/// The radius holding a share `p` of a round normal spread, in σ:
/// √(−2 ln(1 − p)). CEP (p = 0.5) is 1.1774σ, R90 2.1460σ, R95 2.4477σ.
pub fn circular_radius_in_sigmas(p: f64) -> f64 {
    (-2.0 * (-p).ln_1p()).sqrt()
}

/// The radius holding a share `p` of an elliptical normal spread with
/// variances `var_x`, `var_y` and covariance `cov` (mm²), by Grubbs's
/// approximation (Grubbs 1964, after Patnaik 1949): the squared distance
/// from the centre is taken as a scaled χ² with the same mean and
/// variance. Exact for a round spread and for shots on a line.
pub fn grubbs_radius(var_x: f64, var_y: f64, cov: f64, p: f64) -> f64 {
    let mean = var_x + var_y;
    let variance = 2.0 * (var_x * var_x + var_y * var_y + 2.0 * cov * cov);
    if mean.is_nan() || variance.is_nan() || mean <= 0.0 || variance <= 0.0 {
        return 0.0;
    }
    let scale = variance / (2.0 * mean);
    let degrees = 2.0 * mean * mean / variance;
    (scale * chi_squared_quantile(p, degrees)).sqrt()
}

/// Why the statistics can't be worked out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatsError {
    /// There are no shots (or no readings).
    NoShots,
    /// A shot, aim point, reading or size is not a finite number.
    NotFinite,
    /// The calibre or distance is zero or negative.
    NotPositive(&'static str),
    /// The statistic needs more shots.
    TooFewShots { need: usize, have: usize },
    /// A chronograph reading (m/s) no chronograph can have made; the
    /// readings are counted from 0.
    ImplausibleVelocity { index: usize, mps: f64 },
    /// Every shot has the same velocity, so nothing can be fitted to it.
    NoVelocitySpread,
    /// The shots and their velocities are not as many.
    LengthMismatch,
}

impl fmt::Display for StatsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsError::NoShots => formatter.write_str("there are no shots"),
            StatsError::NotFinite => formatter.write_str("a shot or size is not a number"),
            StatsError::NotPositive(what) => write!(formatter, "the {what} must be more than zero"),
            StatsError::TooFewShots { need, have } => {
                write!(formatter, "needs {need} shots, has {have}")
            }
            StatsError::ImplausibleVelocity { index, mps } => {
                write!(
                    formatter,
                    "reading {} ({mps} m/s) is not a velocity",
                    index + 1
                )
            }
            StatsError::NoVelocitySpread => {
                formatter.write_str("all the shots have the same velocity")
            }
            StatsError::LengthMismatch => {
                formatter.write_str("the shots and velocities don't match")
            }
        }
    }
}

impl std::error::Error for StatsError {}

/// A range of values, low to high.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

impl Interval {
    /// Both ends times `factor` (which must not be negative).
    pub fn scaled(self, factor: f64) -> Interval {
        Interval {
            low: self.low * factor,
            high: self.high * factor,
        }
    }

    /// Both ends plus `offset`.
    pub fn shifted(self, offset: f64) -> Interval {
        Interval {
            low: self.low + offset,
            high: self.high + offset,
        }
    }

    pub fn contains(self, value: f64) -> bool {
        (self.low..=self.high).contains(&value)
    }
}

/// A measured value with its 95% range: where the true value (the one
/// many more shots would settle on) lies, with 95% confidence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub value: f64,
    pub range: Interval,
}

/// An estimate σ̂ of σ, the standard deviation per axis of a round normal
/// spread, with the degrees of freedom it rests on: 2(n − 1) for one
/// group of n shots about its own centre. `degrees_of_freedom` × σ̂² / σ²
/// follows χ² with those degrees of freedom, which gives its range and,
/// later, the F test between two groups.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sigma {
    /// σ̂, mm.
    pub value: f64,
    pub degrees_of_freedom: f64,
}

impl Sigma {
    /// The range of σ at `confidence` (0.95 for 95%), from χ².
    pub fn range(&self, confidence: f64) -> Interval {
        let tail = 0.5 * (1.0 - confidence);
        let degrees = self.degrees_of_freedom;
        Interval {
            low: self.value * (degrees / chi_squared_quantile(1.0 - tail, degrees)).sqrt(),
            high: self.value * (degrees / chi_squared_quantile(tail, degrees)).sqrt(),
        }
    }

    /// σ̂ with its 95% range.
    pub fn estimate(&self) -> Estimate {
        Estimate {
            value: self.value,
            range: self.range(CONFIDENCE),
        }
    }
}

/// The sums of squares of shots about their centres, which every spread
/// estimate is made from. Scatters add up, so groups pool by adding
/// theirs (each about its own centre).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scatter {
    /// How many shots.
    pub shots: usize,
    /// How many centres were worked out from these same shots: 1 for a
    /// group about its own centre, 0 about a point known beforehand, k
    /// for k groups pooled each about its own centre.
    pub centres: usize,
    /// Σ (x − cx)², mm².
    pub xx: f64,
    /// Σ (y − cy)², mm².
    pub yy: f64,
    /// Σ (x − cx)(y − cy), mm².
    pub xy: f64,
}

impl Scatter {
    /// The scatter of `shots` about `centre`, which was worked out from
    /// them (their mean) if `centre_from_shots`.
    pub fn about(shots: &[Point], centre: Point, centre_from_shots: bool) -> Scatter {
        let mut scatter = Scatter {
            shots: shots.len(),
            centres: usize::from(centre_from_shots),
            ..Scatter::default()
        };
        for &shot in shots {
            let d = shot - centre;
            scatter.xx += d.x * d.x;
            scatter.yy += d.y * d.y;
            scatter.xy += d.x * d.y;
        }
        scatter
    }

    /// Degrees of freedom per axis: shots less centres.
    pub fn axis_degrees_of_freedom(&self) -> usize {
        self.shots.saturating_sub(self.centres)
    }

    /// The radial estimate σ̂² = Σ|pᵢ − c|² / (2 × axis degrees of
    /// freedom), or `None` without a degree of freedom.
    pub fn sigma(&self) -> Option<Sigma> {
        let degrees = self.axis_degrees_of_freedom();
        if degrees == 0 {
            return None;
        }
        let degrees = 2.0 * degrees as f64;
        Some(Sigma {
            value: ((self.xx + self.yy) / degrees).sqrt(),
            degrees_of_freedom: degrees,
        })
    }

    /// The sample standard deviation across (n − 1 for one group).
    pub fn sd_horizontal(&self) -> Option<f64> {
        let degrees = self.axis_degrees_of_freedom();
        (degrees > 0).then(|| (self.xx / degrees as f64).sqrt())
    }

    /// The sample standard deviation up and down.
    pub fn sd_vertical(&self) -> Option<f64> {
        let degrees = self.axis_degrees_of_freedom();
        (degrees > 0).then(|| (self.yy / degrees as f64).sqrt())
    }

    /// The correlation between across and up, −1 to 1; 0 when either
    /// axis has no spread.
    pub fn correlation(&self) -> f64 {
        if self.xx > 0.0 && self.yy > 0.0 {
            (self.xy / (self.xx * self.yy).sqrt()).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// The same scatter with every length times `factor`: 1 / distance_m
    /// turns millimetres into milliradians, so groups shot at different
    /// distances pool as angles.
    pub fn scaled(&self, factor: f64) -> Scatter {
        let squared = factor * factor;
        Scatter {
            xx: self.xx * squared,
            yy: self.yy * squared,
            xy: self.xy * squared,
            ..*self
        }
    }

    fn is_finite(&self) -> bool {
        self.xx.is_finite()
            && self.yy.is_finite()
            && self.xy.is_finite()
            && self.xx >= 0.0
            && self.yy >= 0.0
    }
}

impl Add for Scatter {
    type Output = Scatter;
    fn add(self, other: Scatter) -> Scatter {
        Scatter {
            shots: self.shots + other.shots,
            centres: self.centres + other.centres,
            xx: self.xx + other.xx,
            yy: self.yy + other.yy,
            xy: self.xy + other.xy,
        }
    }
}

/// What the statistics may use besides the shots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    /// The aim point on the target, mm.
    pub aim: Option<Point>,
    /// The bullet's diameter, mm, for edge-to-edge sizes.
    pub calibre_mm: Option<f64>,
    /// The distance from the muzzle to the target, m, for angles.
    pub distance_m: Option<f64>,
}

impl Options {
    fn check(&self) -> Result<(), StatsError> {
        if self.aim.is_some_and(|aim| !aim.is_finite()) {
            return Err(StatsError::NotFinite);
        }
        for (value, what) in [(self.calibre_mm, "calibre"), (self.distance_m, "distance")] {
            match value {
                Some(value) if !value.is_finite() => return Err(StatsError::NotFinite),
                Some(value) if value <= 0.0 => return Err(StatsError::NotPositive(what)),
                _ => {}
            }
        }
        Ok(())
    }
}

/// How CEP, R90 and R95 were worked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadiusMethod {
    /// From σ̂ of a round spread: R_p = σ̂ √(−2 ln(1 − p)).
    Circular,
    /// By Grubbs's approximation for an elliptical spread, because the
    /// group is clearly elliptical (see [`Shape`]).
    Grubbs,
}

/// How round the group is, judged on its principal axes (the directions
/// in which it is longest and shortest), so turning the target changes
/// nothing but the angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// The SD along the longest axis over the SD along the shortest: 1 for
    /// a perfectly round group, infinite for shots on one line.
    pub ratio: f64,
    /// The direction of the longest axis, degrees anticlockwise from the
    /// right, −90 to 90; 0 for a perfectly round group.
    pub angle_degrees: f64,
    /// The ratio at and beyond which the group counts as elliptical: the
    /// larger of [`ELLIPTICAL_MIN_RATIO`] and the ratio a round group
    /// reaches by chance only [`ELLIPTICAL_FALSE_ALARM`] of the time.
    /// `None` for too few shots to tell.
    pub limit: Option<f64>,
    /// Whether the group is clearly elliptical.
    pub elliptical: bool,
}

impl Shape {
    /// The shape of the shots behind `scatter`.
    pub fn of(scatter: &Scatter) -> Shape {
        let (xx, yy, xy) = (scatter.xx, scatter.yy, scatter.xy);
        let difference = (xx - yy).hypot(2.0 * xy);
        let major = 0.5 * (xx + yy + difference);
        let minor = if major > 0.0 {
            (xx * yy - xy * xy).max(0.0) / major
        } else {
            0.0
        };
        let ratio = if minor > 0.0 {
            (major / minor).sqrt()
        } else if major > 0.0 {
            f64::INFINITY
        } else {
            1.0
        };
        let degrees = scatter.axis_degrees_of_freedom();
        let limit = (degrees >= ELLIPTICAL_MIN_AXIS_DEGREES).then(|| {
            // ((λ₁ − λ₂) / (λ₁ + λ₂))² of a round group follows
            // Beta(1, (ν − 1)/2), so its upper tail is (1 − x)^((ν − 1)/2).
            let exponent = 2.0 / (degrees as f64 - 1.0);
            let r = (1.0 - ELLIPTICAL_FALSE_ALARM.powf(exponent)).sqrt();
            ELLIPTICAL_MIN_RATIO.max(((1.0 + r) / (1.0 - r)).sqrt())
        });
        Shape {
            ratio,
            angle_degrees: (0.5 * (2.0 * xy).atan2(xx - yy)).to_degrees(),
            limit,
            elliptical: limit.is_some_and(|limit| ratio >= limit),
        }
    }
}

/// The two shots furthest apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtremeSpread {
    /// Centre to centre of the two holes, mm.
    pub centre_to_centre: f64,
    /// The two shots, by index, the lower first.
    pub shots: (usize, usize),
    /// Outside edge to outside edge: centre to centre plus a calibre;
    /// `None` without a calibre.
    pub edge_to_edge: Option<f64>,
    /// The 95% range of the centre-to-centre spread to expect from groups
    /// of this many shots: σ's range times the expected spread in σ.
    /// `None` beyond 50 shots.
    pub range: Option<Interval>,
}

/// Everything that needs two shots or more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub extreme_spread: ExtremeSpread,
    /// σ̂, mm, the best single measure of precision, with its range.
    pub sigma: Estimate,
    /// The range of the mean radius many more shots would settle on:
    /// σ's range times √(π/2).
    pub mean_radius_range: Interval,
    /// How CEP, R90 and R95 were worked out.
    pub radius_method: RadiusMethod,
    /// The circle round the centre holding half the shots, mm.
    pub cep: Estimate,
    /// Holding 90% of the shots, mm.
    pub r90: Estimate,
    /// Holding 95% of the shots, mm.
    pub r95: Estimate,
    /// The sample standard deviation across, mm (n − 1).
    pub sd_horizontal: f64,
    /// The sample standard deviation up and down, mm (n − 1).
    pub sd_vertical: f64,
    /// The correlation between across and up, −1 to 1.
    pub correlation: f64,
    pub shape: Shape,
    /// The standard error of the centre, mm, [across, up]: σ̂ / √n on
    /// each axis, or SD-H / √n and SD-V / √n for an elliptical group.
    pub centre_standard_error: [f64; 2],
    /// The degrees of freedom behind `centre_standard_error`, for
    /// Student's t: 2(n − 1), or n − 1 for an elliptical group.
    pub centre_degrees_of_freedom: f64,
}

/// The statistics of a group of shots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupStats {
    /// How many shots.
    pub shots: usize,
    /// The mean of the shots, mm.
    pub centre: Point,
    /// centre − aim, mm, when there is an aim point.
    pub impact: Option<Point>,
    /// The mean distance of the shots from the centre, mm.
    pub mean_radius: f64,
    /// The median distance of the shots from the centre, mm.
    pub median_radius: f64,
    /// Furthest left to furthest right, centre to centre, mm.
    pub width: f64,
    /// Lowest to highest, centre to centre, mm.
    pub height: f64,
    /// The sums every spread estimate is made from.
    pub scatter: Scatter,
    /// `None` for a single shot.
    pub spread: Option<Spread>,
    pub options: Options,
}

impl GroupStats {
    /// The statistics of `shots` (mm) about their own centre.
    pub fn new(shots: &[Point], options: Options) -> Result<GroupStats, StatsError> {
        check_shots(shots)?;
        let centre = centroid(shots).ok_or(StatsError::NoShots)?;
        GroupStats::with_scatter(shots, centre, Scatter::about(shots, centre, true), options)
    }

    /// The statistics of `shots` about `centre`, with the spread
    /// estimates made from `scatter` instead of from these shots alone:
    /// for shots moved together from several groups (with their pooled
    /// scatter), or for shots about a point known beforehand.
    ///
    /// The caller keeps it consistent: `scatter` must count these shots
    /// (or it is refused) and be in the same units; the radii, width and
    /// height are about `centre` as given; and the extreme spread's range
    /// and the centre's standard error assume one group of this many
    /// shots, which a composite of several groups is not.
    pub fn with_scatter(
        shots: &[Point],
        centre: Point,
        scatter: Scatter,
        options: Options,
    ) -> Result<GroupStats, StatsError> {
        check_shots(shots)?;
        options.check()?;
        if !centre.is_finite() || !scatter.is_finite() {
            return Err(StatsError::NotFinite);
        }
        if scatter.shots != shots.len() {
            return Err(StatsError::LengthMismatch);
        }
        let mut radii: Vec<f64> = shots.iter().map(|shot| shot.distance(centre)).collect();
        radii.sort_by(f64::total_cmp);
        let middle = radii.len() / 2;
        let median_radius = if radii.len() % 2 == 1 {
            radii[middle]
        } else {
            0.5 * (radii[middle - 1] + radii[middle])
        };
        let extent = |coordinate: fn(&Point) -> f64| {
            let (low, high) = shots
                .iter()
                .map(coordinate)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), v| {
                    (low.min(v), high.max(v))
                });
            high - low
        };
        let stats = GroupStats {
            shots: shots.len(),
            centre,
            impact: options.aim.map(|aim| centre - aim),
            mean_radius: radii.iter().sum::<f64>() / radii.len() as f64,
            median_radius,
            width: extent(|p| p.x),
            height: extent(|p| p.y),
            scatter,
            spread: spread(shots, scatter, options),
            options,
        };
        let sizes = [stats.mean_radius, stats.width, stats.height];
        let spread_sizes = stats.spread.map_or([0.0; 2], |s| {
            [s.extreme_spread.centre_to_centre, s.sigma.range.high]
        });
        if sizes
            .iter()
            .chain(&spread_sizes)
            .any(|size| !size.is_finite())
            || stats.impact.is_some_and(|impact| !impact.is_finite())
        {
            return Err(StatsError::NotFinite);
        }
        Ok(stats)
    }

    /// The centre with its 95% range on each axis, [across, up], from
    /// Student's t. `None` for one shot.
    pub fn centre_estimate(&self) -> Option<[Estimate; 2]> {
        self.estimate_about(self.centre)
    }

    /// The impact (centre − aim) with its 95% range on each axis,
    /// [across, up]. `None` without an aim point or for one shot.
    pub fn impact_estimate(&self) -> Option<[Estimate; 2]> {
        self.estimate_about(self.impact?)
    }

    fn estimate_about(&self, point: Point) -> Option<[Estimate; 2]> {
        let spread = self.spread?;
        let t = t_quantile(0.5 + 0.5 * CONFIDENCE, spread.centre_degrees_of_freedom);
        let axis = |value: f64, error: f64| Estimate {
            value,
            range: Interval {
                low: value - t * error,
                high: value + t * error,
            },
        };
        let [across, up] = spread.centre_standard_error;
        Some([axis(point.x, across), axis(point.y, up)])
    }

    /// aim − centre, mm: how far to move the group to put it on the aim,
    /// up and right positive.
    pub fn correction(&self) -> Option<Point> {
        self.impact.map(|impact| -impact)
    }

    /// σ̂ with its degrees of freedom, for comparing with other groups.
    pub fn sigma(&self) -> Option<Sigma> {
        self.scatter.sigma()
    }

    /// σ̂ as an angle at the group's distance, mrad, with its degrees of
    /// freedom. `None` without a distance or for one shot.
    pub fn sigma_mrad(&self) -> Option<Sigma> {
        let sigma = self.sigma()?;
        Some(Sigma {
            value: self.mrad(sigma.value)?,
            ..sigma
        })
    }

    /// `mm` on the target as an angle at the group's distance, mrad.
    pub fn mrad(&self, mm: f64) -> Option<f64> {
        self.options
            .distance_m
            .map(|distance| crate::angles::mrad(mm, distance))
    }

    /// The spread the hit chances come from: round with σ̂, or the
    /// group's own ellipse when it is clearly elliptical. `None` for one
    /// shot.
    pub fn model(&self) -> Option<ShotModel> {
        let spread = self.spread?;
        Some(match spread.radius_method {
            RadiusMethod::Circular => ShotModel::circular(self.centre, spread.sigma.value),
            RadiusMethod::Grubbs => ShotModel {
                centre: self.centre,
                sd_x: spread.sd_horizontal,
                sd_y: spread.sd_vertical,
                correlation: spread.correlation,
            },
        })
    }

    /// The chance (0 to 1) that a shot lands within `radius` (mm) of
    /// `centre`: the group's own centre, or the aim point to count its
    /// offset in. `None` for one shot.
    pub fn chance_in_circle(&self, centre: Point, radius: f64) -> Option<f64> {
        Some(self.model()?.chance_in_circle(centre, radius))
    }

    /// The chance (0 to 1) that a shot lands in the axis-aligned
    /// rectangle with corners `min` and `max` (mm). `None` for one shot.
    pub fn chance_in_rectangle(&self, min: Point, max: Point) -> Option<f64> {
        Some(self.model()?.chance_in_rectangle(min, max))
    }
}

fn check_shots(shots: &[Point]) -> Result<(), StatsError> {
    if shots.is_empty() {
        return Err(StatsError::NoShots);
    }
    if shots.iter().any(|shot| !shot.is_finite()) {
        return Err(StatsError::NotFinite);
    }
    Ok(())
}

fn spread(shots: &[Point], scatter: Scatter, options: Options) -> Option<Spread> {
    let ((first, second), centre_to_centre) = farthest_pair(shots)?;
    let sigma = scatter.sigma()?;
    let sigma_range = sigma.range(CONFIDENCE);
    let axis_degrees = scatter.axis_degrees_of_freedom();
    let sd_horizontal = scatter.sd_horizontal()?;
    let sd_vertical = scatter.sd_vertical()?;
    let correlation = scatter.correlation();
    let shape = Shape::of(&scatter);
    let radius_method = if shape.elliptical {
        RadiusMethod::Grubbs
    } else {
        RadiusMethod::Circular
    };
    let root_n = (shots.len() as f64).sqrt();
    let (centre_standard_error, centre_degrees_of_freedom) = match radius_method {
        RadiusMethod::Circular => ([sigma.value / root_n; 2], sigma.degrees_of_freedom),
        RadiusMethod::Grubbs => (
            [sd_horizontal / root_n, sd_vertical / root_n],
            axis_degrees as f64,
        ),
    };
    let radius = |p: f64| {
        let value = match radius_method {
            RadiusMethod::Circular => sigma.value * circular_radius_in_sigmas(p),
            RadiusMethod::Grubbs => {
                let degrees = axis_degrees as f64;
                grubbs_radius(
                    scatter.xx / degrees,
                    scatter.yy / degrees,
                    scatter.xy / degrees,
                    p,
                )
            }
        };
        let in_sigmas = if sigma.value > 0.0 {
            value / sigma.value
        } else {
            circular_radius_in_sigmas(p)
        };
        Estimate {
            value,
            range: sigma_range.scaled(in_sigmas),
        }
    };
    Some(Spread {
        extreme_spread: ExtremeSpread {
            centre_to_centre,
            shots: (first, second),
            edge_to_edge: options.calibre_mm.map(|calibre| centre_to_centre + calibre),
            range: expected_extreme_spread_in_sigmas(shots.len())
                .map(|in_sigmas| sigma_range.scaled(in_sigmas)),
        },
        sigma: Estimate {
            value: sigma.value,
            range: sigma_range,
        },
        mean_radius_range: sigma_range.scaled(MEAN_RADIUS_IN_SIGMAS),
        radius_method,
        cep: radius(0.5),
        r90: radius(0.9),
        r95: radius(0.95),
        sd_horizontal,
        sd_vertical,
        correlation,
        shape,
        centre_standard_error,
        centre_degrees_of_freedom,
    })
}

/// How the flier check judged how far out a shot lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlierTest {
    /// A round group: the shot's distance from the other shots' centre in
    /// their σ̂, against F(2, 2n − 4).
    Round,
    /// A clearly elliptical group: the shot's Mahalanobis distance from
    /// the other shots' own ellipse, against Hotelling's F(2, n − 3).
    Elliptical,
}

/// A shot the flier check suggests leaving out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flier {
    /// The shot, by index.
    pub shot: usize,
    /// Its distance from the centre of the other shots, mm.
    pub distance_mm: f64,
    /// How far out a further shot from the other shots' spread lands at
    /// most 99% of the time, in this shot's direction, mm: their R99,
    /// allowing for their centre and spread being only estimates.
    pub rest_radius_mm: f64,
    /// How much leaving it out cuts σ̂: 0.25 is a quarter.
    pub sigma_cut: f64,
    /// Which test judged it.
    pub test: FlierTest,
    /// The statistics of the group without it.
    pub without: GroupStats,
}

/// The shot that most stands out from the others, if it stands out
/// enough to suggest it was a flier; it is never left out of anything
/// unless the user says so.
///
/// For a group of at least [`FLIER_MIN_SHOTS`], each shot is set against
/// the centre and spread of the other shots: in their σ̂ for a round
/// group, or against their own ellipse when the group is clearly
/// elliptical (see [`FlierTest`]). The one furthest out is suggested if it
/// lies beyond the rest's R99 (see [`Flier::rest_radius_mm`]) and leaving
/// it out cuts σ̂ by at least [`FLIER_MIN_SIGMA_CUT`]. In an elliptical
/// group, a shot whose others lie exactly on one line is not judged.
pub fn flier_check(shots: &[Point], options: Options) -> Result<Option<Flier>, StatsError> {
    let all = GroupStats::new(shots, options)?;
    let Some(spread) = all.spread else {
        return Ok(None);
    };
    if shots.len() < FLIER_MIN_SHOTS {
        return Ok(None);
    }
    let test = if spread.shape.elliptical {
        FlierTest::Elliptical
    } else {
        FlierTest::Round
    };
    let rest_shots = (shots.len() - 1) as f64;
    let limit = match test {
        FlierTest::Round => f_quantile(FLIER_REST_SHARE, 2.0, 2.0 * (rest_shots - 1.0)),
        FlierTest::Elliptical => f_quantile(FLIER_REST_SHARE, 2.0, rest_shots - 2.0),
    };
    let mut most: Option<(usize, f64, f64, f64)> = None;
    for (i, &shot) in shots.iter().enumerate() {
        let rest = without(shots, i);
        let centre = centroid(&rest).unwrap_or(Point::ORIGIN);
        let scatter = Scatter::about(&rest, centre, true);
        let sigma = scatter.sigma().map_or(0.0, |sigma| sigma.value);
        let offset = shot - centre;
        let statistic = match test {
            FlierTest::Round => outside(
                offset.dot(offset),
                2.0 * (1.0 + 1.0 / rest_shots) * sigma * sigma,
            ),
            FlierTest::Elliptical => {
                let (xx, yy, xy) = (scatter.xx, scatter.yy, scatter.xy);
                let spread = xx * yy - xy * xy;
                // dᵀ S⁻¹ d with S = scatter / (m − 1), the adjugate over the
                // determinant.
                let along = yy * offset.x * offset.x - 2.0 * xy * offset.x * offset.y
                    + xx * offset.y * offset.y;
                let squared = if xx + yy == 0.0 {
                    outside(offset.dot(offset), 0.0)
                } else if spread > 0.0 {
                    along * (rest_shots - 1.0) / spread
                } else {
                    // The others lie exactly on one line: no ellipse.
                    0.0
                };
                // Hotelling's T² for a further shot, as an F(2, m − 2).
                squared * rest_shots / (rest_shots + 1.0) * (rest_shots - 2.0)
                    / (2.0 * (rest_shots - 1.0))
            }
        };
        if most.is_none_or(|(_, best, _, _)| statistic > best) {
            most = Some((i, statistic, offset.length(), sigma));
        }
    }
    let Some((shot, statistic, distance_mm, sigma_rest)) = most else {
        return Ok(None);
    };
    let sigma_all = spread.sigma.value;
    let sigma_cut = if sigma_all > 0.0 {
        1.0 - sigma_rest / sigma_all
    } else {
        0.0
    };
    if statistic <= limit || sigma_cut < FLIER_MIN_SIGMA_CUT {
        return Ok(None);
    }
    Ok(Some(Flier {
        shot,
        distance_mm,
        rest_radius_mm: distance_mm * (limit / statistic).sqrt(),
        sigma_cut,
        test,
        without: GroupStats::new(&without(shots, shot), options)?,
    }))
}

/// `size / scale`, where a zero scale makes any size infinitely far.
fn outside(size: f64, scale: f64) -> f64 {
    if size == 0.0 {
        0.0
    } else if scale == 0.0 {
        f64::INFINITY
    } else {
        size / scale
    }
}

/// The radius, in σ̂ of `shots` shots, that a further shot from the same
/// round spread lands inside with chance `p`. The further shot's squared
/// distance from their centre, over 2 (1 + 1/n) σ̂², follows
/// F(2, 2(n − 1)) exactly.
pub fn prediction_radius_in_sigmas(shots: usize, p: f64) -> f64 {
    let n = shots as f64;
    (2.0 * (1.0 + 1.0 / n) * f_quantile(p, 2.0, 2.0 * (n - 1.0))).sqrt()
}

fn without(shots: &[Point], left_out: usize) -> Vec<Point> {
    shots
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != left_out)
        .map(|(_, &shot)| shot)
        .collect()
}

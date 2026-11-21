//! Is the zero holding? The impacts of several groups, pooled.
//!
//! Each group gives its impact (centre − aim, mrad) less whatever was
//! dialled for it, its number of shots and its σ̂. The per-shot mean
//! impact is the groups' impacts weighted by their shots. Its standard
//! error counts the shifts of impact from group to group (from day to
//! day, or remounting the scope) as well as the scatter within groups:
//! on each axis separately,
//!
//! - the between-group variance τ² is estimated by the method of moments
//!   of the one-way random-effects model, τ̂² = max(0, (SSB − (k − 1) σ̂²)
//!   ÷ (N − Σnᵢ²/N)), with SSB = Σ nᵢ (ȳᵢ − ȳ)² and σ̂² the pooled
//!   within-group variance;
//! - Var(ȳ) = τ̂² Σwᵢ² + σ̂²/N with wᵢ = nᵢ/N, which for groups of the same
//!   size is the scatter of the group impacts, s²/k, whenever τ̂² > 0;
//! - the range is ȳ ± t₀.₉₇₅ √Var(ȳ). When τ̂² > 0 the degrees of
//!   freedom are Satterthwaite's for Var(ȳ) as a combination of the
//!   between- and within-group mean squares (k − 1 for groups of the same
//!   size, fewer when one group holds most of the shots, never below 1);
//!   else those of σ̂.
//!
//! One or two groups can't show how impact shifts from group to group:
//! their range comes from the shots alone (σ̂/√N) and says so
//! ([`ZeroCheck::counts_group_to_group`] is false), and they give no
//! verdict but [`ZeroVerdict::CantTell`].

use crate::error::{AnalysisError, check_sigma, pool_sigmas, t95};
use crate::geometry::Point;
use crate::group::{Estimate, GroupStats, Interval, Sigma, StatsError};

/// The default tolerance of a zero, mrad either way on each axis.
pub const ZERO_TOLERANCE_MRAD: f64 = 0.1;

/// The fewest groups whose scatter the range counts, and the fewest that
/// give a verdict other than [`ZeroVerdict::CantTell`].
pub const MIN_GROUPS_FOR_SHIFTS: usize = 3;

/// The most further groups [`ZeroCheck::more_groups`] looks at.
pub const MAX_MORE_GROUPS: usize = 1000;

/// One group of a zero check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZeroGroup {
    /// The group's centre less its aim point, mrad, [across, up].
    pub impact_mrad: Point,
    pub shots: usize,
    /// The group's radial σ̂, mrad; ignored for a single shot.
    pub sigma_mrad: f64,
    /// What was dialled away from the zero for this group, mrad, up and
    /// right positive: subtracted from the impact first.
    pub dialled_mrad: Point,
}

impl ZeroGroup {
    /// The zero-check group of a measured group, which needs an aim point
    /// and a distance to give its impact and σ̂ in mrad; `dialled_mrad` is
    /// what was dialled away from the zero for it.
    pub fn from_stats(stats: &GroupStats, dialled_mrad: Point) -> Result<ZeroGroup, AnalysisError> {
        let impact = stats.impact.ok_or(AnalysisError::MissingAim)?;
        let distance = stats
            .options
            .distance_m
            .ok_or(AnalysisError::MissingDistance)?;
        Ok(ZeroGroup {
            impact_mrad: impact * (1.0 / distance),
            shots: stats.shots,
            sigma_mrad: stats.sigma_mrad().map_or(0.0, |sigma| sigma.value),
            dialled_mrad,
        })
    }
}

/// What a zero check says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroVerdict {
    /// The mean impact's range lies within the tolerance on both axes,
    /// from three groups or more.
    Holding,
    /// On an axis, the range leaves out 0 and the mean impact is beyond
    /// the tolerance, from three groups or more.
    Drifting,
    /// Neither yet, or fewer than three groups.
    CantTell,
}

/// The mean impact on one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZeroAxis {
    /// The per-shot mean impact, mrad, with its 95% range.
    pub impact: Estimate,
    /// Its standard error, mrad.
    pub standard_error: f64,
    /// The degrees of freedom of Student's t for the range.
    pub degrees_of_freedom: f64,
    /// τ̂, the SD of the impact from group to group beyond what the shots
    /// explain, mrad; `None` with fewer than three groups.
    pub between_group_sd: Option<f64>,
}

/// The pooled impact of several groups.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroCheck {
    pub groups: usize,
    pub shots: usize,
    /// Each group's impact less what was dialled for it, mrad, in input
    /// order: the points the mean is made of.
    pub net_impacts_mrad: Vec<Point>,
    /// N² / Σnᵢ²: how many groups of equal size would weigh in as evenly.
    /// Well below `groups`, one group holds most of the shots, and the
    /// range leans on few groups to show its shift (see the docs).
    pub effective_groups: f64,
    pub across: ZeroAxis,
    pub up: ZeroAxis,
    /// σ̂ pooled within the groups, mrad; `None` when every group is one
    /// shot.
    pub within_group_sigma: Option<Sigma>,
    /// Whether the ranges count the shifts from group to group (three
    /// groups or more); with one or two they rest on the shots alone and
    /// are too narrow if the impact moves between groups.
    pub counts_group_to_group: bool,
    /// What to dial to put the mean impact on the aim, mrad: minus the
    /// impact, up and right positive.
    pub correction_mrad: Point,
    pub tolerance_mrad: f64,
    pub verdict: ZeroVerdict,
    /// For `CantTell`: about how many more groups like these (the same
    /// shots, scatter and mean impact) would settle it; `None` when even
    /// [`MAX_MORE_GROUPS`] would not, or the verdict is settled.
    pub more_groups: Option<usize>,
}

impl ZeroCheck {
    /// The zero check of `groups` with the default tolerance,
    /// [`ZERO_TOLERANCE_MRAD`].
    pub fn new(groups: &[ZeroGroup]) -> Result<ZeroCheck, AnalysisError> {
        ZeroCheck::with_tolerance(groups, ZERO_TOLERANCE_MRAD)
    }

    /// The zero check of `groups` with a tolerance of `tolerance_mrad`
    /// either way on each axis.
    pub fn with_tolerance(
        groups: &[ZeroGroup],
        tolerance_mrad: f64,
    ) -> Result<ZeroCheck, AnalysisError> {
        if !tolerance_mrad.is_finite() {
            return Err(AnalysisError::NotFinite);
        }
        if tolerance_mrad <= 0.0 {
            return Err(AnalysisError::NotPositive("tolerance"));
        }
        let pooled = Pooled::of(groups)?;
        let across = pooled.axis(|point| point.x);
        let up = pooled.axis(|point| point.y);
        if across.variance == 0.0 || up.variance == 0.0 {
            return Err(AnalysisError::NoSpread);
        }
        for axis in [&across, &up] {
            let values = [
                axis.mean,
                axis.variance,
                axis.t,
                axis.estimate().range.low,
                axis.estimate().range.high,
            ];
            if values.iter().any(|value| !value.is_finite()) {
                return Err(AnalysisError::NotFinite);
            }
        }
        let verdict = verdict(
            groups.len(),
            [across.estimate().range, up.estimate().range],
            [across.mean, up.mean],
            tolerance_mrad,
        );
        let more_groups = match verdict {
            ZeroVerdict::CantTell => pooled.more_groups(&across, &up, tolerance_mrad),
            _ => None,
        };
        Ok(ZeroCheck {
            groups: groups.len(),
            shots: pooled.shots,
            net_impacts_mrad: pooled.groups.iter().map(|&(impact, _)| impact).collect(),
            effective_groups: {
                let squares: f64 = pooled.groups.iter().map(|&(_, n)| (n * n) as f64).sum();
                (pooled.shots as f64).powi(2) / squares
            },
            across: across.public(),
            up: up.public(),
            within_group_sigma: pooled.within,
            counts_group_to_group: groups.len() >= MIN_GROUPS_FOR_SHIFTS,
            correction_mrad: -Point::new(across.mean, up.mean),
            tolerance_mrad,
            verdict,
            more_groups,
        })
    }
}

fn verdict(groups: usize, ranges: [Interval; 2], means: [f64; 2], tolerance: f64) -> ZeroVerdict {
    let inside = |range: Interval| range.low >= -tolerance && range.high <= tolerance;
    if groups < MIN_GROUPS_FOR_SHIFTS {
        ZeroVerdict::CantTell
    } else if ranges.iter().all(|&range| inside(range)) {
        ZeroVerdict::Holding
    } else if ranges
        .iter()
        .zip(means)
        .any(|(range, mean)| !range.contains(0.0) && mean.abs() > tolerance)
    {
        ZeroVerdict::Drifting
    } else {
        ZeroVerdict::CantTell
    }
}

/// The groups, checked and net of what was dialled.
pub(crate) struct Pooled {
    /// (net impact, shots) per group.
    groups: Vec<(Point, usize)>,
    shots: usize,
    within: Option<Sigma>,
}

/// One axis worked out.
pub(crate) struct Axis {
    pub(crate) mean: f64,
    /// Var(ȳ), mrad².
    pub(crate) variance: f64,
    pub(crate) degrees: f64,
    pub(crate) t: f64,
    /// τ̂², or `None` with fewer than three groups.
    between: Option<f64>,
}

impl Axis {
    pub(crate) fn estimate(&self) -> Estimate {
        let half = self.t * self.variance.sqrt();
        Estimate {
            value: self.mean,
            range: Interval {
                low: self.mean - half,
                high: self.mean + half,
            },
        }
    }

    fn public(&self) -> ZeroAxis {
        ZeroAxis {
            impact: self.estimate(),
            standard_error: self.variance.sqrt(),
            degrees_of_freedom: self.degrees,
            between_group_sd: self.between.map(f64::sqrt),
        }
    }
}

impl Pooled {
    pub(crate) fn of(groups: &[ZeroGroup]) -> Result<Pooled, AnalysisError> {
        if groups.is_empty() {
            return Err(AnalysisError::Empty);
        }
        for (index, group) in groups.iter().enumerate() {
            check_sigma(index, group.shots, group.sigma_mrad)?;
            if !group.impact_mrad.is_finite() || !group.dialled_mrad.is_finite() {
                return Err(AnalysisError::Input {
                    index,
                    error: StatsError::NotFinite,
                });
            }
        }
        let within = pool_sigmas(groups.iter().map(|g| (g.shots, g.sigma_mrad)));
        if within.is_none() && groups.len() < MIN_GROUPS_FOR_SHIFTS {
            return Err(AnalysisError::NoSpreadEstimate);
        }
        Ok(Pooled {
            groups: groups
                .iter()
                .map(|g| (g.impact_mrad - g.dialled_mrad, g.shots))
                .collect(),
            shots: groups.iter().map(|g| g.shots).sum(),
            within,
        })
    }

    pub(crate) fn axis(&self, coordinate: fn(&Point) -> f64) -> Axis {
        let total = self.shots as f64;
        let k = self.groups.len();
        let mean = self
            .groups
            .iter()
            .map(|(impact, n)| *n as f64 * coordinate(impact))
            .sum::<f64>()
            / total;
        let (within_variance, within_degrees) = self
            .within
            .map_or((0.0, 0.0), |s| (s.value * s.value, s.degrees_of_freedom));
        if k < MIN_GROUPS_FOR_SHIFTS {
            return Axis {
                mean,
                variance: within_variance / total,
                degrees: within_degrees,
                t: t95(within_degrees),
                between: None,
            };
        }
        let between_squares: f64 = self
            .groups
            .iter()
            .map(|(impact, n)| *n as f64 * (coordinate(impact) - mean).powi(2))
            .sum();
        let squared_sizes: f64 = self.groups.iter().map(|(_, n)| (*n * *n) as f64).sum();
        // N − Σnᵢ²/N is positive for two groups or more.
        let spread_of_sizes = total - squared_sizes / total;
        let tau_squared =
            ((between_squares - (k - 1) as f64 * within_variance) / spread_of_sizes).max(0.0);
        let weights_squared = squared_sizes / (total * total);
        let variance = tau_squared * weights_squared + within_variance / total;
        let degrees = if tau_squared > 0.0 || within_degrees == 0.0 {
            // Var(ȳ) = a MSB + b σ̂², with MSB = SSB / (k − 1) on k − 1
            // degrees of freedom and σ̂² on its own (b ≤ 0).
            let between = (k - 1) as f64;
            let a = weights_squared * between / spread_of_sizes;
            let b = 1.0 / total - a;
            let between_part = (a * between_squares / between).powi(2) / between;
            let within_part = if within_degrees > 0.0 {
                (b * within_variance).powi(2) / within_degrees
            } else {
                0.0
            };
            (variance * variance / (between_part + within_part)).clamp(1.0, between)
        } else {
            within_degrees
        };
        Axis {
            mean,
            variance,
            degrees,
            t: t95(degrees),
            between: Some(tau_squared),
        }
    }

    /// The fewest further groups, like the mean of these, that settle the
    /// verdict when the mean impact stays where it is.
    fn more_groups(&self, across: &Axis, up: &Axis, tolerance: f64) -> Option<usize> {
        let k = self.groups.len() as f64;
        let mean_shots = self.shots as f64 / k;
        let (within_variance, within_degrees) = self
            .within
            .map_or((0.0, 0.0), |s| (s.value * s.value, s.degrees_of_freedom));
        let settled = |more: usize| {
            let groups = k + more as f64;
            let ranges = [across, up].map(|axis| {
                let tau_squared = axis.between.unwrap_or(0.0);
                let variance = tau_squared / groups + within_variance / (groups * mean_shots);
                let degrees = if tau_squared > 0.0 || within_degrees == 0.0 {
                    groups - 1.0
                } else {
                    within_degrees * groups / k
                };
                let half = t95(degrees) * variance.sqrt();
                Interval {
                    low: axis.mean - half,
                    high: axis.mean + half,
                }
            });
            verdict(
                self.groups.len() + more,
                ranges,
                [across.mean, up.mean],
                tolerance,
            ) != ZeroVerdict::CantTell
        };
        if !settled(MAX_MORE_GROUPS) {
            return None;
        }
        let (mut low, mut high) = (0, MAX_MORE_GROUPS);
        // settled(high) and, as the verdict is open now, !settled(low).
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if settled(middle) {
                high = middle;
            } else {
                low = middle;
            }
        }
        Some(high)
    }
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

    fn group(x: f64, y: f64, shots: usize, sigma: f64) -> ZeroGroup {
        ZeroGroup {
            impact_mrad: Point::new(x, y),
            shots,
            sigma_mrad: sigma,
            dialled_mrad: Point::ORIGIN,
        }
    }

    #[test]
    fn two_groups_by_hand() {
        // 5 and 3 shots; σ̂ pooled: (8 × 0.04 + 4 × 0.01) / 12 = 0.03.
        let mut groups = [group(0.1, 0.4, 5, 0.2), group(0.3, 0.2, 3, 0.1)];
        groups[1].dialled_mrad = Point::new(0.1, -0.2);
        let check = ZeroCheck::new(&groups).unwrap();
        assert!(!check.counts_group_to_group);
        let within = check.within_group_sigma.unwrap();
        assert_eq!(within.degrees_of_freedom, 12.0);
        close(within.value, 0.03f64.sqrt(), 1e-15);
        // Net impacts (0.1, 0.4) and (0.2, 0.4); per-shot mean x =
        // (0.5 + 0.6) / 8.
        close(check.across.impact.value, 1.1 / 8.0, 1e-15);
        close(check.up.impact.value, 0.4, 1e-15);
        close(check.across.standard_error, (0.03f64 / 8.0).sqrt(), 1e-15);
        assert_eq!(check.across.degrees_of_freedom, 12.0);
        let half = t95(12.0) * (0.03f64 / 8.0).sqrt();
        close(check.up.impact.range.low, 0.4 - half, 1e-14);
        assert_eq!(check.up.between_group_sd, None);
        close(check.correction_mrad.y, -0.4, 1e-15);
        // 0.4 mrad high, its range clear of zero, but two groups can't see
        // shifts between groups: no verdict; one more like these would.
        assert_eq!(check.verdict, ZeroVerdict::CantTell);
        assert_eq!(check.more_groups, Some(1));
        assert_eq!(
            check.net_impacts_mrad,
            [Point::new(0.1, 0.4), Point::new(0.3 - 0.1, 0.2 + 0.2)]
        );
        close(check.effective_groups, 64.0 / 34.0, 1e-15);
    }

    #[test]
    fn three_groups_by_hand() {
        // Equal sizes: with τ̂² > 0 the variance is s²/k of the impacts.
        let groups = [
            group(0.0, 0.0, 4, 0.05),
            group(0.0, 0.3, 4, 0.05),
            group(0.0, 0.6, 4, 0.05),
        ];
        let check = ZeroCheck::new(&groups).unwrap();
        assert!(check.counts_group_to_group);
        // s² of 0, 0.3, 0.6 is 0.09; s²/k = 0.03.
        close(check.up.standard_error, 0.03f64.sqrt(), 1e-14);
        close(check.up.degrees_of_freedom, 2.0, 1e-12);
        // τ̂² = (SSB − 2σ̂²) / (N − Σn²/N) = (0.72 − 0.005) / 8.
        close(
            check.up.between_group_sd.unwrap(),
            ((0.72 - 0.005f64) / 8.0).sqrt(),
            1e-14,
        );
        // Across, the groups agree: the shots' own scatter, 18 df.
        assert_eq!(check.across.between_group_sd, Some(0.0));
        assert_eq!(check.across.degrees_of_freedom, 18.0);
        close(check.across.standard_error, 0.05 / 12f64.sqrt(), 1e-15);
        assert_eq!(check.verdict, ZeroVerdict::CantTell);
        assert!(check.more_groups.is_some());
    }

    #[test]
    fn unequal_groups_by_hand() {
        // 6, 2 and 2 shots at 0, 0.3 and 0.6 up, σ̂ 0.1 (14 df): ȳ = 0.18,
        // SSB = 0.576, N − Σn²/N = 5.6, τ̂² = (0.576 − 0.02) / 5.6.
        let groups = [
            group(0.0, 0.0, 6, 0.1),
            group(0.0, 0.3, 2, 0.1),
            group(0.0, 0.6, 2, 0.1),
        ];
        let check = ZeroCheck::new(&groups).unwrap();
        close(check.up.impact.value, 0.18, 1e-15);
        close(
            check.up.between_group_sd.unwrap(),
            (0.556f64 / 5.6).sqrt(),
            1e-14,
        );
        // Var = τ̂² Σwᵢ² + σ̂²/N with Σwᵢ² = 0.44 (τ̂²/k would give 0.0341).
        let variance: f64 = 0.556 / 5.6 * 0.44 + 0.001;
        close(check.up.standard_error, variance.sqrt(), 1e-14);
        // Satterthwaite: Var = a MSB + b σ̂², a = 0.44 × 2 / 5.6, b = 0.1 − a.
        let a = 0.44 * 2.0 / 5.6;
        let degrees = variance * variance
            / ((a * 0.288f64).powi(2) / 2.0 + ((0.1 - a) * 0.01f64).powi(2) / 14.0);
        close(degrees, 1.949_769_389, 1e-8);
        close(check.up.degrees_of_freedom, degrees, 1e-9);
        close(check.effective_groups, 100.0 / 44.0, 1e-14);
    }

    #[test]
    fn groups_from_their_statistics() {
        let shots = [Point::new(-10.0, 30.0), Point::new(10.0, 30.0)];
        let options = crate::Options {
            aim: Some(Point::new(0.0, 10.0)),
            distance_m: Some(200.0),
            ..crate::Options::default()
        };
        let stats = GroupStats::new(&shots, options).unwrap();
        let group = ZeroGroup::from_stats(&stats, Point::new(0.0, 0.05)).unwrap();
        assert_eq!(group.impact_mrad, Point::new(0.0, 0.1));
        assert_eq!(group.dialled_mrad, Point::new(0.0, 0.05));
        close(
            group.sigma_mrad,
            stats.sigma().unwrap().value / 200.0,
            1e-15,
        );
        assert_eq!(stats.sigma_mrad().unwrap().degrees_of_freedom, 2.0);
        let no_aim = GroupStats::new(&shots, crate::Options::default()).unwrap();
        assert_eq!(
            ZeroGroup::from_stats(&no_aim, Point::ORIGIN),
            Err(AnalysisError::MissingAim)
        );
        let no_distance = GroupStats::new(
            &shots,
            crate::Options {
                distance_m: None,
                ..options
            },
        )
        .unwrap();
        assert_eq!(
            ZeroGroup::from_stats(&no_distance, Point::ORIGIN),
            Err(AnalysisError::MissingDistance)
        );
    }

    #[test]
    fn single_shots_and_verdicts() {
        // Cold-bore shots: σ̂ is ignored, the impacts' own scatter counts.
        let shots = [
            group(0.02, -0.01, 1, f64::NAN),
            group(-0.01, 0.02, 1, 0.0),
            group(0.0, 0.0, 1, 9.0),
            group(-0.01, -0.01, 1, 0.0),
        ];
        let check = ZeroCheck::new(&shots).unwrap();
        assert_eq!(check.within_group_sigma, None);
        close(check.across.degrees_of_freedom, 3.0, 1e-12);
        // s of 0.02, −0.01, 0, −0.01 (mean 0): √(0.0006 / 3); SE s/2.
        close(
            check.across.standard_error,
            (0.0006f64 / 3.0).sqrt() / 2.0,
            1e-15,
        );
        assert_eq!(check.verdict, ZeroVerdict::Holding);
        // A looser tolerance can't undo holding; a tighter one can't tell.
        assert_eq!(
            ZeroCheck::with_tolerance(&shots, 0.5).unwrap().verdict,
            ZeroVerdict::Holding
        );
        let tight = ZeroCheck::with_tolerance(&shots, 0.01).unwrap();
        assert_eq!(tight.verdict, ZeroVerdict::CantTell);
        // Two single shots measure nothing.
        assert_eq!(
            ZeroCheck::new(&shots[..2]),
            Err(AnalysisError::NoSpreadEstimate)
        );
    }

    #[test]
    fn more_groups_settle_it() {
        // Three 3-shot groups 0.08 mrad low with σ̂ 0.15: can't tell yet.
        let groups = [
            group(0.0, -0.08, 3, 0.15),
            group(0.0, -0.08, 3, 0.15),
            group(0.0, -0.08, 3, 0.15),
        ];
        let check = ZeroCheck::new(&groups).unwrap();
        assert_eq!(check.verdict, ZeroVerdict::CantTell);
        let more = check.more_groups.unwrap();
        // That many more like them are enough, one fewer is not.
        let with = |extra: usize| {
            let all: Vec<ZeroGroup> = std::iter::repeat_n(groups[0], 3 + extra).collect();
            ZeroCheck::new(&all).unwrap().verdict
        };
        assert_eq!(with(more), ZeroVerdict::Holding);
        assert_eq!(with(more - 1), ZeroVerdict::CantTell);
        // Exactly on the tolerance, nothing settles it.
        let on_edge = [group(0.0, 0.1, 5, 0.1), group(0.0, 0.1, 5, 0.1)];
        assert_eq!(ZeroCheck::new(&on_edge).unwrap().more_groups, None);
    }

    #[test]
    fn bad_inputs_are_refused() {
        assert_eq!(ZeroCheck::new(&[]), Err(AnalysisError::Empty));
        let good = group(0.0, 0.0, 5, 0.1);
        assert!(matches!(
            ZeroCheck::new(&[good, group(0.0, 0.0, 0, 0.1)]),
            Err(AnalysisError::Input {
                index: 1,
                error: StatsError::NoShots
            })
        ));
        for bad in [
            group(f64::NAN, 0.0, 5, 0.1),
            group(0.0, 0.0, 5, f64::INFINITY),
            group(0.0, 0.0, 5, -0.1),
        ] {
            assert!(matches!(
                ZeroCheck::new(&[good, bad]),
                Err(AnalysisError::Input { index: 1, .. })
            ));
        }
        let mut dialled = good;
        dialled.dialled_mrad = Point::new(0.0, f64::NAN);
        assert!(ZeroCheck::new(&[dialled]).is_err());
        assert!(ZeroCheck::with_tolerance(&[good], 0.0).is_err());
        assert!(ZeroCheck::with_tolerance(&[good], f64::NAN).is_err());
        // Overflow is refused, not hung on.
        let huge = group(1e300, 1e300, 5, 1e300);
        assert_eq!(
            ZeroCheck::new(&[huge, huge, group(-1e300, 0.0, 5, 0.0)]),
            Err(AnalysisError::NotFinite)
        );
        // No spread at all gives no range, as in `compare`.
        assert_eq!(
            ZeroCheck::new(&[group(0.0, 0.0, 3, 0.0), group(0.0, 0.0, 3, 0.0)]),
            Err(AnalysisError::NoSpread)
        );
        let cold = group(0.02, 0.01, 1, 0.0);
        assert_eq!(
            ZeroCheck::new(&[cold, cold, cold]),
            Err(AnalysisError::NoSpread)
        );
    }

    /// Simulated groups: k groups of `shots` shots, σ = 1 within, the
    /// group's true impact shifted by a normal of SD `tau` on each axis
    /// round a true mean of `mean`. Returns the share of ranges (both
    /// axes counted) that cover the true mean, and the share of checks
    /// that say drifting.
    fn simulate(
        rng: &mut Rng,
        sizes: &[usize],
        tau: f64,
        mean: f64,
        tolerance: f64,
        trials: usize,
    ) -> (f64, f64) {
        let (mut covered, mut drifting) = (0usize, 0usize);
        let mut groups = Vec::with_capacity(sizes.len());
        for _ in 0..trials {
            groups.clear();
            for &shots in sizes {
                let centre = Point::new(mean + tau * rng.normal(), mean + tau * rng.normal());
                let points: Vec<Point> = (0..shots).map(|_| rng.shot(centre, 1.0)).collect();
                let stats = crate::GroupStats::new(&points, crate::Options::default()).unwrap();
                groups.push(ZeroGroup {
                    impact_mrad: stats.centre,
                    shots,
                    sigma_mrad: stats.sigma().map_or(0.0, |s| s.value),
                    dialled_mrad: Point::ORIGIN,
                });
            }
            let pooled = Pooled::of(&groups).unwrap();
            let axes = [pooled.axis(|p| p.x), pooled.axis(|p| p.y)];
            let ranges = axes.each_ref().map(|axis| axis.estimate().range);
            covered += ranges.iter().filter(|range| range.contains(mean)).count();
            if verdict(
                sizes.len(),
                ranges,
                axes.each_ref().map(|axis| axis.mean),
                tolerance,
            ) == ZeroVerdict::Drifting
            {
                drifting += 1;
            }
        }
        (
            covered as f64 / (2 * trials) as f64,
            drifting as f64 / trials as f64,
        )
    }

    /// The numbers in `docs/group-statistics.md`: `cargo test --release
    /// -p groups-core -- --ignored --nocapture zero_check_table`.
    #[test]
    #[ignore]
    fn zero_check_table() {
        let mut rng = Rng::new(2026);
        let trials = 20_000;
        let cases: [(&[usize], f64); 17] = [
            (&[5, 5], 1.0),
            (&[5, 5, 5], 0.0),
            (&[5, 5, 5], 0.5),
            (&[5, 5, 5], 1.0),
            (&[5, 5, 5], 2.0),
            (&[3; 6], 0.0),
            (&[3; 6], 1.0),
            (&[5; 10], 0.5),
            (&[10, 3, 5, 3], 0.0),
            (&[10, 3, 5, 3], 1.0),
            (&[10, 5, 3, 3, 3], 0.5),
            (&[10, 5, 3, 3, 3], 1.0),
            (&[20, 3, 3], 0.0),
            (&[20, 3, 3], 0.5),
            (&[20, 3, 3], 1.0),
            (&[20, 3, 3], 2.0),
            (&[1; 5], 1.0),
        ];
        for (sizes, tau) in cases {
            // σ = 1; tolerance 1σ (a 1-MOA rifle at 0.1 mrad), σ/2, and
            // 0.01σ (where only the range decides).
            let (coverage, realistic) = simulate(&mut rng, sizes, tau, 0.0, 1.0, trials);
            let (_, half) = simulate(&mut rng, sizes, tau, 0.0, 0.5, trials);
            let (_, worst) = simulate(&mut rng, sizes, tau, 0.0, 0.01, trials);
            let n: usize = sizes.iter().sum();
            let squares: usize = sizes.iter().map(|s| s * s).sum();
            println!(
                "{sizes:?} (effective {:.1}) τ = {tau}σ: coverage {:.1}%, drifting {:.2}% (tolerance σ), {:.2}% (σ/2), {:.2}% (0.01σ)",
                (n * n) as f64 / squares as f64,
                100.0 * coverage,
                100.0 * realistic,
                100.0 * half,
                100.0 * worst
            );
        }
    }

    #[test]
    fn ranges_cover_with_and_without_shifts() {
        let mut rng = Rng::new(42);
        let trials = 1_200;
        // (group sizes, τ in σ): coverage within 2.5% of 95% (4 standard
        // errors of 2,400 ranges is 1.8%; the method is approximate).
        let cases: [(&[usize], f64); 9] = [
            (&[5, 5, 5], 0.0),
            (&[5, 5, 5], 0.5),
            (&[5, 5, 5], 2.0),
            (&[3, 3, 3, 3, 3, 3], 0.0),
            (&[3, 3, 3, 3, 3, 3], 1.0),
            (&[10, 3, 5, 3], 0.0),
            (&[10, 3, 5, 3], 1.0),
            (&[10, 5, 3, 3, 3], 1.0),
            (&[1, 1, 1, 1, 1], 1.0),
        ];
        for (sizes, tau) in cases {
            let (coverage, _) = simulate(&mut rng, sizes, tau, 0.0, 0.1, trials);
            assert!(
                (0.935..=0.985).contains(&coverage),
                "{sizes:?}, τ = {tau}: {coverage}"
            );
        }
        // Two groups can't see the shifts: with τ = σ the shots-only range
        // undercovers badly, which is why it says so.
        let (coverage, _) = simulate(&mut rng, &[5, 5], 1.0, 0.0, 0.1, trials);
        assert!(coverage < 0.8, "{coverage}");
        let (coverage, _) = simulate(&mut rng, &[5, 5], 0.0, 0.0, 0.1, trials);
        assert!((coverage - 0.95).abs() < 0.015, "{coverage}");
        // One group holding most of the shots leans on two small ones to
        // show its shift: about 88% at τ = σ/2, as documented.
        let (coverage, _) = simulate(&mut rng, &[20, 3, 3], 0.5, 0.0, 0.1, trials);
        assert!((0.85..0.92).contains(&coverage), "{coverage}");
    }

    #[test]
    fn a_true_zero_seldom_looks_drifting() {
        let mut rng = Rng::new(43);
        // σ = 1 per shot and a tolerance of 1σ, as for a 1-MOA rifle
        // (σ ≈ 0.1 mrad) at the default 0.1 mrad.
        for (sizes, tau) in [
            (&[5usize, 5, 5][..], 0.0),
            (&[5, 5, 5], 1.0),
            (&[3; 6], 1.0),
        ] {
            let (_, drifting) = simulate(&mut rng, sizes, tau, 0.0, 1.0, 2_000);
            assert!(drifting < 0.075, "{sizes:?}, τ = {tau}: {drifting}");
        }
        // A tolerance far inside the ranges leaves them to decide, on
        // either axis: about twice what each axis leaves out.
        let (_, drifting) = simulate(&mut rng, &[5, 5, 5], 0.0, 0.0, 0.01, 2_000);
        assert!(drifting < 0.08, "{drifting}");
        // Two groups never say drifting, however the impact moves.
        let (_, drifting) = simulate(&mut rng, &[5, 5], 1.0, 0.0, 0.01, 500);
        assert_eq!(drifting, 0.0);
        // A zero 1σ out is mostly caught by 15 shots.
        let (_, drifting) = simulate(&mut rng, &[5, 5, 5], 0.0, 1.0, 0.3, 1_000);
        assert!(drifting > 0.5, "{drifting}");
    }
}

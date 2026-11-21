//! Comparing the precision of two groups, and ranking several by size.
//!
//! Two groups are compared by the ratio of their radial σ̂ (see
//! [`Sigma`]): under the model, (σ̂²_A / σ_A²) ÷ (σ̂²_B / σ_B²) follows
//! F(ν_A, ν_B) with the two groups' degrees of freedom, which gives the
//! ratio's 95% range exactly.

use crate::composite::ShotGroup;
use crate::distributions::{f_cdf, f_quantile};
use crate::error::AnalysisError;
use crate::group::{CONFIDENCE, Estimate, GroupStats, Interval, Options, Sigma, StatsError};

/// Two groups count as the same when the whole range of their σ ratio
/// lies within 1 / `SAME_BAND` to `SAME_BAND`, whichever group comes first.
pub const SAME_BAND: f64 = 1.1;

/// The chance of telling a ratio from 1 that the shots needed are worked
/// out for.
pub const POWER: f64 = 0.8;

/// The two-sided false-alarm rate of "different".
pub const ALPHA: f64 = 0.05;

/// The most shots per group [`SigmaComparison::shots_needed`] looks at.
pub const MAX_SHOTS_NEEDED: usize = 1_000_000;

/// What the comparison of two groups' σ says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigmaVerdict {
    /// The whole range of the ratio lies within 0.91 to 1.1 (1 / 1.1 to
    /// 1.1, so swapping the groups gives the same verdict): any
    /// difference is under 10%. Takes precedence over `Different`.
    Same,
    /// The range of the ratio leaves out 1: one group really is tighter.
    Different,
    /// Neither: the groups can't be told apart yet.
    CantTell,
}

/// The comparison of two groups' radial σ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SigmaComparison {
    pub a: Sigma,
    pub b: Sigma,
    /// σ̂_A / σ̂_B, with the 95% range of σ_A / σ_B.
    pub ratio: Estimate,
    pub verdict: SigmaVerdict,
    /// How many shots each group would need for this ratio, if it were
    /// the true one, to show as different 80% of the time (at 5%, two
    /// sided), with both groups the same size. `None` when the ratio is
    /// within 1 / 1.1 to 1.1 (too small a difference to chase) or more
    /// than [`MAX_SHOTS_NEEDED`] would be needed.
    pub shots_needed: Option<usize>,
}

/// Compares two groups' σ̂ with their degrees of freedom. Both must be
/// more than zero and in the same units.
pub fn compare(a: Sigma, b: Sigma) -> Result<SigmaComparison, AnalysisError> {
    for sigma in [a, b] {
        if !sigma.value.is_finite() || !sigma.degrees_of_freedom.is_finite() {
            return Err(AnalysisError::NotFinite);
        }
        if sigma.degrees_of_freedom <= 0.0 {
            return Err(AnalysisError::NotPositive("degrees of freedom"));
        }
        if sigma.value <= 0.0 {
            return Err(AnalysisError::NoSpread);
        }
    }
    let ratio = ratio_estimate(a, b);
    if !ratio.value.is_finite() || !ratio.range.low.is_finite() || !ratio.range.high.is_finite() {
        return Err(AnalysisError::NotFinite);
    }
    Ok(SigmaComparison {
        a,
        b,
        ratio,
        verdict: verdict_of(ratio.range),
        shots_needed: shots_needed(ratio.value),
    })
}

/// Compares two groups' σ̂: as angles when both have a distance, so groups
/// shot at different distances compare fairly, or in millimetres when
/// neither has (taken as shot at the same distance). Unlike [`rank`] and
/// [`Composite`](crate::Composite), which take [`ShotGroup`]s and so always
/// have a distance, this takes measured groups, whose distance is optional:
/// a ratio needs only that both sizes are in one unit.
pub fn compare_groups(a: &GroupStats, b: &GroupStats) -> Result<SigmaComparison, AnalysisError> {
    let sigma = |index: usize, stats: &GroupStats| {
        stats.sigma().ok_or(AnalysisError::Input {
            index,
            error: StatsError::TooFewShots {
                need: 2,
                have: stats.shots,
            },
        })
    };
    let (sigma_a, sigma_b) = (sigma(0, a)?, sigma(1, b)?);
    match (a.sigma_mrad(), b.sigma_mrad()) {
        (Some(mrad_a), Some(mrad_b)) => compare(mrad_a, mrad_b),
        (None, None) => compare(sigma_a, sigma_b),
        _ => Err(AnalysisError::MixedUnits),
    }
}

/// σ̂_A / σ̂_B with the range of σ_A / σ_B: the ratio over √F at 97.5% and
/// at 2.5% of F(ν_A, ν_B).
pub(crate) fn ratio_estimate(a: Sigma, b: Sigma) -> Estimate {
    let tail = 0.5 * (1.0 - CONFIDENCE);
    let (da, db) = (a.degrees_of_freedom, b.degrees_of_freedom);
    let ratio = a.value / b.value;
    Estimate {
        value: ratio,
        range: Interval {
            low: ratio / f_quantile(1.0 - tail, da, db).sqrt(),
            high: ratio / f_quantile(tail, da, db).sqrt(),
        },
    }
}

pub(crate) fn verdict_of(range: Interval) -> SigmaVerdict {
    if range.low >= 1.0 / SAME_BAND && range.high <= SAME_BAND {
        SigmaVerdict::Same
    } else if range.low > 1.0 || range.high < 1.0 {
        SigmaVerdict::Different
    } else {
        SigmaVerdict::CantTell
    }
}

/// The chance that two groups of `shots` shots each, from spreads whose σ
/// differ by `ratio`, show as different: exact under the model, from
/// F(ν, ν) with ν = 2(n − 1). `None` for fewer than two shots or a ratio
/// that is not a positive number.
pub fn power(ratio: f64, shots: usize) -> Option<f64> {
    if shots < 2 || !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    let degrees = 2.0 * (shots - 1) as f64;
    let squared = ratio * ratio;
    let low = f_quantile(0.5 * ALPHA, degrees, degrees);
    let high = f_quantile(1.0 - 0.5 * ALPHA, degrees, degrees);
    // σ̂²_A / σ̂²_B is ratio² times an F(ν, ν).
    let power =
        (1.0 - f_cdf(high / squared, degrees, degrees)) + f_cdf(low / squared, degrees, degrees);
    power.is_finite().then_some(power)
}

/// The fewest shots per group with [`power`] at least [`POWER`].
fn shots_needed(ratio: f64) -> Option<usize> {
    if !ratio.is_finite() || ratio <= 0.0 || (1.0 / SAME_BAND..=SAME_BAND).contains(&ratio) {
        return None;
    }
    let enough = |shots: usize| power(ratio, shots).is_some_and(|power| power >= POWER);
    let mut high = 2;
    while !enough(high) {
        if high >= MAX_SHOTS_NEEDED {
            return None;
        }
        high = (2 * high).min(MAX_SHOTS_NEEDED);
    }
    let mut low = high / 2;
    if low < 2 {
        return Some(high);
    }
    // enough(high) and !enough(low).
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if enough(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    Some(high)
}

/// What groups are ranked by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RankBy {
    /// The mean radius about the group's own centre: uses every shot.
    #[default]
    MeanRadius,
    /// The extreme spread, centre to centre: comparable only between
    /// groups of the same number of shots.
    ExtremeSpread,
}

/// A group in a ranking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankedGroup {
    /// The group, by its index in the input.
    pub index: usize,
    pub shots: usize,
    /// Its size as an angle, mrad: what it is ranked by.
    pub size_mrad: f64,
    /// Its size on the target, mm.
    pub size_mm: f64,
    /// How much larger it is than the best, as a percentage: 0 for the
    /// best, 50 for half as large again.
    pub above_best_percent: f64,
}

/// Why a group isn't ranked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unrankable {
    /// Its statistics can't be worked out: no shots, a value that is not
    /// a number, a distance that is not positive, or a single shot.
    Stats(StatsError),
    /// Its shots lie on top of each other (to within the smallest normal
    /// number), so it has no size.
    NoSpread,
}

/// A group left out of a ranking, and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnrankedGroup {
    pub index: usize,
    pub reason: Unrankable,
}

/// Groups ranked by size, smallest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranking {
    pub by: RankBy,
    /// Best (smallest) first; equal sizes keep their input order.
    pub ranked: Vec<RankedGroup>,
    /// The groups that could not be measured, in input order.
    pub unranked: Vec<UnrankedGroup>,
    /// The ranked groups don't all have the same number of shots: the
    /// mean radius of a small group reads a little small and its extreme
    /// spread much smaller, so the ranking flatters small groups.
    pub unequal_shots: bool,
}

impl Ranking {
    pub fn best(&self) -> Option<&RankedGroup> {
        self.ranked.first()
    }

    pub fn worst(&self) -> Option<&RankedGroup> {
        self.ranked.last()
    }
}

/// Ranks `groups` by size as angles (mm ÷ distance), so groups shot at
/// different distances rank fairly. The aim points are not used.
pub fn rank(groups: &[ShotGroup], by: RankBy) -> Ranking {
    let mut ranked = Vec::new();
    let mut unranked = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let options = Options {
            distance_m: Some(group.distance_m),
            ..Options::default()
        };
        let stats = match GroupStats::new(&group.shots, options) {
            Ok(stats) => stats,
            Err(error) => {
                unranked.push(UnrankedGroup {
                    index,
                    reason: Unrankable::Stats(error),
                });
                continue;
            }
        };
        let Some(spread) = stats.spread else {
            unranked.push(UnrankedGroup {
                index,
                reason: Unrankable::Stats(StatsError::TooFewShots { need: 2, have: 1 }),
            });
            continue;
        };
        let size_mm = match by {
            RankBy::MeanRadius => stats.mean_radius,
            RankBy::ExtremeSpread => spread.extreme_spread.centre_to_centre,
        };
        let size_mrad = size_mm / group.distance_m;
        let problem = if !size_mrad.is_finite() {
            Some(Unrankable::Stats(StatsError::NotFinite))
        } else if size_mrad < f64::MIN_POSITIVE {
            Some(Unrankable::NoSpread)
        } else {
            None
        };
        if let Some(reason) = problem {
            unranked.push(UnrankedGroup { index, reason });
            continue;
        }
        ranked.push(RankedGroup {
            index,
            shots: stats.shots,
            size_mrad,
            size_mm,
            above_best_percent: 0.0,
        });
    }
    ranked.sort_by(|a, b| a.size_mrad.total_cmp(&b.size_mrad));
    if let Some(best) = ranked.first().map(|group| group.size_mrad) {
        for group in &mut ranked {
            group.above_best_percent = (group.size_mrad / best - 1.0) * 100.0;
        }
        // Sizes too far apart to divide.
        let (fine, overflowing): (Vec<RankedGroup>, Vec<RankedGroup>) = ranked
            .into_iter()
            .partition(|group| group.above_best_percent.is_finite());
        ranked = fine;
        unranked.extend(overflowing.into_iter().map(|group| UnrankedGroup {
            index: group.index,
            reason: Unrankable::Stats(StatsError::NotFinite),
        }));
        unranked.sort_by_key(|group| group.index);
    }
    let unequal_shots = ranked.windows(2).any(|pair| pair[0].shots != pair[1].shots);
    Ranking {
        by,
        ranked,
        unranked,
        unequal_shots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::testing::Rng;

    fn sigma(value: f64, shots: usize) -> Sigma {
        Sigma {
            value,
            degrees_of_freedom: 2.0 * (shots - 1) as f64,
        }
    }

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    #[test]
    fn ratio_and_range_by_hand() {
        // Two 5-shot groups: ν = 8 each; F₀.₉₇₅(8, 8) = 4.4333.
        let comparison = compare(sigma(2.0, 5), sigma(1.0, 5)).unwrap();
        let f = f_quantile(0.975, 8.0, 8.0);
        close(f, 4.433_260, 1e-5);
        close(comparison.ratio.value, 2.0, 1e-15);
        close(comparison.ratio.range.low, 2.0 / f.sqrt(), 1e-12);
        // F₀.₀₂₅(8, 8) = 1 / F₀.₉₇₅(8, 8).
        close(comparison.ratio.range.high, 2.0 * f.sqrt(), 1e-9);
        assert_eq!(comparison.verdict, SigmaVerdict::CantTell);
        // Twice the σ is told apart 80% of the time by 10 shots each.
        let needed = comparison.shots_needed.unwrap();
        assert!(power(2.0, needed).unwrap() >= POWER && power(2.0, needed - 1).unwrap() < POWER);
        assert_eq!(power(f64::NAN, 5), None);
        assert_eq!(power(2.0, 1), None);
        assert_eq!(needed, 10);
    }

    #[test]
    fn swapping_the_groups_inverts_the_ratio() {
        for (a, b) in [
            (sigma(1.3, 5), sigma(1.0, 12)),
            (sigma(0.2, 40), sigma(0.6, 3)),
            (sigma(1.04, 300), sigma(1.0, 300)),
        ] {
            let ab = compare(a, b).unwrap();
            let ba = compare(b, a).unwrap();
            close(ab.ratio.value * ba.ratio.value, 1.0, 1e-12);
            close(ab.ratio.range.low * ba.ratio.range.high, 1.0, 1e-9);
            close(ab.ratio.range.high * ba.ratio.range.low, 1.0, 1e-9);
            assert_eq!(ab.verdict, ba.verdict);
            assert_eq!(ab.shots_needed, ba.shots_needed);
        }
        // Units cancel.
        let mm = compare(sigma(13.0, 5), sigma(7.0, 8)).unwrap();
        let mrad = compare(sigma(0.13, 5), sigma(0.07, 8)).unwrap();
        close(mm.ratio.range.low, mrad.ratio.range.low, 1e-12);
    }

    #[test]
    fn verdicts() {
        assert_eq!(
            compare(sigma(3.0, 10), sigma(1.0, 10)).unwrap().verdict,
            SigmaVerdict::Different
        );
        assert_eq!(
            compare(sigma(1.0, 10), sigma(3.0, 10)).unwrap().verdict,
            SigmaVerdict::Different
        );
        // A thousand shots each pin a ratio of 1.02 inside 0.91–1.1.
        let same = compare(sigma(1.02, 1000), sigma(1.0, 1000)).unwrap();
        assert_eq!(same.verdict, SigmaVerdict::Same);
        assert_eq!(same.shots_needed, None);
        assert_eq!(
            verdict_of(Interval {
                low: 0.9,
                high: 1.05
            }),
            SigmaVerdict::CantTell
        );
        assert_eq!(
            verdict_of(Interval {
                low: 0.92,
                high: 1.1
            }),
            SigmaVerdict::Same
        );
        // Inside the band and clear of 1: the same for a shooter.
        assert_eq!(
            verdict_of(Interval {
                low: 1.01,
                high: 1.09
            }),
            SigmaVerdict::Same
        );
    }

    #[test]
    fn bad_inputs_are_refused() {
        let good = sigma(1.0, 5);
        assert_eq!(compare(sigma(0.0, 5), good), Err(AnalysisError::NoSpread));
        assert_eq!(
            compare(good, sigma(f64::NAN, 5)),
            Err(AnalysisError::NotFinite)
        );
        let none = Sigma {
            value: 1.0,
            degrees_of_freedom: 0.0,
        };
        assert!(matches!(
            compare(good, none),
            Err(AnalysisError::NotPositive(_))
        ));
        let infinite = Sigma {
            value: f64::INFINITY,
            degrees_of_freedom: 4.0,
        };
        assert_eq!(compare(infinite, good), Err(AnalysisError::NotFinite));
        // Extreme but finite: no hang, an answer or a refusal.
        let tiny = Sigma {
            value: 1e-300,
            degrees_of_freedom: 4.0,
        };
        let _ = compare(tiny, sigma(1e300, 5));
        assert_eq!(shots_needed(f64::INFINITY), None);
    }

    #[test]
    fn groups_compare_as_angles() {
        let shots = |scale: f64| -> Vec<Point> {
            [
                (-3.0, 1.0),
                (2.0, 4.0),
                (1.0, -2.0),
                (4.0, 0.0),
                (-4.0, -3.0),
            ]
            .iter()
            .map(|&(x, y)| Point::new(x * scale, y * scale))
            .collect()
        };
        let at = |distance: Option<f64>, scale: f64| {
            GroupStats::new(
                &shots(scale),
                Options {
                    distance_m: distance,
                    ..Options::default()
                },
            )
            .unwrap()
        };
        // Twice the size at twice the distance is the same angle.
        let comparison = compare_groups(&at(Some(100.0), 1.0), &at(Some(200.0), 2.0)).unwrap();
        close(comparison.ratio.value, 1.0, 1e-12);
        let mm = compare_groups(&at(None, 1.0), &at(None, 2.0)).unwrap();
        close(mm.ratio.value, 0.5, 1e-12);
        assert_eq!(
            compare_groups(&at(Some(100.0), 1.0), &at(None, 1.0)),
            Err(AnalysisError::MixedUnits)
        );
        let one = GroupStats::new(&[Point::ORIGIN], Options::default()).unwrap();
        assert!(matches!(
            compare_groups(&one, &at(None, 1.0)),
            Err(AnalysisError::Input { index: 0, .. })
        ));
    }

    /// σ̂ of `shots` shots from a round spread of `sigma` about the origin.
    fn simulated(rng: &mut Rng, shots: usize, sigma_true: f64) -> Sigma {
        let points: Vec<Point> = (0..shots)
            .map(|_| rng.shot(Point::ORIGIN, sigma_true))
            .collect();
        GroupStats::new(&points, Options::default())
            .unwrap()
            .sigma()
            .unwrap()
    }

    #[test]
    fn honest_pairs_seldom_look_different() {
        let mut rng = Rng::new(20_261_121);
        let trials = 4_000;
        for (shots_a, shots_b) in [(5, 5), (3, 10), (10, 20)] {
            // The same degrees of freedom every time, so the F quantiles
            // are worked out once and the verdict as `compare` does.
            let reference = ratio_estimate(sigma(1.0, shots_a), sigma(1.0, shots_b));
            let mut different = 0;
            for _ in 0..trials {
                let a = simulated(&mut rng, shots_a, 1.0);
                let b = simulated(&mut rng, shots_b, 1.0);
                let ratio = a.value / b.value;
                let range = reference.range.scaled(ratio);
                if verdict_of(range) == SigmaVerdict::Different {
                    different += 1;
                }
            }
            let rate = different as f64 / trials as f64;
            // 5% ± 4 standard errors (0.34%).
            assert!((rate - 0.05).abs() < 0.014, "{shots_a} v {shots_b}: {rate}");
        }
        // And through `compare` itself, fewer times.
        let mut different = 0;
        for _ in 0..300 {
            let a = simulated(&mut rng, 5, 0.3);
            let b = simulated(&mut rng, 5, 0.3);
            if compare(a, b).unwrap().verdict == SigmaVerdict::Different {
                different += 1;
            }
        }
        assert!(different <= 35, "{different} of 300");
    }

    #[test]
    fn the_power_formula_matches_simulation() {
        let mut rng = Rng::new(7);
        let trials = 3_000;
        for ratio in [1.5, 2.0] {
            let needed = shots_needed(ratio).unwrap();
            for shots in [needed, needed / 2] {
                let reference = ratio_estimate(sigma(1.0, shots), sigma(1.0, shots));
                let mut different = 0;
                for _ in 0..trials {
                    let a = simulated(&mut rng, shots, ratio);
                    let b = simulated(&mut rng, shots, 1.0);
                    if verdict_of(reference.range.scaled(a.value / b.value))
                        == SigmaVerdict::Different
                    {
                        different += 1;
                    }
                }
                let rate = different as f64 / trials as f64;
                let expected = power(ratio, shots).unwrap();
                let error = (expected * (1.0 - expected) / trials as f64).sqrt();
                assert!(
                    (rate - expected).abs() < 4.0 * error,
                    "ratio {ratio}, {shots} shots: simulated {rate}, formula {expected}"
                );
            }
        }
    }

    /// The numbers in `docs/group-statistics.md`: `cargo test --release
    /// -p groups-core -- --ignored --nocapture comparison_table`.
    #[test]
    #[ignore]
    fn comparison_table() {
        let mut rng = Rng::new(2026);
        let trials = 40_000;
        for (shots_a, shots_b) in [(3, 3), (5, 5), (10, 10), (3, 10), (5, 20)] {
            let reference = ratio_estimate(sigma(1.0, shots_a), sigma(1.0, shots_b));
            let different = (0..trials)
                .filter(|_| {
                    let a = simulated(&mut rng, shots_a, 1.0);
                    let b = simulated(&mut rng, shots_b, 1.0);
                    verdict_of(reference.range.scaled(a.value / b.value)) == SigmaVerdict::Different
                })
                .count();
            println!(
                "same spread, {shots_a} v {shots_b} shots: different {:.2}%",
                100.0 * different as f64 / trials as f64
            );
        }
        for ratio in [1.15, 1.2, 1.3, 1.5, 2.0, 3.0] {
            let needed = shots_needed(ratio).unwrap();
            let reference = ratio_estimate(sigma(1.0, needed), sigma(1.0, needed));
            let runs = if needed > 100 { 4_000 } else { trials };
            let different = (0..runs)
                .filter(|_| {
                    let a = simulated(&mut rng, needed, ratio);
                    let b = simulated(&mut rng, needed, 1.0);
                    verdict_of(reference.range.scaled(a.value / b.value)) == SigmaVerdict::Different
                })
                .count();
            println!(
                "ratio {ratio}: {needed} shots each, power {:.3}, simulated {:.3}",
                power(ratio, needed).unwrap(),
                different as f64 / runs as f64
            );
        }
    }

    fn group(shots: &[(f64, f64)], distance_m: f64) -> ShotGroup {
        ShotGroup {
            shots: shots.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            aim: None,
            distance_m,
        }
    }

    #[test]
    fn ranking_by_hand() {
        let groups = [
            // ES 20 mm at 100 m: 0.2 mrad; mean radius 10 mm: 0.1 mrad.
            group(&[(-10.0, 0.0), (10.0, 0.0)], 100.0),
            // ES 20 mm at 200 m: 0.1 mrad.
            group(&[(0.0, -10.0), (0.0, 10.0)], 200.0),
            group(&[(5.0, 5.0)], 100.0),
            group(&[(1.0, 1.0), (1.0, 1.0)], 100.0),
            group(&[(f64::NAN, 0.0), (1.0, 1.0)], 100.0),
            group(&[(0.0, 0.0), (3.0, 4.0)], 0.0),
            // ES 30 mm at 100 m: 0.3 mrad.
            group(&[(0.0, 0.0), (30.0, 0.0), (15.0, 0.0)], 100.0),
            group(&[(0.0, 0.0), (1e300, 0.0)], 1e-300),
            // A subnormal size counts as no spread.
            group(&[(0.0, 0.0), (1e-300, 0.0)], 1e10),
        ];
        let ranking = rank(&groups, RankBy::ExtremeSpread);
        let order: Vec<usize> = ranking.ranked.iter().map(|g| g.index).collect();
        assert_eq!(order, [1, 0, 6]);
        assert_eq!(ranking.best().unwrap().index, 1);
        assert_eq!(ranking.worst().unwrap().index, 6);
        close(ranking.ranked[1].above_best_percent, 100.0, 1e-9);
        close(ranking.ranked[2].above_best_percent, 200.0, 1e-9);
        close(ranking.ranked[2].size_mm, 30.0, 1e-12);
        assert!(ranking.unequal_shots);
        let reasons: Vec<(usize, Unrankable)> = ranking
            .unranked
            .iter()
            .map(|g| (g.index, g.reason))
            .collect();
        assert_eq!(
            reasons,
            [
                (
                    2,
                    Unrankable::Stats(StatsError::TooFewShots { need: 2, have: 1 })
                ),
                (3, Unrankable::NoSpread),
                (4, Unrankable::Stats(StatsError::NotFinite)),
                (5, Unrankable::Stats(StatsError::NotPositive("distance"))),
                (7, Unrankable::Stats(StatsError::NotFinite)),
                (8, Unrankable::NoSpread),
            ]
        );
        let by_radius = rank(&groups[..2], RankBy::default());
        assert_eq!(by_radius.by, RankBy::MeanRadius);
        close(by_radius.ranked[0].size_mrad, 0.05, 1e-15);
        close(by_radius.ranked[1].size_mrad, 0.1, 1e-15);
        assert!(!by_radius.unequal_shots);
        assert!(rank(&[], RankBy::MeanRadius).best().is_none());
        // Sizes too far apart to divide leave the larger unranked.
        let apart = rank(
            &[
                group(&[(0.0, 0.0), (1e-300, 0.0)], 1.0),
                group(&[(0.0, 0.0), (1e10, 0.0)], 1.0),
            ],
            RankBy::ExtremeSpread,
        );
        assert_eq!(apart.ranked.len(), 1);
        assert_eq!(
            apart.unranked,
            [UnrankedGroup {
                index: 1,
                reason: Unrankable::Stats(StatsError::NotFinite)
            }]
        );
    }
}

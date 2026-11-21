//! Several groups pooled into one composite, as angles.
//!
//! Every shot is turned into milliradians (mm ÷ the group's distance in
//! m) before anything is pooled, so groups shot at different distances
//! never mix millimetres. Each group is first moved onto a common origin:
//!
//! - [`CompositeMode::AboutAims`]: each shot less its group's aim point,
//!   so the points are the shots' impacts. Statistics are of all the
//!   points about the composite's own centre (the mean impact): the
//!   shifts of impact between groups are part of the result.
//! - [`CompositeMode::AboutOwnCentres`]: each shot less its group's own
//!   centre. σ̂ pools the scatter within the groups, with Σ(nᵢ − 1)
//!   degrees of freedom per axis; nothing about impact is left. The
//!   residuals are scaled by √(nᵢ / (nᵢ − 1)) so the sizes measured from
//!   them are not shrunk, and single-shot groups are left out.
//! - [`CompositeMode::ColdBore`]: single-shot groups with aim points, as
//!   their k impacts about their centre, k − 1 degrees of freedom per
//!   axis.
//!
//! Every figure is worked out from the pooled points, never averaged
//! over the groups.

use crate::error::AnalysisError;
use crate::geometry::{Point, centroid};
use crate::group::{
    Estimate, GroupStats, Interval, Options, RadiusMethod, Scatter, Shape, Sigma, StatsError,
};

/// A group's shots on the target, mm, with its aim point (if any) and
/// the distance it was shot at, m.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotGroup {
    pub shots: Vec<Point>,
    pub aim: Option<Point>,
    pub distance_m: f64,
}

/// How the groups of a composite are moved onto a common origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositeMode {
    /// Each group on its aim point; statistics about the composite's
    /// centre, with N − 1 degrees of freedom per axis for N shots.
    AboutAims,
    /// Each group on its own centre; σ̂ pooled within the groups, with
    /// Σ(nᵢ − 1) degrees of freedom per axis.
    AboutOwnCentres,
    /// One shot per group, each on its aim point: the k first shots
    /// about their centre, with k − 1 degrees of freedom per axis.
    ColdBore,
}

/// A composite's figures in one unit: mrad, or mm at a chosen distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositeFigures {
    /// The mean of the shots' impacts (shot − aim), [across, up]; `None`
    /// about the groups' own centres, which leaves no impact.
    pub impact: Option<Point>,
    /// The mean distance of the points from their origin: the composite's
    /// centre, or (about own centres) each shot's own group centre, with
    /// the residuals scaled (see [`Composite::points_mrad`]).
    pub mean_radius: f64,
    /// The median of the same distances.
    pub median_radius: f64,
    /// The two points furthest apart, centre to centre, as the composite
    /// is drawn.
    pub extreme_spread: f64,
    /// The 95% range of the extreme spread to expect from this many shots
    /// from one spread, as for one group: only for a cold-bore series of
    /// up to 50 shots, the one composite whose points are, under the
    /// model, independent shots from one spread.
    pub extreme_spread_range: Option<Interval>,
    /// Furthest left to furthest right of the points.
    pub width: f64,
    /// Lowest to highest of the points.
    pub height: f64,
    /// σ̂ with its 95% range from χ² with the composite's degrees of
    /// freedom (see [`Composite::sigma`]).
    pub sigma: Estimate,
    /// The range of the true mean radius: σ's range times √(π/2).
    pub mean_radius_range: Interval,
    pub cep: Estimate,
    pub r90: Estimate,
    pub r95: Estimate,
    /// The sample SD across, with the composite's degrees of freedom.
    pub sd_horizontal: f64,
    /// The sample SD up and down, with the composite's degrees of freedom.
    pub sd_vertical: f64,
}

impl CompositeFigures {
    fn scaled(&self, factor: f64) -> CompositeFigures {
        let estimate = |estimate: Estimate| Estimate {
            value: estimate.value * factor,
            range: estimate.range.scaled(factor),
        };
        CompositeFigures {
            impact: self.impact.map(|impact| impact * factor),
            mean_radius: self.mean_radius * factor,
            median_radius: self.median_radius * factor,
            extreme_spread: self.extreme_spread * factor,
            extreme_spread_range: self.extreme_spread_range.map(|range| range.scaled(factor)),
            width: self.width * factor,
            height: self.height * factor,
            sigma: estimate(self.sigma),
            mean_radius_range: self.mean_radius_range.scaled(factor),
            cep: estimate(self.cep),
            r90: estimate(self.r90),
            r95: estimate(self.r95),
            sd_horizontal: self.sd_horizontal * factor,
            sd_vertical: self.sd_vertical * factor,
        }
    }
}

/// Several groups pooled as angles.
#[derive(Clone, Debug, PartialEq)]
pub struct Composite {
    pub mode: CompositeMode,
    /// How many groups were pooled.
    pub groups: usize,
    /// About own centres only: the single-shot groups, by index, left out
    /// because a single shot is its own centre and shows no spread.
    pub left_out: Vec<usize>,
    /// How many shots were pooled.
    pub shots: usize,
    /// Every pooled shot after moving its group, mrad, group by group in
    /// input order. About own centres each is its residual from its
    /// group's centre times √(n / (n − 1)): a residual varies by only
    /// √(1 − 1/n) σ, so scaled it varies by σ, like a shot about a known
    /// centre, and the sizes measured from these points (mean and median
    /// radius, extreme spread, width, height) are not shrunk.
    pub points_mrad: Vec<Point>,
    /// The group, by index in the input, of each of `points_mrad`.
    pub point_groups: Vec<usize>,
    /// The pooled sums of squares, mrad², about the composite's centre
    /// (`centres` = 1) or about each group's own centre (`centres` = k),
    /// of the shots as shot (not of the scaled points).
    pub scatter_mrad: Scatter,
    /// σ̂, mrad, with its degrees of freedom: 2(N − 1) about aims,
    /// 2Σ(nᵢ − 1) about own centres, 2(k − 1) for a cold-bore series.
    pub sigma: Sigma,
    /// About aims only, with at least one group of two shots: σ̂ within
    /// the groups (about their own centres), mrad, for comparison with
    /// `sigma`, which also holds the shifts between groups.
    pub within_group_sigma: Option<Sigma>,
    /// The correlation between across and up, −1 to 1.
    pub correlation: f64,
    /// The shape of the pooled points (see [`Shape`]).
    pub shape: Shape,
    /// How CEP, R90 and R95 were worked out.
    pub radius_method: RadiusMethod,
    /// The figures in mrad.
    pub mrad: CompositeFigures,
}

impl Composite {
    /// Pools `groups`: about their aims when every group has one (as a
    /// cold-bore series when, in addition, every group is one shot), else
    /// about their own centres.
    pub fn new(groups: &[ShotGroup]) -> Result<Composite, AnalysisError> {
        let mode = if groups.iter().all(|group| group.aim.is_some()) {
            if groups.iter().all(|group| group.shots.len() == 1) {
                CompositeMode::ColdBore
            } else {
                CompositeMode::AboutAims
            }
        } else {
            CompositeMode::AboutOwnCentres
        };
        Composite::with_mode(groups, mode)
    }

    /// Pools `groups` in the given way. About aims and as a cold-bore
    /// series every group needs an aim point; a cold-bore series takes
    /// one shot per group.
    pub fn with_mode(
        groups: &[ShotGroup],
        mode: CompositeMode,
    ) -> Result<Composite, AnalysisError> {
        check_groups(groups)?;
        for (index, group) in groups.iter().enumerate() {
            if mode != CompositeMode::AboutOwnCentres && group.aim.is_none() {
                return Err(AnalysisError::NoAim { index });
            }
            if mode == CompositeMode::ColdBore && group.shots.len() != 1 {
                return Err(AnalysisError::NotOneShot {
                    index,
                    shots: group.shots.len(),
                });
            }
        }
        // About own centres a single shot is its own centre: no spread.
        let left_out: Vec<usize> = match mode {
            CompositeMode::AboutOwnCentres => (0..groups.len())
                .filter(|&index| groups[index].shots.len() == 1)
                .collect(),
            _ => Vec::new(),
        };
        let pooled: Vec<usize> = (0..groups.len())
            .filter(|index| !left_out.contains(index))
            .collect();
        if pooled.is_empty() {
            return Err(AnalysisError::NoSpreadEstimate);
        }
        let own_centres: Vec<Point> = groups
            .iter()
            .map(|group| centroid(&group.shots).unwrap_or(Point::ORIGIN))
            .collect();
        let within = pooled
            .iter()
            .map(|&index| {
                let group = &groups[index];
                Scatter::about(&group.shots, own_centres[index], true)
                    .scaled(1.0 / group.distance_m)
            })
            .fold(Scatter::default(), |sum, scatter| sum + scatter);
        let mut points = Vec::new();
        let mut point_groups = Vec::new();
        for &index in &pooled {
            let group = &groups[index];
            let n = group.shots.len() as f64;
            let (origin, factor) = match mode {
                // A residual from the group's own centre varies by
                // √(1 − 1/n) σ; scaled back up it varies by σ.
                CompositeMode::AboutOwnCentres => (
                    own_centres[index],
                    (n / (n - 1.0)).sqrt() / group.distance_m,
                ),
                _ => (group.aim.unwrap_or(Point::ORIGIN), 1.0 / group.distance_m),
            };
            points.extend(group.shots.iter().map(|&shot| (shot - origin) * factor));
            point_groups.extend(std::iter::repeat_n(index, group.shots.len()));
        }
        let shots = points.len();
        if shots < 2 {
            return Err(AnalysisError::TooFew {
                what: "shots",
                need: 2,
                have: shots,
            });
        }
        let (centre, scatter) = match mode {
            CompositeMode::AboutOwnCentres => (Point::ORIGIN, within),
            _ => {
                let centre = centroid(&points).unwrap_or(Point::ORIGIN);
                (centre, Scatter::about(&points, centre, true))
            }
        };
        let stats = GroupStats::with_scatter(&points, centre, scatter, Options::default())
            .map_err(|error| match error {
                StatsError::NotFinite => AnalysisError::NotFinite,
                error => AnalysisError::Stats(error),
            })?;
        let spread = stats.spread.ok_or(AnalysisError::NoSpreadEstimate)?;
        let sigma = scatter.sigma().ok_or(AnalysisError::NoSpreadEstimate)?;
        let within_group_sigma = match mode {
            CompositeMode::AboutAims => within.sigma(),
            _ => None,
        };
        let mrad = CompositeFigures {
            impact: (mode != CompositeMode::AboutOwnCentres).then_some(centre),
            mean_radius: stats.mean_radius,
            median_radius: stats.median_radius,
            extreme_spread: spread.extreme_spread.centre_to_centre,
            extreme_spread_range: match mode {
                CompositeMode::ColdBore => spread.extreme_spread.range,
                _ => None,
            },
            width: stats.width,
            height: stats.height,
            sigma: spread.sigma,
            mean_radius_range: spread.mean_radius_range,
            cep: spread.cep,
            r90: spread.r90,
            r95: spread.r95,
            sd_horizontal: spread.sd_horizontal,
            sd_vertical: spread.sd_vertical,
        };
        Ok(Composite {
            mode,
            groups: pooled.len(),
            left_out,
            shots,
            points_mrad: points,
            point_groups,
            scatter_mrad: scatter,
            sigma,
            within_group_sigma,
            correlation: spread.correlation,
            shape: spread.shape,
            radius_method: spread.radius_method,
            mrad,
        })
    }

    /// The figures as millimetres on a target at `distance_m` metres.
    pub fn mm_at(&self, distance_m: f64) -> Result<CompositeFigures, AnalysisError> {
        if !distance_m.is_finite() {
            return Err(AnalysisError::NotFinite);
        }
        if distance_m <= 0.0 {
            return Err(AnalysisError::NotPositive("distance"));
        }
        Ok(self.mrad.scaled(distance_m))
    }
}

fn check_groups(groups: &[ShotGroup]) -> Result<(), AnalysisError> {
    if groups.is_empty() {
        return Err(AnalysisError::Empty);
    }
    for (index, group) in groups.iter().enumerate() {
        let error = |error: StatsError| AnalysisError::Input { index, error };
        if group.shots.is_empty() {
            return Err(error(StatsError::NoShots));
        }
        if group.shots.iter().any(|shot| !shot.is_finite())
            || group.aim.is_some_and(|aim| !aim.is_finite())
            || !group.distance_m.is_finite()
        {
            return Err(error(StatsError::NotFinite));
        }
        if group.distance_m <= 0.0 {
            return Err(error(StatsError::NotPositive("distance")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::expected_extreme_spread_in_sigmas;
    use crate::testing::Rng;

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    fn group(shots: &[(f64, f64)], aim: Option<(f64, f64)>, distance_m: f64) -> ShotGroup {
        ShotGroup {
            shots: shots.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            aim: aim.map(|(x, y)| Point::new(x, y)),
            distance_m,
        }
    }

    #[test]
    fn about_own_centres_by_hand() {
        // At 100 m: centre (1, 0), squares 2 + 2 = 4 mm² → 4e-4 mrad².
        // At 200 m: centre (0, 2), squares 16 + 16 = 32 mm² → 8e-4 mrad².
        // The single shot at 50 m is left out.
        let groups = [
            group(
                &[(0.0, 0.0), (2.0, 0.0), (1.0, 1.0), (1.0, -1.0)],
                None,
                100.0,
            ),
            group(&[(7.0, 7.0)], None, 50.0),
            group(&[(-4.0, 2.0), (4.0, 2.0)], Some((0.0, 0.0)), 200.0),
        ];
        let composite = Composite::new(&groups).unwrap();
        assert_eq!(composite.mode, CompositeMode::AboutOwnCentres);
        assert_eq!((composite.groups, composite.shots), (2, 6));
        assert_eq!(composite.left_out, [1]);
        assert_eq!(composite.point_groups, [0, 0, 0, 0, 2, 2]);
        // Degrees of freedom 2 × ((4 − 1) + (2 − 1)) = 8.
        assert_eq!(composite.sigma.degrees_of_freedom, 8.0);
        close(composite.sigma.value, (12e-4f64 / 8.0).sqrt(), 1e-15);
        assert_eq!(composite.scatter_mrad.shots, 6);
        assert_eq!(composite.mrad.impact, None);
        assert_eq!(composite.mrad.extreme_spread_range, None);
        assert_eq!(composite.within_group_sigma, None);
        // Residuals 0.01 mrad from the first centre scaled by √(4/3), and
        // (∓0.02, 0) from the second scaled by √2.
        let (first, second) = (0.01 * (4.0f64 / 3.0).sqrt(), 0.02 * 2f64.sqrt());
        close(composite.points_mrad[4].x, -second, 1e-15);
        close(composite.mrad.extreme_spread, 2.0 * second, 1e-15);
        close(composite.mrad.width, 2.0 * second, 1e-15);
        close(composite.mrad.height, 2.0 * first, 1e-15);
        close(
            composite.mrad.mean_radius,
            (4.0 * first + 2.0 * second) / 6.0,
            1e-15,
        );
        let mm = composite.mm_at(300.0).unwrap();
        close(mm.extreme_spread, 600.0 * second, 1e-12);
        close(mm.sigma.value, composite.sigma.value * 300.0, 1e-12);
        close(
            mm.sigma.range.high,
            composite.mrad.sigma.range.high * 300.0,
            1e-12,
        );
        assert!(composite.mm_at(0.0).is_err() && composite.mm_at(f64::NAN).is_err());
    }

    /// About own centres, the scaled residuals give sizes that read true:
    /// the mean radius of 20 groups of 3 averages √(π/2) σ.
    #[test]
    fn sizes_about_own_centres_are_not_shrunk() {
        let mut rng = Rng::new(17);
        let trials = 300;
        let (mut ratio, mut inside) = (0.0, 0);
        for _ in 0..trials {
            let groups: Vec<ShotGroup> = (0..20)
                .map(|_| ShotGroup {
                    shots: (0..3).map(|_| rng.shot(Point::ORIGIN, 1.0)).collect(),
                    aim: None,
                    distance_m: 1.0,
                })
                .collect();
            let composite = Composite::new(&groups).unwrap();
            ratio += composite.mrad.mean_radius / crate::group::MEAN_RADIUS_IN_SIGMAS;
            if composite
                .mrad
                .mean_radius_range
                .contains(composite.mrad.mean_radius)
            {
                inside += 1;
            }
        }
        // Unscaled it averaged √(2/3) = 0.82 and fell below its range
        // every time.
        let mean = ratio / trials as f64;
        assert!((mean - 1.0).abs() < 0.02, "{mean}");
        assert!(inside as f64 > 0.95 * trials as f64, "{inside}");
    }

    #[test]
    fn about_aims_counts_the_shifts_between_groups() {
        // Two groups the same shape, one 10 mm above its aim at 100 m.
        let groups = [
            group(&[(-1.0, 0.0), (1.0, 0.0)], Some((0.0, 0.0)), 100.0),
            group(&[(49.0, 60.0), (51.0, 60.0)], Some((50.0, 50.0)), 100.0),
        ];
        let composite = Composite::new(&groups).unwrap();
        assert_eq!(composite.mode, CompositeMode::AboutAims);
        // Impacts (∓0.01, 0) and (∓0.01, 0.1): centre (0, 0.05).
        let impact = composite.mrad.impact.unwrap();
        close(impact.x, 0.0, 1e-15);
        close(impact.y, 0.05, 1e-15);
        // About it: Σdx² = 4e-4, Σdy² = 4 × 0.0025 = 0.01; df 2 × 3.
        assert_eq!(composite.sigma.degrees_of_freedom, 6.0);
        close(
            composite.sigma.value,
            ((4e-4 + 0.01) / 6.0f64).sqrt(),
            1e-15,
        );
        let within = composite.within_group_sigma.unwrap();
        assert_eq!(within.degrees_of_freedom, 4.0);
        close(within.value, (4e-4f64 / 4.0).sqrt(), 1e-15);
        assert!(composite.sigma.value > 4.0 * within.value);
        assert_eq!(composite.point_groups, [0, 0, 1, 1]);
        assert!(composite.left_out.is_empty());
        assert_eq!(composite.mrad.extreme_spread_range, None);
        // Forced about own centres, the shift is gone.
        let own = Composite::with_mode(&groups, CompositeMode::AboutOwnCentres).unwrap();
        close(own.sigma.value, within.value, 1e-15);
    }

    #[test]
    fn a_cold_bore_series() {
        let groups = [
            group(&[(3.0, 4.0)], Some((0.0, 0.0)), 100.0),
            group(&[(10.0, 10.0)], Some((10.0, 10.0)), 100.0),
            group(&[(-6.0, -8.0)], Some((0.0, 0.0)), 200.0),
        ];
        let composite = Composite::new(&groups).unwrap();
        assert_eq!(composite.mode, CompositeMode::ColdBore);
        // Points (0.03, 0.04), (0, 0), (−0.03, −0.04): centre 0, df 2 × 2.
        assert_eq!(composite.sigma.degrees_of_freedom, 4.0);
        close(composite.sigma.value, (0.005f64 / 4.0).sqrt(), 1e-15);
        close(composite.mrad.extreme_spread, 0.1, 1e-15);
        let range = composite.mrad.extreme_spread_range.unwrap();
        let in_sigmas = expected_extreme_spread_in_sigmas(3).unwrap();
        close(range.low, composite.mrad.sigma.range.low * in_sigmas, 1e-12);
        assert_eq!(composite.within_group_sigma, None);
        // The same series forced about own centres has no spread left.
        assert_eq!(
            Composite::with_mode(&groups, CompositeMode::AboutOwnCentres),
            Err(AnalysisError::NoSpreadEstimate)
        );
        let mut two_shots = groups.to_vec();
        two_shots[1].shots.push(Point::new(11.0, 10.0));
        assert_eq!(
            Composite::with_mode(&two_shots, CompositeMode::ColdBore),
            Err(AnalysisError::NotOneShot { index: 1, shots: 2 })
        );
    }

    #[test]
    fn moving_and_scaling_change_nothing() {
        let mut rng = Rng::new(3);
        let base: Vec<ShotGroup> = (0..3)
            .map(|i| ShotGroup {
                shots: (0..4)
                    .map(|_| rng.shot(Point::new(i as f64, 2.0), 5.0))
                    .collect(),
                aim: Some(Point::new(0.5 * i as f64, 0.0)),
                distance_m: 100.0 * (i + 1) as f64,
            })
            .collect();
        for mode in [CompositeMode::AboutAims, CompositeMode::AboutOwnCentres] {
            let reference = Composite::with_mode(&base, mode).unwrap();
            // Each group moved with its aim, and each at twice the distance
            // with twice the size.
            let moved: Vec<ShotGroup> = base
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    let shift = Point::new(37.0 * i as f64, -11.0);
                    ShotGroup {
                        shots: g.shots.iter().map(|&s| (s + shift) * 2.0).collect(),
                        aim: g.aim.map(|a| (a + shift) * 2.0),
                        distance_m: g.distance_m * 2.0,
                    }
                })
                .collect();
            let other = Composite::with_mode(&moved, mode).unwrap();
            close(other.sigma.value, reference.sigma.value, 1e-12);
            close(
                other.mrad.extreme_spread,
                reference.mrad.extreme_spread,
                1e-12,
            );
            close(
                other.mrad.cep.range.high,
                reference.mrad.cep.range.high,
                1e-12,
            );
            close(other.mrad.mean_radius, reference.mrad.mean_radius, 1e-12);
            // Order of the groups changes nothing either.
            let reversed: Vec<ShotGroup> = base.iter().rev().cloned().collect();
            let reversed = Composite::with_mode(&reversed, mode).unwrap();
            close(reversed.sigma.value, reference.sigma.value, 1e-12);
            close(
                reversed.mrad.median_radius,
                reference.mrad.median_radius,
                1e-12,
            );
        }
    }

    #[test]
    fn bad_inputs_are_refused() {
        assert_eq!(Composite::new(&[]), Err(AnalysisError::Empty));
        let good = group(&[(0.0, 0.0), (1.0, 1.0)], None, 100.0);
        let refused = |bad: ShotGroup| Composite::new(&[good.clone(), bad]).unwrap_err();
        assert_eq!(
            refused(group(&[], None, 100.0)),
            AnalysisError::Input {
                index: 1,
                error: StatsError::NoShots
            }
        );
        for bad in [
            group(&[(f64::NAN, 0.0)], None, 100.0),
            group(&[(0.0, 0.0)], Some((f64::INFINITY, 0.0)), 100.0),
            group(&[(0.0, 0.0)], None, f64::NAN),
        ] {
            assert_eq!(
                refused(bad),
                AnalysisError::Input {
                    index: 1,
                    error: StatsError::NotFinite
                }
            );
        }
        assert!(matches!(
            refused(group(&[(0.0, 0.0)], None, -5.0)),
            AnalysisError::Input {
                error: StatsError::NotPositive(_),
                ..
            }
        ));
        assert_eq!(
            Composite::with_mode(std::slice::from_ref(&good), CompositeMode::AboutAims),
            Err(AnalysisError::NoAim { index: 0 })
        );
        // One shot in all.
        assert_eq!(
            Composite::new(&[group(&[(0.0, 0.0)], Some((0.0, 0.0)), 100.0)]),
            Err(AnalysisError::TooFew {
                what: "shots",
                need: 2,
                have: 1
            })
        );
        // Sizes that overflow are refused, not hung on.
        let huge = group(&[(1e300, 0.0), (-1e300, 0.0)], None, 1e-300);
        assert_eq!(Composite::new(&[huge]), Err(AnalysisError::NotFinite));
        // All shots in one hole: σ̂ of zero is a figure, not an error.
        let one_hole = group(&[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)], None, 100.0);
        let composite = Composite::new(&[one_hole]).unwrap();
        assert_eq!(composite.sigma.value, 0.0);
    }

    /// The numbers in `docs/group-statistics.md`: `cargo test --release
    /// -p groups-core -- --ignored --nocapture composite_table`.
    #[test]
    #[ignore]
    fn composite_table() {
        let mut rng = Rng::new(2026);
        let trials = 20_000;
        for (groups, shots) in [(3, 5), (5, 3), (4, 10)] {
            for tau in [0.0, 1.0, 2.0] {
                let (mut own_covered, mut aims_covered) = (0, 0);
                let (mut own_spread, mut aims_spread) = (0.0, 0.0);
                for _ in 0..trials {
                    let set: Vec<ShotGroup> = (0..groups)
                        .map(|_| {
                            let centre = Point::new(tau * rng.normal(), tau * rng.normal());
                            ShotGroup {
                                shots: (0..shots).map(|_| rng.shot(centre, 1.0)).collect(),
                                aim: Some(Point::ORIGIN),
                                distance_m: 1.0,
                            }
                        })
                        .collect();
                    let own = Composite::with_mode(&set, CompositeMode::AboutOwnCentres).unwrap();
                    let aims = Composite::new(&set).unwrap();
                    let total = (1.0 + tau * tau).sqrt();
                    own_covered += usize::from(own.mrad.sigma.range.contains(1.0));
                    aims_covered += usize::from(aims.mrad.sigma.range.contains(total));
                    own_spread += own.mrad.extreme_spread;
                    aims_spread += aims.mrad.extreme_spread / total;
                }
                let n = trials as f64;
                let table = expected_extreme_spread_in_sigmas(groups * shots).unwrap();
                println!(
                    "{groups} × {shots}, τ = {tau}σ: σ range covers {:.1}% (own centres), {:.1}% (aims); \
                     mean ES {:.2}σ (own centres), {:.2}σ (aims, in total σ); one group of {}: {table:.2}σ",
                    100.0 * own_covered as f64 / n,
                    100.0 * aims_covered as f64 / n,
                    own_spread / n,
                    aims_spread / n,
                    groups * shots
                );
            }
        }
    }

    /// The pooled σ̂ about own centres covers the true σ 95% of the time
    /// with Σ(nᵢ − 1) degrees of freedom per axis, and the extreme spread
    /// of such a composite is not the one-group table's, so it gets no
    /// range.
    #[test]
    fn pooled_sigma_covers_and_the_table_does_not_apply() {
        let mut rng = Rng::new(11);
        let trials = 2_000;
        let mut covered = [0usize; 2];
        let mut spread_in_sigmas = [0.0f64; 2];
        for _ in 0..trials {
            // Five groups of three at 100 m, centres scattered by a
            // between-group σ of 2 (σ = 1 mm within).
            let groups: Vec<ShotGroup> = (0..5)
                .map(|_| {
                    let centre = Point::new(2.0 * rng.normal(), 2.0 * rng.normal());
                    ShotGroup {
                        shots: (0..3).map(|_| rng.shot(centre, 1.0)).collect(),
                        aim: Some(Point::ORIGIN),
                        distance_m: 100.0,
                    }
                })
                .collect();
            let own = Composite::with_mode(&groups, CompositeMode::AboutOwnCentres).unwrap();
            assert_eq!(own.sigma.degrees_of_freedom, 20.0);
            if own.mrad.sigma.range.contains(0.01) {
                covered[0] += 1;
            }
            spread_in_sigmas[0] += own.mrad.extreme_spread / 0.01;
            // About aims the points' true σ is √(1 + 4) mm per axis.
            let aims = Composite::new(&groups).unwrap();
            if aims.mrad.sigma.range.contains(0.05f64.sqrt() * 0.1) {
                covered[1] += 1;
            }
            spread_in_sigmas[1] += aims.mrad.extreme_spread / (5f64.sqrt() * 0.01);
        }
        let rate = covered[0] as f64 / trials as f64;
        assert!((rate - 0.95).abs() < 0.02, "{rate}");
        // The overlay of five centred, scaled 3-shot groups spreads about
        // 4.35σ; 15 shots from one spread would spread 4.2σ.
        let mean_overlay = spread_in_sigmas[0] / trials as f64;
        let table = expected_extreme_spread_in_sigmas(15).unwrap();
        assert!(mean_overlay > 1.02 * table, "{mean_overlay} v {table}");
        // About aims with shifts as large as these, σ̂'s range undercovers
        // (its degrees of freedom count every shot as independent) and the
        // extreme spread reads short of the table too.
        let rate = covered[1] as f64 / trials as f64;
        let mean_aims = spread_in_sigmas[1] / trials as f64;
        assert!(rate < 0.9, "{rate}");
        assert!(mean_aims < 0.95 * table, "{mean_aims} v {table}");
    }
}

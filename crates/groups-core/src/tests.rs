use crate::distributions::chi_squared_quantile;
use crate::geometry::centroid;
use crate::group::{
    EXPECTED_EXTREME_SPREAD_IN_SIGMAS, MEAN_RADIUS_IN_SIGMAS, circular_radius_in_sigmas,
    expected_extreme_spread_in_sigmas, grubbs_radius, prediction_radius_in_sigmas,
};
use crate::testing::Rng;
use crate::velocity::{FASTEST_MPS, SLOWEST_MPS};
use crate::*;

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} is not within {tolerance} of {expected}"
    );
}

fn near(a: Point, b: Point, tolerance: f64) {
    assert!(
        a.distance(b) <= tolerance,
        "{a:?} is not within {tolerance} of {b:?}"
    );
}

fn points(coordinates: &[(f64, f64)]) -> Vec<Point> {
    coordinates.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

/// Five shots with whole-number coordinates centred on the origin.
fn five() -> Vec<Point> {
    points(&[
        (-3.0, 1.0),
        (2.0, 4.0),
        (1.0, -2.0),
        (4.0, 0.0),
        (-4.0, -3.0),
    ])
}

fn spread(stats: &GroupStats) -> Spread {
    stats.spread.expect("two shots or more")
}

#[test]
fn five_shots_by_hand() {
    let options = Options {
        aim: Some(Point::new(1.0, 2.0)),
        calibre_mm: Some(6.5),
        distance_m: Some(100.0),
    };
    let stats = GroupStats::new(&five(), options).unwrap();
    assert_eq!(stats.shots, 5);
    near(stats.centre, Point::ORIGIN, 1e-15);
    // Impact is centre − aim: low and left. The correction is up and right.
    near(stats.impact.unwrap(), Point::new(-1.0, -2.0), 1e-15);
    near(stats.correction().unwrap(), Point::new(1.0, 2.0), 1e-15);
    close(stats.mrad(stats.impact.unwrap().y).unwrap(), -0.02, 1e-15);
    // Radii √10, √20, √5, 4 and 5.
    let mean_radius = (10f64.sqrt() + 20f64.sqrt() + 5f64.sqrt() + 4.0 + 5.0) / 5.0;
    close(stats.mean_radius, mean_radius, 1e-14);
    close(stats.median_radius, 4.0, 1e-15);
    assert_eq!((stats.width, stats.height), (8.0, 7.0));
    // Σ dx² = 46, Σ dy² = 30, Σ dx dy = 15.
    assert_eq!(
        stats.scatter,
        Scatter {
            shots: 5,
            centres: 1,
            xx: 46.0,
            yy: 30.0,
            xy: 15.0
        }
    );
    let s = spread(&stats);
    // The furthest pair is shots 2 and 5: 6 across and 7 up, √85.
    assert_eq!(s.extreme_spread.shots, (1, 4));
    close(s.extreme_spread.centre_to_centre, 85f64.sqrt(), 1e-14);
    close(
        s.extreme_spread.edge_to_edge.unwrap(),
        85f64.sqrt() + 6.5,
        1e-14,
    );
    // σ̂² = 76 / (2 × 4) = 9.5; its range from χ²(8).
    let sigma = 9.5f64.sqrt();
    close(s.sigma.value, sigma, 1e-15);
    let (low, high) = (
        (76.0 / 17.534_546_139_484_65f64).sqrt(),
        (76.0 / 2.179_730_747_252_649f64).sqrt(),
    );
    close(s.sigma.range.low, low, 1e-9);
    close(s.sigma.range.high, high, 1e-9);
    assert_eq!(stats.sigma().unwrap().degrees_of_freedom, 8.0);
    close(s.mean_radius_range.low, low * MEAN_RADIUS_IN_SIGMAS, 1e-9);
    let es_in_sigmas = expected_extreme_spread_in_sigmas(5).unwrap();
    let es_range = s.extreme_spread.range.unwrap();
    close(es_range.low, low * es_in_sigmas, 1e-9);
    close(es_range.high, high * es_in_sigmas, 1e-9);
    close(s.sd_horizontal, 11.5f64.sqrt(), 1e-15);
    close(s.sd_vertical, 7.5f64.sqrt(), 1e-15);
    close(s.correlation, 15.0 / 1380f64.sqrt(), 1e-15);
    // The principal axes of [[46, 15], [15, 30]]: 55 and 21, the longest
    // at ½ atan(30 / 16) = 30.96°. Round enough for five shots.
    close(s.shape.ratio, (55.0f64 / 21.0).sqrt(), 1e-14);
    close(
        s.shape.angle_degrees,
        0.5 * (30.0f64 / 16.0).atan().to_degrees(),
        1e-12,
    );
    close(s.shape.limit.unwrap(), 5.238, 0.001);
    assert!(!s.shape.elliptical);
    // The centre's standard error is σ̂ / √5 on each axis, on 8 degrees of
    // freedom.
    let error = sigma / 5f64.sqrt();
    assert_eq!(s.centre_standard_error, [error, error]);
    assert_eq!(s.centre_degrees_of_freedom, 8.0);
    let [across, up] = stats.impact_estimate().unwrap();
    close(across.value, -1.0, 1e-15);
    close(up.range.high, -2.0 + 2.306_004_135 * error, 1e-9);
    let [centre_x, _] = stats.centre_estimate().unwrap();
    close(centre_x.range.low, -2.306_004_135 * error, 1e-9);
    assert_eq!(s.radius_method, RadiusMethod::Circular);
    close(s.cep.value, sigma * (2.0 * 2f64.ln()).sqrt(), 1e-14);
    close(s.r90.value, sigma * (2.0 * 10f64.ln()).sqrt(), 1e-14);
    close(s.r95.value, sigma * (2.0 * 20f64.ln()).sqrt(), 1e-14);
    close(s.cep.range.high, high * 1.177_410_022_515_475, 1e-9);
    close(s.r95.range.low, low * 2.447_746_830_680_816, 1e-9);
    // Half the shots land inside CEP.
    close(
        stats.chance_in_circle(stats.centre, s.cep.value).unwrap(),
        0.5,
        1e-12,
    );
}

#[test]
fn radius_factors() {
    close(circular_radius_in_sigmas(0.5), 1.177_410_022_515_475, 1e-15);
    close(circular_radius_in_sigmas(0.9), 2.145_966_026_289_347, 1e-15);
    close(
        circular_radius_in_sigmas(0.95),
        2.447_746_830_680_816,
        1e-15,
    );
    close(
        MEAN_RADIUS_IN_SIGMAS,
        (std::f64::consts::PI / 2.0).sqrt(),
        1e-15,
    );
}

#[test]
fn grubbs_is_exact_at_both_ends() {
    // Round: the Rayleigh radius.
    for p in [0.5, 0.9, 0.95] {
        close(
            grubbs_radius(4.0, 4.0, 0.0, p),
            2.0 * circular_radius_in_sigmas(p),
            1e-12,
        );
    }
    // On a line: the half-normal, R50 = 0.6745σ.
    close(
        grubbs_radius(9.0, 0.0, 0.0, 0.5),
        3.0 * 0.674_489_750_196_082,
        1e-12,
    );
    // A line at 45° is the same line.
    close(
        grubbs_radius(4.5, 4.5, 4.5, 0.5),
        3.0 * 0.674_489_750_196_082,
        1e-12,
    );
    assert_eq!(grubbs_radius(0.0, 0.0, 0.0, 0.5), 0.0);
}

/// `n` shots strung 3 : 1 along `degrees` from the right.
fn strung(rng: &mut Rng, n: usize, degrees: f64) -> Vec<Point> {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (0..n)
        .map(|_| {
            let (along, across) = (3.0 * rng.normal(), rng.normal());
            Point::new(cos * along - sin * across, sin * along + cos * across)
        })
        .collect()
}

#[test]
fn elliptical_groups_use_grubbs() {
    // Strung vertically: SD-V three times SD-H.
    let mut rng = Rng::new(7);
    let shots = strung(&mut rng, 40, 90.0);
    let stats = GroupStats::new(&shots, Options::default()).unwrap();
    let s = spread(&stats);
    assert!(s.shape.elliptical);
    assert!(s.shape.ratio > 2.0);
    assert!(s.shape.angle_degrees.abs() > 80.0);
    assert_eq!(s.shape.limit, Some(1.5));
    assert_eq!(s.radius_method, RadiusMethod::Grubbs);
    let [across, up] = s.centre_standard_error;
    close(across, s.sd_horizontal / 40f64.sqrt(), 1e-15);
    close(up, s.sd_vertical / 40f64.sqrt(), 1e-15);
    assert_eq!(s.centre_degrees_of_freedom, 39.0);
    // Grubbs's CEP holds half the group's own ellipse, within the
    // approximation's error.
    let model = stats.model().unwrap();
    assert_eq!(model.sd_y, s.sd_vertical);
    close(
        stats.chance_in_circle(stats.centre, s.cep.value).unwrap(),
        0.5,
        0.01,
    );
    close(
        stats.chance_in_circle(stats.centre, s.r90.value).unwrap(),
        0.9,
        0.01,
    );
    close(
        stats.chance_in_circle(stats.centre, s.r95.value).unwrap(),
        0.95,
        0.01,
    );
    // The ranges keep the radius's share of σ̂.
    close(
        s.cep.range.low / s.cep.value,
        s.sigma.range.low / s.sigma.value,
        1e-12,
    );
    // The same ratio with five shots is not enough to tell.
    let few = points(&[
        (0.0, 3.0),
        (1.0, -3.0),
        (-1.0, 1.0),
        (1.2, -1.0),
        (-1.2, 0.0),
    ]);
    let s = spread(&GroupStats::new(&few, Options::default()).unwrap());
    assert!(s.shape.ratio > 2.0 && s.shape.ratio < 3.0);
    assert!(!s.shape.elliptical);
    assert_eq!(s.radius_method, RadiusMethod::Circular);
}

#[test]
fn diagonal_groups_are_elliptical_too() {
    let mut rng = Rng::new(7);
    let upright = GroupStats::new(&strung(&mut rng, 40, 90.0), Options::default()).unwrap();
    let mut rng = Rng::new(7);
    let shots = strung(&mut rng, 40, 45.0);
    let stats = GroupStats::new(&shots, Options::default()).unwrap();
    let s = spread(&stats);
    // SD-H and SD-V are alike, but the principal axes are not.
    assert!((s.sd_horizontal / s.sd_vertical - 1.0).abs() < 0.3);
    assert!(s.shape.elliptical);
    close(s.shape.ratio, spread(&upright).shape.ratio, 1e-9);
    close(
        s.shape.angle_degrees,
        spread(&upright).shape.angle_degrees - 45.0,
        1e-9,
    );
    assert_eq!(s.radius_method, RadiusMethod::Grubbs);
    close(s.cep.value, spread(&upright).cep.value, 1e-9);
    // The model keeps the tilt.
    let model = stats.model().unwrap();
    assert_eq!(model.correlation, s.correlation);
    assert!(s.correlation > 0.6);
    close(
        stats.chance_in_circle(stats.centre, s.cep.value).unwrap(),
        0.5,
        0.01,
    );
    // A box along the string holds what the tilted ellipse puts in it,
    // which the same box across the string does not.
    let along = stats
        .chance_in_rectangle(stats.centre, stats.centre + Point::new(5.0, 5.0))
        .unwrap();
    let across = stats
        .chance_in_rectangle(stats.centre, stats.centre + Point::new(5.0, -5.0))
        .unwrap();
    assert!(along > 0.2 && across < 0.15, "{along} {across}");
}

#[test]
fn elliptical_limit_by_group_size() {
    let limit = |n: usize| {
        let scatter = Scatter {
            shots: n,
            centres: 1,
            xx: 1.0,
            yy: 1.0,
            xy: 0.0,
        };
        Shape::of(&scatter).limit
    };
    assert_eq!(limit(4), None);
    close(limit(5).unwrap(), 5.238, 0.001);
    close(limit(10).unwrap(), 2.510, 0.001);
    close(limit(20).unwrap(), 1.810, 0.001);
    assert!(limit(39).unwrap() > 1.5);
    assert_eq!(limit(40), Some(1.5));
}

#[test]
fn round_groups_seldom_look_elliptical() {
    let mut rng = Rng::new(12);
    let groups = 20_000;
    for n in [5, 10, 20] {
        let elliptical = (0..groups)
            .filter(|_| {
                let shots: Vec<Point> = (0..n).map(|_| rng.shot(Point::ORIGIN, 1.0)).collect();
                let centre = centroid(&shots).unwrap();
                Shape::of(&Scatter::about(&shots, centre, true)).elliptical
            })
            .count();
        // The standard error of the share is 0.0015.
        close(elliptical as f64 / groups as f64, 0.05, 0.006);
    }
}

/// Every length and range of `stats`, for comparing groups.
fn lengths(stats: &GroupStats) -> Vec<f64> {
    let s = spread(stats);
    let range = |r: Interval| [r.low, r.high];
    let mut all = vec![
        stats.mean_radius,
        stats.median_radius,
        s.extreme_spread.centre_to_centre,
        s.sigma.value,
        s.cep.value,
        s.r90.value,
        s.r95.value,
    ];
    all.extend(range(s.sigma.range));
    all.extend(range(s.mean_radius_range));
    all.extend(range(s.cep.range));
    all.extend(range(s.extreme_spread.range.unwrap()));
    all
}

#[test]
fn moving_turning_and_scaling_the_group() {
    let mut rng = Rng::new(11);
    let shots: Vec<Point> = (0..10)
        .map(|_| rng.shot(Point::new(2.0, -1.0), 4.0))
        .collect();
    let base = GroupStats::new(&shots, Options::default()).unwrap();
    let base_lengths = lengths(&base);
    assert_eq!(spread(&base).radius_method, RadiusMethod::Circular);

    let offset = Point::new(123.4, -56.7);
    let moved: Vec<Point> = shots.iter().map(|&p| p + offset).collect();
    let stats = GroupStats::new(&moved, Options::default()).unwrap();
    near(stats.centre, base.centre + offset, 1e-12);
    for (a, b) in lengths(&stats).iter().zip(&base_lengths) {
        close(*a, *b, 1e-9);
    }
    close(
        spread(&stats).sd_horizontal,
        spread(&base).sd_horizontal,
        1e-9,
    );

    let (sin, cos) = 37f64.to_radians().sin_cos();
    let turn = |p: Point| Point::new(cos * p.x - sin * p.y, sin * p.x + cos * p.y);
    let turned: Vec<Point> = shots.iter().map(|&p| turn(p)).collect();
    let stats = GroupStats::new(&turned, Options::default()).unwrap();
    near(stats.centre, turn(base.centre), 1e-12);
    for (a, b) in lengths(&stats).iter().zip(&base_lengths) {
        close(*a, *b, 1e-9);
    }
    // Turning moves spread between the axes, never σ̂ or the shape.
    let (h, v) = (spread(&stats).sd_horizontal, spread(&stats).sd_vertical);
    close(
        ((h * h + v * v) / 2.0).sqrt(),
        spread(&base).sigma.value,
        1e-12,
    );
    close(spread(&stats).shape.ratio, spread(&base).shape.ratio, 1e-9);
    let turned_by = spread(&stats).shape.angle_degrees - spread(&base).shape.angle_degrees;
    close(turned_by.rem_euclid(180.0), 37.0, 1e-9);

    let factor = 2.5;
    let scaled: Vec<Point> = shots.iter().map(|&p| p * factor).collect();
    let stats = GroupStats::new(&scaled, Options::default()).unwrap();
    for (a, b) in lengths(&stats).iter().zip(&base_lengths) {
        close(*a, b * factor, 1e-9);
    }
    close(spread(&stats).correlation, spread(&base).correlation, 1e-12);
    close(spread(&stats).shape.ratio, spread(&base).shape.ratio, 1e-12);
    assert_eq!(stats.width, base.width * factor);
}

#[test]
fn one_two_and_three_shots() {
    let aim = Options {
        aim: Some(Point::new(0.0, 0.0)),
        ..Options::default()
    };
    let one = GroupStats::new(&[Point::new(3.0, 4.0)], aim).unwrap();
    assert_eq!(one.centre, Point::new(3.0, 4.0));
    assert_eq!(one.impact, Some(Point::new(3.0, 4.0)));
    assert_eq!((one.mean_radius, one.median_radius), (0.0, 0.0));
    assert_eq!((one.width, one.height), (0.0, 0.0));
    assert_eq!(one.spread, None);
    assert_eq!(one.sigma(), None);
    assert_eq!(one.chance_in_circle(Point::ORIGIN, 10.0), None);

    // Two shots d apart: σ̂ = d / 2 on 2 degrees of freedom.
    let two = GroupStats::new(&points(&[(0.0, 0.0), (6.0, 8.0)]), aim).unwrap();
    let s = spread(&two);
    close(s.extreme_spread.centre_to_centre, 10.0, 1e-15);
    close(s.sigma.value, 5.0, 1e-15);
    close(two.mean_radius, 5.0, 1e-15);
    close(two.median_radius, 5.0, 1e-15);
    // χ²(2) quantiles are −2 ln(1 − p).
    close(
        s.sigma.range.low,
        5.0 * (2.0 / (-2.0 * 0.025f64.ln())).sqrt(),
        1e-12,
    );
    close(
        s.sigma.range.high,
        5.0 * (2.0 / (-2.0 * 0.975f64.ln())).sqrt(),
        1e-9,
    );
    close(
        s.extreme_spread.range.unwrap().low / s.sigma.range.low,
        std::f64::consts::PI.sqrt(),
        0.005,
    );
    assert_eq!(s.shape.limit, None);
    assert!(!s.shape.elliptical);

    let three = GroupStats::new(&points(&[(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)]), aim).unwrap();
    let s = spread(&three);
    near(three.centre, Point::new(2.0, 1.0), 1e-15);
    // Radii √5, √5 and 2; Σ r² = 14 on 4 degrees of freedom.
    close(three.median_radius, 5f64.sqrt(), 1e-15);
    close(s.sigma.value, (14.0f64 / 4.0).sqrt(), 1e-15);
    close(s.extreme_spread.centre_to_centre, 4.0, 1e-15);
    assert_eq!(s.extreme_spread.shots, (0, 1));
    close(
        s.sigma.range.high,
        (14.0 / chi_squared_quantile(0.025, 4.0)).sqrt(),
        1e-12,
    );
}

#[test]
fn degenerate_groups() {
    assert_eq!(
        GroupStats::new(&[], Options::default()),
        Err(StatsError::NoShots)
    );
    assert_eq!(
        GroupStats::new(&[Point::new(f64::NAN, 0.0)], Options::default()),
        Err(StatsError::NotFinite)
    );
    let shots = five();
    let bad = |options: Options| GroupStats::new(&shots, options).unwrap_err();
    let calibre = |mm| Options {
        calibre_mm: Some(mm),
        ..Options::default()
    };
    assert_eq!(bad(calibre(0.0)), StatsError::NotPositive("calibre"));
    assert_eq!(bad(calibre(f64::INFINITY)), StatsError::NotFinite);
    assert_eq!(
        bad(Options {
            distance_m: Some(-100.0),
            ..Options::default()
        }),
        StatsError::NotPositive("distance")
    );
    assert_eq!(
        bad(Options {
            aim: Some(Point::new(0.0, f64::NAN)),
            ..Options::default()
        }),
        StatsError::NotFinite
    );

    // Every shot through one hole: no spread at all.
    let hole = vec![Point::new(5.0, 5.0); 6];
    let stats = GroupStats::new(&hole, Options::default()).unwrap();
    let s = spread(&stats);
    assert_eq!(s.sigma.value, 0.0);
    assert_eq!(
        s.sigma.range,
        Interval {
            low: 0.0,
            high: 0.0
        }
    );
    assert_eq!((s.cep.value, s.r95.value), (0.0, 0.0));
    assert_eq!(s.shape.ratio, 1.0);
    assert!(!s.shape.elliptical);
    assert_eq!(stats.chance_in_circle(Point::new(5.5, 5.0), 1.0), Some(1.0));
    assert_eq!(stats.chance_in_circle(Point::new(9.0, 5.0), 1.0), Some(0.0));

    // Shots on one horizontal line: infinitely long, and elliptical.
    let line = points(&[(0.0, 2.0), (1.0, 2.0), (3.0, 2.0), (4.5, 2.0), (7.0, 2.0)]);
    let s = spread(&GroupStats::new(&line, Options::default()).unwrap());
    assert_eq!(s.sd_vertical, 0.0);
    assert_eq!(s.shape.ratio, f64::INFINITY);
    assert_eq!(s.shape.angle_degrees, 0.0);
    assert!(s.shape.elliptical);
    close(s.cep.value, s.sd_horizontal * 0.674_489_750_196_082, 1e-12);
    // The same along the diagonal.
    let diagonal: Vec<Point> = line.iter().map(|p| Point::new(p.x, p.x)).collect();
    let s = spread(&GroupStats::new(&diagonal, Options::default()).unwrap());
    assert_eq!(s.shape.ratio, f64::INFINITY);
    close(s.shape.angle_degrees, 45.0, 1e-12);
}

#[test]
fn numbers_too_large_are_refused_not_hung_on() {
    // Coordinates near 1e155 square past f64's range.
    let huge = points(&[(1e155, 0.0), (-1e155, 0.0), (0.0, 1e155)]);
    assert_eq!(
        GroupStats::new(&huge, Options::default()),
        Err(StatsError::NotFinite)
    );
    assert_eq!(
        flier_check(&[huge, five()].concat(), Options::default()),
        Err(StatsError::NotFinite)
    );
    let wide = points(&[(-1.7e308, 0.0), (1.7e308, 0.0)]);
    assert_eq!(
        GroupStats::new(&wide, Options::default()),
        Err(StatsError::NotFinite)
    );
    let shots = five();
    let centre = Point::ORIGIN;
    let mut scatter = Scatter::about(&shots, centre, true);
    scatter.xx = f64::INFINITY;
    assert_eq!(
        GroupStats::with_scatter(&shots, centre, scatter, Options::default()),
        Err(StatsError::NotFinite)
    );
    scatter.xx = -1.0;
    assert_eq!(
        GroupStats::with_scatter(&shots, centre, scatter, Options::default()),
        Err(StatsError::NotFinite)
    );
    let short = Scatter::about(&shots[1..], centre, true);
    assert_eq!(
        GroupStats::with_scatter(&shots, centre, short, Options::default()),
        Err(StatsError::LengthMismatch)
    );
    // Models no group can give.
    for model in [
        ShotModel::circular(Point::ORIGIN, f64::INFINITY),
        ShotModel::circular(Point::ORIGIN, f64::NAN),
        ShotModel::circular(Point::new(f64::INFINITY, 0.0), 1.0),
        ShotModel {
            centre: Point::ORIGIN,
            sd_x: -1.0,
            sd_y: 1.0,
            correlation: 0.0,
        },
        ShotModel {
            centre: Point::ORIGIN,
            sd_x: 1.0,
            sd_y: 1.0,
            correlation: 1.5,
        },
    ] {
        assert!(!model.is_valid());
        assert!(model.chance_in_circle(Point::new(1.0, 0.0), 1.0).is_nan());
        assert!(
            model
                .chance_in_rectangle(Point::ORIGIN, Point::new(1.0, 1.0))
                .is_nan()
        );
    }
}

#[test]
fn hit_chances_count_the_offset() {
    let stats = GroupStats::new(&five(), Options::default()).unwrap();
    let sigma = spread(&stats).sigma.value;
    // A circle on an aim point 3σ̂ off holds far less than one on the group.
    let on_group = stats.chance_in_circle(stats.centre, 2.0 * sigma).unwrap();
    let off_group = stats
        .chance_in_circle(stats.centre + Point::new(0.0, 3.0 * sigma), 2.0 * sigma)
        .unwrap();
    close(on_group, 1.0 - (-2.0f64).exp(), 1e-12);
    assert!(off_group < 0.3 && off_group > 0.01);
    // A rectangle round the centre ±σ̂ each way: (Φ(1) − Φ(−1))².
    let box_chance = stats
        .chance_in_rectangle(
            stats.centre - Point::new(sigma, sigma),
            stats.centre + Point::new(sigma, sigma),
        )
        .unwrap();
    close(box_chance, 0.682_689_492_137_086f64.powi(2), 1e-12);
}

#[test]
fn pooled_scatter_adds_degrees_of_freedom() {
    let a = five();
    let b: Vec<Point> = five()
        .iter()
        .map(|&p| p * 0.5 + Point::new(40.0, 0.0))
        .collect();
    let scatter_a = GroupStats::new(&a, Options::default()).unwrap().scatter;
    let scatter_b = GroupStats::new(&b, Options::default()).unwrap().scatter;
    let pooled = scatter_a + scatter_b;
    assert_eq!((pooled.shots, pooled.centres), (10, 2));
    let sigma = pooled.sigma().unwrap();
    assert_eq!(sigma.degrees_of_freedom, 16.0);
    close(sigma.value, ((76.0 + 19.0) / 16.0f64).sqrt(), 1e-14);
    // Both groups moved onto one point, measured with the pooled scatter.
    let moved: Vec<Point> = a
        .iter()
        .copied()
        .chain(b.iter().map(|&p| p - Point::new(40.0, 0.0)))
        .collect();
    let stats =
        GroupStats::with_scatter(&moved, Point::ORIGIN, pooled, Options::default()).unwrap();
    assert_eq!(stats.shots, 10);
    close(spread(&stats).sigma.value, sigma.value, 1e-15);
    // About a known point, no centre is worked out from the shots.
    let known = Scatter::about(&a, Point::new(1.0, 1.0), false);
    assert_eq!(known.axis_degrees_of_freedom(), 5);
    close(
        known.sigma().unwrap().value,
        ((76.0 + 10.0) / 10.0f64).sqrt(),
        1e-14,
    );
}

/// The mean and standard error of the extreme spread (in σ) of groups of
/// 2 to 50 shots from a round normal spread, from `samples` strings of
/// 50 shots measured after each shot.
fn monte_carlo_extreme_spread(samples: usize) -> Vec<(f64, f64)> {
    let mut rng = Rng::new(0x4d58_5343_4553);
    let mut sum = [0.0; 49];
    let mut sum_squares = [0.0; 49];
    let mut shots = [Point::ORIGIN; 50];
    for _ in 0..samples {
        let mut widest: f64 = 0.0;
        for n in 0..50 {
            shots[n] = rng.shot(Point::ORIGIN, 1.0);
            for earlier in &shots[..n] {
                let d = shots[n] - *earlier;
                widest = widest.max(d.x * d.x + d.y * d.y);
            }
            if n > 0 {
                let spread = widest.sqrt();
                sum[n - 1] += spread;
                sum_squares[n - 1] += spread * spread;
            }
        }
    }
    let count = samples as f64;
    (0..49)
        .map(|k| {
            let mean = sum[k] / count;
            let variance = (sum_squares[k] / count - mean * mean).max(0.0);
            (mean, (variance / count).sqrt())
        })
        .collect()
}

/// Checks the table against `samples` strings of shots, within 4
/// standard errors and the table's rounding.
fn check_extreme_spread_table(samples: usize) -> Vec<(f64, f64)> {
    let table = monte_carlo_extreme_spread(samples);
    for (k, &(mean, error)) in table.iter().enumerate() {
        close(
            EXPECTED_EXTREME_SPREAD_IN_SIGMAS[k],
            mean,
            4.0 * error + 0.0005,
        );
    }
    table
}

#[test]
fn expected_extreme_spread_is_the_monte_carlo_one() {
    check_extreme_spread_table(20_000);
    // Two shots: their distance is Rayleigh with scale σ√2, mean σ√π.
    assert_eq!(EXPECTED_EXTREME_SPREAD_IN_SIGMAS[0], 1.772);
    close(
        EXPECTED_EXTREME_SPREAD_IN_SIGMAS[0],
        std::f64::consts::PI.sqrt(),
        0.0006,
    );
    let pinned = [
        (3, 2.409),
        (5, 3.066),
        (10, 3.811),
        (20, 4.451),
        (50, 5.188),
    ];
    for (shots, value) in pinned {
        assert_eq!(expected_extreme_spread_in_sigmas(shots), Some(value));
    }
    // The values the dispersion model has used for 3 and 5 shots.
    close(expected_extreme_spread_in_sigmas(3).unwrap(), 2.41, 0.01);
    close(expected_extreme_spread_in_sigmas(5).unwrap(), 3.07, 0.01);
    assert_eq!(expected_extreme_spread_in_sigmas(1), None);
    assert_eq!(expected_extreme_spread_in_sigmas(51), None);
    assert!(
        EXPECTED_EXTREME_SPREAD_IN_SIGMAS
            .windows(2)
            .all(|w| w[0] < w[1])
    );
}

/// The table as made: `cargo test --release -p groups-core --
/// --ignored --nocapture expected_extreme_spread` checks it against ten
/// million strings and prints it.
#[test]
#[ignore = "ten million strings of shots: run with --release"]
fn expected_extreme_spread_from_ten_million_strings() {
    let table = check_extreme_spread_table(10_000_000);
    let values: Vec<String> = table.iter().map(|(mean, _)| format!("{mean:.3}")).collect();
    println!("[{}]", values.join(", "));
    for (k, &(mean, _)) in table.iter().enumerate() {
        close(EXPECTED_EXTREME_SPREAD_IN_SIGMAS[k], mean, 0.0005 + 1e-12);
    }
}

#[test]
fn sigma_range_covers_95_percent() {
    let mut rng = Rng::new(2026);
    let groups = 10_000;
    for n in [2, 3, 5, 10] {
        let mut covered = 0;
        for _ in 0..groups {
            let shots: Vec<Point> = (0..n)
                .map(|_| rng.shot(Point::new(10.0, 5.0), 2.0))
                .collect();
            let stats = GroupStats::new(&shots, Options::default()).unwrap();
            if spread(&stats).sigma.range.contains(2.0) {
                covered += 1;
            }
        }
        let coverage = covered as f64 / groups as f64;
        // The standard error of the coverage is 0.0022.
        close(coverage, 0.95, 0.008);
    }
}

#[test]
fn ranges_of_mean_radius_and_cep_cover_their_true_values() {
    let mut rng = Rng::new(99);
    let groups = 4_000;
    let sigma = 3.0;
    let (mut mean_radius, mut cep, mut extreme) = (0, 0, 0);
    let n = 5;
    let mut es_sum = 0.0;
    for _ in 0..groups {
        let shots: Vec<Point> = (0..n).map(|_| rng.shot(Point::ORIGIN, sigma)).collect();
        let s = spread(&GroupStats::new(&shots, Options::default()).unwrap());
        mean_radius += usize::from(s.mean_radius_range.contains(MEAN_RADIUS_IN_SIGMAS * sigma));
        cep += usize::from(s.cep.range.contains(circular_radius_in_sigmas(0.5) * sigma));
        let expected = expected_extreme_spread_in_sigmas(n).unwrap() * sigma;
        extreme += usize::from(s.extreme_spread.range.unwrap().contains(expected));
        es_sum += s.extreme_spread.centre_to_centre;
    }
    let share = |count: usize| count as f64 / groups as f64;
    // All three ranges are σ's range scaled, so they cover as σ's does.
    close(share(mean_radius), 0.95, 0.012);
    close(share(cep), 0.95, 0.012);
    close(share(extreme), 0.95, 0.012);
    close(
        es_sum / groups as f64 / sigma,
        expected_extreme_spread_in_sigmas(n).unwrap(),
        0.04,
    );
}

/// Eight shots round the origin, perfectly round: Σ dx² = Σ dy² = 40.
fn eight() -> Vec<Point> {
    points(&[
        (3.0, 1.0),
        (3.0, -1.0),
        (-3.0, 1.0),
        (-3.0, -1.0),
        (1.0, 3.0),
        (1.0, -3.0),
        (-1.0, 3.0),
        (-1.0, -3.0),
    ])
}

#[test]
fn a_flier_is_suggested_with_the_group_without_it() {
    let options = Options {
        aim: Some(Point::ORIGIN),
        ..Options::default()
    };
    // The other eight: σ̂² = 80 / 14, and F(2, 14) at 99% is
    // 7 (0.01^(−1/7) − 1) = 6.515.
    let rest = (80.0f64 / 14.0).sqrt();
    let f = 7.0 * (0.01f64.powf(-1.0 / 7.0) - 1.0);
    let radius = rest * (2.0 * 1.125 * f).sqrt();
    let mut shots = eight();
    shots.push(Point::new(12.0, 0.0));
    let flier = flier_check(&shots, options).unwrap().unwrap();
    assert_eq!(flier.test, FlierTest::Round);
    assert_eq!(flier.shot, 8);
    close(flier.distance_mm, 12.0, 1e-12);
    close(flier.rest_radius_mm, radius, 1e-9);
    close(
        flier.rest_radius_mm,
        rest * prediction_radius_in_sigmas(8, 0.99),
        1e-9,
    );
    let all = GroupStats::new(&shots, options).unwrap();
    close(
        flier.sigma_cut,
        1.0 - rest / spread(&all).sigma.value,
        1e-12,
    );
    assert_eq!(flier.without, GroupStats::new(&eight(), options).unwrap());

    // Leaving out a shot 10.33 mm out cuts σ̂ by 27%: enough for 25%.
    let mut shots = eight();
    shots.push(Point::new(10.33, 0.0));
    let flier = flier_check(&shots, options).unwrap().unwrap();
    assert!(
        flier.sigma_cut > 0.25 && flier.sigma_cut < 0.3,
        "{}",
        flier.sigma_cut
    );
    // 9.6 mm is beyond R99 (9.15 mm), but cuts σ̂ by less than 25%.
    shots[8] = Point::new(9.6, 0.0);
    assert!(9.6 > radius);
    assert_eq!(flier_check(&shots, options).unwrap(), None);
    // 8.9 mm cuts σ̂ by 23%, but is within R99.
    shots[8] = Point::new(8.9, 0.0);
    assert_eq!(flier_check(&shots, options).unwrap(), None);

    // The five shots alone have none; four are too few to tell.
    assert_eq!(flier_check(&five(), options).unwrap(), None);
    assert_eq!(flier_check(&five()[1..], options).unwrap(), None);
    // Four shots through one hole and one beside it.
    let mut hole = vec![Point::new(1.0, 1.0); 4];
    hole.push(Point::new(4.0, 1.0));
    let flier = flier_check(&hole, options).unwrap().unwrap();
    assert_eq!(flier.shot, 4);
    assert_eq!(flier.sigma_cut, 1.0);
    assert_eq!(flier_check(&[], options), Err(StatsError::NoShots));
}

#[test]
fn a_flier_in_an_elliptical_group_is_judged_against_its_ellipse() {
    // Nine shots strung up and down: Σ dx² = 4, Σ dy² = 176, Σ dx dy = 0.
    let string = points(&[
        (0.0, -8.0),
        (0.0, 8.0),
        (0.0, -4.0),
        (0.0, 4.0),
        (1.0, 2.0),
        (-1.0, -2.0),
        (1.0, -2.0),
        (-1.0, 2.0),
        (0.0, 0.0),
    ]);
    let options = Options::default();
    let mut shots = string.clone();
    shots.push(Point::new(0.0, 30.0));
    assert!(
        spread(&GroupStats::new(&shots, options).unwrap())
            .shape
            .elliptical
    );
    let flier = flier_check(&shots, options).unwrap().unwrap();
    assert_eq!(flier.test, FlierTest::Elliptical);
    assert_eq!(flier.shot, 9);
    // S = diag(0.5, 22); d = (0, 30): d S⁻¹ d = 900 / 22, T² = 9/10 of it,
    // F = T² × 7/16; the limit is F(2, 7) at 99%, 3.5 (0.01^(−2/7) − 1).
    let statistic = 900.0 / 22.0 * 0.9 * 7.0 / 16.0;
    let limit = 3.5 * (0.01f64.powf(-2.0 / 7.0) - 1.0);
    close(
        flier.rest_radius_mm,
        30.0 * (limit / statistic).sqrt(),
        1e-9,
    );
    // Along the string 16 mm out is beyond the round R99 and cuts σ̂ by
    // 30%, but it is within the string's own ellipse.
    shots[9] = Point::new(0.0, 16.0);
    let stats = GroupStats::new(&shots, options).unwrap();
    assert!(spread(&stats).shape.elliptical);
    assert_eq!(flier_check(&shots, options).unwrap(), None);
}

#[test]
fn a_wide_shot_in_a_big_group_cuts_too_little() {
    // 30 shots and one 4σ out: beyond R99 of the rest, but leaving it out
    // cuts σ̂ by far less than a quarter.
    let mut rng = Rng::new(5);
    let mut shots: Vec<Point> = (0..30).map(|_| rng.shot(Point::ORIGIN, 1.0)).collect();
    let stats = GroupStats::new(&shots, Options::default()).unwrap();
    let radius = prediction_radius_in_sigmas(30, 0.99);
    assert!(radius > 3.1 && radius < 3.3);
    let far = stats.centre + Point::new(4.2 * spread(&stats).sigma.value, 0.0);
    shots.push(far);
    assert_eq!(flier_check(&shots, Options::default()).unwrap(), None);
}

#[test]
fn round_groups_seldom_show_a_flier() {
    let mut rng = Rng::new(31);
    let groups = 2_000;
    let mut rates = Vec::new();
    for n in [5, 10, 20] {
        let flagged = (0..groups)
            .filter(|_| {
                let shots: Vec<Point> = (0..n).map(|_| rng.shot(Point::ORIGIN, 1.0)).collect();
                flier_check(&shots, Options::default()).unwrap().is_some()
            })
            .count();
        rates.push(flagged as f64 / groups as f64);
    }
    println!("flier suggested in round groups of 5, 10, 20 shots: {rates:?}");
    // At most n × P(F(2, 2n − 4) > the limit): 5.0%, 3.9% and 0.02%.
    assert!(
        rates[0] < 0.065 && rates[1] < 0.05 && rates[2] < 0.002,
        "{rates:?}"
    );
}

#[test]
fn strung_groups_seldom_show_a_flier() {
    // Honest 3 : 1 strings, which judged as round groups would be flagged
    // 13% (5 shots) and 14% (10 shots) of the time.
    let mut rng = Rng::new(41);
    let groups = 2_000;
    let mut rates = Vec::new();
    for n in [5, 10] {
        let flagged = (0..groups)
            .filter(|_| {
                let shots = strung(&mut rng, n, 90.0);
                flier_check(&shots, Options::default()).unwrap().is_some()
            })
            .count();
        rates.push(flagged as f64 / groups as f64);
    }
    println!("flier suggested in 3 : 1 strings of 5, 10 shots: {rates:?}");
    assert!(rates[0] < 0.1 && rates[1] < 0.075, "{rates:?}");
}

#[test]
fn velocity_statistics() {
    let readings = [790.0, 795.0, 788.0, 801.0, 796.0];
    let stats = VelocityStats::new(&readings).unwrap();
    assert_eq!(stats.shots, 5);
    close(stats.average, 794.0, 1e-12);
    assert_eq!(
        (stats.slowest, stats.fastest, stats.extreme_spread),
        (788.0, 801.0, 13.0)
    );
    // Σ (v − v̄)² = 16 + 1 + 36 + 49 + 4 = 106 on 4 degrees of freedom.
    let sd = stats.sd.unwrap();
    close(sd.value, (106.0f64 / 4.0).sqrt(), 1e-12);
    close(sd.range.low, (106.0 / 11.143_286_781_877_8f64).sqrt(), 1e-9);
    close(
        sd.range.high,
        (106.0 / 0.484_418_557_087_86f64).sqrt(),
        1e-9,
    );
    assert_eq!(VelocityStats::new(&[800.0]).unwrap().sd, None);
    assert_eq!(VelocityStats::new(&[]), Err(StatsError::NoShots));
    // A reading in hundreds of metres a second without its decimal point
    // is refused, not guessed at.
    assert_eq!(
        VelocityStats::new(&[790.0, 2850.0, 795.0]),
        Err(StatsError::ImplausibleVelocity {
            index: 1,
            mps: 2850.0
        })
    );
    assert_eq!(
        VelocityStats::new(&[790.0, f64::NAN]),
        Err(StatsError::NotFinite)
    );
    assert!(VelocityStats::new(&[SLOWEST_MPS, FASTEST_MPS]).is_ok());
    assert!(VelocityStats::new(&[SLOWEST_MPS - 0.1]).is_err());
    // The same bounds as the chronograph import: 10 to 2,500 m/s.
    assert!(VelocityStats::new(&[12.0, 2_499.0]).is_ok());
    assert!(VelocityStats::new(&[9.9]).is_err());
    assert!(VelocityStats::new(&[2_500.1]).is_err());
    assert!(
        VelocityStats::new(&[790.0, 795.0])
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn velocity_readings_worth_a_second_look() {
    // 2,450 is feet per second typed as metres per second; 79.0 a slipped
    // decimal point; an airgun's 50 m/s is accepted, as unusual.
    let readings = [790.0, 795.0, 2_450.0, 788.0, 79.0, 801.0];
    let stats = VelocityStats::new(&readings).unwrap();
    assert_eq!(
        stats.warnings,
        vec![
            VelocityWarning::Unusual {
                index: 2,
                mps: 2_450.0
            },
            VelocityWarning::FarFromMedian {
                index: 2,
                mps: 2_450.0,
                median_mps: 792.5
            },
            VelocityWarning::Unusual {
                index: 4,
                mps: 79.0
            },
            VelocityWarning::FarFromMedian {
                index: 4,
                mps: 79.0,
                median_mps: 792.5
            },
        ]
    );
    assert_eq!(
        velocity_warnings(&[50.0, 51.0]),
        vec![
            VelocityWarning::Unusual {
                index: 0,
                mps: 50.0
            },
            VelocityWarning::Unusual {
                index: 1,
                mps: 51.0
            },
        ]
    );
    // 9% from the median is a string's own spread, if a wide one.
    assert!(velocity_warnings(&[800.0, 810.0, 880.0]).is_empty());
    assert_eq!(velocity_warnings(&[800.0, 810.0, 891.0]).len(), 1);
    // Two readings have no median to judge by.
    assert!(velocity_warnings(&[300.0, 900.0]).is_empty());
}

#[test]
fn velocity_against_vertical() {
    // Heights exactly 0.1 mm per m/s, plus a known scatter.
    let velocities = [780.0, 790.0, 800.0, 810.0, 820.0, 785.0];
    let noise = [0.3, -0.2, 0.1, -0.4, 0.2, 0.0];
    let shots: Vec<Point> = velocities
        .iter()
        .zip(noise)
        .map(|(v, e)| Point::new(1.0, 0.1 * (v - 800.0) + e + 50.0))
        .collect();
    let with_velocities: Vec<Option<f64>> = velocities.iter().map(|&v| Some(v)).collect();
    let fit = VelocityVertical::new(&shots, &with_velocities).unwrap();
    assert_eq!(fit.shots, 6);
    // The least-squares slope, worked out separately.
    let mean_v = velocities.iter().sum::<f64>() / 6.0;
    let heights: Vec<f64> = shots.iter().map(|p| p.y).collect();
    let mean_h = heights.iter().sum::<f64>() / 6.0;
    let vv: f64 = velocities.iter().map(|v| (v - mean_v).powi(2)).sum();
    let vh: f64 = velocities
        .iter()
        .zip(&heights)
        .map(|(v, h)| (v - mean_v) * (h - mean_h))
        .sum();
    let hh: f64 = heights.iter().map(|h| (h - mean_h).powi(2)).sum();
    close(fit.slope.value, vh / vv, 1e-12);
    close(fit.mean_velocity, mean_v, 1e-12);
    close(fit.mean_height, 0.0, 1e-12);
    close(fit.intercept, -fit.slope.value * mean_v, 1e-9);
    close(fit.r_squared, vh * vh / (vv * hh), 1e-12);
    assert!(fit.r_squared > 0.95);
    let se = ((hh - vh * vh / vv) / 4.0 / vv).sqrt();
    close(fit.slope_standard_error, se, 1e-12);
    close(
        fit.slope.range.high - fit.slope.value,
        2.776_445_105 * se,
        1e-9,
    );
    assert!(fit.slope.range.contains(0.1));

    // Shots without a velocity are left out; five pairs are needed.
    let mut partial = with_velocities.clone();
    partial[0] = None;
    assert_eq!(VelocityVertical::new(&shots, &partial).unwrap().shots, 5);
    partial[1] = None;
    assert_eq!(
        VelocityVertical::new(&shots, &partial),
        Err(StatsError::TooFewShots { need: 5, have: 4 })
    );
    assert_eq!(
        VelocityVertical::new(&shots[1..], &with_velocities),
        Err(StatsError::LengthMismatch)
    );
    assert_eq!(
        VelocityVertical::fit(&[800.0; 5], &[1.0, 2.0, 3.0, 4.0, 5.0]),
        Err(StatsError::NoVelocitySpread)
    );
    assert_eq!(
        VelocityVertical::fit(
            &[800.0, 810.0, 8.0, 805.0, 795.0],
            &[1.0, 2.0, 3.0, 4.0, 5.0]
        ),
        Err(StatsError::ImplausibleVelocity { index: 2, mps: 8.0 })
    );
    // Heights too large to square are refused, not reported as a fit.
    assert_eq!(
        VelocityVertical::fit(
            &[790.0, 800.0, 810.0, 805.0, 795.0],
            &[1e300, -1e300, 1e300, -1e300, 0.0]
        ),
        Err(StatsError::NotFinite)
    );
    // Level shots: nothing to explain.
    let level = VelocityVertical::fit(&[790.0, 800.0, 810.0, 795.0, 805.0], &[0.0; 5]).unwrap();
    assert_eq!((level.slope.value, level.r_squared), (0.0, 0.0));
}

use crate::analysis;
use crate::solver::{self, GRAVITY};
use crate::*;

/// A typical .308 Win hunting load: 11.7 g (180 gr) bullet, G1 0.45.
fn scenario() -> Scenario {
    Scenario {
        bullet: Bullet {
            mass_kg: 0.0117,
            diameter_m: 0.00782,
            length_m: Some(0.031),
            ballistic_coefficient: 0.45,
            drag_model: DragModel::G1,
            bc_bands: BcBands::NONE,
            drag_curve: None,
        },
        rifle: Rifle {
            sight_height_m: 0.045,
            zero_range_m: 100.0,
            zero_height_m: 0.0,
            twist_m: 11.0 * 0.0254,
            right_hand_twist: true,
        },
        load: Load {
            muzzle_velocity: 790.0,
            reference_temperature_c: 15.0,
            temperature_sensitivity: 0.0,
        },
        zero_atmosphere: Atmosphere::standard(),
        zero_powder_temperature_c: 15.0,
        impact_offset: [0.0, 0.0],
        shot: Conditions {
            atmosphere: Atmosphere::standard(),
            powder_temperature_c: 15.0,
            wind: Wind::CALM,
            inclination_degrees: 0.0,
            coriolis: None,
            cant_degrees: 0.0,
        },
        spin_drift: true,
        aerodynamic_jump: true,
    }
}

fn row(scenario: &Scenario, range: f64) -> RangeRow {
    solver::solve(scenario, range + 10.0)
        .unwrap()
        .at(range)
        .unwrap()
}

#[test]
fn without_drag_the_path_is_a_parabola() {
    let mut vacuum = scenario();
    vacuum.bullet.ballistic_coefficient = 1e12;
    vacuum.rifle.sight_height_m = 0.0;
    vacuum.spin_drift = false;
    let samples = solver::fly_level(&vacuum, 0.0, |sample| sample.range < 300.0);
    let at = solver::interpolate(&samples, 300.0).unwrap();
    let expected = -0.5 * GRAVITY * (300.0_f64 / 790.0).powi(2);
    assert!(
        (at.drop - expected).abs() < 1e-6,
        "{} vs {expected}",
        at.drop
    );
    assert!((at.velocity - 790.0_f64.hypot(GRAVITY * at.time)).abs() < 1e-6);
}

#[test]
fn zeroed_rifle_hits_the_crosshair_at_the_zero_range() {
    let zeroed = row(&scenario(), 100.0);
    assert!(zeroed.drop.abs() < 1e-5);
    assert!(zeroed.windage.abs() < 1e-5);
    let muzzle = solver::solve(&scenario(), 10.0).unwrap().at(0.0).unwrap();
    assert!((muzzle.drop + 0.045).abs() < 1e-9);
}

#[test]
fn bullet_slows_and_drops_further_out() {
    let solution = solver::solve(&scenario(), 500.0).unwrap();
    let rows: Vec<_> = (2..=5)
        .map(|i| solution.at(i as f64 * 100.0).unwrap())
        .collect();
    assert!(rows.windows(2).all(|pair| pair[1].drop < pair[0].drop));
    assert!(
        rows.windows(2)
            .all(|pair| pair[1].velocity < pair[0].velocity)
    );
    assert!(
        rows.windows(2)
            .all(|pair| pair[1].elevation_correction > pair[0].elevation_correction)
    );
}

#[test]
fn crosswind_pushes_the_bullet_downwind() {
    let mut from_right = scenario();
    from_right.spin_drift = false;
    from_right.shot.wind = Wind {
        speed: 5.0,
        from_degrees: 90.0,
    };
    let mut from_left = from_right.clone();
    from_left.shot.wind.from_degrees = 270.0;
    let right = row(&from_right, 300.0);
    let left = row(&from_left, 300.0);
    assert!(right.windage < -0.1);
    assert!((right.windage + left.windage).abs() < 1e-6);
    assert!(right.windage_correction > 0.0);
}

#[test]
fn headwind_costs_more_drop_than_tailwind() {
    let mut head = scenario();
    head.shot.wind = Wind {
        speed: 5.0,
        from_degrees: 0.0,
    };
    let mut tail = head.clone();
    tail.shot.wind.from_degrees = 180.0;
    assert!(row(&head, 400.0).drop < row(&tail, 400.0).drop);
    assert!(row(&head, 400.0).windage.abs() < 0.05);
}

#[test]
fn shooting_uphill_or_downhill_needs_less_hold_than_level() {
    let level = row(&scenario(), 300.0).elevation_correction;
    for angle in [-30.0, 30.0] {
        let mut sloped = scenario();
        sloped.shot.inclination_degrees = angle;
        let hold = row(&sloped, 300.0).elevation_correction;
        assert!(
            hold > 0.0 && hold < level,
            "{angle}°: {hold} vs level {level}"
        );
    }
}

#[test]
fn cold_powder_and_cold_air_drop_more_than_the_zero_day() {
    let mut cold = scenario();
    cold.load.temperature_sensitivity = 0.6;
    cold.shot.atmosphere.temperature_c = -20.0;
    cold.shot.powder_temperature_c = -20.0;
    let cold_solution = solver::solve(&cold, 310.0).unwrap();
    assert!((cold_solution.muzzle_velocity - (790.0 - 0.6 * 35.0)).abs() < 1e-9);
    assert!(cold_solution.at(300.0).unwrap().drop < row(&scenario(), 300.0).drop);
}

#[test]
fn coriolis_deflects_right_in_the_north_and_lifts_eastward_shots() {
    let with_coriolis = |azimuth_degrees| {
        let mut shot = scenario();
        shot.spin_drift = false;
        shot.shot.coriolis = Some(Coriolis {
            latitude_degrees: 65.6,
            azimuth_degrees,
        });
        row(&shot, 500.0)
    };
    let north = with_coriolis(0.0);
    assert!(north.windage > 0.0 && north.windage < 0.05);
    assert!(with_coriolis(90.0).drop > with_coriolis(270.0).drop);
}

#[test]
fn spin_drift_goes_with_the_twist() {
    let left_twist = {
        let mut rifle = scenario();
        rifle.rifle.right_hand_twist = false;
        row(&rifle, 400.0)
    };
    let right_twist = row(&scenario(), 400.0);
    assert!(right_twist.windage > 0.0);
    assert!((right_twist.windage + left_twist.windage).abs() < 1e-6);
}

#[test]
fn optimal_point_blank_zero_peaks_at_the_top_of_the_vital_zone() {
    let vital = 0.20;
    let optimal = analysis::optimal_point_blank(&scenario(), vital).unwrap();
    assert!(optimal.zero_range > 150.0 && optimal.zero_range < optimal.point_blank.far_limit);
    assert!(optimal.peak_range < optimal.zero_range);
    assert_eq!(optimal.point_blank.near_limit, 0.0);

    let mut rezeroed = scenario();
    rezeroed.rifle.zero_range_m = optimal.zero_range;
    let solution = solver::solve(&rezeroed, 600.0).unwrap();
    let top = solution
        .samples
        .iter()
        .map(|s| s.drop)
        .fold(f64::MIN, f64::max);
    assert!((top - vital / 2.0).abs() < 1e-3, "peak {top}");
    let far = solution.at(optimal.point_blank.far_limit).unwrap();
    assert!((far.drop + vital / 2.0).abs() < 1e-3);

    let with_100m_zero = solver::solve(&scenario(), 600.0).unwrap();
    let limit = analysis::point_blank_with_zero(&with_100m_zero, vital).unwrap();
    assert!(limit.far_limit < optimal.point_blank.far_limit);
}

#[test]
fn ethical_limit_is_where_energy_runs_out() {
    let solution = solver::solve(&scenario(), 1000.0).unwrap();
    let limit = analysis::ethical_limit(&solution, 2000.0, 0.0).unwrap();
    let energy = solution.at(limit).unwrap().energy_j;
    assert!((energy - 2000.0).abs() < 1.0, "{energy} J at {limit} m");
    assert_eq!(analysis::ethical_limit(&solution, 1e6, 0.0), Some(0.0));
    assert_eq!(analysis::ethical_limit(&solution, 0.0, 100.0), None);
}

#[test]
fn moving_target_lead_is_speed_times_flight_time() {
    let at_100 = row(&scenario(), 100.0);
    let lead = analysis::moving_target_lead(&at_100, 5.0);
    assert!((lead.distance - 5.0 * at_100.time).abs() < 1e-12);
    assert!(lead.distance > 0.6 && lead.distance < 0.7);
}

#[test]
fn unknown_bullet_length_skips_stability_and_spin_drift() {
    let mut unknown = scenario();
    unknown.bullet.length_m = None;
    let solution = solver::solve(&unknown, 310.0).unwrap();
    assert_eq!(solution.stability, None);
    assert!(solution.at(300.0).unwrap().windage.abs() < 1e-9);
    assert!(solver::solve(&scenario(), 10.0).unwrap().stability.unwrap() > 1.0);
}

#[test]
fn unreachable_zero_is_an_error() {
    let mut weak = scenario();
    weak.load.muzzle_velocity = 60.0;
    weak.rifle.zero_range_m = 2000.0;
    assert_eq!(solver::solve(&weak, 100.0), Err(SolveError::ZeroOutOfReach));
}

/// Values from py-ballisticcalc 3.1.1 (an independent RK4 solver using the
/// same drag tables) for the same inputs: range, drop cm, velocity, time.
#[test]
fn matches_an_independent_reference_solver() {
    let mut long_range = scenario();
    long_range.bullet = Bullet {
        mass_kg: 0.0093,
        diameter_m: 0.00671,
        length_m: Some(0.036),
        ballistic_coefficient: 0.315,
        drag_model: DragModel::G7,
        bc_bands: BcBands::NONE,
        drag_curve: None,
    };
    long_range.rifle.twist_m = 8.0 * 0.0254;
    long_range.load.muzzle_velocity = 820.0;
    long_range.shot.wind = Wind {
        speed: 4.0,
        from_degrees: 90.0,
    };
    // The reference is a plain point-mass solver: no aerodynamic jump.
    long_range.aerodynamic_jump = false;
    let cases = [
        (
            scenario(),
            [
                (300.0, -50.84, 603.87, 0.4345),
                (600.0, -347.13, 448.82, 1.0121),
            ],
        ),
        (
            long_range,
            [
                (500.0, -170.60, 595.58, 0.7159),
                (1000.0, -1063.71, 407.14, 1.7308),
            ],
        ),
    ];
    for (scenario, expected) in cases {
        for (range, drop_cm, velocity, time) in expected {
            let row = row(&scenario, range);
            assert!(
                (row.drop * 100.0 - drop_cm).abs() < 0.5,
                "{range} m: {} cm",
                row.drop * 100.0
            );
            assert!(
                (row.velocity - velocity).abs() < 0.5,
                "{range} m: {} m/s",
                row.velocity
            );
            assert!((row.time - time).abs() < 0.001, "{range} m: {} s", row.time);
        }
    }
}

#[test]
fn bc_bands_interpolate_and_match_a_constant_bc_when_flat() {
    let bands = BcBands::new(&[(800.0, 0.40), (400.0, 0.30)]).unwrap();
    assert_eq!(bands.at(900.0), Some(0.40));
    assert_eq!(bands.at(300.0), Some(0.30));
    assert!((bands.at(600.0).unwrap() - 0.35).abs() < 1e-12);
    assert_eq!(BcBands::NONE.at(600.0), None);
    assert!(BcBands::new(&[(800.0, 0.4), (800.0, 0.3)]).is_err());
    assert!(BcBands::new(&[(800.0, -0.4)]).is_err());

    let mut flat = scenario();
    flat.bullet.bc_bands = BcBands::new(&[(300.0, 0.45), (900.0, 0.45)]).unwrap();
    assert!((row(&flat, 500.0).drop - row(&scenario(), 500.0).drop).abs() < 1e-9);

    // A BC that falls as the bullet slows costs drop far out.
    let mut falling = scenario();
    falling.bullet.bc_bands = BcBands::new(&[(500.0, 0.38), (790.0, 0.45)]).unwrap();
    assert!(row(&falling, 500.0).drop < row(&scenario(), 500.0).drop - 0.02);
}

#[test]
fn impact_offset_moves_the_hit_by_the_same_angle_at_every_range() {
    let mut shifted = scenario();
    shifted.impact_offset = [0.0005, 0.001];
    for range in [100.0, 300.0] {
        let (base, moved) = (row(&scenario(), range), row(&shifted, range));
        let up = (moved.drop - base.drop) / range;
        let right = (moved.windage - base.windage) / range;
        assert!((up - 0.001).abs() < 0.000_05, "{range} m: {up}");
        assert!((right - 0.0005).abs() < 0.000_05, "{range} m: {right}");
    }
}

#[test]
fn a_zero_made_with_cold_powder_shoots_high_when_the_powder_is_warm() {
    let mut cold_zero = scenario();
    cold_zero.load.temperature_sensitivity = 0.6;
    cold_zero.zero_powder_temperature_c = -15.0;
    assert!(
        row(&cold_zero, 100.0).drop > 0.002,
        "faster than at the zero, so high at the zero range"
    );
    assert!(row(&cold_zero, 300.0).drop > row(&scenario(), 300.0).drop);
}

#[test]
fn a_drag_curve_equal_to_the_g7_model_gives_the_same_trajectory() {
    // The bullet's own Cd equals the G7 Cd scaled by (m/d²)/BC, so both
    // descriptions are the same drag.
    let mut g7 = scenario();
    g7.bullet.drag_model = DragModel::G7;
    g7.bullet.ballistic_coefficient = 0.25;
    let bullet = &g7.bullet;
    let scale = (bullet.mass_kg / bullet.diameter_m.powi(2)) / (0.25 * 703.069_58);
    let points = crate::drag::g7_points()
        .iter()
        .map(|&(mach, cd)| (mach, cd * scale))
        .collect();
    let mut curve = g7.clone();
    curve.bullet.drag_curve = Some(std::sync::Arc::new(DragCurve::new(points).unwrap()));
    curve.bullet.ballistic_coefficient = 0.0;
    for range in [200.0, 500.0] {
        let (expected, actual) = (row(&g7, range), row(&curve, range));
        assert!((expected.drop - actual.drop).abs() < 1e-6, "{range} m");
        assert!(
            (expected.velocity - actual.velocity).abs() < 1e-6,
            "{range} m"
        );
    }
}

#[test]
fn a_crosswind_from_the_right_throws_a_right_hand_twist_bullet_up() {
    let windy = |from_degrees: f64, jump: bool, right_hand: bool| {
        let mut s = scenario();
        s.shot.wind = Wind {
            speed: 4.0,
            from_degrees,
        };
        s.aerodynamic_jump = jump;
        s.rifle.right_hand_twist = right_hand;
        row(&s, 300.0).drop
    };
    let from_right = windy(90.0, true, true) - windy(90.0, false, true);
    let from_left = windy(270.0, true, true) - windy(270.0, false, true);
    assert!(
        from_right > 0.0 && from_left < 0.0,
        "{from_right} {from_left}"
    );
    assert!((from_right + from_left).abs() < 1e-3);
    // A left-hand twist turns it round.
    assert!(windy(90.0, true, false) - windy(90.0, false, false) < 0.0);
    // Litz: (0.01·Sg − 0.0024·L + 0.032) MOA per mph, here about 0.3 MOA
    // for 4 m/s, a few centimetres at 300 m.
    let s = scenario();
    let sg = solver::solve(&s, 100.0).unwrap().stability.unwrap();
    let moa = (0.01 * sg - 0.0024 * (0.031 / 0.00782) + 0.032) * 4.0 * 2.236_936;
    let expected = moa * std::f64::consts::PI / (180.0 * 60.0) * 300.0;
    assert!(
        (from_right.abs() - expected).abs() < 1e-3,
        "{from_right} {expected}"
    );
    // Head and tail winds give no jump.
    assert!((windy(0.0, true, true) - windy(0.0, false, true)).abs() < 1e-9);
}

#[test]
fn a_cant_to_the_right_lands_low_and_right() {
    let level = solver::solve(&scenario(), 410.0).unwrap();
    let mut canted_scenario = scenario();
    canted_scenario.shot.cant_degrees = 5.0;
    let canted = solver::solve(&canted_scenario, 410.0).unwrap();
    let range = 400.0;
    let (flat, tilted) = (level.at(range).unwrap(), canted.at(range).unwrap());
    // No cant, no change.
    assert_eq!(level.cant_shift(range), Some((0.0, 0.0)));
    let (right, up) = canted.cant_shift(range).unwrap();
    assert!(right > 0.0 && up < 0.0, "{right} {up}");
    // A dead-on hold: the bore's line (r·θ − sight height over the line of
    // sight) turns with the rifle, gravity doesn't. Sideways that is
    // (r·θ − h)·sin(cant).
    let bore_line = range * level.bore[1] - level.sight_height;
    assert!(
        (right - bore_line * 5f64.to_radians().sin()).abs() < 1e-3,
        "{right}"
    );
    assert!((tilted.windage - flat.windage - right).abs() < 1e-9);
    // The correction, held in the tilted reticle, brings the bullet back
    // onto the line of sight.
    let (sin, cos) = 5f64.to_radians().sin_cos();
    let [bore_right, bore_up] = canted.bore;
    let aim = (
        range * (bore_right + tilted.windage_correction),
        range * (bore_up + tilted.elevation_correction) - canted.sight_height,
    );
    let pushed = (
        flat.windage - range * bore_right,
        flat.drop - (range * bore_up - canted.sight_height),
    );
    let lands = (
        aim.0 * cos + aim.1 * sin + pushed.0,
        -aim.0 * sin + aim.1 * cos + pushed.1,
    );
    assert!(lands.0.abs() < 1e-9 && lands.1.abs() < 1e-9, "{lands:?}");
    // In the tilted reticle "up" leans right: the hold goes left by the
    // fall × sin(cant), and up by only the fall × cos(cant).
    assert!(tilted.windage_correction < flat.windage_correction);
    assert!(tilted.elevation_correction < flat.elevation_correction);
}

#[test]
fn a_zero_high_at_100_m_is_told_both_ways() {
    // Zeroed 4 cm high at 100 m.
    let mut high = scenario();
    high.rifle.zero_height_m = 0.04;
    let at_100 = row(&high, 100.0);
    assert!((at_100.drop - 0.04).abs() < 1e-4, "{}", at_100.drop);
    let figure = analysis::zero_figure(&high, 100.0, 600.0).unwrap();
    assert!((figure.height - 0.04).abs() < 1e-4);
    // It comes back down through the line of sight further out, where a
    // rifle zeroed there plainly hits the same.
    let far = figure.far_zero.unwrap();
    assert!(far > 150.0 && far < 300.0, "{far}");
    let mut plain = scenario();
    plain.rifle.zero_range_m = far;
    assert!((row(&plain, 100.0).drop - 0.04).abs() < 2e-3);
    // A plain zero is level with the line of sight at its range.
    let figure = analysis::zero_figure(&scenario(), 100.0, 600.0).unwrap();
    assert!(figure.height.abs() < 1e-4);
}

#[test]
fn the_best_point_blank_zero_is_told_at_100_m() {
    let s = scenario();
    let best = analysis::optimal_point_blank(&s, 0.15).unwrap();
    let height = analysis::height_at(&s, best.elevation, 100.0).unwrap();
    // High at 100 m, but below the zone's top (7.5 cm) which it reaches at
    // its peak further out.
    assert!(height > 0.03 && height < 0.075, "{height}");
    let at_zero = analysis::height_at(&s, best.elevation, best.zero_range).unwrap();
    assert!(at_zero.abs() < 1e-3);
}

#[test]
fn solvers_reject_non_finite_environmental_inputs() {
    for field in [
        "zero temperature",
        "zero pressure",
        "zero humidity",
        "shot temperature",
        "shot pressure",
        "shot humidity",
        "latitude",
        "azimuth",
    ] {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut invalid = scenario();
            invalid.shot.coriolis = Some(Coriolis {
                latitude_degrees: 0.0,
                azimuth_degrees: 0.0,
            });
            let input = match field {
                "zero temperature" => &mut invalid.zero_atmosphere.temperature_c,
                "zero pressure" => &mut invalid.zero_atmosphere.pressure_hpa,
                "zero humidity" => &mut invalid.zero_atmosphere.humidity,
                "shot temperature" => &mut invalid.shot.atmosphere.temperature_c,
                "shot pressure" => &mut invalid.shot.atmosphere.pressure_hpa,
                "shot humidity" => &mut invalid.shot.atmosphere.humidity,
                "latitude" => &mut invalid.shot.coriolis.as_mut().unwrap().latitude_degrees,
                "azimuth" => &mut invalid.shot.coriolis.as_mut().unwrap().azimuth_degrees,
                _ => unreachable!(),
            };
            *input = value;
            for result in [
                solver::solve(&invalid, 1.0),
                solver::solve_with_zero(&invalid, 1.0, (0.0, 0.0)),
            ] {
                assert!(
                    matches!(result, Err(SolveError::InvalidInput(_))),
                    "{field} = {value} must be rejected"
                );
            }
        }
    }
}

#[test]
fn bc_bands_reject_non_finite_points() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(BcBands::new(&[(value, 0.4)]).is_err());
        assert!(BcBands::new(&[(1.0, value)]).is_err());
    }
}

#[test]
fn bc_band_lookup_rejects_non_finite_speeds() {
    for bands in [
        BcBands::NONE,
        BcBands::new(&[(1.0, 0.4)]).unwrap(),
        BcBands::new(&[(1.0, 0.4), (2.0, 0.5)]).unwrap(),
    ] {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(bands.at(value), None);
        }
    }
}

#[test]
fn solve_with_zero_rejects_non_finite_angles() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for zero in [(value, 0.0), (0.0, value)] {
            assert!(matches!(
                solver::solve_with_zero(&scenario(), 1.0, zero),
                Err(SolveError::InvalidInput(_))
            ));
        }
    }
}

#[test]
fn solvers_reject_invalid_maximum_ranges() {
    let s = scenario();
    for range in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        for result in [
            solver::solve(&s, range),
            solver::solve_with_zero(&s, range, (0.0, 0.0)),
        ] {
            assert!(
                matches!(result, Err(SolveError::InvalidInput(_))),
                "maximum range {range} must be rejected"
            );
        }
    }
    // A request at the origin is valid, including the signed-zero boundary.
    for range in [0.0, -0.0] {
        assert!(solver::solve(&s, range).unwrap().at(0.0).is_some());
        assert!(
            solver::solve_with_zero(&s, range, (0.0, 0.0))
                .unwrap()
                .at(0.0)
                .is_some()
        );
    }
}

#[test]
fn absurd_bullets_are_refused_or_fly_short_without_a_panic() {
    // Found by stress testing: these panicked in the drag table lookup.
    let mut feather = scenario();
    feather.bullet.mass_kg = 0.000_02;
    feather.bullet.diameter_m = 0.015;
    assert!(solver::solve(&feather, 300.0).is_err());

    // Light and draggy, but allowed: the path may end early, never panic.
    let mut draggy = scenario();
    draggy.bullet.mass_kg = 0.000_15;
    draggy.bullet.diameter_m = 0.015;
    draggy.bullet.drag_curve = Some(std::sync::Arc::new(
        DragCurve::new(vec![(0.0, 2.9), (5.0, 2.9)]).unwrap(),
    ));
    draggy.load.muzzle_velocity = 1300.0;
    let _ = solver::solve(&draggy, 300.0);

    for velocity in [f64::INFINITY, f64::NAN, 1e300] {
        let mut fast = scenario();
        fast.load.muzzle_velocity = velocity;
        assert!(solver::solve(&fast, 300.0).is_err(), "{velocity}");
    }
    assert_eq!(
        DragModel::G7.drag_coefficient(f64::NAN),
        DragModel::G7.drag_coefficient(0.0)
    );
}

#[test]
fn the_best_zero_saved_to_the_millimetre_keeps_its_range() {
    // Found by stress testing: saving the best point-blank zero as a height
    // at 100 m, rounded to a millimetre, could halve the dead-on range.
    for (velocity, bc, sight) in [
        (600.0, 0.25, 0.03),
        (790.0, 0.45, 0.045),
        (900.0, 0.3, 0.06),
    ] {
        let mut s = scenario();
        s.load.muzzle_velocity = velocity;
        s.bullet.ballistic_coefficient = bc;
        s.bullet.drag_model = DragModel::G7;
        s.rifle.sight_height_m = sight;
        let best = analysis::optimal_point_blank(&s, 0.3).unwrap();
        let height = analysis::height_at(&s, best.elevation, 100.0).unwrap();
        let mut zeroed = s.clone();
        zeroed.rifle.zero_range_m = 100.0;
        zeroed.rifle.zero_height_m = (height * 1000.0).floor() / 1000.0;
        let solution = solver::solve(&zeroed, 600.0).unwrap();
        let kept = analysis::point_blank_with_zero(&solution, 0.3).unwrap();
        assert!(
            (kept.far_limit - best.point_blank.far_limit).abs() < 5.0,
            "{velocity} m/s: {} vs {}",
            kept.far_limit,
            best.point_blank.far_limit
        );
    }
}

mod dispersion {
    use super::scenario;
    use crate::dispersion::{Sensitivities, Spread, Uncertainty, hit_chance};
    use crate::solver::{self, Wind};

    fn exact() -> Uncertainty {
        Uncertainty {
            aim: 0.0,
            muzzle_velocity: 0.0,
            range: 0.0,
            wind_speed: 0.0,
            wind_direction: 0.0,
        }
    }

    #[test]
    fn the_aim_alone_spreads_in_proportion_to_range() {
        let s = scenario();
        let base = solver::solve(&s, 400.0).unwrap();
        let sens = Sensitivities::new(&s, base, 400.0).unwrap();
        let u = Uncertainty {
            aim: Uncertainty::aim_from_group(0.0307, 100.0),
            ..exact()
        };
        let at_100 = sens.spread_at(100.0, &u).unwrap();
        let at_300 = sens.spread_at(300.0, &u).unwrap();
        // A 3.07 cm 5-shot group is 1 cm one sigma at 100 m.
        assert!((at_100.across - 0.01).abs() < 1e-6, "{at_100:?}");
        assert!((at_300.up - 0.03).abs() < 1e-6, "{at_300:?}");
    }

    #[test]
    fn velocity_spread_drops_the_shot_more_far_out_and_not_at_the_zero() {
        let s = scenario();
        let base = solver::solve(&s, 400.0).unwrap();
        let sens = Sensitivities::new(&s, base, 400.0).unwrap();
        let u = Uncertainty {
            muzzle_velocity: 10.0,
            ..exact()
        };
        let near = sens.spread_at(50.0, &u).unwrap().up;
        let far = sens.spread_at(300.0, &u).unwrap().up;
        assert!(far > 5.0 * near, "near {near}, far {far}");
        // 10 m/s at 300 m: a few centimetres.
        assert!((0.01..0.06).contains(&far), "{far}");
    }

    #[test]
    fn wind_uncertainty_spreads_across_and_a_range_error_up() {
        let mut s = scenario();
        s.shot.wind = Wind {
            speed: 4.0,
            from_degrees: 90.0,
        };
        let base = solver::solve(&s, 400.0).unwrap();
        let sens = Sensitivities::new(&s, base, 400.0).unwrap();
        let wind = sens
            .spread_at(
                300.0,
                &Uncertainty {
                    wind_speed: 1.0,
                    ..exact()
                },
            )
            .unwrap();
        assert!(wind.across > 5.0 * wind.up, "{wind:?}");
        let range = sens
            .spread_at(
                300.0,
                &Uncertainty {
                    range: 10.0,
                    ..exact()
                },
            )
            .unwrap();
        // The drift goes on growing past the target too, a little.
        assert!(range.up > 3.0 * range.across, "{range:?}");
        // The drift grows with the wind it's asked for.
        let calm = sens.drift_at(300.0, 0.0).unwrap();
        let strong = sens.drift_at(300.0, 8.0).unwrap();
        let now = sens.drift_at(300.0, 4.0).unwrap();
        assert!((now - sens.base().at(300.0).unwrap().windage).abs() < 1e-9);
        assert!(calm.abs() < now.abs() && now.abs() < strong.abs());
    }

    #[test]
    fn hit_chance_matches_the_round_normal_formula() {
        let spread = Spread {
            across: 0.1,
            up: 0.1,
        };
        // Within one sigma radius: 1 − e^(−1/2) ≈ 0.3935.
        let p = hit_chance(spread, [0.0, 0.0], |[x, y]| x * x + y * y <= 0.01);
        assert!((p - 0.3935).abs() < 0.01, "{p}");
        // Far bigger than the spread: nearly certain; off to one side: half.
        assert!(hit_chance(spread, [0.0, 0.0], |[x, y]| x * x + y * y <= 1.0) > 0.999);
        let half = hit_chance(spread, [0.0, 0.0], |[x, _]| x > 0.0);
        assert!((half - 0.5).abs() < 0.01, "{half}");
    }
}

#[test]
fn gravity_follows_the_latitude() {
    assert!((solver::normal_gravity(0.0) - 9.780_33).abs() < 1e-4);
    assert!((solver::normal_gravity(45.0) - 9.806_20).abs() < 1e-4);
    assert!((solver::normal_gravity(65.0) - 9.822_9).abs() < 1e-3);
    assert!((solver::normal_gravity(-65.0) - solver::normal_gravity(65.0)).abs() < 1e-12);
    // Shooting north-south, Coriolis only moves the bullet sideways, so the
    // drop changes by the stronger gravity up north: a few millimetres at
    // 600 m.
    let mut north = scenario();
    let level = row(&north, 600.0).drop;
    north.shot.coriolis = Some(Coriolis {
        latitude_degrees: 65.0,
        azimuth_degrees: 0.0,
    });
    let change = row(&north, 600.0).drop - level;
    assert!(change < -0.003 && change > -0.01, "{change}");
}

/// No. 7.5 (2.4 mm) lead, about 0.08 g: lighter than any rifle bullet.
const SEVEN_AND_A_HALF: pellets::Pellet = pellets::Pellet {
    diameter_m: 0.0024,
    density_kg_m3: pellets::LEAD_DENSITY_KG_M3,
};

fn pellet_at(samples: &[solver::Sample], range: f64) -> solver::Sample {
    solver::interpolate(samples, range).unwrap()
}

#[test]
fn small_shot_slows_down_like_shot_does() {
    assert!(SEVEN_AND_A_HALF.mass_kg() < 0.000_1);
    let path = pellets::fly(&SEVEN_AND_A_HALF, 400.0, &scenario().shot, 50.0).unwrap();
    let at_40_yards = pellet_at(&path, pellets::PATTERN_DISTANCE_M);
    // About 184 m/s, 0.145 s and 8 cm low; ±10 %.
    let (velocity, time, drop) = (at_40_yards.velocity, at_40_yards.time, at_40_yards.drop);
    assert!((166.0..203.0).contains(&velocity), "velocity {velocity}");
    assert!((0.130..0.159).contains(&time), "time {time}");
    assert!((-0.090..-0.074).contains(&drop), "drop {drop}");
}

#[test]
fn a_pellet_flies_like_a_bullet_with_the_same_drag() {
    // Buckshot is heavy enough for the rifle solver's checks.
    let buckshot = pellets::Pellet {
        diameter_m: 0.0084,
        density_kg_m3: pellets::LEAD_DENSITY_KG_M3,
    };
    let mut scenario = scenario();
    scenario.shot.wind = Wind {
        speed: 5.0,
        from_degrees: 60.0,
    };
    scenario.shot.inclination_degrees = 8.0;
    scenario.shot.coriolis = Some(Coriolis {
        latitude_degrees: 60.0,
        azimuth_degrees: 90.0,
    });
    scenario.bullet = Bullet {
        mass_kg: buckshot.mass_kg(),
        diameter_m: buckshot.diameter_m,
        length_m: None,
        ballistic_coefficient: 0.0,
        drag_model: DragModel::G1,
        bc_bands: BcBands::NONE,
        drag_curve: Some(pellets::sphere_drag()),
    };
    scenario.rifle.sight_height_m = 0.0;
    scenario.load.muzzle_velocity = 400.0;
    scenario.spin_drift = false;
    scenario.aerodynamic_jump = false;
    let as_bullet = solver::solve_with_zero(&scenario, 60.0, (0.0, 0.0)).unwrap();
    let as_pellet = pellets::fly(&buckshot, 400.0, &scenario.shot, 60.0).unwrap();
    assert_eq!(as_pellet, as_bullet.samples);
}

#[test]
fn pellets_refuse_what_cannot_be_shot() {
    let calm = scenario().shot;
    let fly = |pellet: &pellets::Pellet, velocity: f64, shot: Conditions, range: f64| {
        pellets::fly(pellet, velocity, &shot, range)
    };
    let no_size = pellets::Pellet {
        diameter_m: 0.0,
        ..SEVEN_AND_A_HALF
    };
    assert!(fly(&no_size, 400.0, calm, 40.0).is_err());
    assert!(fly(&SEVEN_AND_A_HALF, f64::NAN, calm, 40.0).is_err());
    assert!(fly(&SEVEN_AND_A_HALF, 0.0, calm, 40.0).is_err());
    assert!(fly(&SEVEN_AND_A_HALF, 400.0, calm, -1.0).is_err());
    assert!(fly(&SEVEN_AND_A_HALF, 400.0, calm, f64::NAN).is_err());
    let unshootable = [
        Conditions {
            wind: Wind {
                speed: f64::INFINITY,
                from_degrees: 90.0,
            },
            ..calm
        },
        Conditions {
            wind: Wind {
                speed: -10.0,
                from_degrees: 90.0,
            },
            ..calm
        },
        Conditions {
            inclination_degrees: 180.0,
            ..calm
        },
        Conditions {
            atmosphere: Atmosphere {
                pressure_hpa: 0.0,
                ..calm.atmosphere
            },
            ..calm
        },
        Conditions {
            atmosphere: Atmosphere {
                pressure_hpa: -500.0,
                ..calm.atmosphere
            },
            ..calm
        },
        Conditions {
            atmosphere: Atmosphere {
                temperature_c: -300.0,
                ..calm.atmosphere
            },
            ..calm
        },
    ];
    for shot in unshootable {
        assert!(
            fly(&SEVEN_AND_A_HALF, 400.0, shot, 40.0).is_err(),
            "{shot:?}"
        );
    }
}

#[test]
fn a_pellet_that_falls_short_ends_its_path_where_it_falls_steeply() {
    let calm = scenario().shot;
    let reached = pellets::fly(&SEVEN_AND_A_HALF, 400.0, &calm, 40.0).unwrap();
    assert!(reached.last().unwrap().range >= 40.0);

    let path = pellets::fly(&SEVEN_AND_A_HALF, 400.0, &calm, 400.0).unwrap();
    let [.., before, last] = path.as_slice() else {
        panic!("a path of {} samples", path.len());
    };
    assert!(last.range < 400.0, "range {}", last.range);
    assert!(last.time < 10.0, "time {}", last.time);
    let along = (last.range - before.range) / (last.time - before.time);
    assert!(along < 0.5 * last.velocity);
}

#[test]
fn a_24_g_load_of_seven_and_a_half_holds_about_300_pellets() {
    let count = SEVEN_AND_A_HALF.count_in(0.024).unwrap();
    assert!((280..310).contains(&count), "{count}");
}

#[test]
fn a_load_of_exactly_n_pellets_counts_n() {
    for diameter_mm in [1.5, 2.0, 2.4, 3.0, 8.4] {
        let pellet = pellets::Pellet {
            diameter_m: diameter_mm / 1000.0,
            ..SEVEN_AND_A_HALF
        };
        for n in 0..2000 {
            assert_eq!(
                pellet.count_in(n as f64 * pellet.mass_kg()),
                Some(n),
                "{diameter_mm} mm, {n}"
            );
        }
    }
}

#[test]
fn counting_refuses_what_is_not_a_load() {
    let no_size = pellets::Pellet {
        diameter_m: 0.0,
        ..SEVEN_AND_A_HALF
    };
    assert_eq!(no_size.count_in(0.024), None);
    assert_eq!(SEVEN_AND_A_HALF.count_in(f64::INFINITY), None);
    assert_eq!(SEVEN_AND_A_HALF.count_in(-0.024), None);
    assert_eq!(SEVEN_AND_A_HALF.count_in(10.0), None);
}

#[test]
fn a_pattern_puts_the_choke_percentage_in_the_circle() {
    let circle = (pellets::PATTERN_RADIUS_M / pellets::PATTERN_DISTANCE_M).atan();
    for percent in [40.0, 55.0, 70.0] {
        let pattern = pellets::pattern(percent, 40_000, 7).unwrap();
        let inside = pattern
            .iter()
            .filter(|[right, up]| right.hypot(*up) <= circle)
            .count() as f64
            / pattern.len() as f64;
        assert!(
            (inside * 100.0 - percent).abs() < 1.0,
            "{percent}%: {inside}"
        );
    }
}

#[test]
fn the_same_seed_gives_the_same_pattern() {
    let pattern = pellets::pattern(65.0, 300, 42).unwrap();
    assert_eq!(pellets::pattern(65.0, 300, 42).unwrap(), pattern);
    assert_ne!(pellets::pattern(65.0, 300, 43).unwrap(), pattern);
    assert!(pattern.iter().flatten().all(|angle| angle.is_finite()));
}

#[test]
fn a_seed_gives_the_same_angles_in_every_version() {
    // Replays depend on these: change the generator only with a new seed scheme.
    let expected = [
        [-3.6582030065896573e-3, 6.591468039852384e-4],
        [-4.522913533066308e-3, 3.998892213865889e-3],
        [-6.778752100548741e-4, -4.538758996630664e-3],
    ];
    let pattern = pellets::pattern(70.0, 3, 2026).unwrap();
    for (got, want) in pattern.iter().flatten().zip(expected.iter().flatten()) {
        assert!((got - want).abs() < 1e-12, "{got} vs {want}");
    }
}

#[test]
fn patterns_refuse_impossible_chokes_and_counts() {
    for percent in [0.0, 100.0, -5.0, f64::NAN] {
        assert!(pellets::pattern_sigma(percent).is_err(), "{percent}");
    }
    let tiny = pellets::pattern_sigma(1e-17).unwrap();
    assert!(tiny.is_finite() && tiny > 0.0);
    assert!(pellets::pattern(70.0, pellets::MAX_PELLETS + 1, 1).is_err());
    assert!(pellets::pattern(70.0, 0, 1).unwrap().is_empty());
}

/// The six borings of Gough Thomas's tables, by their 40-yard percentage.
const BORINGS: [f64; 6] = [40.0, 50.0, 55.0, 60.0, 65.0, 70.0];
const YARD: f64 = 0.9144;
const INCH: f64 = 0.0254;

#[test]
fn patterns_open_as_gough_thomas_tabled_them() {
    // Percent in the 30-inch circle at 30–60 yards, true cylinder to full
    // (Gough Thomas, via Spencer 2001, table 7).
    let table: [[f64; 7]; 6] = [
        [60.0, 49.0, 40.0, 33.0, 27.0, 22.0, 18.0],
        [72.0, 60.0, 50.0, 41.0, 33.0, 27.0, 22.0],
        [77.0, 65.0, 55.0, 46.0, 38.0, 30.0, 25.0],
        [83.0, 71.0, 60.0, 50.0, 41.0, 33.0, 27.0],
        [91.0, 77.0, 65.0, 55.0, 46.0, 37.0, 30.0],
        [100.0, 84.0, 70.0, 59.0, 49.0, 40.0, 32.0],
    ];
    for (boring, row) in BORINGS.iter().zip(table) {
        for (yards, tabled) in [30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0].iter().zip(row) {
            let share = pellets::share_inside(*boring, pellets::PATTERN_RADIUS_M, yards * YARD)
                .unwrap()
                * 100.0;
            // The table rounds a full pattern up to 100.
            let ok = if tabled == 100.0 {
                share >= 92.0
            } else {
                (share - tabled).abs() <= 3.0
            };
            assert!(ok, "{boring}% at {yards} yd: {share:.1} vs {tabled}");
        }
    }
}

#[test]
fn pattern_spread_matches_the_spread_table() {
    // Spread in inches at 20–40 yards (Gough Thomas, via Spencer 2001,
    // table 11), which is about the circle holding 85 % of the pellets.
    let table: [[f64; 5]; 6] = [
        [32.0, 38.0, 44.0, 51.0, 58.0],
        [26.0, 32.0, 38.0, 44.0, 51.0],
        [23.0, 29.0, 35.0, 41.0, 48.0],
        [21.0, 26.0, 32.0, 38.0, 45.0],
        [18.0, 23.0, 29.0, 35.0, 42.0],
        [16.0, 21.0, 27.0, 33.0, 40.0],
    ];
    for (boring, row) in BORINGS.iter().zip(table) {
        for (yards, tabled) in [20.0, 25.0, 30.0, 35.0, 40.0].iter().zip(row) {
            let spread = pellets::spread_diameter(*boring, 0.85, yards * YARD).unwrap() / INCH;
            assert!(
                (spread - tabled).abs() <= 3.5,
                "{boring}% at {yards} yd: {spread:.1} in vs {tabled}"
            );
        }
    }
}

#[test]
fn a_pattern_is_densest_in_the_middle_as_measured() {
    // Of the pellets in the 30-inch circle at 40 yards, the share inside
    // the 20-inch one: true cylinder 46–51 %, improved cylinder 48–54 %,
    // half 52–57 %, full 57–62 % (Gough Thomas, via Spencer 2001, table 9).
    for (boring, low, high) in [
        (40.0, 46.0, 51.0),
        (50.0, 48.0, 54.0),
        (60.0, 52.0, 57.0),
        (70.0, 57.0, 62.0),
    ] {
        let at = |radius: f64| {
            pellets::share_inside(boring, radius, pellets::PATTERN_DISTANCE_M).unwrap()
        };
        let share = at(10.0 * INCH) / at(15.0 * INCH) * 100.0;
        assert!(
            (low - 1.0..=high + 1.0).contains(&share),
            "{boring}%: {share:.1}"
        );
    }
}

#[test]
fn every_choke_opens_with_range_and_tighter_chokes_stay_tighter() {
    let mut last_by_range = vec![0.0; 50];
    for choke in pellets::Choke::ALL {
        let percent = choke.nominal_percent();
        let mut last = f64::INFINITY;
        for (i, range) in (1..=50).map(|i| i as f64 * 3.0).enumerate() {
            let share = pellets::share_inside(percent, pellets::PATTERN_RADIUS_M, range).unwrap();
            assert!(share <= last, "{choke:?} at {range} m");
            assert!(share >= last_by_range[i], "{choke:?} at {range} m");
            (last, last_by_range[i]) = (share, share);
        }
        let at_40_yards = pellets::share_inside(
            percent,
            pellets::PATTERN_RADIUS_M,
            pellets::PATTERN_DISTANCE_M,
        )
        .unwrap();
        assert!((at_40_yards * 100.0 - percent).abs() < 1e-9, "{choke:?}");
    }
}

#[test]
fn a_pattern_at_a_range_is_the_same_pellets_spread_to_its_size() {
    let angles = pellets::pattern(65.0, 300, 42).unwrap();
    let at_30 = pellets::pattern_at(65.0, 300, 42, 30.0).unwrap();
    let scale = pellets::sigma_at(65.0, 30.0).unwrap() / pellets::pattern_sigma(65.0).unwrap();
    for (angle, place) in angles.iter().zip(&at_30) {
        assert!((angle[0] * scale - place[0]).abs() < 1e-12);
        assert!((angle[1] * scale - place[1]).abs() < 1e-12);
    }
    let circle = 0.30;
    let inside = pellets::pattern_at(60.0, 40_000, 3, 25.0)
        .unwrap()
        .iter()
        .filter(|[right, up]| right.hypot(*up) <= circle)
        .count() as f64
        / 40_000.0;
    let expected = pellets::share_inside(60.0, circle, 25.0).unwrap();
    assert!((inside - expected).abs() < 0.01, "{inside} vs {expected}");
}

#[test]
fn choke_names_constrictions_steel_and_the_nordic_test_convert() {
    assert_eq!(pellets::percent_from_constriction(0.0).unwrap(), 40.0);
    assert_eq!(pellets::percent_from_constriction(0.000_5).unwrap(), 60.0);
    assert_eq!(pellets::percent_from_constriction(0.001_2).unwrap(), 70.0);
    let quarter = pellets::percent_from_constriction(0.000_315).unwrap();
    assert!((52.0..53.0).contains(&quarter), "{quarter}");
    assert!(pellets::percent_from_constriction(-0.000_1).is_err());
    assert!(pellets::percent_from_constriction(f64::NAN).is_err());

    assert_eq!(pellets::steel_percent(50.0).unwrap(), 60.0);
    assert_eq!(pellets::steel_percent(70.0).unwrap(), 72.0);
    assert_eq!(pellets::steel_percent(75.0).unwrap(), 75.0);
    assert!(pellets::Choke::Modified.suits_steel());
    assert!(!pellets::Choke::Full.suits_steel());

    // The 75 cm circle at 35 m holds 1.5–4 points more than the 30-inch one
    // at 40 yards, and the conversion goes back exactly.
    for p40 in BORINGS {
        let nordic = pellets::share_inside(
            p40,
            pellets::NORDIC_PATTERN_RADIUS_M,
            pellets::NORDIC_PATTERN_DISTANCE_M,
        )
        .unwrap()
            * 100.0;
        assert!((1.5..4.0).contains(&(nordic - p40)), "{p40}: {nordic:.1}");
        let back = pellets::percent_from_nordic(nordic).unwrap();
        assert!((back - p40).abs() < 1e-6, "{p40}: {back}");
    }
    assert!(pellets::percent_from_nordic(100.0).is_err());
    assert!(pellets::sigma_at(60.0, -1.0).is_err());
    assert!(pellets::sigma_at(60.0, 1000.0).is_err());
    assert!(pellets::spread_diameter(60.0, 1.0, 30.0).is_err());
    assert_eq!(pellets::share_inside(60.0, 0.1, 0.0).unwrap(), 1.0);
}

const FOOT: f64 = 0.3048;
const FPS: f64 = 0.3048;

/// Lowry's standard air: 59 °F, 29.54 inHg, 50 %.
fn lowry_air() -> Conditions {
    pellets::calm(Atmosphere {
        temperature_c: 15.0,
        pressure_hpa: 1000.33,
        humidity: 0.5,
    })
}

#[test]
fn pellets_slow_down_as_lowry_measured() {
    // Velocity (ft/s) at 0, 60, 90, 120 and 150 ft and time to 120 ft, from
    // the front pellets' true starting velocity (E. D. Lowry, Winchester,
    // American Rifleman 1989, table 3).
    let table: [(f64, f64, [f64; 5], f64); 8] = [
        (0.180, 7_860.0, [1376.0, 952.0, 823.0, 722.0, 639.0], 0.1262),
        (0.160, 7_860.0, [1391.0, 932.0, 797.0, 692.0, 607.0], 0.1291),
        (0.150, 7_860.0, [1399.0, 920.0, 782.0, 676.0, 590.0], 0.1308),
        (0.130, 7_860.0, [1415.0, 892.0, 749.0, 639.0, 552.0], 0.1350),
        (
            0.150,
            11_100.0,
            [1351.0, 968.0, 847.0, 749.0, 669.0],
            0.1241,
        ),
        (
            0.130,
            11_100.0,
            [1363.0, 944.0, 816.0, 715.0, 633.0],
            0.1273,
        ),
        (
            0.120,
            11_100.0,
            [1369.0, 930.0, 799.0, 696.0, 613.0],
            0.1293,
        ),
        (
            0.110,
            11_100.0,
            [1375.0, 914.0, 780.0, 675.0, 591.0],
            0.1316,
        ),
    ];
    for (inches, density, velocities, time_to_120) in table {
        let pellet = pellets::Pellet {
            diameter_m: inches * INCH,
            density_kg_m3: density,
        };
        let path = pellets::fly(&pellet, velocities[0] * FPS, &lowry_air(), 50.0).unwrap();
        for (feet, tabled, tolerance) in [
            (60.0, velocities[1], 0.05),
            (90.0, velocities[2], 0.05),
            (120.0, velocities[3], 0.05),
            (150.0, velocities[4], 0.06),
        ] {
            let velocity = pellet_at(&path, feet * FOOT).velocity / FPS;
            assert!(
                (velocity / tabled - 1.0).abs() <= tolerance,
                "{inches} in, {density}: {velocity:.0} vs {tabled} ft/s at {feet} ft"
            );
        }
        let time = pellet_at(&path, 120.0 * FOOT).time;
        assert!(
            (time / time_to_120 - 1.0).abs() <= 0.04,
            "{inches} in, {density}: {time:.4} vs {time_to_120} s"
        );
    }
}

#[test]
fn pellets_strike_as_eley_tabled() {
    // Striking velocity (m/s) at 30, 35 and 40 m of Eley's standard game
    // cartridges, observed velocity 325 m/s, about 402 m/s at the muzzle
    // (Eley Shooter's Diary 1992, via Spencer 2001, table 13). Nearer than
    // 35 m Eley's small shot is a few per cent faster than one muzzle
    // velocity for all sizes gives.
    for (size, tabled) in [
        ("BB", [254.0, 238.0, 226.0]),
        ("3", [236.0, 219.0, 203.0]),
        ("4", [231.0, 214.0, 197.0]),
        ("5", [224.0, 205.0, 188.0]),
        ("6", [219.0, 199.0, 180.0]),
        ("7", [212.0, 191.0, 172.0]),
    ] {
        let diameter = pellets::diameter_of(&pellets::ENGLISH_SIZES, size).unwrap();
        let pellet = pellets::Pellet::of(pellets::Material::Lead, diameter);
        let calm = pellets::calm(Atmosphere::standard());
        let path = pellets::fly(&pellet, 402.0, &calm, 45.0).unwrap();
        for ((range, tolerance), tabled) in [(30.0, 0.05), (35.0, 0.04), (40.0, 0.025)]
            .iter()
            .zip(tabled)
        {
            let velocity = pellet_at(&path, *range).velocity;
            assert!(
                (velocity / tabled - 1.0).abs() <= *tolerance,
                "English {size}: {velocity:.0} vs {tabled} m/s at {range} m"
            );
        }
    }
}

#[test]
fn pellet_energies_match_gough_thomas_from_the_observed_velocity() {
    // One pellet's energy (ft-lb) at 30, 35, 40 and 45 yards for an
    // observed velocity of 1070 ft/s over 20 yards (Gough Thomas, via
    // Spencer 2001, table 14).
    const FOOT_POUND: f64 = 1.355_818;
    for (size, tabled) in [
        ("3", [4.48, 3.92, 3.43, 2.99]),
        ("4", [3.54, 3.08, 2.66, 2.30]),
        ("5", [2.60, 2.23, 1.90, 1.61]),
        ("6", [2.03, 1.71, 1.44, 1.20]),
        ("7", [1.52, 1.27, 1.06, 0.86]),
    ] {
        let diameter = pellets::diameter_of(&pellets::ENGLISH_SIZES, size).unwrap();
        let pellet = pellets::Pellet::of(pellets::Material::Lead, diameter);
        let calm = pellets::calm(Atmosphere::standard());
        let muzzle = pellets::muzzle_velocity(
            &pellet,
            1070.0 * FPS,
            pellets::VelocityAt::AverageOver(20.0 * YARD),
            &calm,
        )
        .unwrap();
        // An observed 1070 ft/s is about 1320–1350 ft/s at the muzzle, more
        // for small shot, which slows faster.
        assert!(
            (1290.0..1430.0).contains(&(muzzle / FPS)),
            "{size}: {muzzle}"
        );
        let path = pellets::fly(&pellet, muzzle, &calm, 45.0).unwrap();
        for (yards, tabled) in [30.0, 35.0, 40.0, 45.0].iter().zip(tabled) {
            let energy = pellet.energy_j(pellet_at(&path, yards * YARD).velocity) / FOOT_POUND;
            assert!(
                (energy / tabled - 1.0).abs() <= 0.08,
                "English {size}: {energy:.2} vs {tabled} ft-lb at {yards} yd"
            );
        }
    }
}

#[test]
fn small_shot_drifts_by_the_wind_times_its_lag() {
    // No. 8 lead (0.090 in) at 1377 ft/s drifts 9.1 in at 40 yards in a
    // 10 mph crosswind (Lowry and Garner, Shotshell Ballistics for Windows).
    let pellet = pellets::Pellet {
        diameter_m: 0.090 * INCH,
        density_kg_m3: pellets::HARD_LEAD_DENSITY_KG_M3,
    };
    let muzzle = 1377.0 * FPS;
    let wind = 10.0 * 0.447_04;
    let crosswind = Conditions {
        wind: Wind {
            speed: wind,
            from_degrees: 90.0,
        },
        ..lowry_air()
    };
    let path = pellets::fly(&pellet, muzzle, &crosswind, 40.0).unwrap();
    let there = pellet_at(&path, pellets::PATTERN_DISTANCE_M);
    let drift = there.windage.abs();
    assert!(
        (drift / INCH / 9.1 - 1.0).abs() <= 0.15,
        "{} in",
        drift / INCH
    );
    // Didion's rule: the drift is the wind times the time lost to drag.
    let lag = there.time - pellets::PATTERN_DISTANCE_M / muzzle;
    assert!(
        (drift / (wind * lag) - 1.0).abs() < 0.01,
        "{drift} vs {}",
        wind * lag
    );
}

#[test]
fn shot_carries_about_as_far_as_journees_rule() {
    // 2200 yards per inch of pellet: 79.2 m a millimetre, for lead.
    let calm = pellets::calm(Atmosphere::standard());
    let reach = |pellet| pellets::max_range(&pellet, 400.0, &calm).unwrap();
    let mut lead_3_5 = None;
    for millimetres in [2.0, 3.0, 3.5, 4.0] {
        let pellet = pellets::Pellet::of(pellets::Material::Lead, millimetres / 1000.0);
        let farthest = reach(pellet);
        let rule = 2200.0 * YARD * millimetres / 25.4;
        assert!(
            (farthest.distance_m / rule - 1.0).abs() <= 0.10,
            "{millimetres} mm: {:.0} vs {rule:.0} m",
            farthest.distance_m
        );
        assert!(
            (22.0..30.0).contains(&farthest.elevation_degrees),
            "{millimetres} mm: {}°",
            farthest.elevation_degrees
        );
        assert!(farthest.time_s > 2.0 && farthest.velocity > 10.0);
        if millimetres == 3.5 {
            lead_3_5 = Some(farthest.distance_m);
        }
    }
    let lead_3_5 = lead_3_5.unwrap();
    // Steel carries less far than lead of its size, buckshot much farther.
    let steel = reach(pellets::Pellet::of(pellets::Material::Steel, 0.0035)).distance_m;
    assert!((0.7..0.85).contains(&(steel / lead_3_5)), "{steel}");
    let buck = reach(pellets::Pellet::of(pellets::Material::Lead, 0.008_38)).distance_m;
    assert!((480.0..620.0).contains(&buck), "{buck}");
    // A round ball from a 12 bore flies longer than a bullet's 15 s.
    let ball = reach(pellets::Pellet::of(pellets::Material::Lead, 0.0185));
    assert!(
        ball.time_s > 14.0 && (25.0..40.0).contains(&ball.elevation_degrees),
        "{ball:?}"
    );
}

#[test]
fn wind_carries_shot_farther_and_steeper() {
    let calm = pellets::calm(Atmosphere::standard());
    let small = pellets::Pellet::of(pellets::Material::Lead, 0.002);
    let still = pellets::max_range(&small, 400.0, &calm).unwrap();
    let blown = |speed: f64, from_degrees: f64| {
        let windy = Conditions {
            wind: Wind {
                speed,
                from_degrees,
            },
            ..calm
        };
        pellets::max_range(&small, 400.0, &windy).unwrap()
    };
    let tailwind = blown(5.0, 180.0);
    assert!(
        tailwind.distance_m > still.distance_m * 1.05,
        "{tailwind:?}"
    );
    // In a gale the best elevation is far above a calm day's, and found,
    // not the end of the search.
    let gale = blown(30.0, 180.0);
    assert!(
        (50.0..78.0).contains(&gale.elevation_degrees) && gale.distance_m > tailwind.distance_m,
        "{gale:?}"
    );
    // A headwind shortens it, and nothing blown back counts as far.
    let headwind = blown(10.0, 0.0);
    assert!(headwind.distance_m < still.distance_m, "{headwind:?}");
}

#[test]
fn a_velocity_measured_downrange_gives_the_muzzle_velocity_back() {
    let pellet = pellets::Pellet::of(pellets::Material::Lead, 0.003);
    let calm = pellets::calm(Atmosphere::standard());
    let path = pellets::fly(&pellet, 400.0, &calm, 20.0).unwrap();
    let at = |range| pellet_at(&path, range);
    let v2_5 = at(2.5).velocity;
    // A V2.5 is 25–35 m/s below the muzzle velocity.
    assert!((365.0..376.0).contains(&v2_5), "{v2_5}");
    for (velocity, measured) in [
        (v2_5, pellets::VelocityAt::Distance(2.5)),
        (
            18.288 / at(18.288).time,
            pellets::VelocityAt::AverageOver(18.288),
        ),
        (400.0, pellets::VelocityAt::Muzzle),
    ] {
        let muzzle = pellets::muzzle_velocity(&pellet, velocity, measured, &calm).unwrap();
        assert!((muzzle - 400.0).abs() < 0.05, "{measured:?}: {muzzle}");
    }
    for refused in [
        pellets::VelocityAt::Distance(0.0),
        pellets::VelocityAt::Distance(80.0),
        pellets::VelocityAt::AverageOver(f64::NAN),
    ] {
        assert!(pellets::muzzle_velocity(&pellet, 370.0, refused, &calm).is_err());
    }
    assert!(
        pellets::muzzle_velocity(&pellet, 990.0, pellets::VelocityAt::Distance(40.0), &calm)
            .is_err()
    );
    // A tailwind faster than the shot carries even a pellet that barely
    // leaves the muzzle 18 m at 10 m/s on average: the muzzle velocity
    // found is that slow one.
    let gale = Conditions {
        wind: Wind {
            speed: 30.0,
            from_degrees: 180.0,
        },
        ..calm
    };
    let muzzle =
        pellets::muzzle_velocity(&pellet, 10.0, pellets::VelocityAt::AverageOver(18.0), &gale)
            .unwrap();
    let path = pellets::fly(&pellet, muzzle, &gale, 18.0).unwrap();
    assert!(
        muzzle < 10.0 && (18.0 / pellet_at(&path, 18.0).time - 10.0).abs() < 0.01,
        "{muzzle}"
    );
}

#[test]
fn slugs_fly_as_their_makers_tabled_with_a_g1_bc() {
    // Brenneke Classic 12/70, 31.5 g at 430 m/s: 370, 330, 300 and 275 m/s
    // at 25, 50, 75 and 100 m; G1 BC about 0.071.
    let mut slug = scenario();
    slug.bullet = Bullet {
        mass_kg: 0.0315,
        diameter_m: 0.0185,
        length_m: None,
        ballistic_coefficient: 0.071,
        drag_model: DragModel::G1,
        bc_bands: BcBands::NONE,
        drag_curve: None,
    };
    slug.load.muzzle_velocity = 430.0;
    slug.rifle.zero_range_m = 50.0;
    slug.spin_drift = false;
    for (range, tabled) in [(25.0, 370.0), (50.0, 330.0), (75.0, 300.0), (100.0, 275.0)] {
        let velocity = row(&slug, range).velocity;
        assert!(
            (velocity / tabled - 1.0).abs() <= 0.02,
            "{velocity:.0} vs {tabled} m/s at {range} m"
        );
    }
}

#[test]
fn shot_sizes_read_as_their_countries_number_them() {
    assert_eq!(
        pellets::diameter_of(&pellets::US_SIZES, "5"),
        Some(0.003_05)
    );
    assert_eq!(
        pellets::diameter_of(&pellets::US_SIZES, "bb"),
        Some(0.004_57)
    );
    assert_eq!(
        pellets::diameter_of(&pellets::ENGLISH_SIZES, "5"),
        Some(0.002_79)
    );
    assert_eq!(pellets::diameter_of(&pellets::US_SIZES, "13"), None);
    for (number, millimetres) in [
        ("1", 4.1),
        ("2", 3.75),
        ("3", 3.5),
        ("5", 3.0),
        ("7", 2.5),
        ("7½", 2.4),
        ("9", 2.0),
    ] {
        let diameter = pellets::nordic_diameter(number).unwrap();
        assert!((diameter * 1000.0 - millimetres).abs() < 1e-9, "{number}");
    }
    assert_eq!(pellets::nordic_diameter("0"), None);
    assert_eq!(pellets::nordic_diameter("BB"), None);
    // Sizes grow down the tables; whole US numbers from 7 to FF by 0.010 in.
    for sizes in [&pellets::US_SIZES[..], &pellets::ENGLISH_SIZES[..]] {
        let diameters: Vec<f64> = sizes.iter().map(|&(_, diameter)| diameter).collect();
        let rising = diameters.windows(2).all(|pair| pair[0] < pair[1]);
        let falling = diameters.windows(2).all(|pair| pair[0] > pair[1]);
        assert!(rising || falling);
    }
    for pair in pellets::US_SIZES[4..18].windows(2) {
        assert!(
            (pair[1].1 - pair[0].1 - 0.000_254).abs() < 0.000_03,
            "{pair:?}"
        );
    }
    assert!((pellets::steel_for_lead(0.003) - 0.0035).abs() < 1e-12);
    // 32 g of hardened lead US 5 holds about 190 pellets (SAAMI: 171 an ounce).
    let count = pellets::Pellet::of(pellets::Material::Lead, 0.003_05)
        .count_in(0.032)
        .unwrap();
    assert!((185..200).contains(&count), "{count}");
    for material in pellets::Material::ALL {
        assert!((7_000.0..19_000.0).contains(&material.density_kg_m3()));
        assert_eq!(material.is_lead(), material == pellets::Material::Lead);
    }
}

#[test]
fn a_disc_catches_its_share_of_a_round_normal_spread() {
    let (sigma, radius) = (0.2, 0.09);
    let centred = pellets::share_on_disc(sigma, radius, 0.0);
    assert!((centred - (1.0 - (-(radius * radius) / (2.0 * sigma * sigma)).exp())).abs() < 1e-12);
    // Off centre, against counting a large seeded pattern.
    let places = pellets::pattern_at(60.0, 100_000, 11, 30.0).unwrap();
    let sigma = pellets::sigma_at(60.0, 30.0).unwrap();
    for offset in [0.05, 0.2, 0.4, 0.8] {
        let counted = places
            .iter()
            .filter(|[right, up]| (right - offset).hypot(*up) <= radius)
            .count() as f64
            / places.len() as f64;
        let share = pellets::share_on_disc(sigma, radius, offset);
        assert!(
            (share - counted).abs() < 0.002 + share * 0.05,
            "{offset}: {share} vs {counted}"
        );
    }
    assert!(
        (pellets::share_on_disc(sigma, radius, 1e-9) - pellets::share_on_disc(sigma, radius, 0.0))
            .abs()
            < 1e-6
    );
    // The side the offset is on doesn't matter.
    assert_eq!(
        pellets::share_on_disc(0.1, 0.09, -0.05),
        pellets::share_on_disc(0.1, 0.09, 0.05)
    );
    // A spread far smaller than the disc, one σ inside its edge: 84 %.
    let edge = pellets::share_on_disc(0.000_5, 0.09, 0.089_5);
    assert!((edge - 0.841).abs() < 0.003, "{edge}");
    assert_eq!(pellets::share_on_disc(0.01, 0.09, 0.5), 0.0);
    assert!(pellets::share_on_disc(f64::NAN, 0.09, 0.0).is_nan());
    assert_eq!(pellets::share_on_disc(0.0, 0.1, 0.05), 1.0);
    assert_eq!(pellets::share_on_disc(0.0, 0.1, 0.5), 0.0);
    assert_eq!(pellets::share_on_disc(0.1, 0.0, 0.0), 0.0);
}

#[test]
fn hits_on_the_vitals_are_binomial() {
    // Many pellets each unlikely to hit: Poisson, P(at least k) for 3 and 5
    // hits expected.
    for (expected, k, chance) in [
        (3.0, 1, 0.950),
        (3.0, 3, 0.577),
        (5.0, 3, 0.875),
        (5.0, 5, 0.560),
    ] {
        let got = pellets::chance_of_at_least(k, 20_000, expected / 20_000.0);
        assert!((got - chance).abs() < 0.001, "{expected}, {k}: {got}");
    }
    // Few pellets: 5 or more of 9, each half the time, is exactly half.
    assert!((pellets::chance_of_at_least(5, 9, 0.5) - 0.5).abs() < 1e-12);
    assert_eq!(pellets::chance_of_at_least(5, 9, 1.0), 1.0);
    assert_eq!(pellets::chance_of_at_least(10, 9, 0.9), 0.0);
    assert_eq!(pellets::chance_of_at_least(0, 0, 0.0), 1.0);
    assert_eq!(pellets::chance_of_at_least(2, 100, 0.0), 0.0);
    // Hundreds expected: no underflow. 800 or more where 750 are expected
    // is about 3.5 % (normal approximation).
    let far = pellets::chance_of_at_least(800, 100_000, 0.0075);
    assert!((0.025..0.05).contains(&far), "{far}");

    // Nine 00 buckshot all on the vitals at a metre: five hits for sure.
    let buck = pellets::chance_of_hits(9, 60.0, 1.0, 0.18, 5, 0.0).unwrap();
    assert!(buck > 0.999, "{buck}");

    // The Swedish hunting test's roe deer at 20 m: 28 pellets in an 18 cm
    // circle. A 32 g load of US 7 (2.5 mm, about 350 pellets) from half
    // choke, aimed true, puts twice that there; the test leaves room for
    // the running target's aim.
    let pellet = pellets::Pellet::of(pellets::Material::Lead, 0.0025);
    let load = pellet.count_in(0.032).unwrap();
    let sigma = pellets::sigma_at(60.0, 20.0).unwrap();
    let expected = load as f64 * pellets::share_on_disc(sigma, 0.09, 0.0);
    assert!((55.0..80.0).contains(&expected), "{expected}");

    // Aim error lowers the chance; a perfect aim at close range is near
    // certain, a sloppy one isn't.
    let chance = |aim| pellets::chance_of_hits(load, 60.0, 20.0, 0.18, 10, aim).unwrap();
    assert!(chance(0.0) > 0.99);
    assert!(chance(0.1) < chance(0.0) && chance(0.3) < chance(0.1));
    assert!(chance(0.3) > 0.01);
    assert!(pellets::chance_of_hits(load, 60.0, 20.0, 0.18, 1_001, 0.0).is_err());
    assert!(pellets::chance_of_hits(load, 60.0, 20.0, 0.0, 3, 0.0).is_err());
    assert!(pellets::chance_of_hits(load, 60.0, 20.0, 0.18, 3, -1.0).is_err());
    assert!(pellets::chance_of_hits(load, 0.0, 20.0, 0.18, 3, 0.0).is_err());
    assert!(pellets::chance_of_hits(pellets::MAX_PELLETS + 1, 60.0, 20.0, 0.18, 3, 0.0).is_err());
}

#[test]
fn aim_error_is_averaged_as_drawing_many_misses_would() {
    // 36 g of 3.5 mm lead (144 pellets), half choke, 18 cm vitals, 5 hits.
    let misses = pellets::pattern(50.0, 20_000, 5).unwrap();
    let unit = pellets::pattern_sigma(50.0).unwrap();
    for (range, aim_rad) in [(25.0, 0.005), (30.0, 0.05), (10.0, 0.05), (40.0, 0.099)] {
        let aim = aim_rad * range;
        let sigma = pellets::sigma_at(60.0, range).unwrap();
        let drawn = misses
            .iter()
            .map(|[right, up]| {
                let offset = right.hypot(*up) / unit * aim;
                pellets::chance_of_at_least(5, 144, pellets::share_on_disc(sigma, 0.09, offset))
            })
            .sum::<f64>()
            / misses.len() as f64;
        let averaged = pellets::chance_of_hits(144, 60.0, range, 0.18, 5, aim).unwrap();
        assert!(
            (averaged - drawn).abs() < 0.004 + 0.02 * drawn,
            "{range} m, {aim_rad} rad: {averaged} vs {drawn}"
        );
    }
}

#[test]
fn a_load_is_enough_out_to_where_its_hits_or_energy_run_out() {
    let calm = pellets::calm(Atmosphere::standard());
    // 36 g of US 3 (3.5 mm) lead from half choke on a roe deer: 5 pellets of
    // at least 6 J in its 18 cm heart-lung circle, nine times in ten, with a
    // good shot's 5 mrad aim error. Finnish advice is 25 m at most, Danish
    // 20 m.
    let pellet = pellets::Pellet::of(pellets::Material::Lead, 0.0035);
    let load = pellets::ShotLoad {
        pellet,
        muzzle_velocity: 400.0,
        pellets: pellet.count_in(0.036).unwrap(),
        pattern_percent: 60.0,
    };
    let roe_deer = pellets::Quarry {
        vital_diameter_m: 0.18,
        min_hits: 5,
        min_pellet_energy_j: 6.0,
    };
    let effective = pellets::effective_range(&load, &roe_deer, &calm, 0.005, 0.9)
        .unwrap()
        .unwrap();
    assert_eq!(effective.limit, Some(pellets::RangeLimit::Hits));
    assert!(
        (20.0..32.0).contains(&effective.farthest_m),
        "{effective:?}"
    );
    // Aimed true, a tighter choke reaches farther. With a hunter's aim
    // error it gains little: its denser middle misses more often (BASC
    // found fewer mallard hit with tighter chokes).
    let full = pellets::ShotLoad {
        pattern_percent: 70.0,
        ..load
    };
    let reach = |load: &pellets::ShotLoad, aim: f64| {
        pellets::effective_range(load, &roe_deer, &calm, aim, 0.9)
            .unwrap()
            .unwrap()
            .farthest_m
    };
    let aimed_true =
        |percent| pellets::chance_of_hits(load.pellets, percent, 35.0, 0.18, 5, 0.0).unwrap();
    assert!(aimed_true(70.0) > aimed_true(60.0) + 0.05);
    assert!(reach(&full, 0.005) <= effective.farthest_m + 3.0);
    // A sloppier aim reaches less far.
    let sloppy = pellets::effective_range(&load, &roe_deer, &calm, 0.012, 0.9)
        .unwrap()
        .unwrap();
    assert!(sloppy.farthest_m < effective.farthest_m, "{sloppy:?}");
    // Small shot runs out of energy before it runs out of pellets.
    let small = pellets::Pellet::of(pellets::Material::Lead, 0.0024);
    let small_load = pellets::ShotLoad {
        pellet: small,
        pellets: small.count_in(0.036).unwrap(),
        ..load
    };
    let goose = pellets::Quarry {
        vital_diameter_m: 0.16,
        min_hits: 3,
        min_pellet_energy_j: 2.03,
    };
    let by_energy = pellets::effective_range(&small_load, &goose, &calm, 0.005, 0.9)
        .unwrap()
        .unwrap();
    assert_eq!(
        by_energy.limit,
        Some(pellets::RangeLimit::Energy),
        "{by_energy:?}"
    );
    let impossible = pellets::Quarry {
        min_pellet_energy_j: 100.0,
        ..roe_deer
    };
    assert_eq!(
        pellets::effective_range(&load, &impossible, &calm, 0.005, 0.9).unwrap(),
        None
    );
    assert!(pellets::effective_range(&load, &roe_deer, &calm, 0.005, 1.0).is_err());
    assert!(pellets::effective_range(&load, &roe_deer, &calm, -0.1, 0.9).is_err());
}

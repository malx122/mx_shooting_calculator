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
fn a_crosswind_from_the_right_throws_a_right_hand_twist_bullet_down() {
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
        from_right < 0.0 && from_left > 0.0,
        "{from_right} {from_left}"
    );
    assert!((from_right + from_left).abs() < 1e-3);
    // A left-hand twist turns it round.
    assert!(windy(90.0, true, false) - windy(90.0, false, false) > 0.0);
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

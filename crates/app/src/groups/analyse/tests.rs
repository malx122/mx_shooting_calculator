//! The Analyse pages drawn headlessly with the demo data, and the rules
//! behind them: which groups a zero check counts, the calculator's
//! offset and precision, saved analyses and the trend's axes.

use ballistics_store::{
    Analysis, AnalysisKind, Barrel, Clean, Group, Ladder, LadderVariable, MemoryPhotos, Recipe,
    RoundLogEntry, Session, ZeroReset,
};
use eframe::egui;
use groups_core::{CompositeMode, ZeroVerdict};

use super::pool::{self, Against, Ineligible, Metric, Pick, TrendFilters, ZeroScope};
use super::*;
use crate::groups::tests::demo_data;
use crate::groups::{GroupSettings, GroupsPage, View};
use crate::theme::ThemeChoice;
use crate::units::{Quantity, UnitPrefs};

/// Draws the Groups page twice, as the app does when sizes settle.
fn draw(page: &mut GroupsPage, data: &Data, choice: ThemeChoice, width: f32) -> Vec<Action> {
    let context = egui::Context::default();
    crate::theme::apply(&context);
    crate::theme::choose(&context, choice);
    let photos = MemoryPhotos::default();
    let mut actions = Vec::new();
    for _ in 0..2 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 844.0),
                )),
                ..Default::default()
            },
            |ui| {
                crate::mobile::apply_style(ui.ctx());
                actions.extend(page.show(ui, data, &photos));
            },
        );
        output.textures_delta.clear();
    }
    actions
}

fn by_notes<'a>(data: &'a Data, notes: &str) -> &'a Group {
    data.groups
        .groups
        .iter()
        .find(|group| group.notes.starts_with(notes))
        .expect("a demo group")
}

#[test]
fn every_analyse_view_draws_in_both_themes_on_a_desktop_and_a_phone() {
    let data = demo_data();
    let two = vec![
        by_notes(&data, "Shot 3").id.clone(),
        by_notes(&data, "First shot").id.clone(),
    ];
    let several: Vec<String> = data
        .groups
        .groups
        .iter()
        .take(5)
        .map(|g| g.id.clone())
        .collect();
    for choice in [ThemeChoice::Dark, ThemeChoice::Sun] {
        for width in [1200.0, 390.0] {
            let mut page = GroupsPage::new(GroupSettings::default());
            page.view = View::Analyse;
            for tab in Tab::ALL {
                page.analyse.tab = tab;
                assert!(
                    draw(&mut page, &data, choice, width).is_empty(),
                    "{tab:?} saves nothing until asked"
                );
            }
            for metric in Metric::ALL {
                for against in [Against::Date, Against::Temperature, Against::Charge] {
                    page.analyse.tab = Tab::Trend;
                    page.analyse.trend.metric = metric;
                    page.analyse.trend.against = against;
                    page.analyse.trend.boxes = true;
                    page.analyse.trend.filters.ladders = true;
                    draw(&mut page, &data, choice, width);
                }
            }
            page.analyse.compare(two.clone());
            assert!(draw(&mut page, &data, choice, width).is_empty());
            page.analyse.compare(several.clone());
            draw(&mut page, &data, choice, width);
            for mode in [None, Some(CompositeMode::AboutOwnCentres)] {
                page.analyse.composite(several.clone());
                if let Some(state) = &mut page.analyse.composite {
                    state.mode = mode;
                }
                assert!(draw(&mut page, &data, choice, width).is_empty());
            }
            assert_eq!(page.depth(), 2, "the composite over Analyse");
            page.back(&data);
            assert_eq!(page.depth(), 1);
            page.back(&data);
            assert!(matches!(page.view, View::Library));
            page.selection.selecting = true;
            page.selection.ids = two.clone();
            draw(&mut page, &data, choice, width);
        }
    }
}

#[test]
fn saved_analyses_open_rebuilt_from_the_groups_left() {
    let data = demo_data();
    let ids: Vec<String> = data
        .groups
        .groups
        .iter()
        .take(3)
        .map(|g| g.id.clone())
        .collect();
    let mut with_gone = ids.clone();
    with_gone.insert(1, "deleted-since".into());
    let compare = Analysis::new(AnalysisKind::Compare, "", with_gone);
    let rebuilt = pool::rebuild(&compare, &data);
    assert_eq!(
        rebuilt.ids, ids,
        "in the saved order, the deleted one skipped"
    );
    assert_eq!(rebuilt.missing, 1);

    let composite = pool::composite_analysis(
        "Oryx".into(),
        ids[..2].to_vec(),
        CompositeMode::AboutOwnCentres,
        300.0,
    );
    let rebuilt = pool::rebuild(&composite, &data);
    assert_eq!(rebuilt.mode, Some(CompositeMode::AboutOwnCentres));
    assert_eq!(rebuilt.distance_m, Some(300.0));
    assert!(composite.validate().is_ok());

    let mut analyse = Analyse::default();
    let gone = Analysis::new(
        AnalysisKind::Compare,
        "",
        vec![ids[0].clone(), "gone".into()],
    );
    analyse.open_saved(&gone, &data);
    assert_eq!(analyse.tab, Tab::Compare);
    assert_eq!(
        analyse.compare.ids.len(),
        1,
        "fewer than 2 remain: it says so"
    );
    assert_eq!(analyse.compare.missing, 1);
    analyse.open_saved(&composite, &data);
    assert_eq!(analyse.depth(), 1, "a composite opens over the tab");
}

fn grid_group(id: &str, taken_at: i64, shots: &[(f64, f64)]) -> Group {
    let mut group = Group::on_grid(200.0, 200.0, taken_at);
    group.id = id.into();
    if let ballistics_store::Marks::Grid(grid) = &mut group.marks {
        grid.shots = shots
            .iter()
            .map(|&(x_mm, y_mm)| ballistics_store::GridShot {
                x_mm,
                y_mm,
                cold_bore: false,
                called_flier: false,
                velocity_mps: None,
            })
            .collect();
    }
    group.rifle_id = Some("rifle".into());
    group.bullet_id = Some("load".into());
    group.distance_m = Some(100.0);
    group
}

#[test]
fn a_zero_check_counts_only_what_its_rules_let_through() {
    let scope = ZeroScope {
        rifle_id: "rifle".into(),
        barrel: Pick::One("proof".into()),
        load: Pick::One("load".into()),
    };
    let mut resets = vec![ZeroReset::new("rifle", None, 1_000)];
    let mut group = grid_group("a", 2_000, &[(1.0, 2.0), (3.0, 1.0)]);
    group.barrel_id = Some("proof".into());
    let mut proof = Barrel::new("rifle", "Proof");
    proof.id = "proof".into();
    let mut factory = Barrel::new("rifle", "Factory");
    factory.id = "factory".into();
    let barrels = vec![proof, factory];
    let check = |group: &Group, resets: &[ZeroReset]| {
        pool::zero_eligibility(group, &scope, resets, Some(100.0), &barrels)
    };
    assert_eq!(check(&group, &resets), Ok(()));

    // Only at the zero range: within 5% or 5 m, whichever is more.
    let mut other = group.clone();
    other.distance_m = Some(105.0);
    assert_eq!(check(&other, &resets), Ok(()));
    other.distance_m = Some(106.0);
    assert_eq!(check(&other, &resets), Err(Ineligible::NotAtZeroRange));
    other.distance_m = Some(300.0);
    assert_eq!(check(&other, &resets), Err(Ineligible::NotAtZeroRange));
    assert!(pool::at_zero_range(309.0, 300.0) && !pool::at_zero_range(316.0, 300.0));
    assert!(pool::at_zero_range(46.0, 50.0) && !pool::at_zero_range(44.0, 50.0));

    // A barrel deleted since takes its resets with it: its groups stay out.
    let mut removed = group.clone();
    removed.barrel_id = Some("sold".into());
    let anything = ZeroScope {
        barrel: Pick::All,
        ..scope.clone()
    };
    assert_eq!(
        pool::zero_eligibility(&removed, &anything, &resets, Some(100.0), &barrels),
        Err(Ineligible::BarrelRemoved)
    );

    let mut other = group.clone();
    other.barrel_id = Some("factory".into());
    assert_eq!(check(&other, &resets), Err(Ineligible::OtherBarrel));
    other.barrel_id = None;
    assert_eq!(
        check(&other, &resets),
        Err(Ineligible::OtherBarrel),
        "no barrel given"
    );
    let mut other = group.clone();
    other.bullet_id = Some("another".into());
    assert_eq!(check(&other, &resets), Err(Ineligible::OtherLoad));
    let mut other = group.clone();
    other.rifle_id = Some("another".into());
    assert_eq!(check(&other, &resets), Err(Ineligible::OtherRifle));

    let mut other = group.clone();
    other.include_in_zero = false;
    assert_eq!(check(&other, &resets), Err(Ineligible::NotIncluded));
    let mut other = group.clone();
    other.ladder_id = Some("ladder".into());
    assert_eq!(check(&other, &resets), Err(Ineligible::Ladder));
    let mut other = group.clone();
    other.distance_m = None;
    assert_eq!(check(&other, &resets), Err(Ineligible::NoDistance));
    let mut other = group.clone();
    if let ballistics_store::Marks::Grid(grid) = &mut other.marks {
        grid.aim_mm = None;
    }
    assert_eq!(check(&other, &resets), Err(Ineligible::NoAim));

    // A reset for the rifle counts for every barrel; one naming a barrel
    // only for groups through it.
    let mut early = group.clone();
    early.taken_at = 500;
    assert_eq!(check(&early, &resets), Err(Ineligible::BeforeReset));
    resets = vec![ZeroReset::new("rifle", Some("factory"), 3_000)];
    assert_eq!(check(&group, &resets), Ok(()), "another barrel's reset");
    resets.push(ZeroReset::new("rifle", Some("proof"), 2_500));
    assert_eq!(check(&group, &resets), Err(Ineligible::BeforeReset));
    resets.push(ZeroReset::new("another rifle", None, 9_000));
    assert_eq!(
        pool::latest_reset(&resets, "rifle", Some("proof")).map(|r| r.at),
        Some(2_500)
    );
    let any_barrel = ZeroScope {
        barrel: Pick::All,
        load: Pick::All,
        ..scope.clone()
    };
    other = group.clone();
    other.barrel_id = Some("factory".into());
    assert_eq!(
        pool::zero_eligibility(&other, &any_barrel, &resets, Some(100.0), &barrels),
        Err(Ineligible::BeforeReset),
        "every barrel, each from its own reset"
    );
    other.taken_at = 4_000;
    assert_eq!(
        pool::zero_eligibility(&other, &any_barrel, &resets, Some(100.0), &barrels),
        Ok(())
    );
}

#[test]
fn use_in_calculator_writes_where_the_load_hits_and_the_engine_aims_for_it() {
    let mut data = Data::default();
    let mut rifle = ballistics_store::Rifle::new("Tikka", "308-win", 11.0);
    rifle.id = "rifle".into();
    rifle.zero_range_m = 100.0;
    let mut bullet = ballistics_store::Bullet::new_manual("308-win", 7.82);
    bullet.id = "load".into();
    bullet.mass_g = 11.7;
    bullet.bc_g1 = Some(0.45);
    bullet.muzzle_velocity_mps = 790.0;
    data.rifles.push(rifle.clone());
    data.bullets.push(bullet.clone());
    // Three groups landing 20 mm high and 10 mm right at 100 m.
    for (i, (x, y)) in [(9.0, 21.0), (11.0, 19.0), (10.0, 20.0)].iter().enumerate() {
        let shots = [(x - 3.0, y + 2.0), (x + 3.0, y - 1.0), (*x, y - 1.0)];
        data.groups
            .groups
            .push(grid_group(&format!("g{i}"), 10 + i as i64, &shots));
    }
    let scope = ZeroScope {
        rifle_id: "rifle".into(),
        barrel: Pick::All,
        load: Pick::One("load".into()),
    };
    let zero = pool::zero_data(&data, &scope);
    let check = zero.check.expect("groups").expect("a check");
    assert_eq!(check.groups, 3);
    assert!(
        check.correction_mrad.x < 0.0 && check.correction_mrad.y < 0.0,
        "dial left, down"
    );
    let offset = pool::zero_offset(&check);
    assert!(
        (offset[0] - 0.1).abs() < 2e-3,
        "right is positive: {offset:?}"
    );
    assert!((offset[1] - 0.2).abs() < 2e-3, "up is positive: {offset:?}");
    assert!(pool::zero_writable(&check), "a verdict from three groups");
    let load = pool::load_with_offset(&data, "rifle", "load", offset);
    assert_eq!(load.impact_offset_mrad, offset);
    assert_eq!(
        (load.rifle_id.as_str(), load.bullet_id.as_str()),
        ("rifle", "load")
    );

    // With that offset the calculator puts the shot where the groups went:
    // 2 cm high and 1 cm right at the zero range.
    let conditions = ballistics_shot::Conditions::default();
    let solve = |load: Option<&ballistics_store::RifleLoad>| {
        let scenario = ballistics_shot::scenario(&rifle, &bullet, load, &conditions);
        ballistics_core::solver::solve(&scenario, 200.0).expect("a solution")
    };
    let plain = solve(None).at(100.0).expect("the zero range");
    let offset_shot = solve(Some(&load)).at(100.0).expect("the zero range");
    let up_cm = (offset_shot.drop - plain.drop) * 100.0;
    let right_cm = (offset_shot.windage - plain.windage) * 100.0;
    assert!((up_cm - 2.0).abs() < 0.1, "{up_cm} cm higher");
    assert!((right_cm - 1.0).abs() < 0.1, "{right_cm} cm to the right");
}

/// A .308 zeroed at 100 m, with a bullet the solver can fly.
fn zeroed_308(zero_height_cm: f64) -> (Data, ballistics_store::Rifle) {
    let mut data = Data::default();
    let mut rifle = ballistics_store::Rifle::new("Tikka", "308-win", 11.0);
    rifle.id = "rifle".into();
    rifle.zero_range_m = 100.0;
    rifle.zero_height_cm = zero_height_cm;
    let mut bullet = ballistics_store::Bullet::new_manual("308-win", 7.82);
    bullet.id = "load".into();
    bullet.mass_g = 11.7;
    bullet.bc_g1 = Some(0.45);
    bullet.muzzle_velocity_mps = 790.0;
    data.rifles.push(rifle.clone());
    data.bullets.push(bullet);
    (data, rifle)
}

/// Three shots about `centre` (mm), the group's own centre exactly there.
fn about(centre: (f64, f64)) -> [(f64, f64); 3] {
    let (x, y) = centre;
    [(x - 2.0, y + 1.0), (x + 2.0, y + 1.0), (x, y - 2.0)]
}

#[test]
fn a_group_at_300_m_with_the_come_up_dialled_is_no_zero_error() {
    // The case that once wrote [0, -1.69] mil: on the aim at 300 m with
    // the come-up dialled. It isn't shot at the zero range, so it is left
    // out, and nothing reaches the calculator.
    let (mut data, rifle) = zeroed_308(0.0);
    let come_up = -pool::expected_impact_mrad(&grid_group("x", 0, &[]), &rifle, 300.0, &data).y;
    assert!(
        come_up > 1.0,
        "a .308 drops well over 1 mil at 300 m: {come_up}"
    );
    let mut far = grid_group("far", 5, &about((0.0, 0.0)));
    far.distance_m = Some(300.0);
    far.dialled_offset_mrad = [0.0, come_up];
    data.groups.groups.push(far);
    let scope = ZeroScope {
        rifle_id: "rifle".into(),
        barrel: Pick::All,
        load: Pick::One("load".into()),
    };
    let zero = pool::zero_data(&data, &scope);
    assert!(zero.check.is_none(), "no group at the zero range");
    assert_eq!(
        zero.left_out,
        [("far".to_owned(), Ineligible::NotAtZeroRange)]
    );
}

#[test]
fn groups_near_the_zero_range_are_measured_from_where_the_bullet_flies() {
    // Zeroed 4 cm high at 100 m. Groups at 95, 100 and 105 m that land
    // exactly where the solver says the bullet goes (less what was
    // dialled for one of them) have no zero error at all.
    let (mut data, rifle) = zeroed_308(4.0);
    for (i, (distance, dialled)) in [(95.0, 0.0), (100.0, 0.3), (105.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        let expected =
            pool::expected_impact_mrad(&grid_group("x", 0, &[]), &rifle, distance, &data);
        let up_mm = (expected.y + dialled) * distance / 1000.0 * 1000.0;
        let across_mm = expected.x * distance;
        let mut group = grid_group(&format!("g{i}"), 10 + i as i64, &about((across_mm, up_mm)));
        group.distance_m = Some(distance);
        group.dialled_offset_mrad = [0.0, dialled];
        data.groups.groups.push(group);
    }
    let at_100 = pool::expected_impact_mrad(&grid_group("x", 0, &[]), &rifle, 100.0, &data);
    assert!(
        (at_100.y - 0.4).abs() < 1e-3,
        "the zero's own 4 cm: {at_100:?}"
    );
    let scope = ZeroScope {
        rifle_id: "rifle".into(),
        barrel: Pick::All,
        load: Pick::All,
    };
    let zero = pool::zero_data(&data, &scope);
    assert_eq!(zero.dialled_groups, 1);
    let check = zero.check.expect("groups").expect("a check");
    for net in &check.net_impacts_mrad {
        assert!(net.x.abs() < 1e-6 && net.y.abs() < 1e-6, "{net:?}");
    }
}

#[test]
fn your_precision_is_the_five_shot_group_at_100_m_edge_to_edge() {
    // σ 0.1 mrad: an average 5-shot spread of 3.07 cm at 100 m, centre to
    // centre, plus a .308 bullet's 0.782 cm.
    let cm = pool::five_shot_group_cm(0.1, Some(7.82));
    assert!((cm - 3.852).abs() < 1e-9, "{cm}");
    // The calculator reads it back as the per-axis σ of the aim: within a
    // calibre's worth of the measured one.
    let aim_mrad =
        ballistics_core::dispersion::Uncertainty::aim_from_group(cm / 100.0, 100.0) * 1000.0;
    assert!(aim_mrad >= 0.1 && aim_mrad - 0.1 < 0.782 / 30.7 + 1e-9);

    let data = demo_data();
    let x_bolt = "9f017006-3ab4-50b9-b751-ea5070d9b3b7";
    let oryx = "5e88862d-acbc-5cde-b884-20367b2c163e";
    let precision = pool::precision(&data, x_bolt, oryx).expect("the Oryx groups");
    assert_eq!(precision.groups, 7, "every Oryx group, the ladder left out");
    assert!(precision.enough());
    assert!(precision.sigma_mrad > 0.01 && precision.sigma_mrad < 0.1);
    let expected = pool::five_shot_group_cm(precision.sigma_mrad, Some(7.82));
    assert!((precision.group_cm - expected).abs() < 1e-12);

    let accubond = "c251775b-5176-5236-a3ef-01b7f2d5aa10";
    assert!(
        pool::precision(&data, x_bolt, accubond).is_none(),
        "ladder rungs don't count"
    );
    let one = pool::Precision {
        sigma_mrad: 0.1,
        groups: 1,
        shots: 9,
        group_cm: 3.0,
    };
    assert!(!one.enough(), "1 group of 9 shots is too few");
    assert!(pool::Precision { shots: 10, ..one }.enough());
    assert!(pool::Precision { groups: 2, ..one }.enough());
}

#[test]
fn the_demo_zero_holds_its_ladder_has_a_flat_run_and_its_velocity_a_sensitivity() {
    let data = demo_data();
    let scope = ZeroScope::default_for(&data).expect("a rifle");
    assert_eq!(
        scope.load,
        Pick::One("5e88862d-acbc-5cde-b884-20367b2c163e".into())
    );
    let zero = pool::zero_data(&data, &scope);
    let check = zero.check.expect("groups").expect("a check");
    assert_eq!(check.verdict, ZeroVerdict::Holding);
    assert!(
        zero.left_out
            .iter()
            .any(|(_, why)| *why == Ineligible::BeforeReset),
        "the group before the reset"
    );

    let ladder = &data.groups.ladders[0];
    let result = pool::ladder(&data, ladder, groups_core::LadderKey::Default);
    let worked = result.ladder.expect("groups").expect("a ladder");
    assert_eq!(worked.rungs.len(), 6);
    let flat = worked
        .best
        .iter()
        .filter(|run| run.flat)
        .max_by_key(|run| run.rungs)
        .expect("a flat run");
    let middle_gr = pool::run_middle(&worked, flat) / 0.064_798_91;
    assert!((45.0..=46.0).contains(&middle_gr), "{middle_gr} gr");

    let filters = TrendFilters {
        bullet_id: Some(scope.load.one().unwrap().to_owned()),
        ..TrendFilters::default()
    };
    let trend = pool::trend(
        &data,
        Metric::VelocityAverage,
        Against::Temperature,
        &filters,
    );
    let fit = trend.series[0]
        .sensitivity
        .as_ref()
        .expect("fitted")
        .as_ref()
        .expect("4 sessions over 28 °C");
    assert!(fit.per_degree.value > 0.3 && fit.per_degree.value < 1.0);
}

#[test]
fn trend_axes_are_kept_in_base_units_and_shown_in_the_hunters() {
    let mut data = Data::default();
    let mut rifle = ballistics_store::Rifle::new("Tikka", "308-win", 11.0);
    rifle.id = "rifle".into();
    rifle.rounds_before_logging = 100;
    data.rifles.push(rifle);
    let mut bullet = ballistics_store::Bullet::new_manual("308-win", 7.82);
    bullet.id = "load".into();
    bullet.recipe = Some(Recipe {
        charge_g: Some(2.916),
        cbto_mm: Some(55.88),
        ..Recipe::default()
    });
    data.bullets.push(bullet);
    let mut ladder = Ladder::new("rifle", "load", LadderVariable::Seating);
    ladder.id = "ladder".into();
    data.groups.ladders.push(ladder);
    let mut session = Session::new(0);
    session.id = "s".into();
    session.conditions.temperature_c = Some(5.0);
    session.conditions.pressure_hpa = Some(1000.0);
    data.groups.sessions.push(session);
    let mut barrel = Barrel::new("rifle", "Proof");
    barrel.id = "proof".into();
    barrel.rounds_before_logging = 50;
    data.groups.barrels.push(barrel);
    let mut entry = RoundLogEntry::new("rifle", Some("proof"), 20, 1_000);
    entry.id = "e1".into();
    data.groups.round_log.push(entry);
    let mut entry = RoundLogEntry::new("rifle", Some("proof"), 5, 3_000);
    entry.id = "e2".into();
    data.groups.round_log.push(entry);
    data.groups
        .cleans
        .push(Clean::new("rifle", Some("proof"), 1_500));

    let mut group = grid_group("g", 2_000, &[(0.0, 0.0), (3.0, 1.0)]);
    group.session_id = Some("s".into());
    group.barrel_id = Some("proof".into());
    let value = |group: &Group, against: Against| pool::axis_value(group, against, 3, &data);
    assert_eq!(value(&group, Against::Order), Some(3.0));
    assert_eq!(
        value(&group, Against::Charge),
        Some(2.916),
        "grams, from the recipe"
    );
    assert_eq!(value(&group, Against::Seating), Some(55.88), "mm");
    assert_eq!(
        value(&group, Against::Temperature),
        Some(5.0),
        "the session's"
    );
    assert_eq!(value(&group, Against::Pressure), Some(1000.0));
    assert_eq!(
        value(&group, Against::BarrelRounds),
        Some(70.0),
        "50 before, 20 logged"
    );
    assert_eq!(
        value(&group, Against::SinceClean),
        Some(0.0),
        "cleaned since"
    );
    group.conditions.powder_temperature_c = Some(12.0);
    assert_eq!(
        value(&group, Against::Temperature),
        Some(12.0),
        "the powder's first"
    );
    group.ladder_id = Some("ladder".into());
    group.rung_value = Some(56.4);
    assert_eq!(
        value(&group, Against::Seating),
        Some(56.4),
        "a seating rung"
    );
    assert_eq!(
        value(&group, Against::Charge),
        Some(2.916),
        "the ladder varies seating"
    );

    let imperial = UnitPrefs::imperial();
    let metric = UnitPrefs::metric();
    assert_eq!(Against::Charge.quantity(), Some(Quantity::PowderCharge));
    let grains = super::trend::shown_x(&imperial, Against::Charge, 2.916);
    assert!((grains - 45.0).abs() < 0.01, "{grains} gr");
    let metric_grains = super::trend::shown_x(&metric, Against::Charge, 2.916);
    assert!(
        (metric_grains - 45.0).abs() < 0.01,
        "grains for metric users too"
    );
    let inches = super::trend::shown_x(&imperial, Against::Seating, 55.88);
    assert!((inches - 2.2).abs() < 1e-9);
    let fahrenheit = super::trend::shown_x(&imperial, Against::Temperature, 10.0);
    assert!((fahrenheit - 50.0).abs() < 1e-9);
    assert_eq!(
        super::trend::shown_x(&imperial, Against::Date, 20_000.0),
        20_000.0
    );
    assert_eq!(Metric::Sigma.quantity(), Quantity::GroupAngle);
    let moa = super::trend::shown_y(&imperial, Metric::MeanRadius, 1.0);
    assert!((moa - 3.4377).abs() < 1e-3, "mrad shown in MOA");
    let fps = super::trend::shown_y(&imperial, Metric::VelocityAverage, 800.0);
    assert!((fps - 2624.67).abs() < 0.1);
}

#[test]
fn trends_leave_ladders_out_unless_asked_and_keep_a_loads_colour() {
    let data = demo_data();
    let all = TrendFilters::default();
    let with_ladders = TrendFilters {
        ladders: true,
        ..TrendFilters::default()
    };
    let plain = pool::trend(&data, Metric::MeanRadius, Against::Date, &all);
    let more = pool::trend(&data, Metric::MeanRadius, Against::Date, &with_ladders);
    assert_eq!(more.groups, plain.groups + 6, "the six rungs");
    let colour = |trend: &pool::TrendData, id: &str| {
        trend
            .series
            .iter()
            .find(|series| series.bullet_id.as_deref() == Some(id))
            .map(|series| series.colour)
    };
    let oryx = "5e88862d-acbc-5cde-b884-20367b2c163e";
    let only = TrendFilters {
        bullet_id: Some(oryx.into()),
        ..TrendFilters::default()
    };
    let filtered = pool::trend(&data, Metric::MeanRadius, Against::Date, &only);
    assert_eq!(filtered.series.len(), 1);
    assert_eq!(
        colour(&filtered, oryx),
        colour(&plain, oryx),
        "same colour filtered"
    );
    assert!(more.points() >= 2);
}

#[test]
fn the_calculator_gets_m_s_per_degree_and_only_where_it_uses_it() {
    let sessions: Vec<groups_core::TemperatureSession> =
        [(0.0, 790.0), (10.0, 795.0), (20.0, 800.0)]
            .into_iter()
            .map(
                |(temperature_c, mean_mps)| groups_core::TemperatureSession {
                    temperature_c,
                    mean_mps,
                    shots: 5,
                    sd_mps: 2.0,
                },
            )
            .collect();
    let fit = groups_core::TemperatureSensitivity::new(&sessions).expect("a fit");
    let (data, _) = zeroed_308(0.0);
    let bullet = data.bullets[0].clone();
    let written = pool::bullet_with_sensitivity(&bullet, &fit);
    assert!(
        (written.velocity_per_degree - 0.5).abs() < 1e-9,
        "m/s per °C, not %"
    );
    assert!((fit.percent_per_degree.value - 0.5).abs() > 0.1);

    // A load with its own velocities at two temperatures sets its own.
    let mut data = data;
    assert!(!pool::load_sets_own_sensitivity(&data, "rifle", &bullet));
    let mut load = ballistics_store::RifleLoad::new("rifle", &bullet.id);
    for (temperature_c, velocity_mps) in [(0.0, 785.0), (20.0, 800.0)] {
        load.velocities.push(ballistics_store::VelocityPoint {
            temperature_c,
            velocity_mps,
            shots: None,
        });
    }
    data.loads.push(load);
    assert!(pool::load_sets_own_sensitivity(&data, "rifle", &bullet));
}

#[test]
fn a_ladders_run_becomes_a_load_with_the_rifles_data() {
    let (mut data, _) = zeroed_308(0.0);
    let source = data.bullets[0].clone();
    let mut load = ballistics_store::RifleLoad::new("rifle", &source.id);
    load.impact_offset_mrad = [0.1, 0.2];
    data.loads.push(load);
    let charge = Ladder::new("rifle", &source.id, LadderVariable::Charge);
    let new = pool::promotion(&source, None, &charge, 2.95, "45.5 gr charge", &data);
    let recipe = new.bullet.recipe.clone().expect("a recipe");
    assert_eq!(recipe.charge_g, Some(2.95));
    assert_eq!(recipe.cbto_mm, None, "a charge ladder leaves the CBTO");
    assert_ne!(new.bullet.id, source.id, "a new load");
    assert_eq!(
        new.ladder.promoted_bullet_id.as_deref(),
        Some(new.bullet.id.as_str())
    );
    let copied = new.load.expect("the rifle's data for it");
    assert_eq!(copied.bullet_id, new.bullet.id);
    assert_eq!(copied.impact_offset_mrad, [0.1, 0.2]);

    let seating = Ladder::new("rifle", &source.id, LadderVariable::Seating);
    let mut target = source.clone();
    target.recipe = Some(Recipe {
        charge_g: Some(2.9),
        ..Recipe::default()
    });
    let into = pool::promotion(&source, Some(&target), &seating, 55.9, "", &data);
    let recipe = into.bullet.recipe.expect("a recipe");
    assert_eq!(recipe.cbto_mm, Some(55.9), "a seating ladder sets the CBTO");
    assert_eq!(recipe.charge_g, Some(2.9), "and keeps the charge");
    assert_eq!(into.bullet.id, target.id);
    assert!(into.load.is_none());
}

#[test]
fn a_group_keeps_its_recipe_when_the_load_changes_later() {
    let (mut data, _) = zeroed_308(0.0);
    data.bullets[0].recipe = Some(Recipe {
        charge_g: Some(2.9),
        cbto_mm: Some(55.0),
        ..Recipe::default()
    });
    let mut group = grid_group("g", 1, &about((0.0, 0.0)));
    let form = crate::groups::form::Form::of(&group);
    form.apply(&mut group, &data);
    assert_eq!((group.charge_g, group.cbto_mm), (Some(2.9), Some(55.0)));
    assert!(group.validate().is_ok());
    data.bullets[0].recipe.as_mut().unwrap().charge_g = Some(3.0);
    assert_eq!(
        pool::axis_value(&group, Against::Charge, 1, &data),
        Some(2.9)
    );
    group.charge_g = None;
    assert_eq!(
        pool::axis_value(&group, Against::Charge, 1, &data),
        Some(3.0),
        "groups saved without one fall back to the recipe"
    );
    // Without a barrel a group has no barrel rounds.
    data.groups
        .round_log
        .push(RoundLogEntry::new("rifle", None, 20, 0));
    assert_eq!(
        pool::axis_value(&group, Against::BarrelRounds, 1, &data),
        None
    );
}

#[test]
fn a_sensitivity_reads_per_degree_of_the_hunters_unit() {
    let sessions: Vec<groups_core::TemperatureSession> =
        [(0.0, 790.0), (10.0, 795.0), (20.0, 800.0)]
            .into_iter()
            .map(
                |(temperature_c, mean_mps)| groups_core::TemperatureSession {
                    temperature_c,
                    mean_mps,
                    shots: 5,
                    sd_mps: 2.0,
                },
            )
            .collect();
    let fit = groups_core::TemperatureSensitivity::new(&sessions).expect("a fit");
    let metric = super::trend::sensitivity_text(&fit, &UnitPrefs::metric());
    let imperial = super::trend::sensitivity_text(&fit, &UnitPrefs::imperial());
    let per_c = fit.percent_per_degree.value;
    assert!(metric.contains(&format!("{per_c:.2}% per °C")), "{metric}");
    let per_f = per_c / 1.8;
    assert!(
        imperial.contains(&format!("{per_f:.2}% per °F")),
        "{imperial}"
    );
}

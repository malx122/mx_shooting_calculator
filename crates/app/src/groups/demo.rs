//! Groups for the demo data: a few marked on blank grids, with the demo
//! rifles and bullets, so the library has something to show. One has a
//! cold-bore shot and one a called flier; the X-Bolt's Oryx groups since
//! its zero reset give the zero check a verdict and the trend a powder
//! temperature fit, and six rungs of three shots make a charge ladder.

use ballistics_store::model::now_ms;
use ballistics_store::{
    GridShot, Group, Ladder, LadderVariable, Marks, Session, Store, StoreError, ZeroReset,
};

const HOUR_MS: i64 = 3_600_000;
const SESSION: &str = "6d787363-6772-6f75-7073-64656d6f0001";
const X_BOLT: &str = "9f017006-3ab4-50b9-b751-ea5070d9b3b7";
const TIKKA: &str = "e0da971c-b935-5372-92f0-ad2a18519022";
const ORYX_308: &str = "5e88862d-acbc-5cde-b884-20367b2c163e";
const TERMINAL_ASCENT: &str = "c8c47f9b-7bc2-5e90-990a-e9e556da65df";
const ORYX_223: &str = "50f64e7e-5cba-5493-bfdb-44e6837a9343";
const ACCUBOND: &str = "c251775b-5176-5236-a3ef-01b7f2d5aa10";
const LADDER: &str = "6d787363-6772-6f75-7073-64656d6f0201";
const RESET: &str = "6d787363-6772-6f75-7073-64656d6f0301";
const DAY: i64 = 24;
/// The X-Bolt's fitted barrel in the demo's rifle history.
const PROOF: &str = "6d787363-6261-7272-656c-64656d6f0002";
const GRAMS_PER_GRAIN: f64 = 0.064_798_91;

/// A shot: x and y in mm from the aim, and whether it was the cold-bore
/// shot or a called flier.
type Mark = (f64, f64, bool, bool);

struct DemoGroup {
    id: &'static str,
    hours_ago: i64,
    session: bool,
    rifle: &'static str,
    bullet: &'static str,
    distance_m: f64,
    shots: &'static [Mark],
    /// Muzzle velocities of the first shots, m/s.
    velocities: &'static [f64],
    tags: &'static [&'static str],
    notes: &'static str,
    /// A rung of the demo ladder, at this charge in grains.
    rung_gr: Option<f64>,
    /// The temperature, °C, where it differs from the session's.
    temperature_c: Option<f64>,
}

/// How many groups the demo adds.
#[cfg(test)]
pub const GROUP_COUNT: usize = GROUPS.len();

const GROUPS: [DemoGroup; 16] = [
    DemoGroup {
        id: "6d787363-6772-6f75-7073-64656d6f0101",
        hours_ago: 1,
        session: true,
        rifle: X_BOLT,
        bullet: TERMINAL_ASCENT,
        distance_m: 100.0,
        shots: &[
            (14.0, -4.0, false, false),
            (22.0, 3.0, false, false),
            (9.0, -12.0, false, false),
            (18.0, -9.0, false, false),
            (25.0, -2.0, false, false),
        ],
        velocities: &[792.0, 797.0, 789.0, 795.0, 801.0],
        tags: &["zero"],
        notes: "",
        rung_gr: None,
        temperature_c: None,
    },
    DemoGroup {
        id: "6d787363-6772-6f75-7073-64656d6f0102",
        hours_ago: 2,
        session: true,
        rifle: X_BOLT,
        bullet: ORYX_308,
        distance_m: 100.0,
        shots: &[
            (3.0, 6.0, false, false),
            (-8.0, 1.0, false, false),
            (42.0, -31.0, false, true),
            (1.0, -6.0, false, false),
            (-4.0, 10.0, false, false),
            (6.0, -2.0, false, false),
        ],
        velocities: &[],
        tags: &["practice"],
        notes: "Shot 3 jerked the trigger.",
        rung_gr: None,
        temperature_c: None,
    },
    DemoGroup {
        id: "6d787363-6772-6f75-7073-64656d6f0103",
        hours_ago: 3,
        session: true,
        rifle: X_BOLT,
        bullet: ORYX_308,
        distance_m: 100.0,
        shots: &[
            (-11.0, 19.0, true, false),
            (2.0, 4.0, false, false),
            (-3.0, -2.0, false, false),
        ],
        velocities: &[],
        tags: &["cold bore"],
        notes: "First shot of the morning from a clean barrel.",
        rung_gr: None,
        temperature_c: None,
    },
    DemoGroup {
        id: "6d787363-6772-6f75-7073-64656d6f0104",
        hours_ago: 9 * 24 + 5,
        session: false,
        rifle: TIKKA,
        bullet: ORYX_223,
        distance_m: 100.0,
        shots: &[
            (-6.0, 8.0, false, false),
            (4.0, 13.0, false, false),
            (-1.0, 2.0, false, false),
            (9.0, 6.0, false, false),
            (-3.0, 15.0, false, false),
            (2.0, 9.0, false, false),
            (-7.0, 4.0, false, false),
            (52.0, -38.0, false, false),
        ],
        velocities: &[],
        tags: &["practice", "prone"],
        notes: "",
        rung_gr: None,
        temperature_c: None,
    },
    DemoGroup {
        id: "6d787363-6772-6f75-7073-64656d6f0105",
        hours_ago: 9 * 24 + 4,
        session: false,
        rifle: X_BOLT,
        bullet: TERMINAL_ASCENT,
        distance_m: 300.0,
        shots: &[
            (-21.0, 30.0, false, false),
            (14.0, 8.0, false, false),
            (-5.0, -18.0, false, false),
            (30.0, 22.0, false, false),
            (2.0, 41.0, false, false),
            (-30.0, 5.0, false, false),
            (19.0, -9.0, false, false),
            (-12.0, 16.0, false, false),
            (8.0, 28.0, false, false),
            (-2.0, 3.0, false, false),
        ],
        velocities: &[],
        tags: &["practice"],
        notes: "Wind from 9 o'clock, gusting.",
        rung_gr: None,
        temperature_c: None,
    },
    zero_group(
        "6d787363-6772-6f75-7073-64656d6f0106",
        0,
        true,
        &[
            (4.0, 6.0, false, false),
            (-1.0, 0.0, false, false),
            (6.0, 3.0, false, false),
            (-2.0, 7.0, false, false),
            (3.0, -1.0, false, false),
        ],
        &[788.0, 791.0, 786.0, 790.0, 789.0],
        None,
    ),
    zero_group(
        "6d787363-6772-6f75-7073-64656d6f0107",
        6 * DAY,
        false,
        &[
            (-6.0, 3.0, false, false),
            (1.0, -2.0, false, false),
            (-4.0, 5.0, false, false),
            (-1.0, -3.0, false, false),
            (-5.0, 2.0, false, false),
        ],
        &[795.0, 797.0, 794.0, 798.0, 796.0],
        Some(14.0),
    ),
    zero_group(
        "6d787363-6772-6f75-7073-64656d6f0108",
        13 * DAY,
        false,
        &[
            (3.0, -5.0, false, false),
            (-2.0, 1.0, false, false),
            (4.0, -1.0, false, false),
            (-1.0, -3.0, false, false),
        ],
        &[801.0, 799.0, 803.0, 800.0],
        Some(22.0),
    ),
    zero_group(
        "6d787363-6772-6f75-7073-64656d6f0109",
        20 * DAY,
        false,
        &[
            (2.0, 7.0, false, false),
            (-3.0, 2.0, false, false),
            (1.0, 6.0, false, false),
            (-2.0, 1.0, false, false),
            (2.0, 4.0, false, false),
        ],
        &[782.0, 785.0, 781.0, 784.0, 783.0],
        Some(-6.0),
    ),
    // Before the zero reset: the old rings shot right and high.
    zero_group(
        "6d787363-6772-6f75-7073-64656d6f0110",
        40 * DAY,
        false,
        &[
            (27.0, 24.0, false, false),
            (31.0, 19.0, false, false),
            (25.0, 21.0, false, false),
            (30.0, 26.0, false, false),
        ],
        &[],
        Some(8.0),
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0201",
        44.0,
        &[
            (-3.0, -12.0, false, false),
            (4.0, -7.0, false, false),
            (-1.0, -8.0, false, false),
        ],
        &[768.0, 772.0, 765.0],
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0202",
        44.5,
        &[
            (2.0, -5.0, false, false),
            (-4.0, 1.0, false, false),
            (3.0, -2.0, false, false),
        ],
        &[779.0, 776.0, 782.0],
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0203",
        45.0,
        &[
            (-2.0, 9.0, false, false),
            (3.0, 4.0, false, false),
            (1.0, 5.0, false, false),
        ],
        &[787.0, 790.0, 785.0],
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0204",
        45.5,
        &[
            (4.0, 6.0, false, false),
            (-3.0, 9.0, false, false),
            (0.0, 6.0, false, false),
        ],
        &[789.0, 792.0, 790.0],
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0205",
        46.0,
        &[
            (1.0, 3.0, false, false),
            (-2.0, 8.0, false, false),
            (5.0, 7.0, false, false),
        ],
        &[791.0, 793.0, 789.0],
    ),
    rung(
        "6d787363-6772-6f75-7073-64656d6f0206",
        46.5,
        &[
            (-1.0, 17.0, false, false),
            (4.0, 12.0, false, false),
            (-4.0, 16.0, false, false),
        ],
        &[801.0, 805.0, 799.0],
    ),
];

/// A group of the X-Bolt with Oryx at 100 m, for the zero check.
const fn zero_group(
    id: &'static str,
    hours_ago: i64,
    session: bool,
    shots: &'static [Mark],
    velocities: &'static [f64],
    temperature_c: Option<f64>,
) -> DemoGroup {
    DemoGroup {
        id,
        hours_ago,
        session,
        rifle: X_BOLT,
        bullet: ORYX_308,
        distance_m: 100.0,
        shots,
        velocities,
        tags: &["zero"],
        notes: "",
        rung_gr: None,
        temperature_c,
    }
}

/// A rung of the AccuBond charge ladder, shot two days ago at 100 m.
const fn rung(
    id: &'static str,
    charge_gr: f64,
    shots: &'static [Mark],
    velocities: &'static [f64],
) -> DemoGroup {
    DemoGroup {
        id,
        hours_ago: 2 * DAY + 4,
        session: false,
        rifle: X_BOLT,
        bullet: ACCUBOND,
        distance_m: 100.0,
        shots,
        velocities,
        tags: &["ladder"],
        notes: "",
        rung_gr: Some(charge_gr),
        temperature_c: Some(9.0),
    }
}

/// Adds the demo groups (and their session) that aren't there yet; says
/// how many were added.
pub fn load(store: &Store) -> Result<usize, StoreError> {
    let now = now_ms();
    if store.session(SESSION)?.is_none() {
        let mut session = Session::new(now - 3 * HOUR_MS - HOUR_MS / 2);
        session.id = SESSION.to_owned();
        session.place = "Demo range".to_owned();
        session.conditions.temperature_c = Some(3.0);
        session.conditions.pressure_hpa = Some(1011.0);
        store.save_session(&mut session)?;
    }
    if store.ladder(LADDER)?.is_none() {
        let mut ladder = Ladder::new(X_BOLT, ACCUBOND, LadderVariable::Charge);
        ladder.id = LADDER.to_owned();
        ladder.name = "N140 ladder".to_owned();
        store.save_ladder(&mut ladder)?;
    }
    if !store.zero_resets()?.iter().any(|reset| reset.id == RESET) {
        let mut reset = ZeroReset::new(X_BOLT, None, (now - 30 * DAY * HOUR_MS).max(0));
        reset.id = RESET.to_owned();
        reset.note = "New rings".to_owned();
        store.save_zero_reset(&mut reset)?;
    }
    let mut added = 0;
    for demo in &GROUPS {
        if store.group(demo.id)?.is_some() {
            continue;
        }
        let mut group = Group::on_grid(200.0, 200.0, (now - demo.hours_ago * HOUR_MS).max(0));
        group.id = demo.id.to_owned();
        if let Marks::Grid(grid) = &mut group.marks {
            grid.shots = demo
                .shots
                .iter()
                .map(|&(x_mm, y_mm, cold_bore, called_flier)| GridShot {
                    x_mm,
                    y_mm,
                    cold_bore,
                    called_flier,
                    velocity_mps: None,
                })
                .collect();
            for (shot, mps) in grid.shots.iter_mut().zip(demo.velocities) {
                shot.velocity_mps = Some(*mps);
            }
        }
        group.rifle_id = Some(demo.rifle.to_owned());
        if demo.rifle == X_BOLT {
            group.barrel_id = Some(PROOF.to_owned());
        }
        group.bullet_id = Some(demo.bullet.to_owned());
        group.distance_m = Some(demo.distance_m);
        group.session_id = demo.session.then(|| SESSION.to_owned());
        group.tags = demo.tags.iter().map(|tag| (*tag).to_owned()).collect();
        group.notes = demo.notes.to_owned();
        group.count_rounds = false;
        group.conditions.temperature_c = demo.temperature_c;
        if let Some(charge_gr) = demo.rung_gr {
            group.ladder_id = Some(LADDER.to_owned());
            group.rung_value = Some(charge_gr * GRAMS_PER_GRAIN);
            group.include_in_zero = false;
        }
        store.save_group(&mut group)?;
        added += 1;
    }
    Ok(added)
}

//! Groups for the demo data: a few marked on blank grids, with the demo
//! rifles and bullets, so the library has something to show. One has a
//! cold-bore shot and one a called flier.

use ballistics_store::model::now_ms;
use ballistics_store::{GridShot, Group, Marks, Session, Store, StoreError};

const HOUR_MS: i64 = 3_600_000;
const SESSION: &str = "6d787363-6772-6f75-7073-64656d6f0001";
const X_BOLT: &str = "9f017006-3ab4-50b9-b751-ea5070d9b3b7";
const TIKKA: &str = "e0da971c-b935-5372-92f0-ad2a18519022";
const ORYX_308: &str = "5e88862d-acbc-5cde-b884-20367b2c163e";
const TERMINAL_ASCENT: &str = "c8c47f9b-7bc2-5e90-990a-e9e556da65df";
const ORYX_223: &str = "50f64e7e-5cba-5493-bfdb-44e6837a9343";

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
}

const GROUPS: [DemoGroup; 5] = [
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
    },
];

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
        group.bullet_id = Some(demo.bullet.to_owned());
        group.distance_m = Some(demo.distance_m);
        group.session_id = demo.session.then(|| SESSION.to_owned());
        group.tags = demo.tags.iter().map(|tag| (*tag).to_owned()).collect();
        group.notes = demo.notes.to_owned();
        group.count_rounds = false;
        store.save_group(&mut group)?;
        added += 1;
    }
    Ok(added)
}

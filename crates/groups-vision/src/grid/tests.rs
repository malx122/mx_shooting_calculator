use std::time::Instant;

use super::synth::Scene;
use super::*;

/// How a found rectangle compares with the truth.
#[derive(Debug)]
struct Check {
    /// The furthest any corner is from its true crossing, px.
    worst_px: f64,
    /// The corners' true crossings, in small squares.
    #[allow(dead_code)]
    indices: [[i64; 2]; 4],
}

/// Checks a rectangle against the scene: every corner within 0.5 px of a
/// true crossing (of the big squares when `every` > 1), the counts exactly
/// the squares between them, and the edges along the lines.
fn check(scene: &Scene, squares: &Squares, every: i64) -> Check {
    let mut worst: f64 = 0.0;
    let mut indices = [[0i64; 2]; 4];
    for (k, corner) in squares.corners.iter().enumerate() {
        let grid = scene.grid_at(*corner);
        let index = [grid[0].round(), grid[1].round()];
        let miss = miss_px(scene, *corner, index);
        worst = worst.max(miss);
        indices[k] = [index[0] as i64, index[1] as i64];
        if every > 1 {
            assert!(
                indices[k].iter().all(|i| i.rem_euclid(every) == 0),
                "corner {k} at {indices:?} is not on a heavy line"
            );
        }
    }
    let squares_between = |a: usize, b: usize| {
        let (di, dj) = (indices[b][0] - indices[a][0], indices[b][1] - indices[a][1]);
        assert!(
            di == 0 || dj == 0,
            "edge {a}–{b} runs across the lines: {indices:?}"
        );
        (di.abs() + dj.abs()) / every
    };
    assert_eq!(
        squares_between(0, 1),
        i64::from(squares.across),
        "across: {indices:?}"
    );
    assert_eq!(
        squares_between(3, 2),
        i64::from(squares.across),
        "across at the bottom"
    );
    assert_eq!(
        squares_between(0, 3),
        i64::from(squares.down),
        "down: {indices:?}"
    );
    assert_eq!(
        squares_between(1, 2),
        i64::from(squares.down),
        "down at the right"
    );
    assert!(
        worst < 0.5,
        "a corner is {worst:.2} px from its crossing: {squares:?}"
    );
    println!(
        "worst corner {worst:.3} px, {} × {} squares (of {every})",
        squares.across, squares.down
    );
    Check {
        worst_px: worst,
        indices,
    }
}

/// How far (px) photo point `p` is from crossing `index` of the scene:
/// one Newton step through the local Jacobian, which holds for curled
/// paper too.
fn miss_px(scene: &Scene, p: [f64; 2], index: [f64; 2]) -> f64 {
    let g = scene.grid_at(p);
    let gx = scene.grid_at([p[0] + 0.5, p[1]]);
    let gy = scene.grid_at([p[0], p[1] + 0.5]);
    let j = [
        [(gx[0] - g[0]) / 0.5, (gy[0] - g[0]) / 0.5],
        [(gx[1] - g[1]) / 0.5, (gy[1] - g[1]) / 0.5],
    ];
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let e = [g[0] - index[0], g[1] - index[1]];
    let dx = (j[1][1] * e[0] - j[0][1] * e[1]) / det;
    let dy = (-j[1][0] * e[0] + j[0][0] * e[1]) / det;
    dx.hypot(dy)
}

fn whole(scene: &Scene) -> [f64; 4] {
    [
        0.0,
        0.0,
        scene.camera.width as f64,
        scene.camera.height as f64,
    ]
}

fn find(scene: &Scene) -> (Grid, InkImage) {
    let (_, ink) = scene.render();
    let grid = find_grid(&ink, whole(scene), &Hints::default()).expect("a grid");
    (grid, ink)
}

/// The share of the photo the rectangle covers.
fn coverage(scene: &Scene, squares: &Squares) -> f64 {
    let p = squares.corners;
    let area = 0.5
        * (0..4)
            .map(|i| p[i][0] * p[(i + 1) % 4][1] - p[(i + 1) % 4][0] * p[i][1])
            .sum::<f64>()
            .abs();
    area / (scene.camera.width as f64 * scene.camera.height as f64)
}

fn rings_and_holes(scene: &mut Scene, centre: [f64; 2], ring_step_mm: f64) {
    for k in 1..=6 {
        scene.rings.push((centre, ring_step_mm * k as f64, 0.35));
    }
    scene.bullseye = Some((centre, 1.6 * ring_step_mm));
    let offsets = [
        [3.0, 4.0],
        [-6.0, 2.0],
        [9.0, -7.0],
        [-2.0, -11.0],
        [14.0, 9.0],
        [-15.0, -5.0],
        [1.0, 17.0],
        [-9.0, 13.0],
    ];
    for [dx, dy] in offsets {
        scene
            .holes
            .push(([centre[0] + dx * 2.0, centre[1] + dy * 2.0], 3.2));
    }
}

#[test]
fn finds_a_plain_grid_square_on() {
    let scene = Scene::plain(900, 700, 6.0, 5.0);
    let (grid, _) = find(&scene);
    assert!(grid.major.is_none());
    let found = check(&scene, &grid.minor, 1);
    assert!(coverage(&scene, &grid.minor) > 0.7, "{:?}", grid.minor);
    assert!(
        (grid.across.spacing_px - 30.0).abs() < 0.1,
        "{:?}",
        grid.across
    );
    assert!((grid.down.spacing_px - 30.0).abs() < 0.1, "{:?}", grid.down);
    assert!(
        (grid.across.angle_deg - 90.0).abs() < 0.1,
        "{:?}",
        grid.across
    );
    assert!(
        grid.down.angle_deg.min(180.0 - grid.down.angle_deg) < 0.1,
        "{:?}",
        grid.down
    );
    assert!(grid.confidence > 0.8, "{}", grid.confidence);
    assert!(found.worst_px < 0.2, "{found:?}");
}

#[test]
fn finds_a_turned_grid_in_perspective_through_a_phone_camera() {
    let mut scene = Scene::plain(1400, 1050, 7.0, 6.35).realistic();
    scene.camera.rotation_deg = 17.0;
    scene.camera.tilt = [1.2e-4, -0.8e-4];
    let (grid, _) = find(&scene);
    let found = check(&scene, &grid.minor, 1);
    assert!(coverage(&scene, &grid.minor) > 0.45, "{:?}", grid.minor);
    let [least, most] = grid.across.spacing_range_px;
    assert!(most / least > 1.1, "perspective shows: {:?}", grid.across);
    assert!(grid.confidence > 0.6, "{}", grid.confidence);
    println!("turned, perspective: worst {:.3} px", found.worst_px);
}

#[test]
fn finds_a_grid_turned_30_degrees() {
    let mut scene = Scene::plain(1200, 900, 5.0, 5.0).realistic();
    scene.camera.rotation_deg = -30.0;
    scene.camera.tilt = [-0.6e-4, 1.0e-4];
    let (grid, _) = find(&scene);
    check(&scene, &grid.minor, 1);
}

#[test]
fn counts_big_inch_squares_over_quarter_inches() {
    let mut scene = Scene::plain(1500, 1100, 4.0, 25.4 / 4.0).realistic();
    scene.major_every = 4;
    scene.line_mm = 0.15;
    scene.major_line_mm = 0.5;
    scene.camera.rotation_deg = -12.0;
    scene.camera.tilt = [0.7e-4, 0.5e-4];
    rings_and_holes(&mut scene, [20.0, 10.0], 9.0);
    let (grid, _) = find(&scene);
    let major = grid.major.expect("big squares");
    assert_eq!(major.every, 4);
    check(&scene, &major.squares, 4);
    check(&scene, &grid.minor, 1);
    assert_eq!(grid.squares(), &major.squares);
}

#[test]
fn counts_green_centimetres_over_millimetres() {
    let mut scene = Scene::plain(1300, 1000, 9.0, 1.0).realistic();
    scene.major_every = 10;
    scene.line_mm = 0.08;
    scene.major_line_mm = 0.3;
    scene.ink = [120.0, 190.0, 140.0];
    scene.major_ink = [40.0, 150.0, 80.0];
    scene.camera.rotation_deg = 6.0;
    let (grid, _) = find(&scene);
    let major = grid.major.expect("big squares");
    assert_eq!(major.every, 10);
    check(&scene, &major.squares, 10);
    check(&scene, &grid.minor, 1);
}

/// Millimetre lines too fine to see once the photo is reduced: the
/// centimetres are still found.
#[test]
fn finds_centimetres_when_the_millimetres_fade() {
    let mut scene = Scene::plain(2000, 1500, 9.0, 1.0).realistic();
    scene.major_every = 10;
    scene.line_mm = 0.08;
    scene.major_line_mm = 0.3;
    scene.ink = [120.0, 190.0, 140.0];
    scene.major_ink = [40.0, 150.0, 80.0];
    scene.camera.rotation_deg = -5.0;
    let (grid, _) = find(&scene);
    let every = grid.major.map_or(10, |major| i64::from(major.every));
    check(&scene, grid.squares(), every);
}

#[test]
fn finds_blue_and_orange_grids_under_rings_and_holes() {
    for (seed, ink) in [(2, [60.0, 100.0, 200.0]), (3, [235.0, 140.0, 40.0])] {
        let mut scene = Scene::plain(1200, 900, 5.0, 12.7).realistic();
        scene.seed = seed;
        scene.ink = ink;
        scene.line_mm = 0.35;
        scene.camera.rotation_deg = 4.0 - 8.0 * (seed - 2) as f64;
        scene.camera.tilt = [0.5e-4, 0.9e-4];
        rings_and_holes(&mut scene, [5.0, 3.0], 12.0);
        let (grid, _) = find(&scene);
        check(&scene, &grid.minor, 1);
        assert!(coverage(&scene, &grid.minor) > 0.35, "{:?}", grid.minor);
    }
}

#[test]
fn finds_the_grid_in_the_part_on_screen() {
    let mut scene = Scene::plain(1600, 1200, 8.0, 5.0).realistic();
    scene.camera.rotation_deg = 8.0;
    let (_, ink) = scene.render();
    let region = [500.0, 300.0, 1100.0, 800.0];
    let grid = find_grid(&ink, region, &Hints::default()).expect("a grid");
    check(&scene, &grid.minor, 1);
    for corner in grid.minor.corners {
        assert!(corner[0] > region[0] && corner[0] < region[2], "{corner:?}");
        assert!(corner[1] > region[1] && corner[1] < region[3], "{corner:?}");
    }
}

#[test]
fn refines_on_the_photos_own_pixels_when_it_works_reduced() {
    let mut scene = Scene::plain(2400, 1800, 9.0, 6.35).realistic();
    scene.camera.rotation_deg = -9.0;
    scene.camera.tilt = [0.4e-4, 0.3e-4];
    scene.line_mm = 0.3;
    scene.major_line_mm = 0.7;
    scene.major_every = 4;
    let (_, ink) = scene.render();
    let hints = Hints {
        work_long_edge_px: 1200,
        ..Hints::default()
    };
    let grid = find_grid(&ink, whole(&scene), &hints).expect("a grid");
    let found = check(&scene, &grid.minor, 1);
    let major = grid.major.expect("big squares");
    check(&scene, &major.squares, 4);
    assert!(found.worst_px < 0.1, "{found:?}");
}

#[test]
fn says_why_there_is_no_grid() {
    // Rings, holes and a black centre on blank paper.
    let mut target = Scene::plain(1000, 800, 5.0, 5.0).realistic();
    target.line_mm = 0.0;
    target.major_line_mm = 0.0;
    rings_and_holes(&mut target, [0.0, 0.0], 8.0);
    let (_, ink) = target.render();
    assert!(find_grid(&ink, whole(&target), &Hints::default()).is_err());
    // Blank paper.
    let mut blank = Scene::plain(800, 600, 5.0, 5.0).realistic();
    blank.line_mm = 0.0;
    blank.major_line_mm = 0.0;
    let (_, ink) = blank.render();
    assert!(matches!(
        find_grid(&ink, whole(&blank), &Hints::default()),
        Err(NoGrid::NoLines | NoGrid::TooFewLines)
    ));
    // Lined paper: one family only.
    let ruled = InkImage {
        width: 600,
        height: 400,
        pixels: (0..400)
            .flat_map(|y| (0..600).map(move |_| if y % 20 < 2 { 40 } else { 235 }))
            .collect(),
    };
    assert!(find_grid(&ruled, [0.0, 0.0, 600.0, 400.0], &Hints::default()).is_err());
    // Too small.
    let (_, ink) = Scene::plain(400, 300, 5.0, 5.0).render();
    assert_eq!(
        find_grid(&ink, [10.0, 10.0, 40.0, 200.0], &Hints::default()).unwrap_err(),
        NoGrid::TooSmall
    );
}

#[test]
fn a_grid_blurred_away_is_refused() {
    let mut scene = Scene::plain(800, 600, 4.0, 5.0);
    scene.blur_px = 7.0;
    scene.noise = 3.0;
    let (_, ink) = scene.render();
    let result = find_grid(&ink, whole(&scene), &Hints::default());
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn a_few_squares_still_make_a_rectangle() {
    let scene = Scene::plain(800, 600, 20.0, 10.0).realistic();
    let (grid, _) = find(&scene);
    let found = check(&scene, &grid.minor, 1);
    assert!(
        grid.minor.across >= 2 && grid.minor.down >= 1,
        "{found:?} {grid:?}"
    );
}

/// Turns from −30° to 30°, perspective both ways, big squares or none,
/// harsher JPEG: every corner within half a pixel, every count right.
#[test]
fn holds_over_turns_and_tilts() {
    let mut worst: f64 = 0.0;
    for (case, rotation) in [-30.0, -22.0, -14.0, -5.0, 3.0, 11.0, 19.0, 28.0]
        .into_iter()
        .enumerate()
    {
        let mut scene = Scene::plain(900, 700, 4.5, 25.4 / 4.0).realistic();
        scene.seed = 10 + case as u64;
        scene.camera.rotation_deg = rotation;
        let sign = if case % 2 == 0 { 1.0 } else { -1.0 };
        scene.camera.tilt = [sign * 1.4e-4, -sign * 0.9e-4 * (case % 3) as f64];
        scene.jpeg_quality = Some(65);
        scene.noise = 4.0;
        if case % 2 == 1 {
            scene.major_every = 4;
            scene.line_mm = 0.15;
            scene.major_line_mm = 0.45;
        }
        if case % 3 == 0 {
            rings_and_holes(&mut scene, [3.0, -4.0], 8.0);
        }
        let (grid, _) = find(&scene);
        let every = if scene.major_every > 0 {
            let major = grid
                .major
                .unwrap_or_else(|| panic!("case {case}: big squares"));
            assert_eq!(major.every, 4, "case {case}");
            4
        } else {
            assert!(grid.major.is_none(), "case {case}: {:?}", grid.major);
            1
        };
        worst = worst.max(check(&scene, grid.squares(), every).worst_px);
        assert!(
            coverage(&scene, grid.squares()) > 0.3,
            "case {case}: {:?}",
            grid.squares()
        );
    }
    println!("over turns and tilts: worst corner {worst:.3} px");
}

#[test]
fn ink_shows_coloured_lines_as_dark() {
    let paper = ink(245, 242, 235);
    for (r, g, b) in [(40, 160, 80), (60, 100, 200), (240, 140, 40), (20, 20, 20)] {
        assert!(ink(r, g, b) + 60 < paper, "{r} {g} {b}: {}", ink(r, g, b));
    }
}

/// The time for a 1500 px region; the limit holds for optimised builds.
#[test]
fn is_quick_enough() {
    let mut scene = Scene::plain(1500, 1125, 6.0, 25.4 / 4.0).realistic();
    scene.major_every = 4;
    scene.camera.rotation_deg = 10.0;
    rings_and_holes(&mut scene, [0.0, 0.0], 9.0);
    let (_, ink) = scene.render();
    let start = Instant::now();
    let grid = find_grid(&ink, whole(&scene), &Hints::default()).expect("a grid");
    let elapsed = start.elapsed();
    check(
        &scene,
        grid.squares(),
        grid.major.map_or(1, |m| i64::from(m.every)),
    );
    println!("1500 × 1125 px: {elapsed:?}");
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_millis() < 300, "{elapsed:?}");
    }
}

/// Writes sample photos for trying the app (`cargo test -p groups-vision
/// sample_photos -- --ignored`).
#[test]
#[ignore]
fn sample_photos() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens/m4");
    std::fs::create_dir_all(&directory).expect("a folder");
    let mut scene = Scene::plain(2400, 1800, 7.0, 25.4 / 4.0).realistic();
    scene.major_every = 4;
    scene.line_mm = 0.2;
    scene.major_line_mm = 0.55;
    scene.camera.rotation_deg = 7.0;
    scene.camera.tilt = [0.6e-4, 0.4e-4];
    rings_and_holes(&mut scene, [10.0, 5.0], 10.0);
    let (picture, _) = scene.render();
    picture
        .save(directory.join("grid-inch.jpg"))
        .expect("saved");
    let mut green = Scene::plain(2000, 1500, 9.0, 1.0).realistic();
    green.major_every = 10;
    green.line_mm = 0.08;
    green.major_line_mm = 0.3;
    green.ink = [120.0, 190.0, 140.0];
    green.major_ink = [40.0, 150.0, 80.0];
    green.camera.rotation_deg = -5.0;
    let (picture, _) = green.render();
    picture.save(directory.join("grid-mm.jpg")).expect("saved");
}

/// Targets as shot: a black centre and rings over a grid, seen at
/// different turns, tilts and distances.
#[test]
fn holds_on_targets_with_black_centres() {
    let mut worst: f64 = 0.0;
    for case in 0..8u64 {
        let size = [(1000, 750), (1400, 1050), (1200, 1200), (1600, 1000)][case as usize % 4];
        let px_per_mm = [3.5, 5.0, 7.0, 4.2][case as usize % 4];
        let mut scene = Scene::plain(
            size.0,
            size.1,
            px_per_mm,
            [5.0, 25.4 / 4.0, 10.0, 12.7][(case as usize / 2) % 4],
        )
        .realistic();
        scene.seed = 100 + case;
        scene.camera.rotation_deg = -25.0 + 7.0 * case as f64;
        scene.camera.tilt = [
            1.3e-4 * ((case % 3) as f64 - 1.0),
            1.1e-4 * ((case % 2) as f64 * 2.0 - 1.0),
        ];
        let centre = [
            scene.camera.centre_mm[0] + 3.0,
            scene.camera.centre_mm[1] - 2.0,
        ];
        rings_and_holes(&mut scene, centre, 6.0 + 2.0 * (case % 4) as f64);
        scene.bullseye = Some((centre, 12.0 + 4.0 * (case % 3) as f64));
        let (grid, _) = find(&scene);
        worst = worst.max(check(&scene, &grid.minor, 1).worst_px);
        assert!(
            coverage(&scene, &grid.minor) > 0.3,
            "case {case}: {:?}",
            grid.minor
        );
    }
    println!("black centres: worst corner {worst:.3} px");
}

/// A black centre bigger than a few squares, with rings and holes, in the
/// middle where the search starts.
#[test]
fn starts_round_a_black_centre() {
    let mut scene = Scene::plain(1800, 1350, 5.0, 25.4 / 4.0).realistic();
    scene.major_every = 4;
    scene.line_mm = 0.2;
    scene.major_line_mm = 0.55;
    scene.camera.rotation_deg = 7.0;
    scene.camera.tilt = [0.8e-4, 0.5e-4];
    rings_and_holes(&mut scene, [2.0, 3.0], 12.0);
    scene.bullseye = Some(([2.0, 3.0], 30.0));
    let (grid, _) = find(&scene);
    let major = grid.major.expect("big squares");
    assert_eq!(major.every, 4);
    check(&scene, &major.squares, 4);
    check(&scene, &grid.minor, 1);
    assert!(coverage(&scene, &major.squares) > 0.5, "{major:?}");
    // Looking round the black centre, no corner is placed where the lines
    // can't be seen; closer in, it finds the grid right or not at all.
    let (_, ink) = scene.render();
    let middle = scene.crossing(2.0 / scene.square_mm, 3.0 / scene.square_mm);
    for (reach_mm, must) in [(70.0, true), (45.0, false)] {
        let reach = reach_mm * scene.camera.px_per_mm;
        let region = [
            middle[0] - reach,
            middle[1] - 0.8 * reach,
            middle[0] + reach,
            middle[1] + 0.8 * reach,
        ];
        let grid = match find_grid(&ink, region, &Hints::default()) {
            Ok(grid) => grid,
            Err(reason) if !must => {
                println!("{reach_mm} mm round the centre: {reason}");
                continue;
            }
            Err(reason) => panic!("{reach_mm} mm round the centre: {reason}"),
        };
        check(
            &scene,
            grid.squares(),
            grid.major.map_or(1, |m| i64::from(m.every)),
        );
        for corner in grid.squares().corners {
            let from_middle = (corner[0] - middle[0]).hypot(corner[1] - middle[1]);
            assert!(
                from_middle > 30.0 * scene.camera.px_per_mm,
                "{corner:?} is in the black centre"
            );
        }
    }
}

/// Paper curling away on the right can't be one homography: the count is
/// either right along every edge or there is none.
#[test]
fn curled_paper_is_counted_right_or_refused() {
    let mut found = 0;
    for (case, curl) in [0.05, 0.1, 0.2].into_iter().enumerate() {
        let mut fine = Scene::plain(1600, 1200, 6.0, 5.0).realistic();
        fine.curl = curl;
        fine.seed = 30 + case as u64;
        let mut inch = Scene::plain(1600, 1200, 4.0, 25.4 / 4.0).realistic();
        inch.curl = curl;
        inch.major_every = 4;
        inch.line_mm = 0.2;
        inch.major_line_mm = 0.45;
        inch.camera.rotation_deg = 3.0;
        for scene in [fine, inch] {
            let (_, ink) = scene.render();
            if let Ok(grid) = find_grid(&ink, whole(&scene), &Hints::default()) {
                let every = grid.major.map_or(1, |m| i64::from(m.every));
                check(&scene, grid.squares(), every);
                if every > 1 {
                    check(&scene, &grid.minor, 1);
                }
                found += 1;
            }
        }
    }
    assert!(found >= 3, "only {found} of 6 curled sheets found");
}

/// A photographed screen's pixels beat with the grid (moiré): right or
/// nothing.
#[test]
fn a_photographed_screen_is_counted_right_or_refused() {
    for (pitch, px_per_mm) in [(2.0, 3.0), (2.0, 4.0), (3.3, 4.0), (2.2, 3.0)] {
        let mut scene = Scene::plain(1600, 1200, px_per_mm, 2.0).realistic();
        scene.major_every = 5;
        scene.line_mm = 0.15;
        scene.major_line_mm = 0.4;
        scene.screen_px = Some(pitch);
        let (_, ink) = scene.render();
        if let Ok(grid) = find_grid(&ink, whole(&scene), &Hints::default()) {
            check(
                &scene,
                grid.squares(),
                grid.major.map_or(1, |m| i64::from(m.every)),
            );
        }
    }
}

/// Ruled paper (one family) under a lattice of dots is no grid of squares
/// unless the count holds.
#[test]
fn one_family_and_periodic_clutter_is_no_grid() {
    let mut scene = Scene::plain(1200, 900, 5.0, 5.0).realistic();
    scene.families = [false, true];
    for i in -12..=12 {
        for j in -9..=9 {
            scene
                .holes
                .push(([5.0 * f64::from(i) + 2.5, 5.0 * f64::from(j)], 0.5));
        }
    }
    let (_, ink) = scene.render();
    if let Ok(grid) = find_grid(&ink, whole(&scene), &Hints::default()) {
        check(
            &scene,
            grid.squares(),
            grid.major.map_or(1, |m| i64::from(m.every)),
        );
    }
}

#[test]
fn the_safety_gates_hold_their_lines() {
    // Three of four arms, or both of two at the edge.
    assert!(arms_enough(3, 4) && arms_enough(2, 2));
    assert!(!arms_enough(2, 4) && !arms_enough(2, 3));
    // A line found in 60% of its looks, off by under 0.15 spacing on average.
    assert!(line_is_held(10, 6, 0.8));
    assert!(!line_is_held(10, 5, 0.1));
    assert!(!line_is_held(10, 8, 1.3));
    assert!(supported(0.5) && !supported(0.45));
    assert!(fit_is_tight(0.08, 1.0) && !fit_is_tight(0.1, 1.0));
    assert!(looks_square(10.0, 12.5) && !looks_square(10.0, 13.0));
    assert!(on_crossing_distance(0.2) && !on_crossing_distance(0.3));
    assert!(enough_refined(15, 20) && !enough_refined(14, 20));
}

/// A survey with lines found as `found(f, line)` says, every look made.
fn survey_with(count: usize, found: impl Fn(usize, usize) -> bool) -> Survey {
    let mut survey = Survey {
        lines: [(0, count), (0, count)],
        looked: [vec![true; count * count], vec![true; count * count]],
        found: [0, 1].map(|f| (0..count * count).map(|i| found(f, i / count)).collect()),
        strength: [vec![None; count], vec![None; count]],
        miss: [vec![0.0; count * count], vec![0.0; count * count]],
        sums: Default::default(),
        rms_px: 0.0,
    };
    survey.tally();
    survey
}

#[test]
fn a_model_holds_only_when_most_of_its_lines_are_found() {
    assert!(survey_with(10, |_, _| true).holds(None));
    // Every fourth line found: a spacing a quarter of the real one.
    assert!(!survey_with(10, |_, line| line % 4 == 0).holds(None));
    // The heavy lines are what count when there are any.
    assert!(survey_with(10, |_, line| line % 4 == 0).holds(Some((4, [0, 0]))));
}

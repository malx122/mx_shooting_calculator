//! Marking: gestures, coordinates, undo, steps, rings, and every step
//! drawn on a desktop and a phone in both themes.

use ballistics_store::{Calibration, Group, Marks};
use eframe::egui::{self, Pos2, Rect, Vec2, pos2, vec2};
use groups_core::Point;

use super::geometry::{self, Bounds, Camera, Space};
use super::gesture::{self, Gesture, Release, Step as GestureStep, Under};
use super::history::History;
use super::scale::{ScaleDraft, ScaleMode, ScaleNote, ScaleState};
use super::*;
use crate::theme::{self, ThemeChoice};

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

fn near(a: Point, b: Point, tolerance: f64) -> bool {
    a.distance(b) <= tolerance
}

// --- Gestures ---

type G = Gesture<u8>;

#[test]
fn a_press_lifted_close_by_is_a_tap_where_it_lifts() {
    let mut press = G::new(pos2(100.0, 100.0), 0.0, Under::Nothing, false);
    assert_eq!(press.update(pos2(105.0, 104.0), 0.1), GestureStep::Nothing);
    assert_eq!(
        press.release(pos2(106.0, 104.0), 0.15),
        Release::Tap {
            at: pos2(106.0, 104.0),
            on: None
        }
    );
}

#[test]
fn a_press_that_moves_past_the_threshold_pans() {
    let mut press = G::new(pos2(100.0, 100.0), 0.0, Under::Nothing, false);
    assert_eq!(
        press.update(pos2(100.0 + gesture::MOVE_THRESHOLD - 0.5, 100.0), 0.05),
        GestureStep::Nothing
    );
    assert_eq!(
        press.update(pos2(110.0, 100.0), 0.1),
        GestureStep::Pan(vec2(10.0, 0.0)),
        "the pan catches up the threshold"
    );
    assert_eq!(
        press.update(pos2(115.0, 102.0), 0.15),
        GestureStep::Pan(vec2(5.0, 2.0))
    );
    assert!(!press.is_placing());
    assert_eq!(press.release(pos2(115.0, 102.0), 0.2), Release::Nothing);
}

#[test]
fn a_still_press_becomes_a_hold_that_places_where_it_lifts() {
    let mut press = G::new(pos2(50.0, 50.0), 1.0, Under::Nothing, false);
    assert!(
        press
            .hold_in(1.1)
            .is_some_and(|wait| close(wait, 0.3, 1e-9))
    );
    assert_eq!(
        press.update(pos2(51.0, 50.0), 1.05 + gesture::HOLD_SECONDS),
        GestureStep::Held(None)
    );
    // Once held, the finger steers the loupe instead of panning.
    assert_eq!(press.update(pos2(90.0, 80.0), 1.6), GestureStep::Nothing);
    assert!(press.is_placing());
    assert_eq!(
        press.release(pos2(90.0, 80.0), 1.7),
        Release::Place {
            at: pos2(90.0, 80.0)
        }
    );
}

#[test]
fn holding_a_mark_selects_it_and_dragging_then_moves_it() {
    let under = Under::Mark {
        mark: 3,
        drag_at_once: false,
    };
    let mut press = G::new(pos2(10.0, 10.0), 0.0, under, false);
    assert_eq!(
        press.update(pos2(10.0, 10.0), 0.5),
        GestureStep::Held(Some(3))
    );
    assert!(
        !press.is_placing(),
        "no loupe for placing while holding a mark"
    );
    assert_eq!(
        press.update(pos2(40.0, 10.0), 0.7),
        GestureStep::Move {
            mark: 3,
            offset: vec2(30.0, 0.0)
        }
    );
    assert_eq!(press.moving(), Some(3));
    assert_eq!(press.release(pos2(40.0, 10.0), 0.8), Release::Moved(3));

    let mut held = G::new(pos2(10.0, 10.0), 0.0, under, false);
    let _ = held.update(pos2(10.0, 10.0), 0.5);
    assert_eq!(held.release(pos2(11.0, 10.0), 0.6), Release::Selected(3));
}

#[test]
fn a_mark_not_yet_selected_pans_when_dragged_at_once() {
    let unselected = Under::Mark {
        mark: 1,
        drag_at_once: false,
    };
    let mut press = G::new(pos2(0.0, 0.0), 0.0, unselected, false);
    assert!(matches!(
        press.update(pos2(20.0, 0.0), 0.1),
        GestureStep::Pan(_)
    ));
    let selected = Under::Mark {
        mark: 1,
        drag_at_once: true,
    };
    let mut press = G::new(pos2(0.0, 0.0), 0.0, selected, false);
    assert_eq!(
        press.update(pos2(20.0, 0.0), 0.1),
        GestureStep::Move {
            mark: 1,
            offset: vec2(20.0, 0.0)
        }
    );
    // A tap on a mark reports it.
    let press = G::new(pos2(0.0, 0.0), 0.0, unselected, false);
    assert_eq!(
        press.release(pos2(1.0, 1.0), 0.1),
        Release::Tap {
            at: pos2(1.0, 1.0),
            on: Some(1)
        }
    );
}

#[test]
fn space_drags_and_second_fingers_neither_place_nor_move() {
    let mut press = G::new(pos2(0.0, 0.0), 0.0, Under::Nothing, true);
    assert!(!press.is_placing());
    assert_eq!(
        press.update(pos2(2.0, 0.0), 0.1),
        GestureStep::Pan(vec2(2.0, 0.0))
    );
    assert_eq!(press.release(pos2(2.0, 0.0), 0.2), Release::Nothing);

    let mut press = G::new(pos2(0.0, 0.0), 0.0, Under::Nothing, false);
    press.cancel();
    assert_eq!(press.update(pos2(30.0, 0.0), 0.1), GestureStep::Nothing);
    assert_eq!(press.release(pos2(30.0, 0.0), 0.2), Release::Nothing);
}

#[test]
fn double_taps_are_quick_and_close() {
    let first = Some((pos2(100.0, 100.0), 1.0));
    assert!(gesture::is_double_tap(first, pos2(105.0, 103.0), 1.25));
    assert!(!gesture::is_double_tap(first, pos2(105.0, 103.0), 1.4));
    assert!(!gesture::is_double_tap(first, pos2(130.0, 100.0), 1.1));
    assert!(!gesture::is_double_tap(None, pos2(100.0, 100.0), 1.1));
}

// --- Coordinates ---

/// A photo of a 200 × 300 mm sheet taken at an angle: its far (top) edge
/// is shorter.
fn tilted_sheet() -> groups_core::Calibration {
    groups_core::Calibration::four_corners(
        [
            Point::new(260.0, 120.0),
            Point::new(740.0, 120.0),
            Point::new(820.0, 860.0),
            Point::new(180.0, 860.0),
        ],
        200.0,
        300.0,
    )
    .expect("a sensible sheet")
}

fn straightened() -> Space {
    Space::Straightened {
        size_px: [1000.0, 1000.0],
        calibration: tilted_sheet(),
        rectangle_mm: [200.0, 300.0],
    }
}

#[test]
fn the_camera_maps_screen_and_display_both_ways() {
    let camera = Camera {
        centre: Point::new(500.0, 400.0),
        scale: 2.0,
        screen_centre: pos2(300.0, 200.0),
    };
    assert_eq!(
        camera.to_screen(Point::new(500.0, 400.0)),
        pos2(300.0, 200.0)
    );
    assert_eq!(
        camera.to_screen(Point::new(510.0, 395.0)),
        pos2(320.0, 190.0)
    );
    let back = camera.to_display(pos2(320.0, 190.0));
    assert!(near(back, Point::new(510.0, 395.0), 1e-9));

    let zoomed = camera.zoomed(3.0, pos2(100.0, 50.0));
    let before = camera.to_display(pos2(100.0, 50.0));
    let after = zoomed.to_display(pos2(100.0, 50.0));
    assert!(near(before, after, 1e-9), "what is under the fingers stays");
    assert!(close(zoomed.scale, 6.0, 1e-12));

    let panned = camera.panned(vec2(20.0, -10.0));
    assert!(near(panned.centre, Point::new(490.0, 405.0), 1e-9));
}

#[test]
fn a_grid_turns_y_up_into_y_down() {
    let grid = Space::Grid {
        size_mm: [300.0, 200.0],
    };
    assert_eq!(grid.to_display([10.0, 20.0]), Some(Point::new(10.0, -20.0)));
    assert_eq!(grid.to_mark(Point::new(10.0, -20.0)), Some([10.0, 20.0]));
    let extent = grid.extent();
    assert_eq!(extent.min, Point::new(-150.0, -100.0));
    assert_eq!(extent.max, Point::new(150.0, 100.0));
}

#[test]
fn a_straightened_photo_shows_the_sheet_square_on() {
    let space = straightened();
    // The corners land on the rectangle's corners, y down from its top.
    for (px, display) in [
        ([260.0, 120.0], Point::new(0.0, -300.0)),
        ([740.0, 120.0], Point::new(200.0, -300.0)),
        ([820.0, 860.0], Point::new(200.0, 0.0)),
        ([180.0, 860.0], Point::new(0.0, 0.0)),
    ] {
        let shown = space.to_display(px).expect("on the sheet");
        assert!(near(shown, display, 1e-6), "{px:?} → {shown:?}");
        let back = space.to_mark(display).expect("back to the photo");
        assert!(near(
            Point::new(back[0], back[1]),
            Point::new(px[0], px[1]),
            1e-6
        ));
    }
    // Screen → display → photo → display → screen.
    let camera = Camera {
        centre: Point::new(100.0, -150.0),
        scale: 1.7,
        screen_centre: pos2(400.0, 300.0),
    };
    let screen = pos2(431.0, 262.0);
    let px = space.to_mark(camera.to_display(screen)).unwrap();
    let again = camera.to_screen(space.to_display(px).unwrap());
    assert!((again - screen).length() < 1e-3);
    // The extent holds the sheet and stays within the photo's view.
    let extent = space.extent();
    assert!(extent.contains(Point::new(0.0, -300.0)) && extent.contains(Point::new(200.0, 0.0)));
}

#[test]
fn the_straightened_mesh_samples_the_photo_where_the_space_says() {
    let space = straightened();
    let camera = Camera {
        centre: Point::new(100.0, -150.0),
        scale: 2.0,
        screen_centre: pos2(400.0, 400.0),
    };
    let region = Bounds::new(Point::new(-20.0, -320.0), Point::new(220.0, 20.0));
    let mesh = paint::photo_mesh(egui::TextureId::default(), &space, &region, &camera, 16);
    assert!(mesh.indices.len() >= 3 * 2 * 16 * 16, "every cell drawn");
    for vertex in &mesh.vertices {
        let display = camera.to_display(vertex.pos);
        let px = space.to_mark(display).unwrap();
        assert!(close(vertex.uv.x as f64, px[0] / 1000.0, 1e-4));
        assert!(close(vertex.uv.y as f64, px[1] / 1000.0, 1e-4));
        assert!((0.0..=1.0).contains(&vertex.uv.x) && (0.0..=1.0).contains(&vertex.uv.y));
    }
    // A region reaching past the photo is cut at its edge.
    let wide = Bounds::new(Point::new(-400.0, -900.0), Point::new(600.0, 400.0));
    let mesh = paint::photo_mesh(egui::TextureId::default(), &space, &wide, &camera, 16);
    assert!(!mesh.indices.is_empty());
    for vertex in &mesh.vertices {
        assert!(
            (-1e-4..=1.0 + 1e-4).contains(&vertex.uv.x),
            "{:?}",
            vertex.uv
        );
        assert!(
            (-1e-4..=1.0 + 1e-4).contains(&vertex.uv.y),
            "{:?}",
            vertex.uv
        );
    }
    // The photo as it is is a single quad.
    let photo = Space::Photo {
        size_px: [640.0, 480.0],
    };
    let mesh = paint::photo_mesh(
        egui::TextureId::default(),
        &photo,
        &photo.extent(),
        &camera,
        16,
    );
    assert_eq!(mesh.vertices.len(), 4);
}

#[test]
fn views_stay_on_the_content_and_within_the_zoom_limits() {
    let extent = Bounds::new(Point::ORIGIN, Point::new(640.0, 480.0));
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let fit = geometry::fit_scale(&extent, rect);
    assert!(close(fit, 0.94 * 0.625, 1e-9));
    let lost = Camera {
        centre: Point::new(5000.0, -300.0),
        scale: fit * 0.01,
        screen_centre: rect.center(),
    };
    let kept = geometry::clamped(lost, &extent, fit, 12.0);
    assert_eq!(kept.centre, Point::new(640.0, 0.0));
    assert!(close(kept.scale, fit * geometry::MIN_ZOOM, 1e-12));
    let close_up = Camera {
        scale: 500.0,
        ..kept
    };
    assert!(close(
        geometry::clamped(close_up, &extent, fit, 12.0).scale,
        12.0,
        1e-12
    ));
}

#[test]
fn the_loupe_sits_above_the_finger_or_beside_it_near_the_top() {
    let canvas = Rect::from_min_size(pos2(0.0, 100.0), vec2(390.0, 600.0));
    let above = geometry::loupe_rect(pos2(200.0, 500.0), canvas, 120.0, true);
    assert!(above.bottom() < 500.0 - 40.0, "clear of the finger");
    assert!(close(above.center().x as f64, 200.0, 1e-3));
    let near_top = geometry::loupe_rect(pos2(100.0, 150.0), canvas, 120.0, true);
    assert!(near_top.left() > 100.0, "beside, to the right");
    assert!(canvas.contains_rect(near_top));
    let top_right = geometry::loupe_rect(pos2(360.0, 150.0), canvas, 120.0, true);
    assert!(top_right.right() < 360.0, "beside, to the left");
    assert!(canvas.contains_rect(top_right));
}

#[test]
fn a_nudge_moves_one_photo_pixel_the_way_the_screen_shows() {
    let space = straightened();
    let start = [500.0, 500.0];
    let moved = space.nudged(start, vec2(1.0, 0.0), 1.0).unwrap();
    let step = (moved[0] - start[0]).hypot(moved[1] - start[1]);
    assert!(close(step, 1.0, 1e-3), "{step}");
    let (before, after) = (
        space.to_display(start).unwrap(),
        space.to_display(moved).unwrap(),
    );
    assert!(
        after.x > before.x && close(after.y, before.y, 1e-6),
        "straight right on screen"
    );

    let grid = Space::Grid {
        size_mm: [100.0, 100.0],
    };
    let up = grid.nudged([0.0, 0.0], vec2(0.0, -1.0), 0.5).unwrap();
    assert!(close(up[0], 0.0, 1e-9) && close(up[1], 0.5, 1e-9), "{up:?}");
}

// --- Rings, labels and icons ---

#[test]
fn numbers_sit_outside_their_rings_and_small_rings_are_thin() {
    let centre = pos2(100.0, 100.0);
    for away in [
        vec2(1.0, 0.0),
        vec2(-1.0, 1.0),
        vec2(0.0, -3.0),
        vec2(0.3, 0.9),
        Vec2::ZERO,
    ] {
        let label = paint::label_rect(centre, 20.0, away, vec2(22.0, 15.0));
        let nearest = pos2(
            centre.x.clamp(label.left(), label.right()),
            centre.y.clamp(label.top(), label.bottom()),
        );
        assert!((nearest - centre).length() > 20.0, "{away:?} → {label:?}");
        if away != Vec2::ZERO {
            assert!((label.center() - centre).dot(away) > 0.0, "on the far side");
        }
    }
    let (small_outline, small_line) = paint::ring_widths(4.0, false);
    let (outline, line) = paint::ring_widths(20.0, false);
    assert!(small_line < line && small_outline < outline);
}

#[test]
fn undo_turns_back_to_the_left_and_redo_forward_to_the_right() {
    for redo in [false, true] {
        let (arc, [barb, tip, other]) = paint::arrow(redo);
        let side = if redo { 1.0 } else { -1.0 };
        assert!(tip.x * side > 0.4, "the head is on the {side} side");
        assert!(arc[0].x * side < -0.4, "the arc starts on the other side");
        assert!(
            (*arc.last().unwrap() - tip).length() < 1e-5,
            "it ends at the head"
        );
        assert!(arc.iter().any(|p| p.y < -0.3), "over the top");
        assert!(barb.y < tip.y && other.y < tip.y, "the head points down");
    }
}

#[test]
fn phone_controls_are_big_enough_to_touch() {
    let canvas = Rect::from_min_size(pos2(0.0, 100.0), vec2(390.0, 600.0));
    for (rect, _) in chrome::zoom_buttons(canvas, true) {
        assert!(rect.width() >= crate::mobile::TOUCH && rect.height() >= crate::mobile::TOUCH);
    }
}

/// The phone's header and toolbar fit 390 points in every language.
#[test]
fn the_phone_toolbars_fit_in_finnish_and_swedish() {
    use crate::i18n::Language;
    for language in [Language::Finnish, Language::Swedish, Language::English] {
        crate::i18n::set(language);
        let mut run = Run::new(ThemeChoice::Dark, 390.0);
        let image = photo(&run.context);
        for (name, mut marking) in every_step() {
            for _ in 0..3 {
                run.frame(&mut marking, Some(&image), Vec::new());
            }
            for row in ["marking-header", "marking-toolbar"] {
                let rect: Rect = run
                    .context
                    .data(|data| data.get_temp(egui::Id::unique(row)))
                    .unwrap_or_else(|| panic!("{name}: {row} is drawn"));
                assert!(
                    rect.left() >= 0.0 && rect.right() <= 390.0,
                    "{language:?}, {name}: {row} spans {rect:?}"
                );
                if row == "marking-header" {
                    assert!(
                        rect.height() >= crate::mobile::TOUCH - 0.5,
                        "{name}: {rect:?}"
                    );
                }
            }
        }
    }
    crate::i18n::set(Language::English);
}

// --- Rings ---

#[test]
fn rings_are_bullet_sized_once_the_calibre_and_scale_are_known() {
    let two_points = groups_core::Calibration::two_points(
        Point::new(100.0, 100.0),
        Point::new(200.0, 100.0),
        10.0,
    )
    .unwrap();
    let photo = Space::Photo {
        size_px: [640.0, 480.0],
    };
    // 10 px per mm: a 6.5 mm bullet is 65 px across.
    let r = ring_radius(&photo, Some(&two_points), Some(6.5), [150.0, 150.0]).unwrap();
    assert!(close(r, 32.5, 1e-9), "{r}");
    assert_eq!(ring_radius(&photo, None, Some(6.5), [150.0, 150.0]), None);
    assert_eq!(
        ring_radius(&photo, Some(&two_points), None, [150.0, 150.0]),
        None
    );
    let straight = straightened();
    let r = ring_radius(&straight, None, Some(7.82), [500.0, 500.0]).unwrap();
    assert!(close(r, 3.91, 1e-9), "millimetres on a straightened photo");
    let grid = Space::Grid {
        size_mm: [100.0, 100.0],
    };
    assert_eq!(ring_radius(&grid, None, Some(5.6), [0.0, 0.0]), Some(2.8));
}

// --- Undo ---

#[test]
fn the_history_undoes_and_redoes_in_order() {
    let mut history = History::default();
    history.record(0);
    history.record(1);
    assert_eq!(history.undo(2), Some(1));
    assert_eq!(history.undo(1), Some(0));
    assert_eq!(history.undo(0), None);
    assert_eq!(history.redo(0), Some(1));
    assert!(history.can_redo());
    history.record(1);
    assert!(!history.can_redo(), "a new change ends the redos");
    assert_eq!(history.forget_last(), Some(1));
    assert_eq!(history.undo_depth(), 1);
}

#[test]
fn undo_covers_shots_flags_aim_and_the_scale() {
    let mut marking = Marking::new_photo("photo".into(), None);
    marking.scale.mode = ScaleMode::TwoPoints;
    marking.scale.place([100.0, 100.0]);
    marking.commit();
    marking.scale.place([400.0, 100.0]);
    marking.scale.distance_mm = 50.0;
    marking.commit();
    assert!(matches!(marking.scale.state(), ScaleState::Ready(_)));
    marking.go_to(Step::Aim);
    marking.set_aim(Some([250.0, 250.0]));
    marking.commit();
    marking.go_to(Step::Shots);
    marking.place([260.0, 240.0]);
    marking.commit();
    marking.apply(Command::ColdBore, None);
    marking.commit();
    assert_eq!(marking.shot(0), Some(([260.0, 240.0], true, false)));

    marking.apply(Command::Undo, None);
    assert_eq!(marking.shot(0), Some(([260.0, 240.0], false, false)));
    marking.apply(Command::Undo, None);
    assert_eq!(marking.shot_count(), 0);
    marking.apply(Command::Undo, None);
    assert_eq!(marking.aim(), None);
    marking.apply(Command::Undo, None);
    assert_eq!(marking.scale.points[1], None, "the scale's second point");
    marking.apply(Command::Redo, None);
    marking.apply(Command::Redo, None);
    marking.apply(Command::Redo, None);
    assert_eq!(marking.aim(), Some([250.0, 250.0]));
    assert_eq!(marking.shot_count(), 1);
    marking.refresh_calibration();
    assert!(marking.calibration().is_some());
    assert!(marking.unsaved());
}

// --- Steps ---

#[test]
fn a_photo_steps_from_scale_through_aim_to_shots() {
    let mut marking = Marking::new_photo("photo".into(), None);
    assert_eq!((marking.step, marking.depth()), (Step::Scale, 1));
    assert!(!marking.back(), "the first step: the caller closes");
    assert!(!marking.unsaved());
    marking.apply(Command::Next, None);
    assert_eq!(marking.step, Step::Scale, "no scale yet");
    marking.apply(Command::Skip, None);
    assert_eq!((marking.step, marking.depth()), (Step::Aim, 2));
    assert!(marking.back());
    assert_eq!(marking.step, Step::Scale);
    for corner in [
        [100.0, 100.0],
        [500.0, 100.0],
        [500.0, 600.0],
        [100.0, 600.0],
    ] {
        marking.scale.place(corner);
    }
    marking.apply(Command::Next, None);
    assert_eq!(marking.step, Step::Aim);
    marking.set_aim(Some([300.0, 300.0]));
    marking.apply(Command::Next, None);
    assert_eq!((marking.step, marking.depth()), (Step::Shots, 3));
    assert!(marking.zoom_to_aim, "zooms in on the aim point");
    marking.apply(Command::GoTo(Step::Scale), None);
    assert_eq!(marking.step, Step::Scale);
    marking.apply(Command::GoTo(Step::Shots), None);
    assert_eq!(marking.step, Step::Scale, "forward only through Next");
}

#[test]
fn a_scaled_photo_starts_at_the_aim_and_a_grid_has_no_scale_step() {
    let calibration = Calibration::TwoPoint {
        a_px: [10.0, 10.0],
        b_px: [210.0, 10.0],
        distance_mm: 100.0,
    };
    let marking = Marking::new_photo("photo".into(), Some(calibration));
    assert_eq!((marking.step, marking.depth()), (Step::Aim, 2));
    assert_eq!(marking.calibration(), Some(&calibration));

    let mut grid = Marking::new_grid(300.0, 300.0);
    assert_eq!((grid.step, grid.depth()), (Step::Aim, 1));
    grid.apply(Command::NoAim, None);
    assert_eq!((grid.step, grid.depth()), (Step::Shots, 2));
    assert!(grid.back());
    assert!(!grid.back());
}

#[test]
fn editing_opens_at_the_shots_in_the_saved_framing() {
    let mut marking = Marking::new_grid(200.0, 200.0);
    marking.add_shot([1.0, 2.0]);
    let group = Group {
        marks: marking.marks().clone(),
        ..group_template()
    };
    let view = GroupView {
        zoom: 2.0,
        centre: [5.0, 5.0],
    };
    let edit = Marking::edit(&group, None, Some(view));
    assert_eq!((edit.step, edit.depth()), (Step::Shots, 2));
    assert_eq!(edit.view(), Some(view));
    assert!(!edit.unsaved());

    // A photo group opens at its shots when the photo has a scale, and at
    // the scale step when it has none.
    let photo = Group {
        marks: Marks::Photo(ballistics_store::PhotoMarks {
            photo_id: "photo".into(),
            aim_px: None,
            shots: Vec::new(),
        }),
        ..group_template()
    };
    let scale = Calibration::TwoPoint {
        a_px: [10.0, 10.0],
        b_px: [210.0, 10.0],
        distance_mm: 100.0,
    };
    let scaled = Marking::edit(&photo, Some(&scale), None);
    assert_eq!((scaled.step, scaled.depth()), (Step::Shots, 3));
    let unscaled = Marking::edit(&photo, None, None);
    assert_eq!((unscaled.step, unscaled.depth()), (Step::Scale, 1));
    assert!(!unscaled.unsaved());
}

fn group_template() -> Group {
    serde_json::from_value(serde_json::json!({
        "id": "g1",
        "taken_at": 0,
        "marks": { "on": "grid", "width_mm": 200.0, "height_mm": 200.0 }
    }))
    .unwrap_or_else(|error| panic!("a minimal group reads: {error}"))
}

// --- Scale ---

#[test]
fn four_corners_keep_their_sorted_order_and_say_when_they_look_turned() {
    let mut draft = ScaleDraft {
        mode: ScaleMode::FourCorners,
        ..ScaleDraft::default()
    };
    for corner in [
        [900.0, 700.0],
        [100.0, 100.0],
        [100.0, 700.0],
        [900.0, 100.0],
    ] {
        draft.place(corner);
    }
    // Landscape on the photo, but A4 portrait given.
    let ScaleState::Ready(calibration) = draft.state() else {
        panic!("four corners scale the photo");
    };
    assert!(draft.notes(&calibration).contains(&ScaleNote::LooksTurned));
    draft.swap_sides();
    let ScaleState::Ready(calibration) = draft.state() else {
        panic!("still fine");
    };
    assert!(!draft.notes(&calibration).contains(&ScaleNote::LooksTurned));
    let Some(Calibration::FourCorner {
        corners_px,
        width_mm,
        height_mm,
        ..
    }) = draft.to_store()
    else {
        panic!("a four-corner scale");
    };
    assert_eq!(
        corners_px,
        [
            [100.0, 100.0],
            [900.0, 100.0],
            [900.0, 700.0],
            [100.0, 700.0]
        ]
    );
    assert_eq!((width_mm, height_mm), (297.0, 210.0));
}

#[test]
fn grid_squares_scale_and_straighten_from_four_crossings() {
    let mut draft = ScaleDraft::default();
    assert_eq!(
        draft.mode,
        ScaleMode::GridSquares,
        "grid squares come first"
    );
    draft.square_mm = 25.4;
    draft.squares_across = 4;
    draft.squares_down = 2;
    for crossing in [
        [500.0, 300.0],
        [100.0, 100.0],
        [100.0, 300.0],
        [500.0, 100.0],
    ] {
        draft.place(crossing);
    }
    let ScaleState::Ready(calibration) = draft.state() else {
        panic!("four crossings scale the photo");
    };
    assert!(!draft.notes(&calibration).contains(&ScaleNote::LooksTurned));
    // 4 squares of 1 inch over 400 px across, 2 over 200 px down.
    let mm_per_px = calibration
        .mm_per_px_at(groups_core::Point::new(300.0, 200.0))
        .expect("inside the rectangle");
    assert!((mm_per_px - 101.6 / 400.0).abs() < 1e-6, "{mm_per_px}");
    let stored = draft.to_store();
    let Some(Calibration::FourCorner {
        width_mm,
        height_mm,
        ..
    }) = stored
    else {
        panic!("kept as a four-corner scale");
    };
    assert!((width_mm - 101.6).abs() < 1e-9 && (height_mm - 50.8).abs() < 1e-9);
    // Reopened, it is the grid again, counts and all.
    let again = ScaleDraft::from_store(stored.as_ref());
    assert_eq!(again.mode, ScaleMode::GridSquares);
    assert_eq!((again.squares_across, again.squares_down), (4, 2));
    assert_eq!(again.square_mm, 25.4);

    draft.swap_sides();
    assert_eq!((draft.squares_across, draft.squares_down), (2, 4));
    let ScaleState::Ready(calibration) = draft.state() else {
        panic!("still a scale");
    };
    assert!(draft.notes(&calibration).contains(&ScaleNote::LooksTurned));
}

#[test]
fn a_sheet_reopens_as_a_sheet() {
    let rectangle = |width_mm: f64, height_mm: f64| Calibration::FourCorner {
        corners_px: [[0.0, 0.0], [400.0, 0.0], [400.0, 560.0], [0.0, 560.0]],
        width_mm,
        height_mm,
        ordered: false,
    };
    // Letter is a whole 17 × 22 half-inch squares, but it is a sheet.
    for (width, height) in [(210.0, 297.0), (215.9, 279.4), (420.0, 297.0)] {
        let draft = ScaleDraft::from_store(Some(&rectangle(width, height)));
        assert_eq!(draft.mode, ScaleMode::FourCorners, "{width} × {height}");
        assert_eq!((draft.width_mm, draft.height_mm), (width, height));
    }
    let draft = ScaleDraft::from_store(Some(&rectangle(100.0, 140.0)));
    assert_eq!(draft.mode, ScaleMode::GridSquares);
    assert_eq!(
        (draft.square_mm, draft.squares_across, draft.squares_down),
        (10.0, 10, 14)
    );
}

#[test]
fn close_points_are_refused_or_warned_about() {
    let mut draft = ScaleDraft {
        mode: ScaleMode::TwoPoints,
        distance_mm: 10.0,
        ..ScaleDraft::default()
    };
    draft.place([100.0, 100.0]);
    assert_eq!(draft.state(), ScaleState::ToTap(1));
    draft.place([110.0, 100.0]);
    assert_eq!(
        draft.state(),
        ScaleState::Refused(groups_core::CalibrationError::TooClose)
    );
    draft.set_handle(1, [150.0, 100.0]);
    let ScaleState::Ready(calibration) = draft.state() else {
        panic!("50 px apart is enough");
    };
    let notes = draft.notes(&calibration);
    assert!(
        notes
            .iter()
            .any(|note| matches!(note, ScaleNote::Uncertain(f) if *f > 0.02)),
        "{notes:?}"
    );
    let text = scale::note_text(ScaleMode::TwoPoints, notes[0]);
    assert!(text.contains('%'), "{text}");
}

// --- Live statistics ---

#[test]
fn the_strip_counts_shots_and_sizes_them_from_the_second() {
    let mut marking = Marking::new_grid(200.0, 200.0);
    let units = crate::units::UnitPrefs::metric();
    let context = MarkingContext {
        photo: None,
        calibre_mm: Some(6.5),
        distance_m: Some(100.0),
    };
    marking.add_shot([0.0, 0.0]);
    let live = marking.live_view(&units, &context);
    assert_eq!(live.text, "1 shot");
    assert!(live.centre_mm.is_none());
    marking.add_shot([10.0, 0.0]);
    marking.add_shot([0.0, 10.0]);
    marking.toggle_shot(2, true);
    let live = marking.live_view(&units, &context);
    assert!(
        live.text.starts_with("2 shots (+1 left out) · ES 0.10 mil"),
        "{}",
        live.text
    );
    assert!(near(live.centre_mm.unwrap(), Point::new(5.0, 0.0), 1e-9));
    assert!(marking.live.is_cached_for(marking.marks()));
    let no_distance = MarkingContext {
        distance_m: None,
        ..context
    };
    let live = marking.live_view(&units, &no_distance);
    assert!(live.text.contains("ES 10.0 mm"), "{}", live.text);
}

// --- Drawing and input, headless ---

fn photo(context: &egui::Context) -> PhotoImage {
    let pixels = (0..64 * 48)
        .map(|i| {
            let (x, y) = (i % 64, i / 64);
            if (x / 8 + y / 8) % 2 == 0 {
                egui::Color32::from_gray(235)
            } else {
                egui::Color32::from_gray(40)
            }
        })
        .collect();
    PhotoImage {
        id: "photo".into(),
        texture: context.load_texture(
            "marking-test-photo",
            egui::ColorImage::new([64, 48], pixels),
            egui::TextureOptions::LINEAR,
        ),
        size_px: [1280, 960],
        ink: None,
    }
}

struct Run {
    context: egui::Context,
    width: f32,
    time: f64,
    calibre_mm: Option<f64>,
}

impl Run {
    fn new(choice: ThemeChoice, width: f32) -> Run {
        let context = egui::Context::default();
        theme::apply(&context);
        theme::choose(&context, choice);
        Run {
            context,
            width,
            time: 10.0,
            calibre_mm: Some(6.5),
        }
    }

    fn frame(
        &mut self,
        marking: &mut Marking,
        photo: Option<&PhotoImage>,
        events: Vec<egui::Event>,
    ) -> MarkingOutcome {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, 844.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let context = MarkingContext {
            photo,
            calibre_mm: self.calibre_mm,
            distance_m: Some(100.0),
        };
        let mut outcome = MarkingOutcome::Working;
        let mut output = self.context.clone().run_ui(input, |ui| {
            outcome = marking.show(ui, &context);
        });
        output.textures_delta.clear();
        outcome
    }

    fn press(&mut self, marking: &mut Marking, photo: Option<&PhotoImage>, at: Pos2) {
        self.frame(marking, photo, vec![egui::Event::PointerMoved(at)]);
        self.frame(
            marking,
            photo,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    fn release(&mut self, marking: &mut Marking, photo: Option<&PhotoImage>, at: Pos2) {
        self.frame(
            marking,
            photo,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }

    fn tap(&mut self, marking: &mut Marking, photo: Option<&PhotoImage>, at: Pos2) {
        self.press(marking, photo, at);
        self.release(marking, photo, at);
        // Far enough apart not to make a double tap.
        self.time += 0.5;
    }
}

/// A marking at each step, with something on it.
fn every_step() -> Vec<(&'static str, Marking)> {
    let mut two = Marking::new_photo("photo".into(), None);
    two.scale.mode = ScaleMode::TwoPoints;
    two.scale.place([300.0, 300.0]);
    two.scale.place([900.0, 320.0]);
    two.scale.distance_mm = 100.0;
    two.selected = Some(Handle::Scale(1));

    let mut four = Marking::new_photo("photo".into(), None);
    for corner in [
        [200.0, 150.0],
        [1100.0, 180.0],
        [1180.0, 900.0],
        [120.0, 880.0],
    ] {
        four.scale.place(corner);
    }

    let mut aim = Marking::new_photo("photo".into(), None);
    for corner in [
        [200.0, 150.0],
        [1100.0, 180.0],
        [1180.0, 900.0],
        [120.0, 880.0],
    ] {
        aim.scale.place(corner);
    }
    aim.go_to(Step::Aim);
    aim.set_aim(Some([640.0, 500.0]));
    aim.selected = Some(Handle::Aim);

    let mut shots = Marking::new_photo(
        "photo".into(),
        Some(Calibration::TwoPoint {
            a_px: [100.0, 100.0],
            b_px: [600.0, 100.0],
            distance_mm: 100.0,
        }),
    );
    shots.set_aim(Some([640.0, 480.0]));
    shots.go_to(Step::Shots);
    for (i, at) in [
        [650.0, 470.0],
        [630.0, 500.0],
        [662.0, 488.0],
        [700.0, 420.0],
    ]
    .into_iter()
    .enumerate()
    {
        shots.add_shot(at);
        if i == 0 {
            shots.toggle_shot(0, true);
        }
        if i == 3 {
            shots.toggle_shot(3, false);
        }
    }
    shots.selected = Some(Handle::Shot(2));

    let mut grid = Marking::new_grid(300.0, 300.0);
    grid.set_aim(Some([0.0, 0.0]));
    grid.go_to(Step::Shots);
    grid.add_shot([5.0, 8.0]);
    grid.add_shot([-6.0, 2.0]);
    grid.add_shot([1.0, -9.0]);

    let mut suggested = Marking::new_photo(
        "photo".into(),
        Some(Calibration::TwoPoint {
            a_px: [100.0, 100.0],
            b_px: [600.0, 100.0],
            distance_mm: 100.0,
        }),
    );
    suggested.set_aim(Some([640.0, 480.0]));
    suggested.go_to(Step::Shots);
    suggested.add_shot([650.0, 470.0]);
    let ring = |at: [f64; 2], ticked: bool, blob: Option<u32>| holes::Suggestion {
        at,
        radius: 16.0,
        ticked,
        near_marked: !ticked,
        score: 0.8,
        blob,
        through: false,
    };
    suggested.holes = holes::Search::Shown(vec![
        ring([651.0, 471.0], false, None),
        ring([600.0, 520.0], true, None),
        ring([700.0, 430.0], true, Some(0)),
        ring([728.0, 438.0], true, Some(0)),
    ]);

    vec![
        ("two points", two),
        ("four corners", four),
        ("aim on a straightened photo", aim),
        ("shots", shots),
        ("grid", grid),
        ("hole suggestions", suggested),
    ]
}

#[test]
fn every_step_draws_in_both_themes_on_a_desktop_and_a_phone() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Sun] {
        for width in [1200.0, 390.0] {
            let mut run = Run::new(choice, width);
            let image = photo(&run.context);
            for (name, mut marking) in every_step() {
                for _ in 0..3 {
                    let outcome = run.frame(&mut marking, Some(&image), Vec::new());
                    assert_eq!(outcome, MarkingOutcome::Working, "{name}");
                }
                assert!(marking.view().is_some(), "{name}: the framing is kept");
                let view = marking.view().unwrap();
                assert!(view.validate().is_ok(), "{name}: {view:?}");
                // With a finger down, the loupe shows too.
                run.press(&mut marking, Some(&image), pos2(width / 2.0, 500.0));
                run.frame(&mut marking, Some(&image), Vec::new());
                assert!(marking.press.is_some(), "{name}: pressed");
                run.release(&mut marking, Some(&image), pos2(width / 2.0, 500.0));
            }
            // A photo still loading.
            let mut loading = Marking::new_photo("photo".into(), None);
            run.frame(&mut loading, None, Vec::new());
            assert_eq!(
                theme::palette(&run.context).dark,
                choice == ThemeChoice::Dark
            );
        }
    }
}

#[test]
fn taps_place_shots_drags_pan_and_a_held_shot_moves() {
    for width in [1200.0, 390.0] {
        let mut run = Run::new(ThemeChoice::Dark, width);
        let image = photo(&run.context);
        let mut marking = Marking::new_photo(
            "photo".into(),
            Some(Calibration::TwoPoint {
                a_px: [100.0, 100.0],
                b_px: [600.0, 100.0],
                distance_mm: 100.0,
            }),
        );
        marking.go_to(Step::Shots);
        run.frame(&mut marking, Some(&image), Vec::new());
        let middle = pos2(width / 2.0, 450.0);
        let other = middle + vec2(60.0, 40.0);

        run.tap(&mut marking, Some(&image), middle);
        assert_eq!(marking.shot_count(), 1, "a tap places a shot");
        assert_eq!(marking.selected, Some(Handle::Shot(0)));
        run.tap(&mut marking, Some(&image), other);
        assert_eq!(marking.shot_count(), 2);

        // A drag that starts off the shots pans and places nothing.
        let before = marking.view().unwrap();
        let start = middle + vec2(-80.0, 80.0);
        run.press(&mut marking, Some(&image), start);
        for step in 1..=4 {
            let at = start + vec2(10.0 * step as f32, 0.0);
            run.frame(
                &mut marking,
                Some(&image),
                vec![egui::Event::PointerMoved(at)],
            );
        }
        run.release(&mut marking, Some(&image), start + vec2(40.0, 0.0));
        assert_eq!(marking.shot_count(), 2, "a drag places nothing");
        assert_ne!(marking.view().unwrap().centre, before.centre, "it panned");

        // Back where it was, so the first shot is under `middle` again.
        let start = middle + vec2(-40.0, 80.0);
        run.press(&mut marking, Some(&image), start);
        for step in 1..=4 {
            let at = start - vec2(10.0 * step as f32, 0.0);
            run.frame(
                &mut marking,
                Some(&image),
                vec![egui::Event::PointerMoved(at)],
            );
        }
        run.release(&mut marking, Some(&image), start - vec2(40.0, 0.0));
        run.time += 0.5;

        // Hold the first shot: it is selected, then dragged.
        let (shot_before, ..) = marking.shot(0).unwrap();
        run.press(&mut marking, Some(&image), middle);
        for _ in 0..10 {
            run.frame(&mut marking, Some(&image), Vec::new());
        }
        assert_eq!(marking.selected, Some(Handle::Shot(0)), "held");
        for step in 1..=3 {
            let at = middle + vec2(0.0, 10.0 * step as f32);
            run.frame(
                &mut marking,
                Some(&image),
                vec![egui::Event::PointerMoved(at)],
            );
        }
        run.release(&mut marking, Some(&image), middle + vec2(0.0, 30.0));
        let (shot_after, ..) = marking.shot(0).unwrap();
        assert!(shot_after[1] > shot_before[1] + 1.0, "moved down");
        assert_eq!(marking.shot_count(), 2, "a hold and drag places nothing");
        run.frame(&mut marking, Some(&image), Vec::new());

        marking.apply(Command::Undo, None);
        assert_eq!(
            marking.shot(0).unwrap().0,
            shot_before,
            "one undo for the move"
        );

        // A double tap fits the photo without leaving a shot behind.
        run.press(&mut marking, Some(&image), other + vec2(-150.0, 30.0));
        run.release(&mut marking, Some(&image), other + vec2(-150.0, 30.0));
        assert_eq!(marking.shot_count(), 3);
        run.press(&mut marking, Some(&image), other + vec2(-150.0, 30.0));
        run.release(&mut marking, Some(&image), other + vec2(-150.0, 30.0));
        assert_eq!(
            marking.shot_count(),
            2,
            "the double tap's first tap is taken back"
        );
        run.frame(&mut marking, Some(&image), Vec::new());
        assert!(close(marking.view().unwrap().zoom, 1.0, 1e-9), "fitted");
    }
}

#[test]
fn keys_undo_nudge_flag_and_delete() {
    let mut run = Run::new(ThemeChoice::Sun, 1200.0);
    let mut marking = Marking::new_grid(300.0, 300.0);
    marking.go_to(Step::Shots);
    marking.add_shot([0.0, 0.0]);
    marking.selected = Some(Handle::Shot(0));
    run.frame(&mut marking, None, Vec::new());
    let key = |key: egui::Key, modifiers: egui::Modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
    );
    assert_eq!(marking.shot(0).unwrap().0, [0.5, 0.0]);
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::ArrowUp, egui::Modifiers::SHIFT)],
    );
    assert_eq!(marking.shot(0).unwrap().0, [0.5, 5.0]);
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::C, egui::Modifiers::NONE)],
    );
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::F, egui::Modifiers::NONE)],
    );
    assert_eq!(marking.shot(0), Some(([0.5, 5.0], true, true)));
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
    );
    assert_eq!(marking.shot(0), Some(([0.5, 5.0], true, false)));
    run.frame(
        &mut marking,
        None,
        vec![key(
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )],
    );
    assert_eq!(marking.shot(0), Some(([0.5, 5.0], true, true)));
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
    );
    assert_eq!(marking.shot_count(), 0);
    run.frame(
        &mut marking,
        None,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert_eq!(marking.selected, None);
}

#[test]
fn done_finishes_back_steps_and_cancel_cancels() {
    let mut marking = Marking::new_grid(300.0, 300.0);
    marking.go_to(Step::Shots);
    marking.add_shot([3.0, 4.0]);
    assert_eq!(
        marking.run_command(Command::Done, None),
        Some(MarkingOutcome::Finished)
    );
    assert!(matches!(marking.marks(), Marks::Grid(grid) if grid.shots.len() == 1));
    assert_eq!(marking.run_command(Command::Back, None), None);
    assert_eq!(marking.step, Step::Aim);
    // With a shot marked, closing asks first.
    assert_eq!(marking.run_command(Command::Back, None), None);
    assert!(
        marking.discard_asked.is_some(),
        "back from the first step asks"
    );
    assert_eq!(marking.answer_discard(false, true), None, "keep editing");
    assert!(marking.discard_asked.is_none());
    assert_eq!(marking.run_command(Command::Cancel, None), None);
    assert!(marking.discard_asked.is_some(), "cancel asks");
    assert_eq!(marking.answer_discard(false, false), None, "no answer yet");
    assert_eq!(
        marking.answer_discard(true, false),
        Some(MarkingOutcome::Cancelled)
    );
    // With nothing to lose it closes at once.
    let mut untouched = Marking::new_grid(300.0, 300.0);
    assert_eq!(
        untouched.run_command(Command::Cancel, None),
        Some(MarkingOutcome::Cancelled)
    );
    assert_eq!(
        untouched.run_command(Command::Back, None),
        Some(MarkingOutcome::Cancelled)
    );
}

#[test]
fn the_discard_question_shows_and_blocks_the_keys() {
    let mut run = Run::new(ThemeChoice::Dark, 390.0);
    let mut marking = Marking::new_grid(300.0, 300.0);
    marking.go_to(Step::Shots);
    marking.add_shot([0.0, 0.0]);
    marking.selected = Some(Handle::Shot(0));
    run.frame(&mut marking, None, Vec::new());
    assert_eq!(
        marking.run_command(
            Command::Cancel,
            Some(&Space::Grid {
                size_mm: [300.0, 300.0]
            })
        ),
        None
    );
    let delete = egui::Event::Key {
        key: egui::Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let outcome = run.frame(&mut marking, None, vec![delete]);
    assert_eq!(outcome, MarkingOutcome::Working);
    assert_eq!(marking.shot_count(), 1, "keys wait for the answer");
    assert!(marking.discard_asked.is_some());
}

// --- Finding the grid ---

/// A photo of a plain grid, its pixels half the photo's size: lines every
/// 40 px, 2 px wide, centred at x = 28 and y = 14 (+ 40 n).
fn grid_photo(context: &egui::Context, lines: bool) -> PhotoImage {
    grid_photo_with(context, lines, None)
}

/// The same, with lines 4 px wide every `heavy` lines when given.
fn grid_photo_with(context: &egui::Context, lines: bool, heavy: Option<u32>) -> PhotoImage {
    let (width, height) = (800u32, 600u32);
    let on = |v: u32, shift: u32| {
        let n = (v + shift + 1) / 40;
        let wide = heavy.is_some_and(|k| n.is_multiple_of(k));
        let at = (v + shift) % 40;
        if wide {
            !(3..=38).contains(&at)
        } else {
            at < 2
        }
    };
    let pixels = (0..width * height)
        .map(|i| {
            let (x, y) = (i % width, i / width);
            if lines && (on(x, 13) || on(y, 27)) {
                50
            } else {
                235
            }
        })
        .collect();
    let mut image = photo(context);
    image.size_px = [1600, 1200];
    image.ink = Some(groups_vision::grid::InkImage {
        width,
        height,
        pixels,
    });
    image
}

#[test]
fn a_new_photo_finds_its_grid_by_itself_and_undo_takes_it_back() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let image = grid_photo(&run.context, true);
    let mut marking = Marking::new_photo("photo".into(), None).with_grid_sizes(GridSizes {
        last_mm: Some(5.0),
        ..GridSizes::default()
    });
    assert!(!marking.unsaved());
    for _ in 0..3 {
        run.frame(&mut marking, Some(&image), vec![]);
    }
    assert!(matches!(marking.finder.search, grid::Search::Found(_)));
    assert!(
        marking.finder.counted(&marking.scale),
        "Swap is not offered"
    );
    let corners = marking.scale.corners.clone();
    assert_eq!(corners.len(), 4);
    for [x, y] in &corners {
        let off =
            |v: f64, at: f64| ((v / 2.0 - at).rem_euclid(40.0) + 20.0).rem_euclid(40.0) - 20.0;
        assert!(
            off(*x, 28.0).abs() < 0.2 && off(*y, 14.0).abs() < 0.2,
            "{corners:?}"
        );
    }
    let across = ((corners[1][0] - corners[0][0]) / 80.0).round() as u32;
    let down = ((corners[3][1] - corners[0][1]) / 80.0).round() as u32;
    assert_eq!(
        (marking.scale.squares_across, marking.scale.squares_down),
        (across, down)
    );
    assert!(across >= 15 && down >= 10, "{across} × {down}");
    assert_eq!(marking.scale.square_mm, 5.0);
    assert!(matches!(marking.scale.state(), ScaleState::Ready(_)));
    // The scale counts only once the square size is confirmed.
    assert!(marking.finder.awaits_size(&marking.scale));
    marking.apply(Command::Next, None);
    assert_eq!(marking.step, Step::Scale);
    marking.finder.confirm(&marking.scale);
    assert!(!marking.finder.awaits_size(&marking.scale));
    assert_eq!(marking.grid_sizes().last_mm, Some(5.0));
    // It looks once by itself; asked again, it looks at the part on screen.
    marking.finder.ask(false);
    for _ in 0..2 {
        run.frame(&mut marking, Some(&image), vec![]);
    }
    assert!(matches!(marking.finder.search, grid::Search::Found(_)));
    marking.undo();
    assert!(marking.scale.corners.is_empty());
}

#[test]
fn a_photo_without_a_grid_says_so_and_leaves_the_taps_to_the_shooter() {
    let mut run = Run::new(ThemeChoice::Sun, 390.0);
    let image = grid_photo(&run.context, false);
    let mut marking = Marking::new_photo("photo".into(), None);
    for _ in 0..3 {
        run.frame(&mut marking, Some(&image), vec![]);
    }
    assert_eq!(
        marking.finder.search,
        grid::Search::Failed {
            reason: groups_vision::grid::NoGrid::NoLines,
            corners: Vec::new(),
        }
    );
    assert!(marking.scale.corners.is_empty());
    assert!(!marking.unsaved());
}

#[test]
fn the_last_square_size_is_kept_only_for_a_blank_scale() {
    let sizes = GridSizes {
        last_mm: Some(12.7),
        ..GridSizes::default()
    };
    let blank = Marking::new_photo("photo".into(), None).with_grid_sizes(sizes);
    assert_eq!(blank.scale.square_mm, 12.7);
    let scaled = Calibration::FourCorner {
        corners_px: [
            [100.0, 100.0],
            [500.0, 100.0],
            [500.0, 400.0],
            [100.0, 400.0],
        ],
        width_mm: 100.0,
        height_mm: 75.0,
        ordered: false,
    };
    let kept = Marking::new_photo("photo".into(), Some(scaled)).with_grid_sizes(sizes);
    assert_eq!(kept.scale.square_mm, 25.0);
    assert_eq!(kept.grid_sizes().last_mm, Some(25.0));
}

#[test]
fn edge_labels_give_lengths_to_a_tenth() {
    assert_eq!(tenths(46.0 * 25.4), "1168.4");
    assert_eq!(tenths(254.0), "254");
    assert_eq!(tenths(12.7), "12.7");
}

#[test]
fn big_and_small_squares_each_keep_their_size() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let image = grid_photo_with(&run.context, true, Some(4));
    let mut marking = Marking::new_photo("photo".into(), None);
    for _ in 0..3 {
        run.frame(&mut marking, Some(&image), vec![]);
    }
    let grid::Search::Found(found) = &marking.finder.search else {
        panic!("{:?}", marking.finder.search);
    };
    assert_eq!(found.every, 4);
    assert_eq!(found.level, grid::Level::Big);
    // Nothing remembered: a grid heavier every 4 is taken for inches.
    assert_eq!(marking.scale.square_mm, 25.4);
    let big = (marking.scale.squares_across, marking.scale.squares_down);
    marking
        .finder
        .choose(grid::Level::Small, &mut marking.scale);
    assert_eq!(marking.scale.square_mm, 25.4 / 4.0);
    assert_eq!(
        (marking.scale.squares_across, marking.scale.squares_down),
        (big.0 * 4, big.1 * 4)
    );
    // The shooter says the small squares are 5 mm: the big ones are 20.
    marking.scale.square_mm = 5.0;
    marking.finder.confirm(&marking.scale);
    marking.finder.choose(grid::Level::Big, &mut marking.scale);
    assert_eq!(marking.scale.square_mm, 20.0);
    let sizes = marking.finder.sizes;
    assert_eq!(
        (sizes.big_mm, sizes.small_mm, sizes.every),
        (Some(20.0), Some(5.0), Some(4))
    );
}

#[test]
fn sizes_no_target_has_are_warned_about() {
    let mut draft = ScaleDraft::default();
    for corner in [
        [100.0, 100.0],
        [1100.0, 100.0],
        [1100.0, 900.0],
        [100.0, 900.0],
    ] {
        draft.place(corner);
    }
    let sight = scale::Sight {
        can_search: true,
        calibre_mm: Some(6.5),
        photo_short_px: Some(1000.0),
    };
    let warnings = |draft: &ScaleDraft| {
        let ScaleState::Ready(calibration) = draft.state() else {
            panic!("a scale");
        };
        scale::implausible(draft, &calibration, sight)
    };
    // 4 × 4 inch squares: 101.6 mm, a hole 64 px across.
    (draft.squares_across, draft.squares_down) = (4, 4);
    assert!(warnings(&draft).is_empty(), "{:?}", warnings(&draft));
    // 40 inches across: no target.
    (draft.squares_across, draft.squares_down) = (40, 32);
    assert_eq!(warnings(&draft).len(), 1, "{:?}", warnings(&draft));
    // 1 mm squares: a sheet 4 mm wide, and a hole a fifth of the photo.
    draft.square_mm = 1.0;
    (draft.squares_across, draft.squares_down) = (4, 4);
    assert_eq!(warnings(&draft).len(), 2, "{:?}", warnings(&draft));
}

// --- Hole suggestions ---

/// A hole drawn on a synthetic target: its centre and radius in target mm.
struct DrawnHole {
    mm: Point,
    radius_mm: f64,
}

/// A target photo `width` × `height` pixels, a reduction by `per_px` of the
/// photo: cream paper, blue grid lines every 10 mm, a black aiming disc and
/// the holes, dark on the paper and light on the black, edges smoothed.
/// `to_mm` takes photo pixels to target mm.
fn target_rgba(
    width: u32,
    height: u32,
    per_px: f64,
    to_mm: &dyn Fn(Point) -> Option<Point>,
    bull: (Point, f64),
    holes: &[DrawnHole],
) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0f64; 3];
            for sy in 0..3 {
                for sx in 0..3 {
                    let px = Point::new(
                        (f64::from(x) + (f64::from(sx) + 0.5) / 3.0) / per_px,
                        (f64::from(y) + (f64::from(sy) + 0.5) / 3.0) / per_px,
                    );
                    let Some(mm) = to_mm(px) else {
                        continue;
                    };
                    let in_bull = (mm - bull.0).length() < bull.1;
                    let on_line = (mm.x.rem_euclid(10.0) < 0.25) || (mm.y.rem_euclid(10.0) < 0.25);
                    let mut colour = if in_bull {
                        [25.0, 25.0, 28.0]
                    } else if on_line {
                        [120.0, 160.0, 220.0]
                    } else {
                        [242.0, 236.0, 222.0]
                    };
                    if holes
                        .iter()
                        .any(|hole| (mm - hole.mm).length() < hole.radius_mm)
                    {
                        colour = if in_bull {
                            [205.0, 200.0, 190.0]
                        } else {
                            [45.0, 40.0, 38.0]
                        };
                    }
                    for (total, value) in sum.iter_mut().zip(colour) {
                        *total += value;
                    }
                }
            }
            rgba.extend(sum.map(|total| (total / 9.0).round() as u8));
            rgba.push(255);
        }
    }
    rgba
}

/// The test target as a photo ready to mark, 1600 × 1200 pixels shown and
/// searched at 800 × 600, with the holes' centres in photo pixels.
fn target_photo(
    context: &egui::Context,
    calibration: &Calibration,
) -> (PhotoImage, Vec<u8>, Vec<[f64; 2]>, Point) {
    target_photo_at(context, calibration, 0.5)
}

/// The same, kept at `per_px` image pixels to a photo pixel.
fn target_photo_at(
    context: &egui::Context,
    calibration: &Calibration,
    per_px: f64,
) -> (PhotoImage, Vec<u8>, Vec<[f64; 2]>, Point) {
    let core = crate::groups::measure::scale(calibration).expect("a scale");
    let centre = core.to_mm(Point::new(800.0, 600.0)).unwrap();
    let at = |dx: f64, dy: f64| centre + Point::new(dx, dy);
    let holes = [
        // On the paper.
        at(-45.0, 30.0),
        at(40.0, 35.0),
        at(45.0, -30.0),
        at(-40.0, -38.0),
        // Two touching.
        at(4.0, 46.0),
        at(9.2, 46.0),
        // Through the black.
        at(-8.0, 6.0),
        at(9.0, -7.0),
    ];
    let drawn: Vec<DrawnHole> = holes
        .iter()
        .map(|mm| DrawnHole {
            mm: *mm,
            radius_mm: 3.25,
        })
        .collect();
    let (width, height) = ((1600.0 * per_px) as u32, (1200.0 * per_px) as u32);
    let rgba = target_rgba(
        width,
        height,
        per_px,
        &|px| core.to_mm(px),
        (centre, 22.0),
        &drawn,
    );
    let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
    let photo = PhotoImage {
        id: "photo".into(),
        texture: context.load_texture("holes-target", image, egui::TextureOptions::LINEAR),
        size_px: [1600, 1200],
        ink: Some(groups_vision::grid::InkImage::from_rgba(
            width, height, &rgba,
        )),
    };
    let centres = holes
        .iter()
        .map(|mm| {
            let px = core.to_px(*mm).unwrap();
            [px.x, px.y]
        })
        .collect();
    let aim = core.to_px(centre).unwrap();
    (photo, rgba, centres, aim)
}

fn two_point_scale() -> Calibration {
    Calibration::TwoPoint {
        a_px: [100.0, 100.0],
        b_px: [600.0, 100.0],
        distance_mm: 100.0,
    }
}

fn four_corner_scale() -> Calibration {
    Calibration::FourCorner {
        corners_px: [
            [300.0, 220.0],
            [1310.0, 260.0],
            [1360.0, 1010.0],
            [250.0, 960.0],
        ],
        width_mm: 200.0,
        height_mm: 150.0,
        ordered: false,
    }
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Detect on a drawn target finds its holes, dark and light, on a plain
/// and on a straightened photo, from the key and the button.
#[test]
fn detect_finds_the_holes_of_a_drawn_target() {
    for (calibration, straightened) in [(two_point_scale(), false), (four_corner_scale(), true)] {
        for width in [1200.0, 390.0] {
            let mut run = Run::new(ThemeChoice::Sun, width);
            let (image, _, centres, _) = target_photo(&run.context, &calibration);
            let mut marking = Marking::new_photo("photo".into(), Some(calibration));
            // A hole right on the aim point is suggested too.
            marking.set_aim(Some(centres[6]));
            marking.go_to(Step::Shots);
            marking.zoom_to_aim = false;
            marking.add_shot(centres[0]);
            marking.commit();
            run.frame(&mut marking, Some(&image), Vec::new());
            assert_eq!(
                matches!(
                    marking.space(&MarkingContext {
                        photo: Some(&image),
                        calibre_mm: None,
                        distance_m: None
                    }),
                    Some(Space::Straightened { .. })
                ),
                straightened
            );
            if width > 1000.0 {
                run.frame(&mut marking, Some(&image), vec![key(egui::Key::D)]);
            } else {
                marking.apply(Command::Detect, None);
            }
            assert!(marking.holes.looking());
            run.frame(&mut marking, Some(&image), Vec::new());
            assert_eq!(marking.instruction(width < 1000.0), "Looking for holes…");
            for _ in 0..2 {
                run.frame(&mut marking, Some(&image), Vec::new());
            }
            let suggestions = marking
                .holes
                .shown()
                .unwrap_or_else(|| panic!("{straightened} {width}: {:?}", marking.holes))
                .to_vec();
            assert_eq!(marking.depth(), 4, "Back drops the suggestions first");
            for (index, centre) in centres.iter().enumerate() {
                let found = suggestions.iter().find(|suggestion| {
                    (suggestion.at[0] - centre[0]).hypot(suggestion.at[1] - centre[1]) < 4.0
                });
                let found = found.unwrap_or_else(|| {
                    panic!("{straightened} {width}: hole {index} at {centre:?} in {suggestions:?}")
                });
                assert_eq!(found.ticked, index != 0, "{index}: {found:?}");
                assert_eq!(found.near_marked, index == 0);
                assert_eq!(found.blob.is_some(), index == 4 || index == 5, "{found:?}");
            }
            assert_eq!(suggestions.len(), centres.len(), "{suggestions:?}");
            // Each ring is about the hole's size: 6.5 mm.
            let core = crate::groups::measure::scale(&calibration).unwrap();
            for suggestion in &suggestions {
                let at = Point::new(suggestion.at[0], suggestion.at[1]);
                let mm = if straightened {
                    2.0 * suggestion.radius
                } else {
                    2.0 * suggestion.radius * core.mm_per_px_at(at).unwrap()
                };
                assert!((mm - 6.5).abs() < 1.0, "{mm} mm");
            }
        }
    }
}

/// The photo's colours searched as they are find the same holes as the
/// one-byte ink image the app keeps.
#[test]
fn the_ink_image_finds_what_the_colours_find() {
    use groups_vision::holes::{HoleQuery, Pixels, Region, find_holes};
    let context = egui::Context::default();
    let (image, rgba, _, _) = target_photo(&context, &two_point_scale());
    let ink = image.ink.as_ref().unwrap();
    let query = HoleQuery::new(Some(15.0));
    let region = |pixels| Region {
        width_px: 800,
        height_px: 600,
        pixels,
    };
    let from_ink = find_holes(&region(Pixels::Gray(&ink.pixels)), &query).unwrap();
    let from_colour = find_holes(&region(Pixels::Rgba(&rgba)), &query).unwrap();
    assert_eq!(from_ink.len(), 8, "{from_ink:?}");
    assert_eq!(from_ink.len(), from_colour.len());
    for hole in &from_ink {
        assert!(
            from_colour
                .iter()
                .any(|other| (other.x_px - hole.x_px).hypot(other.y_px - hole.y_px) < 0.5),
            "{hole:?}"
        );
    }
}

#[test]
fn rings_start_ticked_unless_marked_already_or_doubtful() {
    let hole = groups_vision::holes::Hole {
        x_px: 10.0,
        y_px: 10.0,
        diameter_px: 12.0,
        score: 0.8,
        near_marked: false,
        light: false,
        split: None,
    };
    assert!(holes::starts_ticked(&hole));
    assert!(!holes::starts_ticked(&groups_vision::holes::Hole {
        near_marked: true,
        ..hole
    }));
    assert!(!holes::starts_ticked(&groups_vision::holes::Hole {
        score: holes::LOW_SCORE - 0.01,
        ..hole
    }));
    assert!(holes::starts_ticked(&groups_vision::holes::Hole {
        score: holes::LOW_SCORE,
        light: true,
        ..hole
    }));
}

#[test]
fn shots_are_numbered_in_rows_from_the_top_left() {
    let points = [
        Point::new(50.0, 12.0),
        Point::new(10.0, 40.0),
        Point::new(12.0, 10.0),
        Point::new(30.0, 15.0),
        Point::new(40.0, 42.0),
    ];
    assert_eq!(holes::reading_order(&points, 8.0), vec![2, 3, 0, 1, 4]);
    assert!(holes::reading_order(&[], 8.0).is_empty());
}

#[test]
fn tapping_a_ring_ticks_it_and_add_takes_exactly_the_ticked_in_one_undo() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let (image, _, centres, _) = target_photo(&run.context, &two_point_scale());
    let mut marking = Marking::new_photo("photo".into(), Some(two_point_scale()));
    marking.go_to(Step::Shots);
    marking.add_shot([10.0, 10.0]);
    marking.commit();
    let ring = |at: [f64; 2], ticked: bool| holes::Suggestion {
        at,
        radius: 15.0,
        ticked,
        near_marked: false,
        score: 0.8,
        blob: None,
        through: false,
    };
    // Given out of order: numbered from the top left all the same.
    marking.holes = holes::Search::Shown(vec![
        ring(centres[1], true),
        ring(centres[3], true),
        ring(centres[0], false),
        ring(centres[2], true),
    ]);
    run.frame(&mut marking, Some(&image), Vec::new());
    let (camera, _) = marking.last_canvas.unwrap();
    let space = marking.space(&MarkingContext {
        photo: Some(&image),
        calibre_mm: None,
        distance_m: None,
    });
    let screen = |at: [f64; 2]| camera.to_screen(space.unwrap().to_display(at).unwrap());

    // Taps toggle rings and add no shots.
    run.tap(&mut marking, Some(&image), screen(centres[0]));
    run.tap(&mut marking, Some(&image), screen(centres[3]));
    run.tap(
        &mut marking,
        Some(&image),
        screen(centres[3]) + vec2(200.0, 0.0),
    );
    let ticked: Vec<bool> = marking
        .holes
        .shown()
        .unwrap()
        .iter()
        .map(|s| s.ticked)
        .collect();
    assert_eq!(ticked, [true, false, true, true]);
    run.tap(&mut marking, Some(&image), screen(centres[3]));
    run.tap(&mut marking, Some(&image), screen(centres[1]));
    assert_eq!(marking.shot_count(), 1, "taps on and off rings add nothing");

    let before = marking.marks.clone();
    marking.apply(Command::AddSuggested, space.as_ref());
    assert!(marking.holes.shown().is_none());
    assert_eq!(marking.shot_count(), 4);
    let mut expected = vec![centres[0], centres[3], centres[2]];
    expected.sort_by(|a, b| a[1].total_cmp(&b[1]));
    let added: Vec<[f64; 2]> = (1..4).map(|i| marking.shot(i).unwrap().0).collect();
    assert_eq!(added, expected, "from the top");
    marking.undo();
    assert_eq!(marking.marks, before, "one undo takes them all back");

    // Cancel and Escape add nothing.
    for cancel in [Some(Command::CancelSuggested), Some(Command::Back), None] {
        marking.holes = holes::Search::Shown(vec![ring(centres[2], true)]);
        match cancel {
            Some(command) => {
                assert_eq!(marking.run_command(command, space.as_ref()), None);
            }
            None => {
                run.frame(&mut marking, Some(&image), vec![key(egui::Key::Escape)]);
            }
        }
        assert!(marking.holes.shown().is_none(), "{cancel:?}");
        assert_eq!(marking.step, Step::Shots);
        assert_eq!(marking.shot_count(), 1);
    }
    // The browser's Back too.
    marking.holes = holes::Search::Shown(vec![ring(centres[2], true)]);
    assert!(marking.back());
    assert_eq!((marking.step, marking.shot_count()), (Step::Shots, 1));

    // Enter with nothing ticked keeps the rings.
    marking.holes = holes::Search::Shown(vec![ring(centres[2], false)]);
    run.frame(&mut marking, Some(&image), vec![key(egui::Key::Enter)]);
    assert_eq!(marking.holes.shown().map(<[_]>::len), Some(1));
    assert_eq!(marking.shot_count(), 1);

    // A full group says how many were left out.
    while marking.shot_count() < ballistics_store::groups::MAX_SHOTS - 1 {
        marking.add_shot([20.0, 20.0]);
    }
    marking.commit();
    marking.holes = holes::Search::Shown(vec![
        ring(centres[1], true),
        ring(centres[2], true),
        ring(centres[3], true),
    ]);
    marking.apply(Command::AddSuggested, space.as_ref());
    assert_eq!(marking.shot_count(), ballistics_store::groups::MAX_SHOTS);
    assert_eq!(
        marking.holes,
        holes::Search::Refused(holes::Refusal::Full { left_out: 2 })
    );
    assert!(marking.instruction(false).contains("2 holes weren't added"));
}

#[test]
fn detect_says_why_it_did_not_look() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let (image, ..) = target_photo(&run.context, &two_point_scale());
    let detect = |run: &mut Run, marking: &mut Marking| {
        marking.apply(Command::Detect, None);
        for _ in 0..3 {
            run.frame(marking, Some(&image), Vec::new());
        }
        match &marking.holes {
            holes::Search::Refused(refusal) => (*refusal, marking.instruction(false)),
            other => panic!("{other:?}"),
        }
    };
    let mut marking = Marking::new_photo("photo".into(), None);
    marking.go_to(Step::Shots);
    run.frame(&mut marking, Some(&image), Vec::new());
    assert_eq!(
        detect(&mut run, &mut marking),
        (
            holes::Refusal::NoSize,
            "Set the scale and the bullet first".to_owned()
        )
    );
    let mut marking = Marking::new_photo("photo".into(), Some(two_point_scale()));
    marking.go_to(Step::Shots);
    run.calibre_mm = None;
    run.frame(&mut marking, Some(&image), Vec::new());
    assert_eq!(detect(&mut run, &mut marking).0, holes::Refusal::NoSize);
    run.calibre_mm = Some(6.5);

    // 2 mm a pixel: holes about 3 pixels across, under 2 in the image kept.
    let coarse = Calibration::TwoPoint {
        a_px: [100.0, 100.0],
        b_px: [600.0, 100.0],
        distance_mm: 1000.0,
    };
    let mut marking = Marking::new_photo("photo".into(), Some(coarse));
    marking.go_to(Step::Shots);
    run.frame(&mut marking, Some(&image), Vec::new());
    assert_eq!(
        detect(&mut run, &mut marking),
        (
            holes::Refusal::Coarse,
            "The photo is too coarse here to find holes".to_owned()
        )
    );
    assert_eq!(
        marking.instruction_color(theme::palette(&run.context)),
        theme::palette(&run.context).warning
    );

    // 0.8 mm a pixel in an image kept whole: 8 pixels a hole, 5 on screen.
    // The image's own pixels are searched, so zooming isn't asked for; it
    // is only when the part on screen is too large to search whole.
    let (sharp, ..) = target_photo_at(&run.context, &two_point_scale(), 1.0);
    let small = Calibration::TwoPoint {
        a_px: [100.0, 100.0],
        b_px: [600.0, 100.0],
        distance_mm: 400.0,
    };
    let mut marking = Marking::new_photo("photo".into(), Some(small));
    marking.go_to(Step::Shots);
    run.frame(&mut marking, Some(&sharp), Vec::new());
    marking.apply(Command::Detect, None);
    for _ in 0..3 {
        run.frame(&mut marking, Some(&sharp), Vec::new());
    }
    assert!(
        !matches!(
            marking.holes,
            holes::Search::Looking { .. }
                | holes::Search::Refused(holes::Refusal::ZoomIn(_) | holes::Refusal::Coarse)
        ),
        "{:?}",
        marking.holes
    );

    // 0.02 mm a pixel: holes 300 pixels across.
    let fine = Calibration::TwoPoint {
        a_px: [100.0, 100.0],
        b_px: [600.0, 100.0],
        distance_mm: 10.0,
    };
    let mut marking = Marking::new_photo("photo".into(), Some(fine));
    marking.go_to(Step::Shots);
    run.frame(&mut marking, Some(&image), Vec::new());
    assert_eq!(
        detect(&mut run, &mut marking),
        (holes::Refusal::ZoomOut, "Zoom out a little".to_owned())
    );

    assert_eq!(
        holes::Refusal::ZoomIn(1.9).message(),
        "Zoom in about 2× to look for holes"
    );
    assert_eq!(
        holes::Refusal::ZoomIn(1.1).message(),
        "Zoom in about 1.5× to look for holes"
    );
}

/// The window searched maps screen points, photo pixels and its own pixels
/// onto each other in the straightened view.
#[test]
fn the_window_maps_the_straightened_view_both_ways() {
    let space = straightened();
    let rect = Rect::from_min_size(pos2(0.0, 80.0), vec2(390.0, 600.0));
    let extent = space.extent();
    let camera = Camera {
        centre: extent.centre(),
        scale: geometry::fit_scale(&extent, rect) * 2.0,
        screen_centre: rect.center(),
    };
    let window = holes::Window::over(&space, &camera, rect, 3.0, [0.5, 0.5]).unwrap();
    assert!(close(window.density, 3.0, 1e-9), "{window:?}");
    assert!(window.size_px[0] <= 390 * 3 && window.size_px[1] <= 600 * 3);
    for mark in [[500.0, 500.0], [400.0, 300.0], [620.0, 700.0]] {
        let screen = camera.to_screen(space.to_display(mark).unwrap());
        let inside = window.to_window(screen);
        let back = space
            .to_mark(camera.to_display(window.to_screen(inside)))
            .unwrap();
        assert!(close(back[0], mark[0], 1e-3) && close(back[1], mark[1], 1e-3));
    }
    // Zoomed far out the photo's own pixels are finer than the screen's.
    let far = Camera {
        scale: camera.scale / 20.0,
        ..camera
    };
    let window = holes::Window::over(&space, &far, rect, 1.0, [1.0, 1.0]).unwrap();
    assert!(window.density > 1.0, "{window:?}");
}

/// Writes a target photo with .308 holes for trying Detect in the app (`cargo
/// test -p mxsc holes_sample_photo -- --ignored`, then convert the BMP with
/// `sips -s format jpeg`).
#[test]
#[ignore]
fn holes_sample_photo() {
    let calibration = Calibration::FourCorner {
        corners_px: [
            [400.0, 300.0],
            [2000.0, 360.0],
            [2080.0, 1500.0],
            [330.0, 1440.0],
        ],
        width_mm: 200.0,
        height_mm: 150.0,
        ordered: false,
    };
    let core = crate::groups::measure::scale(&calibration).unwrap();
    let centre = core.to_mm(Point::new(1200.0, 900.0)).unwrap();
    let offsets = [
        (-30.0, 22.0),
        (28.0, 30.0),
        (35.0, -25.0),
        (-26.0, -31.0),
        (8.0, 36.0),
        (14.2, 36.0),
        (-7.0, 5.0),
        (6.0, -8.0),
        (-50.0, 2.0),
    ];
    let holes: Vec<DrawnHole> = offsets
        .iter()
        .map(|(dx, dy)| DrawnHole {
            mm: centre + Point::new(*dx, *dy),
            radius_mm: 3.9,
        })
        .collect();
    let (width, height) = (2400u32, 1800u32);
    let rgba = target_rgba(
        width,
        height,
        1.0,
        &|px| core.to_mm(px),
        (centre, 20.0),
        &holes,
    );
    let row = (width * 3).div_ceil(4) * 4;
    let mut bmp = Vec::new();
    let size = 54 + row * height;
    bmp.extend(b"BM");
    bmp.extend(size.to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(width.to_le_bytes());
    bmp.extend(height.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(24u16.to_le_bytes());
    bmp.extend([0u8; 24]);
    for y in (0..height).rev() {
        let start = bmp.len();
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            bmp.extend([rgba[i + 2], rgba[i + 1], rgba[i]]);
        }
        bmp.resize(start + row as usize, 0);
    }
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens/m4");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("holes-sample.bmp"), bmp).unwrap();
}

// --- MX targets ---

type Matrix = [[f64; 3]; 3];

fn apply(m: &Matrix, [x, y]: [f64; 2]) -> [f64; 2] {
    let w = m[2][0] * x + m[2][1] * y + m[2][2];
    [
        (m[0][0] * x + m[0][1] * y + m[0][2]) / w,
        (m[1][0] * x + m[1][1] * y + m[1][2]) / w,
    ]
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn invert(m: &Matrix) -> Matrix {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let adj = [
        [c(1, 2, 1, 2), -c(0, 2, 1, 2), c(0, 1, 1, 2)],
        [-c(1, 2, 0, 2), c(0, 2, 0, 2), -c(0, 1, 0, 2)],
        [c(1, 2, 0, 1), -c(0, 2, 0, 1), c(0, 1, 0, 1)],
    ];
    let det = m[0][0] * adj[0][0] + m[0][1] * adj[1][0] + m[0][2] * adj[2][0];
    adj.map(|row| row.map(|v| v / det))
}

/// An A4 MX target with four aim points and a 10 mm grid.
fn mx_layout() -> groups_vision::marks::Layout {
    crate::cards::mx_target::MxTargetOptions {
        paper: crate::cards::mx_target::MxPaper::A4,
        landscape: false,
        aims: 4,
        grid: crate::cards::mx_target::MxGrid::Mm10,
        square: false,
        size_mm: 40,
    }
    .layout()
}

/// Holes on the MX test sheet, mm from its top left: between the aims.
fn sheet_holes(offsets: &[[f64; 2]]) -> Vec<[f64; 2]> {
    offsets
        .iter()
        .map(|[x, y]| [105.0 + x, 148.5 + y])
        .collect()
}

/// A photo 1600 × 1200 of the MX test sheet with dark holes `radius_mm`
/// at `holes` (sheet mm), turned `turn` degrees and seen a little from the
/// side; `cover` whites out the bottom-right mark. Also sheet mm to px.
fn mx_photo(
    context: &egui::Context,
    holes: &[[f64; 2]],
    radius_mm: f64,
    turn: f64,
    cover: bool,
) -> (PhotoImage, groups_vision::Thumbnail, Matrix) {
    use crate::cards::sheet::{Ink, Mark};
    use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
    let layout = mx_layout();
    let mut sheet = crate::cards::mx_target::page(&layout);
    for [x, y] in holes {
        let points = (0..32)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 32.0;
                [
                    (x + radius_mm * a.cos()) as f32,
                    (y + radius_mm * a.sin()) as f32,
                ]
            })
            .collect();
        sheet.fill(points, Ink::Black);
    }
    if cover {
        let [cx, cy] = layout.mark_centres_mm()[2];
        let half = (layout.mark_mm() * 0.7) as f32;
        sheet.rect(
            cx as f32 - half,
            cy as f32 - half,
            2.0 * half,
            2.0 * half,
            Ink::White,
        );
    }
    let scale = 6.0f32;
    let (sw, sh) = (
        (sheet.width * scale).ceil() as u32,
        (sheet.height * scale).ceil() as u32,
    );
    let mut pixmap = Pixmap::new(sw, sh).unwrap();
    pixmap.fill(Color::WHITE);
    for mark in &sheet.marks {
        let Mark::Fill { points, ink, .. } = mark else {
            if let Mark::Line {
                points,
                width,
                ink,
                closed,
                ..
            } = mark
            {
                let mut path = PathBuilder::new();
                path.move_to(points[0][0], points[0][1]);
                for p in &points[1..] {
                    path.line_to(p[0], p[1]);
                }
                if *closed {
                    path.close();
                }
                if let Some(path) = path.finish() {
                    let [r, g, b] = ink.rgb();
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(r, g, b, 255);
                    let stroke = tiny_skia::Stroke {
                        width: *width,
                        ..Default::default()
                    };
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &stroke,
                        Transform::from_scale(scale, scale),
                        None,
                    );
                }
            }
            continue;
        };
        let mut path = PathBuilder::new();
        path.move_to(points[0][0], points[0][1]);
        for p in &points[1..] {
            path.line_to(p[0], p[1]);
        }
        path.close();
        if let Some(path) = path.finish() {
            let [r, g, b] = ink.rgb();
            let mut paint = Paint::default();
            paint.set_color_rgba8(r, g, b, 255);
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::from_scale(scale, scale),
                None,
            );
        }
    }
    let grey: Vec<u8> = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| ((u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2])) / 10) as u8)
        .collect();
    let [width_mm, height_mm] = layout.sheet_mm();
    let (w, h) = (1600u32, 1200u32);
    let (s, c) = turn.to_radians().sin_cos();
    let fit = (0.85 * f64::from(h) / (width_mm * s.abs() + height_mm * c.abs()))
        .min(0.85 * f64::from(w) / (width_mm * c.abs() + height_mm * s.abs()));
    let centre = [
        [1.0, 0.0, -width_mm / 2.0],
        [0.0, 1.0, -height_mm / 2.0],
        [0.0, 0.0, 1.0],
    ];
    let turned = [
        [fit * c, -fit * s, 0.0],
        [fit * s, fit * c, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let tilt = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.5e-4, -1.0e-4, 1.0]];
    let to_photo = [
        [1.0, 0.0, f64::from(w) / 2.0 + 13.0],
        [0.0, 1.0, f64::from(h) / 2.0 - 7.0],
        [0.0, 0.0, 1.0],
    ];
    let to_px = multiply(&to_photo, &multiply(&tilt, &multiply(&turned, &centre)));
    let to_mm = invert(&to_px);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let at = |x: usize, y: usize| f64::from(grey[y * sw as usize + x]);
    for y in 0..h {
        for x in 0..w {
            let [mx, my] = apply(&to_mm, [f64::from(x) + 0.5, f64::from(y) + 0.5]);
            let (fx, fy) = (mx * f64::from(scale) - 0.5, my * f64::from(scale) - 0.5);
            let value =
                if fx < 0.0 || fy < 0.0 || fx >= f64::from(sw - 1) || fy >= f64::from(sh - 1) {
                    60.0
                } else {
                    let (ix, iy) = (fx as usize, fy as usize);
                    let (ax, ay) = (fx - ix as f64, fy - iy as f64);
                    let top = at(ix, iy) + ax * (at(ix + 1, iy) - at(ix, iy));
                    let bottom = at(ix, iy + 1) + ax * (at(ix + 1, iy + 1) - at(ix, iy + 1));
                    top + ay * (bottom - top)
                };
            let v = value.round() as u8;
            rgba.extend([v, v, v, 255]);
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let photo = PhotoImage {
        id: "mx".into(),
        texture: context.load_texture("mx-photo", image, egui::TextureOptions::LINEAR),
        size_px: [w, h],
        ink: Some(groups_vision::grid::InkImage::from_rgba(w, h, &rgba)),
    };
    let thumbnail = groups_vision::Thumbnail {
        width_px: w,
        height_px: h,
        rgba,
    };
    (photo, thumbnail, to_px)
}

fn near_px(a: [f64; 2], b: [f64; 2], tolerance: f64) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < tolerance
}

/// A photo of an MX target is scaled by its marks and straightened with
/// the sheet upright, however the photo is turned; the aim step offers the
/// sheet's aim points, the first one picked.
#[test]
fn an_mx_target_scales_and_keeps_the_sheet_up() {
    let layout = mx_layout();
    for turn in [0.0, 90.0, 180.0] {
        let mut run = Run::new(ThemeChoice::Dark, 1200.0);
        let (image, _, to_px) = mx_photo(&run.context, &[], 3.25, turn, false);
        let mut marking = Marking::new_photo("mx".into(), None).with_print_scale(Some(1.0));
        for _ in 0..3 {
            run.frame(&mut marking, Some(&image), Vec::new());
        }
        assert_eq!(marking.scale.mode, ScaleMode::MxTarget, "{turn}");
        assert!(
            matches!(marking.finder.search, grid::Search::Idle),
            "the grid isn't looked for"
        );
        let Some(Calibration::FourCorner {
            ordered,
            width_mm,
            height_mm,
            ..
        }) = marking.scale.to_store()
        else {
            panic!("{turn}: {:?}", marking.scale.state());
        };
        assert!(ordered);
        let rectangle = layout.rectangle_mm();
        assert!(close(width_mm, rectangle[0], 1e-9) && close(height_mm, rectangle[1], 1e-9));
        for (corner, mm) in marking.scale.corners.iter().zip(layout.mark_centres_mm()) {
            assert!(
                near_px(*corner, apply(&to_px, mm), 1.0),
                "{turn}: {corner:?}"
            );
        }
        marking.apply(Command::Next, None);
        assert_eq!(marking.step, Step::Aim);
        let aims = marking.mx_aims();
        assert_eq!(aims.len(), 4);
        let printed = layout.aim_points_mm();
        for (aim, mm) in aims.iter().zip(&printed) {
            assert!(near_px(*aim, apply(&to_px, *mm), 1.5), "{turn}");
        }
        assert_eq!(marking.aim(), Some(aims[0]), "the first aim is taken");
        assert!(!marking.unsaved() || marking.calibration.is_some());
        // Straightened: the sheet's top is up on screen.
        run.frame(&mut marking, Some(&image), Vec::new());
        let space = marking
            .space(&MarkingContext {
                photo: Some(&image),
                calibre_mm: None,
                distance_m: None,
            })
            .unwrap();
        assert!(matches!(space, Space::Straightened { .. }));
        let shown = |px: [f64; 2]| space.to_display(px).unwrap();
        assert!(
            shown(aims[0]).y < shown(aims[2]).y - 50.0,
            "{turn}: top row higher"
        );
        assert!(
            shown(aims[0]).x < shown(aims[1]).x - 50.0,
            "{turn}: first aim on the left"
        );
        // Picking and snapping to aim points.
        marking.apply(Command::PickAim(3), None);
        assert_eq!(marking.aim(), Some(aims[3]));
        let near_second = apply(&to_px, [printed[1][0] + 3.0, printed[1][1] - 2.0]);
        marking.place(near_second);
        assert!(near_px(marking.aim().unwrap(), aims[1], 1e-6), "snapped");
        let far = apply(&to_px, [105.0, 148.5]);
        marking.place(far);
        assert_eq!(
            marking.aim(),
            Some(far),
            "tapped by hand away from the aims"
        );
    }
}

/// The print scale measured on this device sizes the rectangle; another
/// group on the photo starts at the next free aim; marks moved by hand
/// make a tapped scale.
#[test]
fn the_print_scale_and_the_next_aim_apply() {
    let layout = mx_layout();
    let mut run = Run::new(ThemeChoice::Sun, 390.0);
    let (image, _, to_px) = mx_photo(&run.context, &[], 3.25, 0.0, false);
    let mut marking = Marking::new_photo("mx".into(), None).with_print_scale(Some(1.02));
    for _ in 0..3 {
        run.frame(&mut marking, Some(&image), Vec::new());
    }
    let rectangle = layout.rectangle_mm();
    assert!(close(marking.scale.width_mm, rectangle[0] * 1.02, 1e-9));
    assert!(close(marking.scale.height_mm, rectangle[1] * 1.02, 1e-9));
    let stored = marking.scale.to_store();
    let first_aim = apply(&to_px, layout.aim_points_mm()[0]);

    let mut another = Marking::new_photo("mx".into(), stored)
        .with_print_scale(Some(1.02))
        .with_taken_aims(vec![first_aim]);
    assert_eq!(another.step, Step::Aim);
    for _ in 0..3 {
        run.frame(&mut another, Some(&image), Vec::new());
    }
    assert_eq!(another.scale.mode, ScaleMode::MxTarget);
    assert!(close(another.scale.print_scale, 1.02, 1e-9), "found again");
    let second = apply(&to_px, layout.aim_points_mm()[1]);
    assert!(near_px(another.aim().unwrap(), second, 1.5));
    assert!(!another.unsaved(), "the aim offered is no change");

    // Back to the MX target from another way keeps this draft's own factor.
    marking.scale.mode = ScaleMode::TwoPoints;
    let sheet = mx::find(image.ink.as_ref().unwrap(), image.size_px).unwrap();
    let corners = sheet.corners_px();
    marking
        .scale
        .place_sheet(corners, sheet.marks.rectangle_mm());
    assert!(close(marking.scale.width_mm, rectangle[0] * 1.02, 1e-9));
}

#[test]
fn hidden_marks_and_curled_sheets_are_warned_about() {
    use groups_vision::marks::Seen;
    let context = egui::Context::default();
    let (image, ..) = mx_photo(&context, &[], 3.25, 0.0, true);
    let sheet = mx::find(image.ink.as_ref().unwrap(), image.size_px).expect("three marks do");
    assert_eq!(sheet.marks.seen[2], Seen::Estimated);
    assert!(
        sheet
            .warnings()
            .contains(&"One corner mark was estimated: less exact".to_owned())
    );
    let read = [Seen::Read; 4];
    assert!(mx::warnings(&read, 0.03).is_empty());
    assert_eq!(
        mx::warnings(&read, 0.07),
        ["The sheet doesn't lie flat: press it flat for exact numbers"]
    );
    assert_eq!(
        mx::failure(groups_vision::marks::NoMarks::NotFound),
        None,
        "most photos aren't MX targets"
    );
    assert!(mx::failure(groups_vision::marks::NoMarks::Inconsistent).is_some());
}

/// A marking on the MX test sheet with shots at `marked` (sheet mm).
fn marked_sheet(run: &mut Run, image: &PhotoImage, to_px: &Matrix, marked: &[[f64; 2]]) -> Marking {
    marked_sheet_at(run, image, to_px, marked, 1.0)
}

/// The same, on a sheet printed `print` times its size.
fn marked_sheet_at(
    run: &mut Run,
    image: &PhotoImage,
    to_px: &Matrix,
    marked: &[[f64; 2]],
    print: f64,
) -> Marking {
    let mut marking = Marking::new_photo("mx".into(), None).with_print_scale(Some(print));
    for _ in 0..3 {
        run.frame(&mut marking, Some(image), Vec::new());
    }
    marking.go_to(Step::Aim);
    marking.go_to(Step::Shots);
    for mm in marked {
        marking.add_shot(apply(to_px, *mm));
    }
    marking.commit();
    run.frame(&mut marking, Some(image), Vec::new());
    marking
}

fn compare_later(
    run: &mut Run,
    marking: &mut Marking,
    image: &PhotoImage,
    later: &groups_vision::Thumbnail,
) {
    marking.apply(Command::LaterPhoto, None);
    assert!(marking.take_later_request());
    marking.later_photo(later);
    for _ in 0..4 {
        run.frame(marking, Some(image), Vec::new());
    }
}

/// A later photo of the sheet, turned round and seen from elsewhere, gives
/// its new holes as suggestions where they are on the sheet.
#[test]
fn a_later_photo_suggests_the_new_holes() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[[-12.0, -20.0], [10.0, -5.0], [-3.0, 15.0]]);
    let new = sheet_holes(&[[14.0, 22.0], [-15.0, 5.0]]);
    let (image, _, to_px) = mx_photo(&run.context, &old, 3.25, 0.0, false);
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let all: Vec<[f64; 2]> = old.iter().chain(&new).copied().collect();
    let (_, later, _) = mx_photo(&run.context, &all, 3.25, 180.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    let suggestions = marking
        .holes
        .shown()
        .unwrap_or_else(|| panic!("{:?}", marking.holes))
        .to_vec();
    assert_eq!(suggestions.len(), 2, "{suggestions:?}");
    for mm in &new {
        let want = apply(&to_px, *mm);
        assert!(
            suggestions
                .iter()
                .any(|s| near_px(s.at, want, 6.0) && s.ticked && !s.through),
            "{mm:?} in {suggestions:?}"
        );
    }
    let space = marking.space(&MarkingContext {
        photo: Some(&image),
        calibre_mm: None,
        distance_m: None,
    });
    marking.apply(Command::AddSuggested, space.as_ref());
    assert_eq!(marking.shot_count(), 5);
}

#[test]
fn a_later_photo_of_another_sheet_or_a_grown_hole_is_flagged() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[[-12.0, -20.0], [10.0, -5.0], [-3.0, 15.0]]);
    let (image, _, to_px) = mx_photo(&run.context, &old, 3.25, 0.0, false);

    // Other holes altogether: maybe another sheet.
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let elsewhere = sheet_holes(&[[15.0, 25.0], [-18.0, 0.0], [5.0, 30.0]]);
    let (_, later, _) = mx_photo(&run.context, &elsewhere, 3.25, 90.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    let holes::Search::Doubt {
        lined_up, marked, ..
    } = marking.holes
    else {
        panic!("{:?}", marking.holes);
    };
    assert_eq!((lined_up, marked), (0, 3));
    assert!(
        marking
            .instruction(false)
            .starts_with("This may be a different sheet")
    );
    marking.apply(Command::GoOn, None);
    assert_eq!(marking.holes.shown().map(<[_]>::len), Some(3));

    // A second shot half through the first hole.
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let mut grown = old.clone();
    grown.push([old[0][0] + 4.5, old[0][1] + 1.0]);
    let (_, later, _) = mx_photo(&run.context, &grown, 3.25, 0.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    let suggestions = marking
        .holes
        .shown()
        .unwrap_or_else(|| panic!("{:?}", marking.holes));
    assert!(
        suggestions
            .iter()
            .any(|s| s.through && near_px(s.at, apply(&to_px, old[0]), 30.0)),
        "{suggestions:?}"
    );

    // A later photo with a mark hidden.
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let (_, later, _) = mx_photo(&run.context, &old, 3.25, 0.0, true);
    compare_later(&mut run, &mut marking, &image, &later);
    assert_eq!(
        marking.holes,
        holes::Search::Refused(holes::Refusal::Later(mx::LaterError::MarkHidden))
    );
}

/// Writes two photos of the MX test sheet for trying the app (`cargo test
/// -p mxsc mx_sample_photos -- --ignored`): three holes, then five, turned.
#[test]
#[ignore]
fn mx_sample_photos() {
    let context = egui::Context::default();
    let old = sheet_holes(&[[-12.0, -20.0], [10.0, -5.0], [-3.0, 15.0]]);
    let all = sheet_holes(&[
        [-12.0, -20.0],
        [10.0, -5.0],
        [-3.0, 15.0],
        [14.0, 22.0],
        [-15.0, 5.0],
    ]);
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens/m4");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, holes, turn) in [
        ("mx-sample-1.png", &old, 0.0),
        ("mx-sample-2.png", &all, 180.0),
    ] {
        let (_, picture, _) = mx_photo(&context, holes, 3.9, turn, false);
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, picture.width_px, picture.height_px);
            encoder.set_color(png::ColorType::Rgba);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&picture.rgba).unwrap();
        }
        std::fs::write(directory.join(name), png).unwrap();
    }
}

/// The aim (sheet mm → calibration mm) of an MX marking at the printed
/// aim points, and their distances.
fn aim_spacing(marking: &Marking, to_px: &Matrix) -> (f64, f64) {
    let layout = mx_layout();
    let aims = layout.aim_points_mm();
    let ScaleState::Ready(calibration) = marking.scale.state() else {
        panic!("{:?}", marking.scale.state());
    };
    let mm = |sheet: [f64; 2]| {
        let px = apply(to_px, sheet);
        calibration.to_mm(Point::new(px[0], px[1])).unwrap()
    };
    let (first, second, third) = (mm(aims[0]), mm(aims[1]), mm(aims[2]));
    (second.x - first.x, first.y - third.y)
}

/// Marks dragged by hand keep the sheet's order; marks left for tapped
/// corners keep their sizes along the right edges, the sheet on its side
/// or upside down.
#[test]
fn hand_edits_keep_the_sheet_s_sizes() {
    let layout = mx_layout();
    let aims = layout.aim_points_mm();
    let (across, down) = (aims[1][0] - aims[0][0], aims[2][1] - aims[0][1]);
    for turn in [90.0, 180.0] {
        let mut run = Run::new(ThemeChoice::Dark, 1200.0);
        let (image, _, to_px) = mx_photo(&run.context, &[], 3.25, turn, false);
        let mut marking = Marking::new_photo("mx".into(), None).with_print_scale(Some(1.0));
        for _ in 0..3 {
            run.frame(&mut marking, Some(&image), Vec::new());
        }
        let first = marking.scale.corners[0];
        marking.scale.set_handle(0, [first[0] + 0.3, first[1]]);
        assert_eq!(marking.scale.mode, ScaleMode::MxTarget, "{turn}");
        assert!(marking.scale.moved_by_hand);
        let (x, y) = aim_spacing(&marking, &to_px);
        assert!(
            close(x, across, 0.5) && close(y, down, 0.5),
            "{turn}: {x} {y}"
        );

        marking.scale.leave_mx();
        assert_eq!(marking.scale.mode, ScaleMode::FourCorners);
        assert!(marking.scale.retapped);
        let ScaleState::Ready(calibration) = marking.scale.state() else {
            panic!();
        };
        let mm = |sheet: [f64; 2]| {
            let px = apply(&to_px, sheet);
            calibration.to_mm(Point::new(px[0], px[1])).unwrap()
        };
        let (a, b, c) = (mm(aims[0]), mm(aims[1]), mm(aims[2]));
        assert!(
            close(a.distance(b), across, 0.5) && close(a.distance(c), down, 0.5),
            "{turn}: {} {}",
            a.distance(b),
            a.distance(c)
        );
        assert!(!matches!(
            marking.scale.to_store(),
            Some(Calibration::FourCorner { ordered: true, .. })
        ));
    }
}

/// The aim point is offered once, on a new group only: "No aim point" and
/// a reopened group without an aim stay without one.
#[test]
fn an_aim_given_up_stays_given_up() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let (image, _, _) = mx_photo(&run.context, &[], 3.25, 0.0, false);
    let mut marking = Marking::new_photo("mx".into(), None).with_print_scale(Some(1.0));
    for _ in 0..3 {
        run.frame(&mut marking, Some(&image), Vec::new());
    }
    marking.apply(Command::Next, None);
    assert!(marking.aim().is_some());
    marking.apply(Command::NoAim, None);
    marking.apply(Command::GoTo(Step::Aim), None);
    assert_eq!(marking.aim(), None, "not brought back");

    let stored = marking.scale.to_store();
    let mut group = group_template();
    group.marks = marking.marks.clone();
    let mut reopened = Marking::edit(&group, stored.as_ref(), None);
    for _ in 0..3 {
        run.frame(&mut reopened, Some(&image), Vec::new());
    }
    reopened.apply(Command::GoTo(Step::Aim), None);
    assert_eq!(reopened.aim(), None);
    assert!(!reopened.unsaved());
}

/// A ragged hole holding several marked shots lines them all up; a later
/// photo of the same sheet with nothing new says so, and of another sheet
/// with nothing new is still flagged.
#[test]
fn ragged_holes_line_up() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[
        [0.0, 0.0],
        [2.0, 0.5],
        [1.0, 2.0],
        [-1.0, 1.0],
        [20.0, 20.0],
    ]);
    let (image, _, to_px) = mx_photo(&run.context, &old, 3.25, 0.0, false);
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let (_, later, _) = mx_photo(&run.context, &old, 3.25, 0.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    assert_eq!(
        marking.holes,
        holes::Search::Refused(holes::Refusal::NoneNew)
    );

    let mut all = old.clone();
    all.push(sheet_holes(&[[-20.0, 25.0]])[0]);
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let (_, later, _) = mx_photo(&run.context, &all, 3.25, 0.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    assert_eq!(
        marking.holes.shown().map(<[_]>::len),
        Some(1),
        "{:?}",
        marking.holes
    );

    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let (_, blank, _) = mx_photo(&run.context, &[], 3.25, 0.0, false);
    compare_later(&mut run, &mut marking, &image, &blank);
    assert!(
        matches!(
            marking.holes,
            holes::Search::Doubt {
                lined_up: 0,
                marked: 5,
                ..
            }
        ),
        "{:?}",
        marking.holes
    );
    assert!(mx::doubtful(1, 3) && !mx::doubtful(2, 4) && !mx::doubtful(0, 0));
}

/// A separate shot touching a marked one's hole is ticked; a hole a second
/// shot went through is flagged and left to check.
#[test]
fn touching_shots_are_ticked_and_grown_holes_checked() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[[-12.0, -20.0], [10.0, -5.0], [-3.0, 15.0]]);
    let (image, _, to_px) = mx_photo(&run.context, &old, 3.25, 0.0, false);
    for gap in [6.6, 7.0] {
        let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
        let mut all = old.clone();
        all.push([old[0][0] + gap, old[0][1]]);
        let (_, later, _) = mx_photo(&run.context, &all, 3.25, 0.0, false);
        compare_later(&mut run, &mut marking, &image, &later);
        let rings = marking
            .holes
            .shown()
            .unwrap_or_else(|| panic!("{:?}", marking.holes));
        let want = apply(&to_px, all[3]);
        assert!(
            rings
                .iter()
                .any(|ring| near_px(ring.at, want, 6.0) && ring.ticked),
            "{gap}: {rings:?}"
        );
    }
}

/// Holes on a printed aim mark's outline, at its cross and on a grid
/// crossing are found, beside a corner mark too; the printed marks give no
/// ticked rings.
#[test]
fn holes_on_the_print_are_found() {
    let layout = mx_layout();
    let aims = layout.aim_points_mm();
    let size = layout.aim_size_mm();
    let marks = layout.mark_centres_mm();
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[[-12.0, -20.0]]);
    let (image, _, to_px) = mx_photo(&run.context, &old, 3.25, 0.0, false);
    let mut marking = marked_sheet(&mut run, &image, &to_px, &old);
    let new = vec![
        // Inside the first diamond, against its outline.
        [aims[0][0] - size * 0.16, aims[0][1] - size * 0.16],
        // On the second aim's cross.
        [aims[1][0] + 4.0, aims[1][1]],
        // On a grid crossing.
        [105.0, 120.0 + 0.5],
        // Beside a corner mark.
        [marks[3][0] + 17.0, marks[3][1]],
    ];
    let all: Vec<[f64; 2]> = old.iter().chain(&new).copied().collect();
    let (_, later, _) = mx_photo(&run.context, &all, 3.25, 0.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    let rings = marking
        .holes
        .shown()
        .unwrap_or_else(|| panic!("{:?}", marking.holes));
    let missing: Vec<&[f64; 2]> = new
        .iter()
        .filter(|mm| {
            !rings
                .iter()
                .any(|ring| near_px(ring.at, apply(&to_px, **mm), 6.0))
        })
        .collect();
    assert!(missing.is_empty(), "{missing:?} not in {rings:?}");
    for ring in rings.iter().filter(|ring| ring.ticked) {
        assert!(
            new.iter()
                .any(|mm| near_px(ring.at, apply(&to_px, *mm), 6.0)),
            "a ticked ring on no hole: {ring:?}"
        );
    }
}

/// On a sheet printed larger, holes are matched at the bullet's size as
/// printed, and the rings are the true size.
#[test]
fn a_later_photo_allows_for_the_print_scale() {
    let print = 1.1;
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let old = sheet_holes(&[[-12.0, -20.0], [10.0, -5.0], [-3.0, 15.0]]);
    let new = sheet_holes(&[[14.0, 22.0], [-15.0, 5.0]]);
    let radius = 3.25 / print;
    let (image, _, to_px) = mx_photo(&run.context, &old, radius, 90.0, false);
    let mut marking = marked_sheet_at(&mut run, &image, &to_px, &old, print);
    let all: Vec<[f64; 2]> = old.iter().chain(&new).copied().collect();
    let (_, later, _) = mx_photo(&run.context, &all, radius, 180.0, false);
    compare_later(&mut run, &mut marking, &image, &later);
    let rings = marking
        .holes
        .shown()
        .unwrap_or_else(|| panic!("{:?}", marking.holes));
    assert_eq!(rings.len(), 2, "{rings:?}");
    for ring in rings {
        assert!(
            new.iter()
                .any(|mm| near_px(ring.at, apply(&to_px, *mm), 3.0))
        );
        // Straightened: display units are true mm.
        assert!(close(2.0 * ring.radius, 6.5, 0.8), "{}", ring.radius);
    }
    assert!(close(mx::sheet_calibre(6.5, print), 6.5 / 1.1, 1e-12));
}

/// A later photo's request goes when the shooter leaves the shots.
#[test]
fn a_later_photo_is_dropped_on_leaving() {
    let mut run = Run::new(ThemeChoice::Dark, 1200.0);
    let (image, later, to_px) = mx_photo(&run.context, &[], 3.25, 0.0, false);
    let mut marking = marked_sheet(&mut run, &image, &to_px, &[]);
    marking.later_photo(&later);
    assert!(marking.back());
    assert!(marking.later.is_none());
    marking.later_photo(&later);
    marking.apply(Command::CancelSuggested, None);
    assert!(marking.later.is_none());
    marking.later_photo(&later);
    marking.go_to(Step::Aim);
    assert!(marking.later.is_none());
}

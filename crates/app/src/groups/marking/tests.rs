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
    let mut draft = ScaleDraft::default();
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
    }
}

struct Run {
    context: egui::Context,
    width: f32,
    time: f64,
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
            calibre_mm: Some(6.5),
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

    vec![
        ("two points", two),
        ("four corners", four),
        ("aim on a straightened photo", aim),
        ("shots", shots),
        ("grid", grid),
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

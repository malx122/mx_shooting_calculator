//! The Groups pages drawn headlessly, and their navigation.

use ballistics_store::{MemoryPhotos, Store};

use super::*;
use crate::theme::ThemeChoice;

/// The demo data, with its groups, as the app would hold it.
pub(crate) fn demo_data() -> Data {
    let mut store = Store::open_in_memory().unwrap();
    store.load_demo().unwrap();
    assert_eq!(demo::load(&store).unwrap(), 5);
    assert_eq!(demo::load(&store).unwrap(), 0, "loaded once");
    Data {
        rifles: store.rifles().unwrap(),
        bullets: store.bullets().unwrap(),
        loads: store.rifle_loads().unwrap(),
        groups: store.groups_data().unwrap(),
        ..Data::default()
    }
}

fn demo_group<'a>(data: &'a Data, notes: &str) -> &'a Group {
    data.groups
        .groups
        .iter()
        .find(|group| group.notes.starts_with(notes))
        .expect("a demo group")
}

/// Draws the page twice, as the app does when sizes settle.
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
                mobile::apply_style(ui.ctx());
                actions.extend(page.show(ui, data, &photos));
            },
        );
        output.textures_delta.clear();
    }
    actions
}

fn marked_flow(data: &Data, saving: bool) -> View {
    let source = demo_group(data, "Shot 3");
    View::Marking(Box::new(Flow {
        marking: Marking::edit(source, None, None),
        purpose: Purpose::New {
            group: Box::new(Group::on_grid(200.0, 200.0, 1_000)),
            photo: None,
            from: None,
        },
        form: Form::like(source),
        saving,
        pending: None,
    }))
}

#[test]
fn every_groups_view_draws_in_both_themes_on_a_desktop_and_a_phone() {
    let data = demo_data();
    for choice in [ThemeChoice::Dark, ThemeChoice::Sun] {
        for width in [1200.0, 390.0] {
            let mut page = GroupsPage::new(GroupSettings::default());
            assert!(
                draw(&mut page, &data, choice, width).is_empty(),
                "the library"
            );
            for group in &data.groups.groups {
                page.open(group);
                assert!(
                    draw(&mut page, &data, choice, width).is_empty(),
                    "{}",
                    group.notes
                );
            }
            page.numbers.with_called_fliers = true;
            page.open(demo_group(&data, "Shot 3"));
            draw(&mut page, &data, choice, width);
            page.numbers.with_called_fliers = false;
            if let View::Group(open) = &mut page.view {
                open.editing = true;
            }
            draw(&mut page, &data, choice, width);

            page.view = View::Library;
            for sheet in [Sheet::NewGroup, Sheet::Grid] {
                page.sheet = Some(sheet);
                draw(&mut page, &data, choice, width);
            }
            page.sheet = None;
            page.start_grid(&data);
            draw(&mut page, &data, choice, width);
            for saving in [false, true] {
                page.view = marked_flow(&data, saving);
                let actions = draw(&mut page, &data, choice, width);
                assert!(actions.is_empty(), "nothing is saved until asked");
            }
        }
    }
}

#[test]
fn back_closes_the_views_one_at_a_time() {
    let data = demo_data();
    let mut page = GroupsPage::new(GroupSettings::default());
    assert_eq!(page.depth(), 0);
    page.sheet = Some(Sheet::NewGroup);
    assert_eq!(page.depth(), 1);
    page.back(&data);
    assert!(page.sheet.is_none());

    let group = demo_group(&data, "Shot 3");
    page.open(group);
    assert_eq!(page.depth(), 1);
    if let View::Group(open) = &mut page.view {
        open.editing = true;
        open.form.notes = "changed".into();
    }
    assert_eq!(page.depth(), 2);
    assert!(page.unsaved(&data), "closing the sheet drops the edit");
    assert!(page.has_unsaved(&data));
    page.back(&data);
    assert_eq!(page.depth(), 1);
    assert!(!page.has_unsaved(&data), "the edit was dropped");
    if let View::Group(open) = &mut page.view {
        open.photo = true;
    }
    assert_eq!(page.depth(), 2);
    page.back(&data);
    assert_eq!(page.depth(), 1);
    page.back(&data);
    assert!(matches!(page.view, View::Library));

    // Marking from the library: Back steps through marking, then closes it.
    page.start_grid(&data);
    let mut depth = page.depth();
    assert!(depth >= 1);
    for _ in 0..10 {
        if matches!(page.view, View::Library) {
            break;
        }
        page.back(&data);
        assert!(page.depth() < depth, "each Back closes a view");
        depth = page.depth();
    }
    assert!(matches!(page.view, View::Library));

    // The save sheet goes back to the shots, and a save opens the group.
    page.view = marked_flow(&data, true);
    let with_sheet = page.depth();
    assert!(page.has_unsaved(&data), "a marked group isn't saved yet");
    assert!(!page.unsaved(&data), "back to the shots drops nothing");
    page.back(&data);
    assert_eq!(page.depth(), with_sheet - 1);
    let mut actions = Vec::new();
    if let View::Marking(flow) = &mut page.view {
        flow.saving = true;
    }
    page.save_new(&data, &mut actions);
    let Some(Action::SaveGroup(saved)) = actions.last() else {
        panic!("the group is saved last");
    };
    assert_eq!(saved.group.shot_count(), 6);
    assert_eq!(
        saved.group.rifle_id, group.rifle_id,
        "like the group it was marked from"
    );
    let saved = saved.group.clone();
    page.saved(&saved);
    assert!(matches!(&page.view, View::Group(open) if open.id == saved.id));
}

#[test]
fn marking_another_group_on_a_photo_goes_back_to_the_first() {
    let data = demo_data();
    let mut page = GroupsPage::new(GroupSettings::default());
    let group = demo_group(&data, "Shot 3");
    page.open(group);
    page.start_marking(
        Marking::new_grid(200.0, 200.0),
        Purpose::New {
            group: Box::new(Group::on_grid(200.0, 200.0, 1)),
            photo: None,
            from: Some(group.id.clone()),
        },
        Form::like(group),
    );
    assert!(page.depth() >= 2, "over the group's page");
    for _ in 0..10 {
        if !matches!(page.view, View::Marking(_)) {
            break;
        }
        page.back(&data);
    }
    assert!(matches!(&page.view, View::Group(open) if open.id == group.id));
}

#[test]
fn a_new_group_starts_with_the_last_used_rifle_bullet_and_distance() {
    let data = demo_data();
    let form = Form::for_new(&data, true);
    let newest = data
        .groups
        .groups
        .iter()
        .max_by_key(|group| group.taken_at)
        .unwrap();
    assert_eq!(form.rifle_id, newest.rifle_id);
    assert_eq!(form.bullet_id, newest.bullet_id);
    assert_eq!(Some(form.distance_m), newest.distance_m);
    assert!(
        matches!(&form.session, form::SessionChoice::Existing(id) if Some(id) == newest.session_id.as_ref()),
        "today's session"
    );
    assert!(form.count_rounds);
}

#[test]
fn calling_a_flier_marks_that_shot_only() {
    let data = demo_data();
    let group = demo_group(&data, "First shot");
    let called = called_flier(group, 1);
    let Marks::Grid(marks) = &called.marks else {
        panic!("a grid group");
    };
    assert!(marks.shots[1].called_flier);
    assert_eq!(
        marks.shots.iter().filter(|shot| shot.called_flier).count(),
        1
    );
    assert_eq!(called.id, group.id);
}

#[test]
fn the_demo_has_a_cold_bore_shot_a_called_flier_and_a_flier_to_suggest() {
    let data = demo_data();
    let measured: Vec<Measured> = data
        .groups
        .groups
        .iter()
        .map(|group| measure_group(group, &data).expect("a grid group measures"))
        .collect();
    assert!(measured.iter().any(|m| !m.cold_bore.is_empty()));
    assert!(measured.iter().any(|m| !m.called_fliers.is_empty()));
    let suggested = measured.iter().filter(|m| {
        let points: Vec<_> = m.counted.iter().map(|&i| m.shots_mm[i]).collect();
        let options = groups_core::Options {
            aim: m.aim_mm,
            ..Default::default()
        };
        groups_core::flier_check(&points, options)
            .ok()
            .flatten()
            .is_some()
    });
    assert_eq!(suggested.count(), 1, "one group shows the flier card");
}

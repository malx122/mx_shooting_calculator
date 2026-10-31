//! Two endpoints for a decorative line, on a shared percentage scale.

use eframe::egui;

use crate::i18n::tr;

pub(super) fn show(ui: &mut egui::Ui, start: &mut f32, end: &mut f32) -> egui::Response {
    let normalized_end = if end.is_finite() {
        end.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let normalized_start = if start.is_finite() {
        start.clamp(0.0, normalized_end)
    } else {
        0.0
    };
    let mut high = normalized_end * 100.0;
    let mut low = normalized_start * 100.0;
    let before = (low, high);
    ui.strong(tr("Line position"));
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Start"));
        ui.add(
            egui::DragValue::new(&mut low)
                .range(0.0..=high)
                .speed(0.1)
                .fixed_decimals(1)
                .suffix(" %"),
        );
        ui.label(tr("End"));
        ui.add(
            egui::DragValue::new(&mut high)
                .range(low..=100.0)
                .speed(0.1)
                .fixed_decimals(1)
                .suffix(" %"),
        );
    });
    let mut response = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 0))
        .show(ui, |ui| {
            ui.spacing_mut().slider_width = ui.available_width();
            ui.add(
                egui::RangeSlider::new(&mut low, &mut high, 0.0..=100.0)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Edits)
                    .integer(),
            )
        })
        .inner;
    ui.weak(tr(
        "0% is just below each mark. 100% is the label's bottom edge, including the ruler.",
    ));
    let new_start = low / 100.0;
    let new_end = high / 100.0;
    if (low, high) != before || *start != normalized_start || *end != normalized_end {
        *start = new_start;
        *end = new_end;
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Language, set};

    fn draw(
        context: &egui::Context,
        width: f32,
        start: &mut f32,
        end: &mut f32,
        events: Vec<egui::Event>,
    ) -> egui::Response {
        let mut response = None;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 500.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                crate::mobile::apply_style(context);
                response = Some(show(ui, start, end));
                assert!(
                    ui.min_rect().right() <= width + 0.5,
                    "control overflows a narrow screen"
                );
            },
        );
        output.textures_delta.clear();
        response.unwrap()
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn either_handle_can_move_without_moving_the_other_and_handles_cannot_cross() {
        let context = egui::Context::default();
        let (mut start, mut end) = (0.0, 1.0);
        for _ in 0..2 {
            draw(&context, 320.0, &mut start, &mut end, Vec::new());
        }
        let rect = draw(&context, 320.0, &mut start, &mut end, Vec::new()).rect;
        let point =
            |fraction: f32| egui::pos2(rect.left() + rect.width() * fraction, rect.center().y);
        let first = point(0.3);
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![egui::Event::PointerMoved(first), button(first, true)],
        );
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![button(first, false)],
        );
        assert!((0.25..0.35).contains(&start));
        assert_eq!(end, 1.0);
        let chosen_start = start;
        let second = point(0.75);
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![egui::Event::PointerMoved(second), button(second, true)],
        );
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![button(second, false)],
        );
        assert!((0.7..0.8).contains(&end));
        assert_eq!(start, chosen_start);
        let chosen_end = end;
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![egui::Event::PointerMoved(first), button(first, true)],
        );
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![egui::Event::PointerMoved(point(1.0))],
        );
        draw(
            &context,
            320.0,
            &mut start,
            &mut end,
            vec![button(point(1.0), false)],
        );
        assert!(start <= end);
        assert_eq!(end, chosen_end);
    }

    #[test]
    fn control_fits_narrow_screens_in_all_languages_without_changing_idle_values() {
        for language in Language::ALL {
            set(language);
            for width in [240.0, 320.0, 600.0] {
                let context = egui::Context::default();
                let (mut start, mut end) = (0.333_f32, 0.789_f32);
                for _ in 0..2 {
                    draw(&context, width, &mut start, &mut end, Vec::new());
                }
                assert_eq!((start, end), (0.333, 0.789));
            }
        }
        set(Language::English);
    }

    #[test]
    fn invalid_endpoints_are_normalized_before_display() {
        for (initial, expected) in [
            ((f32::NAN, f32::INFINITY), (0.0, 1.0)),
            ((-1.0, 2.0), (0.0, 1.0)),
            ((0.9, 0.2), (0.2, 0.2)),
        ] {
            let (mut start, mut end) = initial;
            draw(
                &egui::Context::default(),
                320.0,
                &mut start,
                &mut end,
                Vec::new(),
            );
            assert_eq!((start, end), expected);
        }
    }
}

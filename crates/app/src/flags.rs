//! Small country flags, painted (the app's font has no flag emoji). Both
//! countries fly a Nordic cross, drawn in its official proportions.

use ballistics_store::Country;
use eframe::egui::{self, Color32, Rect, pos2, vec2};

/// A Nordic cross flag: field, cross, the flag's width and height in units,
/// where the vertical arm starts and ends and where the horizontal one does.
struct Nordic {
    field: Color32,
    cross: Color32,
    size: [f32; 2],
    upright: [f32; 2],
    bar: [f32; 2],
}

fn design(country: Country) -> Nordic {
    match country {
        // 5 + 2 + 9 by 4 + 2 + 4.
        Country::Sweden => Nordic {
            field: Color32::from_rgb(0x00, 0x6A, 0xA7),
            cross: Color32::from_rgb(0xFE, 0xCC, 0x02),
            size: [16.0, 10.0],
            upright: [5.0, 7.0],
            bar: [4.0, 6.0],
        },
        // 5 + 3 + 10 by 4 + 3 + 4.
        Country::Finland => Nordic {
            field: Color32::WHITE,
            cross: Color32::from_rgb(0x00, 0x2F, 0x6C),
            size: [18.0, 11.0],
            upright: [5.0, 8.0],
            bar: [4.0, 7.0],
        },
    }
}

/// How wide the flag is at `height`.
pub fn width(country: Country, height: f32) -> f32 {
    let size = design(country).size;
    height * size[0] / size[1]
}

/// Paints the flag `height` high with its left edge centred on `left`.
pub fn paint(painter: &egui::Painter, left: egui::Pos2, country: Country, height: f32) -> Rect {
    let flag = design(country);
    let rect = Rect::from_min_size(
        pos2(left.x, left.y - height / 2.0),
        vec2(width(country, height), height),
    );
    let unit = height / flag.size[1];
    painter.rect_filled(rect, 1.0, flag.field);
    painter.rect_filled(
        Rect::from_x_y_ranges(
            rect.left() + flag.upright[0] * unit..=rect.left() + flag.upright[1] * unit,
            rect.y_range(),
        ),
        0.0,
        flag.cross,
    );
    painter.rect_filled(
        Rect::from_x_y_ranges(
            rect.x_range(),
            rect.top() + flag.bar[0] * unit..=rect.top() + flag.bar[1] * unit,
        ),
        0.0,
        flag.cross,
    );
    // A hairline edge, so the white of Finland's flag shows on light ground.
    painter.rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(0.5_f32, Color32::from_black_alpha(90)),
        egui::StrokeKind::Inside,
    );
    rect
}

/// The flag as a widget, `height` high, with the country's name on hover.
pub fn show(ui: &mut egui::Ui, country: Country, height: f32) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width(country, height), height), egui::Sense::hover());
    paint(ui.painter(), rect.left_center(), country, height);
    response.on_hover_text(crate::i18n::country_name(country))
}

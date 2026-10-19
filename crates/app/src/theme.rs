//! The MX Shooting Calculator look: forest green, bone and blaze orange.
//! The logo and icon are SVG (assets/), rendered at whatever size is needed.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const FOREST_950: Color32 = Color32::from_rgb(0x0B, 0x1A, 0x11);
pub const FOREST_900: Color32 = Color32::from_rgb(0x12, 0x26, 0x1A);
pub const FOREST_800: Color32 = Color32::from_rgb(0x18, 0x30, 0x22);
pub const FOREST_700: Color32 = Color32::from_rgb(0x22, 0x40, 0x2E);
pub const FOREST_600: Color32 = Color32::from_rgb(0x2B, 0x5A, 0x3E);
pub const FOREST_500: Color32 = Color32::from_rgb(0x3A, 0x6E, 0x4E);
pub const BONE: Color32 = Color32::from_rgb(0xF1, 0xEA, 0xDB);
pub const BONE_MUTED: Color32 = Color32::from_rgb(0xB8, 0xB2, 0xA2);
pub const BLAZE: Color32 = Color32::from_rgb(0xE8, 0x70, 0x2A);
pub const BLAZE_DEEP: Color32 = Color32::from_rgb(0xB4, 0x50, 0x18);
pub const MEADOW: Color32 = Color32::from_rgb(0x7F, 0xC0, 0x8C);
pub const AMBER: Color32 = Color32::from_rgb(0xE9, 0xB9, 0x49);
pub const ALARM: Color32 = Color32::from_rgb(0xF0, 0x62, 0x4D);

#[cfg(not(target_arch = "wasm32"))]
pub const ICON_SVG: &[u8] = include_bytes!("../assets/mxsc-icon.svg");

pub fn icon() -> egui::ImageSource<'static> {
    egui::include_image!("../assets/mxsc-icon.svg")
}

pub fn wordmark() -> egui::ImageSource<'static> {
    egui::include_image!("../assets/mxsc-wordmark.svg")
}

#[cfg(not(target_arch = "wasm32"))]
/// The app icon rasterised from the SVG, for the window and dock.
pub fn window_icon() -> Option<egui::IconData> {
    let image = egui_extras::image::load_svg_bytes_with_size(
        ICON_SVG,
        Some(egui::SizeHint::Size(256, 256)),
    )
    .ok()?;
    Some(egui::IconData {
        rgba: image
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_srgba_unmultiplied())
            .collect(),
        width: image.size[0] as u32,
        height: image.size[1] as u32,
    })
}

/// A one-pixel line in a brand colour.
pub fn hairline(color: Color32) -> Stroke {
    Stroke::new(1.0_f32, color)
}

pub fn apply(context: &egui::Context) {
    egui_extras::install_image_loaders(context);
    context.set_visuals(visuals());
    context.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
    });
}

fn visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = FOREST_800;
    visuals.window_fill = FOREST_800;
    visuals.extreme_bg_color = FOREST_950;
    visuals.faint_bg_color = FOREST_700;
    visuals.code_bg_color = FOREST_900;
    visuals.hyperlink_color = Color32::from_rgb(0xF2, 0x9A, 0x5C);
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = ALARM;
    visuals.selection.bg_fill = BLAZE_DEEP;
    visuals.selection.stroke = hairline(BONE);
    visuals.window_stroke = hairline(FOREST_600);

    let radius = CornerRadius::same(4);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = FOREST_800;
    widgets.noninteractive.weak_bg_fill = FOREST_800;
    widgets.noninteractive.bg_stroke = hairline(FOREST_600);
    widgets.noninteractive.fg_stroke = hairline(BONE);
    for (state, fill) in [
        (&mut widgets.inactive, FOREST_700),
        (&mut widgets.hovered, FOREST_600),
        (&mut widgets.active, FOREST_500),
        (&mut widgets.open, FOREST_600),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = radius;
        state.fg_stroke = hairline(BONE);
    }
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.hovered.bg_stroke = hairline(BLAZE);
    widgets.active.bg_stroke = hairline(BLAZE);
    widgets.open.bg_stroke = hairline(FOREST_500);
    visuals
}

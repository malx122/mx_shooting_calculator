//! The MX Shooting Calculator look: forest green, bone and blaze orange,
//! dark (Dark) or on paper for bright daylight (Sun). The logo and icon are
//! SVG (assets/), rendered at whatever size is needed.

use eframe::egui::{self, Color32, CornerRadius, Stroke};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

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

/// The app's colours by what they are for. Painting code takes them from
/// [`palette`]; the brand constants above are for things that keep their
/// colour in every theme, like the 3D scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Text fields and plots: the deepest well, or the brightest paper.
    pub sunken: Color32,
    /// Headers, the tab bar, quiet panels and dial faces.
    pub chrome: Color32,
    /// Pages, windows and sheets.
    pub background: Color32,
    /// Tiles, list rows and widgets at rest.
    pub raised: Color32,
    /// Hovered and selected tiles and rows.
    pub hover: Color32,
    /// Pressed tiles and widgets.
    pub active: Color32,
    /// Hairlines, separators and borders.
    pub stroke: Color32,
    /// The outline of tiles, rows and widgets at rest: none in the dark.
    pub edge: Stroke,
    /// Scale lines and ticks in the app's drawings.
    pub line: Color32,
    /// Slider rails, and the handles on them at rest.
    pub rail: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    /// Blaze: the current choice, the marks that matter, the leading button.
    pub accent: Color32,
    /// Text and marks on an accent fill.
    pub on_accent: Color32,
    /// Selected widgets and text.
    pub selection: Color32,
    pub on_selection: Color32,
    pub link: Color32,
    pub good: Color32,
    pub warning: Color32,
    pub alarm: Color32,
    /// Behind a warning.
    pub warning_tint: Color32,
    /// Behind something closed or out of bounds, with its text.
    pub alarm_tint: Color32,
    pub alarm_on_tint: Color32,
    /// Dimension lines and their labels on drawings.
    pub measure: Color32,
}

pub const DARK: Palette = Palette {
    dark: true,
    sunken: FOREST_950,
    chrome: FOREST_900,
    background: FOREST_800,
    raised: FOREST_700,
    hover: FOREST_600,
    active: FOREST_500,
    stroke: FOREST_600,
    edge: Stroke::NONE,
    line: FOREST_500,
    rail: FOREST_700,
    text: BONE,
    text_muted: BONE_MUTED,
    accent: BLAZE,
    on_accent: FOREST_900,
    selection: BLAZE_DEEP,
    on_selection: BONE,
    link: Color32::from_rgb(0xF2, 0x9A, 0x5C),
    good: MEADOW,
    warning: AMBER,
    alarm: ALARM,
    warning_tint: Color32::from_rgb(0x4A, 0x3A, 0x17),
    alarm_tint: Color32::from_rgb(0x4A, 0x20, 0x20),
    alarm_on_tint: Color32::from_rgb(0xF0, 0x8A, 0x7A),
    measure: Color32::from_rgb(0x8C, 0xC0, 0xE0),
};

const SUN_STROKE: Color32 = Color32::from_rgb(0x8A, 0x83, 0x6F);

pub const SUN: Palette = Palette {
    dark: false,
    sunken: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    chrome: Color32::from_rgb(0xEC, 0xE5, 0xD2),
    background: Color32::from_rgb(0xF6, 0xF1, 0xE4),
    raised: Color32::from_rgb(0xFF, 0xFD, 0xF8),
    hover: Color32::from_rgb(0xEE, 0xE6, 0xD3),
    active: Color32::from_rgb(0xE2, 0xD7, 0xBE),
    stroke: SUN_STROKE,
    edge: Stroke {
        width: 1.0,
        color: SUN_STROKE,
    },
    line: SUN_STROKE,
    rail: SUN_STROKE,
    text: FOREST_900,
    text_muted: Color32::from_rgb(0x56, 0x60, 0x55),
    accent: Color32::from_rgb(0xA8, 0x46, 0x0C),
    on_accent: Color32::from_rgb(0xFF, 0xFB, 0xF5),
    selection: Color32::from_rgb(0x9C, 0x41, 0x0B),
    on_selection: Color32::from_rgb(0xFF, 0xFB, 0xF5),
    link: Color32::from_rgb(0x8C, 0x3A, 0x0A),
    good: Color32::from_rgb(0x2A, 0x6F, 0x38),
    warning: Color32::from_rgb(0x87, 0x58, 0x00),
    alarm: Color32::from_rgb(0xB3, 0x26, 0x1E),
    warning_tint: Color32::from_rgb(0xF4, 0xE6, 0xC2),
    alarm_tint: Color32::from_rgb(0xF6, 0xDA, 0xD3),
    alarm_on_tint: Color32::from_rgb(0xB3, 0x26, 0x1E),
    measure: Color32::from_rgb(0x2F, 0x6B, 0x8F),
};

/// The palette the app is shown in now.
pub fn palette(context: &egui::Context) -> &'static Palette {
    match context.theme() {
        egui::Theme::Dark => &DARK,
        egui::Theme::Light => &SUN,
    }
}

/// The theme chosen in the menu, kept on this device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Sun,
    /// Dark or Sun, as the device is set.
    System,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::Dark, ThemeChoice::Sun, ThemeChoice::System];

    pub fn name(self) -> &'static str {
        match self {
            ThemeChoice::Dark => tr("Dark"),
            ThemeChoice::Sun => tr("Sun"),
            ThemeChoice::System => tr("Follow the system"),
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            ThemeChoice::Dark => tr("Forest green, easy on the eyes in dim light"),
            ThemeChoice::Sun => tr("Light and high-contrast, for bright sunlight"),
            ThemeChoice::System => tr("Dark or Sun, as the device is set"),
        }
    }
}

/// Shows the app in `choice` from the next frame on.
pub fn choose(context: &egui::Context, choice: ThemeChoice) {
    context.set_theme(match choice {
        ThemeChoice::Dark => egui::ThemePreference::Dark,
        ThemeChoice::Sun => egui::ThemePreference::Light,
        ThemeChoice::System => egui::ThemePreference::System,
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub const ICON_SVG: &[u8] = include_bytes!("../assets/mxsc-icon.svg");

pub fn icon() -> egui::ImageSource<'static> {
    egui::include_image!("../assets/mxsc-icon.svg")
}

#[cfg(not(target_arch = "wasm32"))]
/// The app icon rasterised from the SVG, for the window and dock.
pub fn window_icon() -> Option<egui::IconData> {
    let image = egui_extras::image::load_svg_bytes_with_size(
        ICON_SVG,
        egui::SizeHint::Size {
            width: 256,
            height: 256,
            maintain_aspect_ratio: true,
        },
        &resvg::usvg::Options::default(),
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

/// Paints this ui's slider rails, and their handles at rest, so they show
/// against the page.
pub fn rails(ui: &mut egui::Ui) {
    ui.visuals_mut().widgets.inactive.bg_fill = palette(ui.ctx()).rail;
}

/// A one-pixel line in a brand colour.
pub fn hairline(color: Color32) -> Stroke {
    Stroke::new(1.0_f32, color)
}

/// The wordmark drawn for the palette's header.
pub fn wordmark(palette: &Palette) -> egui::ImageSource<'static> {
    if palette.dark {
        egui::include_image!("../assets/mxsc-wordmark.svg")
    } else {
        egui::include_image!("../assets/mxsc-wordmark-sun.svg")
    }
}

/// Installs both themes' looks, showing Dark until a choice is made.
pub fn apply(context: &egui::Context) {
    egui_extras::install_image_loaders(context);
    context.set_visuals_of(egui::Theme::Dark, visuals(&DARK));
    context.set_visuals_of(egui::Theme::Light, visuals(&SUN));
    choose(context, ThemeChoice::default());
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
    });
}

fn visuals(palette: &Palette) -> egui::Visuals {
    let mut visuals = if palette.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.background;
    visuals.extreme_bg_color = palette.sunken;
    visuals.faint_bg_color = palette.raised;
    visuals.code_bg_color = palette.chrome;
    visuals.hyperlink_color = palette.link;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.alarm;
    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = hairline(palette.on_selection);
    visuals.window_stroke = hairline(palette.stroke);
    if !palette.dark {
        visuals.weak_text_color = Some(palette.text_muted);
        visuals.text_cursor.stroke.color = palette.text;
    }

    let radius = CornerRadius::same(4);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = palette.background;
    widgets.noninteractive.weak_bg_fill = palette.background;
    widgets.noninteractive.bg_stroke = hairline(palette.stroke);
    widgets.noninteractive.fg_stroke = hairline(palette.text);
    for (state, fill) in [
        (&mut widgets.inactive, palette.raised),
        (&mut widgets.hovered, palette.hover),
        (&mut widgets.active, palette.active),
        (&mut widgets.open, palette.hover),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = radius;
        state.fg_stroke = hairline(palette.text);
    }
    widgets.inactive.bg_stroke = palette.edge;
    widgets.hovered.bg_stroke = hairline(palette.accent);
    widgets.active.bg_stroke = hairline(palette.accent);
    widgets.open.bg_stroke = hairline(if palette.dark {
        palette.active
    } else {
        palette.stroke
    });
    visuals
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, Color32};

    use super::*;

    /// The WCAG 2 contrast ratio of two opaque colours, from 1 to 21.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let luminance = |colour: Color32| {
            let channel = |value: u8| {
                let value = f32::from(value) / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(colour.r())
                + 0.7152 * channel(colour.g())
                + 0.0722 * channel(colour.b())
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    fn at_least(what: &str, fg: Color32, on: &str, bg: Color32, minimum: f32) {
        let ratio = contrast(fg, bg);
        assert!(
            ratio >= minimum,
            "{what} on {on}: {ratio:.2}:1, needs {minimum}:1"
        );
    }

    #[test]
    fn contrast_is_measured_as_wcag_does() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast(BONE, BONE) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn text_and_accents_stand_out_in_both_themes() {
        for (name, palette) in [("Dark", &DARK), ("Sun", &SUN)] {
            let surfaces = [
                ("background", palette.background),
                ("chrome", palette.chrome),
                ("raised", palette.raised),
            ];
            for (surface, colour) in surfaces {
                let on = format!("{name} {surface}");
                at_least("text", palette.text, &on, colour, 7.0);
                at_least("muted text", palette.text_muted, &on, colour, 4.5);
                at_least("accent", palette.accent, &on, colour, 3.0);
            }
            // Blaze text: the tab bar, the tagline, "‹ Back".
            for (surface, colour) in &surfaces[..2] {
                let on = format!("{name} {surface}");
                at_least("accent text", palette.accent, &on, *colour, 4.5);
            }
            at_least("text", palette.text, name, palette.sunken, 7.0);
            at_least(
                "text on accent",
                palette.on_accent,
                name,
                palette.accent,
                4.5,
            );
        }
    }

    #[test]
    fn sun_holds_its_text_and_signals_to_text_contrast() {
        let palette = &SUN;
        for (surface, colour) in [
            ("background", palette.background),
            ("chrome", palette.chrome),
            ("raised", palette.raised),
            ("hover", palette.hover),
            ("sunken", palette.sunken),
        ] {
            for (what, fg) in [
                ("muted text", palette.text_muted),
                ("accent", palette.accent),
                ("link", palette.link),
                ("good", palette.good),
                ("warning", palette.warning),
                ("alarm", palette.alarm),
                ("measure", palette.measure),
            ] {
                at_least(what, fg, surface, colour, 4.5);
            }
        }
        at_least(
            "selected text",
            palette.on_selection,
            "selection",
            palette.selection,
            4.5,
        );
        at_least(
            "closed",
            palette.alarm_on_tint,
            "alarm tint",
            palette.alarm_tint,
            4.5,
        );
        at_least(
            "warning",
            palette.warning,
            "warning tint",
            palette.warning_tint,
            4.5,
        );
        for (surface, colour) in [
            ("background", palette.background),
            ("chrome", palette.chrome),
            ("raised", palette.raised),
            ("hover", palette.hover),
        ] {
            at_least("outlines", palette.edge.color, surface, colour, 3.0);
            at_least("hairlines", palette.stroke, surface, colour, 3.0);
            at_least("scale lines", palette.line, surface, colour, 3.0);
            at_least("slider rails", palette.rail, surface, colour, 3.0);
        }
    }

    /// The look before the Sun theme, to keep Dark exactly as it was.
    fn dark_visuals_before_sun() -> egui::Visuals {
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

    #[test]
    fn dark_looks_as_it_did_before_sun() {
        assert_eq!(visuals(&DARK), dark_visuals_before_sun());
        assert_eq!(
            DARK.rail,
            dark_visuals_before_sun().widgets.inactive.bg_fill
        );
    }

    #[test]
    fn the_choice_picks_the_palette() {
        let context = egui::Context::default();
        apply(&context);
        assert_eq!(palette(&context), &DARK);
        choose(&context, ThemeChoice::Sun);
        assert_eq!(palette(&context), &SUN);
        assert!(!context.global_style().visuals.dark_mode);
        assert_eq!(context.global_style().visuals.panel_fill, SUN.background);
        choose(&context, ThemeChoice::Dark);
        assert_eq!(context.global_style().visuals.panel_fill, FOREST_800);
        let stored = ThemeChoice::ALL.map(|choice| serde_json::to_string(&choice).unwrap());
        assert_eq!(stored, ["\"dark\"", "\"sun\"", "\"system\""]);
    }

    /// The symbols the app writes come out on the desktop: the built-in
    /// fonts lack most of them, so eframe's system font fallback must find
    /// them. It depends on the system's fonts, so it's checked by hand:
    /// `cargo test -p mxsc symbols -- --ignored`.
    #[test]
    #[ignore = "depends on the system's fonts"]
    fn the_desktop_draws_the_symbols_the_app_uses() {
        let context = egui::Context::default();
        context.add_font_provider(std::sync::Arc::new(
            egui_system_font_provider::SystemFontProvider::new(),
        ));
        let mut output = context.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(14.0);
        let missing: String = context.fonts_mut(|fonts| {
            "↑↓←→✖↺↻⚠✔✓📍⌘🧭"
                .chars()
                .filter(|c| !fonts.has_glyph(&font, *c))
                .collect()
        });
        assert!(missing.is_empty(), "no glyph for {missing}");
    }
}

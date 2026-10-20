//! The fonts a card can use: PDF's own Helvetica (nothing embedded), and a
//! few open-licence fonts (SIL OFL, in assets/fonts with their licences)
//! that are embedded in the PDF and shown in the preview from the same
//! file, so the preview looks like the print.

use std::sync::OnceLock;

use eframe::egui;
use serde::{Deserialize, Serialize};

use super::sheet::{PT, win_ansi_char};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Font {
    #[default]
    Helvetica,
    HelveticaBold,
    BarlowCondensed,
    BarlowCondensedBold,
    BebasNeue,
    ShareTechMono,
    BlackOpsOne,
}

impl Font {
    pub const ALL: [Font; 7] = [
        Font::Helvetica,
        Font::HelveticaBold,
        Font::BarlowCondensed,
        Font::BarlowCondensedBold,
        Font::BebasNeue,
        Font::ShareTechMono,
        Font::BlackOpsOne,
    ];

    pub fn from_bold(bold: bool) -> Font {
        if bold {
            Font::HelveticaBold
        } else {
            Font::Helvetica
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Font::Helvetica => "Helvetica",
            Font::HelveticaBold => "Helvetica Bold",
            Font::BarlowCondensed => "Barlow Condensed",
            Font::BarlowCondensedBold => "Barlow Condensed Bold",
            Font::BebasNeue => "Bebas Neue",
            Font::ShareTechMono => "Share Tech Mono",
            Font::BlackOpsOne => "Black Ops One",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Font::Helvetica | Font::HelveticaBold => "Plain and clear",
            Font::BarlowCondensed | Font::BarlowCondensedBold => {
                "Narrow: big numbers where the marks are close"
            }
            Font::BebasNeue => "Tall and narrow, capitals only",
            Font::ShareTechMono => "Technical, every character as wide",
            Font::BlackOpsOne => "Stencil, military",
        }
    }

    /// The TrueType file of an embedded font; `None` for PDF's own.
    pub fn file(self) -> Option<&'static [u8]> {
        match self {
            Font::Helvetica | Font::HelveticaBold => None,
            Font::BarlowCondensed => Some(include_bytes!(
                "../../assets/fonts/BarlowCondensed-Medium.ttf"
            )),
            Font::BarlowCondensedBold => Some(include_bytes!(
                "../../assets/fonts/BarlowCondensed-Bold.ttf"
            )),
            Font::BebasNeue => Some(include_bytes!("../../assets/fonts/BebasNeue-Regular.ttf")),
            Font::ShareTechMono => Some(include_bytes!(
                "../../assets/fonts/ShareTechMono-Regular.ttf"
            )),
            Font::BlackOpsOne => Some(include_bytes!("../../assets/fonts/BlackOpsOne-Regular.ttf")),
        }
    }

    /// The font's PostScript name in the PDF.
    pub fn postscript_name(self) -> &'static str {
        match self {
            Font::Helvetica => "Helvetica",
            Font::HelveticaBold => "Helvetica-Bold",
            Font::BarlowCondensed => "BarlowCondensed-Medium",
            Font::BarlowCondensedBold => "BarlowCondensed-Bold",
            Font::BebasNeue => "BebasNeue-Regular",
            Font::ShareTechMono => "ShareTechMono-Regular",
            Font::BlackOpsOne => "BlackOpsOne-Regular",
        }
    }

    /// The font in egui, for the preview.
    pub fn family(self) -> egui::FontFamily {
        match self {
            Font::Helvetica | Font::HelveticaBold => egui::FontFamily::Proportional,
            other => egui::FontFamily::Name(other.postscript_name().into()),
        }
    }

    /// The text's width in millimetres at `size` points.
    pub fn width(self, text: &str, size: f32) -> f32 {
        let widths = self.metrics().widths;
        let units: u32 = text
            .chars()
            .map(|c| {
                super::sheet::win_ansi(c)
                    .map_or(widths[usize::from(b'?')], |code| widths[usize::from(code)])
            })
            .map(u32::from)
            .sum();
        units as f32 / 1000.0 * size * PT
    }

    pub fn metrics(self) -> &'static Metrics {
        static CELLS: [OnceLock<Metrics>; 7] = [const { OnceLock::new() }; 7];
        let index = Font::ALL.iter().position(|font| *font == self).unwrap_or(0);
        CELLS[index].get_or_init(|| Metrics::of(self))
    }
}

/// What the PDF needs to know of a font, in thousandths of its size.
pub struct Metrics {
    /// Advance widths by WinAnsi code.
    pub widths: [u16; 256],
    pub bbox: [i32; 4],
    pub ascent: i32,
    pub descent: i32,
    pub cap_height: i32,
}

impl Metrics {
    fn of(font: Font) -> Metrics {
        let Some(file) = font.file() else {
            let bold = font == Font::HelveticaBold;
            let mut widths = [0u16; 256];
            for (code, width) in widths.iter_mut().enumerate() {
                if let Some(c) = win_ansi_char(code as u8) {
                    let mut buffer = [0u8; 4];
                    *width = super::sheet::text_width(c.encode_utf8(&mut buffer), 1000.0 / PT, bold)
                        .round() as u16;
                }
            }
            return Metrics {
                widths,
                bbox: [-166, -225, 1000, 931],
                ascent: 718,
                descent: -207,
                cap_height: 718,
            };
        };
        let face = ttf_parser::Face::parse(file, 0).expect("the bundled fonts parse");
        let scale = 1000.0 / f32::from(face.units_per_em());
        let to = |value: i16| (f32::from(value) * scale).round() as i32;
        let fallback = face
            .glyph_index('?')
            .and_then(|glyph| face.glyph_hor_advance(glyph))
            .unwrap_or(500);
        let mut widths = [0u16; 256];
        for (code, width) in widths.iter_mut().enumerate() {
            let advance = win_ansi_char(code as u8)
                .and_then(|c| face.glyph_index(c))
                .and_then(|glyph| face.glyph_hor_advance(glyph))
                .unwrap_or(fallback);
            *width = (f32::from(advance) * scale).round() as u16;
        }
        let bbox = face.global_bounding_box();
        Metrics {
            widths,
            bbox: [
                to(bbox.x_min),
                to(bbox.y_min),
                to(bbox.x_max),
                to(bbox.y_max),
            ],
            ascent: to(face.ascender()),
            descent: to(face.descender()),
            cap_height: to(face.capital_height().unwrap_or(face.ascender())),
        }
    }
}

/// Makes the embedded fonts available to egui, for the previews.
pub fn install(context: &egui::Context) {
    for font in Font::ALL {
        if let Some(file) = font.file() {
            context.add_font(egui::epaint::text::FontInsert::new(
                font.postscript_name(),
                egui::FontData::from_static(file),
                vec![egui::epaint::text::InsertFontFamily {
                    family: font.family(),
                    priority: egui::epaint::text::FontPriority::Highest,
                }],
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fonts_have_widths() {
        for font in Font::ALL {
            let metrics = font.metrics();
            assert!(metrics.widths[usize::from(b'0')] > 200, "{font:?}");
            assert!(font.width("100", 10.0) > 0.0);
        }
        // Helvetica keeps the standard metrics.
        assert_eq!(Font::Helvetica.metrics().widths[usize::from(b'M')], 833);
        // The condensed font is narrower than Helvetica.
        assert!(Font::BarlowCondensed.width("500", 10.0) < Font::Helvetica.width("500", 10.0));
    }
}

//! A printable page as a list of marks in millimetres, origin at the top
//! left and y down. The same marks are painted on screen for the preview
//! and written to PDF, so what is seen is what prints.

use eframe::egui::{self, Color32, Pos2, Shape, Stroke, pos2};

/// Millimetres per typographic point.
pub const PT: f32 = 25.4 / 72.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Black,
    Grey,
    /// Thin rules and grid lines.
    Rule,
    /// A light fill, for alternate rows and areas.
    Shade,
    /// A darker fill, for areas not to shoot into.
    Block,
    Red,
    RedShade,
    Green,
    GreenShade,
    White,
}

impl Ink {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Ink::Black => [0x14, 0x14, 0x14],
            Ink::Grey => [0x6B, 0x6B, 0x6B],
            Ink::Rule => [0xA8, 0xA8, 0xA8],
            Ink::Shade => [0xEC, 0xEC, 0xEC],
            Ink::Block => [0xC8, 0xC8, 0xC8],
            Ink::Red => [0xC0, 0x24, 0x1C],
            Ink::RedShade => [0xF8, 0xDE, 0xDB],
            Ink::Green => [0x2E, 0x7D, 0x32],
            Ink::GreenShade => [0xDF, 0xF0, 0xDC],
            Ink::White => [0xFF, 0xFF, 0xFF],
        }
    }

    fn colour(self) -> Color32 {
        let [r, g, b] = self.rgb();
        Color32::from_rgb(r, g, b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A line through the points; `closed` joins the last to the first.
    Line {
        points: Vec<[f32; 2]>,
        width: f32,
        ink: Ink,
        dashed: bool,
        closed: bool,
    },
    /// A filled polygon, with the triangles that fill it when concave.
    Fill {
        points: Vec<[f32; 2]>,
        triangles: Vec<[u32; 3]>,
        ink: Ink,
    },
    /// One line of Helvetica, `size` in points, `at` on its baseline.
    Text {
        at: [f32; 2],
        text: String,
        size: f32,
        bold: bool,
        align: Align,
        ink: Ink,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub width: f32,
    pub height: f32,
    pub marks: Vec<Mark>,
}

impl Sheet {
    pub fn new(width: f32, height: f32) -> Sheet {
        Sheet {
            width,
            height,
            marks: Vec::new(),
        }
    }

    pub fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, ink: Ink) {
        self.polyline(vec![from, to], width, ink, false, false);
    }

    pub fn polyline(
        &mut self,
        points: Vec<[f32; 2]>,
        width: f32,
        ink: Ink,
        dashed: bool,
        closed: bool,
    ) {
        self.marks.push(Mark::Line {
            points,
            width,
            ink,
            dashed,
            closed,
        });
    }

    pub fn fill(&mut self, points: Vec<[f32; 2]>, ink: Ink) {
        let corners: Vec<[f64; 2]> = points
            .iter()
            .map(|&[x, y]| [f64::from(x), f64::from(y)])
            .collect();
        self.marks.push(Mark::Fill {
            triangles: crate::figure::triangulate(&corners),
            points,
            ink,
        });
    }

    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, ink: Ink) {
        self.fill(
            vec![
                [x, y],
                [x + width, y],
                [x + width, y + height],
                [x, y + height],
            ],
            ink,
        );
    }

    pub fn frame(&mut self, x: f32, y: f32, width: f32, height: f32, line: f32, ink: Ink) {
        self.polyline(
            vec![
                [x, y],
                [x + width, y],
                [x + width, y + height],
                [x, y + height],
            ],
            line,
            ink,
            false,
            true,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        at: [f32; 2],
        text: impl Into<String>,
        size: f32,
        bold: bool,
        align: Align,
        ink: Ink,
    ) {
        self.marks.push(Mark::Text {
            at,
            text: printable(&text.into()),
            size,
            bold,
            align,
            ink,
        });
    }

    /// Copies another sheet's marks onto this one, its top left at `at`.
    pub fn place(&mut self, other: &Sheet, at: [f32; 2]) {
        let shift = |[x, y]: [f32; 2]| [x + at[0], y + at[1]];
        for mark in &other.marks {
            self.marks.push(match mark {
                Mark::Line {
                    points,
                    width,
                    ink,
                    dashed,
                    closed,
                } => Mark::Line {
                    points: points.iter().copied().map(shift).collect(),
                    width: *width,
                    ink: *ink,
                    dashed: *dashed,
                    closed: *closed,
                },
                Mark::Fill {
                    points,
                    triangles,
                    ink,
                } => Mark::Fill {
                    points: points.iter().copied().map(shift).collect(),
                    triangles: triangles.clone(),
                    ink: *ink,
                },
                Mark::Text {
                    at: text_at,
                    text,
                    size,
                    bold,
                    align,
                    ink,
                } => Mark::Text {
                    at: shift(*text_at),
                    text: text.clone(),
                    size: *size,
                    bold: *bold,
                    align: *align,
                    ink: *ink,
                },
            });
        }
    }

    /// Paints the sheet, white paper and all, filling `rect`.
    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect) {
        let scale = rect.width() / self.width;
        let at = |[x, y]: [f32; 2]| pos2(rect.left() + x * scale, rect.top() + y * scale);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        for mark in &self.marks {
            match mark {
                Mark::Line {
                    points,
                    width,
                    ink,
                    dashed,
                    closed,
                } => {
                    let mut screen: Vec<Pos2> = points.iter().copied().map(at).collect();
                    if *closed && let Some(&first) = screen.first() {
                        screen.push(first);
                    }
                    let stroke = Stroke::new((width * scale).max(0.6), ink.colour());
                    if *dashed {
                        let dash = (1.2 * scale).max(2.0);
                        painter.extend(Shape::dashed_line(&screen, stroke, dash, dash * 0.8));
                    } else {
                        painter.add(Shape::line(screen, stroke));
                    }
                }
                Mark::Fill {
                    points,
                    triangles,
                    ink,
                } => {
                    let screen: Vec<Pos2> = points.iter().copied().map(at).collect();
                    if triangles.is_empty() {
                        painter.add(Shape::convex_polygon(screen, ink.colour(), Stroke::NONE));
                    } else {
                        let mut mesh = egui::Mesh::default();
                        for &point in &screen {
                            mesh.colored_vertex(point, ink.colour());
                        }
                        for &[a, b, c] in triangles {
                            mesh.add_triangle(a, b, c);
                        }
                        painter.add(Shape::mesh(mesh));
                    }
                }
                Mark::Text {
                    at: baseline,
                    text,
                    size,
                    bold,
                    align,
                    ink,
                } => {
                    let height = size * PT * scale;
                    // egui's font sits a little higher than Helvetica; this
                    // puts its baseline about where the PDF's is.
                    let anchor = at(*baseline) + egui::vec2(0.0, height * 0.22);
                    let align = match align {
                        Align::Left => egui::Align2::LEFT_BOTTOM,
                        Align::Center => egui::Align2::CENTER_BOTTOM,
                        Align::Right => egui::Align2::RIGHT_BOTTOM,
                    };
                    let font = egui::FontId::proportional(height.max(1.0) * 1.08);
                    let colour = ink.colour();
                    painter.text(anchor, align, text, font.clone(), colour);
                    if *bold {
                        // The default fonts have no bold: overprint a hair to the side.
                        painter.text(
                            anchor + egui::vec2(height * 0.04, 0.0),
                            align,
                            text,
                            font,
                            colour,
                        );
                    }
                }
            }
        }
    }
}

/// Points around a circle (or an arc of one) at `center`, angles in
/// degrees clockwise from straight up.
pub fn arc(center: [f32; 2], radius: f32, from: f32, to: f32) -> Vec<[f32; 2]> {
    let steps = (((to - from).abs() / 3.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|step| {
            let angle = (from + (to - from) * step as f32 / steps as f32).to_radians();
            [
                center[0] + radius * angle.sin(),
                center[1] - radius * angle.cos(),
            ]
        })
        .collect()
}

/// The text with characters Helvetica's encoding lacks replaced.
pub fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '≥' => out.push_str(">="),
            '≤' => out.push_str("<="),
            '→' => out.push_str("->"),
            '−' => out.push('-'),
            '↑' => out.push('^'),
            c if win_ansi(c).is_some() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// The character's code in PDF's WinAnsiEncoding, if it has one.
pub fn win_ansi(c: char) -> Option<u8> {
    match c {
        ' '..='~' | '\u{A0}'..='\u{FF}' => Some(c as u8),
        '€' => Some(0x80),
        '…' => Some(0x85),
        '’' => Some(0x92),
        '“' => Some(0x93),
        '”' => Some(0x94),
        '•' => Some(0x95),
        '–' => Some(0x96),
        '—' => Some(0x97),
        _ => None,
    }
}

/// How wide the text is in Helvetica, in millimetres.
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    let widths = if bold { &BOLD } else { &REGULAR };
    let units: u32 = text
        .chars()
        .map(|c| match win_ansi(c) {
            Some(code @ 32..=126) => u32::from(widths[usize::from(code - 32)]),
            Some(code @ 0xA0..=0xFF) => u32::from(widths[usize::from(code - 0xA0) + 95]),
            Some(0x85 | 0x97) => 1000,
            Some(0x95) => 350,
            Some(0x92) => {
                if bold {
                    278
                } else {
                    222
                }
            }
            Some(0x93 | 0x94) => {
                if bold {
                    500
                } else {
                    333
                }
            }
            _ => 556,
        })
        .sum();
    units as f32 / 1000.0 * size * PT
}

/// The text cut to `width` millimetres with an ellipsis, if it is wider.
pub fn fit(text: &str, size: f32, bold: bool, width: f32) -> String {
    if text_width(text, size, bold) <= width {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while !cut.is_empty() && text_width(&format!("{cut}…"), size, bold) > width {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

// Helvetica's advance widths (thousandths of the size) for the printable
// ASCII and Latin-1 characters, as in the standard font metrics.
const REGULAR: [u16; 191] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556,
    278, 278, 278, 278, 556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556,
    500,
];
const BOLD: [u16; 191] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    278, 333, 556, 556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333,
    333, 333, 611, 556, 278, 333, 333, 365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722,
    1000, 722, 667, 667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778,
    722, 722, 722, 722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556,
    278, 278, 278, 278, 611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611,
    556,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_helvetica() {
        // "Moose" in Helvetica: 833 + 556 + 556 + 500 + 556 = 3001.
        let width = text_width("Moose", 10.0, false);
        assert!((width - 3.001 * 10.0 * PT).abs() < 1e-4);
        assert_eq!(text_width("å", 10.0, false), text_width("a", 10.0, false));
        assert!(text_width("Moose", 10.0, true) > width);
    }

    #[test]
    fn text_is_made_printable_and_fitted() {
        assert_eq!(printable("≥ 2000 J → ok"), ">= 2000 J -> ok");
        assert_eq!(printable("Älg 3 °C ×1.05 – ok"), "Älg 3 °C ×1.05 – ok");
        let cut = fit("Federal Terminal Ascent 11.3 g", 8.0, false, 20.0);
        assert!(cut.ends_with('…') && text_width(&cut, 8.0, false) <= 20.0);
    }
}

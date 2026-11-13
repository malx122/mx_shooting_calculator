//! The cartridge as a file for 3D printing: one closed solid in
//! millimetres, standing on its head, with the caliber's name cut into the
//! side if wanted (docs/cartridges.md, "The printed copy"). The app and the
//! MCP server both save it from here, so they write the same file.

use crate::cartridge::{Cartridge, Engraving};
use crate::catalog::Caliber;

/// Faces round the axis in the printed file: a 9.6 mm rim comes out at
/// most 1 µm inside its circle.
const PRINT_SLICES: usize = 256;

/// The name cut into the printed case: how deep, the mask's cell, the
/// letters' height against the body's diameter, and the limits on it.
const ENGRAVING_DEPTH: f64 = 0.4;
const ENGRAVING_CELL: f64 = 0.15;
const LETTERS_PER_DIAMETER: f64 = 0.32;
const LETTERS_MOST: f64 = 4.5;
const LETTERS_LEAST: f64 = 1.5;

/// A cartridge ready to print.
pub struct Print {
    /// The binary STL file.
    pub stl: Vec<u8>,
    /// The name cut into the side: the caliber's own or, when that is too
    /// long, its longest alias that fits. None when no name was asked for
    /// or none fits.
    pub name: Option<String>,
}

impl Print {
    /// The file name to suggest, e.g. "308-win-cartridge.stl".
    pub fn file_name(caliber: &Caliber) -> String {
        format!("{}-cartridge.stl", caliber.key)
    }
}

/// The caliber's cartridge as an STL, with a name cut into its side in
/// `font` (a TrueType file) when given. None when the caliber has no
/// cartridge.
pub fn print(caliber: &Caliber, font: Option<&[u8]>) -> Option<Print> {
    let cartridge = caliber.cartridge.as_ref()?;
    let engraving = font.and_then(|font| caliber_engraving(caliber, font));
    let slices = match &engraving {
        // Fine enough round the body for the letters.
        Some(_) => {
            let widest = cartridge
                .case
                .rim_diameter
                .max(cartridge.case.body_diameter)
                / 2.0;
            PRINT_SLICES.max((std::f64::consts::TAU * widest / ENGRAVING_CELL).ceil() as usize)
        }
        None => PRINT_SLICES,
    };
    let stl = cartridge
        .engraved_solid(slices, engraving.as_ref().map(|(_, engraving)| engraving))
        .stl(&format!(
            "{} cartridge, mm, MX Shooting Calculator",
            caliber.name
        ));
    Some(Print {
        stl,
        name: engraving.map(|(name, _)| name),
    })
}

/// The caliber's name cut into its case, or failing that its longest
/// other name that fits ("5mm RRM" for 5mm Remington Rimfire Magnum).
fn caliber_engraving(caliber: &Caliber, font: &[u8]) -> Option<(String, Engraving)> {
    let cartridge = caliber.cartridge.as_ref()?;
    let mut aliases: Vec<&String> = caliber.aliases.iter().collect();
    aliases.sort_by_key(|alias| std::cmp::Reverse(alias.chars().count()));
    std::iter::once(&caliber.name)
        .chain(aliases)
        .find_map(|name| Some((name.clone(), name_engraving(cartridge, name, font)?)))
}

/// A name to cut into the side of the case, for the printed copy: along
/// the body from the head (reading towards the tip), as tall as fits. None
/// when the case is too small for it.
fn name_engraving(cartridge: &Cartridge, name: &str, font: &[u8]) -> Option<Engraving> {
    let case = &cartridge.case;
    let start = case.body_start + 1.5;
    let (end, narrowest) = match &case.shoulder {
        // Short of where the shoulder's rounding begins.
        Some(shoulder) => {
            let reach = crate::cartridge::rounding_reach(
                [case.body_start, case.body_diameter / 2.0],
                [shoulder.start, shoulder.diameter / 2.0],
                [shoulder.neck_start, shoulder.neck_diameter / 2.0],
                shoulder.radius,
            );
            (shoulder.start - reach - 1.0, shoulder.diameter)
        }
        None => (case.length - 1.0, case.mouth_diameter),
    };
    let room = end - start;
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let units_per_cap = f64::from(face.capital_height().unwrap_or(face.ascender()));
    let glyphs: Vec<_> = name
        .chars()
        .filter_map(|c| {
            face.glyph_index(c)
                .or_else(|| face.glyph_index(if c == '×' { 'x' } else { '?' }))
        })
        .collect();
    let advance: f64 = glyphs
        .iter()
        .map(|&glyph| f64::from(face.glyph_hor_advance(glyph).unwrap_or(0)))
        .sum();
    let mut height = (LETTERS_PER_DIAMETER * narrowest).min(LETTERS_MOST);
    if advance / units_per_cap * height > room {
        height = room * units_per_cap / advance;
    }
    if height < LETTERS_LEAST || room <= 0.0 {
        return None;
    }
    let scale = height / units_per_cap;
    let width = advance * scale;

    // The letters' outlines, in mm: x along the text, y up from its baseline.
    let mut pen = Pen {
        contours: Vec::new(),
        scale,
        x: 0.0,
    };
    for &glyph in &glyphs {
        face.outline_glyph(glyph, &mut pen);
        pen.x += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0)) * scale;
    }

    // Round the body the letters' "up" is the turning direction's opposite,
    // so they read the right way round from outside, tip up; centred on
    // their capitals.
    let low = f64::from(face.descender()) * scale;
    let high = f64::from(face.ascender()) * scale;
    let half = (high - height / 2.0).max(height / 2.0 - low);
    let cell = ENGRAVING_CELL;
    let rows = (2.0 * half / cell).ceil() as usize;
    let columns = (width / cell).ceil() as usize;
    let text_start = start + (room - width) / 2.0;
    let contours: Vec<([f64; 4], Vec<[f64; 2]>)> = pen
        .contours
        .into_iter()
        .filter(|contour| contour.len() > 2)
        .map(|contour| {
            let bounds = contour
                .iter()
                .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                    [
                        b[0].min(p[0]),
                        b[1].min(p[1]),
                        b[2].max(p[0]),
                        b[3].max(p[1]),
                    ]
                });
            (bounds, contour)
        })
        .collect();
    let mut cut = Vec::with_capacity(rows * columns);
    for row in 0..rows {
        let round = -(rows as f64) * cell / 2.0 + (row as f64 + 0.5) * cell;
        for column in 0..columns {
            let x = (column as f64 + 0.5) * cell;
            let y = height / 2.0 - round;
            cut.push(winding(&contours, x, y) != 0);
        }
    }
    Some(Engraving {
        start: text_start,
        cell,
        columns,
        rows,
        cut,
        depth: ENGRAVING_DEPTH,
    })
}

/// Collects a glyph's outline as closed polygons, its curves cut into short
/// straight pieces.
struct Pen {
    contours: Vec<Vec<[f64; 2]>>,
    scale: f64,
    x: f64,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> [f64; 2] {
        [
            self.x + f64::from(x) * self.scale,
            f64::from(y) * self.scale,
        ]
    }

    fn last(&self) -> [f64; 2] {
        *self
            .contours
            .last()
            .and_then(|c| c.last())
            .unwrap_or(&[0.0, 0.0])
    }

    fn push(&mut self, point: [f64; 2]) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(point);
        }
    }
}

impl ttf_parser::OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let point = self.at(x, y);
        self.contours.push(vec![point]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.at(x, y);
        self.push(point);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.last(), self.at(x1, y1), self.at(x, y));
        for i in 1..=8 {
            let t = f64::from(i) / 8.0;
            let u = 1.0 - t;
            self.push([
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]);
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.last(), self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        for i in 1..=10 {
            let t = f64::from(i) / 10.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push([
                a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0],
                a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1],
            ]);
        }
    }

    fn close(&mut self) {}
}

/// How many times the outlines wind round the point: inside a letter when
/// not zero.
fn winding(contours: &[([f64; 4], Vec<[f64; 2]>)], x: f64, y: f64) -> i32 {
    let mut winding = 0;
    for (bounds, contour) in contours {
        // Only an outline round the point can wind round it.
        if x < bounds[0] || x > bounds[2] || y < bounds[1] || y > bounds[3] {
            continue;
        }
        for (i, a) in contour.iter().enumerate() {
            let b = contour[(i + 1) % contour.len()];
            if a[1] <= y {
                if b[1] > y && (b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1]) > 0.0 {
                    winding += 1;
                }
            } else if b[1] <= y && (b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1]) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;

    const FONT: &[u8] = include_bytes!("../../app/assets/fonts/BarlowCondensed-Bold.ttf");

    #[test]
    fn every_caliber_gets_its_name_cut_into_its_body() {
        for caliber in &Catalog::builtin().calibers {
            let Some(cartridge) = &caliber.cartridge else {
                continue;
            };
            let (_, engraving) = caliber_engraving(caliber, FONT)
                .unwrap_or_else(|| panic!("{}: no name fits", caliber.name));
            assert!(engraving.cut.iter().any(|&cut| cut), "{}", caliber.name);
            // On the body: past where it starts, short of the shoulder.
            assert!(
                engraving.start > cartridge.case.body_start,
                "{}",
                caliber.name
            );
            let end = cartridge
                .case
                .shoulder
                .as_ref()
                .map_or(cartridge.case.length, |shoulder| shoulder.start);
            assert!(engraving.end() < end, "{}", caliber.name);
        }
        // The letters read the right way: the period of ".308 Win" sits on
        // the baseline, which is on the far side of the turning direction.
        let caliber = Catalog::builtin().caliber("308-win").unwrap();
        let (name, e) = caliber_engraving(caliber, FONT).unwrap();
        assert_eq!(name, ".308 Win");
        let cut = |row: usize, column: usize| e.cut[row * e.columns + column];
        let first = (0..e.columns)
            .find(|&c| (0..e.rows).any(|r| cut(r, c)))
            .unwrap();
        let rows: Vec<usize> = (0..e.rows).filter(|&r| cut(r, first)).collect();
        assert!(
            rows.iter().all(|&r| r > e.rows / 2),
            "the period is on the baseline: {rows:?}"
        );
    }

    #[test]
    fn a_print_is_a_binary_stl_with_its_name() {
        let caliber = Catalog::builtin().caliber("223-rem").unwrap();
        let plain = print(caliber, None).unwrap();
        assert_eq!(plain.name, None);
        let named = print(caliber, Some(FONT)).unwrap();
        assert_eq!(named.name.as_deref(), Some(".223 Rem"));
        for stl in [&plain.stl, &named.stl] {
            let triangles = u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize;
            assert_eq!(stl.len(), 84 + 50 * triangles);
        }
        assert!(named.stl.len() > plain.stl.len());
        assert_eq!(Print::file_name(caliber), "223-rem-cartridge.stl");
    }
}

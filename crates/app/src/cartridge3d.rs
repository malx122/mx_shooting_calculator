//! A cartridge in 3D, true to its drawing: the outline from the catalog
//! turned round its axis and shaded as brass, copper and the primer, with a
//! millimetre ruler beside it. Drag to turn it; double-click to put it back.

use ballistics_store::{Caliber, Cartridge, Engraving, Material};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, vec2};

use crate::i18n::{tr, trf};

/// Faces round the axis.
const SLICES: usize = 64;
const BRASS: [f32; 3] = [0.80, 0.62, 0.30];
const COPPER: [f32; 3] = [0.74, 0.42, 0.24];
const PRIMER: [f32; 3] = [0.72, 0.72, 0.70];
/// The view it opens with: a little from above and from behind, so the
/// head shows.
const YAW: f32 = 0.35;
const PITCH: f32 = 0.30;

#[derive(Clone, Copy)]
struct Turn {
    yaw: f32,
    pitch: f32,
}

/// Draws the cartridge in a box of `size` points, as large as fits.
pub fn show(ui: &mut egui::Ui, cartridge: &Cartridge, size: egui::Vec2) {
    let id = egui::Id::unique("cartridge3d");
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let mut turn = ui.data(|data| data.get_temp::<Turn>(id)).unwrap_or(Turn {
        yaw: YAW,
        pitch: PITCH,
    });
    if response.dragged() {
        let delta = response.drag_delta();
        turn.yaw += delta.x * 0.01;
        turn.pitch = (turn.pitch + delta.y * 0.01).clamp(-1.5, 1.5);
    }
    if response.double_clicked() {
        turn = Turn {
            yaw: YAW,
            pitch: PITCH,
        };
    }
    ui.data_mut(|data| data.insert_temp(id, turn));
    let inside = rect.shrink(6.0);
    if ui.is_rect_visible(rect) && inside.width() > 1.0 && inside.height() > 1.0 {
        let ink = ui.visuals().weak_text_color();
        let scene = scene(cartridge, turn, inside, ink);
        let painter = ui.painter();
        let paint_ruler = |painter: &egui::Painter| {
            painter.extend(scene.ruler.iter().cloned());
            for (at, text) in &scene.labels {
                painter.text(
                    *at,
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(LABEL_SIZE),
                    ink,
                );
            }
        };
        if scene.ruler_behind {
            paint_ruler(painter);
        }
        painter.add(Shape::mesh(scene.mesh));
        if !scene.ruler_behind {
            paint_ruler(painter);
        }
    }
    response.on_hover_text(tr("Drag to turn · double-click to reset"));
}

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

/// Saves the caliber's cartridge as an STL file for printing: a save dialog
/// on the desktop, a download in the browser, with the caliber's name cut
/// into the side when `engrave`. Returns the message to show and whether it
/// failed.
pub fn save_stl(caliber: &Caliber, engrave: bool) -> (String, bool) {
    let Some(cartridge) = &caliber.cartridge else {
        return (String::new(), true);
    };
    let engraving = engrave.then(|| caliber_engraving(caliber)).flatten();
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
    let bytes = cartridge
        .engraved_solid(slices, engraving.as_ref())
        .stl(&format!(
            "{} cartridge, mm, MX Shooting Calculator",
            caliber.name
        ));
    let (message, failed) = write_stl(caliber, &bytes);
    if engrave && engraving.is_none() && !failed && message != tr("Not saved") {
        let note = tr("The name is too long to cut into this case.");
        return (format!("{message}. {note}"), failed);
    }
    (message, failed)
}

fn write_stl(caliber: &Caliber, bytes: &[u8]) -> (String, bool) {
    let file = format!("{}-cartridge.stl", caliber.key);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&file)
            .add_filter("STL", &["stl"])
            .save_file()
        else {
            return (tr("Not saved").into(), false);
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => (trf!("Saved {}", path.display()), false),
            Err(error) => (trf!("Could not save {}: {}", path.display(), error), true),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        match crate::web::download_bytes(&file, bytes, "model/stl") {
            Ok(()) => (trf!("Saved {} in your downloads", file), false),
            Err(error) => (error, true),
        }
    }
}

/// The ruler: its gap below the cartridge, its ticks (1, 5 and 10 mm) and
/// how far beyond a 10 mm tick its number sits, all in millimetres.
const RULER_GAP: f32 = 1.5;
const TICKS: [f32; 3] = [1.0, 1.7, 2.6];
const LABEL_GAP: f32 = 2.2;
/// Points for the ruler's numbers.
const LABEL_SIZE: f32 = 10.0;
/// The 1 mm ticks only when they are at least this many points apart.
const FINE_TICKS_SPACING: f32 = 3.0;

/// The shaded cartridge and its ruler, fitted into a rect.
struct Scene {
    mesh: egui::Mesh,
    ruler: Vec<Shape>,
    labels: Vec<(Pos2, String)>,
    /// Paint the ruler before the cartridge: it is behind it.
    ruler_behind: bool,
}

/// The caliber's name cut into its case, or failing that its longest
/// other name that fits ("5mm RRM" for 5mm Remington Rimfire Magnum).
fn caliber_engraving(caliber: &Caliber) -> Option<Engraving> {
    let cartridge = caliber.cartridge.as_ref()?;
    let mut aliases: Vec<&String> = caliber.aliases.iter().collect();
    aliases.sort_by_key(|alias| std::cmp::Reverse(alias.chars().count()));
    std::iter::once(&caliber.name)
        .chain(aliases)
        .find_map(|name| name_engraving(cartridge, name))
}

/// A name to cut into the side of the case, for the printed copy: along
/// the body from the head (reading towards the tip), as tall as fits. None
/// when the case is too small for it.
fn name_engraving(cartridge: &Cartridge, name: &str) -> Option<Engraving> {
    let case = &cartridge.case;
    let start = case.body_start + 1.5;
    let (end, narrowest) = match &case.shoulder {
        // Short of where the shoulder's rounding begins.
        Some(shoulder) => {
            let reach = ballistics_store::cartridge::rounding_reach(
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
    let face =
        ttf_parser::Face::parse(crate::cards::fonts::Font::BarlowCondensedBold.file()?, 0).ok()?;
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

/// A point turned into the view: x right, y up, z towards the viewer.
fn view(turn: Turn, p: [f32; 3]) -> [f32; 3] {
    let (sy, cy) = turn.yaw.sin_cos();
    let (sp, cp) = turn.pitch.sin_cos();
    // Yaw about the vertical, then pitch about the horizontal.
    let x = p[0] * cy + p[2] * sy;
    let z = -p[0] * sy + p[2] * cy;
    let y = p[1] * cp - z * sp;
    let z = p[1] * sp + z * cp;
    [x, y, z]
}

fn scene(cartridge: &Cartridge, turn: Turn, rect: Rect, ink: Color32) -> Scene {
    let outline = cartridge.outline();
    let middle = cartridge.overall_length as f32 / 2.0;
    let light = normalized([-0.35, 0.75, 0.55]);

    // Each band between two outline points, as a ring of quads.
    struct Quad {
        corners: [([f32; 3], [f32; 3]); 4],
        color: [f32; 3],
        depth: f32,
    }
    let mut quads = Vec::new();
    let mut last = outline.start;
    for (material, points) in &outline.stretches {
        let color = match material {
            Material::Case => BRASS,
            Material::Primer => PRIMER,
            Material::Bullet => COPPER,
        };
        for &point in points {
            let (a, b) = (last, point);
            last = point;
            let (dz, dr) = ((b[0] - a[0]) as f32, (b[1] - a[1]) as f32);
            let length = dz.hypot(dr);
            if length < 1e-6 {
                continue;
            }
            // Outward, as the outline runs from the head to the tip.
            let (nz, nr) = (-dr / length, dz / length);
            for slice in 0..SLICES {
                let corner = |p: [f64; 2], step: usize| {
                    let angle = std::f32::consts::TAU * step as f32 / SLICES as f32;
                    let (s, c) = angle.sin_cos();
                    let r = p[1] as f32;
                    let position = view(turn, [p[0] as f32 - middle, r * c, r * s]);
                    let normal = view(turn, [nz, nr * c, nr * s]);
                    (position, normal)
                };
                let corners = [
                    corner(a, slice),
                    corner(b, slice),
                    corner(b, slice + 1),
                    corner(a, slice + 1),
                ];
                // Faces turned away are hidden by the ones in front.
                let mid = (slice as f32 + 0.5) / SLICES as f32 * std::f32::consts::TAU;
                let facing = view(turn, [nz, nr * mid.cos(), nr * mid.sin()]);
                if facing[2] < 0.0 {
                    continue;
                }
                let depth = corners.iter().map(|c| c.0[2]).sum::<f32>() / 4.0;
                quads.push(Quad {
                    corners,
                    color,
                    depth,
                });
            }
        }
    }
    // Back to front.
    quads.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    // The ruler, in the cartridge's own frame: along its axis, from the head
    // (0) to the tip, just below it, its ticks pointing away from it.
    let widest = outline
        .stretches
        .iter()
        .flat_map(|(_, points)| points.iter().map(|p| p[1] as f32))
        .fold(0.0, f32::max);
    let length = cartridge.overall_length as f32;
    let below = -(widest + RULER_GAP);
    let on_ruler = |along: f32, out: f32| view(turn, [along - middle, below - out, 0.0]);
    let mut marks: Vec<(f32, f32, Option<String>)> = Vec::new();
    for mm in 0..=(length.floor() as u32) {
        let (tick, label) = match mm {
            mm if mm % 10 == 0 => (TICKS[2], Some(mm.to_string())),
            mm if mm % 5 == 0 => (TICKS[1], None),
            _ => (TICKS[0], None),
        };
        marks.push((mm as f32, tick, label));
    }
    marks.push((
        length,
        TICKS[2],
        Some(format!("{:.2} mm", cartridge.overall_length)),
    ));

    // Fit the cartridge and its ruler in the rect, whichever way it is turned.
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    let mut fit = |p: [f32; 3]| {
        min = [min[0].min(p[0]), min[1].min(p[1])];
        max = [max[0].max(p[0]), max[1].max(p[1])];
    };
    for quad in &quads {
        for (p, _) in &quad.corners {
            fit(*p);
        }
    }
    for &(along, tick, ref label) in &marks {
        fit(on_ruler(along, tick));
        if label.is_some() {
            fit(on_ruler(along, tick + LABEL_GAP * 1.6));
        }
    }
    let scale = (rect.width() / (max[0] - min[0]).max(1e-3))
        .min(rect.height() / (max[1] - min[1]).max(1e-3));
    let centre = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    let to_screen = |p: [f32; 3]| -> Pos2 {
        rect.center() + vec2((p[0] - centre[0]) * scale, -(p[1] - centre[1]) * scale)
    };

    let mut mesh = egui::Mesh::default();
    for quad in &quads {
        let base = mesh.vertices.len() as u32;
        for (position, normal) in quad.corners {
            mesh.colored_vertex(to_screen(position), shade(quad.color, normal, light));
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }

    let stroke = Stroke::new(1.0, ink);
    let mm_apart = to_screen(on_ruler(1.0, 0.0)).distance(to_screen(on_ruler(0.0, 0.0)));
    let fine = mm_apart >= FINE_TICKS_SPACING;
    let mut ruler = vec![Shape::line_segment(
        [
            to_screen(on_ruler(0.0, 0.0)),
            to_screen(on_ruler(length, 0.0)),
        ],
        stroke,
    )];
    let mut labels = Vec::new();
    for (along, tick, label) in marks {
        if tick == TICKS[0] && !fine {
            continue;
        }
        ruler.push(Shape::line_segment(
            [
                to_screen(on_ruler(along, 0.0)),
                to_screen(on_ruler(along, tick)),
            ],
            stroke,
        ));
        if let Some(label) = label {
            labels.push((to_screen(on_ruler(along, tick + LABEL_GAP)), label));
        }
    }
    // The last full number would crowd the overall length: drop it if close.
    if labels.len() >= 2 {
        let [.., before, last] = labels.as_slice() else {
            unreachable!()
        };
        if before.0.distance(last.0) < LABEL_SIZE * 4.0 {
            labels.remove(labels.len() - 2);
        }
    }
    let ruler_behind = on_ruler(middle, 0.0)[2] < 0.0;
    Scene {
        mesh,
        ruler,
        labels,
        ruler_behind,
    }
}

/// Metal: some light all round, the light's side brighter, and a shine.
fn shade(color: [f32; 3], normal: [f32; 3], light: [f32; 3]) -> Color32 {
    let n = normalized(normal);
    let diffuse = dot(n, light).max(0.0);
    // The shine towards a viewer looking along -z.
    let half = normalized([light[0], light[1], light[2] + 1.0]);
    let shine = dot(n, half).max(0.0).powf(24.0) * 0.55;
    let channel = |c: f32| {
        let value = c * (0.32 + 0.75 * diffuse) + shine;
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color32::from_rgb(channel(color[0]), channel(color[1]), channel(color[2]))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-9);
    [v[0] / length, v[1] / length, v[2] / length]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::Catalog;
    use eframe::egui::pos2;

    #[test]
    fn every_caliber_gets_its_name_cut_into_its_body() {
        for caliber in &Catalog::builtin().calibers {
            let Some(cartridge) = &caliber.cartridge else {
                continue;
            };
            let engraving = caliber_engraving(caliber)
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
        let e = caliber_engraving(caliber).unwrap();
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
    fn the_cartridge_fills_its_rect_and_shows_only_faces_towards_the_viewer() {
        let caliber = Catalog::builtin().caliber("223-rem").unwrap();
        let cartridge = caliber.cartridge.as_ref().unwrap();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 120.0));
        let scene = scene(
            cartridge,
            Turn {
                yaw: YAW,
                pitch: PITCH,
            },
            rect,
            Color32::WHITE,
        );
        assert!(!scene.mesh.indices.is_empty());
        let mut bounds = scene.mesh.calc_bounds();
        for shape in &scene.ruler {
            bounds = bounds.union(shape.visual_bounding_rect());
        }
        for (at, _) in &scene.labels {
            bounds.extend_with(*at);
        }
        assert!(rect.expand(1.5).contains_rect(bounds), "{bounds:?}");
        // It fills the rect one way, less the room kept for the numbers'
        // text.
        assert!(
            (bounds.width() - rect.width()).abs() < 2.0
                || (bounds.height() - rect.height()).abs() < 10.0,
            "{bounds:?}"
        );
        // Every 10 mm numbered, up to 50, and the overall length at the tip.
        let labels: Vec<&str> = scene.labels.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(labels, ["0", "10", "20", "30", "40", "50", "57.40 mm"]);
    }
}

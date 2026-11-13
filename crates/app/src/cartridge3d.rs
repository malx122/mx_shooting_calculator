//! A cartridge in 3D, true to its drawing: the outline from the catalog
//! turned round its axis and shaded as brass, copper and the primer, with a
//! millimetre ruler beside it. Drag to turn it; double-click to put it back.

use ballistics_store::{Caliber, Cartridge, Material};
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

/// Saves the caliber's cartridge as an STL file for printing: a save dialog
/// on the desktop, a download in the browser, with the caliber's name cut
/// into the side when `engrave`. Returns the message to show and whether it
/// failed.
pub fn save_stl(caliber: &Caliber, engrave: bool) -> (String, bool) {
    let font = engrave
        .then(|| crate::cards::fonts::Font::BarlowCondensedBold.file())
        .flatten();
    let Some(print) = ballistics_store::print::print(caliber, font) else {
        return (String::new(), true);
    };
    let (message, failed) = write_stl(caliber, &print.stl);
    if engrave && print.name.is_none() && !failed && message != tr("Not saved") {
        let note = tr("The name is too long to cut into this case.");
        return (format!("{message}. {note}"), failed);
    }
    (message, failed)
}

fn write_stl(caliber: &Caliber, bytes: &[u8]) -> (String, bool) {
    let file = ballistics_store::Print::file_name(caliber);
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

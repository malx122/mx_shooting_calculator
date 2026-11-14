//! An animal's 3D body, to scale, for the target's preview: drag to turn it,
//! double-click to put it back. Its hit area and rings are a flat figure
//! through the middle of the body, showing through it.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, pos2};

use crate::figure::{Body3, Figure};

/// The view it opens with: straight from the side, as the hunter sees a
/// broadside animal and the drawing shows it.
const YAW: f32 = 0.0;
const PITCH: f32 = 0.0;
/// Below how square to the viewer (the cosine of the angle) the hit area,
/// nearly edge-on, is left out.
pub const EDGE_ON: f32 = 0.15;

#[derive(Clone, Copy)]
pub struct Turn {
    yaw: f32,
    pitch: f32,
}

impl Default for Turn {
    fn default() -> Turn {
        Turn {
            yaw: YAW,
            pitch: PITCH,
        }
    }
}

impl Turn {
    /// Whether this is the view it opens with.
    pub fn is_side(self) -> bool {
        self.yaw == YAW && self.pitch == PITCH
    }

    /// Turned by a drag of `delta` points.
    pub fn dragged(self, delta: egui::Vec2) -> Turn {
        Turn {
            yaw: self.yaw + delta.x * 0.01,
            pitch: (self.pitch + delta.y * 0.01).clamp(-0.1, 0.9),
        }
    }
}

/// Where the body's points go on screen.
struct View {
    turn: Turn,
    /// The point it turns round, in the figure's metres.
    pivot: [f64; 3],
    scale: f32,
    center: Pos2,
}

impl View {
    /// A point of the figure turned to face the viewer: x right, y up, z
    /// towards the viewer.
    fn turned(&self, [x, y, z]: [f64; 3]) -> [f32; 3] {
        let (x, y, z) = (
            (x - self.pivot[0]) as f32,
            (y - self.pivot[1]) as f32,
            (z - self.pivot[2]) as f32,
        );
        let (sin, cos) = self.turn.yaw.sin_cos();
        let (x, z) = (x * cos + z * sin, -x * sin + z * cos);
        let (sin, cos) = self.turn.pitch.sin_cos();
        let (y, z) = (y * cos - z * sin, y * sin + z * cos);
        [x, y, z]
    }

    fn screen(&self, [x, y, _]: [f32; 3]) -> Pos2 {
        pos2(
            self.center.x + x * self.scale,
            self.center.y - y * self.scale,
        )
    }
}

/// Paints the body in `rect` on a patch of `field` under `sky`, with the
/// figure's zones and rings through it; returns how many points a metre is
/// drawn.
pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    figure: &Figure,
    body: &Body3,
    turn: Turn,
    [sky, field]: [Color32; 2],
) -> f32 {
    let corners: Vec<[f64; 3]> = body
        .body
        .vertices
        .iter()
        .map(|&vertex| body.place(vertex))
        .collect();
    let [low, high] = figure.bounds;
    let ground = body.ground();
    let middle = |axis: usize| {
        let (from, to) = corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| {
                (a.min(c[axis]), b.max(c[axis]))
            });
        (from + to) / 2.0
    };
    let pivot = [middle(0), (low[1] + high[1]) / 2.0, middle(2)];
    // The same size whichever way it turns: wide enough for its longest
    // reach round the pivot.
    let reach = corners
        .iter()
        .map(|c| (c[0] - pivot[0]).hypot(c[2] - pivot[2]))
        .fold(0.0, f64::max)
        .max(1e-3);
    let margin = 24.0;
    let scale = ((rect.width() - 2.0 * margin) / (2.0 * reach) as f32)
        .min((rect.height() - 2.0 * margin) / (high[1] - low[1]).max(1e-3) as f32);
    let view = View {
        turn,
        pivot,
        scale,
        center: rect.center(),
    };

    painter.rect_filled(rect, 6.0, sky);
    // The ground: a field from its feet down, as in the flat preview, and a
    // darker disc round its feet that shows the ground's lie when turned.
    let horizon = view.screen(view.turned([pivot[0], ground, pivot[2]])).y;
    painter.rect_filled(
        Rect::from_min_max(
            pos2(rect.left(), horizon.clamp(rect.top(), rect.bottom())),
            rect.max,
        ),
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        },
        field,
    );
    let ground_points: Vec<Pos2> = (0..48)
        .map(|index| {
            let angle = index as f64 / 48.0 * std::f64::consts::TAU;
            let radius = reach * 1.1;
            view.screen(view.turned([
                pivot[0] + radius * angle.cos(),
                ground,
                pivot[2] + radius * angle.sin(),
            ]))
        })
        .collect();
    painter.add(Shape::convex_polygon(
        ground_points,
        field.gamma_multiply(0.85).to_opaque(),
        Stroke::NONE,
    ));

    let turned: Vec<[f32; 3]> = corners.iter().map(|&c| view.turned(c)).collect();
    painter.add(mesh(body, &turned, |point| view.screen(point)));

    // The zones and rings: flat, through the middle of the body, showing
    // through it; they fade as they turn edge-on.
    let facing = view.turned([pivot[0], pivot[1], pivot[2] + 1.0])[2].abs();
    if facing > EDGE_ON {
        let strength = ((facing - EDGE_ON) / (1.0 - EDGE_ON)).min(1.0);
        for part in &figure.parts[figure.animal_parts..] {
            let points = part
                .points
                .iter()
                .map(|&[x, y]| view.screen(view.turned([x, y, 0.0])))
                .collect();
            painter.add(part.faded(strength).shape(points));
        }
    }
    scale
}

/// The body's faces that face the viewer, lit and painted back to front, as
/// one mesh. `turned` holds its corners turned to face the viewer (x right,
/// y up, z towards the viewer); `screen` says where one shows.
pub fn mesh(body: &Body3, turned: &[[f32; 3]], screen: impl Fn([f32; 3]) -> Pos2) -> Shape {
    let colours = body.body.face_rgb();
    let antler = body.body.antler_mask();
    let light = {
        let [x, y, z] = [-0.4_f32, 0.8, 0.6];
        let length = (x * x + y * y + z * z).sqrt();
        [x / length, y / length, z / length]
    };
    let mut faces: Vec<(f32, [Pos2; 3], Color32)> = Vec::new();
    for (face, &[a, b, c]) in body.body.triangles.iter().enumerate() {
        if antler[face] && !body.antlers {
            continue;
        }
        let [a, b, c] = [turned[a as usize], turned[b as usize], turned[c as usize]];
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let normal = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        // Seen from in front (the viewer is towards +z).
        if normal[2] <= 0.0 {
            continue;
        }
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
            .sqrt()
            .max(1e-12);
        let lit = (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]) / length;
        let brightness = 0.55 + 0.45 * lit.max(0.0);
        let [r, g, bl] = colours[face]
            .map(|channel| (f32::from(channel) * brightness).round().clamp(0.0, 255.0) as u8);
        faces.push((
            (a[2] + b[2] + c[2]) / 3.0,
            [screen(a), screen(b), screen(c)],
            Color32::from_rgb(r, g, bl),
        ));
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut mesh = egui::Mesh::default();
    for (_, points, colour) in &faces {
        let index = mesh.vertices.len() as u32;
        for &point in points {
            mesh.colored_vertex(point, *colour);
        }
        mesh.add_triangle(index, index + 1, index + 2);
    }
    Shape::mesh(mesh)
}

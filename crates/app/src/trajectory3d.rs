//! The trajectory in 3D: the hunter, the bullet's path and the target at
//! the target range on sloping ground, drawn in perspective with egui's painter.
//! Drag to orbit, right-drag to pan, pinch or ⌘/Ctrl + scroll to zoom.
//!
//! The scene uses the solver's frame: x down range, y up, z to the right,
//! with the scope at the origin. Everything is true to scale.

use std::ops::{Add, Mul, Neg, Sub};

use ballistics_core::solver::Sample;
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::figure::Figure;
use crate::theme;

/// Height of the scope above the hunter's feet.
const EYE_HEIGHT: f64 = 1.5;
const NEAR: f64 = 0.05;
const SKY_TOP: Color32 = Color32::from_rgb(0x3E, 0x6B, 0x52);
const SKY_BOTTOM: Color32 = Color32::from_rgb(0xA9, 0xBF, 0xA2);
const GROUND: Color32 = Color32::from_rgb(0x4C, 0x6B, 0x3F);
const GRID: Color32 = Color32::from_rgb(0x3B, 0x57, 0x31);
const TROUSERS: Color32 = Color32::from_rgb(0x2E, 0x3D, 0x2A);
const SKIN: Color32 = Color32::from_rgb(0xD9, 0xB0, 0x8C);
const RIFLE: Color32 = Color32::from_rgb(0x2A, 0x22, 0x1C);

pub struct Scene<'a> {
    pub samples: &'a [Sample],
    pub target_range: f64,
    pub inclination_degrees: f64,
    pub figure: &'a Figure,
    pub wind_speed: f64,
    pub wind_from_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct V3 {
    x: f64,
    y: f64,
    z: f64,
}

const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> V3 {
        self * (1.0 / self.length().max(1e-12))
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, s: f64) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        self * -1.0
    }
}

/// An orbiting camera: it looks at `focus` from `distance` away, `yaw`
/// round from behind the shooter (positive to the right) and `pitch` up.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    yaw: f64,
    pitch: f64,
    distance: f64,
    focus: V3,
}

impl Camera {
    fn position(&self) -> V3 {
        let (yaw, pitch) = (self.yaw, self.pitch);
        self.focus
            + v3(
                -pitch.cos() * yaw.cos(),
                pitch.sin(),
                pitch.cos() * yaw.sin(),
            ) * self.distance
    }
}

#[derive(Clone, Copy)]
enum View {
    Overview,
    Behind,
    Side,
    Target,
    Above,
}

/// The slope the shot is taken along: down range, "up" off the line of
/// sight, and right.
struct Frame {
    down: V3,
    up: V3,
    right: V3,
    range: f64,
    /// Height of the target's aim point above its ground.
    aim_height: f64,
    /// The target's largest dimension.
    target_size: f64,
}

impl Frame {
    fn at(&self, along: f64, up: f64, right: f64) -> V3 {
        self.down * along + self.up * up + self.right * right
    }

    /// Ground height below the line of sight: the hunter's feet at the
    /// muzzle end, the target's at the target, a straight slope between.
    fn ground(&self, along: f64) -> f64 {
        let t = along / self.range.max(1.0);
        -(EYE_HEIGHT + (self.aim_height - EYE_HEIGHT) * t)
    }

    fn view(&self, view: View) -> Camera {
        let r = self.range;
        let middle = self.at(r * 0.5, -0.5, 0.0);
        match view {
            // High behind the hunter's right shoulder, the whole shot in view.
            View::Overview => Camera {
                yaw: 0.4,
                pitch: 0.25,
                distance: r * 0.55 + 15.0,
                focus: self.at(r * 0.4, -0.5, 0.0),
            },
            // Over the hunter's right shoulder, looking at the target.
            View::Behind => {
                let distance = r + 3.0;
                Camera {
                    yaw: (0.9 / distance).atan(),
                    pitch: (0.6 / distance).atan(),
                    distance,
                    focus: self.at(r, 0.0, 0.0),
                }
            }
            View::Side => Camera {
                yaw: std::f64::consts::FRAC_PI_2,
                pitch: 0.08,
                distance: r * 0.7 + 8.0,
                focus: middle,
            },
            // In front of the target, a little to the right: the side the
            // bullet arrives on.
            View::Target => Camera {
                yaw: 0.5,
                pitch: 0.12,
                distance: self.target_size * 2.2 + 1.0,
                focus: self.at(r, 0.0, 0.0),
            },
            View::Above => Camera {
                yaw: 0.001,
                pitch: 1.35,
                distance: r * 1.15 + 15.0,
                focus: middle,
            },
        }
    }
}

struct Projector {
    eye: V3,
    right: V3,
    up: V3,
    forward: V3,
    focal: f64,
    center: Pos2,
    /// Half the view's width and height over the focal length, with a
    /// margin: the slopes of the frustum's side planes.
    half_slopes: [f64; 2],
}

impl Projector {
    fn new(camera: &Camera, rect: Rect) -> Projector {
        let eye = camera.position();
        let forward = (camera.focus - eye).normalized();
        let right = forward.cross(v3(0.0, 1.0, 0.0)).normalized();
        let up = right.cross(forward);
        // A 50° vertical field of view.
        let focal = rect.height() as f64 / 2.0 / (25.0_f64.to_radians()).tan();
        let half_slopes = [
            rect.width() as f64 / 2.0 / focal * 1.1,
            rect.height() as f64 / 2.0 / focal * 1.1,
        ];
        Projector {
            eye,
            right,
            up,
            forward,
            focal,
            center: rect.center(),
            half_slopes,
        }
    }

    fn depth(&self, p: V3) -> f64 {
        (p - self.eye).dot(self.forward)
    }

    fn project(&self, p: V3) -> Option<Pos2> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        (z > NEAR).then(|| {
            pos2(
                self.center.x + (d.dot(self.right) / z * self.focal) as f32,
                self.center.y - (d.dot(self.up) / z * self.focal) as f32,
            )
        })
    }

    /// A segment clipped to the part in front of the camera.
    fn segment(&self, a: V3, b: V3) -> Option<[Pos2; 2]> {
        let (da, db) = (self.depth(a), self.depth(b));
        if da <= NEAR && db <= NEAR {
            return None;
        }
        let clip = |inside: V3, outside: V3, di: f64, dout: f64| {
            inside + (outside - inside) * ((di - NEAR) / (di - dout))
        };
        let (a, b) = match (da > NEAR, db > NEAR) {
            (true, true) => (a, b),
            (true, false) => (a, clip(a, b, da, db)),
            (false, true) => (clip(b, a, db, da), b),
            (false, false) => return None,
        };
        Some([self.project(a)?, self.project(b)?])
    }

    /// A planar polygon clipped to the view (Sutherland–Hodgman against
    /// the near plane and the four sides), on screen. Clipping to the sides
    /// keeps huge ground polygons from turning into huge screen coordinates.
    fn polygon(&self, corners: &[V3]) -> Option<Vec<Pos2>> {
        // Camera space: x right, y up, z forward.
        let mut points: Vec<[f64; 3]> = corners
            .iter()
            .map(|&p| {
                let d = p - self.eye;
                [d.dot(self.right), d.dot(self.up), d.dot(self.forward)]
            })
            .collect();
        let [sx, sy] = self.half_slopes;
        // Each plane as (a, offset): a point p is inside when a·p + offset ≥ 0.
        let planes = [
            ([0.0, 0.0, 1.0], -NEAR),
            ([1.0, 0.0, sx], 0.0),
            ([-1.0, 0.0, sx], 0.0),
            ([0.0, 1.0, sy], 0.0),
            ([0.0, -1.0, sy], 0.0),
        ];
        for (a, offset) in planes {
            let side = |p: [f64; 3]| a[0] * p[0] + a[1] * p[1] + a[2] * p[2] + offset;
            let mut clipped = Vec::with_capacity(points.len() + 2);
            for (index, &current) in points.iter().enumerate() {
                let next = points[(index + 1) % points.len()];
                let (dc, dn) = (side(current), side(next));
                if dc >= 0.0 {
                    clipped.push(current);
                }
                if (dc >= 0.0) != (dn >= 0.0) {
                    let t = dc / (dc - dn);
                    clipped.push([0, 1, 2].map(|k| current[k] + (next[k] - current[k]) * t));
                }
            }
            points = clipped;
            if points.len() < 3 {
                return None;
            }
        }
        Some(
            points
                .iter()
                .map(|p| {
                    pos2(
                        self.center.x + (p[0] / p[2] * self.focal) as f32,
                        self.center.y - (p[1] / p[2] * self.focal) as f32,
                    )
                })
                .collect(),
        )
    }

    /// Screen size of `metres` at a point.
    fn scale_at(&self, p: V3, metres: f64) -> f32 {
        (metres / self.depth(p).max(NEAR) * self.focal) as f32
    }
}

/// A polygon or line waiting to be painted far-to-near.
struct Face {
    depth: f64,
    shape: Shape,
}

fn shade(color: Color32, normal: V3) -> Color32 {
    let light = v3(-0.4, 0.8, 0.45).normalized();
    let brightness = 0.55 + 0.45 * normal.dot(light).max(0.0);
    let scale = |channel: u8| (channel as f64 * brightness).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(scale(color.r()), scale(color.g()), scale(color.b()))
}

/// A flat polygon. With `inside` (a point within the solid it bounds) it
/// is only drawn when it faces the camera; without, from both sides.
fn add_face(
    faces: &mut Vec<Face>,
    projector: &Projector,
    corners: &[V3],
    color: Color32,
    inside: Option<V3>,
) {
    let center =
        corners.iter().fold(v3(0.0, 0.0, 0.0), |sum, &p| sum + p) * (1.0 / corners.len() as f64);
    let mut normal = (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .normalized();
    if let Some(inside) = inside
        && normal.dot(center - inside) < 0.0
    {
        normal = -normal;
    }
    if normal.dot(projector.eye - center) < 0.0 {
        if inside.is_some() {
            return;
        }
        normal = -normal;
    }
    if let Some(points) = projector.polygon(corners) {
        faces.push(Face {
            depth: projector.depth(center),
            shape: Shape::convex_polygon(points, shade(color, normal), Stroke::NONE),
        });
    }
}

/// A box from `origin` spanning `size` along the three `axes`.
fn add_box(
    faces: &mut Vec<Face>,
    projector: &Projector,
    origin: V3,
    axes: [V3; 3],
    size: [f64; 3],
    color: Color32,
) {
    let [a, b, c] = [axes[0] * size[0], axes[1] * size[1], axes[2] * size[2]];
    let o = origin;
    let inside = o + (a + b + c) * 0.5;
    let quads = [
        [o, o + b, o + b + c, o + c],
        [o + a, o + a + c, o + a + b + c, o + a + b],
        [o, o + c, o + a + c, o + a],
        [o + b, o + a + b, o + a + b + c, o + b + c],
        [o, o + a, o + a + b, o + b],
        [o + c, o + b + c, o + a + b + c, o + a + c],
    ];
    for quad in quads {
        add_face(faces, projector, &quad, color, Some(inside));
    }
}

pub fn show(ui: &mut egui::Ui, scene: &Scene<'_>, height: f32) {
    let theta = scene.inclination_degrees.to_radians();
    let frame = Frame {
        down: v3(theta.cos(), theta.sin(), 0.0),
        up: v3(-theta.sin(), theta.cos(), 0.0),
        right: v3(0.0, 0.0, 1.0),
        range: scene.target_range.max(10.0),
        aim_height: scene.figure.aim_height,
        target_size: {
            let [low, high] = scene.figure.bounds;
            (high[0] - low[0]).max(high[1] - low[1])
        },
    };

    let id = ui.id().with("trajectory-3d-camera");
    let mut camera = ui
        .data(|data| data.get_temp::<CameraState>(id))
        .map_or_else(|| frame.view(View::Overview), |s| s.0);
    ui.horizontal(|ui| {
        for (label, view) in [
            ("Overview", View::Overview),
            ("Behind the shooter", View::Behind),
            ("From the side", View::Side),
            ("At the target", View::Target),
            ("From above", View::Above),
        ] {
            if ui.button(label).clicked() {
                camera = frame.view(view);
            }
        }
    });
    ui.weak("Drag to orbit · right-drag to pan · ⌘/Ctrl + scroll or pinch to zoom · double-click to reset");

    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    if response.double_clicked() {
        camera = frame.view(View::Overview);
    } else if response.dragged_by(egui::PointerButton::Primary) {
        let delta = response.drag_delta();
        camera.yaw += delta.x as f64 * 0.008;
        camera.pitch = (camera.pitch + delta.y as f64 * 0.008).clamp(-0.2, 1.5);
    } else if response.dragged_by(egui::PointerButton::Secondary) {
        let delta = response.drag_delta();
        let projector = Projector::new(&camera, rect);
        let metres_per_point = camera.distance / projector.focal;
        camera.focus = camera.focus - projector.right * (delta.x as f64 * metres_per_point)
            + projector.up * (delta.y as f64 * metres_per_point);
    }
    if response.hovered() {
        let zoom = ui.input(|input| input.zoom_delta()) as f64;
        if zoom != 1.0 {
            camera.distance = (camera.distance / zoom).clamp(1.5, 5000.0);
        }
    }
    ui.data_mut(|data| data.insert_temp(id, CameraState(camera)));

    let painter = ui.painter_at(rect);
    let projector = Projector::new(&camera, rect);
    paint_sky(&painter, rect);
    paint_ground(&painter, &projector, &frame);
    paint_wind(&painter, &projector, &frame, scene);

    let mut faces = Vec::new();
    add_hunter(&mut faces, &projector, &frame);
    add_figure(&mut faces, &projector, &frame, scene.figure);

    // The line of sight, dashed, from the scope to the aiming point, and
    // the bullet's path up to the target, sorted with the solids so the
    // target hides them only when they are behind it.
    let mut add_line = |a: V3, b: V3, stroke: Stroke| {
        if let Some(segment) = projector.segment(a, b) {
            faces.push(Face {
                depth: projector.depth((a + b) * 0.5),
                shape: Shape::line_segment(segment, stroke),
            });
        }
    };
    let dashes = 60;
    for index in (0..dashes).step_by(2) {
        let a = frame.at(frame.range * index as f64 / dashes as f64, 0.0, 0.0);
        let b = frame.at(frame.range * (index + 1) as f64 / dashes as f64, 0.0, 0.0);
        add_line(a, b, Stroke::new(1.2_f32, theme::BONE.gamma_multiply(0.8)));
    }
    let mut path: Vec<V3> = scene
        .samples
        .iter()
        .take_while(|sample| sample.range <= frame.range)
        .map(|sample| frame.at(sample.range, sample.drop, sample.windage))
        .collect();
    if let Some(hit) = ballistics_core::solver::interpolate(scene.samples, frame.range) {
        path.push(frame.at(frame.range, hit.drop, hit.windage));
    }
    for pair in path.windows(2) {
        add_line(pair[0], pair[1], Stroke::new(2.5_f32, theme::BLAZE));
    }

    faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    painter.extend(faces.into_iter().map(|face| face.shape));

    // Where the bullet strikes the target's plane.
    if let Some(hit) = ballistics_core::solver::interpolate(scene.samples, frame.range) {
        let point = frame.at(frame.range, hit.drop, hit.windage) - frame.down * 0.02;
        if let Some(screen) = projector.project(point) {
            let radius = projector.scale_at(point, 0.035).clamp(2.0, 10.0);
            painter.circle(
                screen,
                radius,
                theme::BLAZE,
                Stroke::new(1.0_f32, Color32::BLACK),
            );
        }
    }
    paint_distance_labels(&painter, &projector, &frame);
}

#[derive(Clone, Copy)]
struct CameraState(Camera);

fn paint_sky(painter: &egui::Painter, rect: Rect) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), SKY_TOP);
    mesh.colored_vertex(rect.right_top(), SKY_TOP);
    mesh.colored_vertex(rect.right_bottom(), SKY_BOTTOM);
    mesh.colored_vertex(rect.left_bottom(), SKY_BOTTOM);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// The ground, out to the horizon, with a grid around the shooting lane.
fn paint_ground(painter: &egui::Painter, projector: &Projector, frame: &Frame) {
    let point = |along: f64, right: f64| frame.at(along, frame.ground(along), right);
    let far = 2000.0 + frame.range;
    let plane = [
        point(-far, -far),
        point(far, -far),
        point(far, far),
        point(-far, far),
    ];
    if let Some(screen) = projector.polygon(&plane) {
        painter.add(Shape::convex_polygon(screen, GROUND, Stroke::NONE));
    }
    let (start, end) = (-20.0, frame.range + 40.0);
    let half_width = (frame.range * 0.25).clamp(20.0, 120.0);
    let cell = (frame.range / 30.0).clamp(5.0, 25.0);
    let grid = Stroke::new(1.0_f32, GRID);
    let mut along = 0.0;
    while along <= end {
        if let Some(segment) =
            projector.segment(point(along, -half_width), point(along, half_width))
        {
            painter.line_segment(segment, grid);
        }
        along += cell;
    }
    let mut right = -half_width;
    while right <= half_width + 1e-9 {
        // Broken into pieces so the near-plane clip stays accurate.
        let mut along = start;
        while along < end {
            let next = (along + cell).min(end);
            if let Some(segment) = projector.segment(point(along, right), point(next, right)) {
                painter.line_segment(segment, grid);
            }
            along = next;
        }
        right += cell;
    }
}

fn paint_distance_labels(painter: &egui::Painter, projector: &Projector, frame: &Frame) {
    let step = if frame.range > 400.0 { 100.0 } else { 50.0 };
    let mut along = step;
    let mut placed: Vec<Pos2> = Vec::new();
    while along <= frame.range + 1e-9 {
        let p = frame.at(along, frame.ground(along) + 0.05, -3.0);
        // Near the horizon labels crowd together; skip the ones that would overlap.
        if let Some(screen) = projector
            .project(p)
            .filter(|screen| placed.iter().all(|other| other.distance(*screen) > 40.0))
        {
            placed.push(screen);
            painter.text(
                screen,
                egui::Align2::RIGHT_BOTTOM,
                format!("{along:.0} m"),
                egui::FontId::proportional(12.0),
                theme::BONE,
            );
        }
        along += step;
    }
}

/// An arrow on the ground beside the hunter, blowing the way the wind blows.
fn paint_wind(painter: &egui::Painter, projector: &Projector, frame: &Frame, scene: &Scene<'_>) {
    if scene.wind_speed <= 0.0 {
        return;
    }
    let from = scene.wind_from_degrees.to_radians();
    // Unit vector the wind blows towards, in (down range, right).
    let (toward_down, toward_right) = (-from.cos(), -from.sin());
    let length = (scene.wind_speed * 1.2).clamp(1.5, 12.0);
    let center = (6.0, 4.0);
    let at = |down: f64, right: f64| {
        frame.at(
            center.0 + down,
            frame.ground(center.0 + down) + 0.03,
            center.1 + right,
        )
    };
    let tail = at(-toward_down * length / 2.0, -toward_right * length / 2.0);
    let tip = at(toward_down * length / 2.0, toward_right * length / 2.0);
    let stroke = Stroke::new(3.0_f32, theme::BONE);
    if let Some(segment) = projector.segment(tail, tip) {
        painter.line_segment(segment, stroke);
        let (side_down, side_right) = (-toward_right, toward_down);
        for sign in [-1.0, 1.0] {
            let barb = at(
                toward_down * (length / 2.0 - 1.0) + side_down * sign * 0.6,
                toward_right * (length / 2.0 - 1.0) + side_right * sign * 0.6,
            );
            if let Some(segment) = projector.segment(tip, barb) {
                painter.line_segment(segment, stroke);
            }
        }
        painter.text(
            segment[0] + vec2(0.0, 4.0),
            egui::Align2::CENTER_TOP,
            format!("wind {:.1} m/s", scene.wind_speed),
            egui::FontId::proportional(12.0),
            theme::BONE,
        );
    }
}

/// A standing hunter in a blaze vest, rifle shouldered towards the target.
fn add_hunter(faces: &mut Vec<Face>, projector: &Projector, frame: &Frame) {
    let feet = frame.ground(0.0);
    let axes = [frame.down, frame.up, frame.right];
    let part = |down: f64, up: f64, right: f64| frame.at(down, feet + up, right);
    // Legs, body, head over the rifle, and arms, from the feet up. The
    // rifle is on the right shoulder, so the body is left of the sight line.
    add_box(
        faces,
        projector,
        part(-0.12, 0.0, -0.30),
        axes,
        [0.22, 0.85, 0.16],
        TROUSERS,
    );
    add_box(
        faces,
        projector,
        part(-0.12, 0.0, -0.10),
        axes,
        [0.22, 0.85, 0.16],
        TROUSERS,
    );
    add_box(
        faces,
        projector,
        part(-0.16, 0.85, -0.32),
        axes,
        [0.3, 0.6, 0.42],
        theme::BLAZE,
    );
    add_box(
        faces,
        projector,
        part(-0.10, 1.37, -0.12),
        axes,
        [0.2, 0.24, 0.2],
        SKIN,
    );
    add_box(
        faces,
        projector,
        part(-0.02, 1.22, 0.06),
        axes,
        [0.3, 0.1, 0.1],
        theme::BLAZE_DEEP,
    );
    add_box(
        faces,
        projector,
        part(0.05, 1.22, -0.26),
        axes,
        [0.55, 0.1, 0.1],
        theme::BLAZE_DEEP,
    );
    // The rifle, along the bore just under the scope.
    add_box(
        faces,
        projector,
        frame.at(-0.15, -0.09, 0.04),
        axes,
        [1.15, 0.05, 0.05],
        RIFLE,
    );
    add_box(
        faces,
        projector,
        frame.at(0.05, -0.04, 0.035),
        axes,
        [0.32, 0.05, 0.06],
        RIFLE,
    );
}

/// The target, flat and facing the hunter (an animal broadside), with its
/// vital zone on the aiming point. Its parts stay in drawing order.
fn add_figure(faces: &mut Vec<Face>, projector: &Projector, frame: &Frame, figure: &Figure) {
    let depth = projector.depth(frame.at(frame.range, 0.0, 0.0));
    for (index, part) in figure.parts.iter().enumerate() {
        let corners: Vec<V3> = part
            .points
            .iter()
            .map(|&[x, y]| frame.at(frame.range, y, x))
            .collect();
        if let Some(points) = projector.polygon(&corners) {
            faces.push(Face {
                // Equal depths keep their order in the stable sort; later
                // parts are a hair nearer so they are painted on top.
                depth: depth - index as f64 * 1e-6,
                shape: Shape::convex_polygon(
                    points,
                    part.fill,
                    part.stroke
                        .map_or(Stroke::NONE, |colour| Stroke::new(1.0_f32, colour)),
                ),
            });
        }
    }
}

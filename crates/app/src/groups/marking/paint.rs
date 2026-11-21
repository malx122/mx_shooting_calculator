//! Drawing the canvas: the photo (straightened as a mesh), the blank grid,
//! the marks on them, and the toolbar icons.
//!
//! Marks sit on photos of any colour, so they keep one look in both themes:
//! a bright line over a dark outline, readable on white paper and on a dark
//! photo alike.

use eframe::egui::epaint::Vertex;
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
    TextureId, Vec2, pos2, vec2,
};
use groups_core::Point;

use super::geometry::{Bounds, Camera, Space};
use crate::theme;

/// A shot's ring.
pub const RING: Color32 = theme::BLAZE;
/// A cold-bore shot's ring.
pub const COLD: Color32 = Color32::from_rgb(0x8F, 0xD8, 0xFF);
/// A called flier's ring.
pub const FLIER: Color32 = theme::AMBER;
/// The aim point.
pub const AIM: Color32 = theme::MEADOW;
/// The group's centre and mean radius.
pub const STATS: Color32 = theme::BONE;
/// A suggested hole's ring.
pub const SUGGESTED: Color32 = Color32::from_rgb(0xD4, 0x8C, 0xFF);
/// Under every mark.
pub const OUTLINE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 190);
/// The label pills' fill.
const PILL: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 175);

/// A ring this small (points) or smaller is drawn at this size instead, so
/// it stays visible when zoomed out.
pub const MIN_RING_RADIUS: f32 = 3.0;
/// The ring of a shot whose size isn't known.
pub const FIXED_RING_RADIUS: f32 = 7.0;

/// The photo as a textured mesh over `region` (display units), each corner
/// of a `cells` × `cells` grid taken through the space to the photo, so a
/// straightened photo is warped on the GPU. Cells are cut at the photo's
/// edges.
pub fn photo_mesh(
    texture: TextureId,
    space: &Space,
    region: &Bounds,
    camera: &Camera,
    cells: usize,
) -> Mesh {
    let mut mesh = Mesh::with_texture(texture);
    let size = match *space {
        Space::Photo { size_px } | Space::Straightened { size_px, .. } => size_px,
        Space::Grid { .. } => return mesh,
    };
    let uv = |[x, y]: [f64; 2]| pos2((x / size[0]) as f32, (y / size[1]) as f32);
    if let Space::Photo { .. } = space {
        let photo = Bounds::new(Point::ORIGIN, Point::new(size[0], size[1]));
        if let Some(shown) = region.intersect(&photo) {
            mesh.add_rect_with_uv(
                Rect::from_min_max(camera.to_screen(shown.min), camera.to_screen(shown.max)),
                Rect::from_min_max(
                    uv([shown.min.x, shown.min.y]),
                    uv([shown.max.x, shown.max.y]),
                ),
                Color32::WHITE,
            );
        }
        return mesh;
    }
    let cells = cells.max(1);
    let corner = |i: usize, j: usize| {
        Point::new(
            region.min.x + region.width() * i as f64 / cells as f64,
            region.min.y + region.height() * j as f64 / cells as f64,
        )
    };
    for j in 0..cells {
        for i in 0..cells {
            let quad: Option<Vec<Point>> = [
                corner(i, j),
                corner(i + 1, j),
                corner(i + 1, j + 1),
                corner(i, j + 1),
            ]
            .into_iter()
            .map(|display| space.to_mark(display).map(|[x, y]| Point::new(x, y)))
            .collect();
            let Some(quad) = quad else {
                continue;
            };
            let clipped = clip_to_photo(quad, size);
            let shown: Option<Vec<Pos2>> = clipped
                .iter()
                .map(|px| space.to_display([px.x, px.y]).map(|d| camera.to_screen(d)))
                .collect();
            let Some(shown) = shown.filter(|shown| shown.len() >= 3) else {
                continue;
            };
            let first = mesh.vertices.len() as u32;
            for (px, pos) in clipped.iter().zip(shown) {
                mesh.vertices.push(Vertex {
                    pos,
                    uv: uv([px.x, px.y]),
                    color: Color32::WHITE,
                });
            }
            for k in 1..clipped.len() as u32 - 1 {
                mesh.add_triangle(first, first + k, first + k + 1);
            }
        }
    }
    mesh
}

/// The part of a convex polygon (photo pixels) on the photo.
fn clip_to_photo(polygon: Vec<Point>, size: [f64; 2]) -> Vec<Point> {
    // Each side of the photo as a·x + b·y + c ≥ 0 inside.
    let sides = [
        (1.0, 0.0, 0.0),
        (-1.0, 0.0, size[0]),
        (0.0, 1.0, 0.0),
        (0.0, -1.0, size[1]),
    ];
    sides.iter().fold(polygon, |polygon, &(a, b, c)| {
        let inside = |p: Point| a * p.x + b * p.y + c;
        let mut out = Vec::with_capacity(polygon.len() + 2);
        for (k, &end) in polygon.iter().enumerate() {
            let start = polygon[(k + polygon.len() - 1) % polygon.len()];
            let (s, e) = (inside(start), inside(end));
            if (s >= 0.0) != (e >= 0.0) {
                out.push(start + (end - start) * (s / (s - e)));
            }
            if e >= 0.0 {
                out.push(end);
            }
        }
        out
    })
}

/// Paper for the blank grid.
fn paper(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xE4, 0xDF, 0xD0)
    } else {
        Color32::from_rgb(0xFB, 0xF8, 0xEF)
    }
}

/// A blank target sheet over `extent`: lines every 10 mm, bolder every
/// 50 mm, counted from the centre.
pub fn grid(painter: &Painter, extent: &Bounds, camera: &Camera, visible: &Bounds, dark: bool) {
    painter.rect_filled(
        Rect::from_min_max(camera.to_screen(extent.min), camera.to_screen(extent.max)),
        0.0,
        paper(dark),
    );
    let Some(shown) = extent.intersect(visible) else {
        return;
    };
    let thin = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 80, 60, 70));
    let bold = Stroke::new(1.4, Color32::from_rgba_unmultiplied(30, 60, 45, 150));
    let axis = Stroke::new(1.8, Color32::from_rgba_unmultiplied(30, 60, 45, 190));
    let thin_shows = 10.0 * camera.scale >= 5.0;
    let lines = |low: f64, high: f64| {
        let first = (low / 10.0).ceil() as i64;
        let last = (high / 10.0).floor() as i64;
        first..=last
    };
    for k in lines(shown.min.x, shown.max.x) {
        let stroke = if k == 0 {
            axis
        } else if k % 5 == 0 {
            bold
        } else if thin_shows {
            thin
        } else {
            continue;
        };
        let x = 10.0 * k as f64;
        painter.line_segment(
            [
                camera.to_screen(Point::new(x, extent.min.y)),
                camera.to_screen(Point::new(x, extent.max.y)),
            ],
            stroke,
        );
    }
    for k in lines(shown.min.y, shown.max.y) {
        let stroke = if k == 0 {
            axis
        } else if k % 5 == 0 {
            bold
        } else if thin_shows {
            thin
        } else {
            continue;
        };
        let y = 10.0 * k as f64;
        painter.line_segment(
            [
                camera.to_screen(Point::new(extent.min.x, y)),
                camera.to_screen(Point::new(extent.max.x, y)),
            ],
            stroke,
        );
    }
}

/// How a shot is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotLook {
    Counted,
    ColdBore,
    Flier,
}

impl ShotLook {
    pub fn color(self) -> Color32 {
        match self {
            ShotLook::Counted => RING,
            ShotLook::ColdBore => COLD,
            ShotLook::Flier => FLIER,
        }
    }
}

/// Points round a circle, for dashed rings.
fn circle_points(centre: Pos2, radius: f32, count: usize) -> Vec<Pos2> {
    (0..count)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / count as f32;
            centre + radius * Vec2::angled(angle)
        })
        .collect()
}

pub fn dashed_circle(painter: &Painter, centre: Pos2, radius: f32, stroke: Stroke) {
    let dashes = ((radius * std::f32::consts::TAU / 7.0).round() as usize).clamp(6, 64) * 2;
    let points = circle_points(centre, radius, dashes);
    for pair in points.chunks(2) {
        if let [a, b] = pair {
            painter.line_segment([*a, *b], stroke);
        }
    }
}

/// The line widths of a ring of this radius (points), its outline's and
/// its own: thinner for small rings, so a group zoomed far out stays
/// readable.
pub fn ring_widths(radius: f32, selected: bool) -> (f32, f32) {
    let line = if radius < 6.0 {
        1.1
    } else if selected {
        2.4
    } else {
        1.8
    };
    (line + if radius < 6.0 { 1.6 } else { 2.2 }, line)
}

/// Where a shot's number goes: outside its ring in the direction `away`
/// (a unit vector, from the group's middle), clear of the ring by a few
/// points.
pub fn label_rect(centre: Pos2, radius: f32, away: Vec2, size: Vec2) -> Rect {
    let away = if away.length() > 0.5 {
        away.normalized()
    } else {
        vec2(1.0, -1.0).normalized()
    };
    let reach = 0.5 * (away.x.abs() * size.x + away.y.abs() * size.y);
    Rect::from_center_size(centre + away * (radius + 4.0 + reach), size)
}

/// A shot: its ring, a dot at its centre and its number, which goes outside
/// the ring on the side `away` from the group's middle.
pub fn shot(
    painter: &Painter,
    centre: Pos2,
    radius: f32,
    number: usize,
    look: ShotLook,
    selected: bool,
    away: Vec2,
) {
    let color = look.color();
    let radius = radius.max(MIN_RING_RADIUS);
    let (outline, line) = ring_widths(radius, selected);
    if selected {
        painter.circle_filled(centre, radius, color.gamma_multiply(0.22));
        painter.circle_stroke(centre, radius + 4.5, Stroke::new(4.5, OUTLINE));
        painter.circle_stroke(centre, radius + 4.5, Stroke::new(2.0, Color32::WHITE));
    }
    painter.circle_stroke(centre, radius, Stroke::new(outline, OUTLINE));
    let line = Stroke::new(line, color);
    match look {
        ShotLook::Flier => dashed_circle(painter, centre, radius, line),
        _ => {
            painter.circle_stroke(centre, radius, line);
        }
    }
    let dot = (radius * 0.4).min(1.5);
    painter.circle_filled(centre, dot + 1.1, OUTLINE);
    painter.circle_filled(centre, dot, color);

    let font = FontId::proportional(11.0);
    let text = number.to_string();
    let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
    let icon = if look == ShotLook::Counted { 0.0 } else { 11.0 };
    let size = vec2(galley.size().x + icon + 8.0, galley.size().y + 2.0);
    let ring = if selected { radius + 6.0 } else { radius };
    let pill = label_rect(centre, ring, away, size);
    painter.rect_filled(pill, CornerRadius::same(7), PILL);
    if selected {
        painter.rect_stroke(
            pill,
            CornerRadius::same(7),
            Stroke::new(1.2, Color32::WHITE),
            StrokeKind::Inside,
        );
    }
    let text_at = pos2(
        pill.left() + 4.0 + icon,
        pill.center().y - galley.size().y / 2.0,
    );
    if look != ShotLook::Counted {
        let icon_centre = pos2(pill.left() + 4.0 + 4.5, pill.center().y);
        let kind = if look == ShotLook::ColdBore {
            Icon::ColdBore
        } else {
            Icon::Flier
        };
        paint_icon(painter, icon_centre, 9.0, color, kind);
    }
    painter.galley(text_at, galley, Color32::WHITE);
}

/// A crosshair with an open middle, for the aim point and the loupe.
pub fn crosshair(painter: &Painter, at: Pos2, inner: f32, outer: f32, color: Color32, width: f32) {
    for direction in [
        vec2(1.0, 0.0),
        vec2(-1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, -1.0),
    ] {
        let segment = [at + direction * inner, at + direction * outer];
        painter.line_segment(segment, Stroke::new(width + 2.2, OUTLINE));
        painter.line_segment(segment, Stroke::new(width, color));
    }
}

/// The aim point: a crosshair in a circle.
pub fn aim(painter: &Painter, at: Pos2, selected: bool) {
    let r = if selected { 15.0 } else { 12.0 };
    painter.circle_stroke(at, r, Stroke::new(4.0, OUTLINE));
    painter.circle_stroke(at, r, Stroke::new(1.8, AIM));
    crosshair(painter, at, 4.0, r + 8.0, AIM, 1.8);
}

/// A tapped reference point of the scale, with its label.
pub fn handle(painter: &Painter, at: Pos2, label: &str, selected: bool) {
    let color = theme::BLAZE;
    let r = if selected { 11.0 } else { 9.0 };
    painter.circle_filled(at, r, color.gamma_multiply(0.25));
    painter.circle_stroke(at, r, Stroke::new(4.0, OUTLINE));
    painter.circle_stroke(
        at,
        r,
        Stroke::new(if selected { 2.6 } else { 1.8 }, Color32::WHITE),
    );
    crosshair(painter, at, 2.0, r - 2.0, Color32::WHITE, 1.0);
    label_pill(
        painter,
        at + vec2(r + 4.0, -r - 4.0),
        label,
        Align2::LEFT_BOTTOM,
    );
}

/// Text in a dark pill, readable on any photo.
pub fn label_pill(painter: &Painter, at: Pos2, text: &str, anchor: Align2) {
    let galley =
        painter.layout_no_wrap(text.to_owned(), FontId::proportional(11.5), Color32::WHITE);
    let size = galley.size() + vec2(10.0, 4.0);
    let rect = anchor.anchor_size(at, size);
    painter.rect_filled(rect, CornerRadius::same(6), PILL);
    painter.galley(rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
}

/// A line with a dark outline, solid or dashed.
pub fn outlined_line(
    painter: &Painter,
    points: &[Pos2],
    color: Color32,
    dashed: bool,
    closed: bool,
) {
    let mut points = points.to_vec();
    if closed && let Some(&first) = points.first() {
        points.push(first);
    }
    painter.add(Shape::line(points.clone(), Stroke::new(3.6, OUTLINE)));
    if dashed {
        painter.extend(Shape::dashed_line(
            &points,
            Stroke::new(1.6, color),
            6.0,
            4.0,
        ));
    } else {
        painter.add(Shape::line(points, Stroke::new(1.6, color)));
    }
}

/// The group's centre: a small upright cross.
pub fn centre(painter: &Painter, at: Pos2) {
    let r = 6.0;
    for segment in [
        [at - vec2(r, 0.0), at + vec2(r, 0.0)],
        [at - vec2(0.0, r), at + vec2(0.0, r)],
    ] {
        painter.line_segment(segment, Stroke::new(3.8, OUTLINE));
        painter.line_segment(segment, Stroke::new(1.6, STATS));
    }
}

/// The loupe's frame and a crosshair at its middle.
pub fn loupe_frame(painter: &Painter, rect: Rect) {
    let rounding = CornerRadius::same(10);
    painter.rect_stroke(
        rect.expand(1.5),
        rounding,
        Stroke::new(4.0, OUTLINE),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        rect,
        rounding,
        Stroke::new(2.0, Color32::WHITE),
        StrokeKind::Middle,
    );
    crosshair(painter, rect.center(), 5.0, 18.0, Color32::WHITE, 1.2);
}

/// The icons on the marking toolbar, painted so they show on every
/// platform's fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Undo,
    Redo,
    ColdBore,
    Flier,
    Delete,
    Done,
    ZoomIn,
    ZoomOut,
    Fit,
    Detect,
}

/// The undo arrow (or, mirrored, redo's) in a square from −1 to 1 with y
/// down: an arc over the top from right to left, and its head at the left
/// end pointing down, as barb, tip, barb.
pub fn arrow(redo: bool) -> (Vec<Vec2>, [Vec2; 3]) {
    let mirror = |v: Vec2| if redo { vec2(-v.x, v.y) } else { v };
    let centre = vec2(0.0, 0.2);
    let arc: Vec<Vec2> = (0..=12)
        .map(|i| {
            let degrees = 20.0 - 200.0 * i as f32 / 12.0;
            mirror(centre + 0.62 * Vec2::angled(degrees.to_radians()))
        })
        .collect();
    let tip = centre + vec2(-0.62, 0.0);
    let head = [tip + vec2(-0.3, -0.32), tip, tip + vec2(0.3, -0.32)].map(mirror);
    (arc, head)
}

/// `icon` in a square `size` points across round `at`.
pub fn paint_icon(painter: &Painter, at: Pos2, size: f32, color: Color32, icon: Icon) {
    let s = size / 2.0;
    let stroke = Stroke::new((size / 9.0).clamp(1.2, 2.2), color);
    let p = |x: f32, y: f32| at + vec2(x * s, y * s);
    match icon {
        Icon::Undo | Icon::Redo => {
            let (arc, head) = arrow(icon == Icon::Redo);
            let to_screen = |v: Vec2| p(v.x, v.y);
            painter.add(Shape::line(
                arc.into_iter().map(to_screen).collect(),
                stroke,
            ));
            painter.add(Shape::line(
                head.into_iter().map(to_screen).collect(),
                stroke,
            ));
        }
        Icon::ColdBore => {
            for i in 0..3 {
                let v = Vec2::angled(std::f32::consts::PI * i as f32 / 3.0) * s * 0.9;
                painter.line_segment([at - v, at + v], stroke);
                for end in [at + v, at - v] {
                    let back = (at - end).normalized() * s * 0.32;
                    let side = back.rot90() * 0.8;
                    painter
                        .line_segment([end + back * 0.9, end + back * 0.9 + side - back], stroke);
                    painter
                        .line_segment([end + back * 0.9, end + back * 0.9 - side - back], stroke);
                }
            }
        }
        Icon::Flier => {
            painter.line_segment([p(-0.55, -0.9), p(-0.55, 0.9)], stroke);
            painter.add(Shape::convex_polygon(
                vec![p(-0.5, -0.88), p(0.75, -0.5), p(-0.5, -0.1)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Delete => {
            painter.line_segment([p(-0.6, -0.6), p(0.6, 0.6)], stroke);
            painter.line_segment([p(0.6, -0.6), p(-0.6, 0.6)], stroke);
        }
        Icon::Done => {
            painter.add(Shape::line(
                vec![p(-0.7, 0.0), p(-0.2, 0.55), p(0.75, -0.6)],
                stroke,
            ));
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            painter.line_segment([p(-0.65, 0.0), p(0.65, 0.0)], stroke);
            if icon == Icon::ZoomIn {
                painter.line_segment([p(0.0, -0.65), p(0.0, 0.65)], stroke);
            }
        }
        Icon::Detect => {
            painter.circle_stroke(at, 0.55 * s, stroke);
            for direction in [
                vec2(1.0, 0.0),
                vec2(-1.0, 0.0),
                vec2(0.0, 1.0),
                vec2(0.0, -1.0),
            ] {
                painter.line_segment([at + direction * 0.3 * s, at + direction * s], stroke);
            }
        }
        Icon::Fit => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = p(0.7 * x, 0.7 * y);
                painter.line_segment([corner, corner - vec2(0.45 * s * x, 0.0)], stroke);
                painter.line_segment([corner, corner - vec2(0.0, 0.45 * s * y)], stroke);
            }
        }
    }
}

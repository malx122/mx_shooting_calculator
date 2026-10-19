//! A target as flat shapes in metres, for drawing at true size through the
//! scope, in the 3D view and in previews. The origin is the centre of the
//! vital (or hit) zone, where the hunter aims; x runs towards an animal's
//! head (or to the right), y up.

use ballistics_store::{Catalog, Drawing, Target, TargetShape};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};

use crate::theme;

const CIRCLE_POINTS: usize = 48;
/// Height of the aim point of a sport target on its stand, unless the
/// target is too big for it.
const STAND_HEIGHT: f64 = 1.0;

const CARD: Color32 = Color32::from_rgb(0xF4, 0xF1, 0xE8);
const PRINT: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
const STEEL: Color32 = Color32::from_rgb(0x8A, 0x91, 0x96);
const POST: Color32 = Color32::from_rgb(0x7A, 0x5C, 0x3E);

/// One shape, filled and optionally outlined.
pub struct Part {
    pub points: Vec<[f64; 2]>,
    pub fill: Color32,
    pub stroke: Option<Color32>,
    /// How to fill a concave shape, as corners of triangles; empty when
    /// the shape is convex.
    pub triangles: Vec<[u32; 3]>,
}

impl Part {
    fn new(points: Vec<[f64; 2]>, fill: Color32, stroke: Option<Color32>) -> Part {
        Part {
            triangles: triangulate(&points),
            points,
            fill,
            stroke,
        }
    }

    /// The part on screen, its corners at `points`.
    pub fn shape(&self, points: Vec<Pos2>) -> Shape {
        let stroke = self
            .stroke
            .map_or(Stroke::NONE, |colour| Stroke::new(1.0_f32, colour));
        if self.triangles.is_empty() {
            return Shape::convex_polygon(points, self.fill, stroke);
        }
        let mut mesh = egui::Mesh::default();
        for &point in &points {
            mesh.colored_vertex(point, self.fill);
        }
        for &[a, b, c] in &self.triangles {
            mesh.add_triangle(a, b, c);
        }
        // A thin edge in the fill smooths the mesh's jagged border.
        let edge = if stroke == Stroke::NONE {
            Stroke::new(0.75_f32, self.fill)
        } else {
            stroke
        };
        Shape::Vec(vec![Shape::mesh(mesh), Shape::closed_line(points, edge)])
    }
}

pub struct Figure {
    /// Back to front.
    pub parts: Vec<Part>,
    /// Height of the aim point above the ground.
    pub aim_height: f64,
    /// Size of the drawing, for previews: lowest and highest corner.
    pub bounds: [[f64; 2]; 2],
}

impl Figure {
    /// The target with its vital zone `vital_diameter_cm` across (which
    /// the hunter may have changed from the target's own).
    pub fn new(target: &Target, vital_diameter_cm: f64) -> Figure {
        let vital_radius = vital_diameter_cm / 200.0;
        let (mut parts, aim_height) = match target.shape {
            TargetShape::Animal {
                ref silhouette,
                shoulder_height_cm,
                body_length_cm,
                antlers,
            } => animal(
                Catalog::builtin().drawing(silhouette),
                shoulder_height_cm / 100.0,
                body_length_cm / 100.0,
                antlers,
            ),
            TargetShape::Rings { black_mm, .. } => {
                rings(&target.shape.ring_diameters_mm(), black_mm / 1000.0)
            }
            TargetShape::Plate { diameter_mm } => plate(diameter_mm / 1000.0),
        };
        // The zone that counts: shaded on game, outlined on sport targets.
        parts.push(if target.is_game() {
            Part::new(
                circle(vital_radius),
                theme::MEADOW.gamma_multiply(0.55),
                Some(theme::FOREST_600),
            )
        } else {
            Part::new(
                circle(vital_radius),
                Color32::TRANSPARENT,
                Some(theme::MEADOW),
            )
        });
        let mut low = [f64::INFINITY; 2];
        let mut high = [f64::NEG_INFINITY; 2];
        for point in parts.iter().flat_map(|part| &part.points) {
            for axis in 0..2 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
        }
        low[1] = low[1].min(-aim_height);
        Figure {
            parts,
            aim_height,
            bounds: [low, high],
        }
    }
}

pub fn circle(radius: f64) -> Vec<[f64; 2]> {
    (0..CIRCLE_POINTS)
        .map(|index| {
            let angle = index as f64 / CIRCLE_POINTS as f64 * std::f64::consts::TAU;
            [radius * angle.cos(), radius * angle.sin()]
        })
        .collect()
}

/// The drawing scaled to an animal `height` tall at the shoulder and
/// `length` long, around its vital point; and the vital point's height.
fn animal(drawing: &Drawing, height: f64, length: f64, antlers: bool) -> (Vec<Part>, f64) {
    let (sx, sy) = (
        length / drawing.length_m,
        height / drawing.shoulder_height_m,
    );
    let [vx, vy] = drawing.vital;
    let parts = drawing
        .parts
        .iter()
        .filter(|part| antlers || !part.antler)
        .map(|part| {
            let [r, g, b] = part.rgb();
            Part::new(
                part.points
                    .iter()
                    .map(|&[x, y]| [(x - vx) * sx, (y - vy) * sy])
                    .collect(),
                Color32::from_rgb(r, g, b),
                None,
            )
        })
        .collect();
    (parts, vy * sy)
}

/// Triangles filling a simple polygon by ear clipping, as corner indices;
/// none when it is convex (or too broken to fill).
fn triangulate(points: &[[f64; 2]]) -> Vec<[u32; 3]> {
    let n = points.len();
    if n < 4 {
        return Vec::new();
    }
    let turn = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    // Twice the signed area: positive when the corners run anticlockwise.
    let area: f64 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area < 0.0 { -1.0 } else { 1.0 };
    let convex =
        (0..n).all(|i| turn(points[i], points[(i + 1) % n], points[(i + 2) % n]) * sign >= -1e-12);
    if convex {
        return Vec::new();
    }
    let mut corners: Vec<usize> = (0..n).collect();
    let mut triangles = Vec::with_capacity(n - 2);
    while corners.len() > 3 {
        let count = corners.len();
        let ear = (0..count).find(|&i| {
            let [a, b, c] = [
                corners[(i + count - 1) % count],
                corners[i],
                corners[(i + 1) % count],
            ]
            .map(|k| points[k]);
            turn(a, b, c) * sign > 1e-15
                && corners.iter().all(|&k| {
                    let p = points[k];
                    p == a
                        || p == b
                        || p == c
                        || turn(a, b, p) * sign < 0.0
                        || turn(b, c, p) * sign < 0.0
                        || turn(c, a, p) * sign < 0.0
                })
        });
        // A flat corner (three points in a line) or a broken polygon: drop
        // the corner and carry on.
        let i = ear.unwrap_or_else(|| {
            (0..count)
                .find(|&i| {
                    let [a, b, c] = [
                        corners[(i + count - 1) % count],
                        corners[i],
                        corners[(i + 1) % count],
                    ]
                    .map(|k| points[k]);
                    turn(a, b, c).abs() <= 1e-15
                })
                .unwrap_or(0)
        });
        if ear.is_some() {
            triangles.push([
                corners[(i + count - 1) % count] as u32,
                corners[i] as u32,
                corners[(i + 1) % count] as u32,
            ]);
        }
        corners.remove(i);
    }
    triangles.push([corners[0] as u32, corners[1] as u32, corners[2] as u32]);
    triangles
}

/// A paper target on a stand: card, black, rings.
fn rings(diameters_mm: &[f64], black: f64) -> (Vec<Part>, f64) {
    let outer = diameters_mm.last().copied().unwrap_or(100.0) / 1000.0;
    let half = outer / 2.0 + (outer * 0.15).max(0.01);
    let aim_height = STAND_HEIGHT.max(half + 0.3);
    let mut parts = posts(aim_height, half, &[-half * 0.7, half * 0.7]);
    parts.push(Part::new(
        vec![[-half, -half], [half, -half], [half, half], [-half, half]],
        CARD,
        None,
    ));
    if black > 0.0 {
        parts.push(Part::new(circle(black / 2.0), PRINT, None));
    }
    for &diameter in diameters_mm {
        let radius = diameter / 2000.0;
        parts.push(Part::new(
            circle(radius),
            Color32::TRANSPARENT,
            Some(if radius * 2.0 < black { CARD } else { PRINT }),
        ));
    }
    (parts, aim_height)
}

/// A round steel plate hung on a post.
fn plate(diameter: f64) -> (Vec<Part>, f64) {
    let aim_height = STAND_HEIGHT.max(diameter / 2.0 + 0.3);
    let mut parts = posts(aim_height, 0.0, &[0.0]);
    parts.push(Part::new(circle(diameter / 2.0), STEEL, Some(PRINT)));
    (parts, aim_height)
}

/// Posts from the ground up to `top` below the aim point, at each x.
fn posts(aim_height: f64, top: f64, at: &[f64]) -> Vec<Part> {
    let width = 0.05;
    at.iter()
        .map(|&x| {
            Part::new(
                vec![
                    [x - width / 2.0, -aim_height],
                    [x + width / 2.0, -aim_height],
                    [x + width / 2.0, -top],
                    [x - width / 2.0, -top],
                ],
                POST,
                None,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::Catalog;

    /// Twice the area of a polygon, ignoring which way it runs.
    fn area(points: &[[f64; 2]]) -> f64 {
        (0..points.len())
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % points.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            .abs()
    }

    #[test]
    fn every_drawing_part_fills_completely() {
        for drawing in &Catalog::builtin().drawings {
            for (index, part) in drawing.parts.iter().enumerate() {
                let triangles = triangulate(&part.points);
                if triangles.is_empty() {
                    continue;
                }
                let filled: f64 = triangles
                    .iter()
                    .map(|corners| area(&corners.map(|k| part.points[k as usize])))
                    .sum();
                let whole = area(&part.points);
                assert!(
                    (filled - whole).abs() <= whole * 1e-6,
                    "{} part {index}: triangles cover {filled}, the shape {whole}",
                    drawing.id
                );
            }
        }
    }

    #[test]
    fn triangulates_concave_shapes_only() {
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!(triangulate(&square).is_empty());
        // An L, either way round.
        let mut l = vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        assert_eq!(triangulate(&l).len(), 4);
        l.reverse();
        assert_eq!(triangulate(&l).len(), 4);
    }

    #[test]
    fn every_catalog_target_draws_with_its_feet_on_the_ground() {
        for target in &Catalog::builtin().targets {
            let figure = Figure::new(target, target.vital_diameter_cm);
            assert!(figure.aim_height > 0.0, "{}", target.name);
            let lowest = figure.bounds[0][1];
            assert!(
                (lowest + figure.aim_height).abs() < 0.02,
                "{}: lowest {lowest}",
                target.name
            );
            for part in &figure.parts {
                assert!(part.points.len() >= 3, "{}", target.name);
            }
        }
    }

    #[test]
    fn animals_scale_to_their_size() {
        let moose = Catalog::builtin().target("moose").unwrap();
        let figure = Figure::new(moose, 30.0);
        let height = figure.bounds[1][1] - figure.bounds[0][1];
        assert!(
            (2.0..2.8).contains(&height),
            "moose with antlers {height} m tall"
        );
        // The vital zone is the last part, centred on the origin.
        let zone = figure.parts.last().unwrap();
        assert!((zone.points[0][0] - 0.15).abs() < 1e-9);
    }
}

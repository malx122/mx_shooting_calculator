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

/// A training target's rings: centre, and radius and score of each,
/// innermost first.
pub type Rings = ([f64; 2], Vec<(f64, String)>);

pub struct Figure {
    /// Back to front.
    pub parts: Vec<Part>,
    /// Height of the aim point above the ground.
    pub aim_height: f64,
    /// Size of the drawing, for previews: lowest and highest corner.
    pub bounds: [[f64; 2]; 2],
    /// The animal's hit zones, most telling first, in the figure's metres;
    /// empty when the vital zone is a plain circle.
    pub zones: Vec<(String, Vec<[f64; 2]>)>,
    pub rings: Option<Rings>,
}

/// Where a shot landing at a point falls on the target.
#[derive(Debug, PartialEq)]
pub struct Hit {
    /// The zone it lands in, such as "lungs"; `None` outside them all.
    pub zone: Option<String>,
    /// The training ring it scores, if any.
    pub score: Option<String>,
}

impl Figure {
    /// The target with its vital zone `vital_diameter_cm` across (which
    /// the hunter may have changed from the target's own).
    pub fn new(target: &Target, vital_diameter_cm: f64) -> Figure {
        let vital_radius = vital_diameter_cm / 200.0;
        let mut zones = Vec::new();
        let mut training = None;
        let (mut parts, aim_height) = match target.shape {
            TargetShape::Animal {
                ref silhouette,
                shoulder_height_cm,
                body_length_cm,
                antlers,
            } => {
                let drawing = Catalog::builtin().drawing(silhouette);
                let (height, length) = (shoulder_height_cm / 100.0, body_length_cm / 100.0);
                let place = placing(drawing, height, length);
                zones = drawing
                    .zones
                    .iter()
                    .map(|zone| {
                        let points: Vec<[f64; 2]> =
                            zone.points.iter().map(|&point| place(point)).collect();
                        (zone.name.clone(), points)
                    })
                    .collect();
                // The rings keep their printed size; only their place scales.
                training = drawing.rings.as_ref().map(|rings| {
                    let sizes: Vec<(f64, String)> = rings
                        .rings
                        .iter()
                        .map(|ring| (ring.diameter_m / 2.0, ring.score.clone()))
                        .collect();
                    (place(rings.center), sizes)
                });
                animal(drawing, height, length, antlers)
            }
            TargetShape::Rings { black_mm, .. } => {
                rings(&target.shape.ring_diameters_mm(), black_mm / 1000.0)
            }
            TargetShape::Plate { diameter_mm } => plate(diameter_mm / 1000.0),
        };
        if !zones.is_empty() {
            // Least telling first, so a heart is drawn over the area around it.
            for (index, (_, points)) in zones.iter().enumerate().rev() {
                let fill = if index == 0 && zones.len() > 1 {
                    theme::ALARM.gamma_multiply(0.55)
                } else {
                    theme::MEADOW.gamma_multiply(0.45)
                };
                parts.push(Part::new(
                    points.clone(),
                    fill,
                    Some(theme::BONE.gamma_multiply(0.8)),
                ));
            }
            if let Some((center, ref sizes)) = training {
                for &(radius, _) in sizes {
                    let points = circle(radius)
                        .into_iter()
                        .map(|[x, y]| [center[0] + x, center[1] + y])
                        .collect();
                    parts.push(Part::new(
                        points,
                        Color32::TRANSPARENT,
                        Some(theme::BONE.gamma_multiply(0.9)),
                    ));
                }
            }
            return Figure::framed(parts, aim_height, zones, training);
        }
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
        Figure::framed(parts, aim_height, zones, training)
    }

    fn framed(
        parts: Vec<Part>,
        aim_height: f64,
        zones: Vec<(String, Vec<[f64; 2]>)>,
        rings: Option<Rings>,
    ) -> Figure {
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
            zones,
            rings,
        }
    }

    /// Where a shot landing at `point` (metres from the aim point, x
    /// towards the head, y up) falls.
    pub fn hit(&self, point: [f64; 2]) -> Hit {
        let zone = self
            .zones
            .iter()
            .find(|(_, points)| ballistics_store::drawings::contains(points, point))
            .map(|(name, _)| name.clone());
        let score = self.rings.as_ref().and_then(|(center, sizes)| {
            let distance = (point[0] - center[0]).hypot(point[1] - center[1]);
            sizes
                .iter()
                .find(|(radius, _)| distance <= *radius)
                .map(|(_, score)| score.clone())
        });
        Hit { zone, score }
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

/// A point of the drawing on an animal `height` tall at the shoulder and
/// `length` long, around its vital point.
fn placing(drawing: &Drawing, height: f64, length: f64) -> impl Fn([f64; 2]) -> [f64; 2] {
    let (sx, sy) = (
        length / drawing.length_m,
        height / drawing.shoulder_height_m,
    );
    let [vx, vy] = drawing.vital;
    move |[x, y]| [(x - vx) * sx, (y - vy) * sy]
}

/// The drawing scaled to an animal `height` tall at the shoulder and
/// `length` long, around its vital point; and the vital point's height.
fn animal(drawing: &Drawing, height: f64, length: f64, antlers: bool) -> (Vec<Part>, f64) {
    let place = placing(drawing, height, length);
    let parts = drawing
        .parts
        .iter()
        .filter(|part| antlers || !part.antler)
        .map(|part| {
            let [r, g, b] = part.rgb();
            Part::new(
                part.points.iter().map(|&point| place(point)).collect(),
                Color32::from_rgb(r, g, b),
                None,
            )
        })
        .collect();
    (parts, -place([0.0, 0.0])[1])
}

/// Triangles filling a simple polygon by ear clipping, as corner indices;
/// none when it is convex (or too broken to fill).
pub fn triangulate(points: &[[f64; 2]]) -> Vec<[u32; 3]> {
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
        // The moose has the test figure's hit area around the aim point.
        assert_eq!(figure.zones.len(), 1);
        assert_eq!(figure.hit([0.0, 0.0]).zone.as_deref(), Some("hit area"));
    }

    #[test]
    fn a_shot_lands_in_a_zone_and_a_ring() {
        let moose = Catalog::builtin().target("moose").unwrap();
        let figure = Figure::new(moose, moose.vital_diameter_cm);
        let centre = figure.hit([0.0, 0.0]);
        assert_eq!(centre.score.as_deref(), Some("5¹"));
        // 19 cm high: the outer ring, inside the area.
        let high = figure.hit([0.0, 0.19]);
        assert_eq!(high.score.as_deref(), Some("3"));
        assert_eq!(high.zone.as_deref(), Some("hit area"));
        // 30 cm back and 18 up: the area's tongue towards the spine, past the rings.
        let tongue = figure.hit([-0.30, 0.18]);
        assert_eq!(tongue.zone.as_deref(), Some("hit area"));
        assert_eq!(tongue.score, None);
        // Far back on the body: no zone, no ring.
        assert_eq!(
            figure.hit([-0.8, 0.1]),
            Hit {
                zone: None,
                score: None
            }
        );
        // A deer without zones keeps its circle.
        let roe = Catalog::builtin().target("roe-deer").unwrap();
        assert!(Figure::new(roe, roe.vital_diameter_cm).zones.is_empty());
    }
}

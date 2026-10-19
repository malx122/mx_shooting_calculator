//! A target as flat, convex shapes in metres, for drawing at true size
//! through the scope, in the 3D view and in previews. The origin is the
//! centre of the vital (or hit) zone, where the hunter aims; x runs towards
//! an animal's head (or to the right), y up.

use ballistics_store::{Silhouette, Target, TargetShape};
use eframe::egui::Color32;

use crate::theme;

const CIRCLE_POINTS: usize = 48;
/// Height of the aim point of a sport target on its stand, unless the
/// target is too big for it.
const STAND_HEIGHT: f64 = 1.0;

const MOOSE: Color32 = Color32::from_rgb(0x2E, 0x22, 0x1A);
const DEER: Color32 = Color32::from_rgb(0x6B, 0x45, 0x2C);
const BOAR: Color32 = Color32::from_rgb(0x3A, 0x33, 0x2D);
const BEAR: Color32 = Color32::from_rgb(0x4A, 0x34, 0x24);
const FOX: Color32 = Color32::from_rgb(0xB8, 0x5A, 0x24);
const HARE: Color32 = Color32::from_rgb(0x8C, 0x76, 0x5E);
const GROUSE: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2E);
const PTARMIGAN: Color32 = Color32::from_rgb(0xA8, 0x94, 0x74);

/// A grouse standing broadside, beak to the right: tail, legs, body, neck,
/// head and beak. 0.85 m from beak to tail, 0.41 m to the top of the back.
const BIRD: &[&[[f64; 2]]] = &[
    &[[-0.24, 0.22], [-0.44, 0.24], [-0.47, 0.38], [-0.27, 0.34]],
    &[[0.0, 0.0], [0.025, 0.0], [0.035, 0.11], [0.01, 0.11]],
    &[[0.07, 0.0], [0.095, 0.0], [0.1, 0.11], [0.075, 0.11]],
    &[
        [-0.28, 0.3],
        [-0.25, 0.14],
        [-0.05, 0.08],
        [0.15, 0.1],
        [0.26, 0.2],
        [0.24, 0.32],
        [0.1, 0.4],
        [-0.12, 0.41],
    ],
    &[[0.16, 0.3], [0.25, 0.24], [0.31, 0.46], [0.23, 0.5]],
    &[
        [0.23, 0.47],
        [0.31, 0.44],
        [0.37, 0.48],
        [0.36, 0.55],
        [0.28, 0.57],
        [0.23, 0.53],
    ],
    &[[0.36, 0.48], [0.41, 0.5], [0.365, 0.53]],
];
const ANTLER: Color32 = Color32::from_rgb(0xC8, 0xB8, 0x96);
const CARD: Color32 = Color32::from_rgb(0xF4, 0xF1, 0xE8);
const PRINT: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
const STEEL: Color32 = Color32::from_rgb(0x8A, 0x91, 0x96);
const POST: Color32 = Color32::from_rgb(0x7A, 0x5C, 0x3E);

/// One convex shape, filled and optionally outlined.
pub struct Part {
    pub points: Vec<[f64; 2]>,
    pub fill: Color32,
    pub stroke: Option<Color32>,
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
                silhouette,
                shoulder_height_cm,
                body_length_cm,
                antlers,
            } => animal(
                silhouette,
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
            Part {
                points: circle(vital_radius),
                fill: theme::MEADOW.gamma_multiply(0.55),
                stroke: Some(theme::FOREST_600),
            }
        } else {
            Part {
                points: circle(vital_radius),
                fill: Color32::TRANSPARENT,
                stroke: Some(theme::MEADOW),
            }
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

/// A drawing of a reference animal, in metres from its feet: x towards the
/// head, y up.
struct Outline {
    shoulder_height: f64,
    length: f64,
    /// Centre of the heart–lung area.
    vital: [f64; 2],
    colour: Color32,
    body: &'static [&'static [[f64; 2]]],
    /// Antler beams as (from, to, width), drawn only with antlers on.
    antlers: &'static [([f64; 2], [f64; 2], f64)],
    antler_plates: &'static [&'static [[f64; 2]]],
}

fn outline(silhouette: Silhouette) -> Outline {
    match silhouette {
        Silhouette::Moose => Outline {
            shoulder_height: 1.85,
            length: 3.1,
            vital: [0.55, 1.25],
            colour: MOOSE,
            body: &[
                &[[-1.05, 1.0], [1.0, 0.95], [1.15, 1.75], [-0.95, 1.65]],
                &[[0.3, 1.6], [1.15, 1.75], [1.05, 1.98], [0.45, 1.88]],
                &[[0.95, 1.4], [1.35, 1.55], [1.55, 1.95], [1.1, 1.97]],
                &[[1.4, 1.72], [1.95, 1.5], [2.08, 1.66], [1.62, 2.0]],
                &[[0.75, 0.0], [0.85, 0.0], [0.97, 1.0], [0.72, 1.0]],
                &[[0.5, 0.0], [0.6, 0.0], [0.72, 1.0], [0.5, 1.0]],
                &[[-0.9, 0.0], [-0.8, 0.0], [-0.72, 1.05], [-1.0, 1.05]],
                &[[-0.65, 0.0], [-0.55, 0.0], [-0.5, 1.0], [-0.76, 1.0]],
            ],
            antlers: &[],
            antler_plates: &[
                &[[1.5, 1.95], [1.28, 2.28], [1.72, 2.38], [1.72, 2.02]],
                &[[1.58, 1.98], [1.85, 2.3], [2.0, 2.22], [1.78, 1.98]],
            ],
        },
        Silhouette::Deer => Outline {
            shoulder_height: 1.2,
            length: 2.1,
            vital: [0.38, 0.9],
            colour: DEER,
            body: &[
                &[[-0.8, 0.68], [0.55, 0.66], [0.62, 1.16], [-0.75, 1.12]],
                &[[0.35, 1.0], [0.62, 0.9], [0.98, 1.42], [0.74, 1.56]],
                &[
                    [0.78, 1.44],
                    [1.02, 1.34],
                    [1.25, 1.29],
                    [1.26, 1.39],
                    [0.96, 1.6],
                ],
                &[[0.8, 1.55], [0.86, 1.52], [0.72, 1.68]],
                &[[0.38, 0.0], [0.45, 0.0], [0.52, 0.72], [0.36, 0.72]],
                &[[0.22, 0.0], [0.29, 0.0], [0.33, 0.7], [0.2, 0.7]],
                &[[-0.62, 0.0], [-0.55, 0.0], [-0.48, 0.78], [-0.72, 0.78]],
                &[[-0.46, 0.0], [-0.39, 0.0], [-0.36, 0.74], [-0.52, 0.74]],
            ],
            antlers: &[
                ([0.86, 1.56], [0.72, 1.85], 0.04),
                ([0.72, 1.85], [0.62, 2.12], 0.035),
                ([0.84, 1.62], [1.0, 1.66], 0.03),
                ([0.74, 1.8], [0.92, 1.88], 0.03),
                ([0.67, 1.98], [0.85, 2.08], 0.03),
                ([0.62, 2.12], [0.72, 2.25], 0.03),
                ([0.62, 2.12], [0.52, 2.22], 0.03),
            ],
            antler_plates: &[],
        },
        Silhouette::Boar => Outline {
            shoulder_height: 0.85,
            length: 1.6,
            vital: [0.22, 0.5],
            colour: BOAR,
            body: &[
                &[
                    [-0.65, 0.35],
                    [0.45, 0.3],
                    [0.55, 0.8],
                    [0.1, 0.9],
                    [-0.6, 0.72],
                ],
                &[[0.4, 0.38], [0.95, 0.36], [0.95, 0.5], [0.55, 0.84]],
                &[[0.48, 0.78], [0.56, 0.76], [0.5, 0.93]],
                &[[0.25, 0.0], [0.33, 0.0], [0.36, 0.36], [0.22, 0.36]],
                &[[0.1, 0.0], [0.18, 0.0], [0.2, 0.36], [0.08, 0.36]],
                &[[-0.5, 0.0], [-0.42, 0.0], [-0.38, 0.42], [-0.56, 0.42]],
                &[[-0.36, 0.0], [-0.28, 0.0], [-0.26, 0.38], [-0.4, 0.38]],
            ],
            antlers: &[],
            antler_plates: &[],
        },
        Silhouette::Bear => Outline {
            shoulder_height: 1.1,
            length: 2.0,
            vital: [0.3, 0.7],
            colour: BEAR,
            body: &[
                &[
                    [-0.82, 0.45],
                    [0.55, 0.4],
                    [0.62, 1.02],
                    [0.15, 1.12],
                    [-0.78, 0.95],
                ],
                &[[0.45, 0.55], [0.85, 0.68], [0.88, 0.95], [0.5, 1.05]],
                &[
                    [0.75, 0.68],
                    [1.15, 0.7],
                    [1.13, 0.84],
                    [0.92, 1.0],
                    [0.76, 0.95],
                ],
                &[[0.82, 0.96], [0.9, 0.98], [0.84, 1.07]],
                &[[0.3, 0.0], [0.5, 0.0], [0.53, 0.5], [0.28, 0.5]],
                &[[0.12, 0.0], [0.3, 0.0], [0.32, 0.5], [0.1, 0.5]],
                &[[-0.72, 0.0], [-0.52, 0.0], [-0.46, 0.56], [-0.78, 0.56]],
                &[[-0.52, 0.0], [-0.34, 0.0], [-0.3, 0.5], [-0.52, 0.5]],
            ],
            antlers: &[],
            antler_plates: &[],
        },
        Silhouette::Fox => Outline {
            shoulder_height: 0.4,
            length: 0.75,
            vital: [0.06, 0.28],
            colour: FOX,
            body: &[
                &[[-0.28, 0.33], [-0.32, 0.22], [-0.7, 0.12], [-0.76, 0.18]],
                &[[-0.3, 0.2], [0.18, 0.19], [0.21, 0.37], [-0.3, 0.35]],
                &[
                    [0.18, 0.35],
                    [0.32, 0.325],
                    [0.45, 0.31],
                    [0.43, 0.36],
                    [0.3, 0.46],
                    [0.2, 0.45],
                ],
                &[[0.24, 0.44], [0.3, 0.44], [0.27, 0.53]],
                &[[0.12, 0.0], [0.16, 0.0], [0.17, 0.22], [0.11, 0.22]],
                &[[0.05, 0.0], [0.09, 0.0], [0.1, 0.22], [0.04, 0.22]],
                &[[-0.27, 0.0], [-0.23, 0.0], [-0.2, 0.24], [-0.29, 0.24]],
                &[[-0.2, 0.0], [-0.16, 0.0], [-0.14, 0.22], [-0.21, 0.22]],
            ],
            antlers: &[],
            antler_plates: &[],
        },
        Silhouette::Hare => Outline {
            shoulder_height: 0.3,
            length: 0.55,
            vital: [0.03, 0.17],
            colour: HARE,
            body: &[
                &[[0.11, 0.29], [0.15, 0.3], [0.07, 0.5], [0.03, 0.49]],
                &[[-0.24, 0.0], [-0.06, 0.0], [-0.1, 0.07], [-0.26, 0.09]],
                &[
                    [-0.3, 0.17],
                    [-0.27, 0.08],
                    [0.1, 0.06],
                    [0.16, 0.18],
                    [0.02, 0.3],
                    [-0.19, 0.29],
                ],
                &[[0.07, 0.0], [0.12, 0.0], [0.13, 0.12], [0.06, 0.12]],
                &[
                    [0.09, 0.2],
                    [0.2, 0.17],
                    [0.27, 0.21],
                    [0.23, 0.29],
                    [0.13, 0.31],
                ],
                &[[-0.31, 0.19], [-0.27, 0.2], [-0.29, 0.25], [-0.33, 0.24]],
            ],
            antlers: &[],
            antler_plates: &[],
        },
        Silhouette::Grouse | Silhouette::Ptarmigan => Outline {
            shoulder_height: 0.41,
            length: 0.88,
            vital: [0.1, 0.24],
            colour: if silhouette == Silhouette::Grouse {
                GROUSE
            } else {
                PTARMIGAN
            },
            body: BIRD,
            antlers: &[],
            antler_plates: &[],
        },
    }
}

/// Whether the silhouette can carry antlers.
pub fn has_antlers(silhouette: Silhouette) -> bool {
    let outline = outline(silhouette);
    !outline.antlers.is_empty() || !outline.antler_plates.is_empty()
}

fn animal(silhouette: Silhouette, height: f64, length: f64, antlers: bool) -> (Vec<Part>, f64) {
    let outline = outline(silhouette);
    let (sx, sy) = (length / outline.length, height / outline.shoulder_height);
    let [vx, vy] = outline.vital;
    let place = |[x, y]: [f64; 2]| [(x - vx) * sx, (y - vy) * sy];
    let mut parts = Vec::new();
    if antlers {
        for &(from, to, width) in outline.antlers {
            let (a, b) = (place(from), place(to));
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let norm = dx.hypot(dy).max(1e-9);
            let half = width * sy / 2.0;
            let (nx, ny) = (-dy / norm * half, dx / norm * half);
            parts.push(Part {
                points: vec![
                    [a[0] + nx, a[1] + ny],
                    [b[0] + nx, b[1] + ny],
                    [b[0] - nx, b[1] - ny],
                    [a[0] - nx, a[1] - ny],
                ],
                fill: ANTLER,
                stroke: None,
            });
        }
        for plate in outline.antler_plates {
            parts.push(Part {
                points: plate.iter().map(|&p| place(p)).collect(),
                fill: ANTLER,
                stroke: None,
            });
        }
    }
    for shape in outline.body {
        parts.push(Part {
            points: shape.iter().map(|&p| place(p)).collect(),
            fill: outline.colour,
            stroke: None,
        });
    }
    (parts, vy * sy)
}

/// A paper target on a stand: card, black, rings.
fn rings(diameters_mm: &[f64], black: f64) -> (Vec<Part>, f64) {
    let outer = diameters_mm.last().copied().unwrap_or(100.0) / 1000.0;
    let half = outer / 2.0 + (outer * 0.15).max(0.01);
    let aim_height = STAND_HEIGHT.max(half + 0.3);
    let mut parts = posts(aim_height, half, &[-half * 0.7, half * 0.7]);
    parts.push(Part {
        points: vec![[-half, -half], [half, -half], [half, half], [-half, half]],
        fill: CARD,
        stroke: None,
    });
    if black > 0.0 {
        parts.push(Part {
            points: circle(black / 2.0),
            fill: PRINT,
            stroke: None,
        });
    }
    for &diameter in diameters_mm {
        let radius = diameter / 2000.0;
        parts.push(Part {
            points: circle(radius),
            fill: Color32::TRANSPARENT,
            stroke: Some(if radius * 2.0 < black { CARD } else { PRINT }),
        });
    }
    (parts, aim_height)
}

/// A round steel plate hung on a post.
fn plate(diameter: f64) -> (Vec<Part>, f64) {
    let aim_height = STAND_HEIGHT.max(diameter / 2.0 + 0.3);
    let mut parts = posts(aim_height, 0.0, &[0.0]);
    parts.push(Part {
        points: circle(diameter / 2.0),
        fill: STEEL,
        stroke: Some(PRINT),
    });
    (parts, aim_height)
}

/// Posts from the ground up to `top` below the aim point, at each x.
fn posts(aim_height: f64, top: f64, at: &[f64]) -> Vec<Part> {
    let width = 0.05;
    at.iter()
        .map(|&x| Part {
            points: vec![
                [x - width / 2.0, -aim_height],
                [x + width / 2.0, -aim_height],
                [x + width / 2.0, -top],
                [x - width / 2.0, -top],
            ],
            fill: POST,
            stroke: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::Catalog;

    #[test]
    fn every_outline_part_is_convex() {
        for silhouette in Silhouette::ALL {
            let outline = outline(silhouette);
            for (index, part) in outline.body.iter().chain(outline.antler_plates).enumerate() {
                let turns: Vec<f64> = (0..part.len())
                    .map(|i| {
                        let [a, b, c] = [
                            part[i],
                            part[(i + 1) % part.len()],
                            part[(i + 2) % part.len()],
                        ];
                        (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
                    })
                    .collect();
                let all_left = turns.iter().all(|&turn| turn >= -1e-12);
                let all_right = turns.iter().all(|&turn| turn <= 1e-12);
                assert!(
                    all_left || all_right,
                    "{silhouette:?} part {index} is not convex: {turns:?}"
                );
            }
        }
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

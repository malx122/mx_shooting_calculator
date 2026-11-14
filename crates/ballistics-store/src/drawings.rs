//! Drawings of game animals: flat coloured polygons in metres, broadside,
//! for showing a target at true size. A target names its drawing by id
//! (`TargetShape::Animal::silhouette`); species that look alike share one.

use serde::{Deserialize, Serialize};

/// One animal drawn standing broadside, head to the right: x along the body,
/// y up from the ground (the feet at 0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Drawing {
    pub id: String,
    /// What it shows, e.g. "Black grouse (cock)".
    pub name: String,
    #[serde(default)]
    pub bird: bool,
    /// Height at the shoulder (to the top of the back on birds and small
    /// animals) and length (nose to rump, or bill to tail on birds) of the
    /// animal as drawn; a target's own sizes scale the drawing.
    pub shoulder_height_m: f64,
    pub length_m: f64,
    /// Centre of the vital area: where the hunter aims.
    pub vital: [f64; 2],
    /// Back to front.
    pub parts: Vec<DrawingPart>,
    /// Where a shot lands, most telling first: the first zone holding the
    /// impact names it (a heart before the whole hit area). Animals without zones
    /// use a circle the size of the target's vital zone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<HitZone>,
    /// Scoring rings of a training target printed on this animal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rings: Option<TrainingRings>,
}

/// A part of the animal a shot can land in, such as the lungs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HitZone {
    /// What it is, as the app says it: "hit area", "heart".
    pub name: String,
    /// A simple polygon, in the drawing's metres.
    pub points: Vec<[f64; 2]>,
}

/// The rings of a training target, true size whatever the animal's size.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrainingRings {
    /// Whose target, e.g. "Swedish moose test target".
    pub name: String,
    /// The rings' centre, in the drawing's metres.
    pub center: [f64; 2],
    /// Innermost first.
    pub rings: Vec<Ring>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ring {
    pub diameter_m: f64,
    /// What a hit in it scores, as printed: "5¹", "5", "4".
    pub score: String,
}

/// A simple polygon (it may be concave) in one colour.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrawingPart {
    /// "#RRGGBB".
    pub fill: String,
    pub points: Vec<[f64; 2]>,
    /// Antlers or horns, which the hunter can leave out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub antler: bool,
}

impl Drawing {
    pub fn has_antlers(&self) -> bool {
        self.parts.iter().any(|part| part.antler)
    }
}

/// Whether `point` lies inside the simple polygon `points`.
pub fn contains(points: &[[f64; 2]], [x, y]: [f64; 2]) -> bool {
    let mut inside = false;
    let mut j = points.len().wrapping_sub(1);
    for (i, &[xi, yi]) in points.iter().enumerate() {
        let [xj, yj] = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl DrawingPart {
    /// The fill as red, green and blue.
    pub fn rgb(&self) -> [u8; 3] {
        hex_rgb(&self.fill)
    }
}

/// A drawing's animal in 3D: a closed triangle mesh in the drawing's
/// metres, x along the body, y up, z across it towards whoever looks at the
/// drawing. The drawing's hit zones and rings lie in the plane z = 0,
/// through the body, where `placement` puts them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Body {
    /// The drawing it belongs to.
    pub id: String,
    pub vertices: Vec<[f32; 3]>,
    /// Corners, anticlockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
    /// "#RRGGBB", indexed by `face_colour`.
    pub colours: Vec<String>,
    /// Each triangle's colour.
    pub face_colour: Vec<u8>,
    /// The triangles of the antlers or horns, which the hunter can leave out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub antler_faces: Vec<u32>,
    /// Where the body stands against its drawing's hit area, as set in
    /// `tools/bodies/zone-editor.html`.
    #[serde(default, skip_serializing_if = "Placement::is_identity")]
    pub placement: Placement,
    #[serde(skip)]
    near: std::sync::OnceLock<NearSide>,
    #[serde(skip)]
    rgb: std::sync::OnceLock<Vec<[u8; 3]>>,
    #[serde(skip)]
    antler: std::sync::OnceLock<Vec<bool>>,
}

/// A body moved and scaled against its drawing, round the drawing's vital
/// point (the centre of its hit area): a corner `v` goes to
/// `vital + scale × (v − vital) + offset`, z scaled alike.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Placement {
    /// In the drawing's metres: x along the body, y up.
    pub offset: [f64; 2],
    pub scale: f64,
}

impl Default for Placement {
    fn default() -> Placement {
        Placement {
            offset: [0.0, 0.0],
            scale: 1.0,
        }
    }
}

impl Placement {
    fn is_identity(&self) -> bool {
        *self == Placement::default()
    }

    /// A corner of the body placed against a drawing whose vital point is
    /// `vital`.
    pub fn apply(&self, vital: [f64; 2], [x, y, z]: [f64; 3]) -> [f64; 3] {
        [
            vital[0] + self.scale * (x - vital[0]) + self.offset[0],
            vital[1] + self.scale * (y - vital[1]) + self.offset[1],
            self.scale * z,
        ]
    }

    /// A point of the drawing back on the unplaced body.
    pub fn undo(&self, vital: [f64; 2], [x, y]: [f64; 2]) -> [f64; 2] {
        [
            vital[0] + (x - vital[0] - self.offset[0]) / self.scale,
            vital[1] + (y - vital[1] - self.offset[1]) / self.scale,
        ]
    }
}

/// How far the body's near side stands out at each point of the drawing.
#[derive(Debug, Clone, PartialEq)]
struct NearSide {
    low: [f32; 2],
    cell: f32,
    columns: usize,
    z: Vec<f32>,
}

impl Body {
    /// Each triangle's colour as red, green and blue.
    pub fn face_rgb(&self) -> &[[u8; 3]] {
        self.rgb.get_or_init(|| {
            let palette: Vec<[u8; 3]> = self.colours.iter().map(|fill| hex_rgb(fill)).collect();
            self.face_colour
                .iter()
                .map(|&index| palette.get(index as usize).copied().unwrap_or([0x80; 3]))
                .collect()
        })
    }

    /// Whether each triangle belongs to the antlers.
    pub fn antler_mask(&self) -> &[bool] {
        self.antler.get_or_init(|| {
            let mut mask = vec![false; self.triangles.len()];
            for &face in &self.antler_faces {
                if let Some(slot) = mask.get_mut(face as usize) {
                    *slot = true;
                }
            }
            mask
        })
    }

    /// How far towards the viewer the unplaced body's surface (antlers
    /// left out) stands at a point; none off the body.
    pub fn near_side(&self, [x, y]: [f64; 2]) -> Option<f64> {
        let near = self.near.get_or_init(|| self.measure_near_side());
        let column = ((x as f32 - near.low[0]) / near.cell).floor();
        let row = ((y as f32 - near.low[1]) / near.cell).floor();
        let rows = near.z.len() / near.columns.max(1);
        if column < 0.0 || row < 0.0 || column as usize >= near.columns || row as usize >= rows {
            return None;
        }
        let z = near.z[row as usize * near.columns + column as usize];
        z.is_finite().then_some(f64::from(z))
    }

    fn measure_near_side(&self) -> NearSide {
        let mut low = [f32::INFINITY; 2];
        let mut high = [f32::NEG_INFINITY; 2];
        for v in &self.vertices {
            for axis in 0..2 {
                low[axis] = low[axis].min(v[axis]);
                high[axis] = high[axis].max(v[axis]);
            }
        }
        let cell = ((high[0] - low[0]).max(high[1] - low[1]) / 300.0).max(1e-4);
        let columns = ((high[0] - low[0]) / cell) as usize + 1;
        let rows = ((high[1] - low[1]) / cell) as usize + 1;
        let mut z = vec![f32::NEG_INFINITY; columns * rows];
        let antler = self.antler_mask();
        for (face, triangle) in self.triangles.iter().enumerate() {
            if antler[face] {
                continue;
            }
            let [a, b, c] = triangle.map(|k| self.vertices[k as usize]);
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            if area.abs() < 1e-12 {
                continue;
            }
            let cells = |values: [f32; 3], axis: usize| {
                let from = ((values.iter().copied().fold(f32::INFINITY, f32::min) - low[axis])
                    / cell)
                    .floor()
                    .max(0.0) as usize;
                let to = ((values.iter().copied().fold(f32::NEG_INFINITY, f32::max) - low[axis])
                    / cell)
                    .ceil() as usize;
                from..to.min(if axis == 0 { columns } else { rows })
            };
            for row in cells([a[1], b[1], c[1]], 1) {
                for column in cells([a[0], b[0], c[0]], 0) {
                    let px = low[0] + (column as f32 + 0.5) * cell;
                    let py = low[1] + (row as f32 + 0.5) * cell;
                    let wb = ((px - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (py - a[1])) / area;
                    let wc = ((b[0] - a[0]) * (py - a[1]) - (px - a[0]) * (b[1] - a[1])) / area;
                    let wa = 1.0 - wb - wc;
                    if wa >= 0.0 && wb >= 0.0 && wc >= 0.0 {
                        let depth = wa * a[2] + wb * b[2] + wc * c[2];
                        let slot = &mut z[row * columns + column];
                        *slot = slot.max(depth);
                    }
                }
            }
        }
        NearSide {
            low,
            cell,
            columns,
            z,
        }
    }
}

fn hex_rgb(fill: &str) -> [u8; 3] {
    let hex = fill.trim_start_matches('#');
    let channel = |at: usize| {
        hex.get(at..at + 2)
            .and_then(|text| u8::from_str_radix(text, 16).ok())
            .unwrap_or(0x80)
    };
    [channel(0), channel(2), channel(4)]
}

/// Drawing ids from before the drawings were data, and what replaced them.
pub(crate) const RENAMED: &[(&str, &str)] = &[
    ("deer", "red-deer"),
    ("hare", "mountain-hare"),
    ("grouse", "capercaillie"),
    ("ptarmigan", "willow-ptarmigan"),
];

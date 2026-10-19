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
    /// Centre of the heart–lung area.
    pub vital: [f64; 2],
    /// Back to front.
    pub parts: Vec<DrawingPart>,
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

impl DrawingPart {
    /// The fill as red, green and blue.
    pub fn rgb(&self) -> [u8; 3] {
        let hex = self.fill.trim_start_matches('#');
        let channel = |at: usize| {
            hex.get(at..at + 2)
                .and_then(|text| u8::from_str_radix(text, 16).ok())
                .unwrap_or(0x80)
        };
        [channel(0), channel(2), channel(4)]
    }
}

/// Drawing ids from before the drawings were data, and what replaced them.
pub(crate) const RENAMED: &[(&str, &str)] = &[
    ("deer", "red-deer"),
    ("hare", "mountain-hare"),
    ("grouse", "capercaillie"),
    ("ptarmigan", "willow-ptarmigan"),
];

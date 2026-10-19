//! What is shot at: game animals and sport targets. A target sets the zone a
//! shot must land in (an animal's heart–lung area, a sport target's scoring
//! zone) and, for game, the least energy the bullet must still carry.
//!
//! The catalog's targets and the hunter's own share one shape; the hunter's
//! are kept in the database and travel with exports.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Game,
    Sport,
}

/// The outline an animal is drawn with, scaled to its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Silhouette {
    Moose,
    Deer,
    Boar,
    Bear,
    Fox,
    /// Hares and rabbits, crouched.
    Hare,
    /// A dark grouse: capercaillie, black grouse, hazel grouse.
    Grouse,
    /// A pale grouse.
    Ptarmigan,
}

impl Silhouette {
    pub const ALL: [Silhouette; 8] = [
        Silhouette::Moose,
        Silhouette::Deer,
        Silhouette::Boar,
        Silhouette::Bear,
        Silhouette::Fox,
        Silhouette::Hare,
        Silhouette::Grouse,
        Silhouette::Ptarmigan,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Silhouette::Moose => "Moose",
            Silhouette::Deer => "Deer",
            Silhouette::Boar => "Wild boar",
            Silhouette::Bear => "Bear",
            Silhouette::Fox => "Fox",
            Silhouette::Hare => "Hare or rabbit",
            Silhouette::Grouse => "Grouse",
            Silhouette::Ptarmigan => "Ptarmigan",
        }
    }

    /// Birds are measured to the top of the back and from beak to tail.
    pub fn is_bird(self) -> bool {
        matches!(self, Silhouette::Grouse | Silhouette::Ptarmigan)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum TargetShape {
    /// An animal standing broadside.
    Animal {
        silhouette: Silhouette,
        shoulder_height_cm: f64,
        /// Rump to nose, without the tail.
        body_length_cm: f64,
        /// Draw antlers (when the silhouette has them).
        #[serde(default = "yes")]
        antlers: bool,
    },
    /// A paper target with evenly spaced scoring rings and a black aiming
    /// area.
    Rings {
        /// Diameter of the innermost ring.
        inner_mm: f64,
        /// How much each ring is wider than the one inside it.
        step_mm: f64,
        count: u32,
        black_mm: f64,
    },
    /// A round steel plate.
    Plate { diameter_mm: f64 },
}

fn yes() -> bool {
    true
}

impl TargetShape {
    /// Ring diameters, innermost first.
    pub fn ring_diameters_mm(&self) -> Vec<f64> {
        match *self {
            TargetShape::Rings {
                inner_mm,
                step_mm,
                count,
                ..
            } => (0..count)
                .map(|index| inner_mm + step_mm * index as f64)
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub kind: TargetKind,
    #[serde(flatten)]
    pub shape: TargetShape,
    /// The zone a shot must land in: the heart–lung area of an animal, or
    /// the part of a sport target that counts as a hit.
    pub vital_diameter_cm: f64,
    /// Least energy the bullet must carry at the target. Only for game.
    pub min_energy_j: Option<f64>,
    /// Least impact velocity this game asks for, instead of the hunter's
    /// general one (zero: no expansion needed, as for small game).
    #[serde(default)]
    pub min_velocity_mps: Option<f64>,
    #[serde(default)]
    pub notes: String,
    /// The catalog target a custom one was copied from.
    #[serde(default)]
    pub catalog_id: Option<String>,
    /// Zero for catalog targets.
    #[serde(default)]
    pub updated_at: i64,
}

impl Target {
    /// A new custom target to edit.
    pub fn new_custom() -> Target {
        Target {
            id: crate::model::new_id(),
            name: String::new(),
            kind: TargetKind::Game,
            shape: TargetShape::Animal {
                silhouette: Silhouette::Deer,
                shoulder_height_cm: 100.0,
                body_length_cm: 170.0,
                antlers: true,
            },
            vital_diameter_cm: 20.0,
            min_energy_j: Some(1400.0),
            min_velocity_mps: None,
            notes: String::new(),
            catalog_id: None,
            updated_at: 0,
        }
    }

    /// The hunter's own editable copy of a (catalog) target.
    pub fn to_custom(&self) -> Target {
        Target {
            id: crate::model::new_id(),
            name: format!("{} (mine)", self.name),
            catalog_id: Some(self.id.clone()),
            updated_at: 0,
            ..self.clone()
        }
    }

    pub fn is_game(&self) -> bool {
        self.kind == TargetKind::Game
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = |value: f64| value > 0.0;
        let problem = if self.name.trim().is_empty() {
            Some("the target needs a name")
        } else if !(positive(self.vital_diameter_cm) && self.vital_diameter_cm <= 500.0) {
            Some("the vital or hit zone must be between 0 and 500 cm across")
        } else if self
            .min_energy_j
            .is_some_and(|energy| energy < 0.0 || energy.is_nan())
        {
            Some("minimum energy can't be negative")
        } else if self
            .min_velocity_mps
            .is_some_and(|velocity| !(0.0..=1500.0).contains(&velocity))
        {
            Some("minimum velocity must be between 0 and 1500 m/s")
        } else {
            match self.shape {
                TargetShape::Animal {
                    shoulder_height_cm,
                    body_length_cm,
                    ..
                } if !(positive(shoulder_height_cm)
                    && positive(body_length_cm)
                    && shoulder_height_cm <= 400.0
                    && body_length_cm <= 600.0) =>
                {
                    Some("the animal's height and length must be positive and life-sized")
                }
                TargetShape::Rings {
                    inner_mm,
                    step_mm,
                    count,
                    black_mm,
                } if !(positive(inner_mm)
                    && step_mm >= 0.0
                    && (1..=20).contains(&count)
                    && black_mm >= 0.0
                    && inner_mm + step_mm * f64::from(count) <= 5000.0) =>
                {
                    Some("rings need a positive size, 1 to 20 rings, and at most 5 m across")
                }
                TargetShape::Plate { diameter_mm }
                    if !(positive(diameter_mm) && diameter_mm <= 5000.0) =>
                {
                    Some("the plate must be between 0 and 5 m across")
                }
                _ => None,
            }
        };
        match problem {
            Some(problem) if self.name.trim().is_empty() => Err(problem.to_owned()),
            Some(problem) => Err(format!("{}: {problem}", self.name.trim())),
            None => Ok(()),
        }
    }
}

impl Default for Target {
    fn default() -> Self {
        Target::new_custom()
    }
}

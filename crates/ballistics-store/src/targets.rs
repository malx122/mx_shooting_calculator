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

/// How targets are grouped in lists, the way hunters think of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetGroup {
    Deer,
    BoarAndMouflon,
    Bear,
    SmallPredators,
    HaresAndRodents,
    ForestBirds,
    Waterfowl,
    OtherBirds,
    Sport,
    Other,
}

impl TargetGroup {
    pub const ALL: [TargetGroup; 10] = [
        TargetGroup::Deer,
        TargetGroup::BoarAndMouflon,
        TargetGroup::Bear,
        TargetGroup::SmallPredators,
        TargetGroup::HaresAndRodents,
        TargetGroup::ForestBirds,
        TargetGroup::Waterfowl,
        TargetGroup::OtherBirds,
        TargetGroup::Sport,
        TargetGroup::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TargetGroup::Deer => "Deer",
            TargetGroup::BoarAndMouflon => "Wild boar and mouflon",
            TargetGroup::Bear => "Bear",
            TargetGroup::SmallPredators => "Fox, badger and martens",
            TargetGroup::HaresAndRodents => "Hares, rabbits and beaver",
            TargetGroup::ForestBirds => "Grouse, ptarmigan and pheasant",
            TargetGroup::Waterfowl => "Ducks, geese and swans",
            TargetGroup::OtherBirds => "Other birds",
            TargetGroup::Sport => "Sport targets",
            TargetGroup::Other => "Other",
        }
    }
}

/// The ammunition class a load meets under Swedish rules (Naturvårdsverket,
/// NFS 2023:8, 2 kap 11–14 §§), the strictest first:
///
/// - 1: expanding bullet of at least 9 g with E100 ≥ 2 700 J, or 10 g with
///   E100 ≥ 2 000 J, or lead-free 7.7 g with E100 ≥ 2 000 J;
/// - 2: expanding bullet of at least 3.2 g with E100 ≥ 800 J;
/// - 3: any bullet of at least 2.5 g with E100 ≥ 200 J;
/// - 4: any bullet with E0 ≥ 150 J.
///
/// `expanding` is `None` when the bullet's construction doesn't say.
pub fn swedish_class(
    mass_g: f64,
    e100_j: f64,
    e0_j: f64,
    expanding: Option<bool>,
    lead_free: bool,
) -> Option<u8> {
    let expanding = expanding == Some(true);
    if expanding
        && ((mass_g >= 9.0 && e100_j >= 2700.0)
            || (mass_g >= 10.0 && e100_j >= 2000.0)
            || (lead_free && mass_g >= 7.7 && e100_j >= 2000.0))
    {
        Some(1)
    } else if expanding && mass_g >= 3.2 && e100_j >= 800.0 {
        Some(2)
    } else if mass_g >= 2.5 && e100_j >= 200.0 {
        Some(3)
    } else if e0_j >= 150.0 {
        Some(4)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum TargetShape {
    /// An animal standing broadside.
    Animal {
        /// The drawing used, by name (the app's animal drawings, e.g.
        /// "moose" or "black-grouse"), scaled to the sizes below.
        silhouette: String,
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
    /// Where it is listed; `None` for the hunter's own until they choose.
    #[serde(default)]
    pub group: Option<TargetGroup>,
    /// Other names to find it by, such as the Swedish one.
    #[serde(default)]
    pub other_names: Vec<String>,
    /// The ammunition class Swedish rules ask for (1–4, see
    /// `swedish_class`), when it is game in Sweden.
    #[serde(default)]
    pub swedish_class: Option<u8>,
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
                silhouette: "roe-deer".to_owned(),
                shoulder_height_cm: 100.0,
                body_length_cm: 170.0,
                antlers: true,
            },
            vital_diameter_cm: 20.0,
            min_energy_j: Some(1400.0),
            min_velocity_mps: None,
            notes: String::new(),
            group: None,
            other_names: Vec::new(),
            swedish_class: None,
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

    /// The list section the target goes in.
    pub fn group(&self) -> TargetGroup {
        self.group.unwrap_or(match self.kind {
            TargetKind::Game => TargetGroup::Other,
            TargetKind::Sport => TargetGroup::Sport,
        })
    }

    /// Whether a search finds the target: by its name or another name, in
    /// any case. Every target matches an empty search.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || std::iter::once(&self.name)
                .chain(&self.other_names)
                .any(|name| name.to_lowercase().contains(&query))
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
            .swedish_class
            .is_some_and(|class| !(1..=4).contains(&class))
        {
            Some("the Swedish ammunition class must be 1 to 4")
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

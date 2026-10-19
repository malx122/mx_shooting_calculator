//! What is shot at: game animals and sport targets. A target sets the zone a
//! shot must land in (an animal's heart–lung area, a sport target's scoring
//! zone) and, for game, the least energy the bullet must still carry.
//!
//! The catalog's targets and the hunter's own share one shape; the hunter's
//! are kept in the database and travel with exports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rules::Country;

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
    Seals,
    ForestBirds,
    Waterfowl,
    OtherBirds,
    Sport,
    Other,
}

impl TargetGroup {
    pub const ALL: [TargetGroup; 11] = [
        TargetGroup::Deer,
        TargetGroup::BoarAndMouflon,
        TargetGroup::Bear,
        TargetGroup::SmallPredators,
        TargetGroup::HaresAndRodents,
        TargetGroup::Seals,
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
            TargetGroup::Bear => "Bear, wolf, lynx and wolverine",
            TargetGroup::SmallPredators => "Fox, badger, martens and otter",
            TargetGroup::HaresAndRodents => "Hares, beaver and rodents",
            TargetGroup::Seals => "Seals",
            TargetGroup::ForestBirds => "Grouse, ptarmigan and pheasant",
            TargetGroup::Waterfowl => "Ducks, geese and swans",
            TargetGroup::OtherBirds => "Other birds",
            TargetGroup::Sport => "Sport targets",
            TargetGroup::Other => "Other",
        }
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
    /// Other names to find it by, such as the Swedish and Finnish ones.
    #[serde(default)]
    pub other_names: Vec<String>,
    /// The rifle ammunition level each country's rules ask for (see
    /// `crate::rules`), where it is game there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ammunition: BTreeMap<Country, u8>,
    /// What else each country's rules say about it ("a shotgun is allowed
    /// too").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules_notes: BTreeMap<Country, String>,
    /// Sweden's class as earlier versions kept it; read into `ammunition`.
    #[serde(default, rename = "swedish_class", skip_serializing)]
    pub legacy_swedish_class: Option<u8>,
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
            ammunition: BTreeMap::new(),
            rules_notes: BTreeMap::new(),
            legacy_swedish_class: None,
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

    /// The rifle ammunition level `country`'s rules ask for, if it is game
    /// there and the level is known.
    pub fn ammunition_level(&self, country: Country) -> Option<u8> {
        self.ammunition.get(&country).copied().or(match country {
            Country::Sweden => self.legacy_swedish_class,
            _ => None,
        })
    }

    /// Sets (or with `None`, clears) the level `country` asks for.
    pub fn set_ammunition_level(&mut self, country: Country, level: Option<u8>) {
        if country == Country::Sweden {
            self.legacy_swedish_class = None;
        }
        match level {
            Some(level) => self.ammunition.insert(country, level),
            None => self.ammunition.remove(&country),
        };
    }

    /// Moves a level kept the earlier way into `ammunition`.
    pub(crate) fn upgrade_legacy(&mut self) {
        if let Some(class) = self.legacy_swedish_class.take() {
            self.ammunition.entry(Country::Sweden).or_insert(class);
        }
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
        } else if let Some(country) = Country::ALL.into_iter().find(|&country| {
            self.ammunition_level(country)
                .is_some_and(|level| !country.rules().valid_level(level))
        }) {
            return Err(format!(
                "that ammunition level doesn't exist in {}'s rules",
                country.name()
            ));
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

//! Rifle scopes and their reticles, as read from the catalog.
//!
//! A reticle is vector geometry in angular units around the aiming point
//! (x to the right, y up), so it can be drawn at any size and compared with
//! where the bullet lands. Each reticle keeps the unit its maker publishes;
//! `Reticle::in_mrad` converts it for drawing.

use serde::{Deserialize, Serialize};

use crate::model::ClickUnit;

/// One minute of angle in milliradians.
const MRAD_PER_MOA: f64 = 0.290_888_2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AngleUnit {
    Mrad,
    Moa,
    /// Centimetres at 100 metres, common on European hunting scopes; 1 cm at
    /// 100 m is 0.1 mrad.
    #[serde(rename = "cm_at_100m")]
    CmAt100m,
}

impl AngleUnit {
    pub fn to_mrad(self, value: f64) -> f64 {
        match self {
            AngleUnit::Mrad => value,
            AngleUnit::Moa => value * MRAD_PER_MOA,
            AngleUnit::CmAt100m => value * 0.1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AngleUnit::Mrad => "mrad",
            AngleUnit::Moa => "MOA",
            AngleUnit::CmAt100m => "cm/100 m",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocalPlane {
    /// First focal plane: the reticle grows with the image, so its marks are
    /// true at every magnification.
    Ffp,
    /// Second focal plane: the reticle stays the same size, so its marks are
    /// true only at one magnification.
    Sfp,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Scope {
    pub id: String,
    pub manufacturer: String,
    pub model: String,
    pub magnification_min: f64,
    pub magnification_max: f64,
    pub objective_mm: f64,
    pub tube_mm: Option<f64>,
    pub focal_plane: FocalPlane,
    /// For second-focal-plane scopes: the magnification at which the
    /// reticle's published dimensions are true.
    pub reticle_reference_magnification: Option<f64>,
    /// Reticle ids this scope is offered with.
    pub reticles: Vec<String>,
    pub click_unit: AngleUnit,
    /// Per click, in `click_unit`; `None` when the maker doesn't publish it.
    pub click_value: Option<f64>,
    /// Total elevation adjustment, in `click_unit`.
    pub elevation_travel: Option<f64>,
    /// Field of view in metres at 100 m, at minimum and maximum magnification.
    pub fov_m_at_100m: Option<[f64; 2]>,
    pub illuminated: bool,
    pub source_url: String,
    pub verified_on: String,
    #[serde(default)]
    pub notes: String,
}

impl Scope {
    pub fn display_name(&self) -> String {
        format!("{} {}", self.manufacturer, self.model)
    }

    pub fn clamp_magnification(&self, magnification: f64) -> f64 {
        magnification.clamp(self.magnification_min, self.magnification_max)
    }

    /// The scope's clicks as the rifle stores them (mil or MOA), if known.
    pub fn rifle_clicks(&self) -> Option<(ClickUnit, f64)> {
        let value = self.click_value?;
        Some(match self.click_unit {
            AngleUnit::Mrad => (ClickUnit::Mil, value),
            AngleUnit::CmAt100m => (ClickUnit::Mil, value * 0.1),
            AngleUnit::Moa => (ClickUnit::Moa, value),
        })
    }

    /// Full angular field of view in mrad. Field of view falls roughly as
    /// 1/magnification, so it is interpolated in 1/magnification between
    /// the two published values.
    pub fn field_of_view_mrad(&self, magnification: f64) -> Option<f64> {
        let [wide, narrow] = self.fov_m_at_100m?;
        let (low, high) = (self.magnification_min, self.magnification_max);
        let magnification = self.clamp_magnification(magnification);
        let metres = if high > low {
            let t = (1.0 / low - 1.0 / magnification) / (1.0 / low - 1.0 / high);
            wide + (narrow - wide) * t
        } else {
            wide
        };
        // 1 m at 100 m is 10 mrad.
        Some(metres * 10.0)
    }

    /// How much larger the reticle's marks are than their published values
    /// at this magnification: 1 for first focal plane; reference/current for
    /// second focal plane.
    pub fn reticle_scale(&self, magnification: f64) -> f64 {
        match (self.focal_plane, self.reticle_reference_magnification) {
            (FocalPlane::Sfp, Some(reference)) => {
                reference / self.clamp_magnification(magnification)
            }
            _ => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReticleElement {
    Line {
        from: [f64; 2],
        to: [f64; 2],
        /// Thickness; `None` when unpublished (drawn as a hairline).
        width: Option<f64>,
        #[serde(default)]
        illuminated: bool,
    },
    /// A filled shape, such as a heavy post.
    Polygon {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        illuminated: bool,
    },
    Circle {
        center: [f64; 2],
        radius: f64,
        fill: bool,
        /// Ring thickness when not filled; `None` for a hairline.
        width: Option<f64>,
        #[serde(default)]
        illuminated: bool,
    },
}

impl ReticleElement {
    fn scaled(&self, scale: impl Fn(f64) -> f64) -> ReticleElement {
        let point = |[x, y]: [f64; 2]| [scale(x), scale(y)];
        match self {
            ReticleElement::Line {
                from,
                to,
                width,
                illuminated,
            } => ReticleElement::Line {
                from: point(*from),
                to: point(*to),
                width: width.map(&scale),
                illuminated: *illuminated,
            },
            ReticleElement::Polygon {
                points,
                illuminated,
            } => ReticleElement::Polygon {
                points: points.iter().copied().map(point).collect(),
                illuminated: *illuminated,
            },
            ReticleElement::Circle {
                center,
                radius,
                fill,
                width,
                illuminated,
            } => ReticleElement::Circle {
                center: point(*center),
                radius: scale(*radius),
                fill: *fill,
                width: width.map(&scale),
                illuminated: *illuminated,
            },
        }
    }

    fn values(&self) -> Vec<f64> {
        match self {
            ReticleElement::Line {
                from, to, width, ..
            } => [from[0], from[1], to[0], to[1]]
                .into_iter()
                .chain(*width)
                .collect(),
            ReticleElement::Polygon { points, .. } => points.iter().flatten().copied().collect(),
            ReticleElement::Circle {
                center,
                radius,
                width,
                ..
            } => [center[0], center[1], *radius]
                .into_iter()
                .chain(*width)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Reticle {
    pub id: String,
    pub manufacturer: String,
    pub name: String,
    pub unit: AngleUnit,
    pub elements: Vec<ReticleElement>,
    pub source_url: String,
    pub verified_on: String,
    #[serde(default)]
    pub notes: String,
    /// Ids of catalog entries merged into this one (same drawing), so
    /// rifles and backups that name them still find it.
    #[serde(default)]
    pub replaces: Vec<String>,
    /// The maker's own name for the reticle, without scope or unit.
    #[serde(default)]
    pub maker_reticle_name: String,
    /// Other names the same drawing is sold under; searchable like the name.
    #[serde(default)]
    pub also_known_as: Vec<String>,
}

impl Reticle {
    /// Every name the reticle can be found by.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str())
            .chain(
                (!self.maker_reticle_name.is_empty()).then_some(self.maker_reticle_name.as_str()),
            )
            .chain(self.also_known_as.iter().map(String::as_str))
    }

    /// Whether every word of the query appears in the maker's name plus one
    /// of the reticle's names, ignoring case, spaces and dashes: "deadhold
    /// 2a" finds the Dead-Hold 2A.
    pub fn matches(&self, query: &str) -> bool {
        let squash = |text: &str| {
            text.to_lowercase()
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '-')
                .collect::<String>()
        };
        let words: Vec<String> = query.split_whitespace().map(squash).collect();
        self.names().any(|name| {
            let text = squash(&format!("{} {name}", self.manufacturer));
            words.iter().all(|word| text.contains(word.as_str()))
        })
    }

    /// The elements in mrad, at their published size.
    pub fn in_mrad(&self) -> Vec<ReticleElement> {
        self.elements
            .iter()
            .map(|element| element.scaled(|value| self.unit.to_mrad(value)))
            .collect()
    }

    /// How far from the centre the reticle's marks reach, in mrad,
    /// ignoring lines and posts that run to the edge of the view (drawn out
    /// to ±500).
    pub fn inner_extent_mrad(&self) -> f64 {
        let near = |value: &f64| value.abs() < 400.0;
        let extent = self
            .elements
            .iter()
            .flat_map(|element| match element {
                ReticleElement::Line { from, to, .. } => [*from, *to].concat(),
                ReticleElement::Polygon { points, .. } => {
                    points.iter().flatten().copied().collect()
                }
                ReticleElement::Circle { center, radius, .. } => {
                    vec![center[0].abs() + radius, center[1].abs() + radius]
                }
            })
            .filter(near)
            .fold(0.0_f64, |extent, value| extent.max(value.abs()));
        self.unit.to_mrad(extent)
    }

    /// Problems that would make the reticle unsafe to draw.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.elements.is_empty() {
            problems.push("no elements".to_owned());
        }
        for (index, element) in self.elements.iter().enumerate() {
            if element
                .values()
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1000.0)
            {
                problems.push(format!("element {index}: value out of range"));
            }
            match element {
                ReticleElement::Polygon { points, .. } if points.len() < 3 => {
                    problems.push(format!("element {index}: polygon needs three points"));
                }
                ReticleElement::Circle { radius, .. } if *radius <= 0.0 => {
                    problems.push(format!("element {index}: radius must be positive"));
                }
                ReticleElement::Line {
                    width: Some(width), ..
                }
                | ReticleElement::Circle {
                    width: Some(width), ..
                } if *width <= 0.0 => {
                    problems.push(format!("element {index}: width must be positive"));
                }
                _ => {}
            }
        }
        problems
    }
}

/// Id of the built-in plain crosshair, drawn for custom scopes whose reticle
/// isn't in the catalog.
pub const PLAIN_CROSSHAIR: &str = "plain-crosshair";

impl Reticle {
    /// A fine crosshair with no marks: always correct, whatever the scope.
    pub fn plain_crosshair() -> Reticle {
        let line = |from: [f64; 2], to: [f64; 2]| ReticleElement::Line {
            from,
            to,
            width: None,
            illuminated: false,
        };
        Reticle {
            id: PLAIN_CROSSHAIR.to_owned(),
            manufacturer: String::new(),
            name: "Plain crosshair".to_owned(),
            unit: AngleUnit::Mrad,
            elements: vec![
                line([-500.0, 0.0], [500.0, 0.0]),
                line([0.0, -500.0], [0.0, 500.0]),
            ],
            source_url: String::new(),
            verified_on: String::new(),
            notes: "Built in: no holdover marks.".to_owned(),
            replaces: Vec::new(),
            maker_reticle_name: String::new(),
            also_known_as: Vec::new(),
        }
    }
}

/// A scope the hunter entered: one that isn't in the catalog, or one set up
/// differently. Part of the personal data, exported and imported with it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserScope {
    pub id: String,
    pub manufacturer: String,
    pub model: String,
    pub magnification_min: f64,
    pub magnification_max: f64,
    /// Objective lens diameter, if known.
    pub objective_mm: Option<f64>,
    pub focal_plane: FocalPlane,
    pub reticle_reference_magnification: Option<f64>,
    pub click_unit: AngleUnit,
    pub click_value: f64,
    pub elevation_travel: Option<f64>,
    pub fov_m_at_100m: Option<[f64; 2]>,
    pub illuminated: bool,
    /// A catalog reticle to draw; `None` draws a plain crosshair.
    pub reticle_id: Option<String>,
    pub notes: String,
    pub updated_at: i64,
}

impl UserScope {
    pub fn new() -> Self {
        Self {
            id: crate::model::new_id(),
            manufacturer: String::new(),
            model: String::new(),
            magnification_min: 3.0,
            magnification_max: 12.0,
            objective_mm: Some(50.0),
            focal_plane: FocalPlane::Sfp,
            reticle_reference_magnification: Some(12.0),
            click_unit: AngleUnit::CmAt100m,
            click_value: 1.0,
            elevation_travel: None,
            fov_m_at_100m: None,
            illuminated: false,
            reticle_id: None,
            notes: String::new(),
            updated_at: 0,
        }
    }

    pub fn display_name(&self) -> String {
        format!("{} {}", self.manufacturer, self.model)
            .trim()
            .to_owned()
    }

    /// The scope in the form the calculation and drawing use.
    pub fn to_scope(&self) -> Scope {
        Scope {
            id: self.id.clone(),
            manufacturer: self.manufacturer.clone(),
            model: self.model.clone(),
            magnification_min: self.magnification_min,
            magnification_max: self.magnification_max,
            // Zero stands for "unknown" in the catalog shape.
            objective_mm: self.objective_mm.unwrap_or(0.0),
            tube_mm: None,
            focal_plane: self.focal_plane,
            reticle_reference_magnification: match self.focal_plane {
                FocalPlane::Sfp => self.reticle_reference_magnification,
                FocalPlane::Ffp => None,
            },
            reticles: vec![
                self.reticle_id
                    .clone()
                    .unwrap_or_else(|| PLAIN_CROSSHAIR.to_owned()),
            ],
            click_unit: self.click_unit,
            click_value: Some(self.click_value),
            elevation_travel: self.elevation_travel,
            fov_m_at_100m: self.fov_m_at_100m,
            illuminated: self.illuminated,
            source_url: String::new(),
            verified_on: String::new(),
            notes: self.notes.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let name = self.display_name();
        let problem = if name.is_empty() {
            Some("the scope needs a name")
        } else if !(positive(self.magnification_min)
            && self.magnification_min <= self.magnification_max
            && self.magnification_max <= 100.0)
        {
            Some("magnification must run from a positive minimum up to the maximum (at most 100×)")
        } else if self
            .objective_mm
            .is_some_and(|objective| !positive(objective))
        {
            Some("objective diameter must be positive")
        } else if !positive(self.click_value) {
            Some("click value must be positive")
        } else if self.focal_plane == FocalPlane::Sfp
            && !self
                .reticle_reference_magnification
                .is_some_and(|reference| {
                    reference >= self.magnification_min && reference <= self.magnification_max
                })
        {
            Some(
                "a second-focal-plane scope needs the magnification its reticle is true at, within its range",
            )
        } else if self
            .fov_m_at_100m
            .is_some_and(|[wide, narrow]| !(positive(narrow) && wide >= narrow))
        {
            Some("field of view must be positive and widest at the lowest magnification")
        } else if self
            .elevation_travel
            .is_some_and(|travel| !positive(travel))
        {
            Some("elevation travel must be positive")
        } else {
            None
        };
        match problem {
            Some(problem) => Err(format!("{name}: {problem}")
                .trim_start_matches(": ")
                .to_owned()),
            None => Ok(()),
        }
    }
}

/// True for numbers above zero; false for zero, negatives and NaN.
fn positive(value: f64) -> bool {
    value > 0.0
}

impl Default for UserScope {
    fn default() -> Self {
        UserScope::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(focal_plane: FocalPlane) -> Scope {
        Scope {
            id: "s".into(),
            manufacturer: "M".into(),
            model: "2.5-15x56".into(),
            magnification_min: 2.5,
            magnification_max: 15.0,
            objective_mm: 56.0,
            tube_mm: Some(30.0),
            focal_plane,
            reticle_reference_magnification: Some(15.0),
            reticles: vec![],
            click_unit: AngleUnit::CmAt100m,
            click_value: Some(1.0),
            elevation_travel: None,
            fov_m_at_100m: Some([16.0, 2.7]),
            illuminated: true,
            source_url: "https://example.com".into(),
            verified_on: "2026-10-19".into(),
            notes: String::new(),
        }
    }

    #[test]
    fn custom_scopes_convert_and_validate() {
        let mut custom = UserScope::new();
        assert!(custom.validate().is_err(), "needs a name");
        custom.manufacturer = "Old".into();
        custom.model = "6x42".into();
        custom.magnification_min = 6.0;
        custom.magnification_max = 6.0;
        custom.reticle_reference_magnification = Some(6.0);
        custom.validate().unwrap();
        let scope = custom.to_scope();
        assert_eq!(scope.reticles, [PLAIN_CROSSHAIR]);
        assert_eq!(scope.rifle_clicks(), Some((ClickUnit::Mil, 0.1)));
        assert_eq!(scope.reticle_scale(6.0), 1.0);
        custom.reticle_reference_magnification = Some(8.0);
        assert!(custom.validate().is_err(), "reference outside the range");
        assert!(Reticle::plain_crosshair().problems().is_empty());
    }

    #[test]
    fn units_convert_to_mrad() {
        assert_eq!(AngleUnit::CmAt100m.to_mrad(1.0), 0.1);
        assert!((AngleUnit::Moa.to_mrad(1.0) - 0.2909).abs() < 1e-4);
        assert_eq!(
            scope(FocalPlane::Ffp).rifle_clicks(),
            Some((ClickUnit::Mil, 0.1))
        );
    }

    #[test]
    fn field_of_view_matches_published_ends_and_narrows_in_between() {
        let scope = scope(FocalPlane::Ffp);
        assert!((scope.field_of_view_mrad(2.5).unwrap() - 160.0).abs() < 1e-9);
        assert!((scope.field_of_view_mrad(15.0).unwrap() - 27.0).abs() < 1e-9);
        let middle = scope.field_of_view_mrad(6.0).unwrap();
        assert!(middle < 160.0 && middle > 27.0);
        assert_eq!(
            scope.field_of_view_mrad(40.0),
            scope.field_of_view_mrad(15.0)
        );
    }

    #[test]
    fn second_focal_plane_marks_grow_as_magnification_drops() {
        assert_eq!(scope(FocalPlane::Sfp).reticle_scale(15.0), 1.0);
        assert_eq!(scope(FocalPlane::Sfp).reticle_scale(7.5), 2.0);
        assert_eq!(scope(FocalPlane::Ffp).reticle_scale(7.5), 1.0);
    }

    #[test]
    fn reticles_convert_and_report_bad_geometry() {
        let reticle: Reticle = serde_json::from_str(
            r#"{"id":"r","manufacturer":"M","name":"4","unit":"cm_at_100m","source_url":"https://x","verified_on":"d",
                "elements":[{"type":"line","from":[-500,0],"to":[500,0],"width":null},
                            {"type":"polygon","points":[[-500,-2],[-40,-2],[-40,2],[-500,2]]},
                            {"type":"circle","center":[0,0],"radius":1,"fill":true,"width":null,"illuminated":true}]}"#,
        )
        .unwrap();
        assert!(reticle.problems().is_empty());
        // The ±500 crosshair is ignored; the post starts 40 cm/100 m out.
        assert!((reticle.inner_extent_mrad() - 4.0).abs() < 1e-9);
        match &reticle.in_mrad()[2] {
            ReticleElement::Circle {
                radius,
                illuminated,
                ..
            } => {
                assert_eq!(*radius, 0.1);
                assert!(*illuminated);
            }
            other => panic!("{other:?}"),
        }
        let mut broken = reticle.clone();
        broken.elements.push(ReticleElement::Polygon {
            points: vec![[0.0, 0.0]],
            illuminated: false,
        });
        assert_eq!(broken.problems().len(), 1);
    }
}

//! The scale step: two points a known distance apart, or the four corners
//! of a rectangle of known size.

use ballistics_store::Calibration;
use eframe::egui::{self, RichText};
use groups_core::{CalibrationError, Point, Reference};

use crate::i18n::{tr, trf};
use crate::number_field::{NumberField, TypedUnit};
use crate::theme;

/// How the photo is scaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    TwoPoints,
    FourCorners,
}

/// The scale as it is being set, in photo pixels. Both ways are kept, so
/// switching between them loses nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleDraft {
    pub mode: ScaleMode,
    /// Point A and point B.
    pub points: [Option<[f64; 2]>; 2],
    /// Between A and B; 0 until chosen.
    pub distance_mm: f64,
    /// In the order tapped, at most four.
    pub corners: Vec<[f64; 2]>,
    /// Along the rectangle's top edge as the photo shows it.
    pub width_mm: f64,
    /// Along its left edge.
    pub height_mm: f64,
}

/// Lengths offered for two points, mm.
pub fn distance_presets() -> [(f64, &'static str); 5] {
    [
        (10.0, "10 mm"),
        (25.0, "25 mm"),
        (25.4, tr("1 inch")),
        (50.0, "50 mm"),
        (100.0, "100 mm"),
    ]
}

/// Sheets offered for four corners: name, width, height (mm, portrait).
pub fn sheet_presets() -> [(&'static str, f64, f64); 3] {
    [
        ("A4", 210.0, 297.0),
        ("A3", 297.0, 420.0),
        (tr("Letter"), 215.9, 279.4),
    ]
}

/// Whether two lengths are the same to a hundredth of a millimetre.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.005
}

/// What the draft gives.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ScaleState {
    /// Points still to tap: how many.
    ToTap(usize),
    /// All tapped, but refused.
    Refused(CalibrationError),
    Ready(groups_core::Calibration),
}

/// Something worth saying about a scale that works.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleNote {
    /// The photo is coarser than this many mm per pixel somewhere.
    TooCoarse(f64),
    /// The scale may be this far out, as a fraction.
    Uncertain(f64),
    /// The rectangle looks the other way round from its width and height.
    LooksTurned,
}

impl Default for ScaleDraft {
    fn default() -> Self {
        ScaleDraft {
            mode: ScaleMode::FourCorners,
            points: [None, None],
            distance_mm: 0.0,
            corners: Vec::new(),
            width_mm: 210.0,
            height_mm: 297.0,
        }
    }
}

impl ScaleDraft {
    /// The draft of a stored scale, or a blank one.
    pub fn from_store(calibration: Option<&Calibration>) -> Self {
        let blank = ScaleDraft::default();
        match calibration {
            None => blank,
            Some(Calibration::TwoPoint {
                a_px,
                b_px,
                distance_mm,
            }) => ScaleDraft {
                mode: ScaleMode::TwoPoints,
                points: [Some(*a_px), Some(*b_px)],
                distance_mm: *distance_mm,
                ..blank
            },
            Some(Calibration::FourCorner {
                corners_px,
                width_mm,
                height_mm,
            }) => ScaleDraft {
                mode: ScaleMode::FourCorners,
                corners: corners_px.to_vec(),
                width_mm: *width_mm,
                height_mm: *height_mm,
                ..blank
            },
        }
    }

    /// The handles of the current way, in photo pixels, by index.
    pub fn handles(&self) -> Vec<(usize, [f64; 2])> {
        match self.mode {
            ScaleMode::TwoPoints => self
                .points
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.map(|p| (i, p)))
                .collect(),
            ScaleMode::FourCorners => self.corners.iter().copied().enumerate().collect(),
        }
    }

    pub fn handle(&self, index: usize) -> Option<[f64; 2]> {
        match self.mode {
            ScaleMode::TwoPoints => self.points.get(index).copied().flatten(),
            ScaleMode::FourCorners => self.corners.get(index).copied(),
        }
    }

    pub fn set_handle(&mut self, index: usize, at: [f64; 2]) {
        match self.mode {
            ScaleMode::TwoPoints => {
                if let Some(point) = self.points.get_mut(index) {
                    *point = Some(at);
                }
            }
            ScaleMode::FourCorners => {
                if let Some(corner) = self.corners.get_mut(index) {
                    *corner = at;
                }
            }
        }
    }

    /// Places the next untapped point; its index, or `None` when all are in.
    pub fn place(&mut self, at: [f64; 2]) -> Option<usize> {
        match self.mode {
            ScaleMode::TwoPoints => {
                let index = self.points.iter().position(Option::is_none)?;
                self.points[index] = Some(at);
                Some(index)
            }
            ScaleMode::FourCorners => {
                if self.corners.len() >= 4 {
                    return None;
                }
                self.corners.push(at);
                Some(self.corners.len() - 1)
            }
        }
    }

    pub fn remove(&mut self, index: usize) {
        match self.mode {
            ScaleMode::TwoPoints => {
                if let Some(point) = self.points.get_mut(index) {
                    *point = None;
                }
            }
            ScaleMode::FourCorners => {
                if index < self.corners.len() {
                    self.corners.remove(index);
                }
            }
        }
    }

    pub fn state(&self) -> ScaleState {
        let point = |[x, y]: [f64; 2]| Point::new(x, y);
        let result = match self.mode {
            ScaleMode::TwoPoints => match self.points {
                [Some(a), Some(b)] => {
                    groups_core::Calibration::two_points(point(a), point(b), self.distance_mm)
                }
                points => return ScaleState::ToTap(points.iter().filter(|p| p.is_none()).count()),
            },
            ScaleMode::FourCorners => {
                if self.corners.len() < 4 {
                    return ScaleState::ToTap(4 - self.corners.len());
                }
                let corners = [0, 1, 2, 3].map(|i| point(self.corners[i]));
                groups_core::Calibration::four_corners(corners, self.width_mm, self.height_mm)
            }
        };
        match result {
            Ok(calibration) => ScaleState::Ready(calibration),
            Err(error) => ScaleState::Refused(error),
        }
    }

    /// The scale to keep on the photo, when the draft gives one.
    pub fn to_store(&self) -> Option<Calibration> {
        let ScaleState::Ready(calibration) = self.state() else {
            return None;
        };
        let xy = |p: Point| [p.x, p.y];
        Some(match calibration.reference() {
            Reference::TwoPoints { a, b, distance_mm } => Calibration::TwoPoint {
                a_px: xy(a),
                b_px: xy(b),
                distance_mm,
            },
            Reference::Rectangle {
                corners,
                width_mm,
                height_mm,
            } => Calibration::FourCorner {
                corners_px: corners.map(xy),
                width_mm,
                height_mm,
            },
        })
    }

    /// What is worth saying about a scale that works.
    pub fn notes(&self, calibration: &groups_core::Calibration) -> Vec<ScaleNote> {
        let resolution = calibration.resolution();
        let mut notes = Vec::new();
        if resolution.is_too_coarse() {
            notes.push(ScaleNote::TooCoarse(resolution.coarsest_mm_per_px));
        }
        if resolution.is_scale_uncertain() {
            notes.push(ScaleNote::Uncertain(resolution.scale_uncertainty));
        }
        if let Reference::Rectangle {
            corners,
            width_mm,
            height_mm,
        } = calibration.reference()
        {
            let across = 0.5 * (corners[0].distance(corners[1]) + corners[3].distance(corners[2]));
            let down = 0.5 * (corners[0].distance(corners[3]) + corners[1].distance(corners[2]));
            let shown = across / down;
            let given = width_mm / height_mm;
            // Only for a clearly oblong rectangle, which perspective can't turn.
            if (shown > 1.15 && given < 1.0 / 1.15) || (shown < 1.0 / 1.15 && given > 1.15) {
                notes.push(ScaleNote::LooksTurned);
            }
        }
        notes
    }

    pub fn swap_sides(&mut self) {
        std::mem::swap(&mut self.width_mm, &mut self.height_mm);
    }
}

/// The line telling what to do now.
pub fn instruction(draft: &ScaleDraft) -> String {
    match (draft.mode, draft.state()) {
        (ScaleMode::TwoPoints, ScaleState::ToTap(2)) => {
            tr("Tap one end of a known length, such as a grid line.").to_owned()
        }
        (ScaleMode::TwoPoints, ScaleState::ToTap(_)) => {
            tr("Now tap the other end of the known length.").to_owned()
        }
        (ScaleMode::TwoPoints, _) => {
            tr("Drag a point to move it, and give the distance between them.").to_owned()
        }
        (ScaleMode::FourCorners, ScaleState::ToTap(4)) => {
            tr("Tap the four corners of the sheet. Any order.").to_owned()
        }
        (ScaleMode::FourCorners, ScaleState::ToTap(left)) => {
            trf!("Tap the four corners of the sheet: {} to go.", left)
        }
        (ScaleMode::FourCorners, _) => {
            tr("Drag a corner to move it, and give the sheet's size.").to_owned()
        }
    }
}

/// Why the scale was refused, for the shooter.
pub fn refusal(mode: ScaleMode, error: CalibrationError) -> &'static str {
    match (error, mode) {
        (CalibrationError::TooClose, _) => {
            tr("The points are too close together. Place them further apart.")
        }
        (CalibrationError::Collinear, _) => {
            tr("Three of the corners are on one line. Tap the four corners of a rectangle.")
        }
        (CalibrationError::NotConvex, _) => {
            tr("The corners don't make a rectangle. Check that each sits on a corner.")
        }
        (_, ScaleMode::TwoPoints) => tr("Give the distance between the two points."),
        (_, ScaleMode::FourCorners) => tr("Give the sheet's width and height."),
    }
}

/// A note, for the shooter.
pub fn note_text(mode: ScaleMode, note: ScaleNote) -> String {
    match (note, mode) {
        (ScaleNote::TooCoarse(mm_per_px), _) => trf!(
            "The photo is coarse: a pixel spans {:.2} mm. Take a closer photo for exact marks.",
            mm_per_px
        ),
        (ScaleNote::Uncertain(fraction), ScaleMode::TwoPoints) => trf!(
            "Points are close together: the scale may be off by {:.0}%. Use points further apart.",
            (100.0 * fraction).max(1.0)
        ),
        (ScaleNote::Uncertain(fraction), ScaleMode::FourCorners) => trf!(
            "The corners are close together: the scale may be off by {:.0}%. Use a bigger \
             rectangle.",
            (100.0 * fraction).max(1.0)
        ),
        (ScaleNote::LooksTurned, _) => tr(
            "The sheet looks turned: the width runs along the top edge as shown. Swap width and \
             height?",
        )
        .to_owned(),
    }
}

/// The distance a shooter may type in inches or centimetres too.
fn length_units() -> Vec<TypedUnit> {
    vec![
        TypedUnit {
            labels: vec!["in", "inch", "inches", "\""],
            scale: 25.4,
            offset: 0.0,
        },
        TypedUnit {
            labels: vec!["cm"],
            scale: 10.0,
            offset: 0.0,
        },
    ]
}

fn length_field(value: &mut f64) -> NumberField<'_> {
    NumberField::new(value)
        .range(0.1..=10_000.0)
        .max_decimals(2)
        .suffix(" mm")
        .units(length_units())
}

/// The scale step's choices: the way, the size, and what the draft gives.
pub fn controls(ui: &mut egui::Ui, draft: &mut ScaleDraft) {
    let palette = theme::palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut draft.mode, ScaleMode::TwoPoints, tr("2 points"));
        ui.selectable_value(&mut draft.mode, ScaleMode::FourCorners, tr("4 corners"));
    });
    match draft.mode {
        ScaleMode::TwoPoints => {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Distance"));
                for (mm, name) in distance_presets() {
                    if ui
                        .selectable_label(same(draft.distance_mm, mm), name)
                        .clicked()
                    {
                        draft.distance_mm = mm;
                    }
                }
                ui.add(length_field(&mut draft.distance_mm));
            });
        }
        ScaleMode::FourCorners => {
            ui.horizontal_wrapped(|ui| {
                for (name, width, height) in sheet_presets() {
                    let portrait = same(draft.width_mm, width) && same(draft.height_mm, height);
                    let landscape = same(draft.width_mm, height) && same(draft.height_mm, width);
                    if ui
                        .selectable_label(portrait || landscape, name)
                        .on_hover_text(format!("{width} × {height} mm"))
                        .clicked()
                    {
                        if landscape {
                            (draft.width_mm, draft.height_mm) = (height, width);
                        } else {
                            (draft.width_mm, draft.height_mm) = (width, height);
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Width along the top edge"));
                ui.add(length_field(&mut draft.width_mm));
                ui.label(tr("× height"));
                ui.add(length_field(&mut draft.height_mm));
                if ui
                    .button(tr("Swap"))
                    .on_hover_text(tr("Swap width and height"))
                    .clicked()
                {
                    draft.swap_sides();
                }
            });
        }
    }
    match draft.state() {
        ScaleState::ToTap(_) => {}
        ScaleState::Refused(error) => {
            ui.label(RichText::new(refusal(draft.mode, error)).color(palette.alarm));
        }
        ScaleState::Ready(calibration) => {
            let notes = draft.notes(&calibration);
            for note in &notes {
                ui.label(RichText::new(note_text(draft.mode, *note)).color(palette.warning));
            }
            if notes.is_empty() {
                ui.label(
                    RichText::new(trf!(
                        "Scale set: {:.1} pixels per mm.",
                        calibration.resolution().px_per_mm()
                    ))
                    .color(palette.good),
                );
            }
        }
    }
}

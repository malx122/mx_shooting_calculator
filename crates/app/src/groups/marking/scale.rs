//! The scale step: four crossings of the target's grid, two points a known
//! distance apart, or the four corners of a rectangle of known size.

use ballistics_store::Calibration;
use eframe::egui::{self, RichText};
use groups_core::{CalibrationError, Point, Reference};

use crate::i18n::{tr, trf};
use crate::number_field::{NumberField, TypedUnit};
use crate::theme;

/// How the photo is scaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    /// Four crossings of the target's grid, a counted number of squares apart.
    GridSquares,
    TwoPoints,
    FourCorners,
}

/// The scale as it is being set, in photo pixels. Every way is kept, so
/// switching between them loses nothing; grid squares and four corners share
/// their four taps.
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
    /// The side of one grid square.
    pub square_mm: f64,
    /// Squares between the crossings along the top edge as the photo shows it.
    pub squares_across: u32,
    /// Squares between the crossings along the left edge.
    pub squares_down: u32,
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

/// Grid squares offered, mm.
pub fn square_presets() -> [(f64, &'static str); 7] {
    [
        (1.0, "1 mm"),
        (5.0, "5 mm"),
        (6.35, tr("¼ inch")),
        (10.0, "10 mm"),
        (12.7, tr("½ inch")),
        (25.0, "25 mm"),
        (25.4, tr("1 inch")),
    ]
}

/// The most squares a side may count.
pub const MAX_SQUARES: u32 = 200;

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
            mode: ScaleMode::GridSquares,
            points: [None, None],
            distance_mm: 0.0,
            corners: Vec::new(),
            width_mm: 210.0,
            height_mm: 297.0,
            square_mm: 25.4,
            squares_across: 4,
            squares_down: 4,
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
                ..
            }) => {
                let rectangle = ScaleDraft {
                    mode: ScaleMode::FourCorners,
                    corners: corners_px.to_vec(),
                    width_mm: *width_mm,
                    height_mm: *height_mm,
                    ..blank
                };
                match grid_of(*width_mm, *height_mm) {
                    Some((square_mm, across, down)) => ScaleDraft {
                        mode: ScaleMode::GridSquares,
                        square_mm,
                        squares_across: across,
                        squares_down: down,
                        ..rectangle
                    },
                    None => rectangle,
                }
            }
        }
    }

    /// The rectangle's width and height: counted squares on a grid, typed on
    /// a sheet.
    pub fn sides_mm(&self) -> (f64, f64) {
        match self.mode {
            ScaleMode::GridSquares => (
                f64::from(self.squares_across) * self.square_mm,
                f64::from(self.squares_down) * self.square_mm,
            ),
            _ => (self.width_mm, self.height_mm),
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
            ScaleMode::GridSquares | ScaleMode::FourCorners => {
                self.corners.iter().copied().enumerate().collect()
            }
        }
    }

    pub fn handle(&self, index: usize) -> Option<[f64; 2]> {
        match self.mode {
            ScaleMode::TwoPoints => self.points.get(index).copied().flatten(),
            ScaleMode::GridSquares | ScaleMode::FourCorners => self.corners.get(index).copied(),
        }
    }

    pub fn set_handle(&mut self, index: usize, at: [f64; 2]) {
        match self.mode {
            ScaleMode::TwoPoints => {
                if let Some(point) = self.points.get_mut(index) {
                    *point = Some(at);
                }
            }
            ScaleMode::GridSquares | ScaleMode::FourCorners => {
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
            ScaleMode::GridSquares | ScaleMode::FourCorners => {
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
            ScaleMode::GridSquares | ScaleMode::FourCorners => {
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
            ScaleMode::GridSquares | ScaleMode::FourCorners => {
                if self.corners.len() < 4 {
                    return ScaleState::ToTap(4 - self.corners.len());
                }
                let corners = [0, 1, 2, 3].map(|i| point(self.corners[i]));
                let (width_mm, height_mm) = self.sides_mm();
                groups_core::Calibration::four_corners(corners, width_mm, height_mm)
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
                ordered: calibration.is_ordered(),
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
        match self.mode {
            ScaleMode::GridSquares => {
                std::mem::swap(&mut self.squares_across, &mut self.squares_down)
            }
            _ => std::mem::swap(&mut self.width_mm, &mut self.height_mm),
        }
    }
}

/// The grid a stored rectangle was set from: square side and counts, when
/// it isn't one of the offered sheets and both sides are a whole number of
/// one of the offered squares (the largest that fits).
fn grid_of(width_mm: f64, height_mm: f64) -> Option<(f64, u32, u32)> {
    let sheet = sheet_presets().into_iter().any(|(_, width, height)| {
        (same(width, width_mm) && same(height, height_mm))
            || (same(width, height_mm) && same(height, width_mm))
    });
    if sheet {
        return None;
    }
    // Millimetre squares would read almost any rectangle as a grid.
    square_presets()
        .into_iter()
        .rev()
        .filter(|(square, _)| *square >= 5.0)
        .find_map(|(square, _)| {
            let count = |side: f64| {
                let squares = (side / square).round();
                (squares >= 1.0
                    && squares <= f64::from(MAX_SQUARES)
                    && same(squares * square, side))
                .then_some(squares as u32)
            };
            Some((square, count(width_mm)?, count(height_mm)?))
        })
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
        (ScaleMode::GridSquares, ScaleState::ToTap(4)) => {
            tr("Tap four grid crossings that make a rectangle, far apart. Any order.").to_owned()
        }
        (ScaleMode::GridSquares, ScaleState::ToTap(left)) => {
            trf!("Tap four grid crossings: {} to go.", left)
        }
        (ScaleMode::GridSquares, _) => {
            tr("Drag a crossing to move it, and count the squares between them.").to_owned()
        }
    }
}

/// Why the scale was refused, for the shooter.
pub fn refusal(mode: ScaleMode, error: CalibrationError) -> &'static str {
    match (error, mode) {
        (CalibrationError::TooClose, _) => {
            tr("The points are too close together. Place them further apart.")
        }
        (CalibrationError::Collinear, ScaleMode::GridSquares) => {
            tr("Three of the crossings are on one line. Tap four that make a rectangle.")
        }
        (CalibrationError::NotConvex, ScaleMode::GridSquares) => {
            tr("The crossings don't make a rectangle. Check that each sits on a crossing.")
        }
        (CalibrationError::Collinear, _) => {
            tr("Three of the corners are on one line. Tap the four corners of a rectangle.")
        }
        (CalibrationError::NotConvex, _) => {
            tr("The corners don't make a rectangle. Check that each sits on a corner.")
        }
        (_, ScaleMode::TwoPoints) => tr("Give the distance between the two points."),
        (_, ScaleMode::FourCorners) => tr("Give the sheet's width and height."),
        (_, ScaleMode::GridSquares) => {
            tr("Give the square's size and how many squares across and down.")
        }
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
        (ScaleNote::Uncertain(fraction), ScaleMode::GridSquares) => trf!(
            "The crossings are close together: the scale may be off by {:.0}%. Use crossings \
             further apart.",
            (100.0 * fraction).max(1.0)
        ),
        (ScaleNote::Uncertain(fraction), ScaleMode::FourCorners) => trf!(
            "The corners are close together: the scale may be off by {:.0}%. Use a bigger \
             rectangle.",
            (100.0 * fraction).max(1.0)
        ),
        (ScaleNote::LooksTurned, ScaleMode::GridSquares) => tr(
            "The rectangle looks turned: squares across run along the top edge as shown. Swap \
             across and down?",
        )
        .to_owned(),
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

fn count_field(value: &mut u32) -> NumberField<'_> {
    NumberField::new(value)
        .range(1..=MAX_SQUARES)
        .max_decimals(0)
}

/// The scale step's choices: the way, the size, "Find the grid" (when the
/// photo's pixels are ready), and what the draft gives.
/// What the scale step knows besides the draft, for its warnings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sight {
    /// The photo's pixels are there to search.
    pub can_search: bool,
    pub calibre_mm: Option<f64>,
    /// The photo's shorter side, px.
    pub photo_short_px: Option<f64>,
}

/// A square size by its preset's name, else in millimetres.
pub fn square_name(mm: f64) -> String {
    square_presets()
        .into_iter()
        .find(|(preset, _)| same(*preset, mm))
        .map_or_else(
            || format!("{} mm", super::tenths(mm)),
            |(_, name)| name.to_owned(),
        )
}

/// Warnings that the scale may be off by a wrong size: a rectangle no
/// target has, or bullet holes too small or too big to be real.
pub fn implausible(
    draft: &ScaleDraft,
    calibration: &groups_core::Calibration,
    sight: Sight,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let (width, height) = draft.sides_mm();
    if width.max(height) > 600.0 || width.min(height) < 15.0 {
        warnings.push(trf!(
            "The rectangle comes to {} × {} mm: check the size.",
            super::tenths(width),
            super::tenths(height)
        ));
    }
    if let Some(calibre) = sight.calibre_mm.filter(|c| *c > 0.0) {
        let hole_px = calibre * calibration.resolution().px_per_mm();
        if hole_px < 4.0 {
            warnings.push(
                tr("At this scale a bullet hole would be under 4 pixels: check the size.")
                    .to_owned(),
            );
        } else if sight
            .photo_short_px
            .is_some_and(|short| hole_px > 0.2 * short)
        {
            warnings.push(
                tr("At this scale a bullet hole would fill a fifth of the photo: check the size.")
                    .to_owned(),
            );
        }
    }
    warnings
}

pub fn controls(
    ui: &mut egui::Ui,
    draft: &mut ScaleDraft,
    finder: &mut super::grid::Finder,
    sight: Sight,
) {
    let palette = theme::palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut draft.mode, ScaleMode::GridSquares, tr("Grid squares"));
        ui.selectable_value(&mut draft.mode, ScaleMode::FourCorners, tr("4 corners"));
        ui.selectable_value(&mut draft.mode, ScaleMode::TwoPoints, tr("2 points"));
    });
    match draft.mode {
        ScaleMode::GridSquares => {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Square"));
                let mut chosen = false;
                for (mm, name) in square_presets() {
                    if ui
                        .selectable_label(same(draft.square_mm, mm), name)
                        .clicked()
                    {
                        draft.square_mm = mm;
                        chosen = true;
                    }
                }
                chosen |= ui.add(length_field(&mut draft.square_mm)).changed();
                if chosen {
                    finder.confirm(draft);
                }
            });
            // A found count runs along the true top edge; Swap only when the
            // rectangle looks turned.
            let looks_turned = matches!(draft.state(), ScaleState::Ready(calibration)
                if draft.notes(&calibration).contains(&ScaleNote::LooksTurned));
            let offer_swap = !finder.counted(draft) || looks_turned;
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Squares across the top"));
                ui.add(count_field(&mut draft.squares_across));
                ui.label(tr("× down"));
                ui.add(count_field(&mut draft.squares_down));
                if offer_swap
                    && ui
                        .button(tr("Swap"))
                        .on_hover_text(tr("Swap across and down"))
                        .clicked()
                {
                    draft.swap_sides();
                }
            });
            finder.ui(ui, draft, sight.can_search);
        }
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
            let warnings = implausible(draft, &calibration, sight);
            for warning in &warnings {
                ui.label(RichText::new(warning).color(palette.warning));
            }
            if notes.is_empty() && warnings.is_empty() && !finder.awaits_size(draft) {
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

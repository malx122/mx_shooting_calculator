//! MX targets: the sheets MXSC prints, their corner marks, the layout code
//! the marks carry, and finding the marks on a photo.
//!
//! An MX target has a mark near each corner of the sheet, centred on a
//! known rectangle. A mark is nine modules square: a black frame one
//! module wide, a white ring, and five by five data modules. The data's
//! top-left module is always black and its other three corners white, so
//! a mark shows which way is up; the other 21 modules carry the corner the
//! mark is at, the layout of the sheet and a CRC-8. Every mark carries the
//! whole layout, so any one of them tells the paper, the aim points and
//! the grid. The format is in `docs/mx-targets.md`.
//!
//! [`find_marks`] works on a reduced copy of the photo: it thresholds the
//! darkness against the paper round it, takes the dark shapes with a hole
//! that make a square-ish quadrilateral, and reads their modules through
//! the quadrilateral's homography. Each mark read is then measured on the
//! photo's own pixels: the frame's eight edges are found to a fraction of
//! a pixel and the mark's centre is where its homography puts it. A mark
//! that is covered or holed is looked for where the others put it, and
//! otherwise placed from them.

#![allow(clippy::needless_range_loop)]

use std::fmt;

use groups_core::{Calibration, CalibrationError, Point};

use crate::grid::InkImage;

/// A 3 × 3 matrix acting on `[x, y, 1]`.
type Matrix = [[f64; 3]; 3];

/// The version of the format the marks carry (the code's version bit is
/// 0 for this one).
pub const FORMAT: u8 = 1;
/// Modules across a mark.
pub const MARK_MODULES: usize = 9;
const DATA: usize = 5;
const CRC_POLYNOMIAL: u8 = 0x07;

/// The paper an MX target is printed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Paper {
    A4,
    A3,
    Letter,
}

impl Paper {
    pub const ALL: [Paper; 3] = [Paper::A4, Paper::A3, Paper::Letter];

    /// Width and height upright, mm.
    pub fn portrait_mm(self) -> [f64; 2] {
        match self {
            Paper::A4 => [210.0, 297.0],
            Paper::A3 => [297.0, 420.0],
            Paper::Letter => [215.9, 279.4],
        }
    }

    /// The side of one module of the marks, mm.
    pub fn module_mm(self) -> f64 {
        match self {
            Paper::A3 => 3.5,
            Paper::A4 | Paper::Letter => 2.5,
        }
    }

    /// How far each mark's centre is from the two edges nearest it, mm.
    pub fn inset_mm(self) -> f64 {
        match self {
            Paper::A3 => 30.0,
            Paper::A4 | Paper::Letter => 24.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::Letter => "Letter",
        }
    }
}

/// How many aim points (bulls) a sheet has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AimPoints {
    One,
    Two,
    Four,
    Six,
}

impl AimPoints {
    pub const ALL: [AimPoints; 4] = [
        AimPoints::One,
        AimPoints::Two,
        AimPoints::Four,
        AimPoints::Six,
    ];

    pub fn count(self) -> usize {
        match self {
            AimPoints::One => 1,
            AimPoints::Two => 2,
            AimPoints::Four => 4,
            AimPoints::Six => 6,
        }
    }
}

/// The light grid printed under the aim points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridSpacing {
    Off,
    /// 10 mm squares.
    Mm10,
    /// 1 inch squares.
    Inch,
}

impl GridSpacing {
    pub const ALL: [GridSpacing; 3] = [GridSpacing::Off, GridSpacing::Mm10, GridSpacing::Inch];

    pub fn spacing_mm(self) -> Option<f64> {
        match self {
            GridSpacing::Off => None,
            GridSpacing::Mm10 => Some(10.0),
            GridSpacing::Inch => Some(25.4),
        }
    }
}

/// The aim mark's shape: both are outlines with a fine cross through the
/// centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AimShape {
    /// A square standing on a corner; the cross joins its points.
    Diamond,
    Square,
}

/// The aim mark's size: its width across (tip to tip for a diamond).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AimSize {
    /// For about 100 m.
    Mm25,
    /// For about 150 m.
    Mm40,
    /// For about 200 m.
    Mm60,
    /// For about 300 m.
    Mm80,
}

impl AimSize {
    pub const ALL: [AimSize; 4] = [AimSize::Mm25, AimSize::Mm40, AimSize::Mm60, AimSize::Mm80];

    pub fn mm(self) -> f64 {
        match self {
            AimSize::Mm25 => 25.0,
            AimSize::Mm40 => 40.0,
            AimSize::Mm60 => 60.0,
            AimSize::Mm80 => 80.0,
        }
    }

    /// The range the size suits, m.
    pub fn range_m(self) -> u32 {
        match self {
            AimSize::Mm25 => 100,
            AimSize::Mm40 => 150,
            AimSize::Mm60 => 200,
            AimSize::Mm80 => 300,
        }
    }
}

/// A corner of the sheet, as printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl Corner {
    /// Clockwise from the top left, the order of every corner array here.
    pub const ALL: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomRight,
        Corner::BottomLeft,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Everything about an MX target that its marks carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layout {
    pub paper: Paper,
    pub landscape: bool,
    pub aims: AimPoints,
    pub grid: GridSpacing,
    pub shape: AimShape,
    pub size: AimSize,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            paper: Paper::A4,
            landscape: false,
            aims: AimPoints::Four,
            grid: GridSpacing::Mm10,
            shape: AimShape::Diamond,
            size: AimSize::Mm40,
        }
    }
}

impl Layout {
    /// The layout's 11-bit code, as the marks carry it.
    pub fn code(&self) -> u16 {
        let paper = match self.paper {
            Paper::A4 => 0,
            Paper::A3 => 1,
            Paper::Letter => 2,
        };
        let aims = match self.aims {
            AimPoints::One => 0,
            AimPoints::Two => 1,
            AimPoints::Four => 2,
            AimPoints::Six => 3,
        };
        let grid = match self.grid {
            GridSpacing::Off => 0,
            GridSpacing::Mm10 => 1,
            GridSpacing::Inch => 2,
        };
        let shape = match self.shape {
            AimShape::Diamond => 0,
            AimShape::Square => 1,
        };
        let size = match self.size {
            AimSize::Mm25 => 0,
            AimSize::Mm40 => 1,
            AimSize::Mm60 => 2,
            AimSize::Mm80 => 3,
        };
        (paper << 8)
            | (u16::from(self.landscape) << 7)
            | (aims << 5)
            | (grid << 3)
            | (shape << 2)
            | size
    }

    /// The layout a code stands for, or `None` for a code of another
    /// version or with a value this version doesn't have.
    pub fn from_code(code: u16) -> Option<Layout> {
        if code >> 10 != 0 {
            return None;
        }
        Some(Layout {
            paper: match (code >> 8) & 3 {
                0 => Paper::A4,
                1 => Paper::A3,
                2 => Paper::Letter,
                _ => return None,
            },
            landscape: (code >> 7) & 1 == 1,
            aims: AimPoints::ALL[usize::from((code >> 5) & 3)],
            grid: match (code >> 3) & 3 {
                0 => GridSpacing::Off,
                1 => GridSpacing::Mm10,
                2 => GridSpacing::Inch,
                _ => return None,
            },
            shape: if (code >> 2) & 1 == 0 {
                AimShape::Diamond
            } else {
                AimShape::Square
            },
            size: AimSize::ALL[usize::from(code & 3)],
        })
    }

    /// The 21 bits the mark at `corner` carries: the corner (2 bits) and
    /// the code (11 bits), then their CRC-8.
    pub fn word(&self, corner: Corner) -> u32 {
        let payload = ((corner.index() as u32) << 11) | u32::from(self.code());
        (payload << 8) | u32::from(crc8(payload, 13))
    }

    /// The modules of the mark at `corner`, row by row from the top as
    /// printed; `true` is black.
    pub fn modules(&self, corner: Corner) -> [[bool; MARK_MODULES]; MARK_MODULES] {
        let mut modules = [[false; MARK_MODULES]; MARK_MODULES];
        for i in 0..MARK_MODULES {
            for j in 0..MARK_MODULES {
                modules[i][j] = i == 0 || j == 0 || i == MARK_MODULES - 1 || j == MARK_MODULES - 1;
            }
        }
        let word = self.word(corner);
        let mut bit = 21;
        for r in 0..DATA {
            for c in 0..DATA {
                modules[r + 2][c + 2] = match data_corner(r, c) {
                    Some(dot) => dot,
                    None => {
                        bit -= 1;
                        (word >> bit) & 1 == 1
                    }
                };
            }
        }
        modules
    }

    /// The sheet's width and height as printed, mm.
    pub fn sheet_mm(&self) -> [f64; 2] {
        let [width, height] = self.paper.portrait_mm();
        if self.landscape {
            [height, width]
        } else {
            [width, height]
        }
    }

    pub fn module_mm(&self) -> f64 {
        self.paper.module_mm()
    }

    /// A mark's side, mm.
    pub fn mark_mm(&self) -> f64 {
        MARK_MODULES as f64 * self.module_mm()
    }

    /// The marks' centres on the sheet (mm from its top-left corner, y
    /// down): top-left, top-right, bottom-right, bottom-left.
    pub fn mark_centres_mm(&self) -> [[f64; 2]; 4] {
        let [width, height] = self.sheet_mm();
        let inset = self.paper.inset_mm();
        [
            [inset, inset],
            [width - inset, inset],
            [width - inset, height - inset],
            [inset, height - inset],
        ]
    }

    /// The width and height of the rectangle through the marks' centres,
    /// mm.
    pub fn rectangle_mm(&self) -> [f64; 2] {
        let [width, height] = self.sheet_mm();
        let inset = self.paper.inset_mm();
        [width - 2.0 * inset, height - 2.0 * inset]
    }

    /// The area for the aim points and the grid, `[x0, y0, x1, y1]` mm:
    /// from the marks' outer edges across, and between the rows of marks
    /// (with a module and a half to spare) down.
    pub fn field_mm(&self) -> [f64; 4] {
        let [width, height] = self.sheet_mm();
        let half = self.mark_mm() / 2.0;
        let inset = self.paper.inset_mm();
        let x0 = inset - half;
        let y0 = inset + half + 1.5 * self.module_mm();
        [x0, y0, width - x0, height - y0]
    }

    /// Columns and rows of aim points.
    pub fn columns_rows(&self) -> (usize, usize) {
        match (self.aims, self.landscape) {
            (AimPoints::One, _) => (1, 1),
            (AimPoints::Two, false) => (1, 2),
            (AimPoints::Two, true) => (2, 1),
            (AimPoints::Four, _) => (2, 2),
            (AimPoints::Six, false) => (2, 3),
            (AimPoints::Six, true) => (3, 2),
        }
    }

    /// The aim points' centres on the sheet (mm from its top-left corner,
    /// y down), row by row from the top left. Each is the middle of its
    /// share of the field, moved to the nearest grid crossing (counted
    /// from the sheet's middle; whole 5 mm without a grid).
    pub fn aim_points_mm(&self) -> Vec<[f64; 2]> {
        let [x0, y0, x1, y1] = self.field_mm();
        let [width, height] = self.sheet_mm();
        let (columns, rows) = self.columns_rows();
        let step = self.grid.spacing_mm().unwrap_or(5.0);
        let snap = |value: f64, middle: f64| middle + ((value - middle) / step).round() * step;
        let mut points = Vec::with_capacity(columns * rows);
        for row in 0..rows {
            for column in 0..columns {
                let x = x0 + (x1 - x0) * (column as f64 + 0.5) / columns as f64;
                let y = y0 + (y1 - y0) * (row as f64 + 0.5) / rows as f64;
                points.push([snap(x, width / 2.0), snap(y, height / 2.0)]);
            }
        }
        points
    }

    /// The aim marks' width as printed, mm: the size chosen, or less (in
    /// whole 5 mm) where it wouldn't leave 10 mm between neighbours and
    /// 3 mm to the field's edge.
    pub fn aim_size_mm(&self) -> f64 {
        let points = self.aim_points_mm();
        let [x0, y0, x1, y1] = self.field_mm();
        let mut room = f64::INFINITY;
        for (i, p) in points.iter().enumerate() {
            let edge = (p[0] - x0).min(x1 - p[0]).min(p[1] - y0).min(y1 - p[1]);
            room = room.min(2.0 * (edge - 3.0));
            for q in &points[i + 1..] {
                let apart = (p[0] - q[0]).abs().max((p[1] - q[1]).abs());
                room = room.min(apart - 10.0);
            }
        }
        let fitted = ((room / 5.0).floor() * 5.0).max(10.0);
        self.size.mm().min(fitted)
    }
}

/// The fixed value of a data corner module (`Some(true)` black), or `None`
/// for a module that carries a bit.
fn data_corner(r: usize, c: usize) -> Option<bool> {
    match (r, c) {
        (0, 0) => Some(true),
        (0, 4) | (4, 0) | (4, 4) => Some(false),
        _ => None,
    }
}

/// CRC-8 (polynomial x⁸ + x² + x + 1, zero start) of the low `bits` bits
/// of `value`, most significant first.
fn crc8(value: u32, bits: u32) -> u8 {
    let mut crc = 0u8;
    for i in (0..bits).rev() {
        let bit = ((value >> i) & 1) as u8;
        let top = crc >> 7;
        crc <<= 1;
        if bit ^ top == 1 {
            crc ^= CRC_POLYNOMIAL;
        }
    }
    crc
}

/// The corner and layout code a word carries, if its CRC holds.
fn decode_word(word: u32) -> Option<(Corner, u16)> {
    let payload = word >> 8;
    if crc8(payload, 13) != (word & 0xff) as u8 {
        return None;
    }
    let code = (payload & 0x7ff) as u16;
    Layout::from_code(code)?;
    Some((Corner::ALL[(payload >> 11) as usize & 3], code))
}

// ------------------------------------------------------------------ result

/// How a corner mark was placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    /// Found and its code read.
    Read,
    /// Found where the other marks put it, its code not read (a hole or a
    /// smudge in its modules).
    Found,
    /// Not found (covered, torn off, out of the photo): placed where the
    /// other three put it, so less exact.
    Estimated,
}

/// An MX target found on a photo.
#[derive(Clone, Debug, PartialEq)]
pub struct Marks {
    pub layout: Layout,
    /// The marks' centres, photo px: the sheet's top-left, top-right,
    /// bottom-right and bottom-left as printed, whichever way the photo is
    /// turned.
    pub centres_px: [[f64; 2]; 4],
    /// How each mark was placed, in the same order.
    pub seen: [Seen; 4],
    /// Quarter turns clockwise the photo shows the sheet at: 0 upright,
    /// 1 with the sheet's top on the right.
    pub quarter_turns: u8,
    /// The marks' edges' RMS distance from one flat sheet, px: a few
    /// tenths for a flat sheet, more for curled paper or a lens's
    /// distortion.
    pub fit_px: f64,
    /// `fit_px` in modules of the marks as seen: below about 0.05 for a
    /// flat sheet. Worth a warning above [`CURLED`]; with a mark
    /// estimated, the photo is refused above it.
    pub flatness: f64,
    /// Sheet mm to photo px.
    to_px: Matrix,
    /// Photo px to sheet mm.
    to_mm: Matrix,
}

/// A calibration from the marks and the aim points through it.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkCalibration {
    /// [`Calibration::sheet_corners`] through the four mark centres: its
    /// millimetres follow the sheet, whichever way the photo is turned,
    /// with the origin at the bottom-left mark, x along the bottom row of
    /// marks and y up the sheet.
    pub calibration: Calibration,
    /// The aim points in the calibration's millimetres (y up).
    pub aim_points_mm: Vec<Point>,
    /// The aim points on the photo, px, through the calibration.
    pub aim_points_px: Vec<Point>,
    /// The top-left mark's centre on the sheet, and the rectangle's height.
    top_left_mm: [f64; 2],
    height_mm: f64,
}

impl MarkCalibration {
    /// A point on the sheet (mm from its top-left corner, y down) in the
    /// calibration's millimetres.
    pub fn target_mm(&self, sheet_mm: [f64; 2]) -> Point {
        Point::new(
            sheet_mm[0] - self.top_left_mm[0],
            self.height_mm - (sheet_mm[1] - self.top_left_mm[1]),
        )
    }
}

impl Marks {
    /// The width and height of the rectangle through the marks' centres,
    /// mm, as printed.
    pub fn rectangle_mm(&self) -> [f64; 2] {
        self.layout.rectangle_mm()
    }

    /// The aim points on the sheet, mm from its top-left corner, y down.
    pub fn aim_points_mm(&self) -> Vec<[f64; 2]> {
        self.layout.aim_points_mm()
    }

    /// The aim points on the photo, px, through the homography fitted to
    /// all the marks' edges.
    pub fn aim_points_px(&self) -> Vec<[f64; 2]> {
        self.aim_points_mm()
            .into_iter()
            .filter_map(|p| self.sheet_to_px(p))
            .collect()
    }

    /// Where a point of the sheet (mm from its top-left corner, y down)
    /// shows on the photo, px.
    pub fn sheet_to_px(&self, mm: [f64; 2]) -> Option<[f64; 2]> {
        apply(&self.to_px, mm)
    }

    /// The point of the sheet (mm from its top-left corner, y down) a
    /// photo pixel shows.
    pub fn px_to_sheet(&self, px: [f64; 2]) -> Option<[f64; 2]> {
        apply(&self.to_mm, px)
    }

    /// A calibration through the mark centres in the sheet's own order and
    /// frame ([`Calibration::sheet_corners`]), with the aim points mapped
    /// through it: "up" is up the sheet, whichever way the photo is turned.
    pub fn calibration(&self) -> Result<MarkCalibration, CalibrationError> {
        let [width, height] = self.rectangle_mm();
        let corners = self.centres_px.map(|[x, y]| Point::new(x, y));
        let calibration = Calibration::sheet_corners(corners, width, height)?;
        let mut result = MarkCalibration {
            calibration,
            aim_points_mm: Vec::new(),
            aim_points_px: Vec::new(),
            top_left_mm: self.layout.mark_centres_mm()[0],
            height_mm: height,
        };
        for aim in self.aim_points_mm() {
            let mm = result.target_mm(aim);
            if let Some(px) = calibration.to_px(mm) {
                result.aim_points_mm.push(mm);
                result.aim_points_px.push(px);
            }
        }
        Ok(result)
    }
}

/// Why no MX target was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoMarks {
    /// The region is too small to look in.
    TooSmall,
    /// Nothing like an MX target's corner marks.
    NotFound,
    /// Marks like an MX target's, but none of their codes could be read.
    Unreadable,
    /// Only some marks were found; `true` for those seen, in the order
    /// top-left, top-right, bottom-right, bottom-left. Three are needed.
    OnlySome { seen: [bool; 4] },
    /// The marks don't lie on one flat sheet: two targets in the photo, or
    /// a folded or badly curled sheet.
    Inconsistent,
}

impl NoMarks {
    fn progress(self) -> u8 {
        match self {
            NoMarks::TooSmall => 0,
            NoMarks::NotFound => 1,
            NoMarks::Unreadable => 2,
            NoMarks::OnlySome { .. } => 3,
            NoMarks::Inconsistent => 4,
        }
    }
}

impl fmt::Display for NoMarks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoMarks::TooSmall => formatter.write_str("the area is too small to look for marks"),
            NoMarks::NotFound => formatter.write_str("no MX target corner marks were found"),
            NoMarks::Unreadable => {
                formatter.write_str("corner marks were found, but their codes couldn't be read")
            }
            NoMarks::OnlySome { seen } => write!(
                formatter,
                "only {} of the 4 corner marks were found",
                seen.iter().filter(|s| **s).count()
            ),
            NoMarks::Inconsistent => {
                formatter.write_str("the corner marks don't lie on one flat sheet")
            }
        }
    }
}

impl std::error::Error for NoMarks {}

// --------------------------------------------------------------- the search

/// The work copy's longest side to start with, px.
const WORK_LONG_EDGE_PX: usize = 1200;
/// The largest work copy looked at when marks are small, px.
const MAX_WORK_LONG_EDGE_PX: usize = 2400;
/// The region's shorter side must be at least this, photo px.
const MIN_SIDE_PX: usize = 48;
/// A [`Marks::flatness`] above this is a curled or bent sheet.
pub const CURLED: f64 = 0.12;
/// The smallest mark looked for, work px across.
const MIN_MARK_PX: f64 = 20.0;

/// Looks for an MX target's corner marks in `region` (`[x0, y0, x1, y1]`,
/// photo px) of `image`.
pub fn find_marks(image: &InkImage, region: [f64; 4]) -> Result<Marks, NoMarks> {
    let (iw, ih) = (f64::from(image.width), f64::from(image.height));
    if image.pixels.len() < image.width as usize * image.height as usize {
        return Err(NoMarks::TooSmall);
    }
    let clip = |v: f64, most: f64| {
        if v.is_finite() {
            v.clamp(0.0, most)
        } else {
            0.0
        }
    };
    let x0 = clip(region[0].min(region[2]), iw).floor() as usize;
    let y0 = clip(region[1].min(region[3]), ih).floor() as usize;
    let x1 = clip(region[0].max(region[2]), iw).ceil() as usize;
    let y1 = clip(region[1].max(region[3]), ih).ceil() as usize;
    if x1.saturating_sub(x0).min(y1.saturating_sub(y0)) < MIN_SIDE_PX {
        return Err(NoMarks::TooSmall);
    }
    let rect = [x0, y0, x1, y1];
    let long = (x1 - x0).max(y1 - y0);
    let mut factor = long.div_ceil(WORK_LONG_EDGE_PX).max(1);
    let mut failure = NoMarks::TooSmall;
    let mut best: Option<Marks> = None;
    let score = |marks: &Marks| {
        marks
            .seen
            .iter()
            .map(|s| match s {
                Seen::Read => 4,
                Seen::Found => 3,
                Seen::Estimated => 0,
            })
            .sum::<u32>()
    };
    loop {
        match search(image, rect, factor) {
            Ok(marks) => {
                if best.as_ref().is_none_or(|b| score(&marks) > score(b)) {
                    best = Some(marks);
                }
            }
            Err(reason) => {
                if reason.progress() >= failure.progress() {
                    failure = reason;
                }
                if reason == NoMarks::Inconsistent && best.is_none() {
                    return Err(reason);
                }
            }
        }
        // Small, blurred or holed marks may read better from a finer copy.
        let finer = factor / 2;
        let done = best.as_ref().is_some_and(|b| b.seen == [Seen::Read; 4]);
        if done || finer == 0 || long.div_ceil(finer) > MAX_WORK_LONG_EDGE_PX {
            return best.ok_or(failure);
        }
        factor = finer;
    }
}

/// A single-channel picture of floats; pixel `(x, y)` covers
/// `[x, x + 1) × [y, y + 1)`.
struct Plane {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

/// Something to sample between pixel centres.
trait Picture {
    /// The value at `(x, y)` (bilinear), or `None` off the picture.
    fn sample(&self, x: f64, y: f64) -> Option<f32>;
}

fn bilinear(width: usize, height: usize, x: f64, y: f64, at: impl Fn(usize) -> f32) -> Option<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    if !(fx >= 0.0 && fy >= 0.0) {
        return None;
    }
    let (ix, iy) = (fx as usize, fy as usize);
    if ix + 1 >= width || iy + 1 >= height {
        return None;
    }
    let ax = (fx - ix as f64) as f32;
    let ay = (fy - iy as f64) as f32;
    let i = iy * width + ix;
    let top = at(i) + ax * (at(i + 1) - at(i));
    let bottom = at(i + width) + ax * (at(i + width + 1) - at(i + width));
    Some(top + ay * (bottom - top))
}

impl Picture for Plane {
    fn sample(&self, x: f64, y: f64) -> Option<f32> {
        bilinear(self.width, self.height, x, y, |i| self.values[i])
    }
}

impl Picture for InkImage {
    fn sample(&self, x: f64, y: f64) -> Option<f32> {
        bilinear(self.width as usize, self.height as usize, x, y, |i| {
            f32::from(self.pixels[i])
        })
    }
}

/// The brightness of `image` over `[x0, x1) × [y0, y1)`, each work pixel
/// the mean of `factor` × `factor` photo pixels.
fn reduce(image: &InkImage, [x0, y0, x1, y1]: [usize; 4], factor: usize) -> Plane {
    let width = (x1 - x0) / factor;
    let height = (y1 - y0) / factor;
    let stride = image.width as usize;
    let mut values = vec![0f32; width * height];
    let mut row = vec![0u32; width];
    let scale = 1.0 / (factor * factor) as f32;
    for y in 0..height {
        row.iter_mut().for_each(|v| *v = 0);
        for dy in 0..factor {
            let start = (y0 + y * factor + dy) * stride + x0;
            let line = &image.pixels[start..start + width * factor];
            for (x, chunk) in line.chunks_exact(factor).enumerate() {
                row[x] += chunk.iter().map(|&p| u32::from(p)).sum::<u32>();
            }
        }
        for x in 0..width {
            values[y * width + x] = row[x] as f32 * scale;
        }
    }
    Plane {
        width,
        height,
        values,
    }
}

/// Ink: pixels darker than three quarters of the mean round them.
fn threshold(plane: &Plane) -> Vec<bool> {
    let (w, h) = (plane.width, plane.height);
    // Whole grey levels summed: at most 255 × 2400 × 2400, within a u32.
    let mut integral = vec![0u32; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut sum = 0u32;
        for x in 0..w {
            sum += plane.values[y * w + x].round() as u32;
            integral[(y + 1) * (w + 1) + x + 1] = integral[y * (w + 1) + x + 1] + sum;
        }
    }
    let radius = (w.max(h) / 30).max(8);
    let mut dark = vec![false; w * h];
    for y in 0..h {
        let ya = y.saturating_sub(radius);
        let yb = (y + radius + 1).min(h);
        for x in 0..w {
            let xa = x.saturating_sub(radius);
            let xb = (x + radius + 1).min(w);
            let sum = integral[yb * (w + 1) + xb] - integral[ya * (w + 1) + xb]
                + integral[ya * (w + 1) + xa]
                - integral[yb * (w + 1) + xa];
            let mean = f64::from(sum) / ((yb - ya) * (xb - xa)) as f64;
            let v = f64::from(plane.values[y * w + x]);
            dark[y * w + x] = v < 0.75 * mean && mean - v > 8.0;
        }
    }
    dark
}

/// A run of ink pixels on row `y`, `[x0, x1)`.
#[derive(Clone, Copy, Debug)]
struct Run {
    y: usize,
    x0: usize,
    x1: usize,
}

fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

/// The 8-connected shapes of ink, each as its runs from the top, keeping
/// only those whose bounding box is between `least` and `most` px across.
fn shapes(dark: &[bool], w: usize, h: usize, least: f64, most: f64) -> Vec<Vec<Run>> {
    let mut runs: Vec<Run> = Vec::new();
    let mut row_start = vec![0usize; h + 1];
    for y in 0..h {
        row_start[y] = runs.len();
        let line = &dark[y * w..(y + 1) * w];
        let mut x = 0;
        while x < w {
            if line[x] {
                let x0 = x;
                while x < w && line[x] {
                    x += 1;
                }
                runs.push(Run { y, x0, x1: x });
            } else {
                x += 1;
            }
        }
    }
    row_start[h] = runs.len();
    let mut parents: Vec<usize> = (0..runs.len()).collect();
    for y in 1..h {
        let (mut a, a_end) = (row_start[y - 1], row_start[y]);
        let (mut b, b_end) = (row_start[y], row_start[y + 1]);
        while a < a_end && b < b_end {
            let (ra, rb) = (runs[a], runs[b]);
            if ra.x0 <= rb.x1 && rb.x0 <= ra.x1 {
                let (pa, pb) = (root(&mut parents, a), root(&mut parents, b));
                if pa != pb {
                    parents[pa.max(pb)] = pa.min(pb);
                }
            }
            if ra.x1 < rb.x1 {
                a += 1;
            } else {
                b += 1;
            }
        }
    }
    let mut index = vec![usize::MAX; runs.len()];
    let mut groups: Vec<Vec<Run>> = Vec::new();
    let mut boxes: Vec<[usize; 4]> = Vec::new();
    for i in 0..runs.len() {
        let r = root(&mut parents, i);
        if index[r] == usize::MAX {
            index[r] = groups.len();
            groups.push(Vec::new());
            boxes.push([usize::MAX, usize::MAX, 0, 0]);
        }
        let g = index[r];
        let run = runs[i];
        groups[g].push(run);
        let b = &mut boxes[g];
        b[0] = b[0].min(run.x0);
        b[1] = b[1].min(run.y);
        b[2] = b[2].max(run.x1);
        b[3] = b[3].max(run.y + 1);
    }
    groups
        .into_iter()
        .zip(boxes)
        .filter(|(_, b)| {
            let (bw, bh) = ((b[2] - b[0]) as f64, (b[3] - b[1]) as f64);
            bw.max(bh) >= least && bw.max(bh) <= most && bw.min(bh) >= 0.3 * least
        })
        .map(|(group, _)| group)
        .collect()
}

fn cross(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// The polygon's area, positive when it runs clockwise on screen (y down).
fn area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        / 2.0
}

fn hull(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let mut lower: Vec<[f64; 2]> = Vec::new();
    for &p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<[f64; 2]> = Vec::new();
    for &p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// The quadrilateral a shape of ink makes, clockwise on screen, when it
/// looks like a mark's frame: nearly all of its hull, with a hole.
fn quad_of(runs: &[Run]) -> Option<[[f64; 2]; 4]> {
    let count: usize = runs.iter().map(|r| r.x1 - r.x0).sum();
    let mut points = Vec::new();
    let mut i = 0;
    while i < runs.len() {
        let y = runs[i].y;
        let (mut x0, mut x1) = (runs[i].x0, runs[i].x1);
        while i < runs.len() && runs[i].y == y {
            x0 = x0.min(runs[i].x0);
            x1 = x1.max(runs[i].x1);
            i += 1;
        }
        let (fy, fx0, fx1) = (y as f64, x0 as f64, x1 as f64);
        points.extend([[fx0, fy], [fx0, fy + 1.0], [fx1, fy], [fx1, fy + 1.0]]);
    }
    let hull = hull(points);
    if hull.len() < 4 {
        return None;
    }
    let hull_area = area(&hull).abs();
    let (mut a, mut c, mut far) = (0, 0, 0.0);
    for i in 0..hull.len() {
        for j in i + 1..hull.len() {
            let d = (hull[i][0] - hull[j][0]).hypot(hull[i][1] - hull[j][1]);
            if d > far {
                (a, c, far) = (i, j, d);
            }
        }
    }
    let (mut b, mut d) = (a, a);
    let (mut most, mut least) = (0.0, 0.0);
    for (i, &p) in hull.iter().enumerate() {
        let side = cross(hull[a], hull[c], p);
        if side > most {
            (b, most) = (i, side);
        }
        if side < least {
            (d, least) = (i, side);
        }
    }
    if b == a || d == a {
        return None;
    }
    let mut quad = [hull[a], hull[b], hull[c], hull[d]];
    let quad_area = area(&quad);
    if quad_area < 0.0 {
        quad.reverse();
    }
    let quad_area = quad_area.abs();
    if quad_area < 0.85 * hull_area {
        return None;
    }
    let fill = count as f64 / quad_area;
    if !(0.2..=0.62).contains(&fill) {
        return None;
    }
    let sides: Vec<f64> = (0..4)
        .map(|i| {
            let (p, q) = (quad[i], quad[(i + 1) % 4]);
            (p[0] - q[0]).hypot(p[1] - q[1])
        })
        .collect();
    let (short, long) = sides
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(s, l), &v| (s.min(v), l.max(v)));
    if long > 3.0 * short || short < 0.6 * MIN_MARK_PX {
        return None;
    }
    for i in 0..4 {
        let (p, q, r) = (quad[(i + 3) % 4], quad[i], quad[(i + 1) % 4]);
        let u = [p[0] - q[0], p[1] - q[1]];
        let v = [r[0] - q[0], r[1] - q[1]];
        let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]));
        if cos.abs() > 0.8 {
            return None;
        }
    }
    Some(quad)
}

/// A mark's square in module units, its corners clockwise from the top
/// left.
const SQUARE: [[f64; 2]; 4] = [[0.0, 0.0], [9.0, 0.0], [9.0, 9.0], [0.0, 9.0]];

/// What sampling a quadrilateral's modules showed.
#[derive(Clone, Copy, Debug)]
struct Reading {
    /// The quadrilateral's corners, starting at the mark's top left when
    /// `oriented`.
    quad: [[f64; 2]; 4],
    /// A dark frame round a light ring.
    framed: bool,
    /// Exactly one dark data corner, so the mark's top is known.
    oriented: bool,
    word: Option<(Corner, u16)>,
}

/// Samples the 9 × 9 modules of the mark whose outer corners are `quad`.
fn sample_modules(picture: &impl Picture, quad: &[[f64; 2]; 4]) -> Option<[[f32; 9]; 9]> {
    let h = fit_homography(&SQUARE, quad)?;
    let mut values = [[0f32; 9]; 9];
    for i in 0..9 {
        for j in 0..9 {
            let mut sum = 0.0;
            let mut n = 0;
            for di in [-0.22, 0.0, 0.22] {
                for dj in [-0.22, 0.0, 0.22] {
                    let p = apply(&h, [j as f64 + 0.5 + dj, i as f64 + 0.5 + di])?;
                    if let Some(v) = picture.sample(p[0], p[1]) {
                        sum += v;
                        n += 1;
                    }
                }
            }
            if n == 0 {
                return None;
            }
            values[i][j] = sum / n as f32;
        }
    }
    Some(values)
}

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

/// The frame's ink and the ring's paper levels, when they look like a
/// mark's: `(ink, paper)`.
fn frame_levels(values: &[[f32; 9]; 9], strict: bool) -> Option<(f32, f32)> {
    let mut frame = Vec::with_capacity(32);
    let mut ring = Vec::with_capacity(24);
    for i in 0..9 {
        for j in 0..9 {
            let edge = i.min(j).min(8 - i).min(8 - j);
            match edge {
                0 => frame.push(values[i][j]),
                1 => ring.push(values[i][j]),
                _ => {}
            }
        }
    }
    let ink = median(frame.clone());
    let paper = median(ring.clone());
    if paper - ink < 12.0 || paper - ink < 0.2 * paper {
        return None;
    }
    let mid = 0.5 * (ink + paper);
    let dark = frame.iter().filter(|&&v| v < mid).count();
    let light = ring.iter().filter(|&&v| v >= mid).count();
    let (least_dark, least_light) = if strict { (27, 20) } else { (22, 17) };
    (dark >= least_dark && light >= least_light).then_some((ink, paper))
}

/// Reads a mark through its outer corners `quad` (clockwise on screen).
fn read_mark(picture: &impl Picture, quad: [[f64; 2]; 4]) -> Reading {
    let mut reading = Reading {
        quad,
        framed: false,
        oriented: false,
        word: None,
    };
    let Some(values) = sample_modules(picture, &quad) else {
        return reading;
    };
    let Some((ink, paper)) = frame_levels(&values, true) else {
        return reading;
    };
    reading.framed = true;
    let mid = 0.5 * (ink + paper);
    let mut data = [[false; DATA]; DATA];
    for r in 0..DATA {
        for c in 0..DATA {
            data[r][c] = values[r + 2][c + 2] < mid;
        }
    }
    // Turn the data until its one dark corner is at the top left: each
    // clockwise turn makes the quadrilateral's previous corner the top left.
    for turns in 0..4 {
        let corners = [data[0][0], data[0][4], data[4][4], data[4][0]];
        if corners.iter().filter(|&&d| d).count() != 1 {
            return reading;
        }
        if data[0][0] {
            reading.oriented = true;
            reading.quad = std::array::from_fn(|k| quad[(4 - turns + k) % 4]);
            let mut word = 0u32;
            for r in 0..DATA {
                for c in 0..DATA {
                    if data_corner(r, c).is_none() {
                        word = (word << 1) | u32::from(data[r][c]);
                    }
                }
            }
            reading.word = decode_word(word);
            return reading;
        }
        data = std::array::from_fn(|i| std::array::from_fn(|j| data[DATA - 1 - j][i]));
    }
    reading
}

/// A mark measured on the photo: its homography from module units to
/// photo px, fitted to its frame's edges.
#[derive(Clone, Copy, Debug)]
struct Measured {
    h: Matrix,
    /// The frame's outer and inner corners, photo px, with their module
    /// coordinates.
    points: [([f64; 2], [f64; 2]); 8],
    count: usize,
    seen: Seen,
}

impl Measured {
    fn centre(&self) -> Option<[f64; 2]> {
        apply(&self.h, [4.5, 4.5])
    }
}

/// An edge of the frame in module units: from, to, and the unit step
/// towards the ink.
struct Edge {
    from: [f64; 2],
    to: [f64; 2],
    ink: [f64; 2],
    /// The part of the edge sampled, as shares of its length.
    span: [f64; 2],
}

const EDGES: [Edge; 8] = [
    Edge {
        from: [0.0, 0.0],
        to: [9.0, 0.0],
        ink: [0.0, 1.0],
        span: [0.15, 0.85],
    },
    Edge {
        from: [9.0, 0.0],
        to: [9.0, 9.0],
        ink: [-1.0, 0.0],
        span: [0.15, 0.85],
    },
    Edge {
        from: [9.0, 9.0],
        to: [0.0, 9.0],
        ink: [0.0, -1.0],
        span: [0.15, 0.85],
    },
    Edge {
        from: [0.0, 9.0],
        to: [0.0, 0.0],
        ink: [1.0, 0.0],
        span: [0.15, 0.85],
    },
    Edge {
        from: [1.0, 1.0],
        to: [8.0, 1.0],
        ink: [0.0, -1.0],
        span: [0.2, 0.8],
    },
    Edge {
        from: [8.0, 1.0],
        to: [8.0, 8.0],
        ink: [1.0, 0.0],
        span: [0.2, 0.8],
    },
    Edge {
        from: [8.0, 8.0],
        to: [1.0, 8.0],
        ink: [0.0, 1.0],
        span: [0.2, 0.8],
    },
    Edge {
        from: [1.0, 8.0],
        to: [1.0, 1.0],
        ink: [-1.0, 0.0],
        span: [0.2, 0.8],
    },
];

/// The module coordinates of the frame's outer then inner corners, each
/// where edges `i` and `i + 3 (mod 4)` meet.
const FRAME_CORNERS: [[f64; 2]; 8] = [
    [0.0, 0.0],
    [9.0, 0.0],
    [9.0, 9.0],
    [0.0, 9.0],
    [1.0, 1.0],
    [8.0, 1.0],
    [8.0, 8.0],
    [1.0, 8.0],
];

/// Where the edge from ink to paper crosses the profile from `dark` to
/// `light`, as a share of the way, by the centroid of the rise.
fn edge_on(picture: &impl Picture, dark: [f64; 2], light: [f64; 2]) -> Option<f64> {
    let length = (light[0] - dark[0]).hypot(light[1] - dark[1]);
    if length < 2.0 {
        return None;
    }
    let steps = ((length / 0.25).ceil() as usize).clamp(8, 400);
    let mut values = Vec::with_capacity(steps + 1);
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        values.push(picture.sample(
            dark[0] + t * (light[0] - dark[0]),
            dark[1] + t * (light[1] - dark[1]),
        )?);
    }
    let ends = 1 + steps / 8;
    let ink: f32 = values[..ends].iter().sum::<f32>() / ends as f32;
    let paper: f32 = values[steps + 1 - ends..].iter().sum::<f32>() / ends as f32;
    if paper - ink < 10.0 {
        return None;
    }
    let rise: Vec<f32> = (1..steps).map(|k| values[k + 1] - values[k - 1]).collect();
    let (peak, &top) = rise.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    if top <= 0.0 || peak == 0 || peak + 1 == rise.len() {
        return None;
    }
    let floor = 0.3 * top;
    let (mut lo, mut hi) = (peak, peak);
    while lo > 0 && rise[lo - 1] > floor {
        lo -= 1;
    }
    while hi + 1 < rise.len() && rise[hi + 1] > floor {
        hi += 1;
    }
    let (mut weight, mut sum) = (0.0f64, 0.0f64);
    for k in lo..=hi {
        let w = f64::from(rise[k] - floor);
        weight += w;
        sum += w * (k + 1) as f64;
    }
    (weight > 0.0).then(|| sum / weight / steps as f64)
}

/// A line through points: a point on it and its unit direction, fitted by
/// total least squares with far points left out.
fn fit_line(points: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2])> {
    let fit = |points: &[[f64; 2]]| -> Option<([f64; 2], [f64; 2])> {
        let n = points.len() as f64;
        if points.len() < 4 {
            return None;
        }
        let mx = points.iter().map(|p| p[0]).sum::<f64>() / n;
        let my = points.iter().map(|p| p[1]).sum::<f64>() / n;
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for p in points {
            let (dx, dy) = (p[0] - mx, p[1] - my);
            sxx += dx * dx;
            sxy += dx * dy;
            syy += dy * dy;
        }
        let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        Some(([mx, my], [angle.cos(), angle.sin()]))
    };
    let (mut at, mut along) = fit(points)?;
    for _ in 0..2 {
        let distance = |p: &[f64; 2]| ((p[0] - at[0]) * along[1] - (p[1] - at[1]) * along[0]).abs();
        let mut spread: Vec<f64> = points.iter().map(distance).collect();
        spread.sort_by(f64::total_cmp);
        let limit = (3.0 * spread[spread.len() / 2]).max(0.35);
        let kept: Vec<[f64; 2]> = points
            .iter()
            .copied()
            .filter(|p| distance(p) <= limit)
            .collect();
        if kept.len() < (points.len() * 3).div_ceil(5) {
            return None;
        }
        (at, along) = fit(&kept)?;
    }
    Some((at, along))
}

fn intersect(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> Option<[f64; 2]> {
    let ((p, d), (q, e)) = (a, b);
    let det = d[0] * e[1] - d[1] * e[0];
    if det.abs() < 0.2 {
        return None;
    }
    let t = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / det;
    Some([p[0] + t * d[0], p[1] + t * d[1]])
}

/// Measures a mark from a first guess of its homography (module units to
/// `picture` px): its frame's edges are found and the homography fitted to
/// their corners, twice.
fn measure(picture: &impl Picture, mut h: Matrix) -> Option<Measured> {
    let mut measured = None;
    for _ in 0..2 {
        let module = module_px(&h)?;
        let samples = ((9.0 * module / 3.0) as usize).clamp(8, 32);
        let mut lines = [None; 8];
        for (e, edge) in EDGES.iter().enumerate() {
            let mut points = Vec::with_capacity(samples);
            for s in 0..samples {
                let t =
                    edge.span[0] + (edge.span[1] - edge.span[0]) * s as f64 / (samples - 1) as f64;
                let p = [
                    edge.from[0] + t * (edge.to[0] - edge.from[0]),
                    edge.from[1] + t * (edge.to[1] - edge.from[1]),
                ];
                let dark = apply(&h, [p[0] + 0.5 * edge.ink[0], p[1] + 0.5 * edge.ink[1]])?;
                let light = apply(&h, [p[0] - 0.5 * edge.ink[0], p[1] - 0.5 * edge.ink[1]])?;
                if let Some(t) = edge_on(picture, dark, light) {
                    points.push([
                        dark[0] + t * (light[0] - dark[0]),
                        dark[1] + t * (light[1] - dark[1]),
                    ]);
                }
            }
            if points.len() * 2 >= samples {
                lines[e] = fit_line(&points);
            }
        }
        let mut found = [([0.0; 2], [0.0; 2]); 8];
        let mut count = 0;
        for group in [0, 4] {
            for i in 0..4 {
                let (a, b) = (lines[group + i], lines[group + (i + 3) % 4]);
                if let (Some(a), Some(b)) = (a, b)
                    && let Some(p) = intersect(a, b)
                {
                    let guess = apply(&h, FRAME_CORNERS[group + i])?;
                    if (p[0] - guess[0]).hypot(p[1] - guess[1]) < 0.8 * module {
                        found[count] = (FRAME_CORNERS[group + i], p);
                        count += 1;
                    }
                }
            }
        }
        if count < 6 {
            return None;
        }
        let from: Vec<[f64; 2]> = found[..count].iter().map(|p| p.0).collect();
        let to: Vec<[f64; 2]> = found[..count].iter().map(|p| p.1).collect();
        h = fit_homography(&from, &to)?;
        measured = Some(Measured {
            h,
            points: found,
            count,
            seen: Seen::Read,
        });
    }
    measured
}

/// About how many px one module spans, from a mark's homography.
fn module_px(h: &Matrix) -> Option<f64> {
    let quad: Vec<[f64; 2]> = SQUARE.iter().map(|&p| apply(h, p)).collect::<Option<_>>()?;
    Some(area(&quad).abs().sqrt() / 9.0)
}

fn search(image: &InkImage, rect: [usize; 4], factor: usize) -> Result<Marks, NoMarks> {
    let plane = reduce(image, rect, factor);
    if plane.width.min(plane.height) < 32 {
        return Err(NoMarks::TooSmall);
    }
    let dark = threshold(&plane);
    let most = 0.5 * plane.width.min(plane.height) as f64;
    let origin = [rect[0] as f64, rect[1] as f64];
    let scale = factor as f64;
    let mut readings: Vec<Reading> = Vec::new();
    for shape in shapes(&dark, plane.width, plane.height, MIN_MARK_PX, most) {
        let Some(quad) = quad_of(&shape) else {
            continue;
        };
        let Some(coarse) = fit_homography(&SQUARE, &quad) else {
            continue;
        };
        // A first measurement on the work copy places the modules better
        // than the shape's outline does.
        let quad = measure(&plane, coarse)
            .and_then(|m| {
                SQUARE
                    .iter()
                    .map(|&p| apply(&m.h, p))
                    .collect::<Option<Vec<_>>>()
            })
            .and_then(|v| <[[f64; 2]; 4]>::try_from(v).ok())
            .unwrap_or(quad);
        let mut reading = read_mark(&plane, quad);
        if !reading.framed {
            continue;
        }
        reading.quad = reading
            .quad
            .map(|[x, y]| [origin[0] + scale * x, origin[1] + scale * y]);
        readings.push(reading);
    }
    assemble(image, &readings)
}

/// The marks from the readings: those whose code was read, those found
/// where they put the others, and those placed from them.
fn assemble(image: &InkImage, readings: &[Reading]) -> Result<Marks, NoMarks> {
    let size = |r: &Reading| area(&r.quad).abs();
    let mut codes: Vec<(u16, usize, f64)> = Vec::new();
    for reading in readings {
        if let Some((_, code)) = reading.word {
            match codes.iter_mut().find(|c| c.0 == code) {
                Some(entry) => {
                    entry.1 += 1;
                    entry.2 += size(reading);
                }
                None => codes.push((code, 1, size(reading))),
            }
        }
    }
    let Some(&(code, _, _)) = codes
        .iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)))
    else {
        return Err(if readings.iter().any(|r| r.oriented) {
            NoMarks::Unreadable
        } else {
            NoMarks::NotFound
        });
    };
    let layout = Layout::from_code(code).ok_or(NoMarks::Unreadable)?;
    let mut chosen: [Option<&Reading>; 4] = [None; 4];
    for reading in readings {
        if let Some((corner, c)) = reading.word
            && c == code
            && chosen[corner.index()].is_none_or(|old| size(old) < size(reading))
        {
            chosen[corner.index()] = Some(reading);
        }
    }
    let mut marks: [Option<Measured>; 4] = [None; 4];
    for k in 0..4 {
        if let Some(reading) = chosen[k] {
            marks[k] = fit_homography(&SQUARE, &reading.quad).and_then(|h| measure(image, h));
        }
    }
    let seen = |marks: &[Option<Measured>; 4]| marks.map(|m| m.is_some());
    if marks.iter().flatten().count() < 2 {
        return Err(NoMarks::OnlySome { seen: seen(&marks) });
    }
    // The marks not read, looked for where the others put them.
    let to_px = fit_sheet(&layout, &marks).ok_or(NoMarks::Inconsistent)?;
    let read = marks.iter().flatten().count();
    for k in 0..4 {
        if marks[k].is_some() {
            continue;
        }
        let Some(predicted) = mark_homography(&layout, k, &to_px) else {
            continue;
        };
        let Some(module) = module_px(&predicted) else {
            continue;
        };
        let centre = apply(&predicted, [4.5, 4.5]).unwrap_or([f64::NAN; 2]);
        let reach = if read >= 3 { 2.0 } else { 6.0 } * module;
        let nearby = readings
            .iter()
            .filter(|r| r.word.is_none_or(|(_, c)| c != code))
            .filter_map(|r| {
                let h = fit_homography(&SQUARE, &r.quad)?;
                let at = apply(&h, [4.5, 4.5])?;
                let distance = (at[0] - centre[0]).hypot(at[1] - centre[1]);
                (distance < reach).then_some((r, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(r, _)| r);
        let guess = match nearby {
            Some(reading) => {
                // Start the quadrilateral at the corner nearest where the
                // others put the mark's top left.
                let top_left = apply(&predicted, [0.0, 0.0]).unwrap_or(centre);
                let start = (0..4)
                    .min_by(|&a, &b| {
                        let d = |i: usize| {
                            let p = reading.quad[i];
                            (p[0] - top_left[0]).hypot(p[1] - top_left[1])
                        };
                        d(a).total_cmp(&d(b))
                    })
                    .unwrap_or(0);
                let quad: [[f64; 2]; 4] = std::array::from_fn(|i| reading.quad[(start + i) % 4]);
                fit_homography(&SQUARE, &quad)
            }
            None if read >= 3 => Some(predicted),
            None => None,
        };
        let Some(mut found) = guess.and_then(|h| measure(image, h)) else {
            continue;
        };
        let at = found.centre().unwrap_or([f64::NAN; 2]);
        let framed = sample_modules(image, &SQUARE.map(|p| apply(&found.h, p).unwrap_or(p)))
            .and_then(|values| frame_levels(&values, false))
            .is_some();
        if framed && (at[0] - centre[0]).hypot(at[1] - centre[1]) < reach {
            found.seen = Seen::Found;
            marks[k] = Some(found);
        }
    }
    if marks.iter().flatten().count() < 3 {
        return Err(NoMarks::OnlySome { seen: seen(&marks) });
    }
    let mut to_px = fit_sheet(&layout, &marks).ok_or(NoMarks::Inconsistent)?;
    // A mark well off the sheet the others make is left out, if three
    // others remain.
    loop {
        let worst = (0..4)
            .filter_map(|k| {
                let m = marks[k].as_ref()?;
                Some((k, mark_residual(&layout, k, m, &to_px)?, module_px(&m.h)?))
            })
            .max_by(|a, b| (a.1 / a.2).total_cmp(&(b.1 / b.2)));
        let Some((k, residual, module)) = worst else {
            return Err(NoMarks::Inconsistent);
        };
        if residual <= (0.35 * module).max(1.5) {
            break;
        }
        if marks.iter().flatten().count() <= 3 {
            return Err(NoMarks::Inconsistent);
        }
        marks[k] = None;
        to_px = fit_sheet(&layout, &marks).ok_or(NoMarks::Inconsistent)?;
    }
    let to_mm = invert(&to_px).ok_or(NoMarks::Inconsistent)?;
    let sheet_centres = layout.mark_centres_mm();
    let mut centres_px = [[0.0; 2]; 4];
    let mut seen_as = [Seen::Estimated; 4];
    for k in 0..4 {
        let measured = marks[k].as_ref().and_then(|m| Some((m.centre()?, m.seen)));
        match measured {
            Some((centre, how)) => {
                centres_px[k] = centre;
                seen_as[k] = how;
            }
            None => {
                centres_px[k] = apply(&to_px, sheet_centres[k]).ok_or(NoMarks::Inconsistent)?;
            }
        }
    }
    let (mut sum, mut n) = (0.0, 0);
    for k in 0..4 {
        if let Some(m) = &marks[k] {
            for &(module, px) in &m.points[..m.count] {
                if let Some(p) = apply(&to_px, sheet_point(&layout, k, module)) {
                    sum += (p[0] - px[0]).powi(2) + (p[1] - px[1]).powi(2);
                    n += 1;
                }
            }
        }
    }
    let fit_px = (sum / n.max(1) as f64).sqrt();
    let modules: Vec<f64> = marks
        .iter()
        .flatten()
        .filter_map(|m| module_px(&m.h))
        .collect();
    let flatness = fit_px / (modules.iter().sum::<f64>() / modules.len().max(1) as f64);
    // Three marks on a curled sheet still fit some homography well; the
    // fourth placed from it can be millimetres out.
    if seen_as.contains(&Seen::Estimated) && (flatness.is_nan() || flatness > CURLED) {
        return Err(NoMarks::Inconsistent);
    }
    let across = [
        centres_px[1][0] - centres_px[0][0],
        centres_px[1][1] - centres_px[0][1],
    ];
    let turn = across[1].atan2(across[0]).to_degrees();
    let quarter_turns = ((turn / 90.0).round() as i64).rem_euclid(4) as u8;
    Ok(Marks {
        layout,
        centres_px,
        seen: seen_as,
        quarter_turns,
        fit_px,
        flatness,
        to_px,
        to_mm,
    })
}

/// A point of mark `k` in module units, on the sheet (mm, y down).
fn sheet_point(layout: &Layout, k: usize, module: [f64; 2]) -> [f64; 2] {
    let centre = layout.mark_centres_mm()[k];
    let u = layout.module_mm();
    [
        centre[0] + (module[0] - 4.5) * u,
        centre[1] + (module[1] - 4.5) * u,
    ]
}

/// Where the sheet homography puts mark `k`: module units to photo px.
fn mark_homography(layout: &Layout, k: usize, to_px: &Matrix) -> Option<Matrix> {
    let quad: Vec<[f64; 2]> = SQUARE
        .iter()
        .map(|&p| apply(to_px, sheet_point(layout, k, p)))
        .collect::<Option<_>>()?;
    fit_homography(&SQUARE, &quad)
}

/// The sheet homography (mm to photo px) fitted to every measured frame
/// corner.
fn fit_sheet(layout: &Layout, marks: &[Option<Measured>; 4]) -> Option<Matrix> {
    let mut from = Vec::new();
    let mut to = Vec::new();
    for k in 0..4 {
        if let Some(m) = &marks[k] {
            for &(module, px) in &m.points[..m.count] {
                from.push(sheet_point(layout, k, module));
                to.push(px);
            }
        }
    }
    fit_homography(&from, &to)
}

/// The RMS distance of mark `k`'s corners from where the sheet homography
/// puts them, px.
fn mark_residual(layout: &Layout, k: usize, m: &Measured, to_px: &Matrix) -> Option<f64> {
    let mut sum = 0.0;
    for &(module, px) in &m.points[..m.count] {
        let p = apply(to_px, sheet_point(layout, k, module))?;
        sum += (p[0] - px[0]).powi(2) + (p[1] - px[1]).powi(2);
    }
    Some((sum / m.count as f64).sqrt())
}

// ----------------------------------------------------------------- algebra

fn apply(m: &Matrix, [x, y]: [f64; 2]) -> Option<[f64; 2]> {
    let w = m[2][0] * x + m[2][1] * y + m[2][2];
    if w.abs() < 1e-12 || !w.is_finite() {
        return None;
    }
    let p = [
        (m[0][0] * x + m[0][1] * y + m[0][2]) / w,
        (m[1][0] * x + m[1][1] * y + m[1][2]) / w,
    ];
    (p[0].is_finite() && p[1].is_finite()).then_some(p)
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn invert(m: &Matrix) -> Option<Matrix> {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let adjugate = [
        [c(1, 2, 1, 2), -c(0, 2, 1, 2), c(0, 1, 1, 2)],
        [-c(1, 2, 0, 2), c(0, 2, 0, 2), -c(0, 1, 0, 2)],
        [c(1, 2, 0, 1), -c(0, 2, 0, 1), c(0, 1, 0, 1)],
    ];
    let det = m[0][0] * adjugate[0][0] + m[0][1] * adjugate[1][0] + m[0][2] * adjugate[2][0];
    if det.abs() < 1e-300 || !det.is_finite() {
        return None;
    }
    Some(adjugate.map(|row| row.map(|value| value / det)))
}

/// A similarity that moves the points' centroid to the origin and their
/// mean distance from it to √2.
fn normaliser(points: &[[f64; 2]]) -> Option<Matrix> {
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p[0]).sum::<f64>() / n;
    let my = points.iter().map(|p| p[1]).sum::<f64>() / n;
    let spread = points
        .iter()
        .map(|p| (p[0] - mx).hypot(p[1] - my))
        .sum::<f64>()
        / n;
    if spread.is_nan() || spread <= 1e-12 {
        return None;
    }
    let s = std::f64::consts::SQRT_2 / spread;
    Some([[s, 0.0, -s * mx], [0.0, s, -s * my], [0.0, 0.0, 1.0]])
}

/// The homography taking each `from` point to its `to` point (least
/// squares for more than four).
fn fit_homography(from: &[[f64; 2]], to: &[[f64; 2]]) -> Option<Matrix> {
    if from.len() < 4 || from.len() != to.len() {
        return None;
    }
    let tf = normaliser(from)?;
    let tt = normaliser(to)?;
    let mut ata = [[0.0f64; 8]; 8];
    let mut atb = [0.0f64; 8];
    for (p, q) in from.iter().zip(to) {
        let [x, y] = apply(&tf, *p)?;
        let [u, v] = apply(&tt, *q)?;
        for (row, rhs) in [
            ([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u),
            ([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v),
        ] {
            for i in 0..8 {
                atb[i] += row[i] * rhs;
                for j in 0..8 {
                    ata[i][j] += row[i] * row[j];
                }
            }
        }
    }
    let h = solve(ata, atb)?;
    let normalised = [[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]];
    Some(multiply(&invert(&tt)?, &multiply(&normalised, &tf)))
}

/// Solves `a x = b` by Gaussian elimination with partial pivoting.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..N {
            let f = a[row][col] / a[col][col];
            for k in col..N {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let s: f64 = (row + 1..N).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - s) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// A random number source for the pictures.
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn gaussian(&mut self) -> f64 {
            let (u, v) = (self.next().max(1e-12), self.next());
            (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
        }
    }

    /// A phone's view of the sheet: tilted `tilt` degrees about an axis
    /// `axis` degrees round from the sheet's x, turned `roll` degrees, and
    /// filling `fill` of the photo. Sheet mm to photo px.
    fn view(
        sheet: [f64; 2],
        photo: [u32; 2],
        tilt: f64,
        axis: f64,
        roll: f64,
        fill: f64,
    ) -> Matrix {
        let (st, ct) = tilt.to_radians().sin_cos();
        let (sa, ca) = axis.to_radians().sin_cos();
        let (x, y) = (ca, sa);
        // Rodrigues: a turn of `tilt` about the in-plane axis (x, y, 0).
        let r = [
            [ct + x * x * (1.0 - ct), x * y * (1.0 - ct), y * st],
            [x * y * (1.0 - ct), ct + y * y * (1.0 - ct), -x * st],
            [-y * st, x * st, ct],
        ];
        let (sr, cr) = roll.to_radians().sin_cos();
        let roll = [[cr, -sr, 0.0], [sr, cr, 0.0], [0.0, 0.0, 1.0]];
        let r = multiply(&roll, &r);
        let distance = 0.9 * sheet[0].max(sheet[1]);
        // Columns: the sheet's x and y axes, and its middle, seen by the camera.
        let centred = [
            [
                r[0][0],
                r[0][1],
                -r[0][0] * sheet[0] / 2.0 - r[0][1] * sheet[1] / 2.0,
            ],
            [
                r[1][0],
                r[1][1],
                -r[1][0] * sheet[0] / 2.0 - r[1][1] * sheet[1] / 2.0,
            ],
            [
                r[2][0],
                r[2][1],
                -r[2][0] * sheet[0] / 2.0 - r[2][1] * sheet[1] / 2.0 + distance,
            ],
        ];
        let corners: Vec<[f64; 2]> = [[0.0, 0.0], [sheet[0], 0.0], sheet, [0.0, sheet[1]]]
            .iter()
            .map(|&p| apply(&centred, p).unwrap())
            .collect();
        let (lo, hi) = corners.iter().fold(
            ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
            |(lo, hi), p| {
                (
                    [lo[0].min(p[0]), lo[1].min(p[1])],
                    [hi[0].max(p[0]), hi[1].max(p[1])],
                )
            },
        );
        let (pw, ph) = (f64::from(photo[0]), f64::from(photo[1]));
        let s = fill * (pw / (hi[0] - lo[0])).min(ph / (hi[1] - lo[1]));
        let fit = [
            [s, 0.0, pw / 2.0 - s * (lo[0] + hi[0]) / 2.0 + 0.37],
            [0.0, s, ph / 2.0 - s * (lo[1] + hi[1]) / 2.0 - 0.21],
            [0.0, 0.0, 1.0],
        ];
        multiply(&fit, &centred)
    }

    /// What is printed where, worked out once for a picture.
    struct Print {
        modules: [[[bool; 9]; 9]; 4],
        centres: [[f64; 2]; 4],
        aims: Vec<[f64; 2]>,
        size: f64,
        field: [f64; 4],
    }

    /// A photo of a printed target and what happened to it.
    struct Shot {
        layout: Layout,
        width: u32,
        height: u32,
        to_px: Matrix,
        /// Print the marks and nothing else (no aims, grid or text).
        marks: bool,
        aims: bool,
        /// Holes through the paper: centre and radius, sheet mm.
        holes: Vec<([f64; 2], f64)>,
        /// Covered areas (a thumb, a stone), `[x0, y0, x1, y1]` sheet mm.
        covers: Vec<[f64; 4]>,
        /// Another sheet's mark over this one: its corner, centre (mm) and
        /// size against this sheet's marks.
        strays: Vec<(Corner, [f64; 2], f64)>,
        /// Modules inked over: mark, row, column.
        blots: Vec<(usize, usize, usize)>,
        /// How far the sheet's right edge curls up towards the camera, mm.
        lift_mm: f64,
        light_gradient: f64,
        shadow: f64,
        blur_px: f64,
        noise: f64,
        jpeg: Option<u8>,
        samples: usize,
        seed: u64,
    }

    impl Shot {
        /// The sheet point a photo point shows. A curled sheet's raised
        /// part is nearer the camera, so it shows larger about the photo's
        /// middle.
        fn on_sheet(&self, to_mm: &Matrix, px: [f64; 2]) -> [f64; 2] {
            let mut mm = apply(to_mm, px).unwrap_or([-1.0, -1.0]);
            if self.lift_mm == 0.0 {
                return mm;
            }
            let [w, h] = self.layout.sheet_mm();
            let distance = 0.9 * w.max(h);
            let middle = [f64::from(self.width) / 2.0, f64::from(self.height) / 2.0];
            for _ in 0..4 {
                let z = self.lift_mm * ((mm[0] - w / 2.0) / (w / 2.0)).max(0.0).powi(2);
                let shrink = (distance - z) / distance;
                let flat = [
                    middle[0] + (px[0] - middle[0]) * shrink,
                    middle[1] + (px[1] - middle[1]) * shrink,
                ];
                mm = apply(to_mm, flat).unwrap_or([-1.0, -1.0]);
            }
            mm
        }

        fn new(layout: Layout, width: u32, height: u32, to_px: Matrix) -> Shot {
            Shot {
                layout,
                width,
                height,
                to_px,
                marks: true,
                aims: true,
                holes: Vec::new(),
                covers: Vec::new(),
                strays: Vec::new(),
                blots: Vec::new(),
                lift_mm: 0.0,
                light_gradient: 0.35,
                shadow: 0.3,
                blur_px: 1.2,
                noise: 3.0,
                jpeg: Some(80),
                samples: 2,
                seed: 7,
            }
        }

        /// The sheet's brightness (0–1) at a point, mm.
        fn paper_at(&self, p: [f64; 2], print: &Print) -> f64 {
            let [w, h] = self.layout.sheet_mm();
            if p[0] < 0.0 || p[1] < 0.0 || p[0] > w || p[1] > h {
                return 0.22;
            }
            for c in &self.covers {
                if p[0] >= c[0] && p[0] <= c[2] && p[1] >= c[1] && p[1] <= c[3] {
                    return 0.45;
                }
            }
            for &(centre, radius) in &self.holes {
                if (p[0] - centre[0]).hypot(p[1] - centre[1]) < radius {
                    return 0.1;
                }
            }
            let (ink, paper) = (0.08, 0.94);
            let u = self.layout.module_mm();
            for &(corner, centre, scale) in &self.strays {
                let mx = (p[0] - centre[0]) / (u * scale) + 4.5;
                let my = (p[1] - centre[1]) / (u * scale) + 4.5;
                if (0.0..9.0).contains(&mx) && (0.0..9.0).contains(&my) {
                    let modules = self.layout.modules(corner);
                    return if modules[my as usize][mx as usize] {
                        ink
                    } else {
                        paper
                    };
                }
            }
            if self.marks {
                for (k, centre) in print.centres.iter().enumerate() {
                    let mx = (p[0] - centre[0]) / u + 4.5;
                    let my = (p[1] - centre[1]) / u + 4.5;
                    if (0.0..9.0).contains(&mx) && (0.0..9.0).contains(&my) {
                        let (row, column) = (my as usize, mx as usize);
                        return if print.modules[k][row][column]
                            || self.blots.contains(&(k, row, column))
                        {
                            ink
                        } else {
                            paper
                        };
                    }
                }
            }
            if self.aims {
                let size = print.size;
                for a in &print.aims {
                    let (dx, dy) = (p[0] - a[0], p[1] - a[1]);
                    let r = match self.layout.shape {
                        AimShape::Diamond => dx.abs() + dy.abs(),
                        AimShape::Square => dx.abs().max(dy.abs()),
                    };
                    if r <= size / 2.0 && r >= 0.4 * size {
                        return ink;
                    }
                    if r < size / 2.0 && (dx.abs() < 0.15 || dy.abs() < 0.15) {
                        return ink;
                    }
                }
                if let Some(step) = self.layout.grid.spacing_mm() {
                    let [x0, y0, x1, y1] = print.field;
                    if p[0] > x0 && p[0] < x1 && p[1] > y0 && p[1] < y1 {
                        let near = |v: f64, middle: f64| {
                            let q = (v - middle) / step;
                            (q - q.round()).abs() * step < 0.1
                        };
                        if near(p[0], w / 2.0) || near(p[1], h / 2.0) {
                            return 0.75;
                        }
                    }
                }
                // Lines of text between the marks, at the top and bottom.
                let inset = self.layout.paper.inset_mm();
                for y in [inset - 4.0, inset + 4.0, h - inset - 6.0, h - inset + 2.0] {
                    if (p[1] - y).abs() < 1.2
                        && (p[0] - w / 2.0).abs() < 40.0
                        && (p[0] * 1.7).sin() > 0.2
                    {
                        return ink;
                    }
                }
            }
            paper
        }

        fn take(&self) -> InkImage {
            let (w, h) = (self.width as usize, self.height as usize);
            let to_mm = invert(&self.to_px).unwrap();
            let print = Print {
                modules: Corner::ALL.map(|c| self.layout.modules(c)),
                centres: self.layout.mark_centres_mm(),
                aims: self.layout.aim_points_mm(),
                size: self.layout.aim_size_mm(),
                field: self.layout.field_mm(),
            };
            let mut random = Random(self.seed);
            let mut plane = vec![0f32; w * h];
            let n = self.samples;
            let shade_angle = 0.6 + random.next();
            for y in 0..h {
                for x in 0..w {
                    let mut sum = 0.0;
                    for sy in 0..n {
                        for sx in 0..n {
                            let px = x as f64 + (sx as f64 + 0.5) / n as f64;
                            let py = y as f64 + (sy as f64 + 0.5) / n as f64;
                            let mm = self.on_sheet(&to_mm, [px, py]);
                            sum += self.paper_at(mm, &print);
                        }
                    }
                    let value = sum / (n * n) as f64;
                    let across = x as f64 / w as f64 - 0.5;
                    let side = (x as f64 - 0.6 * w as f64) * shade_angle.cos()
                        + (y as f64 - 0.5 * h as f64) * shade_angle.sin();
                    let shade = 1.0 - self.shadow / (1.0 + (-side / 40.0).exp());
                    let light = (1.0 - self.light_gradient * across) * shade;
                    plane[y * w + x] = (245.0 * value * light) as f32;
                }
            }
            if self.blur_px > 0.0 {
                gaussian(&mut plane, w, h, self.blur_px);
            }
            let grey: Vec<u8> = plane
                .iter()
                .map(|&v| {
                    (f64::from(v) + self.noise * random.gaussian())
                        .round()
                        .clamp(0.0, 255.0) as u8
                })
                .collect();
            let mut picture = image::GrayImage::from_raw(w as u32, h as u32, grey).unwrap();
            if let Some(quality) = self.jpeg {
                let mut jpeg = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
                    .encode_image(&picture)
                    .unwrap();
                picture = image::load_from_memory(&jpeg).unwrap().into_luma8();
            }
            InkImage {
                width: w as u32,
                height: h as u32,
                pixels: picture.into_raw(),
            }
        }
    }

    fn gaussian(plane: &mut [f32], w: usize, h: usize, sigma: f64) {
        let radius = (3.0 * sigma).ceil() as i64;
        let kernel: Vec<f32> = (-radius..=radius)
            .map(|k| (-(k * k) as f64 / (2.0 * sigma * sigma)).exp() as f32)
            .collect();
        let total: f32 = kernel.iter().sum();
        let kernel: Vec<f32> = kernel.iter().map(|k| k / total).collect();
        let blur = |plane: &mut [f32], start: usize, step: usize, count: usize| {
            let line: Vec<f32> = (0..count).map(|i| plane[start + i * step]).collect();
            for i in 0..count {
                let mut sum = 0.0;
                for (k, weight) in kernel.iter().enumerate() {
                    let j = (i as i64 + k as i64 - radius).clamp(0, count as i64 - 1) as usize;
                    sum += weight * line[j];
                }
                plane[start + i * step] = sum;
            }
        };
        for y in 0..h {
            blur(plane, y * w, 1, w);
        }
        for x in 0..w {
            blur(plane, x, w, h);
        }
    }

    fn whole(image: &InkImage) -> [f64; 4] {
        [0.0, 0.0, f64::from(image.width), f64::from(image.height)]
    }

    /// The worst miss of a mark centre (px) and of an aim point (sheet mm).
    fn misses(shot: &Shot, marks: &Marks) -> (f64, f64) {
        let mut corner: f64 = 0.0;
        for (k, centre) in shot.layout.mark_centres_mm().iter().enumerate() {
            if marks.seen[k] == Seen::Estimated {
                continue;
            }
            let truth = apply(&shot.to_px, *centre).unwrap();
            let got = marks.centres_px[k];
            corner = corner.max((truth[0] - got[0]).hypot(truth[1] - got[1]));
        }
        let to_mm = invert(&shot.to_px).unwrap();
        let mut aim: f64 = 0.0;
        let aims = marks.aim_points_px();
        assert_eq!(aims.len(), shot.layout.aims.count());
        for (got, truth) in aims.iter().zip(shot.layout.aim_points_mm()) {
            let mm = apply(&to_mm, *got).unwrap();
            aim = aim.max((mm[0] - truth[0]).hypot(mm[1] - truth[1]));
        }
        (corner, aim)
    }

    fn all_layouts() -> Vec<Layout> {
        let mut layouts = Vec::new();
        for paper in Paper::ALL {
            for landscape in [false, true] {
                for aims in AimPoints::ALL {
                    for grid in GridSpacing::ALL {
                        for shape in [AimShape::Diamond, AimShape::Square] {
                            for size in AimSize::ALL {
                                layouts.push(Layout {
                                    paper,
                                    landscape,
                                    aims,
                                    grid,
                                    shape,
                                    size,
                                });
                            }
                        }
                    }
                }
            }
        }
        layouts
    }

    #[test]
    fn codes_round_trip_and_errors_are_caught() {
        let layouts = all_layouts();
        assert_eq!(layouts.len(), 576);
        let mut codes = std::collections::HashSet::new();
        for layout in &layouts {
            let code = layout.code();
            assert!(code < 1 << 10);
            assert!(codes.insert(code));
            assert_eq!(Layout::from_code(code), Some(*layout));
            for corner in Corner::ALL {
                let word = layout.word(corner);
                assert!(word < 1 << 21);
                assert_eq!(decode_word(word), Some((corner, code)));
                // Every one, two or three modules read wrong is caught.
                for a in 0..21 {
                    assert_eq!(decode_word(word ^ (1 << a)), None);
                    for b in a + 1..21 {
                        assert_eq!(decode_word(word ^ (1 << a) ^ (1 << b)), None);
                        for c in [(b + 1) % 21, (b + 7) % 21] {
                            if c != a && c != b {
                                assert_eq!(
                                    decode_word(word ^ (1 << a) ^ (1 << b) ^ (1 << c)),
                                    None
                                );
                            }
                        }
                    }
                }
            }
        }
        // Another version's code isn't read as this one's.
        assert_eq!(Layout::from_code(1 << 10), None);
        assert_eq!(Layout::from_code(3 << 8), None);
    }

    #[test]
    fn marks_show_their_top_left() {
        let layout = Layout::default();
        for corner in Corner::ALL {
            let m = layout.modules(corner);
            for i in 0..9 {
                assert!(m[0][i] && m[8][i] && m[i][0] && m[i][8]);
                if (1..8).contains(&i) {
                    assert!(!m[1][i] && !m[7][i] && !m[i][1] && !m[i][7]);
                }
            }
            assert!(m[2][2] && !m[2][6] && !m[6][6] && !m[6][2]);
        }
    }

    #[test]
    fn every_layout_fits_its_sheet() {
        for layout in all_layouts() {
            let [w, h] = layout.sheet_mm();
            let half = layout.mark_mm() / 2.0;
            for c in layout.mark_centres_mm() {
                // Clear of a printer's unprintable margin.
                assert!(c[0] - half >= 12.0 && c[1] - half >= 12.0, "{layout:?}");
                assert!(
                    w - c[0] - half >= 12.0 && h - c[1] - half >= 12.0,
                    "{layout:?}"
                );
            }
            let [x0, y0, x1, y1] = layout.field_mm();
            let size = layout.aim_size_mm();
            assert!(size >= 25.0, "{layout:?}: {size}");
            assert!(size <= layout.size.mm());
            let points = layout.aim_points_mm();
            assert_eq!(points.len(), layout.aims.count());
            for (i, p) in points.iter().enumerate() {
                assert!(
                    p[0] - size / 2.0 >= x0 && p[0] + size / 2.0 <= x1,
                    "{layout:?}"
                );
                assert!(
                    p[1] - size / 2.0 >= y0 && p[1] + size / 2.0 <= y1,
                    "{layout:?}"
                );
                for q in &points[i + 1..] {
                    assert!((p[0] - q[0]).abs().max((p[1] - q[1]).abs()) >= size + 10.0);
                }
                if let Some(step) = layout.grid.spacing_mm() {
                    let off = |v: f64, middle: f64| {
                        let q = (v - middle) / step;
                        (q - q.round()).abs()
                    };
                    assert!(off(p[0], w / 2.0) < 1e-9 && off(p[1], h / 2.0) < 1e-9);
                }
            }
        }
    }

    #[test]
    fn finds_marks_square_on() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1500, 1600], 0.0, 0.0, 0.0, 0.92);
        let shot = Shot::new(layout, 1500, 1600, to_px);
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        assert_eq!(marks.layout, layout);
        assert_eq!(marks.seen, [Seen::Read; 4]);
        assert_eq!(marks.quarter_turns, 0);
        let (corner, aim) = misses(&shot, &marks);
        println!(
            "square on: corners {corner:.3} px, aims {aim:.3} mm, fit {:.3} px",
            marks.fit_px
        );
        assert!(corner < 0.3, "{corner}");
        assert!(aim < 0.5, "{aim}");
    }

    type Case = (Paper, bool, f64, f64, f64, u8);

    /// Photos of the sheet turned `roll` and tilted `tilt` (up to 35°)
    /// about an axis at `axis`, with every layout setting changing.
    fn turned_and_tilted(cases: &[Case], first: usize) {
        let mut worst = (0.0f64, 0.0f64);
        for (n, &(paper, landscape, tilt, axis, roll, turns)) in cases.iter().enumerate() {
            let i = first + n;
            let layout = Layout {
                paper,
                landscape,
                aims: AimPoints::ALL[i % 4],
                grid: GridSpacing::ALL[i % 3],
                shape: if i.is_multiple_of(2) {
                    AimShape::Diamond
                } else {
                    AimShape::Square
                },
                size: AimSize::ALL[(i + 1) % 4],
            };
            let (w, h) = if i.is_multiple_of(2) {
                (1800, 1350)
            } else {
                (1350, 1800)
            };
            let to_px = view(layout.sheet_mm(), [w, h], tilt, axis, roll, 0.9);
            let mut shot = Shot::new(layout, w, h, to_px);
            shot.seed = i as u64;
            shot.blur_px = 0.8 + 0.2 * i as f64;
            let image = shot.take();
            let marks =
                find_marks(&image, whole(&image)).unwrap_or_else(|e| panic!("case {i}: {e}"));
            assert_eq!(marks.layout, layout, "case {i}");
            assert_eq!(marks.seen, [Seen::Read; 4], "case {i}");
            assert_eq!(marks.quarter_turns, turns, "case {i}");
            let (corner, aim) = misses(&shot, &marks);
            println!(
                "case {i}: corners {corner:.3} px, aims {aim:.3} mm, fit {:.3} px",
                marks.fit_px
            );
            worst = (worst.0.max(corner), worst.1.max(aim));
        }
        println!("worst: corners {:.3} px, aims {:.3} mm", worst.0, worst.1);
        assert!(worst.0 < 0.3, "{worst:?}");
        assert!(worst.1 < 0.5, "{worst:?}");
    }

    #[test]
    fn finds_marks_turned_any_way() {
        turned_and_tilted(
            &[
                (Paper::A4, false, 20.0, 30.0, 90.0, 1),
                (Paper::A4, true, 35.0, 0.0, 180.0, 2),
                (Paper::Letter, false, 30.0, 90.0, -90.0, 3),
                (Paper::A4, true, 15.0, 300.0, 250.0, 3),
            ],
            0,
        );
    }

    #[test]
    fn finds_marks_tilted_35_degrees() {
        turned_and_tilted(
            &[
                (Paper::A3, false, 25.0, 60.0, 12.0, 0),
                (Paper::A3, true, 35.0, 120.0, -150.0, 2),
                (Paper::Letter, true, 10.0, 45.0, 44.0, 0),
                (Paper::A4, false, 35.0, 200.0, -30.0, 0),
            ],
            4,
        );
    }

    #[test]
    fn a_covered_mark_is_placed_from_the_other_three() {
        let layout = Layout {
            aims: AimPoints::Six,
            ..Layout::default()
        };
        let to_px = view(layout.sheet_mm(), [1400, 1700], 25.0, 70.0, 5.0, 0.9);
        let mut shot = Shot::new(layout, 1400, 1700, to_px);
        let c = layout.mark_centres_mm()[2];
        shot.covers
            .push([c[0] - 20.0, c[1] - 20.0, c[0] + 30.0, c[1] + 30.0]);
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        assert_eq!(
            marks.seen,
            [Seen::Read, Seen::Read, Seen::Estimated, Seen::Read]
        );
        let truth = apply(&to_px, c).unwrap();
        let placed = marks.centres_px[2];
        let miss = (truth[0] - placed[0]).hypot(truth[1] - placed[1]);
        let (corner, aim) = misses(&shot, &marks);
        println!(
            "covered: estimated corner {miss:.3} px, others {corner:.3} px, aims {aim:.3} mm, \
             flatness {:.3}",
            marks.flatness
        );
        assert!(marks.flatness < CURLED);
        assert!(miss < 1.5, "{miss}");
        assert!(corner < 0.3 && aim < 0.5);
    }

    #[test]
    fn a_holed_mark_is_found_where_the_others_put_it() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1500, 1600], 15.0, 10.0, -8.0, 0.92);
        let mut shot = Shot::new(layout, 1500, 1600, to_px);
        // Two .30 holes through the top-right mark's data modules, and
        // one through the bottom-left's frame.
        let c = layout.mark_centres_mm()[1];
        let u = layout.module_mm();
        shot.holes.push(([c[0] - 0.5 * u, c[1]], 3.8));
        shot.holes.push(([c[0] + 1.5 * u, c[1] + 1.0 * u], 3.8));
        let d = layout.mark_centres_mm()[3];
        shot.holes.push(([d[0] + 4.0 * u, d[1] - 2.0 * u], 3.8));
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        assert_eq!(marks.seen[0], Seen::Read);
        assert_ne!(marks.seen[1], Seen::Estimated, "{:?}", marks.seen);
        assert_ne!(marks.seen[3], Seen::Estimated, "{:?}", marks.seen);
        let (corner, aim) = misses(&shot, &marks);
        println!(
            "holed: {:?}, corners {corner:.3} px, aims {aim:.3} mm",
            marks.seen
        );
        assert!(corner < 0.3 && aim < 0.5, "{corner} {aim}");
    }

    #[test]
    fn two_covered_marks_are_too_few() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1200, 1500], 10.0, 0.0, 0.0, 0.9);
        let mut shot = Shot::new(layout, 1200, 1500, to_px);
        for k in [0, 1] {
            let c = layout.mark_centres_mm()[k];
            shot.covers
                .push([c[0] - 18.0, c[1] - 18.0, c[0] + 18.0, c[1] + 18.0]);
        }
        let image = shot.take();
        assert_eq!(
            find_marks(&image, whole(&image)),
            Err(NoMarks::OnlySome {
                seen: [false, false, true, true]
            })
        );
    }

    #[test]
    fn a_photo_without_marks_has_none() {
        let layout = Layout {
            shape: AimShape::Square,
            ..Layout::default()
        };
        let to_px = view(layout.sheet_mm(), [1200, 1500], 15.0, 30.0, 3.0, 0.9);
        let mut shot = Shot::new(layout, 1200, 1500, to_px);
        shot.marks = false;
        let image = shot.take();
        assert_eq!(find_marks(&image, whole(&image)), Err(NoMarks::NotFound));
        let blank = InkImage {
            width: 800,
            height: 600,
            pixels: vec![200; 800 * 600],
        };
        assert_eq!(find_marks(&blank, whole(&blank)), Err(NoMarks::NotFound));
        assert_eq!(
            find_marks(&blank, [0.0, 0.0, 40.0, 40.0]),
            Err(NoMarks::TooSmall)
        );
    }

    #[test]
    fn small_marks_are_found_on_a_finer_look() {
        // An A3 sheet filling a third of a 4000 px photo.
        let layout = Layout {
            paper: Paper::A3,
            ..Layout::default()
        };
        let to_px = view(layout.sheet_mm(), [1300, 1000], 10.0, 0.0, 3.0, 0.95);
        let shift = [[1.0, 0.0, 1500.0], [0.0, 1.0, 900.0], [0.0, 0.0, 1.0]];
        let to_px = multiply(&shift, &to_px);
        let mut shot = Shot::new(layout, 4000, 3000, to_px);
        shot.samples = 1;
        shot.blur_px = 0.0;
        shot.jpeg = None;
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        let (corner, aim) = misses(&shot, &marks);
        println!("small: corners {corner:.3} px, aims {aim:.3} mm");
        assert!(corner < 0.3 && aim < 0.5);
    }

    /// The calibration's mm keep the sheet's own sizes and its up, at any
    /// turn: 10 mm up the sheet is 10 mm up the calibration.
    fn check_calibration(tilt: f64, axis: f64, roll: f64) -> f64 {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1100, 1100], tilt, axis, roll, 0.92);
        let mut shot = Shot::new(layout, 1100, 1100, to_px);
        shot.samples = 1;
        shot.seed = roll as u64;
        let image = shot.take();
        let marks =
            find_marks(&image, whole(&image)).unwrap_or_else(|e| panic!("roll {roll}: {e}"));
        let fix = marks.calibration().expect("a calibration");
        let at = |sheet: [f64; 2]| {
            let px = apply(&to_px, sheet).unwrap();
            fix.calibration.to_mm(Point::new(px[0], px[1])).unwrap()
        };
        let [cx, cy] = layout.mark_centres_mm()[0];
        let [w, h] = layout.rectangle_mm();
        let top_left = at([cx, cy]);
        assert!(
            top_left.distance(Point::new(0.0, h)) < 0.3,
            "roll {roll}: {top_left:?}"
        );
        let top_right = at([cx + w, cy]);
        assert!(
            (top_right.distance(top_left) - w).abs() < 0.3,
            "roll {roll}"
        );
        let middle = [layout.sheet_mm()[0] / 2.0, layout.sheet_mm()[1] / 2.0];
        let up = at([middle[0], middle[1] - 10.0]) - at(middle);
        assert!(
            up.distance(Point::new(0.0, 10.0)) < 0.1,
            "roll {roll}: {up:?}"
        );
        let right = at([middle[0] + 10.0, middle[1]]) - at(middle);
        assert!(
            right.distance(Point::new(10.0, 0.0)) < 0.1,
            "roll {roll}: {right:?}"
        );
        let to_mm = invert(&to_px).unwrap();
        let mut worst: f64 = 0.0;
        for (px, truth) in fix.aim_points_px.iter().zip(layout.aim_points_mm()) {
            let mm = apply(&to_mm, [px.x, px.y]).unwrap();
            worst = worst.max((mm[0] - truth[0]).hypot(mm[1] - truth[1]));
        }
        assert!(worst < 0.5, "roll {roll}: {worst}");
        worst
    }

    #[test]
    fn calibration_follows_the_sheet_at_any_turn() {
        // A pose where sorting the corners as the photo shows them took the
        // sheet's height for its width.
        check_calibration(29.4, 0.0, 39.2);
        for (i, roll) in (0..360).step_by(15).enumerate() {
            check_calibration(10.0 + (i % 3) as f64 * 10.0, (i * 47) as f64, roll as f64);
        }
    }

    /// A sheet curled 10 mm at one edge: the marks don't lie on one plane.
    #[test]
    fn a_curled_sheet_with_a_mark_covered_is_refused() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1500, 1600], 15.0, 30.0, 4.0, 0.92);
        let mut shot = Shot::new(layout, 1500, 1600, to_px);
        shot.lift_mm = 10.0;
        let image = shot.take();
        match find_marks(&image, whole(&image)) {
            Ok(marks) => {
                println!("curled, all marks: flatness {:.3}", marks.flatness);
                assert!(marks.flatness > CURLED, "{}", marks.flatness);
            }
            Err(error) => assert_eq!(error, NoMarks::Inconsistent),
        }
        let c = layout.mark_centres_mm()[1];
        shot.covers
            .push([c[0] - 18.0, c[1] - 18.0, c[0] + 18.0, c[1] + 18.0]);
        let image = shot.take();
        assert_eq!(
            find_marks(&image, whole(&image)),
            Err(NoMarks::Inconsistent)
        );
    }

    /// A bigger mark of another sheet, over this sheet's bottom-right mark
    /// and a little off it, is left out and the mark placed from the
    /// other three.
    #[test]
    fn a_mark_off_the_sheet_is_left_out() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1500, 1600], 15.0, 20.0, -6.0, 0.92);
        let mut shot = Shot::new(layout, 1500, 1600, to_px);
        let c = layout.mark_centres_mm()[2];
        shot.strays
            .push((Corner::BottomRight, [c[0] - 3.0, c[1] - 2.0], 1.15));
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        assert_eq!(
            marks.seen,
            [Seen::Read, Seen::Read, Seen::Estimated, Seen::Read]
        );
        let (corner, aim) = misses(&shot, &marks);
        let truth = apply(&to_px, c).unwrap();
        let placed = marks.centres_px[2];
        let miss = (truth[0] - placed[0]).hypot(truth[1] - placed[1]);
        println!("stray: estimated {miss:.3} px, others {corner:.3} px, aims {aim:.3} mm");
        assert!(miss < 1.5 && corner < 0.3 && aim < 0.5);
    }

    /// Ink over a mark's white ring: its edges can still be measured, but
    /// it isn't a mark's frame, so it is placed from the others instead.
    #[test]
    fn a_blotted_frame_is_not_taken_for_a_mark() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [1500, 1600], 10.0, 0.0, 3.0, 0.92);
        let mut shot = Shot::new(layout, 1500, 1600, to_px);
        for (row, column) in [(1i32, 1i32), (1, 7), (7, 1), (7, 7)] {
            for (dr, dc) in [(0i32, 0i32), (0, 1), (1, 0), (0, -1), (-1, 0)] {
                let (r, c) = (row + dr, column + dc);
                if (1..=7).contains(&r)
                    && (1..=7).contains(&c)
                    && (r == 1 || r == 7 || c == 1 || c == 7)
                {
                    shot.blots.push((3, r as usize, c as usize));
                }
            }
        }
        let image = shot.take();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        assert_eq!(marks.seen[3], Seen::Estimated, "{:?}", marks.seen);
    }

    /// The time for a 4000 × 3000 px photo; the limit holds for optimised
    /// builds.
    #[test]
    fn is_quick_enough() {
        let layout = Layout::default();
        let to_px = view(layout.sheet_mm(), [4000, 3000], 20.0, 0.0, 93.0, 0.85);
        let mut shot = Shot::new(layout, 4000, 3000, to_px);
        shot.samples = 1;
        shot.blur_px = 1.0;
        shot.jpeg = None;
        let image = shot.take();
        let start = Instant::now();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        let elapsed = start.elapsed();
        let (corner, aim) = misses(&shot, &marks);
        println!("4000 × 3000 px: {elapsed:?}, corners {corner:.3} px, aims {aim:.3} mm");
        assert!(corner < 0.3 && aim < 0.5);
        if !cfg!(debug_assertions) {
            assert!(elapsed.as_millis() < 150, "{elapsed:?}");
        }
        // A covered mark: the search looks again on a finer copy.
        let c = layout.mark_centres_mm()[3];
        shot.covers
            .push([c[0] - 20.0, c[1] - 20.0, c[0] + 20.0, c[1] + 20.0]);
        let image = shot.take();
        let start = Instant::now();
        let marks = find_marks(&image, whole(&image)).expect("marks");
        let elapsed = start.elapsed();
        assert_eq!(marks.seen[3], Seen::Estimated);
        println!("4000 × 3000 px, a mark covered: {elapsed:?}");
        if !cfg!(debug_assertions) {
            assert!(elapsed.as_millis() < 150, "{elapsed:?}");
        }
    }
}

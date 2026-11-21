//! MX targets while marking: the sheet found by its corner marks scales and
//! straightens the photo, offers its aim points, and lets a later photo of
//! the same sheet show the holes shot since.

use groups_vision::grid::InkImage;
use groups_vision::holes::{self as vision, HoleError, HoleQuery, Pixels, Region};
use groups_vision::marks::{self, Layout, Marks, NoMarks, Seen};

use crate::i18n::{tr, trf};

/// A [`Marks::flatness`] above this share of [`marks::CURLED`] is warned
/// about.
const FLATNESS_WARNING: f64 = 0.5;
/// How close a later photo's hole must be to a marked shot to be that shot,
/// in calibres.
pub const SAME_HOLE_CALIBRES: f64 = 0.6;
/// A hole this many calibres across where a shot was marked is likely a
/// second shot through it: about twice the area.
const GROWN_CALIBRES: f64 = 1.3;
/// A tapped aim within this of a printed aim point takes the aim point, mm.
pub const AIM_SNAP_MM: f64 = 8.0;
/// The print scale asked about: what the sheet's ruler should measure, mm.
pub const RULER_MM: f64 = 100.0;

/// An MX target found on the photo being marked.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    /// The marks, in the ink image's pixels.
    pub marks: Marks,
    /// Ink image pixels to a photo pixel, across and down.
    pub ink_per_px: [f64; 2],
}

impl Sheet {
    pub fn layout(&self) -> Layout {
        self.marks.layout
    }

    /// The mark centres in photo pixels, as printed: top left, top right,
    /// bottom right, bottom left.
    pub fn corners_px(&self) -> [[f64; 2]; 4] {
        let [sx, sy] = self.ink_per_px;
        self.marks.centres_px.map(|[x, y]| [x / sx, y / sy])
    }

    /// A point of the sheet (mm from its top-left corner, y down) on the
    /// photo, px.
    pub fn sheet_to_px(&self, mm: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y] = self.marks.sheet_to_px(mm)?;
        Some([x / self.ink_per_px[0], y / self.ink_per_px[1]])
    }

    /// The point of the sheet a photo pixel shows.
    pub fn px_to_sheet(&self, [x, y]: [f64; 2]) -> Option<[f64; 2]> {
        self.marks
            .px_to_sheet([x * self.ink_per_px[0], y * self.ink_per_px[1]])
    }

    /// The aim points on the photo, px, numbered as printed.
    pub fn aim_points_px(&self) -> Vec<[f64; 2]> {
        self.marks
            .aim_points_mm()
            .into_iter()
            .filter_map(|mm| self.sheet_to_px(mm))
            .collect()
    }

    /// What is worth warning about: a mark placed from the others, a sheet
    /// that doesn't lie flat.
    pub fn warnings(&self) -> Vec<String> {
        warnings(&self.marks.seen, self.marks.flatness)
    }
}

/// Looks for an MX target on the whole ink image of a photo `photo_px` in
/// size.
pub fn find(ink: &InkImage, photo_px: [u32; 2]) -> Result<Sheet, NoMarks> {
    let region = [0.0, 0.0, f64::from(ink.width), f64::from(ink.height)];
    let marks = marks::find_marks(ink, region)?;
    Ok(Sheet {
        marks,
        ink_per_px: [
            f64::from(ink.width) / f64::from(photo_px[0].max(1)),
            f64::from(ink.height) / f64::from(photo_px[1].max(1)),
        ],
    })
}

/// The warnings for marks seen so, on a sheet this flat.
pub fn warnings(seen: &[Seen; 4], flatness: f64) -> Vec<String> {
    let mut warnings = Vec::new();
    match seen.iter().filter(|s| **s == Seen::Estimated).count() {
        0 => {}
        1 => warnings.push(tr("One corner mark was estimated: less exact").to_owned()),
        n => warnings.push(trf!("{} corner marks were estimated: less exact", n)),
    }
    if flatness > FLATNESS_WARNING * marks::CURLED {
        warnings.push(tr("The sheet doesn't lie flat: press it flat for exact numbers").to_owned());
    }
    warnings
}

/// Why no MX target was found, for the shooter; `None` when there is
/// nothing like one to speak of.
pub fn failure(reason: NoMarks) -> Option<&'static str> {
    match reason {
        NoMarks::TooSmall | NoMarks::NotFound => None,
        NoMarks::Unreadable => Some(tr(
            "Corner marks of an MX target were found, but their codes couldn't be read.",
        )),
        NoMarks::OnlySome { .. } => Some(tr(
            "Only some of the MX target's corner marks show: three are needed.",
        )),
        NoMarks::Inconsistent => Some(tr(
            "The MX target's corner marks don't lie on one flat sheet: press it flat.",
        )),
    }
}

/// The first aim point (index) not within [`AIM_SNAP_MM`] of an aim already
/// used, `taken` in sheet mm.
pub fn next_aim(aims_mm: &[[f64; 2]], taken: &[[f64; 2]]) -> Option<usize> {
    aims_mm.iter().position(|aim| {
        taken
            .iter()
            .all(|used| (aim[0] - used[0]).hypot(aim[1] - used[1]) > AIM_SNAP_MM)
    })
}

/// What a hole in a later photo is to the shots marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Nothing marked at it.
    New,
    /// One of the circles a blob was split into, the other at a marked
    /// shot: a separate shot touching that shot's hole.
    Split { shot: usize },
    /// A marked shot's hole grown to about twice its area: maybe a second
    /// shot through it, to check.
    Grown { shot: usize },
}

/// A hole in a later photo with nothing marked at it yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewHole {
    /// On the sheet, mm from its top-left corner, y down.
    pub sheet_mm: [f64; 2],
    pub diameter_mm: f64,
    pub score: f64,
    pub kind: Kind,
    /// Mostly on the sheet's printed ink: less sure.
    pub on_print: bool,
}

/// What a later photo of the sheet showed.
#[derive(Clone, Debug, PartialEq)]
pub struct Later {
    pub new: Vec<NewHole>,
    /// Marked shots with a hole at their place in the later photo.
    pub lined_up: usize,
    pub marked: usize,
    /// The later photo's marks, in its own pixels.
    pub marks: Marks,
}

impl Later {
    /// Whether the later photo may well be of another sheet: most marked
    /// shots have no hole there.
    pub fn doubtful(&self) -> bool {
        doubtful(self.lined_up, self.marked)
    }
}

/// Whether so few of the marked shots line up that the sheet may be
/// another: under half of them.
pub fn doubtful(lined_up: usize, marked: usize) -> bool {
    marked > 0 && 2 * lined_up < marked
}

/// The calibre on the sheet as printed, for a sheet printed `print_scale`
/// times its size.
pub fn sheet_calibre(calibre_mm: f64, print_scale: f64) -> f64 {
    calibre_mm / print_scale.max(0.5)
}

/// Why a later photo can't be used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaterError {
    /// The photo being marked isn't on a recognised MX target.
    NotMx,
    NoCalibre,
    NoMarks(NoMarks),
    /// Another layout of MX target.
    OtherLayout,
    /// A mark was only estimated: all four have to be seen.
    MarkHidden,
    /// Holes are too small in the later photo's pixels.
    Coarse,
}

impl LaterError {
    pub fn message(self) -> String {
        match self {
            LaterError::NotMx => {
                tr("A later photo can be compared only on an MX target whose marks were found.")
                    .to_owned()
            }
            LaterError::NoCalibre => tr("Set the scale and the bullet first").to_owned(),
            LaterError::NoMarks(reason) => match failure(reason) {
                Some(why) => trf!("No MX target in the later photo. {}", why),
                None => tr("No MX target in the later photo.").to_owned(),
            },
            LaterError::OtherLayout => {
                tr("The later photo shows another kind of MX target.").to_owned()
            }
            LaterError::MarkHidden => {
                tr("A corner mark is hidden in the later photo: all four have to show.").to_owned()
            }
            LaterError::Coarse => {
                tr("The later photo is too coarse to find holes: take a closer one.").to_owned()
            }
        }
    }
}

/// The tiles a photo is searched in are at most this many pixels across,
/// with an overlap.
const TILE_PX: f64 = 1400.0;
/// A hole whose disc lies this much on printed ink starts unticked.
const ON_PRINT: f64 = 0.5;
/// The printed-ink mask's pixels a sheet millimetre.
const MASK_PX_PER_MM: f64 = 4.0;

/// Compares a later photo of the sheet with the shots marked: the holes
/// it shows at no marked shot and not already in the first photo, and how
/// many marked shots it shows a hole at. `shots_mm` are on the sheet;
/// `calibre_mm` is the bullet's on the sheet as printed. A new shot closer
/// than about half a calibre to a marked one can't be told from it.
pub fn compare(
    first: &Sheet,
    first_ink: Option<&InkImage>,
    later_ink: &InkImage,
    shots_mm: &[[f64; 2]],
    calibre_mm: f64,
) -> Result<Later, LaterError> {
    let region = [
        0.0,
        0.0,
        f64::from(later_ink.width),
        f64::from(later_ink.height),
    ];
    let marks = marks::find_marks(later_ink, region).map_err(LaterError::NoMarks)?;
    if marks.layout.code() != first.marks.layout.code() {
        return Err(LaterError::OtherLayout);
    }
    if marks.seen.contains(&Seen::Estimated) {
        return Err(LaterError::MarkHidden);
    }
    let print = Print::of(&marks.layout);
    let holes = holes_on_sheet(&marks, later_ink, calibre_mm, &print)?;
    let before = match first_ink {
        Some(ink) => holes_on_sheet(&first.marks, ink, calibre_mm, &print).unwrap_or_default(),
        None => Vec::new(),
    };
    let tolerance = SAME_HOLE_CALIBRES * calibre_mm;
    let distance = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    // A hole holds a shot within its radius and the tolerance: a ragged
    // hole may hold several.
    let holds = |hole: &SheetHole, shot: [f64; 2]| {
        distance(shot, hole.sheet_mm) <= hole.diameter_mm / 2.0 + tolerance
    };
    let lined_up = shots_mm
        .iter()
        .filter(|shot| holes.iter().any(|hole| holds(hole, **shot)))
        .count();
    // Each hole to the nearest marked shot within the tolerance.
    let owners: Vec<Option<usize>> = holes
        .iter()
        .map(|hole| {
            shots_mm
                .iter()
                .enumerate()
                .filter(|(_, shot)| distance(**shot, hole.sheet_mm) <= tolerance)
                .min_by(|a, b| {
                    distance(*a.1, hole.sheet_mm).total_cmp(&distance(*b.1, hole.sheet_mm))
                })
                .map(|(index, _)| index)
        })
        .collect();
    let mut new = Vec::new();
    for (hole, owner) in holes.iter().zip(owners.iter()) {
        let kind = match owner {
            None => {
                let seen_before = before
                    .iter()
                    .any(|old| distance(old.sheet_mm, hole.sheet_mm) <= tolerance);
                if seen_before {
                    continue;
                }
                let split_from = hole.blob.and_then(|blob| {
                    holes
                        .iter()
                        .zip(owners.iter())
                        .find(|(other, o)| other.blob == Some(blob) && o.is_some())
                        .and_then(|(_, o)| *o)
                });
                match split_from {
                    Some(shot) => Kind::Split { shot },
                    None => Kind::New,
                }
            }
            Some(shot) => {
                let shots_in = shots_mm.iter().filter(|s| holds(hole, **s)).count();
                if hole.diameter_mm < GROWN_CALIBRES * calibre_mm || shots_in >= 2 {
                    continue;
                }
                let grew = before.iter().all(|old| {
                    distance(old.sheet_mm, hole.sheet_mm) > tolerance
                        || old.diameter_mm < GROWN_CALIBRES * calibre_mm
                });
                if !grew {
                    continue;
                }
                Kind::Grown { shot: *shot }
            }
        };
        new.push(NewHole {
            sheet_mm: hole.sheet_mm,
            diameter_mm: if matches!(kind, Kind::Grown { .. }) {
                calibre_mm
            } else {
                hole.diameter_mm
            },
            score: hole.score,
            kind,
            on_print: print.share(hole.sheet_mm, hole.diameter_mm / 2.0) > ON_PRINT,
        });
    }
    Ok(Later {
        new,
        lined_up,
        marked: shots_mm.len(),
        marks,
    })
}

/// The sheet's dark printed ink as a mask in sheet millimetres: the aim
/// marks, their crosses and the corner marks. The grid's light blue is
/// left: it is too light to take for a hole or to break one up.
struct Print {
    width: usize,
    height: usize,
    ink: Vec<bool>,
}

impl Print {
    fn of(layout: &Layout) -> Print {
        use crate::cards::sheet::Mark;
        use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
        let sheet = crate::cards::mx_target::page(layout);
        let scale = MASK_PX_PER_MM as f32;
        let (width, height) = (
            (sheet.width * scale).ceil() as usize,
            (sheet.height * scale).ceil() as usize,
        );
        let Some(mut pixmap) = Pixmap::new(width as u32, height as u32) else {
            return Print {
                width: 0,
                height: 0,
                ink: Vec::new(),
            };
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        let transform = Transform::from_scale(scale, scale);
        let path = |points: &[[f32; 2]], closed: bool| {
            let mut path = PathBuilder::new();
            path.move_to(points.first()?[0], points.first()?[1]);
            for p in &points[1..] {
                path.line_to(p[0], p[1]);
            }
            if closed {
                path.close();
            }
            path.finish()
        };
        // Printed edges blur in a photo: each mark a little wider.
        let spread = Stroke {
            width: 0.6,
            ..Stroke::default()
        };
        for mark in &sheet.marks {
            match mark {
                Mark::Fill { points, ink, .. } if dark(*ink) => {
                    if let Some(path) = path(points, true) {
                        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                        pixmap.stroke_path(&path, &paint, &spread, transform, None);
                    }
                }
                Mark::Line {
                    points,
                    width,
                    closed,
                    ink,
                    ..
                } if dark(*ink) => {
                    if let Some(path) = path(points, *closed) {
                        let stroke = Stroke {
                            width: width + 0.6,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                    }
                }
                _ => {}
            }
        }
        let ink = pixmap.pixels().iter().map(|p| p.alpha() > 0).collect();
        Print { width, height, ink }
    }
}

/// Whether printed ink is dark enough to look like a hole.
fn dark(ink: crate::cards::sheet::Ink) -> bool {
    let [r, g, b] = ink.rgb();
    u32::from(r) + u32::from(g) + u32::from(b) < 3 * 128
}

impl Print {
    /// Whether the point of the sheet (mm, y down) is printed.
    fn at(&self, [x, y]: [f64; 2]) -> bool {
        let (i, j) = (x * MASK_PX_PER_MM, y * MASK_PX_PER_MM);
        if i < 0.0 || j < 0.0 {
            return false;
        }
        let (i, j) = (i as usize, j as usize);
        i < self.width && j < self.height && self.ink[j * self.width + i]
    }

    /// The share of a disc that is printed.
    fn share(&self, centre: [f64; 2], radius: f64) -> f64 {
        let (mut inside, mut printed) = (0u32, 0u32);
        for i in -6..=6 {
            for j in -6..=6 {
                let (dx, dy) = (f64::from(i) / 6.0, f64::from(j) / 6.0);
                if dx.hypot(dy) <= 1.0 {
                    inside += 1;
                    if self.at([centre[0] + dx * radius, centre[1] + dy * radius]) {
                        printed += 1;
                    }
                }
            }
        }
        f64::from(printed) / f64::from(inside.max(1))
    }
}

/// The photo's ink with the sheet's printed ink filled in from the paper
/// or the hole round it, layer by layer from its edges, so a hole across a
/// printed line is whole and the print shows no holes.
fn without_print(marks: &Marks, ink: &InkImage, print: &Print, area: [usize; 4]) -> Vec<u8> {
    let [x0, y0, x1, y1] = area;
    let (w, h) = (x1 - x0, y1 - y0);
    let mut pixels = Vec::with_capacity(w * h);
    let mut unknown = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let index = (y - y0) * w + (x - x0);
            let printed = marks
                .px_to_sheet([x as f64 + 0.5, y as f64 + 0.5])
                .is_some_and(|mm| print.at(mm));
            if printed {
                unknown.push(index);
            }
            pixels.push(ink.pixels[y * ink.width as usize + x]);
        }
    }
    let mut known: Vec<bool> = vec![true; w * h];
    for &index in &unknown {
        known[index] = false;
    }
    // Across a printed line: the nearest seen pixels on its two sides, in
    // the direction where they are closest together.
    const DIRECTIONS: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    const REACH: i64 = 60;
    let source = pixels.clone();
    let mut levels: Vec<u8> = (0..w * h)
        .step_by(7)
        .filter(|&n| known[n])
        .map(|n| source[n])
        .collect();
    levels.sort_unstable();
    let paper = levels
        .get(levels.len() * 9 / 10)
        .copied()
        .unwrap_or(u8::MAX);
    let seen = |x: i64, y: i64| -> Option<u8> {
        (x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h)
            .then(|| y as usize * w + x as usize)
            .filter(|&n| known[n])
            .map(|n| source[n])
    };
    for &index in &unknown {
        let (x, y) = ((index % w) as i64, (index / w) as i64);
        let across: Vec<(i64, f64)> = DIRECTIONS
            .iter()
            .filter_map(|&(dx, dy)| {
                let side = |sign: i64| {
                    (1..=REACH).find_map(|step| {
                        seen(x + sign * dx * step, y + sign * dy * step)
                            .map(|v| (step, f64::from(v)))
                    })
                };
                let ((a, va), (b, vb)) = (side(1)?, side(-1)?);
                Some((a + b, (va * b as f64 + vb * a as f64) / (a + b) as f64))
            })
            .collect();
        // Across the line, or a little slanted where that is darker: a
        // hole's rim hidden by the line is taken from its slanted sides.
        let shortest = across.iter().map(|(span, _)| *span).min();
        let best = shortest.and_then(|shortest| {
            across
                .iter()
                .filter(|(span, _)| *span <= 2 * shortest)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .copied()
        });
        // Deep inside a large print (a corner mark): paper.
        pixels[index] = best.map_or(paper, |(_, value)| value.round() as u8);
    }
    pixels
}

/// A hole found on a photo of the sheet, on the sheet.
#[derive(Clone, Copy, Debug)]
struct SheetHole {
    sheet_mm: [f64; 2],
    diameter_mm: f64,
    score: f64,
    blob: Option<(usize, u32)>,
}

/// Ink pixels a sheet millimetre spans at a photo pixel.
fn px_per_mm(marks: &Marks, px: [f64; 2]) -> Option<f64> {
    let mm = marks.px_to_sheet(px)?;
    let a = marks.sheet_to_px(mm)?;
    let b = marks.sheet_to_px([mm[0] + 1.0, mm[1]])?;
    let c = marks.sheet_to_px([mm[0], mm[1] + 1.0])?;
    Some(0.5 * ((b[0] - a[0]).hypot(b[1] - a[1]) + (c[0] - a[0]).hypot(c[1] - a[1])))
}

/// The holes on the sheet in a photo, the printed ink filled in first,
/// looked for tile by tile so each stays inside the detector's limits, each
/// with the hole's size there; holes in two tiles kept once.
fn holes_on_sheet(
    marks: &Marks,
    ink: &InkImage,
    calibre_mm: f64,
    print: &Print,
) -> Result<Vec<SheetHole>, LaterError> {
    let [width_mm, height_mm] = marks.layout.sheet_mm();
    let corners: Vec<[f64; 2]> = [
        [0.0, 0.0],
        [width_mm, 0.0],
        [width_mm, height_mm],
        [0.0, height_mm],
    ]
    .iter()
    .filter_map(|corner| marks.sheet_to_px(*corner))
    .collect();
    let (iw, ih) = (f64::from(ink.width), f64::from(ink.height));
    let lo = [0, 1].map(|i| corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min));
    let hi = [0, 1].map(|i| {
        corners
            .iter()
            .map(|p| p[i])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let area = [
        lo[0].max(0.0).floor() as usize,
        lo[1].max(0.0).floor() as usize,
        hi[0].min(iw).ceil() as usize,
        hi[1].min(ih).ceil() as usize,
    ];
    if area[2] <= area[0] || area[3] <= area[1] {
        return Ok(Vec::new());
    }
    let clean = without_print(marks, ink, print, area);
    let clean_width = area[2] - area[0];
    let largest = corners
        .iter()
        .filter_map(|corner| px_per_mm(marks, *corner))
        .fold(0.0f64, f64::max);
    let biggest_hole = calibre_mm * largest;
    let overlap = 3.0 * biggest_hole;
    let tile = TILE_PX.max(8.0 * biggest_hole);
    let mark_half = marks.layout.mark_mm() / 2.0 + calibre_mm / 2.0;
    let mark_centres = marks.layout.mark_centres_mm();
    let mut found: Vec<SheetHole> = Vec::new();
    let mut tile_number = 0;
    let (x0, y0, x1, y1) = (
        area[0] as f64,
        area[1] as f64,
        area[2] as f64,
        area[3] as f64,
    );
    let mut coarse = true;
    let mut ty = y0;
    while ty < y1 {
        let mut tx = x0;
        while tx < x1 {
            let (ax, ay) = (tx as usize, ty as usize);
            let bx = ((tx + tile + overlap).min(x1)) as usize;
            let by = ((ty + tile + overlap).min(y1)) as usize;
            let (w, h) = (bx.saturating_sub(ax), by.saturating_sub(ay));
            let middle = [(ax + bx) as f64 / 2.0, (ay + by) as f64 / 2.0];
            let diameter = px_per_mm(marks, middle).map_or(0.0, |scale| calibre_mm * scale);
            if diameter >= vision::MIN_DIAMETER_PX {
                coarse = false;
            }
            if diameter >= vision::MIN_DIAMETER_PX
                && w.min(h) as f64 >= vision::MIN_REGION_DIAMETERS * diameter
            {
                let mut pixels = Vec::with_capacity(w * h);
                for row in ay..by {
                    let start = (row - area[1]) * clean_width + (ax - area[0]);
                    pixels.extend_from_slice(&clean[start..start + w]);
                }
                let region = Region {
                    width_px: w as u32,
                    height_px: h as u32,
                    pixels: Pixels::Gray(&pixels),
                };
                let holes = match vision::find_holes(&region, &HoleQuery::new(Some(diameter))) {
                    Ok(holes) => holes,
                    Err(HoleError::TooSmall { .. }) => Vec::new(),
                    Err(_) => Vec::new(),
                };
                let scale = diameter / calibre_mm;
                for hole in holes {
                    let px = [hole.x_px + ax as f64, hole.y_px + ay as f64];
                    let Some(mm) = marks.px_to_sheet(px) else {
                        continue;
                    };
                    let on_sheet =
                        (0.0..=width_mm).contains(&mm[0]) && (0.0..=height_mm).contains(&mm[1]);
                    let on_mark = mark_centres.iter().any(|c| {
                        (mm[0] - c[0]).abs() < mark_half && (mm[1] - c[1]).abs() < mark_half
                    });
                    if !on_sheet || on_mark {
                        continue;
                    }
                    let hole = SheetHole {
                        sheet_mm: mm,
                        diameter_mm: hole.diameter_px / scale,
                        score: hole.score,
                        blob: hole.split.map(|split| (tile_number, split.blob)),
                    };
                    match found.iter_mut().find(|seen| {
                        (seen.sheet_mm[0] - mm[0]).hypot(seen.sheet_mm[1] - mm[1])
                            < 0.5 * calibre_mm
                    }) {
                        Some(seen) if seen.score < hole.score => *seen = hole,
                        Some(_) => {}
                        None => found.push(hole),
                    }
                }
            }
            tile_number += 1;
            tx += tile;
        }
        ty += tile;
    }
    if coarse {
        return Err(LaterError::Coarse);
    }
    Ok(found)
}

/// The aim point (sheet mm) nearest `mm` within [`AIM_SNAP_MM`].
pub fn snap_aim(aims_mm: &[[f64; 2]], mm: [f64; 2]) -> Option<usize> {
    aims_mm
        .iter()
        .enumerate()
        .map(|(index, aim)| (index, (aim[0] - mm[0]).hypot(aim[1] - mm[1])))
        .filter(|(_, d)| *d <= AIM_SNAP_MM)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

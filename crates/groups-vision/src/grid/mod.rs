//! Finding the printed grid on a target photo, to set the photo's scale
//! from whole squares.
//!
//! [`find_grid`] looks at a region of the photo and gives the grid's two
//! families of lines, how sure it is, and four crossings that make the
//! largest rectangle of whole squares it can vouch for, each refined to a
//! fraction of a pixel, with the squares counted across and down. A grid
//! with heavier lines every few (1 inch over ¼ inch, 10 mm over 1 mm)
//! gives both: the big squares and the small ones.
//!
//! The method, in short (see `docs/grid-detection.md`):
//!
//! 1. The region is reduced to about 1600 px and turned into darkness
//!    against the paper round it, which takes out shadows and uneven light.
//! 2. A histogram of gradient directions gives the two line directions.
//! 3. For each, the darkness is projected along the lines, thin peaks are
//!    kept (bullet holes and the black centre are wide, not thin), and the
//!    autocorrelation gives the spacing; the phase of the profile's
//!    Fourier term at that spacing places the lines.
//! 4. From the middle outwards, each line is looked for across its
//!    predicted place, and a homography from pixels to grid units is
//!    fitted to the points found (robust least squares), so perspective
//!    is allowed for. The area grows until it covers the region.
//! 5. Lines between the found ones (a finer grid) and lines heavier every
//!    `k` (the big squares) are then looked for.
//! 6. The largest rectangle whose lines hold is chosen, and the count is
//!    checked on the pixels: every crossing along its four edges is refined
//!    by fitting the two lines through it on the image given, and must sit
//!    where the model puts it; otherwise the rectangle shrinks or there is
//!    no grid.

#![allow(clippy::needless_range_loop)]

mod model;
mod raster;
#[cfg(test)]
mod synth;
#[cfg(test)]
mod tests;

use std::fmt;

use model::{Model, Seen};
use raster::Plane;

/// A photo as one channel for the grid search: 0 is ink, 255 paper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InkImage {
    pub width: u32,
    pub height: u32,
    /// One byte a pixel, row by row from the top left.
    pub pixels: Vec<u8>,
}

impl InkImage {
    /// From RGBA pixels. Each pixel is the mean of its darkest channel and
    /// its luma, so coloured lines (green, blue, orange) show as ink as
    /// well as black ones.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> InkImage {
        InkImage {
            width,
            height,
            pixels: rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| ink(p[0], p[1], p[2]))
                .collect(),
        }
    }

    /// From RGB pixels, as [`InkImage::from_rgba`].
    pub fn from_rgb(width: u32, height: u32, rgb: &[u8]) -> InkImage {
        InkImage {
            width,
            height,
            pixels: rgb
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| ink(p[0], p[1], p[2]))
                .collect(),
        }
    }
}

fn ink(r: u8, g: u8, b: u8) -> u8 {
    let (r, g, b) = (u32::from(r), u32::from(g), u32::from(b));
    let darkest = r.min(g).min(b);
    let luma = (77 * r + 150 * g + 29 * b) >> 8;
    (darkest + luma).div_ceil(2) as u8
}

/// What may help the search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hints {
    /// The longest side the search works at, px; the corners are refined
    /// on the photo's own pixels.
    pub work_long_edge_px: u32,
    /// Roughly the spacing of the finest lines, photo px, when known.
    pub spacing_px: Option<f64>,
}

impl Default for Hints {
    fn default() -> Self {
        Hints {
            work_long_edge_px: 1600,
            spacing_px: None,
        }
    }
}

/// One family of grid lines, as seen in the middle of the region (photo px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Family {
    /// The lines' direction, degrees clockwise from the photo's x axis
    /// (y is down), from 0 to 180.
    pub angle_deg: f64,
    /// Between neighbouring lines, in the middle of the region.
    pub spacing_px: f64,
    /// The least and most spacing across the region: perspective makes it
    /// vary.
    pub spacing_range_px: [f64; 2],
    /// From the region's middle to the nearest line, along the lines'
    /// normal (towards +x for the lines across, +y for the lines down).
    pub phase_px: f64,
    /// How many of the family's lines were seen.
    pub lines: u32,
}

/// Four grid crossings making a rectangle of whole squares (photo px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Squares {
    /// Top-left, top-right, bottom-right, bottom-left as the photo shows
    /// them, sorted as `groups_core::Calibration::four_corners` sorts them.
    pub corners: [[f64; 2]; 4],
    /// Squares along the top edge (first to second corner).
    pub across: u32,
    /// Squares along the left edge (first to last corner).
    pub down: u32,
}

/// The heavier lines every few: the big squares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Major {
    /// Small squares to a big square's side.
    pub every: u32,
    pub squares: Squares,
}

/// A grid found on a photo.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// The lines the top edge crosses (running roughly up and down).
    pub across: Family,
    /// The lines the left edge crosses (running roughly left to right).
    pub down: Family,
    /// From 0 to 1: how much of the grid was seen where it was looked for,
    /// and how well the lines fit.
    pub confidence: f64,
    /// The rectangle counted in the finest squares found.
    pub minor: Squares,
    /// The rectangle in big squares, when heavier lines were found.
    pub major: Option<Major>,
    /// Finest-grid units to photo px.
    model: Model,
}

impl Grid {
    /// The rectangle to offer first: in big squares when there are any.
    pub fn squares(&self) -> &Squares {
        self.major
            .as_ref()
            .map_or(&self.minor, |major| &major.squares)
    }

    /// Where the crossing of finest lines `u` (across) and `v` (down) is,
    /// photo px. Lines are numbered from the one nearest the region's
    /// middle; fractions give points between them.
    pub fn crossing(&self, u: f64, v: f64) -> Option<[f64; 2]> {
        self.model.to_px([u, v])
    }
}

/// Why no grid was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoGrid {
    /// The region is too small to look in.
    TooSmall,
    /// Nothing like two families of evenly spaced lines.
    NoLines,
    /// Lines, but too few to make a rectangle of squares.
    TooFewLines,
    /// Something like a grid, but its lines are too blurred to place.
    TooBlurry,
}

impl fmt::Display for NoGrid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            NoGrid::TooSmall => "the area is too small to look for a grid",
            NoGrid::NoLines => "no grid lines were found",
            NoGrid::TooFewLines => "too few grid lines show",
            NoGrid::TooBlurry => "the grid lines are too blurred",
        })
    }
}

impl std::error::Error for NoGrid {}

/// The region's shorter side must be at least this, photo px.
const MIN_SIDE_PX: usize = 48;
/// The finest spacing looked for, work px.
const MIN_PERIOD: f64 = 3.0;
/// The most lines of one family looked at.
const MAX_LINES: i64 = 600;
/// Samples across a stacked line.
const STACK: usize = 17;

/// Looks for a grid in `region` (`[x0, y0, x1, y1]`, photo px) of `image`.
pub fn find_grid(image: &InkImage, region: [f64; 4], hints: &Hints) -> Result<Grid, NoGrid> {
    let (iw, ih) = (image.width as f64, image.height as f64);
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
    if image.pixels.len() < image.width as usize * image.height as usize
        || x1.saturating_sub(x0).min(y1.saturating_sub(y0)) < MIN_SIDE_PX
    {
        return Err(NoGrid::TooSmall);
    }
    let long = (x1 - x0).max(y1 - y0);
    let factor = long
        .div_ceil(hints.work_long_edge_px.max(256) as usize)
        .max(1);
    let bright = raster::reduce(image, [x0, y0, x1, y1], factor);
    if bright.width.min(bright.height) < 32 {
        return Err(NoGrid::TooSmall);
    }
    let dark = raster::darkness(&bright);
    let search = Search {
        min_height: (4.0 * raster::noise(&dark)).max(0.04),
        dark,
    };
    let normals = orientations(&search.dark).ok_or(NoGrid::NoLines)?;
    let hint = hints.spacing_px.map(|spacing| spacing / factor as f64);
    let start = search.initial(normals, hint).ok_or(NoGrid::NoLines)?;
    let mut model = search.track(start, true)?;
    if let Some((k, r)) = search.merge(&model)
        && let Some(coarser) = model.rescaled(1.0 / k as f64, r)
    {
        model = search.track(coarser, true)?;
    }
    // The finer grid when there is one, else (or if it doesn't hold) the
    // lines as found.
    let finer = search
        .finer(&model)
        .and_then(|k| model.rescaled(k as f64, [0.0, 0.0]))
        .and_then(|finer| search.track(finer, true).ok());
    let mut failure = NoGrid::TooFewLines;
    for model in finer.into_iter().chain([model]) {
        match grid_from(image, &search, model, [x0, y0, x1, y1], factor) {
            Ok(grid) => return Ok(grid),
            Err(reason) => failure = reason,
        }
    }
    Err(failure)
}

/// The grid `model` (work px) describes, when its lines hold up over the
/// region `[x0, y0, x1, y1]` (photo px, reduced `factor` times).
fn grid_from(
    image: &InkImage,
    search: &Search,
    model: Model,
    [x0, y0, x1, y1]: [usize; 4],
    factor: usize,
) -> Result<Grid, NoGrid> {
    let survey = search.survey(&model);
    let seen = [0, 1].map(|f| survey.seen_lines(f));
    if seen[0] < 2 || seen[1] < 2 {
        return Err(NoGrid::TooFewLines);
    }
    let heavy = survey.major(&model);
    // Lines found where the model puts them, not near it by chance, and
    // squares that look square give or take perspective.
    let middle = [
        0.5 * search.dark.width as f64,
        0.5 * search.dark.height as f64,
    ];
    let spacing = [0, 1].map(|f| model.spacing(f, middle));
    let (least, most) = (spacing[0].min(spacing[1]), spacing[0].max(spacing[1]));
    if !survey.holds(heavy) || !fit_is_tight(survey.rms_px, least) || !looks_square(least, most) {
        return Err(NoGrid::NoLines);
    }
    let origin = [x0 as f64, y0 as f64];
    let photo_model = model.to_frame(origin, factor as f64);
    let region_area = ((x1 - x0) * (y1 - y0)) as f64;
    // The rectangle the pixels bear out: every crossing along its edges
    // that can be refined sits where the model says, most of them can be,
    // and all four corners are.
    let squares_at = |k: i64, r: [i64; 2]| -> Option<Squares> {
        for [i0, i1, j0, j1] in survey.rectangles(search, &model, k, r).into_iter().take(12) {
            let quad = [(i0, j0), (i1, j0), (i1, j1), (i0, j1)]
                .map(|(i, j)| photo_model.to_px([i as f64, j as f64]));
            let [Some(a), Some(b), Some(c), Some(d)] = quad else {
                continue;
            };
            if quad_area([a, b, c, d]) < 0.15 * region_area {
                break;
            }
            let Some(corners) = edges_bear_out(image, &photo_model, [i0, i1, j0, j1], k) else {
                continue;
            };
            return Some(sorted_squares(
                corners,
                ((i1 - i0) / k) as u32,
                ((j1 - j0) / k) as u32,
            ));
        }
        None
    };
    let major = heavy.and_then(|(every, r)| {
        Some(Major {
            every: every as u32,
            squares: squares_at(every, r)?,
        })
    });
    // The big squares' corners are crossings of the small ones too.
    let minor = match &major {
        Some(major) => Squares {
            across: major.squares.across * major.every,
            down: major.squares.down * major.every,
            ..major.squares
        },
        None => squares_at(1, [0, 0]).ok_or(NoGrid::TooFewLines)?,
    };
    let middle = [0.5 * (x0 + x1) as f64, 0.5 * (y0 + y1) as f64];
    let corners_px = [
        [x0 as f64, y0 as f64],
        [x1 as f64, y0 as f64],
        [x1 as f64, y1 as f64],
        [x0 as f64, y1 as f64],
    ];
    let family = |f: usize| family_at(&photo_model, f, middle, &corners_px, seen[f]);
    let confidence = survey.confidence(search, &model, heavy);
    Ok(Grid {
        across: family(0),
        down: family(1),
        confidence,
        minor,
        major,
        model: photo_model,
    })
}

/// The four corners in the order `four_corners` sorts them, with the
/// counts along the first and last edges.
fn sorted_squares(quad: [[f64; 2]; 4], across: u32, down: u32) -> Squares {
    let first = (0..4)
        .min_by(|&a, &b| (quad[a][0] + quad[a][1]).total_cmp(&(quad[b][0] + quad[b][1])))
        .unwrap_or(0);
    let corners = [0, 1, 2, 3].map(|i| quad[(first + i) % 4]);
    let (across, down) = if first % 2 == 0 {
        (across, down)
    } else {
        (down, across)
    };
    Squares {
        corners,
        across,
        down,
    }
}

fn family_at(
    model: &Model,
    f: usize,
    middle: [f64; 2],
    corners: &[[f64; 2]; 4],
    lines: u32,
) -> Family {
    let [gx, gy] = model.gradient(f, middle);
    let mut angle = (-gx).atan2(gy).to_degrees();
    if angle < 0.0 {
        angle += 180.0;
    }
    if angle >= 180.0 {
        angle -= 180.0;
    }
    let spacing = model.spacing(f, middle);
    let mut range = [spacing, spacing];
    for corner in corners {
        for t in [0.0, 0.5] {
            let p = [
                corner[0] + t * (middle[0] - corner[0]),
                corner[1] + t * (middle[1] - corner[1]),
            ];
            let s = model.spacing(f, p);
            if s.is_finite() {
                range = [range[0].min(s), range[1].max(s)];
            }
        }
    }
    let phase = model
        .to_grid(middle)
        .map_or(0.0, |uv| (uv[f].round() - uv[f]) * spacing);
    Family {
        angle_deg: angle,
        spacing_px: spacing,
        spacing_range_px: range,
        phase_px: phase,
        lines,
    }
}

/// The normals of the two strongest line directions, radians in `[0, π)`:
/// the first nearer the x axis.
fn orientations(dark: &Plane) -> Option<[[f64; 2]; 2]> {
    const BINS: usize = 180;
    let mut histogram = [0.0f64; BINS];
    let (w, h) = (dark.width, dark.height);
    for y in (1..h - 1).step_by(2) {
        for x in (1..w - 1).step_by(2) {
            let d = |dx: usize, dy: usize| f64::from(dark.get(x + dx - 1, y + dy - 1));
            let gx = d(2, 0) + 2.0 * d(2, 1) + d(2, 2) - d(0, 0) - 2.0 * d(0, 1) - d(0, 2);
            let gy = d(0, 2) + 2.0 * d(1, 2) + d(2, 2) - d(0, 0) - 2.0 * d(1, 0) - d(2, 0);
            let magnitude = gx.hypot(gy);
            if magnitude < 0.02 {
                continue;
            }
            let mut angle = gy.atan2(gx);
            if angle < 0.0 {
                angle += std::f64::consts::PI;
            }
            let position = angle / std::f64::consts::PI * BINS as f64;
            let bin = position.floor() as usize % BINS;
            let part = position - position.floor();
            histogram[bin] += magnitude * (1.0 - part);
            histogram[(bin + 1) % BINS] += magnitude * part;
        }
    }
    let smooth: Vec<f64> = (0..BINS)
        .map(|i| {
            (-4i64..=4)
                .map(|k| {
                    let weight = (-(k * k) as f64 / (2.0 * 1.5 * 1.5)).exp();
                    weight * histogram[(i as i64 + k).rem_euclid(BINS as i64) as usize]
                })
                .sum()
        })
        .collect();
    let mut sorted = smooth.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[BINS / 2].max(1e-9);
    let first = (0..BINS).max_by(|&a, &b| smooth[a].total_cmp(&smooth[b]))?;
    let apart = |a: usize, b: usize| {
        let d = a.abs_diff(b);
        d.min(BINS - d)
    };
    let second = (0..BINS)
        .filter(|&i| apart(i, first) >= 55)
        .max_by(|&a, &b| smooth[a].total_cmp(&smooth[b]))?;
    if smooth[first] < 1.8 * median || smooth[second] < 1.5 * median {
        return None;
    }
    let refined = |i: usize| {
        let (a, b, c) = (
            smooth[(i + BINS - 1) % BINS],
            smooth[i],
            smooth[(i + 1) % BINS],
        );
        let denominator = a - 2.0 * b + c;
        let shift = if denominator.abs() > 1e-12 {
            (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        (i as f64 + shift) / BINS as f64 * std::f64::consts::PI
    };
    let normal = |angle: f64| [angle.cos(), angle.sin()];
    let (mut across, mut down) = (normal(refined(first)), normal(refined(second)));
    if across[0].abs() < down[0].abs() {
        std::mem::swap(&mut across, &mut down);
    }
    if across[0] < 0.0 {
        across = [-across[0], -across[1]];
    }
    if down[1] < 0.0 {
        down = [-down[0], -down[1]];
    }
    Some([across, down])
}

/// A look across a line's predicted place.
enum Probe {
    /// A thin dark line this far along the normal (px), this much darker
    /// than round it.
    Line { offset: f64, height: f64 },
    /// Something dark fills the window: a blurred line, or a hole.
    Wide,
    /// Inside something dark, such as the black centre: nothing to see.
    Dark,
    /// No line.
    Flat,
}

/// Looks for a thin dark line across `at`, along `normal`, within
/// `radius`.
fn probe(
    sample: impl Fn(f64, f64) -> Option<f32>,
    at: [f64; 2],
    normal: [f64; 2],
    radius: f64,
    step: f64,
    min_height: f32,
    buffer: &mut Vec<f32>,
) -> Probe {
    let count = ((2.0 * radius / step).round() as usize + 1).max(5);
    let step = 2.0 * radius / (count - 1) as f64;
    buffer.clear();
    for k in 0..count {
        let t = -radius + k as f64 * step;
        match sample(at[0] + t * normal[0], at[1] + t * normal[1]) {
            Some(value) => buffer.push(value),
            None => return Probe::Flat,
        }
    }
    let values = &buffer[..];
    let (top, &peak) = values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .expect("at least five values");
    if top == 0 || top == count - 1 {
        return Probe::Flat;
    }
    let left = values[..top].iter().copied().fold(f32::MAX, f32::min);
    let right = values[top + 1..].iter().copied().fold(f32::MAX, f32::min);
    let base = left.max(right);
    // Within something dark (the black centre, a hole), not on paper.
    if base > 0.55 {
        return Probe::Dark;
    }
    let height = peak - base;
    if height < min_height {
        return Probe::Flat;
    }
    let half = base + 0.5 * height;
    let (mut lo, mut hi) = (top, top);
    while lo > 0 && values[lo - 1] > half {
        lo -= 1;
    }
    while hi + 1 < count && values[hi + 1] > half {
        hi += 1;
    }
    if lo == 0 || hi == count - 1 {
        return Probe::Wide;
    }
    let (mut sum, mut moment) = (0.0f64, 0.0f64);
    for k in lo..=hi {
        let weight = f64::from(values[k] - half);
        sum += weight;
        moment += weight * (-radius + k as f64 * step);
    }
    Probe::Line {
        offset: moment / sum.max(1e-12),
        height: f64::from(height),
    }
}

/// What the search works on.
struct Search {
    dark: Plane,
    /// The least depth a line must have against the paper round it.
    min_height: f32,
}

/// Points found on lines in one pass, and how the looks went.
#[derive(Default)]
struct Pass {
    seen: Vec<Seen>,
    looked: usize,
    wide: usize,
}

/// Every cell's look, over the whole region, for the rectangle and the
/// heavy lines.
struct Survey {
    /// The first line index and the count, per family.
    lines: [(i64, usize); 2],
    /// Per family, per line, per cell of the other family: whether it was
    /// looked at, and whether the line was found there.
    looked: [Vec<bool>; 2],
    found: [Vec<bool>; 2],
    /// Per family, per line: how heavy the line is (its stacked darkness
    /// across, summed), when it was looked at often enough.
    strength: [Vec<Option<f64>>; 2],
    /// Per family, per line, per cell: how far off the line was found, in
    /// spacings (the mean of the looks that found it).
    miss: [Vec<f32>; 2],
    /// Per family, per line: running sums of `looked`, `found` and `miss`
    /// along the line, one more than the cells.
    sums: [(Vec<u32>, Vec<u32>, Vec<f64>); 2],
    rms_px: f64,
}

impl Search {
    fn bounds(&self) -> [f64; 4] {
        [0.0, 0.0, self.dark.width as f64, self.dark.height as f64]
    }

    /// The parallel-line model from the projections of the middle part.
    fn initial(&self, normals: [[f64; 2]; 2], hint: Option<f64>) -> Option<Model> {
        let (w, h) = (self.dark.width as f64, self.dark.height as f64);
        let centre = [0.5 * w, 0.5 * h];
        let middle = [0.2 * w, 0.2 * h, 0.8 * w, 0.8 * h];
        let mut periods = [0.0; 2];
        let mut offsets = [0.0; 2];
        for f in 0..2 {
            let (period, offset) = self
                .periodicity(normals[f], centre, middle, hint)
                .or_else(|| self.periodicity(normals[f], centre, [0.0, 0.0, w, h], hint))?;
            periods[f] = period;
            offsets[f] = offset;
        }
        // The squares are square: a family far finer than the other has
        // been thrown by something else (aliasing of faint fine lines,
        // JPEG blocks), so it is looked at again near the other's spacing.
        let (fine, coarse) = if periods[0] < periods[1] {
            (0, 1)
        } else {
            (1, 0)
        };
        if periods[coarse] > 1.5 * periods[fine] {
            let (period, offset) = self
                .periodicity(normals[fine], centre, middle, Some(periods[coarse]))
                .or_else(|| {
                    self.periodicity(
                        normals[fine],
                        centre,
                        [0.0, 0.0, w, h],
                        Some(periods[coarse]),
                    )
                })?;
            periods[fine] = period;
            offsets[fine] = offset;
        }
        Model::parallel(normals, periods, offsets, centre, 0.5 * w.max(h))
    }

    /// The spacing of the lines with `normal` in `area`, and where the one
    /// nearest `centre` is, px along the normal.
    fn periodicity(
        &self,
        normal: [f64; 2],
        centre: [f64; 2],
        area: [f64; 4],
        hint: Option<f64>,
    ) -> Option<(f64, f64)> {
        let reach = 0.5 * (area[2] - area[0]).hypot(area[3] - area[1]);
        let bins = (2.0 * reach).ceil() as usize + 2;
        let mut sum = vec![0.0f64; bins];
        let mut count = vec![0.0f64; bins];
        for y in area[1] as usize..area[3] as usize {
            for x in area[0] as usize..area[2] as usize {
                let t = normal[0] * (x as f64 + 0.5 - centre[0])
                    + normal[1] * (y as f64 + 0.5 - centre[1])
                    + reach;
                let bin = t.floor();
                if bin < 0.0 || bin as usize + 1 >= bins {
                    continue;
                }
                let part = t - bin;
                let value = f64::from(self.dark.get(x, y));
                let bin = bin as usize;
                sum[bin] += value * (1.0 - part);
                count[bin] += 1.0 - part;
                sum[bin + 1] += value * part;
                count[bin + 1] += part;
            }
        }
        let most = count.iter().copied().fold(0.0, f64::max);
        let valid: Vec<bool> = count.iter().map(|&c| c >= 0.25 * most).collect();
        let profile: Vec<f64> = sum
            .iter()
            .zip(&count)
            .map(|(&s, &c)| if c > 0.0 { s / c } else { 0.0 })
            .collect();
        // Thin peaks only, at several widths: wide dark things fall away.
        let mut ridge = vec![0.0f64; bins];
        for i in 0..bins {
            if !valid[i] {
                continue;
            }
            for a in [2usize, 3, 5, 8, 13, 21] {
                if i < a || i + a >= bins || !valid[i - a] || !valid[i + a] {
                    continue;
                }
                let r = profile[i] - profile[i - a].max(profile[i + a]);
                ridge[i] = ridge[i].max(r);
            }
        }
        let first = valid.iter().position(|&v| v)?;
        let last = valid.iter().rposition(|&v| v)?;
        let ridge = &ridge[first..=last];
        let n = ridge.len();
        let mean = ridge.iter().sum::<f64>() / n as f64;
        let centred: Vec<f64> = ridge.iter().map(|r| r - mean).collect();
        let energy: f64 = centred.iter().map(|r| r * r).sum();
        if energy <= 1e-12 {
            return None;
        }
        let (lo, hi) = match hint {
            Some(spacing) => ((0.6 * spacing).max(MIN_PERIOD), 1.6 * spacing),
            None => (MIN_PERIOD, n as f64 / 2.2),
        };
        let hi = hi.min(n as f64 / 2.2);
        if hi <= lo + 1.0 {
            return None;
        }
        let max_lag = hi.ceil() as usize + 1;
        let correlation: Vec<f64> = (0..=max_lag)
            .map(|lag| {
                centred[..n - lag]
                    .iter()
                    .zip(&centred[lag..])
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    / energy
            })
            .collect();
        let peaks: Vec<usize> = ((lo.floor() as usize).max(2)..max_lag.min(correlation.len() - 1))
            .filter(|&lag| {
                correlation[lag] > 0.1
                    && correlation[lag] >= correlation[lag - 1]
                    && correlation[lag] > correlation[lag + 1]
                    && lag as f64 >= lo
                    && lag as f64 <= hi
            })
            .collect();
        let best = peaks
            .iter()
            .map(|&lag| correlation[lag])
            .fold(0.0, f64::max);
        if best < 0.15 {
            return None;
        }
        let lag = *peaks.iter().find(|&&lag| correlation[lag] >= 0.5 * best)?;
        let refine = |lag: usize| {
            let (a, b, c) = (correlation[lag - 1], correlation[lag], correlation[lag + 1]);
            let denominator = a - 2.0 * b + c;
            lag as f64
                + if denominator.abs() > 1e-12 {
                    (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
                } else {
                    0.0
                }
        };
        // The furthest clear multiple gives the spacing most precisely.
        let mut period = refine(lag);
        for multiple in 2..=8 {
            let target = period * multiple as f64;
            let near = target.round() as usize;
            if near + 1 >= correlation.len() {
                break;
            }
            let at = (near.saturating_sub(1)..=near + 1)
                .filter(|&l| l >= 1 && l + 1 < correlation.len())
                .max_by(|&a, &b| correlation[a].total_cmp(&correlation[b]));
            match at {
                Some(at) if correlation[at] >= 0.4 * best => {
                    let refined = refine(at) / multiple as f64;
                    if (refined - period).abs() < 0.1 * period {
                        period = refined;
                    }
                }
                _ => break,
            }
        }
        // The phase of the profile's term at that spacing, near the middle.
        let window = (4.0 * period).max(24.0);
        let (mut re, mut im) = (0.0, 0.0);
        for (i, r) in ridge.iter().enumerate() {
            let t = (first + i) as f64 - reach;
            if t.abs() > window {
                continue;
            }
            let angle = std::f64::consts::TAU * t / period;
            re += r * angle.cos();
            im += r * angle.sin();
        }
        let offset = im.atan2(re) / std::f64::consts::TAU * period;
        Some((period, offset))
    }

    /// The lines' index ranges within `bounds` (work px), per family.
    fn ranges(&self, model: &Model, bounds: [f64; 4]) -> Option<[(i64, i64); 2]> {
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for i in 0..=8 {
            for j in 0..=8 {
                let p = [
                    bounds[0] + (bounds[2] - bounds[0]) * i as f64 / 8.0,
                    bounds[1] + (bounds[3] - bounds[1]) * j as f64 / 8.0,
                ];
                let uv = model.to_grid(p)?;
                for f in 0..2 {
                    lo[f] = lo[f].min(uv[f]);
                    hi[f] = hi[f].max(uv[f]);
                }
            }
        }
        let range = |f: usize| {
            let a = lo[f].ceil().max(-(MAX_LINES as f64)) as i64;
            let b = hi[f].floor().min(MAX_LINES as f64) as i64;
            (a, b)
        };
        Some([range(0), range(1)])
    }

    /// Looks for every line within `bounds`, `per_line` times at most each,
    /// `radius` (in spacings) either side of where the model puts it.
    fn pass(&self, model: &Model, bounds: [f64; 4], radius: f64, per_line: usize) -> Pass {
        let mut pass = Pass::default();
        let Some(ranges) = self.ranges(model, bounds) else {
            return pass;
        };
        let mut buffer = Vec::new();
        for f in 0..2 {
            let other = ranges[1 - f];
            let cells = (other.1 - other.0 + 2).max(1) as usize;
            let fractions: &[f64] = if cells * 3 <= per_line {
                &[0.5, 0.25, 0.75]
            } else {
                &[0.5]
            };
            let stride = (cells * fractions.len()).div_ceil(per_line).max(1);
            for n in ranges[f].0..=ranges[f].1 {
                let mut k = 0usize;
                for m in other.0 - 1..=other.1 {
                    for &fraction in fractions {
                        k += 1;
                        if !k.is_multiple_of(stride) {
                            continue;
                        }
                        let along = m as f64 + fraction;
                        let uv = if f == 0 {
                            [n as f64, along]
                        } else {
                            [along, n as f64]
                        };
                        let Some(at) = model.to_px(uv) else {
                            continue;
                        };
                        if at[0] < bounds[0]
                            || at[1] < bounds[1]
                            || at[0] > bounds[2]
                            || at[1] > bounds[3]
                        {
                            continue;
                        }
                        let [gx, gy] = model.gradient(f, at);
                        let length = gx.hypot(gy);
                        if length.is_nan() || length <= 0.0 {
                            continue;
                        }
                        let normal = [gx / length, gy / length];
                        let spacing = 1.0 / length;
                        let r = (radius * spacing).max(1.5);
                        pass.looked += 1;
                        let found = probe(
                            |x, y| self.dark.sample(x, y),
                            at,
                            normal,
                            r,
                            (r / 10.0).clamp(0.2, 0.5),
                            self.min_height,
                            &mut buffer,
                        );
                        match found {
                            Probe::Line { offset, .. } => pass.seen.push(Seen {
                                at: [at[0] + offset * normal[0], at[1] + offset * normal[1]],
                                family: f,
                                index: n as f64,
                            }),
                            Probe::Wide => pass.wide += 1,
                            Probe::Flat | Probe::Dark => {}
                        }
                    }
                }
            }
        }
        pass
    }

    /// Fits the model to a pass's points, leaving out those that don't fit
    /// (rings, holes, writing). The fit and the RMS miss of the points kept, px.
    fn fit(&self, model: &Model, seen: &[Seen], regular: f64) -> Option<(Model, f64)> {
        let mut current = model.clone();
        let mut rms = f64::INFINITY;
        for round in 0..4 {
            let misses: Vec<(f64, f64)> = seen
                .iter()
                .map(|point| {
                    let spacing = current.spacing(point.family, point.at);
                    let miss = current.to_grid(point.at).map_or(f64::INFINITY, |uv| {
                        (uv[point.family] - point.index) * spacing
                    });
                    (miss, spacing)
                })
                .collect();
            let mut absolute: Vec<f64> = misses
                .iter()
                .map(|m| m.0.abs())
                .filter(|m| m.is_finite())
                .collect();
            if absolute.len() < 8 {
                return None;
            }
            let middle = absolute.len() / 2;
            let (_, median, _) = absolute.select_nth_unstable_by(middle, f64::total_cmp);
            let sigma = (1.4826 * *median).max(0.15);
            let cut = if round == 0 { 0.35 } else { 0.25 };
            let weights: Vec<f64> = seen
                .iter()
                .zip(&misses)
                .map(|(point, &(miss, spacing))| {
                    if !miss.is_finite() || miss.abs() > cut * spacing || miss.abs() > 6.0 * sigma {
                        return 0.0;
                    }
                    let huber = if miss.abs() <= 2.0 * sigma {
                        1.0
                    } else {
                        2.0 * sigma / miss.abs()
                    };
                    let depth = current.depth(point.at);
                    huber * (spacing / depth).powi(2)
                })
                .collect();
            let kept: Vec<f64> = misses
                .iter()
                .zip(&weights)
                .filter(|(_, w)| **w > 0.0)
                .map(|(m, _)| m.0 * m.0)
                .collect();
            if kept.len() < 8 {
                return None;
            }
            rms = (kept.iter().sum::<f64>() / kept.len() as f64).sqrt();
            current = current.fit(seen, &weights, regular)?;
        }
        Some((current, rms))
    }

    /// Follows the grid from `model`: from the middle outwards when
    /// `grow`, else over the whole region at once.
    fn track(&self, model: Model, grow: bool) -> Result<Model, NoGrid> {
        let (w, h) = (self.dark.width as f64, self.dark.height as f64);
        let centre = [0.5 * w, 0.5 * h];
        let spacing = [0, 1].map(|f| model.spacing(f, centre));
        // A few squares either side first: the first spacing may be a
        // little out, which adds up over many lines.
        let start = 5.0 * spacing[0].max(spacing[1]);
        let mut half = if grow {
            [
                start.max(0.08 * w).min(0.22 * w.max(h)),
                start.max(0.08 * h).min(0.22 * w.max(h)),
            ]
        } else {
            [w, h]
        };
        let mut model = model;
        let mut whole = 0;
        let mut settled = !grow;
        for round in 0..16 {
            let bounds = [
                (centre[0] - half[0]).max(0.0),
                (centre[1] - half[1]).max(0.0),
                (centre[0] + half[0]).min(w),
                (centre[1] + half[1]).min(h),
            ];
            let covered = ((bounds[2] - bounds[0]) / w).min((bounds[3] - bounds[1]) / h);
            let radius = if covered < 1.0 { 0.42 } else { 0.3 };
            // Perspective is fitted once the area is wide enough to show it.
            let regular = if covered < 0.6 { 1.0 } else { 0.02 };
            let pass = self.pass(&model, bounds, radius, 40);
            let lines = |f: usize| {
                let mut indices: Vec<i64> = pass
                    .seen
                    .iter()
                    .filter(|s| s.family == f)
                    .map(|s| s.index as i64)
                    .collect();
                indices.sort_unstable();
                indices.dedup();
                indices.len()
            };
            let covers = bounds == [0.0, 0.0, w, h];
            if (lines(0) < 3 || lines(1) < 3) && !covers {
                // Something covers the middle, such as the black centre.
                half = [half[0] * 1.4, half[1] * 1.4];
                continue;
            }
            if lines(0) < 2 || lines(1) < 2 {
                return Err(
                    if pass.wide > pass.seen.len() && pass.wide * 4 > pass.looked {
                        NoGrid::TooBlurry
                    } else if round == 0 {
                        NoGrid::NoLines
                    } else {
                        NoGrid::TooFewLines
                    },
                );
            }
            if !settled {
                model = self.settle(&model, bounds);
                settled = true;
                continue;
            }
            let before = model.spacing(0, centre);
            let (fitted, _) = self
                .fit(&model, &pass.seen, regular)
                .ok_or(NoGrid::NoLines)?;
            let after = fitted.spacing(0, centre);
            if !(after > 0.5 * before && after < 2.0 * before) {
                return Err(NoGrid::NoLines);
            }
            model = fitted;
            if covers {
                whole += 1;
                if whole >= 2 {
                    break;
                }
            }
            half = [half[0] * 1.4, half[1] * 1.4];
        }
        Ok(model)
    }

    /// The first spacing and phase of each family, corrected within
    /// `bounds`: the scale (±12%) and shift that line the most looks up
    /// with lines. The projection they came from may be thrown by a black
    /// centre or rings in the middle.
    fn settle(&self, model: &Model, bounds: [f64; 4]) -> Model {
        let Some(ranges) = self.ranges(model, bounds) else {
            return model.clone();
        };
        let middle = [0.5 * (bounds[0] + bounds[2]), 0.5 * (bounds[1] + bounds[3])];
        let Some(at_middle) = model.to_grid(middle) else {
            return model.clone();
        };
        let mut best = model.clone();
        let mut buffer = Vec::new();
        for f in 0..2 {
            let other = ranges[1 - f];
            let step = ((other.1 - other.0) as usize / 12).max(1);
            let mut winner = (0usize, f64::MAX, None);
            for s in -12..=12 {
                let scale = 1.0 + 0.01 * s as f64;
                for p in 0..10 {
                    let phase = 0.1 * p as f64;
                    let shift = (1.0 - scale) * at_middle[f]
                        + if phase > 0.5 { phase - 1.0 } else { phase };
                    let Some(trial) = best.adjusted(f, scale, shift) else {
                        continue;
                    };
                    let Some(range) = self.ranges(&trial, bounds) else {
                        continue;
                    };
                    let mut hits = 0;
                    for n in range[f].0..=range[f].1 {
                        for m in (other.0..=other.1).step_by(step) {
                            let along = m as f64 + 0.5;
                            let uv = if f == 0 {
                                [n as f64, along]
                            } else {
                                [along, n as f64]
                            };
                            let Some(at) = trial.to_px(uv) else { continue };
                            if at[0] < bounds[0]
                                || at[1] < bounds[1]
                                || at[0] > bounds[2]
                                || at[1] > bounds[3]
                            {
                                continue;
                            }
                            let [gx, gy] = trial.gradient(f, at);
                            let length = gx.hypot(gy);
                            let spacing = 1.0 / length;
                            let normal = [gx / length, gy / length];
                            let r = (0.25 * spacing).max(1.5);
                            if let Probe::Line { offset, .. } = probe(
                                |x, y| self.dark.sample(x, y),
                                at,
                                normal,
                                r,
                                (r / 8.0).clamp(0.2, 0.5),
                                self.min_height,
                                &mut buffer,
                            ) && offset.abs() < 0.12 * spacing
                            {
                                hits += 1;
                            }
                        }
                    }
                    let change = (scale - 1.0).abs()
                        + (shift - (1.0 - scale) * at_middle[f]).abs().min(0.5) * 0.1;
                    if hits > winner.0 || (hits == winner.0 && change < winner.1) {
                        winner = (hits, change, Some(trial));
                    }
                }
            }
            if let (_, _, Some(trial)) = winner {
                best = trial;
            }
        }
        best
    }

    /// When only every `k`-th line was found, counting from `r`: the lines
    /// between are not there.
    fn merge(&self, model: &Model) -> Option<(i64, [f64; 2])> {
        let survey = self.survey(model);
        for k in [2i64, 3] {
            let mut residues = [0.0; 2];
            let mut all = true;
            for f in 0..2 {
                let (first, count) = survey.lines[f];
                let fraction = |line: usize| survey.found_fraction(f, line);
                let mut best = None;
                for r in 0..k {
                    let (mut on, mut on_n, mut off, mut off_n) = (0.0, 0, 0.0, 0);
                    for line in 0..count {
                        let Some(found) = fraction(line) else {
                            continue;
                        };
                        if (first + line as i64 - r).rem_euclid(k) == 0 {
                            on += found;
                            on_n += 1;
                        } else {
                            off += found;
                            off_n += 1;
                        }
                    }
                    if on_n >= 2
                        && off_n >= 2
                        && on / on_n as f64 >= 0.5
                        && off / (off_n as f64) < 0.12
                    {
                        best = Some(r);
                    }
                }
                match best {
                    Some(r) => residues[f] = r as f64,
                    None => all = false,
                }
            }
            if all {
                return Some((k, residues));
            }
        }
        None
    }

    /// The median darkness across lines of `family` at grid coordinate
    /// `across`, over places `along` the lines: `STACK` samples from
    /// `−reach` to `reach` spacings (of `spacing_units` grid units) across.
    fn stack(
        &self,
        model: &Model,
        family: usize,
        places: impl Iterator<Item = (f64, f64)>,
        reach: f64,
        spacing_units: f64,
    ) -> Option<[f32; STACK]> {
        let mut profiles: Vec<[f32; STACK]> = Vec::new();
        let (w, h) = (self.dark.width as f64, self.dark.height as f64);
        'places: for (across, along) in places {
            let uv = if family == 0 {
                [across, along]
            } else {
                [along, across]
            };
            let Some(at) = model.to_px(uv) else { continue };
            let [gx, gy] = model.gradient(family, at);
            let length = gx.hypot(gy);
            if length.is_nan() || length <= 0.0 {
                continue;
            }
            let spacing = spacing_units / length;
            let normal = [gx / length, gy / length];
            let r = reach * spacing;
            if at[0] < r + 1.0 || at[1] < r + 1.0 || at[0] > w - r - 1.0 || at[1] > h - r - 1.0 {
                continue;
            }
            let mut values = [0.0f32; STACK];
            for (k, value) in values.iter_mut().enumerate() {
                let t = r * (2.0 * k as f64 / (STACK - 1) as f64 - 1.0);
                match self
                    .dark
                    .sample(at[0] + t * normal[0], at[1] + t * normal[1])
                {
                    Some(v) => *value = v,
                    None => continue 'places,
                }
            }
            profiles.push(values);
        }
        if profiles.len() < 3 {
            return None;
        }
        let middle = profiles.len() / 2;
        let mut column = vec![0.0f32; profiles.len()];
        let mut stack = [0.0f32; STACK];
        for (k, value) in stack.iter_mut().enumerate() {
            for (slot, profile) in column.iter_mut().zip(&profiles) {
                *slot = profile[k];
            }
            *value = *column.select_nth_unstable_by(middle, f32::total_cmp).1;
        }
        Some(stack)
    }

    /// When there are lines between the found ones, every `1 / k`: the
    /// finer grid. Each place between is stacked over many looks, so faint
    /// lines show.
    fn finer(&self, model: &Model) -> Option<i64> {
        let (w, h) = (self.dark.width as f64, self.dark.height as f64);
        let ranges = self.ranges(model, self.bounds())?;
        let least = (0..=4)
            .flat_map(|i| (0..=4).map(move |j| [w * i as f64 / 4.0, h * j as f64 / 4.0]))
            .flat_map(|p| [model.spacing(0, p), model.spacing(1, p)])
            .filter(|s| s.is_finite())
            .fold(f64::MAX, f64::min);
        let places = |f: usize, offset: f64| {
            let (first, last) = ranges[f];
            let (other_first, other_last) = ranges[1 - f];
            let lines = (first..=last).step_by(((last - first) as usize / 16).max(1));
            lines.flat_map(move |n| {
                (other_first - 1..=other_last)
                    .step_by(((other_last - other_first) as usize / 16).max(1))
                    .map(move |m| (n as f64 + offset, m as f64 + 0.5))
            })
        };
        // The depth of a stacked line at the middle, and whether it peaks there.
        let depth = |stack: &[f32; STACK]| {
            let middle = STACK / 2;
            let (top, &peak) = stack
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .expect("samples");
            let ends = 0.5 * (stack[0] + stack[STACK - 1]);
            (top.abs_diff(middle) <= 2).then_some(f64::from(peak - ends))
        };
        let both = |offset: f64, spacing_units: f64| -> Option<f64> {
            let mut least_depth = f64::MAX;
            for f in 0..2 {
                let stack = self.stack(model, f, places(f, offset), 0.5, spacing_units)?;
                least_depth = least_depth.min(depth(&stack)?);
            }
            Some(least_depth)
        };
        let lines = both(0.0, 1.0)?;
        let noise = f64::from(self.min_height) / 4.0;
        for k in [10i64, 8, 6, 5, 4, 3, 2] {
            if least / (k as f64) < MIN_PERIOD + 0.5 {
                continue;
            }
            let step = 1.0 / k as f64;
            let all = (1..k).all(|j| {
                both(j as f64 * step, step)
                    .is_some_and(|d| d >= (0.08 * lines).max(noise) && d >= 0.02)
            });
            if all {
                return Some(k);
            }
        }
        None
    }

    /// Looks for every line in every cell of the region.
    fn survey(&self, model: &Model) -> Survey {
        let bounds = self.bounds();
        let ranges = self.ranges(model, bounds).unwrap_or([(0, -1), (0, -1)]);
        let lines = ranges.map(|(a, b)| (a, (b - a + 1).max(0) as usize));
        let mut survey = Survey {
            lines,
            looked: [Vec::new(), Vec::new()],
            found: [Vec::new(), Vec::new()],
            strength: [vec![None; lines[0].1], vec![None; lines[1].1]],
            miss: [Vec::new(), Vec::new()],
            sums: Default::default(),
            rms_px: 0.0,
        };
        let mut buffer = Vec::new();
        let mut seen = Vec::new();
        for f in 0..2 {
            let (first, count) = lines[f];
            let (other_first, cells) = lines[1 - f];
            survey.looked[f] = vec![false; count * cells];
            survey.found[f] = vec![false; count * cells];
            survey.miss[f] = vec![0.0; count * cells];
            for line in 0..count {
                let n = first + line as i64;
                for cell in 0..cells {
                    let along = (other_first + cell as i64) as f64 + 0.5;
                    let uv = if f == 0 {
                        [n as f64, along]
                    } else {
                        [along, n as f64]
                    };
                    let Some(at) = model.to_px(uv) else { continue };
                    let [gx, gy] = model.gradient(f, at);
                    let length = gx.hypot(gy);
                    if length.is_nan() || length <= 0.0 {
                        continue;
                    }
                    let spacing = 1.0 / length;
                    let normal = [gx / length, gy / length];
                    let r = (0.35 * spacing).max(1.5);
                    if at[0] < r || at[1] < r || at[0] > bounds[2] - r || at[1] > bounds[3] - r {
                        continue;
                    }
                    let index = line * cells + cell;
                    let fractions: &[f64] = if cells < 12 {
                        &[0.0, -0.25, 0.25]
                    } else {
                        &[0.0]
                    };
                    let (mut hits, mut seen_into, mut off) = (0, 0, 0.0);
                    for &shift in fractions {
                        let along = along + shift;
                        let uv = if f == 0 {
                            [n as f64, along]
                        } else {
                            [along, n as f64]
                        };
                        let Some(at) = model.to_px(uv) else { continue };
                        match probe(
                            |x, y| self.dark.sample(x, y),
                            at,
                            normal,
                            r,
                            (r / 10.0).clamp(0.2, 0.5),
                            self.min_height,
                            &mut buffer,
                        ) {
                            Probe::Line { offset, .. } if offset.abs() < 0.2 * spacing => {
                                hits += 1;
                                seen_into += 1;
                                off += offset.abs() / spacing;
                                seen.push((offset, spacing));
                            }
                            Probe::Dark => {}
                            _ => seen_into += 1,
                        }
                    }
                    // Looks inside something dark don't count either way.
                    survey.looked[f][index] = seen_into * 2 > fractions.len();
                    survey.found[f][index] = survey.looked[f][index] && 2 * hits > fractions.len();
                    if hits > 0 {
                        survey.miss[f][index] = (off / hits as f64) as f32;
                    }
                }
                let places =
                    (0..cells).map(|cell| (n as f64, (other_first + cell as i64) as f64 + 0.5));
                survey.strength[f][line] = self.stack(model, f, places, 0.4, 1.0).map(|stack| {
                    let ends =
                        0.5 * (stack[0] + stack[1] + stack[STACK - 2] + stack[STACK - 1]) / 2.0;
                    stack[STACK / 4..=STACK - 1 - STACK / 4]
                        .iter()
                        .map(|&v| f64::from((v - ends).max(0.0)))
                        .sum::<f64>()
                });
            }
        }
        survey.tally();
        if !seen.is_empty() {
            survey.rms_px =
                (seen.iter().map(|(o, _)| o * o).sum::<f64>() / seen.len() as f64).sqrt();
        }
        survey
    }
}

impl Survey {
    /// Fills the running sums along each line.
    fn tally(&mut self) {
        for f in 0..2 {
            let (count, cells) = (self.lines[f].1, self.lines[1 - f].1);
            let mut sums = (
                vec![0u32; count * (cells + 1)],
                vec![0u32; count * (cells + 1)],
                vec![0.0f64; count * (cells + 1)],
            );
            for line in 0..count {
                for cell in 0..cells {
                    let (at, next) = (line * (cells + 1) + cell, line * (cells + 1) + cell + 1);
                    let index = line * cells + cell;
                    let found = self.found[f][index];
                    sums.0[next] = sums.0[at] + u32::from(self.looked[f][index]);
                    sums.1[next] = sums.1[at] + u32::from(found);
                    sums.2[next] = sums.2[at]
                        + if found {
                            f64::from(self.miss[f][index])
                        } else {
                            0.0
                        };
                }
            }
            self.sums[f] = sums;
        }
    }

    /// Looks, finds and summed misses of line `n` of family `f` over cells
    /// `m0..m1` of the other family.
    fn along(&self, f: usize, n: i64, m0: i64, m1: i64) -> (u32, u32, f64) {
        let (first, count) = self.lines[f];
        let (other_first, cells) = self.lines[1 - f];
        let line = n - first;
        if line < 0 || line as usize >= count {
            return (0, 0, 0.0);
        }
        let cell = |m: i64| (m - other_first).clamp(0, cells as i64) as usize;
        let (c0, c1) = (cell(m0), cell(m1));
        let row = line as usize * (cells + 1);
        let (looked, found, miss) = &self.sums[f];
        (
            looked[row + c1] - looked[row + c0],
            found[row + c1] - found[row + c0],
            miss[row + c1] - miss[row + c0],
        )
    }

    /// Whether line `n` of family `f` holds over cells `m0..m1`: found in
    /// most of its looks (those not inside something dark), close to where
    /// the model puts it. A model slipped by whole lines (curled paper)
    /// misses its lines there or finds them off.
    fn line_holds(&self, f: usize, n: i64, m0: i64, m1: i64) -> bool {
        let (looked, found, miss) = self.along(f, n, m0, m1);
        looked < 2 || line_is_held(looked, found, miss)
    }

    /// The share of a line's looks that found it, when it was looked at
    /// often enough to say.
    fn found_fraction(&self, f: usize, line: usize) -> Option<f64> {
        let cells = self.lines[1 - f].1;
        let range = line * cells..(line + 1) * cells;
        let looked = self.looked[f][range.clone()].iter().filter(|&&l| l).count();
        let found = self.found[f][range].iter().filter(|&&l| l).count();
        (looked >= 2).then(|| found as f64 / looked as f64)
    }

    /// Whether most lines the model puts in the region were found: the
    /// heavy ones when there are any (the light ones may be faint), else
    /// all. A model at the wrong spacing misses most of its lines.
    fn holds(&self, heavy: Option<(i64, [i64; 2])>) -> bool {
        (0..2).all(|f| {
            let (first, count) = self.lines[f];
            let (k, r) = heavy.map_or((1, 0), |(k, r)| (k, r[f]));
            let fractions: Vec<f64> = (0..count)
                .filter(|&line| (first + line as i64 - r).rem_euclid(k) == 0)
                .filter_map(|line| self.found_fraction(f, line))
                .collect();
            let seen = fractions.iter().filter(|&&found| found >= 0.3).count();
            seen >= 2 && 2 * seen >= fractions.len()
        })
    }

    /// Lines found in a fair share of their looks.
    fn seen_lines(&self, f: usize) -> u32 {
        (0..self.lines[f].1)
            .filter(|&line| {
                self.found_fraction(f, line)
                    .is_some_and(|found| found >= 0.3)
            })
            .count() as u32
    }

    fn found_at(&self, f: usize, n: i64, m: i64) -> Option<bool> {
        let (first, count) = self.lines[f];
        let (other_first, cells) = self.lines[1 - f];
        let line = n - first;
        let cell = m - other_first;
        if line < 0 || cell < 0 || line as usize >= count || cell as usize >= cells {
            return None;
        }
        let index = line as usize * cells + cell as usize;
        self.looked[f][index].then_some(self.found[f][index])
    }

    /// Whether the crossing of lines `i` and `j` was seen: at least three
    /// of its four arms found, and in the region.
    fn crossing_seen(&self, search: &Search, model: &Model, i: i64, j: i64) -> bool {
        let Some(at) = model.to_px([i as f64, j as f64]) else {
            return false;
        };
        let margin = (0.25 * model.spacing(0, at).min(model.spacing(1, at))).max(2.0);
        let [_, _, w, h] = search.bounds();
        if at[0] < margin || at[1] < margin || at[0] > w - margin || at[1] > h - margin {
            return false;
        }
        let arms = [
            self.found_at(0, i, j - 1),
            self.found_at(0, i, j),
            self.found_at(1, j, i - 1),
            self.found_at(1, j, i),
        ];
        let looked = arms.iter().filter(|arm| arm.is_some()).count();
        let found = arms.iter().filter(|arm| **arm == Some(true)).count();
        arms_enough(found, looked)
    }

    /// The share of looks inside lines `i0..=i1` × `j0..=j1` that found
    /// their line, of the lines every `k` from `r`.
    fn support(&self, [i0, i1, j0, j1]: [i64; 4], k: i64, r: [i64; 2]) -> f64 {
        let (mut looked, mut found) = (0u32, 0u32);
        for (f, (a0, a1), (b0, b1)) in [(0, (i0, i1), (j0, j1)), (1, (j0, j1), (i0, i1))] {
            for n in (a0..=a1).filter(|n| (n - r[f]).rem_euclid(k) == 0) {
                let (l, h, _) = self.along(f, n, b0, b1);
                looked += l;
                found += h;
            }
        }
        if looked == 0 {
            0.0
        } else {
            found as f64 / looked as f64
        }
    }

    /// The largest rectangle of whole squares of `k` lines (counting from
    /// lines `r`) whose corners were seen and whose lines were mostly found.
    fn rectangles(&self, search: &Search, model: &Model, k: i64, r: [i64; 2]) -> Vec<[i64; 4]> {
        let on = |f: usize| -> Vec<i64> {
            let (first, count) = self.lines[f];
            (first..first + count as i64)
                .filter(|n| (n - r[f]).rem_euclid(k) == 0)
                .collect()
        };
        let (us, vs) = (on(0), on(1));
        let seen: Vec<Vec<bool>> = us
            .iter()
            .map(|&i| {
                vs.iter()
                    .map(|&j| self.crossing_seen(search, model, i, j))
                    .collect()
            })
            .collect();
        let area = |[i0, i1, j0, j1]: [i64; 4]| {
            let corner = |i: i64, j: i64| model.to_px([i as f64, j as f64]).unwrap_or([0.0, 0.0]);
            let p = [
                corner(i0, j0),
                corner(i1, j0),
                corner(i1, j1),
                corner(i0, j1),
            ];
            0.5 * (0..4)
                .map(|i| p[i][0] * p[(i + 1) % 4][1] - p[(i + 1) % 4][0] * p[i][1])
                .sum::<f64>()
                .abs()
        };
        let mut found: Vec<([i64; 4], i64, f64)> = Vec::new();
        for a in 0..us.len() {
            for b in a + 1..us.len() {
                // Runs of lines across that hold between the two lines down.
                let mut c = 0;
                while c < vs.len() {
                    if !self.line_holds(1, vs[c], us[a], us[b]) {
                        c += 1;
                        continue;
                    }
                    let start = c;
                    while c < vs.len() && self.line_holds(1, vs[c], us[a], us[b]) {
                        c += 1;
                    }
                    let both: Vec<usize> =
                        (start..c).filter(|&c| seen[a][c] && seen[b][c]).collect();
                    let (Some(&c0), Some(&c1)) = (both.first(), both.last()) else {
                        continue;
                    };
                    if c1 == c0 {
                        continue;
                    }
                    let rectangle = [us[a], us[b], vs[c0], vs[c1]];
                    let down_hold = us[a..=b]
                        .iter()
                        .all(|&n| self.line_holds(0, n, vs[c0], vs[c1]));
                    if !down_hold || !supported(self.support(rectangle, k, r)) {
                        continue;
                    }
                    let squares = (us[b] - us[a]) * (vs[c1] - vs[c0]);
                    found.push((rectangle, squares, area(rectangle)));
                }
            }
        }
        found.sort_by(|x, y| y.1.cmp(&x.1).then(y.2.total_cmp(&x.2)));
        found
            .into_iter()
            .map(|(rectangle, _, _)| rectangle)
            .collect()
    }

    /// The heavier lines: every how many, and the line index of one per
    /// family; `None` when every line looks the same.
    fn major(&self, _model: &Model) -> Option<(i64, [i64; 2])> {
        // Per family, per line: mean strength, for lines found often enough.
        let strengths = &self.strength;
        // Every k-th line from some r is heavier than all the others: the
        // lightest of them (bar one in ten) against the heaviest others.
        let percentile = |mut values: Vec<f64>, at: f64| {
            values.sort_by(f64::total_cmp);
            values[((values.len() - 1) as f64 * at).round() as usize]
        };
        let mut best: Option<(i64, [i64; 2], f64)> = None;
        for k in 2..=10i64 {
            let mut residues = [0; 2];
            let mut separation = f64::MAX;
            for f in 0..2 {
                let (first, _) = self.lines[f];
                let mut family_best: Option<(i64, f64)> = None;
                for r in 0..k {
                    let (mut on, mut off) = (Vec::new(), Vec::new());
                    for (line, s) in strengths[f].iter().enumerate() {
                        let Some(s) = *s else { continue };
                        if (first + line as i64 - r).rem_euclid(k) == 0 {
                            on.push(s);
                        } else {
                            off.push(s);
                        }
                    }
                    if on.len() < 2 || off.len() < 3 {
                        continue;
                    }
                    let ratio = percentile(on, 0.1) / percentile(off, 0.9).max(1e-9);
                    if family_best.is_none_or(|(_, most)| ratio > most) {
                        family_best = Some((r, ratio));
                    }
                }
                match family_best {
                    Some((r, ratio)) => {
                        residues[f] = r;
                        separation = separation.min(ratio);
                    }
                    None => separation = 0.0,
                }
            }
            if separation >= 1.3 && best.is_none_or(|(_, _, most)| separation > most) {
                best = Some((k, residues, separation));
            }
        }
        best.map(|(k, r, _)| (k, r))
    }

    /// How sure the search is, from 0 to 1, judged on the lines counted
    /// (the heavy ones when there are any: faint light lines are no doubt).
    fn confidence(&self, search: &Search, model: &Model, heavy: Option<(i64, [i64; 2])>) -> f64 {
        let (w, h) = (search.dark.width as f64, search.dark.height as f64);
        let spacing = model
            .spacing(0, [0.5 * w, 0.5 * h])
            .min(model.spacing(1, [0.5 * w, 0.5 * h]));
        let all = [0, 1].map(|f| {
            let (first, count) = self.lines[f];
            let (other_first, cells) = self.lines[1 - f];
            let (k, r) = heavy.map_or((1, 0), |(k, r)| (k, r[f]));
            let (mut looked, mut found) = (0u32, 0u32);
            for n in (first..first + count as i64).filter(|n| (n - r).rem_euclid(k) == 0) {
                let (l, h, _) = self.along(f, n, other_first, other_first + cells as i64);
                looked += l;
                found += h;
            }
            f64::from(found) / f64::from(looked.max(1))
        });
        let found = all[0].min(all[1]);
        let fit = 1.0 / (1.0 + (self.rms_px / (0.06 * spacing).max(0.3)).powi(2));
        let lines = (self.seen_lines(0).min(self.seen_lines(1)) as f64 / 6.0).min(1.0);
        (found.sqrt() * fit * lines).clamp(0.0, 1.0)
    }
}

/// Whether a crossing's arms show it: three of four, or both of two at the
/// region's edge.
fn arms_enough(found: usize, looked: usize) -> bool {
    found >= 3 || (found >= 2 && found == looked)
}

/// Whether a line holds over a stretch: found in at least 70% of `looked`
/// looks, on average under a tenth of a spacing off (`miss` summed over
/// the finds, in spacings).
fn line_is_held(looked: u32, found: u32, miss: f64) -> bool {
    f64::from(found) >= 0.6 * f64::from(looked) && miss < 0.15 * f64::from(found.max(1))
}

/// Whether a rectangle's lines were found in enough of their looks.
fn supported(share: f64) -> bool {
    share >= 0.5
}

/// Whether the points found sit close to the model: RMS within 8% of the
/// finer spacing.
fn fit_is_tight(rms_px: f64, least_spacing: f64) -> bool {
    rms_px <= 0.08 * least_spacing
}

/// Whether the two spacings are close enough for square squares seen at
/// an angle (a turn of the paper of up to about 35°).
fn looks_square(least_spacing: f64, most_spacing: f64) -> bool {
    most_spacing <= 1.25 * least_spacing
}

/// The crossing of lines `i` and `j` on the photo's own pixels: each line
/// is found either side of the crossing and fitted straight, and the two
/// fits meet there. `None` when too little of either line shows.
fn refine_crossing(image: &InkImage, model: &Model, i: i64, j: i64) -> Option<[f64; 2]> {
    let (w, h) = (image.width as usize, image.height as usize);
    let sample = |x: f64, y: f64| -> Option<f32> {
        let fx = x - 0.5;
        let fy = y - 0.5;
        if !(fx >= 0.0 && fy >= 0.0) {
            return None;
        }
        let (ix, iy) = (fx as usize, fy as usize);
        if ix + 1 >= w || iy + 1 >= h {
            return None;
        }
        let (ax, ay) = ((fx - ix as f64) as f32, (fy - iy as f64) as f32);
        let p = |x: usize, y: usize| 1.0 - f32::from(image.pixels[y * w + x]) / 255.0;
        let top = p(ix, iy) + ax * (p(ix + 1, iy) - p(ix, iy));
        let bottom = p(ix, iy + 1) + ax * (p(ix + 1, iy + 1) - p(ix, iy + 1));
        Some(top + ay * (bottom - top))
    };
    let mut buffer = Vec::new();
    let mut lines = Vec::new();
    for f in 0..2 {
        let mut points: Vec<([f64; 2], f64)> = Vec::new();
        for side in [-1.0, 1.0] {
            for along in [0.3, 0.5, 0.7, 1.3, 1.5, 1.7] {
                let uv = if f == 0 {
                    [i as f64, j as f64 + side * along]
                } else {
                    [i as f64 + side * along, j as f64]
                };
                let at = model.to_px(uv)?;
                let [gx, gy] = model.gradient(f, at);
                let length = gx.hypot(gy);
                let spacing = 1.0 / length;
                let normal = [gx / length, gy / length];
                let r = (0.3 * spacing).clamp(1.5, 40.0);
                if let Probe::Line { offset, height, .. } = probe(
                    sample,
                    at,
                    normal,
                    r,
                    (r / 12.0).clamp(0.1, 0.25),
                    0.03,
                    &mut buffer,
                ) {
                    points.push((
                        [at[0] + offset * normal[0], at[1] + offset * normal[1]],
                        height,
                    ));
                }
            }
        }
        lines.push(fit_line(&mut points)?);
    }
    intersect(lines[0], lines[1])
}

/// Whether a refined crossing sits where the model puts it.
fn on_crossing(model: &Model, predicted: [f64; 2], refined: [f64; 2]) -> bool {
    let spacing = model.spacing(0, predicted).min(model.spacing(1, predicted));
    on_crossing_distance((refined[0] - predicted[0]).hypot(refined[1] - predicted[1]) / spacing)
}

/// Whether a refined crossing this many spacings from the model's still
/// counts as the model's: lens distortion moves crossings a little, a
/// slipped line half a spacing or more.
fn on_crossing_distance(spacings: f64) -> bool {
    spacings < 0.25
}

fn quad_area(p: [[f64; 2]; 4]) -> f64 {
    0.5 * (0..4)
        .map(|i| p[i][0] * p[(i + 1) % 4][1] - p[(i + 1) % 4][0] * p[i][1])
        .sum::<f64>()
        .abs()
}

/// The four refined corners of rectangle `[i0, i1, j0, j1]` when its edges
/// bear the count out on the pixels: every crossing every `k` lines along
/// the four edges that can be refined sits where the model says, at least
/// three in four can be, and the corners can. `None` when the model has
/// slipped a line somewhere along them (curled paper) or a corner can't be
/// placed.
fn edges_bear_out(
    image: &InkImage,
    model: &Model,
    [i0, i1, j0, j1]: [i64; 4],
    k: i64,
) -> Option<[[f64; 2]; 4]> {
    let mut points: Vec<(i64, i64)> = Vec::new();
    for i in (i0..=i1).step_by(k as usize) {
        points.push((i, j0));
        points.push((i, j1));
    }
    for j in (j0 + k..j1).step_by(k as usize) {
        points.push((i0, j));
        points.push((i1, j));
    }
    let corners = [(i0, j0), (i1, j0), (i1, j1), (i0, j1)];
    let mut placed = [[0.0; 2]; 4];
    let mut refined = 0;
    for &(i, j) in &points {
        let predicted = model.to_px([i as f64, j as f64])?;
        let corner = corners.iter().position(|&c| c == (i, j));
        match refine_crossing(image, model, i, j) {
            Some(at) if on_crossing(model, predicted, at) => {
                refined += 1;
                if let Some(c) = corner {
                    placed[c] = at;
                }
            }
            Some(_) => return None,
            None if corner.is_some() => return None,
            None => {}
        }
    }
    enough_refined(refined, points.len()).then_some(placed)
}

/// Whether enough crossings along a rectangle's edges could be refined to
/// trust its count: three in four (holes and rings hide the rest).
fn enough_refined(refined: usize, along_edges: usize) -> bool {
    4 * refined >= 3 * along_edges
}

/// A straight line through weighted points (total least squares), the
/// furthest dropped while they miss by more than a pixel: its point and
/// direction.
fn fit_line(points: &mut Vec<([f64; 2], f64)>) -> Option<([f64; 2], [f64; 2])> {
    loop {
        if points.len() < 4 {
            return None;
        }
        let total: f64 = points.iter().map(|p| p.1).sum();
        let mean = [
            points.iter().map(|p| p.1 * p.0[0]).sum::<f64>() / total,
            points.iter().map(|p| p.1 * p.0[1]).sum::<f64>() / total,
        ];
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for (p, weight) in points.iter() {
            let (dx, dy) = (p[0] - mean[0], p[1] - mean[1]);
            xx += weight * dx * dx;
            xy += weight * dx * dy;
            yy += weight * dy * dy;
        }
        let angle = 0.5 * (2.0 * xy).atan2(xx - yy);
        let direction = [angle.cos(), angle.sin()];
        let miss = |p: &[f64; 2]| {
            ((p[0] - mean[0]) * direction[1] - (p[1] - mean[1]) * direction[0]).abs()
        };
        let (worst, distance) = points
            .iter()
            .enumerate()
            .map(|(k, p)| (k, miss(&p.0)))
            .max_by(|a, b| a.1.total_cmp(&b.1))?;
        if distance <= 1.0 {
            return Some((mean, direction));
        }
        points.swap_remove(worst);
    }
}

fn intersect((p, d): ([f64; 2], [f64; 2]), (q, e): ([f64; 2], [f64; 2])) -> Option<[f64; 2]> {
    let denominator = d[0] * e[1] - d[1] * e[0];
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / denominator;
    Some([p[0] + t * d[0], p[1] + t * d[1]])
}

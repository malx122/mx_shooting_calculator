//! Hole suggestions: bullet holes found in a zoomed part of a target photo,
//! for the shooter to tick or untick. Nothing here marks a shot; it only
//! proposes circles.
//!
//! The detector looks for holes of one known size, the calibre at the
//! photo's scale, by classic image processing:
//!
//! 1. The region is turned into log brightness, so that a hole's contrast
//!    against the paper around it is a difference that uneven light does not
//!    change. Holes much larger than [`WORK_DIAMETER_MAX_PX`] are looked for
//!    in a box-averaged copy of the region.
//! 2. A centre–surround filter (a box the size of a hole's core against a
//!    square ring just outside the hole, both from an integral image) gives
//!    each pixel a response: positive where it is darker than its
//!    surroundings, negative where it is lighter. Holes are dark on paper and
//!    light grey on a black aiming area; both are looked for.
//! 3. Local extremes of the response, at least a third of a hole apart, are
//!    the candidates, strongest first, if the brightness a third of a hole
//!    out stays near the core's in seven of eight directions (lines,
//!    crossings and edges don't). At most [`MAX_CANDIDATES`] are examined,
//!    within a fixed amount of work, so a crisp printed grid or a scan stays
//!    quick.
//! 4. Around each candidate the pixels at least half as far from the
//!    surround's level as the hole's core are taken, and opened (eroded, then
//!    rebuilt from the part under the candidate): grid lines, ring lines,
//!    digits and other strokes thinner than about 0.45 of a hole vanish. What
//!    remains must lie within a window six holes across (not a black area, a
//!    torn edge or a long line) and be covered well by one round circle of
//!    the right size, by two along its length, or by up to six placed one by
//!    one where each covers the most: holes that touch. A blob that fills
//!    the smallest rectangle around it, or whose edge strays from its
//!    circles at corners and straight sides, is a printed square or block.
//! 5. Circles of different blobs closer than 0.7 of a hole keep the best.
//!
//! A hole must be at least a third darker than the paper around it (or half
//! as light again as a black area, which must be even around it), and lie
//! more than about one hole inside the region's edge. Printed marks of a
//! hole's size and shape, such as an aiming dot or, in a blurred photo, small
//! ring numbers, can't be told from holes; holes that straddle the edge of a
//! black area, and groups merged into one ragged hole of more than six, are
//! not found.

use std::fmt;

/// The smallest hole, in pixels across, the detector looks for; smaller
/// holes have to be zoomed in on first.
pub const MIN_DIAMETER_PX: f64 = 6.0;
/// How many holes across the region's shorter side must be at least.
pub const MIN_REGION_DIAMETERS: f64 = 5.0;
/// The default for [`HoleQuery::tolerance`].
pub const DEFAULT_TOLERANCE: f64 = 0.22;
/// The largest hole, in pixels across, looked for at the region's own
/// resolution; larger holes are looked for in a reduced copy.
pub const WORK_DIAMETER_MAX_PX: f64 = 20.0;
/// The most pixels a region may have.
pub const MAX_REGION_PIXELS: u64 = 16_000_000;
/// How close to a marked shot, in hole diameters, a suggestion is
/// [`Hole::near_marked`], and how close to a point to avoid it is left out.
pub const NEAR_DIAMETERS: f64 = 0.6;

/// Added to brightness (0–255) before taking its logarithm, so the noise of
/// dark ink isn't magnified.
const LOG_OFFSET: f32 = 20.0;
/// The least response, in log brightness, a candidate needs.
const PEAK_RESPONSE: f32 = 0.2;
/// The least contrast, in log brightness, of a hole's core against its
/// surroundings: a hole a third darker, or half as light again.
const MIN_CONTRAST: f32 = 0.4;
/// The most the surround's interquartile range may be around a light hole,
/// as a share of its contrast: a light hole lies in an even dark area.
const SURROUND_SPREAD: f32 = 0.5;
/// The most a single hole's longest spread may exceed its shortest.
const SINGLE_ELONGATION: f64 = 1.2;
/// The most two touching holes' longest spread may exceed their shortest.
const PAIR_ELONGATION: f64 = 2.8;
/// The erosion radius, in hole diameters: strokes thinner than twice this
/// are not holes.
const EROSION_DIAMETERS: f64 = 0.22;
/// Half the analysis window, in hole diameters.
const WINDOW_DIAMETERS: f64 = 3.0;
/// The most a blob may look like a printed square or block: its rectangle
/// fill plus half its outline's stray from the circles (`Shape::squareness`);
/// round holes and their pairs come to about 0.75–0.87, printed marks to
/// 0.9 and more.
const MAX_SQUARENESS: f64 = 0.885;
/// The most candidates examined, strongest first.
pub const MAX_CANDIDATES: usize = 400;
/// The most pixel visits spent on candidates: about 100 ms on a phone.
const WORK_BUDGET: u64 = 60_000_000;
/// The most touching holes one blob is split into.
const MAX_CLUSTER: usize = 6;
/// How far apart, in hole diameters, circles of different blobs must be.
const SUPPRESS_DIAMETERS: f64 = 0.7;

/// The pixels of an image region.
#[derive(Clone, Copy, Debug)]
pub enum Pixels<'a> {
    /// One byte a pixel, row by row from the top left.
    Gray(&'a [u8]),
    /// Four bytes a pixel (red, green, blue, alpha, not premultiplied), row
    /// by row from the top left; transparent parts count as white paper.
    Rgba(&'a [u8]),
}

/// The part of a photo to look for holes in, at a resolution the caller
/// picks (usually the zoomed area on screen).
#[derive(Clone, Copy, Debug)]
pub struct Region<'a> {
    pub width_px: u32,
    pub height_px: u32,
    pub pixels: Pixels<'a>,
}

/// What to look for. Positions are in the region's pixels, with (0, 0) the
/// top-left corner of the top-left pixel (whose centre is (0.5, 0.5)).
#[derive(Clone, Copy, Debug)]
pub struct HoleQuery<'a> {
    /// The calibre's diameter in the region's pixels, from the calibre and
    /// the photo's scale ([`expected_diameter_px`]); `None` when either is
    /// unknown, which [`find_holes`] refuses.
    pub diameter_px: Option<f64>,
    /// How far a hole's measured diameter may be from `diameter_px`, as a
    /// fraction of it: holes in paper come out about 0.8–1.2 × the calibre.
    pub tolerance: f64,
    /// Shots already marked; suggestions close to one are
    /// [`Hole::near_marked`].
    pub marked: &'a [(f64, f64)],
    /// Points where nothing is suggested, such as the aim point and the scale
    /// handles: suggestions within [`NEAR_DIAMETERS`] of one are left out.
    pub avoid: &'a [(f64, f64)],
}

impl HoleQuery<'_> {
    /// A query for holes `diameter_px` across, with the default tolerance and
    /// nothing marked or avoided.
    pub fn new(diameter_px: Option<f64>) -> Self {
        HoleQuery {
            diameter_px,
            tolerance: DEFAULT_TOLERANCE,
            marked: &[],
            avoid: &[],
        }
    }
}

/// A calibre's diameter in pixels at `mm_per_px`, or `None` when either is
/// unknown or not a positive size.
pub fn expected_diameter_px(calibre_mm: Option<f64>, mm_per_px: Option<f64>) -> Option<f64> {
    let diameter = calibre_mm? / mm_per_px?;
    (diameter.is_finite() && diameter > 0.0).then_some(diameter)
}

/// A suggested hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    /// The centre, in the region's pixels (see [`HoleQuery`]).
    pub x_px: f64,
    pub y_px: f64,
    /// The measured diameter in the region's pixels.
    pub diameter_px: f64,
    /// How sure the detector is, from 0 to 1: contrast, roundness and size
    /// together. Split holes score lower than single ones.
    pub score: f64,
    /// Whether a marked shot is within [`NEAR_DIAMETERS`] of the expected
    /// diameter: probably the same hole, so the suggestion should start
    /// unticked.
    pub near_marked: bool,
    /// Whether the hole is lighter than its surroundings (a hole in a black
    /// aiming area) rather than darker.
    pub light: bool,
    /// Set when the hole is one of two or three touching holes found as one
    /// blob.
    pub split: Option<Split>,
}

/// Holes that touch, found as one blob and split into circles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split {
    /// The blob's number, the same for every circle it was split into and
    /// different for each blob in one result.
    pub blob: u32,
    /// How many circles the blob was split into: 2 to 6.
    pub circles: u8,
}

/// Why holes weren't looked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HoleError {
    /// The calibre or the scale is unknown, so the hole's size is.
    NoSize,
    /// Holes are under [`MIN_DIAMETER_PX`] across here: zoom in by at least
    /// `need_zoom` (more than 1).
    TooSmall { need_zoom: f64 },
    /// The region is under [`MIN_REGION_DIAMETERS`] holes across: zoom out
    /// to `need_zoom` (less than 1) or show a larger area.
    TooLarge { need_zoom: f64 },
    /// The pixels don't fit the region's size, or the region is larger than
    /// [`MAX_REGION_PIXELS`].
    BadRegion,
}

impl fmt::Display for HoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HoleError::NoSize => {
                formatter.write_str("holes can be found once the scale and the calibre are set")
            }
            HoleError::TooSmall { need_zoom } => {
                write!(formatter, "zoom in {need_zoom:.1}× to find holes")
            }
            HoleError::TooLarge { need_zoom } => {
                write!(formatter, "zoom out to {need_zoom:.2}× to find holes")
            }
            HoleError::BadRegion => formatter.write_str("the picture can't be searched for holes"),
        }
    }
}

impl std::error::Error for HoleError {}

/// The holes in `region`, best first: about 20 ms for a 1000 × 1000 region
/// on a computer, and at most 40 ms for a crisp printed grid.
pub fn find_holes(region: &Region<'_>, query: &HoleQuery<'_>) -> Result<Vec<Hole>, HoleError> {
    let diameter = query
        .diameter_px
        .filter(|diameter| diameter.is_finite() && *diameter > 0.0)
        .ok_or(HoleError::NoSize)?;
    let (width, height) = (region.width_px as usize, region.height_px as usize);
    let pixels = width as u64 * height as u64;
    let fits = match region.pixels {
        Pixels::Gray(bytes) => bytes.len() as u64 == pixels,
        Pixels::Rgba(bytes) => bytes.len() as u64 == pixels * 4,
    };
    if !fits || pixels > MAX_REGION_PIXELS {
        return Err(HoleError::BadRegion);
    }
    if diameter < MIN_DIAMETER_PX {
        return Err(HoleError::TooSmall {
            need_zoom: MIN_DIAMETER_PX / diameter,
        });
    }
    let short = width.min(height) as f64;
    if short < MIN_REGION_DIAMETERS * diameter {
        return Err(HoleError::TooLarge {
            need_zoom: short / (MIN_REGION_DIAMETERS * diameter),
        });
    }
    let tolerance = query.tolerance.clamp(0.05, 0.6);
    let step = (diameter / WORK_DIAMETER_MAX_PX).ceil().max(1.0) as usize;
    let plane = log_brightness(region, step);
    let scale = step as f64;
    let found = Detector::new(&plane, diameter / scale, tolerance).run();

    let near = NEAR_DIAMETERS * diameter;
    let within = |points: &[(f64, f64)], x: f64, y: f64| {
        points.iter().any(|(px, py)| (px - x).hypot(py - y) < near)
    };
    let mut blobs = Vec::new();
    let mut holes = Vec::new();
    for circle in found {
        let (x, y) = (circle.x * scale, circle.y * scale);
        if within(query.avoid, x, y) {
            continue;
        }
        let split = circle.split.map(|(blob, circles)| {
            let number = match blobs.iter().position(|seen| *seen == blob) {
                Some(index) => index,
                None => {
                    blobs.push(blob);
                    blobs.len() - 1
                }
            };
            Split {
                blob: number as u32,
                circles,
            }
        });
        holes.push(Hole {
            x_px: x,
            y_px: y,
            diameter_px: circle.diameter * scale,
            score: circle.score,
            near_marked: within(query.marked, x, y),
            light: circle.light,
            split,
        });
    }
    Ok(holes)
}

/// Log brightness, box-averaged over `step` × `step` pixels.
struct Plane {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

fn log_brightness(region: &Region<'_>, step: usize) -> Plane {
    let width = region.width_px as usize;
    let (out_width, out_height) = (width / step, region.height_px as usize / step);
    let mut sums = vec![0u32; out_width * out_height];
    for y in 0..out_height * step {
        let row = (y / step) * out_width;
        for x in 0..out_width * step {
            let index = y * width + x;
            let luma = match region.pixels {
                Pixels::Gray(bytes) => u32::from(bytes[index]),
                Pixels::Rgba(bytes) => {
                    let [red, green, blue, alpha] =
                        [0, 1, 2, 3].map(|c| u32::from(bytes[index * 4 + c]));
                    let luma = (77 * red + 150 * green + 29 * blue + 128) >> 8;
                    (luma * alpha + 255 * (255 - alpha) + 127) / 255
                }
            };
            sums[row + x / step] += luma;
        }
    }
    let count = (step * step) as f32;
    Plane {
        width: out_width,
        height: out_height,
        values: sums
            .into_iter()
            .map(|sum| (sum as f32 / count + LOG_OFFSET).ln())
            .collect(),
    }
}

/// A circle found, in the working plane's pixels.
#[derive(Clone, Copy, Debug)]
struct Circle {
    x: f64,
    y: f64,
    diameter: f64,
    score: f64,
    light: bool,
    /// The blob's id, for every circle.
    blob: usize,
    /// The blob's id and circle count when it was split.
    split: Option<(usize, u8)>,
}

struct Detector<'a> {
    plane: &'a Plane,
    /// The expected diameter in the plane's pixels.
    diameter: f64,
    tolerance: f64,
    integral: Vec<f64>,
    /// Pixel visits spent on candidates so far.
    work: std::cell::Cell<u64>,
    /// Half-sides of the core box and of the surround ring's inner and outer
    /// squares.
    core: usize,
    ring_inner: usize,
    ring_outer: usize,
}

/// Pixels of a structure already looked at, for dark and light candidates.
struct Claims {
    dark: Vec<bool>,
    light: Vec<bool>,
}

impl<'a> Detector<'a> {
    fn new(plane: &'a Plane, diameter: f64, tolerance: f64) -> Self {
        let core = ((0.35 * diameter).round() as usize).max(1);
        let ring_inner = ((0.75 * diameter).round() as usize).max(core + 1);
        let ring_outer = ((1.1 * diameter).round() as usize).max(ring_inner + 1);
        Detector {
            plane,
            diameter,
            tolerance,
            integral: integral(plane),
            work: std::cell::Cell::new(0),
            core,
            ring_inner,
            ring_outer,
        }
    }

    fn box_sum(&self, x: usize, y: usize, half: usize) -> f64 {
        let stride = self.plane.width + 1;
        let (x0, y0, x1, y1) = (x - half, y - half, x + half + 1, y + half + 1);
        self.integral[y1 * stride + x1]
            - self.integral[y0 * stride + x1]
            - self.integral[y1 * stride + x0]
            + self.integral[y0 * stride + x0]
    }

    /// The surround's mean minus the core's mean at each pixel: positive
    /// where darker than around it.
    fn response(&self) -> Vec<f32> {
        let (width, height) = (self.plane.width, self.plane.height);
        let mut response = vec![0.0f32; width * height];
        let outer = self.ring_outer;
        if width <= 2 * outer || height <= 2 * outer {
            return response;
        }
        let side = |half: usize| ((2 * half + 1) * (2 * half + 1)) as f64;
        let ring_count = side(outer) - side(self.ring_inner);
        let core_count = side(self.core);
        for y in outer..height - outer {
            for x in outer..width - outer {
                let ring = self.box_sum(x, y, outer) - self.box_sum(x, y, self.ring_inner);
                let core = self.box_sum(x, y, self.core);
                response[y * width + x] = (ring / ring_count - core / core_count) as f32;
            }
        }
        response
    }

    /// Whether the plane stays at least half as dark (or light) as the core
    /// in seven of eight directions a third of a hole from `index`: not a
    /// line, a crossing or an edge.
    fn solid_around(&self, index: usize, light: bool) -> bool {
        let plane = self.plane;
        let (x, y) = (index % plane.width, index / plane.width);
        let side = |half: usize| ((2 * half + 1) * (2 * half + 1)) as f64;
        let ring = (self.box_sum(x, y, self.ring_outer) - self.box_sum(x, y, self.ring_inner))
            / (side(self.ring_outer) - side(self.ring_inner));
        let core = self.box_sum(x, y, self.core) / side(self.core);
        let sign = if light { -1.0 } else { 1.0 };
        let depth = sign * (ring - core);
        let distance = (0.3 * self.diameter).max(1.0);
        let at = |px: f64, py: f64| {
            let (x0, y0) = (px.floor(), py.floor());
            let (fx, fy) = (px - x0, py - y0);
            let (x0, y0) = (x0 as usize, y0 as usize);
            let (x1, y1) = (
                (x0 + 1).min(plane.width - 1),
                (y0 + 1).min(plane.height - 1),
            );
            let value = |x: usize, y: usize| f64::from(plane.values[y * plane.width + x]);
            (value(x0, y0) * (1.0 - fx) + value(x1, y0) * fx) * (1.0 - fy)
                + (value(x0, y1) * (1.0 - fx) + value(x1, y1) * fx) * fy
        };
        let solid = (0..8)
            .filter(|step| {
                let angle = f64::from(*step) * std::f64::consts::FRAC_PI_4;
                let sample = at(
                    x as f64 + distance * angle.cos(),
                    y as f64 + distance * angle.sin(),
                );
                sign * (ring - sample) >= 0.5 * depth
            })
            .count();
        solid >= 7
    }

    fn spend(&self, visits: usize) {
        self.work.set(self.work.get() + visits as u64);
    }

    fn run(&self) -> Vec<Circle> {
        let (width, height) = (self.plane.width, self.plane.height);
        let response = self.response();
        let reach = ((0.35 * self.diameter).round() as usize).max(1);
        let strength: Vec<f32> = response.iter().map(|value| value.abs()).collect();
        let largest = max_filter(&strength, width, height, reach);
        let mut peaks: Vec<usize> = (0..width * height)
            .filter(|&index| {
                strength[index] >= PEAK_RESPONSE
                    && strength[index] >= largest[index]
                    && self.solid_around(index, response[index] < 0.0)
            })
            .collect();
        peaks.sort_by(|a, b| strength[*b].total_cmp(&strength[*a]));
        let mut taken = vec![false; width * height];
        let mut candidates = Vec::new();
        for index in peaks {
            if taken[index] {
                continue;
            }
            candidates.push(index);
            if candidates.len() == MAX_CANDIDATES {
                break;
            }
            let (x, y) = (index % width, index / width);
            for ny in y.saturating_sub(reach)..(y + reach + 1).min(height) {
                taken
                    [ny * width + x.saturating_sub(reach)..ny * width + (x + reach + 1).min(width)]
                    .fill(true);
            }
        }

        let mut claims = Claims {
            dark: vec![false; width * height],
            light: vec![false; width * height],
        };
        let mut circles = Vec::new();
        let mut blobs = 0;
        for index in candidates {
            if self.work.get() > WORK_BUDGET {
                break;
            }
            let light = response[index] < 0.0;
            let claimed = if light { &claims.light } else { &claims.dark };
            if claimed[index] {
                continue;
            }
            let found = self.analyse(index % width, index / width, light, &mut claims, blobs);
            if !found.is_empty() {
                blobs += 1;
                circles.extend(found);
            }
        }

        circles.sort_by(|a, b| b.score.total_cmp(&a.score));
        let spacing = SUPPRESS_DIAMETERS * self.diameter;
        let mut kept: Vec<Circle> = Vec::new();
        for circle in circles {
            let crowded = kept.iter().any(|other| {
                other.blob != circle.blob
                    && (other.x - circle.x).hypot(other.y - circle.y) < spacing
            });
            if !crowded {
                kept.push(circle);
            }
        }
        kept
    }

    /// The hole or touching holes at a candidate, if it is one.
    fn analyse(
        &self,
        peak_x: usize,
        peak_y: usize,
        light: bool,
        claims: &mut Claims,
        blob: usize,
    ) -> Vec<Circle> {
        let plane = self.plane;
        let diameter = self.diameter;
        let half = (WINDOW_DIAMETERS * diameter).ceil() as usize;
        let x0 = peak_x.saturating_sub(half);
        let y0 = peak_y.saturating_sub(half);
        let x1 = (peak_x + half + 1).min(plane.width);
        let y1 = (peak_y + half + 1).min(plane.height);
        let (w, h) = (x1 - x0, y1 - y0);
        self.spend(30 * w * h);
        let value = |x: usize, y: usize| plane.values[y * plane.width + x];
        let sign = if light { -1.0 } else { 1.0 };

        // The surround's level and the core's contrast against it.
        let mut ring = Vec::new();
        let (inner, outer) = (self.ring_inner, self.ring_outer);
        for y in peak_y.saturating_sub(outer)..(peak_y + outer + 1).min(plane.height) {
            for x in peak_x.saturating_sub(outer)..(peak_x + outer + 1).min(plane.width) {
                if x.abs_diff(peak_x).max(y.abs_diff(peak_y)) > inner {
                    ring.push(value(x, y));
                }
            }
        }
        let surround = quantile(&mut ring, 0.5);
        let spread = quantile(&mut ring, 0.75) - quantile(&mut ring, 0.25);
        let mut core = Vec::new();
        for y in peak_y.saturating_sub(self.core)..(peak_y + self.core + 1).min(plane.height) {
            for x in peak_x.saturating_sub(self.core)..(peak_x + self.core + 1).min(plane.width) {
                core.push(sign * (surround - value(x, y)));
            }
        }
        let contrast = quantile(&mut core, 0.75);
        if contrast < MIN_CONTRAST || (light && spread > SURROUND_SPREAD * contrast) {
            return Vec::new();
        }
        let level = contrast / 2.0;
        let holeness: Vec<f32> = (0..w * h)
            .map(|i| sign * (surround - value(x0 + i % w, y0 + i / w)))
            .collect();
        let mask: Vec<bool> = holeness.iter().map(|depth| *depth >= level).collect();

        // Opening: erode, keep the part under the candidate, grow it back.
        let radius = ((EROSION_DIAMETERS * diameter * 3.0).round() as u16).max(3);
        let background: Vec<bool> = mask.iter().map(|inside| !inside).collect();
        let depth = chamfer(&background, w, h);
        let eroded: Vec<bool> = depth.iter().map(|d| *d > radius).collect();
        let (px, py) = (peak_x - x0, peak_y - y0);
        let reach = (0.5 * diameter).powi(2);
        let seed = (0..w * h)
            .filter(|&i| eroded[i])
            .map(|i| {
                let (dx, dy) = ((i % w) as f64 - px as f64, (i / w) as f64 - py as f64);
                (dx * dx + dy * dy, i)
            })
            .filter(|(distance, _)| *distance <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, seed)) = seed else {
            return Vec::new();
        };
        let kernel = flood(&eroded, w, h, seed);
        let near_kernel = chamfer(&kernel, w, h);
        let blob_mask: Vec<bool> = (0..w * h)
            .map(|i| mask[i] && near_kernel[i] <= radius)
            .collect();
        let claimed = if light {
            &mut claims.light
        } else {
            &mut claims.dark
        };
        let mut claim = |pixels: &[bool]| {
            for (i, _) in pixels.iter().enumerate().filter(|(_, inside)| **inside) {
                claimed[(y0 + i / w) * plane.width + x0 + i % w] = true;
            }
        };
        let touches_edge = (0..w * h).any(|i| {
            blob_mask[i] && (i % w == 0 || i % w == w - 1 || i / w == 0 || i / w == h - 1)
        });
        if touches_edge {
            claim(&kernel);
            return Vec::new();
        }

        let shape = Shape::of(&blob_mask, &holeness, level, w, h);
        let low = (1.0 - self.tolerance) * diameter;
        let high = (1.0 + self.tolerance) * diameter;
        let fits = self
            .single(&shape, low, high)
            .or_else(|| self.pair(&shape, low, high))
            .or_else(|| self.cluster(&shape, &blob_mask, w, h));
        let Some((circles, quality)) = fits else {
            return Vec::new();
        };
        claim(&blob_mask);
        let count = circles.len();
        let penalty = match count {
            1 => 1.0,
            2 => 0.85,
            3 => 0.7,
            _ => 0.6,
        };
        let contrast_term = f64::from(contrast.min(1.0)).sqrt();
        let quality_term = ((quality - 0.75) / 0.25).clamp(0.0, 1.0);
        circles
            .into_iter()
            .map(|(x, y, d)| {
                let size_term = 1.0 - ((d / diameter - 1.0).abs() / self.tolerance).min(1.0);
                Circle {
                    x: x0 as f64 + x,
                    y: y0 as f64 + y,
                    diameter: d,
                    score: (contrast_term
                        * (0.5 + 0.5 * quality_term)
                        * (0.7 + 0.3 * size_term)
                        * penalty)
                        .clamp(0.0, 1.0),
                    light,
                    blob,
                    split: (count > 1).then_some((blob, count as u8)),
                }
            })
            .collect()
    }

    /// One circle the blob's area across, if it covers the blob well.
    fn single(&self, shape: &Shape, low: f64, high: f64) -> Option<(Vec<Disc>, f64)> {
        let diameter = 2.0 * (shape.area / std::f64::consts::PI).sqrt();
        if !(low..=high).contains(&diameter) || shape.elongation() > SINGLE_ELONGATION {
            return None;
        }
        let circles = vec![(shape.cx, shape.cy, diameter)];
        let quality = shape.coverage(&circles);
        (quality >= 0.85 && shape.squareness(&circles) <= MAX_SQUARENESS)
            .then_some((circles, quality))
    }

    /// Two equal circles along the blob's long axis, sized to its area and
    /// length, if they cover it well.
    fn pair(&self, shape: &Shape, low: f64, high: f64) -> Option<(Vec<Disc>, f64)> {
        if !(1.2..=PAIR_ELONGATION).contains(&shape.elongation()) {
            return None;
        }
        let (ux, uy) = shape.major_axis();
        let (near, far) = shape
            .pixels
            .iter()
            .map(|(x, y)| (x - shape.cx) * ux + (y - shape.cy) * uy)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
                (lo.min(t), hi.max(t))
            });
        let length = far - near + 1.0;
        let (mut best, mut best_error) = (None, f64::INFINITY);
        for step in 0..=24 {
            let diameter = low + (high - low) * f64::from(step) / 24.0;
            let separation = length - diameter;
            if separation < 0.45 * self.diameter || separation > 1.2 * diameter {
                continue;
            }
            let error = (union_area(diameter / 2.0, separation) - shape.area).abs();
            if error < best_error {
                (best, best_error) = (Some((diameter, separation)), error);
            }
        }
        let (diameter, separation) = best?;
        if best_error > 0.25 * shape.area {
            return None;
        }
        let middle = (near + far) / 2.0;
        let (mx, my) = (shape.cx + middle * ux, shape.cy + middle * uy);
        let offset = separation / 2.0;
        let circles = vec![
            (mx - offset * ux, my - offset * uy, diameter),
            (mx + offset * ux, my + offset * uy, diameter),
        ];
        let quality = shape.coverage(&circles);
        (quality >= 0.8 && shape.squareness(&circles) <= MAX_SQUARENESS)
            .then_some((circles, quality))
    }

    /// Circles of the expected size for a blob of several touching holes:
    /// placed one by one where each covers the most of the blob not yet
    /// covered (and the least outside it), then thinned and nudged; `None`
    /// unless they cover the blob well.
    fn cluster(
        &self,
        shape: &Shape,
        mask: &[bool],
        width: usize,
        height: usize,
    ) -> Option<(Vec<Disc>, f64)> {
        let diameter = self.diameter;
        let single = std::f64::consts::PI * diameter * diameter / 4.0;
        if shape.area < 1.5 * single * (1.0 - self.tolerance).powi(2) {
            return None;
        }
        let radius = diameter / 2.0;
        let reach = radius.ceil() as isize;
        let disc: Vec<(isize, isize)> = (-reach..=reach)
            .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| ((dx * dx + dy * dy) as f64) <= radius * radius)
            .collect();
        let area = disc.len() as f64;
        let pixels = |(cx, cy): (isize, isize)| {
            disc.iter().filter_map(move |(dx, dy)| {
                let (x, y) = (cx + dx, cy + dy);
                (x >= 0 && y >= 0 && x < width as isize && y < height as isize)
                    .then(|| y as usize * width + x as usize)
            })
        };
        let value = |index: usize| if mask[index] { 1.0 } else { -0.5 };
        let mut counts = vec![0u8; width * height];
        // What the blob gains from a circle at `centre`, or loses without
        // it when `present` is 1.
        let worth = |counts: &[u8], centre, present: u8| {
            pixels(centre)
                .filter(|&index| counts[index] == present)
                .map(value)
                .sum::<f64>()
        };
        let shift = |counts: &mut [u8], centre, add: bool| {
            for index in pixels(centre) {
                if add {
                    counts[index] += 1;
                } else {
                    counts[index] -= 1;
                }
            }
        };

        let depth = chamfer(
            &mask.iter().map(|inside| !inside).collect::<Vec<_>>(),
            width,
            height,
        );
        let inner = (0.3 * diameter * 3.0).round() as u16;
        let candidates: Vec<(isize, isize)> = (0..width * height)
            .filter(|&i| depth[i] >= inner)
            .map(|i| ((i % width) as isize, (i / width) as isize))
            .collect();
        let mut centres: Vec<(isize, isize)> = Vec::new();
        while centres.len() < MAX_CLUSTER + 2 {
            self.spend(candidates.len() * disc.len());
            let best = candidates
                .iter()
                .map(|&centre| (centre, worth(&counts, centre, 0)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            let Some((centre, _)) = best.filter(|(_, gain)| *gain >= 0.35 * area) else {
                break;
            };
            shift(&mut counts, centre, true);
            centres.push(centre);
        }
        let prune = |counts: &mut Vec<u8>, centres: &mut Vec<(isize, isize)>| {
            while let Some((index, loss)) = centres
                .iter()
                .enumerate()
                .map(|(index, &centre)| (index, worth(counts, centre, 1)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                if loss >= 0.3 * area {
                    break;
                }
                shift(counts, centres.remove(index), false);
            }
        };
        prune(&mut counts, &mut centres);
        for _ in 0..2 {
            for centre in centres.iter_mut() {
                let (cx, cy) = *centre;
                shift(&mut counts, (cx, cy), false);
                let moved = (-2..=2)
                    .flat_map(|dy| (-2..=2).map(move |dx| (cx + dx, cy + dy)))
                    .map(|centre| (centre, worth(&counts, centre, 0)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or((cx, cy), |(centre, _)| centre);
                shift(&mut counts, moved, true);
                *centre = moved;
            }
        }
        prune(&mut counts, &mut centres);
        if !(2..=MAX_CLUSTER).contains(&centres.len()) {
            return None;
        }
        let circles: Vec<_> = centres
            .iter()
            .map(|(x, y)| (*x as f64 + 0.5, *y as f64 + 0.5, diameter))
            .collect();
        for (i, a) in circles.iter().enumerate() {
            if circles[i + 1..]
                .iter()
                .any(|b| (a.0 - b.0).hypot(a.1 - b.1) < 0.4 * diameter)
            {
                return None;
            }
        }
        let quality = shape.coverage(&circles);
        (quality >= 0.75 && shape.rectangle_fill() <= MAX_SQUARENESS).then_some((circles, quality))
    }
}

/// A circle's centre and diameter.
type Disc = (f64, f64, f64);

/// A blob's pixels (their centres, in window coordinates) and moments.
struct Shape {
    pixels: Vec<(f64, f64)>,
    /// Where the blob's edge crosses the threshold, between each edge pixel
    /// and each of its four neighbours outside.
    outline: Vec<(f64, f64)>,
    area: f64,
    cx: f64,
    cy: f64,
    xx: f64,
    yy: f64,
    xy: f64,
}

impl Shape {
    fn of(mask: &[bool], holeness: &[f32], level: f32, width: usize, height: usize) -> Self {
        let mut outline = Vec::new();
        for (i, _) in mask.iter().enumerate().filter(|(_, inside)| **inside) {
            let (x, y) = (i % width, i / width);
            let neighbours = [
                (x > 0).then(|| (i - 1, -1.0, 0.0)),
                (x + 1 < width).then(|| (i + 1, 1.0, 0.0)),
                (y > 0).then(|| (i - width, 0.0, -1.0)),
                (y + 1 < height).then(|| (i + width, 0.0, 1.0)),
            ];
            for (j, dx, dy) in neighbours.into_iter().flatten() {
                if !mask[j] {
                    let fall = holeness[i] - holeness[j];
                    let t = if fall > 0.0 {
                        f64::from(((holeness[i] - level) / fall).clamp(0.0, 1.0))
                    } else {
                        0.5
                    };
                    outline.push((x as f64 + 0.5 + t * dx, y as f64 + 0.5 + t * dy));
                }
            }
        }
        let pixels: Vec<(f64, f64)> = (0..width * height)
            .filter(|&i| mask[i])
            .map(|i| ((i % width) as f64 + 0.5, (i / width) as f64 + 0.5))
            .collect();
        let area = pixels.len() as f64;
        let cx = pixels.iter().map(|p| p.0).sum::<f64>() / area;
        let cy = pixels.iter().map(|p| p.1).sum::<f64>() / area;
        let moment = |f: &dyn Fn(&(f64, f64)) -> f64| pixels.iter().map(f).sum::<f64>() / area;
        let xx = moment(&|p| (p.0 - cx).powi(2));
        let yy = moment(&|p| (p.1 - cy).powi(2));
        let xy = moment(&|p| (p.0 - cx) * (p.1 - cy));
        Shape {
            pixels,
            outline,
            area,
            cx,
            cy,
            xx,
            yy,
            xy,
        }
    }

    fn eigenvalues(&self) -> (f64, f64) {
        let mean = (self.xx + self.yy) / 2.0;
        let spread = (((self.xx - self.yy) / 2.0).powi(2) + self.xy * self.xy).sqrt();
        (mean + spread, (mean - spread).max(1e-9))
    }

    /// The ratio of the blob's longest to its shortest spread.
    fn elongation(&self) -> f64 {
        let (major, minor) = self.eigenvalues();
        (major / minor).sqrt()
    }

    fn major_axis(&self) -> (f64, f64) {
        let angle = 0.5 * (2.0 * self.xy).atan2(self.xx - self.yy);
        (angle.cos(), angle.sin())
    }

    /// The blob's share of the smallest rectangle around its outline, at
    /// any angle: about π/4 for round holes and their pairs, near 1 for
    /// printed squares and blocks.
    fn rectangle_fill(&self) -> f64 {
        (0..30)
            .map(|step| {
                let (sin, cos) = (f64::from(step) * std::f64::consts::FRAC_PI_2 / 30.0).sin_cos();
                let (mut u, mut v) = (
                    (f64::INFINITY, f64::NEG_INFINITY),
                    (f64::INFINITY, f64::NEG_INFINITY),
                );
                for (x, y) in &self.outline {
                    let (a, b) = (x * cos + y * sin, y * cos - x * sin);
                    u = (u.0.min(a), u.1.max(a));
                    v = (v.0.min(b), v.1.max(b));
                }
                self.area / ((u.1 - u.0) * (v.1 - v.0)).max(1.0)
            })
            .fold(0.0, f64::max)
    }

    /// How much the blob looks like a printed square or block rather than
    /// round holes: corners fill out the smallest rectangle around it, and
    /// corners and straight sides stray from the circles fitted to it.
    fn squareness(&self, circles: &[Disc]) -> f64 {
        self.rectangle_fill() + 0.5 * self.outline_error(circles)
    }

    /// How far the blob's edge strays from the circles' outline: the 90th
    /// percentile of the distance, in radii. Corners and straight sides
    /// stray further than a torn round edge.
    fn outline_error(&self, circles: &[Disc]) -> f64 {
        let radius = circles.iter().map(|c| c.2 / 2.0).sum::<f64>() / circles.len() as f64;
        let mut errors: Vec<f32> = self
            .outline
            .iter()
            .map(|(x, y)| {
                let edge = circles
                    .iter()
                    .map(|(cx, cy, d)| (x - cx).hypot(y - cy) - d / 2.0)
                    .fold(f64::INFINITY, f64::min);
                (edge.abs() / radius) as f32
            })
            .collect();
        f64::from(quantile(&mut errors, 0.9))
    }

    /// The lesser of the blob's share inside the circles and the circles'
    /// share covered by the blob.
    fn coverage(&self, circles: &[Disc]) -> f64 {
        let inside = |x: f64, y: f64| {
            circles
                .iter()
                .any(|(cx, cy, d)| (x - cx).powi(2) + (y - cy).powi(2) <= d * d / 4.0)
        };
        let covered = self.pixels.iter().filter(|(x, y)| inside(*x, *y)).count() as f64;
        let reach = circles
            .iter()
            .map(|(_, _, d)| d / 2.0 + 1.0)
            .fold(0.0, f64::max);
        let (xs, ys) = (circles.iter().map(|c| c.0), circles.iter().map(|c| c.1));
        let left = (xs.clone().fold(f64::INFINITY, f64::min) - reach).floor();
        let right = (xs.fold(f64::NEG_INFINITY, f64::max) + reach).ceil();
        let top = (ys.clone().fold(f64::INFINITY, f64::min) - reach).floor();
        let bottom = (ys.fold(f64::NEG_INFINITY, f64::max) + reach).ceil();
        let mut union = 0.0f64;
        let mut y = top + 0.5;
        while y < bottom {
            let mut x = left + 0.5;
            while x < right {
                if inside(x, y) {
                    union += 1.0;
                }
                x += 1.0;
            }
            y += 1.0;
        }
        (covered / self.area).min(covered / union.max(1.0))
    }
}

/// The area covered by two circles of `radius` whose centres are
/// `separation` apart.
fn union_area(radius: f64, separation: f64) -> f64 {
    let circle = std::f64::consts::PI * radius * radius;
    if separation >= 2.0 * radius {
        return 2.0 * circle;
    }
    let overlap = 2.0 * radius * radius * (separation / (2.0 * radius)).acos()
        - separation / 2.0 * (4.0 * radius * radius - separation * separation).sqrt();
    2.0 * circle - overlap
}

/// A summed-area table, one row and column larger than the plane.
fn integral(plane: &Plane) -> Vec<f64> {
    let stride = plane.width + 1;
    let mut table = vec![0.0f64; stride * (plane.height + 1)];
    for y in 0..plane.height {
        let mut row = 0.0;
        for x in 0..plane.width {
            row += f64::from(plane.values[y * plane.width + x]);
            table[(y + 1) * stride + x + 1] = table[y * stride + x + 1] + row;
        }
    }
    table
}

/// Each value's largest neighbour within `reach` in both directions.
fn max_filter(values: &[f32], width: usize, height: usize, reach: usize) -> Vec<f32> {
    let mut rows = vec![0.0f32; values.len()];
    for y in 0..height {
        let row = &values[y * width..(y + 1) * width];
        for x in 0..width {
            let span = &row[x.saturating_sub(reach)..(x + reach + 1).min(width)];
            rows[y * width + x] = span.iter().copied().fold(f32::MIN, f32::max);
        }
    }
    let mut both = vec![0.0f32; values.len()];
    for y in 0..height {
        for x in 0..width {
            both[y * width + x] = (y.saturating_sub(reach)..(y + reach + 1).min(height))
                .map(|row| rows[row * width + x])
                .fold(f32::MIN, f32::max);
        }
    }
    both
}

/// The `share` quantile of `values` (reordering them); 0 for none.
fn quantile(values: &mut [f32], share: f64) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * share).round() as usize;
    *values.select_nth_unstable_by(index, f32::total_cmp).1
}

/// The chamfer distance (3 a step, 4 a diagonal step) from each pixel to the
/// nearest `source` pixel; pixels outside the window are not sources.
fn chamfer(source: &[bool], width: usize, height: usize) -> Vec<u16> {
    const FAR: u16 = u16::MAX / 2;
    let mut distance: Vec<u16> = source.iter().map(|s| if *s { 0 } else { FAR }).collect();
    let at = |x: usize, y: usize| y * width + x;
    for y in 0..height {
        for x in 0..width {
            let mut best = distance[at(x, y)];
            if x > 0 {
                best = best.min(distance[at(x - 1, y)] + 3);
            }
            if y > 0 {
                best = best.min(distance[at(x, y - 1)] + 3);
                if x > 0 {
                    best = best.min(distance[at(x - 1, y - 1)] + 4);
                }
                if x + 1 < width {
                    best = best.min(distance[at(x + 1, y - 1)] + 4);
                }
            }
            distance[at(x, y)] = best;
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let mut best = distance[at(x, y)];
            if x + 1 < width {
                best = best.min(distance[at(x + 1, y)] + 3);
            }
            if y + 1 < height {
                best = best.min(distance[at(x, y + 1)] + 3);
                if x + 1 < width {
                    best = best.min(distance[at(x + 1, y + 1)] + 4);
                }
                if x > 0 {
                    best = best.min(distance[at(x - 1, y + 1)] + 4);
                }
            }
            distance[at(x, y)] = best;
        }
    }
    distance
}

/// The 8-connected part of `mask` that holds `seed`.
fn flood(mask: &[bool], width: usize, height: usize, seed: usize) -> Vec<bool> {
    let mut part = vec![false; mask.len()];
    let mut stack = vec![seed];
    part[seed] = true;
    while let Some(index) = stack.pop() {
        let (x, y) = (index % width, index / width);
        for ny in y.saturating_sub(1)..(y + 2).min(height) {
            for nx in x.saturating_sub(1)..(x + 2).min(width) {
                let next = ny * width + nx;
                if mask[next] && !part[next] {
                    part[next] = true;
                    stack.push(next);
                }
            }
        }
    }
    part
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn unit(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1u64 << 53) as f64
        }

        fn range(&mut self, low: f64, high: f64) -> f64 {
            low + (high - low) * self.unit()
        }

        fn normal(&mut self) -> f64 {
            let (a, b) = (self.unit().max(1e-12), self.unit());
            (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
        }
    }

    /// How a photo of a target looks.
    #[derive(Clone, Copy)]
    struct Look {
        /// Gaussian blur, in pixels.
        blur: f64,
        jpeg: Option<u8>,
        /// How much darker the far corner is: 0 even, 0.7 strongly uneven.
        uneven: f64,
        /// Sensor and paper noise, 1 normal.
        noise: f64,
        rgba: bool,
        /// A torn, ragged paper edge down the right side.
        torn: bool,
    }

    const CLEAN: Look = Look {
        blur: 0.0,
        jpeg: None,
        uneven: 0.0,
        noise: 1.0,
        rgba: false,
        torn: false,
    };
    const PHOTO: Look = Look {
        blur: 1.0,
        jpeg: Some(80),
        uneven: 0.4,
        noise: 1.0,
        rgba: true,
        torn: false,
    };

    /// What is on a target.
    #[derive(Clone, Copy)]
    struct Layout {
        size: usize,
        /// The calibre in pixels.
        diameter: f64,
        singles: usize,
        pairs: usize,
        /// Groups of five holes within 1.3 holes of a point, overlapping as
        /// they fall.
        groups: usize,
        /// Holes may straddle the black area's edge.
        anywhere: bool,
    }

    struct Truth {
        x: f64,
        y: f64,
        pair: Option<usize>,
    }

    struct Target {
        size: usize,
        diameter: f64,
        rgba: bool,
        pixels: Vec<u8>,
        truths: Vec<Truth>,
    }

    impl Target {
        fn find(&self, marked: &[(f64, f64)], avoid: &[(f64, f64)]) -> Vec<Hole> {
            let region = Region {
                width_px: self.size as u32,
                height_px: self.size as u32,
                pixels: if self.rgba {
                    Pixels::Rgba(&self.pixels)
                } else {
                    Pixels::Gray(&self.pixels)
                },
            };
            let query = HoleQuery {
                marked,
                avoid,
                ..HoleQuery::new(Some(self.diameter))
            };
            find_holes(&region, &query).expect("holes looked for")
        }
    }

    fn coverage(signed_distance: f64) -> f64 {
        (0.5 - signed_distance).clamp(0.0, 1.0)
    }

    /// Draws a stroke from `a` to `b`, `width` wide, in `colour(x, y)`.
    fn stroke(
        canvas: &mut [f64],
        size: usize,
        a: (f64, f64),
        b: (f64, f64),
        width: f64,
        colour: &dyn Fn(f64, f64) -> f64,
    ) {
        let reach = width + 2.0;
        let x0 = (a.0.min(b.0) - reach).max(0.0) as usize;
        let x1 = ((a.0.max(b.0) + reach) as usize).min(size);
        let y0 = (a.1.min(b.1) - reach).max(0.0) as usize;
        let y1 = ((a.1.max(b.1) + reach) as usize).min(size);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).max(1e-9);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let t = (((px - a.0) * dx + (py - a.1) * dy) / length).clamp(0.0, 1.0);
                let distance = (px - a.0 - t * dx).hypot(py - a.1 - t * dy);
                let cover = coverage(distance - width / 2.0);
                if cover > 0.0 {
                    let value = &mut canvas[y * size + x];
                    *value += (colour(px, py) - *value) * cover;
                }
            }
        }
    }

    /// Seven-segment digits (or random glyphs) `height` tall from `origin`.
    #[allow(clippy::too_many_arguments)]
    fn glyphs(
        canvas: &mut [f64],
        size: usize,
        origin: (f64, f64),
        height: f64,
        width: f64,
        masks: &[u8],
        colour: &dyn Fn(f64, f64) -> f64,
    ) {
        let w = 0.55 * height;
        let segments = [
            ((0.0, 0.0), (w, 0.0)),
            ((w, 0.0), (w, height / 2.0)),
            ((w, height / 2.0), (w, height)),
            ((0.0, height), (w, height)),
            ((0.0, height / 2.0), (0.0, height)),
            ((0.0, 0.0), (0.0, height / 2.0)),
            ((0.0, height / 2.0), (w, height / 2.0)),
        ];
        for (index, mask) in masks.iter().enumerate() {
            let left = origin.0 + index as f64 * (w + 0.45 * height);
            for (bit, (a, b)) in segments.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    let a = (left + a.0, origin.1 + a.1);
                    let b = (left + b.0, origin.1 + b.1);
                    stroke(canvas, size, a, b, width, colour);
                }
            }
        }
    }

    const DIGITS: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];

    fn gaussian_blur(canvas: &mut [f64], size: usize, sigma: f64) {
        let reach = (3.0 * sigma).ceil() as isize;
        let kernel: Vec<f64> = (-reach..=reach)
            .map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp())
            .collect();
        let total: f64 = kernel.iter().sum();
        let sample = |line: &[f64], i: isize| line[i.clamp(0, size as isize - 1) as usize];
        let mut row = vec![0.0; size];
        for y in 0..size {
            let line = canvas[y * size..(y + 1) * size].to_vec();
            for (x, out) in row.iter_mut().enumerate() {
                *out = (-reach..=reach)
                    .map(|k| kernel[(k + reach) as usize] * sample(&line, x as isize + k))
                    .sum::<f64>()
                    / total;
            }
            canvas[y * size..(y + 1) * size].copy_from_slice(&row);
        }
        for x in 0..size {
            let line: Vec<f64> = (0..size).map(|y| canvas[y * size + x]).collect();
            for y in 0..size {
                canvas[y * size + x] = (-reach..=reach)
                    .map(|k| kernel[(k + reach) as usize] * sample(&line, y as isize + k))
                    .sum::<f64>()
                    / total;
            }
        }
    }

    /// A bullet hole about `d` across with a ragged edge: dark with a grey
    /// grease ring on paper, light grey where `in_black`.
    fn draw_hole(
        canvas: &mut [f64],
        size: usize,
        rng: &mut Rng,
        (hx, hy): (f64, f64),
        d: f64,
        noise: f64,
        in_black: &dyn Fn(f64, f64) -> bool,
    ) {
        let radius = d * rng.range(0.88, 1.08) / 2.0;
        let ragged: Vec<(f64, f64)> = (3..9)
            .map(|_| (rng.range(0.0, 0.03), rng.range(0.0, 6.3)))
            .collect();
        let fill = if in_black(hx, hy) {
            rng.range(140.0, 185.0)
        } else {
            rng.range(25.0, 50.0)
        };
        let grease = 0.13 * 2.0 * radius;
        let reach = radius * 1.3 + grease + 2.0;
        let y0 = (hy - reach).max(0.0) as usize;
        let x0 = (hx - reach).max(0.0) as usize;
        for y in y0..((hy + reach) as usize).min(size) {
            for x in x0..((hx + reach) as usize).min(size) {
                let (dx, dy) = (x as f64 + 0.5 - hx, y as f64 + 0.5 - hy);
                let angle = dy.atan2(dx);
                let edge = radius
                    * (1.0
                        + ragged
                            .iter()
                            .enumerate()
                            .map(|(k, (a, phase))| a * ((k + 3) as f64 * angle + phase).sin())
                            .sum::<f64>());
                let signed = dx.hypot(dy) - edge;
                let value = &mut canvas[y * size + x];
                if !in_black(x as f64 + 0.5, y as f64 + 0.5) {
                    *value *= 1.0 - 0.38 * coverage(signed - grease);
                }
                let cover = coverage(signed);
                *value += (fill + 4.0 * noise * rng.normal() - *value) * cover;
            }
        }
    }

    /// A photo of a paper target: paper texture, a 10 mm grid, scoring
    /// rings with a black aiming area, ring numbers and a bold title, and
    /// holes with ragged edges and a grease ring (light grey in the black).
    fn render(layout: Layout, look: Look, seed: u64) -> Target {
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x1234);
        let size = layout.size;
        let n = size as f64;
        let d = layout.diameter;
        let mm = d / 6.5;
        let (cx, cy) = (n * rng.range(0.42, 0.58), n * rng.range(0.42, 0.58));
        let black = 0.22 * n;
        let ring_step = black / 3.0;
        let in_black = move |x: f64, y: f64| (x - cx).hypot(y - cy) < black;
        let ink = move |x: f64, y: f64| if in_black(x, y) { 205.0 } else { 45.0 };
        let waves: Vec<(f64, f64, f64)> = (0..3)
            .map(|_| {
                (
                    rng.range(0.005, 0.03),
                    rng.range(0.005, 0.03),
                    rng.range(0.0, 6.3),
                )
            })
            .collect();
        let grid = 10.0 * mm;
        let (gx, gy) = (rng.range(0.0, grid), rng.range(0.0, grid));
        let grid_width = 0.25 * mm;
        let mut canvas = vec![0.0f64; size * size];
        for y in 0..size {
            for x in 0..size {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let mut v = 226.0
                    + waves
                        .iter()
                        .map(|(fx, fy, phase)| 2.0 * (fx * px + fy * py + phase).sin())
                        .sum::<f64>()
                    + 2.5 * look.noise * rng.normal();
                let line = |p: f64, offset: f64| {
                    let t = (p - offset).rem_euclid(grid);
                    coverage(t.min(grid - t) - grid_width / 2.0)
                };
                v *= 1.0 - 0.35 * line(px, gx).max(line(py, gy));
                let r = (px - cx).hypot(py - cy);
                let dark = coverage(r - black);
                v += (28.0 + 3.0 * look.noise * rng.normal() - v) * dark;
                let ring = (r / ring_step).round().max(1.0) * ring_step;
                let cover = coverage((r - ring).abs() - 0.35 * mm / 2.0);
                v += (if ring < black { 205.0 } else { 45.0 } - v) * cover;
                canvas[y * size + x] = v;
            }
        }
        let digit_height = 4.0 * mm;
        for k in 1..8 {
            let along = (k as f64 - 0.5) * ring_step;
            let label = [DIGITS[(9 - k) % 10]];
            for (ox, oy) in [(along, 0.0), (-along, 0.0), (0.0, along), (0.0, -along)] {
                let origin = (cx + ox - 0.15 * digit_height, cy + oy - 0.5 * digit_height);
                glyphs(
                    &mut canvas,
                    size,
                    origin,
                    digit_height,
                    0.55 * mm,
                    &label,
                    &ink,
                );
            }
        }
        let title: Vec<u8> = (0..6).map(|_| (rng.next() as u8) | 0x40).collect();
        glyphs(
            &mut canvas,
            size,
            (2.0 * mm, 2.0 * mm),
            8.0 * mm,
            1.2 * mm,
            &title,
            &|_, _| 40.0,
        );

        let mut truths: Vec<Truth> = Vec::new();
        let margin = 2.5 * d;
        let torn_at = 0.9 * n;
        let fits = |truths: &[Truth], x: f64, y: f64, spacing: f64, partner: Option<usize>| {
            let r = (x - cx).hypot(y - cy);
            x > margin
                && y > margin
                && x < n - margin
                && y < n - margin
                && (!look.torn || x < torn_at - 2.0 * d)
                && (layout.anywhere || (r - black).abs() > 0.9 * d)
                && truths.iter().enumerate().all(|(index, other)| {
                    Some(index) == partner || (other.x - x).hypot(other.y - y) > spacing
                })
        };
        let place = |rng: &mut Rng, truths: &[Truth], spacing: f64| {
            let want_black = rng.unit() < 0.35;
            for _ in 0..400 {
                let (x, y) = (rng.range(0.0, n), rng.range(0.0, n));
                if fits(truths, x, y, spacing, None) && in_black(x, y) == want_black {
                    return Some((x, y));
                }
            }
            None
        };
        for _ in 0..layout.pairs {
            for _ in 0..50 {
                let Some((x, y)) = place(&mut rng, &truths, 3.2 * d) else {
                    break;
                };
                let angle = rng.range(0.0, 6.3);
                let separation = rng.range(0.55, 0.95) * d;
                let (x2, y2) = (x + separation * angle.cos(), y + separation * angle.sin());
                let first = truths.len();
                if fits(&truths, x2, y2, 3.2 * d, None) && in_black(x, y) == in_black(x2, y2) {
                    truths.push(Truth {
                        x,
                        y,
                        pair: Some(first + 1),
                    });
                    truths.push(Truth {
                        x: x2,
                        y: y2,
                        pair: Some(first),
                    });
                    break;
                }
            }
        }
        for _ in 0..layout.groups {
            let Some((gx, gy)) = place(&mut rng, &truths, 4.5 * d) else {
                continue;
            };
            let mut placed = 0;
            for _ in 0..200 {
                let (angle, reach) = (rng.range(0.0, 6.3), 1.3 * d * rng.unit().sqrt());
                let (x, y) = (gx + reach * angle.cos(), gy + reach * angle.sin());
                if fits(&truths, x, y, 0.5 * d, None) && in_black(x, y) == in_black(gx, gy) {
                    truths.push(Truth { x, y, pair: None });
                    placed += 1;
                    if placed == 5 {
                        break;
                    }
                }
            }
        }
        for _ in 0..layout.singles {
            if let Some((x, y)) = place(&mut rng, &truths, 1.8 * d) {
                truths.push(Truth { x, y, pair: None });
            }
        }

        for truth in &truths {
            draw_hole(
                &mut canvas,
                size,
                &mut rng,
                (truth.x, truth.y),
                d,
                look.noise,
                &in_black,
            );
        }

        if look.torn {
            for y in 0..size {
                let edge = torn_at + 6.0 * (y as f64 / 7.0).sin() + 3.0 * rng.normal();
                for x in 0..size {
                    let cover = coverage(edge - (x as f64 + 0.5));
                    let value = &mut canvas[y * size + x];
                    *value += (50.0 - *value) * cover;
                }
            }
        }
        if look.blur > 0.0 {
            gaussian_blur(&mut canvas, size, look.blur);
        }
        for y in 0..size {
            for x in 0..size {
                let (u, v) = (x as f64 / n, y as f64 / n);
                let light = 1.0 - look.uneven * (0.6 * u + 0.4 * (v - 0.2).powi(2));
                let value = &mut canvas[y * size + x];
                *value = *value * light + 1.5 * look.noise * rng.normal();
            }
        }
        let gray: Vec<u8> = canvas
            .iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect();
        let mut pixels = if look.rgba {
            gray.iter()
                .flat_map(|v| {
                    let v = f64::from(*v);
                    [v * 1.02, v * 0.99, v * 0.9].map(|c| c.round().clamp(0.0, 255.0) as u8)
                })
                .collect::<Vec<u8>>()
        } else {
            gray
        };
        let colour = if look.rgba {
            image::ExtendedColorType::Rgb8
        } else {
            image::ExtendedColorType::L8
        };
        if let Some(quality) = look.jpeg {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode(&pixels, size as u32, size as u32, colour)
                .expect("encoded");
            let decoded = image::load_from_memory(&jpeg).expect("decoded");
            pixels = if look.rgba {
                decoded.into_rgb8().into_raw()
            } else {
                decoded.into_luma8().into_raw()
            };
        }
        if look.rgba {
            pixels = pixels
                .chunks(3)
                .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                .collect();
        }
        Target {
            size,
            diameter: d,
            rgba: look.rgba,
            pixels,
            truths,
        }
    }

    #[derive(Default, Clone, Copy)]
    struct Tally {
        truths: usize,
        found: usize,
        suggestions: usize,
        right: usize,
        pairs: usize,
        pairs_split: usize,
    }

    impl Tally {
        fn recall(&self) -> f64 {
            self.found as f64 / self.truths.max(1) as f64
        }

        fn precision(&self) -> f64 {
            if self.suggestions == 0 {
                1.0
            } else {
                self.right as f64 / self.suggestions as f64
            }
        }
    }

    /// Matches suggestions to true holes within a quarter of a hole, nearest
    /// first, each at most once.
    fn tally(target: &Target, holes: &[Hole]) -> Tally {
        let reach = 0.25 * target.diameter;
        let mut pairs = Vec::new();
        for (t, truth) in target.truths.iter().enumerate() {
            for (h, hole) in holes.iter().enumerate() {
                let distance = (truth.x - hole.x_px).hypot(truth.y - hole.y_px);
                if distance <= reach {
                    pairs.push((distance, t, h));
                }
            }
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut truth_to = vec![None; target.truths.len()];
        let mut used = vec![false; holes.len()];
        for (_, t, h) in pairs {
            if truth_to[t].is_none() && !used[h] {
                truth_to[t] = Some(h);
                used[h] = true;
            }
        }
        let mut result = Tally {
            truths: target.truths.len(),
            found: truth_to.iter().flatten().count(),
            suggestions: holes.len(),
            right: used.iter().filter(|u| **u).count(),
            ..Tally::default()
        };
        for (t, truth) in target.truths.iter().enumerate() {
            let Some(partner) = truth.pair.filter(|p| *p > t) else {
                continue;
            };
            result.pairs += 1;
            let split = |index: Option<usize>| index.and_then(|h| holes[h].split);
            if let (Some(a), Some(b)) = (split(truth_to[t]), split(truth_to[partner]))
                && a.blob == b.blob
            {
                result.pairs_split += 1;
            }
        }
        result
    }

    fn scenario(name: &str, layout: Layout, look: Look, seeds: u64) -> Tally {
        let mut total = Tally::default();
        for seed in 0..seeds {
            let target = render(layout, look, seed);
            let holes = target.find(&[], &[]);
            let one = tally(&target, &holes);
            total.truths += one.truths;
            total.found += one.found;
            total.suggestions += one.suggestions;
            total.right += one.right;
            total.pairs += one.pairs;
            total.pairs_split += one.pairs_split;
        }
        eprintln!(
            "{name:<10} holes {:>3}  recall {:>5.1} %  precision {:>5.1} %  suggestions {:>3}  pairs split {}/{}",
            total.truths,
            100.0 * total.recall(),
            100.0 * total.precision(),
            total.suggestions,
            total.pairs_split,
            total.pairs,
        );
        total
    }

    const SINGLES: Layout = Layout {
        size: 480,
        diameter: 12.0,
        singles: 14,
        pairs: 0,
        groups: 0,
        anywhere: false,
    };

    #[test]
    fn clean_targets_are_found() {
        for (name, look) in [("clean", CLEAN), ("photo", PHOTO)] {
            let tally = scenario(name, SINGLES, look, 4);
            assert!(tally.recall() >= 0.9, "{name} recall {}", tally.recall());
            assert!(
                tally.precision() >= 0.9,
                "{name} precision {}",
                tally.precision()
            );
        }
    }

    #[test]
    fn a_blank_target_gives_nothing() {
        let blank = Layout {
            singles: 0,
            ..SINGLES
        };
        let torn = Look {
            torn: true,
            ..PHOTO
        };
        let rough = Look {
            blur: 1.6,
            jpeg: Some(50),
            uneven: 0.7,
            noise: 2.0,
            ..PHOTO
        };
        for look in [CLEAN, PHOTO, torn, rough] {
            for seed in 0..3 {
                let target = render(blank, look, seed);
                let holes = target.find(&[], &[]);
                assert!(holes.is_empty(), "seed {seed}: {holes:?}");
            }
        }
    }

    #[test]
    fn touching_holes_are_split() {
        let layout = Layout {
            size: 520,
            diameter: 14.0,
            singles: 3,
            pairs: 5,
            groups: 0,
            anywhere: false,
        };
        let tally = scenario("touching", layout, PHOTO, 4);
        assert!(tally.recall() >= 0.85, "recall {}", tally.recall());
        assert!(tally.precision() >= 0.85, "precision {}", tally.precision());
        assert!(
            tally.pairs_split * 10 >= tally.pairs * 7,
            "{}/{}",
            tally.pairs_split,
            tally.pairs
        );
    }

    #[test]
    fn hard_targets_are_reported() {
        let small = Layout {
            size: 360,
            diameter: 7.0,
            ..SINGLES
        };
        let large = Layout {
            size: 640,
            diameter: 32.0,
            singles: 10,
            ..SINGLES
        };
        let hard = Look {
            blur: 1.6,
            jpeg: Some(50),
            uneven: 0.7,
            noise: 2.0,
            rgba: false,
            torn: true,
        };
        let anywhere = Layout {
            size: 420,
            diameter: 9.0,
            anywhere: true,
            ..SINGLES
        };
        let groups = Layout {
            size: 480,
            singles: 2,
            groups: 3,
            ..SINGLES
        };
        let blurred = Look {
            blur: 2.5,
            jpeg: Some(70),
            ..PHOTO
        };
        scenario("groups", groups, PHOTO, 4);
        scenario("blurred", SINGLES, blurred, 4);
        let results = [
            scenario("small", small, PHOTO, 4),
            scenario("large", large, PHOTO, 3),
            scenario("hard", anywhere, hard, 4),
        ];
        for tally in results {
            assert!(tally.recall() >= 0.6, "recall {}", tally.recall());
            assert!(tally.precision() >= 0.8, "precision {}", tally.precision());
        }
    }

    fn jpeg_round_trip(gray: &[u8], size: usize, quality: u8) -> Vec<u8> {
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode(gray, size as u32, size as u32, image::ExtendedColorType::L8)
            .expect("encoded");
        image::load_from_memory(&jpeg)
            .expect("decoded")
            .into_luma8()
            .into_raw()
    }

    /// Paper with printed solid squares a hole wide, bold blocks 0.7 × 1.1
    /// holes (some turned), and real holes.
    fn printed_marks(d: f64, blur: f64, jpeg: Option<u8>, seed: u64) -> Target {
        let mut rng = Rng(seed ^ 0x5EED);
        let cell = 3.4 * d;
        let size = (4.0 * cell).ceil() as usize;
        let mut canvas: Vec<f64> = (0..size * size)
            .map(|_| 228.0 + 2.5 * rng.normal())
            .collect();
        let mut truths = Vec::new();
        for index in 0..16 {
            let centre = (
                (index % 4) as f64 * cell + cell / 2.0 + rng.range(-0.3, 0.3) * d,
                (index / 4) as f64 * cell + cell / 2.0 + rng.range(-0.3, 0.3) * d,
            );
            if index >= 11 {
                draw_hole(&mut canvas, size, &mut rng, centre, d, 1.0, &|_, _| false);
                truths.push(Truth {
                    x: centre.0,
                    y: centre.1,
                    pair: None,
                });
                continue;
            }
            let half = if index < 6 {
                (d / 2.0, d / 2.0)
            } else {
                (0.35 * d, 0.55 * d)
            };
            let angle = if index % 2 == 0 {
                0.0
            } else {
                rng.range(0.0, 1.5)
            };
            let (sin, cos) = angle.sin_cos();
            let reach = (half.0.hypot(half.1) + 2.0) as isize;
            let (cx, cy) = (centre.0 as isize, centre.1 as isize);
            for y in (cy - reach).max(0)..(cy + reach).min(size as isize) {
                for x in (cx - reach).max(0)..(cx + reach).min(size as isize) {
                    let (dx, dy) = (x as f64 + 0.5 - centre.0, y as f64 + 0.5 - centre.1);
                    let (u, v) = (
                        (dx * cos + dy * sin).abs() - half.0,
                        (dy * cos - dx * sin).abs() - half.1,
                    );
                    let signed = u.max(0.0).hypot(v.max(0.0)) + u.max(v).min(0.0);
                    let value = &mut canvas[y as usize * size + x as usize];
                    *value += (35.0 - *value) * coverage(signed);
                }
            }
        }
        if blur > 0.0 {
            gaussian_blur(&mut canvas, size, blur);
        }
        let mut pixels: Vec<u8> = canvas
            .iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect();
        if let Some(quality) = jpeg {
            pixels = jpeg_round_trip(&pixels, size, quality);
        }
        Target {
            size,
            diameter: d,
            rgba: false,
            pixels,
            truths,
        }
    }

    #[test]
    fn printed_squares_and_blocks_are_not_holes() {
        let mut total = Tally::default();
        for d in [39.0, 19.5, 10.4] {
            for (blur, jpeg) in [(0.0, None), (0.04 * d, Some(80))] {
                for seed in 0..2 {
                    let target = printed_marks(d, blur, jpeg, seed);
                    let holes = target.find(&[], &[]);
                    let one = tally(&target, &holes);
                    total.truths += one.truths;
                    total.found += one.found;
                    total.suggestions += one.suggestions;
                    total.right += one.right;
                    assert_eq!(one.suggestions, one.right, "d {d} blur {blur}: {holes:?}");
                }
            }
        }
        eprintln!(
            "marks      holes {:>3}  recall {:>5.1} %  precision {:>5.1} %",
            total.truths,
            100.0 * total.recall(),
            100.0 * total.precision()
        );
        assert!(total.recall() >= 0.9, "recall {}", total.recall());
    }

    /// A crisp black-on-white grid, `width` px lines every `pitch` px.
    fn crisp_grid(size: usize, pitch: usize, width: usize) -> Vec<u8> {
        (0..size * size)
            .map(|i| {
                let (x, y) = (i % size, i / size);
                if x % pitch < width || y % pitch < width {
                    0
                } else {
                    255
                }
            })
            .collect()
    }

    #[test]
    fn crisp_grids_are_quick_and_empty() {
        for (pitch, diameter) in [(30, 10.0), (12, 6.0), (20, 8.0)] {
            let pixels = crisp_grid(1000, pitch, 2);
            let start = std::time::Instant::now();
            let holes = find_gray(1000, &pixels, &HoleQuery::new(Some(diameter)));
            let elapsed = start.elapsed();
            eprintln!(
                "crisp grid pitch {pitch} d {diameter}: {elapsed:?}, {} suggestions",
                holes.len()
            );
            assert!(
                holes.len() <= 3,
                "pitch {pitch}: {:?}",
                &holes[..holes.len().min(5)]
            );
            if !cfg!(debug_assertions) {
                assert!(elapsed.as_millis() <= 150, "pitch {pitch}: {elapsed:?}");
            }
        }
    }

    fn disc(size: usize, centre: (f64, f64), diameter: f64, paper: f64, hole: f64) -> Vec<u8> {
        (0..size * size)
            .map(|i| {
                let (x, y) = ((i % size) as f64 + 0.5, (i / size) as f64 + 0.5);
                let cover = coverage((x - centre.0).hypot(y - centre.1) - diameter / 2.0);
                (paper + (hole - paper) * cover).round() as u8
            })
            .collect()
    }

    fn find_gray(size: usize, pixels: &[u8], query: &HoleQuery<'_>) -> Vec<Hole> {
        let region = Region {
            width_px: size as u32,
            height_px: size as u32,
            pixels: Pixels::Gray(pixels),
        };
        find_holes(&region, query).expect("holes looked for")
    }

    #[test]
    fn a_hole_is_measured_to_a_fraction_of_a_pixel() {
        let pixels = disc(96, (47.3, 50.8), 13.0, 230.0, 30.0);
        let holes = find_gray(96, &pixels, &HoleQuery::new(Some(12.0)));
        assert_eq!(holes.len(), 1, "{holes:?}");
        let hole = holes[0];
        assert!(
            (hole.x_px - 47.3).abs() < 0.15 && (hole.y_px - 50.8).abs() < 0.15,
            "{hole:?}"
        );
        assert!((hole.diameter_px - 13.0).abs() < 0.6, "{hole:?}");
        assert!(!hole.light && hole.split.is_none() && hole.score > 0.8);
    }

    #[test]
    fn three_touching_holes_are_split_in_three() {
        let size = 140;
        let centres = [(62.0, 64.0), (73.0, 63.0), (67.5, 73.5)];
        let mut canvas = vec![228.0f64; size * size];
        for (x, y) in centres {
            for (i, value) in canvas.iter_mut().enumerate() {
                let (px, py) = ((i % size) as f64 + 0.5, (i / size) as f64 + 0.5);
                *value += (35.0 - *value) * coverage((px - x).hypot(py - y) - 6.5);
            }
        }
        let pixels: Vec<u8> = canvas.iter().map(|v| v.round() as u8).collect();
        let holes = find_gray(size, &pixels, &HoleQuery::new(Some(13.0)));
        assert_eq!(holes.len(), 3, "{holes:?}");
        for (x, y) in centres {
            let hole = holes
                .iter()
                .find(|hole| (hole.x_px - x).hypot(hole.y_px - y) < 0.25 * 13.0);
            let hole = hole.unwrap_or_else(|| panic!("({x}, {y}) in {holes:?}"));
            assert_eq!(hole.split.map(|split| split.circles), Some(3));
        }
    }

    #[test]
    fn a_hole_in_black_is_light() {
        let pixels = disc(96, (48.0, 48.0), 12.0, 25.0, 170.0);
        let holes = find_gray(96, &pixels, &HoleQuery::new(Some(12.0)));
        assert_eq!(holes.len(), 1, "{holes:?}");
        assert!(holes[0].light);
    }

    #[test]
    fn large_holes_are_found_in_a_reduced_copy() {
        let pixels = disc(400, (201.0, 190.0), 64.0, 230.0, 30.0);
        let holes = find_gray(400, &pixels, &HoleQuery::new(Some(64.0)));
        assert_eq!(holes.len(), 1, "{holes:?}");
        assert!((holes[0].x_px - 201.0).abs() < 1.0 && (holes[0].y_px - 190.0).abs() < 1.0);
        assert!((holes[0].diameter_px - 64.0).abs() < 4.0, "{:?}", holes[0]);
    }

    #[test]
    fn marked_shots_flag_and_avoided_points_drop() {
        let pixels = disc(96, (48.0, 48.0), 12.0, 230.0, 30.0);
        let marked = [(50.0, 46.0)];
        let query = HoleQuery {
            marked: &marked,
            ..HoleQuery::new(Some(12.0))
        };
        assert!(find_gray(96, &pixels, &query)[0].near_marked);
        let far = [(70.0, 48.0)];
        let query = HoleQuery {
            marked: &far,
            ..HoleQuery::new(Some(12.0))
        };
        assert!(!find_gray(96, &pixels, &query)[0].near_marked);
        let query = HoleQuery {
            avoid: &marked,
            ..HoleQuery::new(Some(12.0))
        };
        assert!(find_gray(96, &pixels, &query).is_empty());
    }

    #[test]
    fn line_crossings_and_strokes_are_not_holes() {
        let size = 120;
        let mut canvas = vec![228.0f64; size * size];
        let dark = |_: f64, _: f64| 40.0;
        stroke(&mut canvas, size, (10.0, 60.0), (110.0, 60.0), 2.5, &dark);
        stroke(&mut canvas, size, (60.0, 10.0), (60.0, 110.0), 2.5, &dark);
        glyphs(
            &mut canvas,
            size,
            (20.0, 20.0),
            14.0,
            3.0,
            &[DIGITS[8], DIGITS[0]],
            &dark,
        );
        let pixels: Vec<u8> = canvas.iter().map(|v| *v as u8).collect();
        let holes = find_gray(size, &pixels, &HoleQuery::new(Some(12.0)));
        assert!(holes.is_empty(), "{holes:?}");
    }

    #[test]
    fn refuses_rather_than_guesses() {
        let pixels = vec![200u8; 100 * 100];
        let region = Region {
            width_px: 100,
            height_px: 100,
            pixels: Pixels::Gray(&pixels),
        };
        assert_eq!(
            find_holes(&region, &HoleQuery::new(None)),
            Err(HoleError::NoSize)
        );
        assert_eq!(
            find_holes(&region, &HoleQuery::new(Some(4.0))),
            Err(HoleError::TooSmall { need_zoom: 1.5 })
        );
        assert_eq!(
            find_holes(&region, &HoleQuery::new(Some(40.0))),
            Err(HoleError::TooLarge { need_zoom: 0.5 })
        );
        let short = Region {
            pixels: Pixels::Rgba(&pixels),
            ..region
        };
        assert_eq!(
            find_holes(&short, &HoleQuery::new(Some(10.0))),
            Err(HoleError::BadRegion)
        );
        assert_eq!(expected_diameter_px(Some(6.5), Some(0.5)), Some(13.0));
        assert_eq!(expected_diameter_px(None, Some(0.5)), None);
        assert_eq!(expected_diameter_px(Some(6.5), None), None);
        assert_eq!(expected_diameter_px(Some(6.5), Some(0.0)), None);
    }

    /// `cargo test --release -p groups-vision holes::tests::timing -- --ignored --nocapture`
    #[test]
    #[ignore = "a timing, meaningful only in a release build"]
    fn timing() {
        let layout = Layout {
            size: 1000,
            diameter: 12.0,
            singles: 40,
            pairs: 5,
            groups: 0,
            anywhere: false,
        };
        for diameter in [8.0, 12.0, 20.0, 40.0] {
            let target = render(Layout { diameter, ..layout }, PHOTO, 7);
            let start = std::time::Instant::now();
            let runs = 5;
            let mut holes = Vec::new();
            for _ in 0..runs {
                holes = target.find(&[], &[]);
            }
            let elapsed = start.elapsed() / runs;
            let tally = tally(&target, &holes);
            eprintln!(
                "1000×1000, holes {diameter} px: {elapsed:?} a run, recall {:.2}, precision {:.2}",
                tally.recall(),
                tally.precision()
            );
            assert!(elapsed.as_millis() < 200);
        }
    }
}

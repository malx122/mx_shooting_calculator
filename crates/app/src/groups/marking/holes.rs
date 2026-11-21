//! Hole suggestions on the shots step: Detect looks for holes in the part
//! of the photo on screen and shows them as dashed rings to check. The
//! ticked ones become shots in one undo step; nothing is added by itself.

use eframe::egui::{Painter, Pos2, Rect, Stroke, pos2, vec2};
use groups_core::Point;
use groups_vision::grid::InkImage;
use groups_vision::holes::{self as vision, HoleError, HoleQuery, Pixels, Region};

use super::geometry::{Camera, Space};
use super::paint;
use crate::i18n::{tr, trf};

/// A suggestion scoring under this starts unticked. Clear single holes
/// score about 0.8 and more, touching pairs about 0.7; faint or ragged
/// ones fall towards 0.25.
pub const LOW_SCORE: f64 = 0.45;
/// The most pixels searched at once: the part on screen is reduced past
/// this.
const MAX_PIXELS: f64 = 8.0e6;

/// A hole found, to be checked by the shooter.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    /// The centre in mark units (photo pixels).
    pub at: [f64; 2],
    /// The radius as measured, in display units.
    pub radius: f64,
    pub ticked: bool,
    pub near_marked: bool,
    pub score: f64,
    /// The touching holes found as one blob share a number.
    pub blob: Option<u32>,
}

/// Why no rings are shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    /// No scale or no calibre: the hole's size is unknown.
    NoSize,
    /// Holes are too small in the part searched, reduced to fit: zoom in
    /// about this much.
    ZoomIn(f64),
    /// Too few holes fit on screen.
    ZoomOut,
    /// Holes are too small in the photo's own pixels for zooming to help.
    Coarse,
    /// The group was full: this many ticked holes weren't added.
    Full {
        left_out: usize,
    },
    /// Nothing of the photo shows, or it can't be read.
    Unreadable,
    NoneFound,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::NoSize => tr("Set the scale and the bullet first").to_owned(),
            Refusal::ZoomIn(zoom) => trf!(
                "Zoom in about {}× to look for holes",
                super::tenths((zoom * 2.0).ceil().max(3.0) / 2.0)
            ),
            Refusal::ZoomOut => tr("Zoom out a little").to_owned(),
            Refusal::Coarse => tr("The photo is too coarse here to find holes").to_owned(),
            Refusal::Full { left_out } => trf!(
                "A group holds at most {} shots: {} holes weren't added.",
                ballistics_store::groups::MAX_SHOTS,
                left_out
            ),
            Refusal::Unreadable => {
                tr("This part of the photo can't be searched for holes.").to_owned()
            }
            Refusal::NoneFound => tr("No holes found here. Tap them to mark them.").to_owned(),
        }
    }
}

/// Where the search is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Search {
    #[default]
    Idle,
    /// Asked for; runs once "Looking for holes…" has been drawn.
    Looking {
        frames: u8,
    },
    Shown(Vec<Suggestion>),
    Refused(Refusal),
}

impl Search {
    pub fn ask(&mut self) {
        *self = Search::Looking { frames: 0 };
    }

    /// Whether the search should run now: its notice has been on screen
    /// for a frame.
    pub fn due(&self) -> bool {
        matches!(self, Search::Looking { frames } if *frames >= 2)
    }

    pub fn after_frame(&mut self, context: &eframe::egui::Context) {
        if let Search::Looking { frames } = self {
            *frames = frames.saturating_add(1);
            context.request_repaint();
        }
    }

    pub fn looking(&self) -> bool {
        matches!(self, Search::Looking { .. })
    }

    pub fn shown(&self) -> Option<&[Suggestion]> {
        match self {
            Search::Shown(suggestions) => Some(suggestions),
            _ => None,
        }
    }

    /// Ticks or unticks the ring under `screen`, the nearest when rings
    /// overlap; `false` when no ring is there.
    pub fn toggle_at(&mut self, screen: Pos2, space: &Space, camera: &Camera, reach: f32) -> bool {
        let Search::Shown(suggestions) = self else {
            return false;
        };
        let nearest = suggestions
            .iter()
            .enumerate()
            .filter_map(|(index, suggestion)| {
                let centre = camera.to_screen(space.to_display(suggestion.at)?);
                let radius = (suggestion.radius * camera.scale) as f32;
                let distance = (centre - screen).length();
                (distance <= (radius + 4.0).max(reach)).then_some((index, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match nearest {
            Some((index, _)) => {
                suggestions[index].ticked = !suggestions[index].ticked;
                true
            }
            None => false,
        }
    }
}

/// Whether a hole found starts ticked: not one already marked, and not a
/// doubtful one.
pub fn starts_ticked(hole: &vision::Hole) -> bool {
    !hole.near_marked && hole.score >= LOW_SCORE
}

/// The order to number points in: rows from the top, each from the left.
/// Points less than `row` lower than a row's first point join that row.
pub fn reading_order(points: &[Point], row: f64) -> Vec<usize> {
    let mut by_height: Vec<usize> = (0..points.len()).collect();
    by_height.sort_by(|&a, &b| points[a].y.total_cmp(&points[b].y));
    let mut order = Vec::with_capacity(points.len());
    let mut start = 0;
    while start < by_height.len() {
        let top = points[by_height[start]].y;
        let end = by_height[start..]
            .iter()
            .position(|&index| points[index].y - top > row)
            .map_or(by_height.len(), |offset| start + offset);
        let mut line = by_height[start..end].to_vec();
        line.sort_by(|&a, &b| points[a].x.total_cmp(&points[b].x));
        order.extend(line);
        start = end;
    }
    order
}

/// The part of the screen searched, sampled at `density` pixels a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    /// The top-left corner on screen.
    pub origin: Pos2,
    pub density: f64,
    /// The ink image's own pixels a screen point spans, at the middle.
    pub native: f64,
    pub size_px: [u32; 2],
}

impl Window {
    /// The part of `rect` where the camera shows the photo, at the finer of
    /// the screen's pixels and the photo's own (`ink_per_mark` image pixels
    /// to a photo pixel), reduced to [`MAX_PIXELS`]. `None` when nothing of
    /// the photo shows.
    pub fn over(
        space: &Space,
        camera: &Camera,
        rect: Rect,
        pixels_per_point: f32,
        ink_per_mark: [f64; 2],
    ) -> Option<Window> {
        let seen = space.extent().intersect(&camera.visible(rect))?;
        let (min, max) = (camera.to_screen(seen.min), camera.to_screen(seen.max));
        let screen = Rect::from_min_max(min, max).intersect(rect);
        if !(screen.width() >= 1.0 && screen.height() >= 1.0) {
            return None;
        }
        // The photo's own pixels a screen point spans, at the middle.
        let middle = screen.center();
        let ink = |at: Pos2| {
            let [x, y] = space.to_mark(camera.to_display(at))?;
            Some(Point::new(x * ink_per_mark[0], y * ink_per_mark[1]))
        };
        let native = match (
            ink(middle),
            ink(middle + vec2(1.0, 0.0)),
            ink(middle + vec2(0.0, 1.0)),
        ) {
            (Some(at), Some(right), Some(down)) => (right - at).length().max((down - at).length()),
            _ => 0.0,
        };
        let mut density = f64::from(pixels_per_point).max(native);
        let area = f64::from(screen.width()) * f64::from(screen.height());
        if area * density * density > MAX_PIXELS {
            density = (MAX_PIXELS / area).sqrt();
        }
        let size_px = [
            (f64::from(screen.width()) * density).floor() as u32,
            (f64::from(screen.height()) * density).floor() as u32,
        ];
        (size_px[0] > 0 && size_px[1] > 0).then_some(Window {
            origin: screen.min,
            density,
            native,
            size_px,
        })
    }

    /// The screen point at a position in the window's pixels.
    pub fn to_screen(self, [x, y]: [f64; 2]) -> Pos2 {
        pos2(
            self.origin.x + (x / self.density) as f32,
            self.origin.y + (y / self.density) as f32,
        )
    }

    /// A screen point's position in the window's pixels.
    pub fn to_window(self, at: Pos2) -> [f64; 2] {
        [
            f64::from(at.x - self.origin.x) * self.density,
            f64::from(at.y - self.origin.y) * self.density,
        ]
    }
}

/// The window's pixels from the ink image, each read at its middle through
/// the camera and the space (so a straightened view is searched straight),
/// between the image's pixels, its edges repeated beyond them.
pub fn sample(
    ink: &InkImage,
    window: &Window,
    space: &Space,
    camera: &Camera,
    ink_per_mark: [f64; 2],
) -> Vec<u8> {
    let [width, height] = window.size_px;
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    let (w, h) = (ink.width as usize, ink.height as usize);
    let read = |x: usize, y: usize| f64::from(ink.pixels[y.min(h - 1) * w + x.min(w - 1)]);
    for row in 0..height {
        for column in 0..width {
            let screen = window.to_screen([f64::from(column) + 0.5, f64::from(row) + 0.5]);
            let value = match space.to_mark(camera.to_display(screen)) {
                Some([x, y]) if w > 0 && h > 0 => {
                    let x = (x * ink_per_mark[0] - 0.5).clamp(0.0, (w - 1) as f64);
                    let y = (y * ink_per_mark[1] - 0.5).clamp(0.0, (h - 1) as f64);
                    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
                    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
                    let top = read(x0, y0) * (1.0 - fx) + read(x0 + 1, y0) * fx;
                    let bottom = read(x0, y0 + 1) * (1.0 - fx) + read(x0 + 1, y0 + 1) * fx;
                    (top * (1.0 - fy) + bottom * fy).round() as u8
                }
                _ => 255,
            };
            pixels.push(value);
        }
    }
    pixels
}

/// What a search needs from the marking view.
pub struct Look<'a> {
    pub space: &'a Space,
    pub camera: &'a Camera,
    pub rect: Rect,
    pub pixels_per_point: f32,
    pub ink: &'a InkImage,
    /// The photo's own size, of which the ink image is a reduction.
    pub photo_px: [u32; 2],
    pub calibration: Option<&'a groups_core::Calibration>,
    pub calibre_mm: Option<f64>,
    /// Shots already marked, in mark units.
    pub marked: &'a [[f64; 2]],
}

/// Looks for holes in the part of the photo on screen.
pub fn look(look: &Look) -> Result<Vec<Suggestion>, Refusal> {
    let calibre = look
        .calibre_mm
        .filter(|calibre| calibre.is_finite() && *calibre > 0.0)
        .ok_or(Refusal::NoSize)?;
    let calibration = look.calibration.ok_or(Refusal::NoSize)?;
    let ink_per_mark = [
        f64::from(look.ink.width) / f64::from(look.photo_px[0].max(1)),
        f64::from(look.ink.height) / f64::from(look.photo_px[1].max(1)),
    ];
    let (space, camera) = (look.space, look.camera);
    let window = Window::over(
        space,
        camera,
        look.rect,
        look.pixels_per_point,
        ink_per_mark,
    )
    .ok_or(Refusal::Unreadable)?;
    let middle = camera.to_display(window.to_screen([
        f64::from(window.size_px[0]) / 2.0,
        f64::from(window.size_px[1]) / 2.0,
    ]));
    let mm_per_unit = space
        .mm_per_unit(Some(calibration), middle)
        .filter(|mm| mm.is_finite() && *mm > 0.0)
        .ok_or(Refusal::NoSize)?;
    let per_unit = camera.scale * window.density;
    let diameter = vision::expected_diameter_px(Some(calibre), Some(mm_per_unit / per_unit))
        .ok_or(Refusal::NoSize)?;
    if diameter * window.native / window.density < vision::MIN_DIAMETER_PX {
        return Err(Refusal::Coarse);
    }
    let to_window = |marks: &[[f64; 2]]| -> Vec<(f64, f64)> {
        marks
            .iter()
            .filter_map(|mark| {
                let [x, y] = window.to_window(camera.to_screen(space.to_display(*mark)?));
                Some((x, y))
            })
            .collect()
    };
    let marked = to_window(look.marked);
    let pixels = sample(look.ink, &window, space, camera, ink_per_mark);
    let region = Region {
        width_px: window.size_px[0],
        height_px: window.size_px[1],
        pixels: Pixels::Gray(&pixels),
    };
    let query = HoleQuery {
        marked: &marked,
        ..HoleQuery::new(Some(diameter))
    };
    let holes = vision::find_holes(&region, &query).map_err(|error| match error {
        HoleError::NoSize => Refusal::NoSize,
        // On screen the hole is this many device pixels across.
        HoleError::TooSmall { .. } => Refusal::ZoomIn(
            vision::MIN_DIAMETER_PX * window.density
                / (diameter * f64::from(look.pixels_per_point)),
        ),
        HoleError::TooLarge { .. } => Refusal::ZoomOut,
        HoleError::BadRegion => Refusal::Unreadable,
    })?;
    let suggestions: Vec<Suggestion> = holes
        .iter()
        .filter_map(|hole| {
            let at = space.to_mark(camera.to_display(window.to_screen([hole.x_px, hole.y_px])))?;
            Some(Suggestion {
                at,
                radius: 0.5 * hole.diameter_px / per_unit,
                ticked: starts_ticked(hole),
                near_marked: hole.near_marked,
                score: hole.score,
                blob: hole.split.map(|split| split.blob),
            })
        })
        .collect();
    if suggestions.is_empty() {
        Err(Refusal::NoneFound)
    } else {
        Ok(suggestions)
    }
}

/// The rings: dashed, the ticked ones filled and checked, touching holes
/// linked.
pub fn paint(painter: &Painter, suggestions: &[Suggestion], space: &Space, camera: &Camera) {
    let placed: Vec<Option<(Pos2, f32)>> = suggestions
        .iter()
        .map(|suggestion| {
            let centre = camera.to_screen(space.to_display(suggestion.at)?);
            let radius = ((suggestion.radius * camera.scale) as f32).max(paint::MIN_RING_RADIUS);
            Some((centre, radius))
        })
        .collect();
    for (index, (a, b)) in suggestions.iter().zip(&placed).enumerate() {
        let (Some(blob), Some((from, _))) = (a.blob, b) else {
            continue;
        };
        let next = suggestions[index + 1..]
            .iter()
            .zip(&placed[index + 1..])
            .find(|(other, _)| other.blob == Some(blob));
        if let Some((_, Some((to, _)))) = next {
            paint::outlined_line(painter, &[*from, *to], paint::SUGGESTED, false, false);
        }
    }
    for (suggestion, place) in suggestions.iter().zip(placed) {
        let Some((centre, radius)) = place else {
            continue;
        };
        let (outline, line) = paint::ring_widths(radius, false);
        // A hole already marked shows faint, under its shot.
        let strength = if suggestion.near_marked { 0.5 } else { 1.0 };
        if suggestion.ticked {
            painter.circle_filled(centre, radius, paint::SUGGESTED.gamma_multiply(0.22));
        }
        painter.circle_stroke(
            centre,
            radius,
            Stroke::new(outline, paint::OUTLINE.gamma_multiply(strength)),
        );
        paint::dashed_circle(
            painter,
            centre,
            radius,
            Stroke::new(line, paint::SUGGESTED.gamma_multiply(strength)),
        );
        let badge = centre + vec2(0.71, -0.71) * (radius + 2.0);
        if suggestion.near_marked {
            painter.circle_filled(badge, 5.0, paint::OUTLINE);
            painter.circle_filled(badge, 3.5, paint::RING);
        } else if suggestion.score < LOW_SCORE {
            painter.circle_filled(badge, 8.0, paint::OUTLINE);
            painter.text(
                badge,
                eframe::egui::Align2::CENTER_CENTER,
                "?",
                eframe::egui::FontId::proportional(12.0),
                paint::SUGGESTED,
            );
        }
        if suggestion.ticked && radius >= 6.0 {
            let s = (radius * 0.45).min(7.0);
            let tick = vec![
                centre + vec2(-0.7 * s, 0.0),
                centre + vec2(-0.2 * s, 0.55 * s),
                centre + vec2(0.75 * s, -0.6 * s),
            ];
            painter.line(tick.clone(), Stroke::new(3.6, paint::OUTLINE));
            painter.line(tick, Stroke::new(1.8, eframe::egui::Color32::WHITE));
        }
    }
}

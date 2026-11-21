//! Where things are on the marking canvas.
//!
//! Three kinds of coordinates meet here:
//!
//! - **mark units**, as the marks are stored: photo pixels (y down), or a
//!   grid's millimetres from its centre (y up);
//! - **display units**, the plane the canvas shows, always with y down:
//!   photo pixels for a photo as it is, or target millimetres with y turned
//!   over for a straightened photo and for a grid;
//! - **screen points**, egui's.
//!
//! [`Space`] maps mark units to display units and back; [`Camera`] maps
//! display units to the screen.

use eframe::egui::{Pos2, Rect, Vec2, pos2};
use groups_core::Point;

/// A rectangle in display units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub fn new(min: Point, max: Point) -> Self {
        Bounds { min, max }
    }

    /// The smallest bounds holding every point; `None` for none.
    pub fn around(points: impl IntoIterator<Item = Point>) -> Option<Bounds> {
        points.into_iter().fold(None, |bounds: Option<Bounds>, p| {
            Some(match bounds {
                None => Bounds::new(p, p),
                Some(b) => Bounds::new(
                    Point::new(b.min.x.min(p.x), b.min.y.min(p.y)),
                    Point::new(b.max.x.max(p.x), b.max.y.max(p.y)),
                ),
            })
        })
    }

    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn centre(&self) -> Point {
        Point::new(
            0.5 * (self.min.x + self.max.x),
            0.5 * (self.min.y + self.max.y),
        )
    }

    pub fn expand(&self, by: f64) -> Bounds {
        Bounds::new(
            Point::new(self.min.x - by, self.min.y - by),
            Point::new(self.max.x + by, self.max.y + by),
        )
    }

    /// The overlap of the two; `None` when they don't meet.
    pub fn intersect(&self, other: &Bounds) -> Option<Bounds> {
        let min = Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y));
        let max = Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y));
        (min.x < max.x && min.y < max.y).then_some(Bounds::new(min, max))
    }

    pub fn contains(&self, p: Point) -> bool {
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }

    /// The nearest point inside.
    pub fn clamp(&self, p: Point) -> Point {
        Point::new(
            p.x.clamp(self.min.x, self.max.x),
            p.y.clamp(self.min.y, self.max.y),
        )
    }
}

/// What the canvas shows, and how marks sit on it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Space {
    /// The photo as taken: display units are its pixels.
    Photo { size_px: [f64; 2] },
    /// The photo straightened by a four-corner scale: display units are
    /// target millimetres from the rectangle's bottom-left corner with y
    /// turned down, so the rectangle spans y from −height to 0.
    Straightened {
        size_px: [f64; 2],
        calibration: groups_core::Calibration,
        rectangle_mm: [f64; 2],
    },
    /// A blank grid at true scale: display units are millimetres from its
    /// centre, y down.
    Grid { size_mm: [f64; 2] },
}

/// Target millimetres (y up) to display units (y down) and back.
fn flip(p: Point) -> Point {
    Point::new(p.x, -p.y)
}

impl Space {
    /// Where a point in mark units shows, in display units.
    pub fn to_display(self, mark: [f64; 2]) -> Option<Point> {
        let [x, y] = mark;
        let p = Point::new(x, y);
        if !p.is_finite() {
            return None;
        }
        match self {
            Space::Photo { .. } => Some(p),
            Space::Straightened { calibration, .. } => {
                calibration.to_mm(p).map(flip).filter(|p| p.is_finite())
            }
            Space::Grid { .. } => Some(flip(p)),
        }
    }

    /// The point in mark units shown at `display`.
    pub fn to_mark(self, display: Point) -> Option<[f64; 2]> {
        if !display.is_finite() {
            return None;
        }
        let p = match self {
            Space::Photo { .. } => display,
            Space::Straightened { calibration, .. } => calibration.to_px(flip(display))?,
            Space::Grid { .. } => flip(display),
        };
        p.is_finite().then_some([p.x, p.y])
    }

    /// Target millimetres (y up, in the calibration's or the grid's own
    /// frame) to display units, for drawing statistics.
    pub fn mm_to_display(
        &self,
        calibration: Option<&groups_core::Calibration>,
        mm: Point,
    ) -> Option<Point> {
        match self {
            Space::Photo { .. } => calibration?.to_px(mm),
            Space::Straightened { .. } | Space::Grid { .. } => Some(flip(mm)),
        }
    }

    /// The part of the display plane worth showing: the photo, the
    /// straightened photo (no further than its rectangle's size beyond the
    /// rectangle, where perspective runs away), or the grid.
    pub fn extent(&self) -> Bounds {
        match *self {
            Space::Photo { size_px } => {
                Bounds::new(Point::ORIGIN, Point::new(size_px[0], size_px[1]))
            }
            Space::Straightened {
                size_px,
                calibration,
                rectangle_mm: [width, height],
            } => {
                let rectangle = Bounds::new(Point::new(0.0, -height), Point::new(width, 0.0));
                let near = rectangle.expand(width.max(height));
                let [w, h] = size_px;
                let corners = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
                let photo: Option<Vec<Point>> = corners
                    .iter()
                    .map(|&corner| {
                        calibration
                            .to_mm(Point::new(corner[0], corner[1]))
                            .map(flip)
                    })
                    .collect();
                photo
                    .and_then(Bounds::around)
                    .and_then(|photo| photo.intersect(&near))
                    .unwrap_or(near)
            }
            Space::Grid { size_mm: [w, h] } => {
                Bounds::new(Point::new(-0.5 * w, -0.5 * h), Point::new(0.5 * w, 0.5 * h))
            }
        }
    }

    /// Millimetres on the target per display unit near `display`, when the
    /// scale is known.
    pub fn mm_per_unit(
        &self,
        calibration: Option<&groups_core::Calibration>,
        display: Point,
    ) -> Option<f64> {
        match self {
            Space::Photo { .. } => calibration?.mm_per_px_at(display),
            Space::Straightened { .. } | Space::Grid { .. } => Some(1.0),
        }
    }

    /// The most a view may be enlarged, in screen points per display unit:
    /// a photo pixel 12 points across, or a millimetre 40 points across.
    pub fn max_scale(&self) -> f64 {
        match self {
            Space::Photo { .. } => 12.0,
            Space::Straightened { calibration, .. } => {
                12.0 / calibration.resolution().mm_per_px.max(1e-6)
            }
            Space::Grid { .. } => 40.0,
        }
    }

    /// A point `mark` moved `step` mark units (photo pixels or grid
    /// millimetres) in the screen direction `direction`, as the canvas
    /// shows it.
    pub fn nudged(&self, mark: [f64; 2], direction: Vec2, step: f64) -> Option<[f64; 2]> {
        let at = self.to_display(mark)?;
        let length = (direction.x as f64).hypot(direction.y as f64);
        if length == 0.0 {
            return Some(mark);
        }
        let unit = Point::new(direction.x as f64 / length, direction.y as f64 / length);
        // How far a small move on the display goes in mark units.
        let probe = 1e-3 * self.extent().width().max(self.extent().height()).max(1.0);
        let moved = self.to_mark(at + unit * probe)?;
        let marks_per_unit = (moved[0] - mark[0]).hypot(moved[1] - mark[1]) / probe;
        if !(marks_per_unit.is_finite() && marks_per_unit > 0.0) {
            return None;
        }
        self.to_mark(at + unit * (step / marks_per_unit))
    }
}

/// How display units sit on the screen: `centre` shows at `screen_centre`,
/// `scale` screen points to a display unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub centre: Point,
    pub scale: f64,
    pub screen_centre: Pos2,
}

impl Camera {
    pub fn to_screen(self, display: Point) -> Pos2 {
        let offset = (display - self.centre) * self.scale;
        pos2(
            self.screen_centre.x + offset.x as f32,
            self.screen_centre.y + offset.y as f32,
        )
    }

    pub fn to_display(self, screen: Pos2) -> Point {
        let offset = screen - self.screen_centre;
        self.centre + Point::new(offset.x as f64, offset.y as f64) * (1.0 / self.scale)
    }

    /// The display units the screen rectangle shows.
    pub fn visible(&self, rect: Rect) -> Bounds {
        Bounds::new(self.to_display(rect.min), self.to_display(rect.max))
    }

    /// Moved by `delta` screen points, as a drag moves the picture.
    pub fn panned(self, delta: Vec2) -> Camera {
        Camera {
            centre: self.centre - Point::new(delta.x as f64, delta.y as f64) * (1.0 / self.scale),
            ..self
        }
    }

    /// Enlarged `factor` times, keeping what shows at `about` in place.
    pub fn zoomed(self, factor: f64, about: Pos2) -> Camera {
        let fixed = self.to_display(about);
        let scale = self.scale * factor;
        let offset = about - self.screen_centre;
        Camera {
            centre: fixed - Point::new(offset.x as f64, offset.y as f64) * (1.0 / scale),
            scale,
            ..self
        }
    }
}

/// The smallest a view may be, as a share of the whole content fitted.
pub const MIN_ZOOM: f64 = 0.5;

/// Screen points per display unit that fit `extent` into `rect`, with a
/// little room round it.
pub fn fit_scale(extent: &Bounds, rect: Rect) -> f64 {
    let width = extent.width().max(1e-9);
    let height = extent.height().max(1e-9);
    0.94 * (rect.width() as f64 / width).min(rect.height() as f64 / height)
}

/// The camera kept usable: between half the fitted size and the most
/// enlargement, its centre on the content so the content can't be lost.
pub fn clamped(camera: Camera, extent: &Bounds, fit: f64, max_scale: f64) -> Camera {
    let low = fit * MIN_ZOOM;
    let high = max_scale.max(fit);
    let scale = if camera.scale.is_finite() {
        camera.scale.clamp(low, high)
    } else {
        fit
    };
    let centre = if camera.centre.is_finite() {
        extent.clamp(camera.centre)
    } else {
        extent.centre()
    };
    Camera {
        centre,
        scale,
        screen_centre: camera.screen_centre,
    }
}

/// Where the loupe goes for a finger at `at` on `canvas`: above it, or
/// beside it when there's no room above, and always on the canvas.
pub fn loupe_rect(at: Pos2, canvas: Rect, size: f32, touch: bool) -> Rect {
    let gap = if touch { 48.0 } else { 24.0 };
    let half = 0.5 * size;
    let mut centre = pos2(at.x, at.y - gap - half);
    if centre.y - half < canvas.top() + 4.0 {
        let right = at.x + gap + size <= canvas.right() - 4.0;
        centre.x = if right {
            at.x + gap + half
        } else {
            at.x - gap - half
        };
        centre.y = at.y;
    }
    let clamp = |value: f32, low: f32, high: f32| {
        if low > high {
            0.5 * (low + high)
        } else {
            value.clamp(low, high)
        }
    };
    centre.x = clamp(
        centre.x,
        canvas.left() + half + 4.0,
        canvas.right() - half - 4.0,
    );
    centre.y = clamp(
        centre.y,
        canvas.top() + half + 4.0,
        canvas.bottom() - half - 4.0,
    );
    Rect::from_center_size(centre, eframe::egui::vec2(size, size))
}

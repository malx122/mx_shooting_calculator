//! Turning taps on a photo into millimetres on the target, and back.
//!
//! Photo pixels have y down; target millimetres have y up (see
//! [`geometry`](crate::geometry)). A calibration is a projective map
//! (a homography) between the two:
//!
//! - **Two points** a known distance apart give a uniform scale. The photo
//!   is taken as upright and square-on. The origin is the first point.
//! - **Four corners** of a rectangle of known size, tapped in any order,
//!   also straighten a photo taken at an angle. The corners are sorted as
//!   the photo shows them; the width is the side along the top and bottom
//!   edges as shown, the height the side along the left and right edges.
//!   A photo shown a quarter turn round is turned upright first, or the
//!   two sizes swapped. The origin is the rectangle's bottom-left corner
//!   as the photo shows it, x along its bottom edge to the right, y up its
//!   left edge.

use std::fmt;

use crate::geometry::{Point, centroid};

/// Reference points closer together than this (px) are refused: a tap is
/// good to about a pixel, so the scale would be several percent out.
pub const MIN_SPACING_PX: f64 = 30.0;

/// How far out a tap may be, one standard deviation per axis (px), for
/// [`Resolution::scale_uncertainty`].
pub const TAP_ERROR_PX: f64 = 1.0;

/// A scale more uncertain than this (relative) is worth a warning.
pub const SCALE_UNCERTAINTY_WARNING: f64 = 0.01;

/// A corner whose angle is within this many degrees of a straight line
/// (or of folding back on itself) counts as three corners on a line.
pub const MIN_CORNER_ANGLE_DEGREES: f64 = 3.0;

/// A photo coarser than this (mm per px, in its most stretched direction)
/// is too coarse to mark: a tap is good to about a pixel, so each shot
/// would be half a millimetre out, and a .22 hole would be barely a dozen
/// pixels across.
pub const COARSEST_MM_PER_PX: f64 = 0.5;

/// What a calibration was made from, as tapped on the photo (px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reference {
    /// Two points `distance_mm` apart.
    TwoPoints {
        a: Point,
        b: Point,
        distance_mm: f64,
    },
    /// A rectangle's corners sorted as the photo shows them: top-left,
    /// top-right, bottom-right, bottom-left. `width_mm` is the side along
    /// the top and bottom edges and `height_mm` the side along the left
    /// and right edges.
    Rectangle {
        corners: [Point; 4],
        width_mm: f64,
        height_mm: f64,
    },
}

/// Why a calibration can't be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationError {
    /// A point or a size is not a finite number.
    NotFinite,
    /// A distance, width or height is zero or negative.
    SizeNotPositive,
    /// Two points are closer than [`MIN_SPACING_PX`].
    TooClose,
    /// Three of the corners lie on (or nearly on) one line.
    Collinear,
    /// One corner lies inside the triangle of the other three.
    NotConvex,
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            CalibrationError::NotFinite => "a point or size is not a number",
            CalibrationError::SizeNotPositive => "the size must be more than zero",
            CalibrationError::TooClose => "the points are too close together",
            CalibrationError::Collinear => "three of the corners are on one line",
            CalibrationError::NotConvex => "the corners don't make a rectangle",
        })
    }
}

impl std::error::Error for CalibrationError {}

/// How finely a photo shows the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    /// Millimetres per pixel in the middle of the reference points, as
    /// the geometric mean of the two directions.
    pub mm_per_px: f64,
    /// The most millimetres one pixel spans in any direction, at the
    /// coarsest of the reference points: the far side of a photo taken at
    /// an angle, and the foreshortened direction there.
    pub coarsest_mm_per_px: f64,
    /// The scale's relative uncertainty from taps [`TAP_ERROR_PX`] out:
    /// √2 × that ÷ the distance between the two points, or ÷ the
    /// rectangle's shortest side.
    pub scale_uncertainty: f64,
}

impl Resolution {
    /// Pixels per millimetre in the middle of the reference points.
    pub fn px_per_mm(&self) -> f64 {
        1.0 / self.mm_per_px
    }

    /// Whether any part of the reference is coarser than
    /// [`COARSEST_MM_PER_PX`] in any direction, so the user should take a
    /// closer or straighter photo.
    pub fn is_too_coarse(&self) -> bool {
        self.coarsest_mm_per_px > COARSEST_MM_PER_PX
    }

    /// Whether the scale is more uncertain than
    /// [`SCALE_UNCERTAINTY_WARNING`], so the user should tap points
    /// further apart.
    pub fn is_scale_uncertain(&self) -> bool {
        self.scale_uncertainty > SCALE_UNCERTAINTY_WARNING
    }
}

/// A projective map of the plane, as a 3 × 3 matrix on (x, y, 1),
/// scaled so that w = 1 at the middle of the reference points.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Homography([[f64; 3]; 3]);

impl Homography {
    /// The image of `point`, or `None` on or beyond the vanishing line
    /// (where w ≤ 0: the point would map to infinity or behind the
    /// camera).
    fn apply(&self, point: Point) -> Option<Point> {
        let [r0, r1, r2] = self.0;
        let w = r2[0] * point.x + r2[1] * point.y + r2[2];
        if w.is_nan() || w <= 1e-9 {
            return None;
        }
        Some(Point::new(
            (r0[0] * point.x + r0[1] * point.y + r0[2]) / w,
            (r1[0] * point.x + r1[1] * point.y + r1[2]) / w,
        ))
    }

    /// The map's Jacobian at `point`: [[∂u/∂x, ∂u/∂y], [∂v/∂x, ∂v/∂y]].
    fn jacobian(&self, point: Point) -> Option<[[f64; 2]; 2]> {
        let [r0, r1, r2] = self.0;
        let w = r2[0] * point.x + r2[1] * point.y + r2[2];
        let image = self.apply(point)?;
        Some([
            [(r0[0] - image.x * r2[0]) / w, (r0[1] - image.x * r2[1]) / w],
            [(r1[0] - image.y * r2[0]) / w, (r1[1] - image.y * r2[1]) / w],
        ])
    }

    /// How much an area is scaled at `point`.
    fn area_scale(&self, point: Point) -> Option<f64> {
        let [[a, b], [c, d]] = self.jacobian(point)?;
        Some((a * d - b * c).abs())
    }

    /// The most a length is stretched at `point`, in any direction: the
    /// larger singular value of the Jacobian.
    fn largest_stretch(&self, point: Point) -> Option<f64> {
        let [[a, b], [c, d]] = self.jacobian(point)?;
        let squares = a * a + b * b + c * c + d * d;
        let determinant = a * d - b * c;
        let spread = (squares * squares - 4.0 * determinant * determinant).max(0.0);
        Some((0.5 * (squares + spread.sqrt())).sqrt())
    }

    fn inverse(&self) -> Option<Homography> {
        let m = self.0;
        let cofactor = |r0: usize, r1: usize, c0: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let adjugate = [
            [
                cofactor(1, 2, 1, 2),
                -cofactor(0, 2, 1, 2),
                cofactor(0, 1, 1, 2),
            ],
            [
                -cofactor(1, 2, 0, 2),
                cofactor(0, 2, 0, 2),
                -cofactor(0, 1, 0, 2),
            ],
            [
                cofactor(1, 2, 0, 1),
                -cofactor(0, 2, 0, 1),
                cofactor(0, 1, 0, 1),
            ],
        ];
        let determinant =
            m[0][0] * adjugate[0][0] + m[0][1] * adjugate[1][0] + m[0][2] * adjugate[2][0];
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }
        Some(Homography(adjugate))
    }

    /// The same map, scaled so that w = 1 at `point`.
    fn normalised_at(self, point: Point) -> Option<Homography> {
        let r2 = self.0[2];
        let w = r2[0] * point.x + r2[1] * point.y + r2[2];
        if w == 0.0 || !w.is_finite() {
            return None;
        }
        Some(Homography(self.0.map(|row| row.map(|value| value / w))))
    }

    fn then(&self, next: &Homography) -> Homography {
        let (a, b) = (next.0, self.0);
        let mut product = [[0.0; 3]; 3];
        for (i, row) in product.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        Homography(product)
    }

    /// The map moving `points` to have their centroid at the origin and
    /// a mean distance of √2 from it, which keeps the DLT well
    /// conditioned (Hartley's normalisation).
    fn normalising(points: &[Point]) -> Homography {
        let centre = centroid(points).unwrap_or(Point::ORIGIN);
        let mean = points.iter().map(|p| p.distance(centre)).sum::<f64>() / points.len() as f64;
        let scale = std::f64::consts::SQRT_2 / mean;
        Homography([
            [scale, 0.0, -scale * centre.x],
            [0.0, scale, -scale * centre.y],
            [0.0, 0.0, 1.0],
        ])
    }

    /// The homography taking each of `from` to the matching `to`, by the
    /// direct linear transform: eight equations in the eight unknowns
    /// left once the bottom-right entry is fixed at 1, solved by Gaussian
    /// elimination with partial pivoting.
    fn from_four(from: &[Point; 4], to: &[Point; 4]) -> Option<Homography> {
        let from_normal = Homography::normalising(from);
        let to_normal = Homography::normalising(to);
        let mut system = [[0.0; 9]; 8];
        for (k, (&p, &q)) in from.iter().zip(to).enumerate() {
            let p = from_normal.apply(p)?;
            let q = to_normal.apply(q)?;
            system[2 * k] = [p.x, p.y, 1.0, 0.0, 0.0, 0.0, -q.x * p.x, -q.x * p.y, q.x];
            system[2 * k + 1] = [0.0, 0.0, 0.0, p.x, p.y, 1.0, -q.y * p.x, -q.y * p.y, q.y];
        }
        let h = solve(system)?;
        let normalised = Homography([[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]]);
        let undo = to_normal.inverse()?;
        Some(from_normal.then(&normalised).then(&undo))
    }
}

/// Solves an 8 × 8 linear system given as rows of its augmented matrix.
fn solve(mut system: [[f64; 9]; 8]) -> Option<[f64; 8]> {
    for column in 0..8 {
        let pivot = (column..8)
            .max_by(|&a, &b| system[a][column].abs().total_cmp(&system[b][column].abs()))?;
        if system[pivot][column].abs() < 1e-12 {
            return None;
        }
        system.swap(column, pivot);
        let pivot_row = system[column];
        for row in &mut system[column + 1..] {
            let factor = row[column] / pivot_row[column];
            if factor != 0.0 {
                for (cell, &above) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                    *cell -= factor * above;
                }
            }
        }
    }
    let mut solution = [0.0; 8];
    for row in (0..8).rev() {
        let known: f64 = (row + 1..8).map(|k| system[row][k] * solution[k]).sum();
        solution[row] = (system[row][8] - known) / system[row][row];
    }
    Some(solution)
}

/// A map between photo pixels (y down) and target millimetres (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    reference: Reference,
    to_mm: Homography,
    to_px: Homography,
    /// Made by [`Calibration::sheet_corners`].
    ordered: bool,
}

impl Calibration {
    /// A uniform scale from two points `distance_mm` apart, for a photo
    /// taken square-on and upright. The first point is the origin.
    pub fn two_points(a: Point, b: Point, distance_mm: f64) -> Result<Self, CalibrationError> {
        if !a.is_finite() || !b.is_finite() || !distance_mm.is_finite() {
            return Err(CalibrationError::NotFinite);
        }
        if distance_mm <= 0.0 {
            return Err(CalibrationError::SizeNotPositive);
        }
        let span = a.distance(b);
        if span < MIN_SPACING_PX {
            return Err(CalibrationError::TooClose);
        }
        let s = distance_mm / span;
        Ok(Calibration {
            reference: Reference::TwoPoints { a, b, distance_mm },
            to_mm: Homography([[s, 0.0, -s * a.x], [0.0, -s, s * a.y], [0.0, 0.0, 1.0]]),
            to_px: Homography([[1.0 / s, 0.0, a.x], [0.0, -1.0 / s, a.y], [0.0, 0.0, 1.0]]),
            ordered: false,
        })
    }

    /// A straightening map from the four corners of a `width_mm` ×
    /// `height_mm` rectangle, tapped in any order. `width_mm` is the side
    /// along the top and bottom edges as the photo is shown, `height_mm`
    /// the side along its left and right edges, exactly as given; for a
    /// target shown a quarter turn round, turn the photo upright or swap
    /// the two. The corners are told apart for targets turned up to 45°
    /// either way.
    pub fn four_corners(
        corners: [Point; 4],
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Self, CalibrationError> {
        check_rectangle(&corners, width_mm, height_mm)?;
        Self::rectangle(sort_corners(corners)?, width_mm, height_mm, false)
    }

    /// Like [`four_corners`](Calibration::four_corners), with the corners
    /// given in order and kept as given: the rectangle's own top-left,
    /// top-right, bottom-right and bottom-left, whichever way the photo
    /// shows them. The millimetres follow the rectangle (its top is up),
    /// not the photo; `width_mm` is from its top-left to its top-right
    /// corner. For corners that something other than a tap placed, such as
    /// a printed target's marks.
    pub fn sheet_corners(
        corners: [Point; 4],
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Self, CalibrationError> {
        check_rectangle(&corners, width_mm, height_mm)?;
        check_convex(&corners)?;
        Self::rectangle(corners, width_mm, height_mm, true)
    }

    /// Whether the corners were given in the rectangle's own order
    /// ([`sheet_corners`](Calibration::sheet_corners)) rather than sorted as
    /// the photo shows them.
    pub fn is_ordered(&self) -> bool {
        self.ordered
    }

    fn rectangle(
        corners: [Point; 4],
        width_mm: f64,
        height_mm: f64,
        ordered: bool,
    ) -> Result<Self, CalibrationError> {
        let rectangle = [
            Point::new(0.0, height_mm),
            Point::new(width_mm, height_mm),
            Point::new(width_mm, 0.0),
            Point::new(0.0, 0.0),
        ];
        let middle_px = centroid(&corners).unwrap_or(Point::ORIGIN);
        let middle_mm = Point::new(width_mm / 2.0, height_mm / 2.0);
        let to_mm = Homography::from_four(&corners, &rectangle)
            .and_then(|h| h.normalised_at(middle_px))
            .ok_or(CalibrationError::Collinear)?;
        let to_px = to_mm
            .inverse()
            .and_then(|h| h.normalised_at(middle_mm))
            .ok_or(CalibrationError::Collinear)?;
        Ok(Calibration {
            reference: Reference::Rectangle {
                corners,
                width_mm,
                height_mm,
            },
            to_mm,
            to_px,
            ordered,
        })
    }

    /// What the calibration was made from.
    pub fn reference(&self) -> Reference {
        self.reference
    }

    /// The point on the target (mm, y up) shown at `px` in the photo, or
    /// `None` for a point on or past the photo's horizon, which no point
    /// of the target plane can be.
    pub fn to_mm(self, px: Point) -> Option<Point> {
        map_point(&self.to_mm, px)
    }

    /// Where the target point `mm` (y up) shows in the photo (px), or
    /// `None` when it would be behind the camera. For drawing marks and
    /// for drawing a straightened photo as a mesh of triangles.
    pub fn to_px(self, mm: Point) -> Option<Point> {
        map_point(&self.to_px, mm)
    }

    /// Millimetres per pixel near the photo point `px`: the side of the
    /// square on the target that one pixel's square there covers. A ring
    /// of a bullet's diameter is `calibre / mm_per_px_at` pixels across.
    pub fn mm_per_px_at(&self, px: Point) -> Option<f64> {
        if !px.is_finite() {
            return None;
        }
        self.to_mm.area_scale(px).map(f64::sqrt)
    }

    /// The most millimetres one pixel spans in any direction near `px`:
    /// more than [`mm_per_px_at`](Calibration::mm_per_px_at) where the
    /// photo is foreshortened.
    pub fn coarsest_mm_per_px_at(&self, px: Point) -> Option<f64> {
        if !px.is_finite() {
            return None;
        }
        self.to_mm.largest_stretch(px)
    }

    /// How finely and how surely the photo shows the target, for warning
    /// that a photo is too coarse to mark or the scale too uncertain.
    pub fn resolution(&self) -> Resolution {
        let (points, shortest_px): (Vec<Point>, f64) = match self.reference {
            Reference::TwoPoints { a, b, .. } => (vec![a, b], a.distance(b)),
            Reference::Rectangle { corners, .. } => {
                let sides = (0..4).map(|i| corners[i].distance(corners[(i + 1) % 4]));
                (corners.to_vec(), sides.fold(f64::INFINITY, f64::min))
            }
        };
        let middle = centroid(&points).unwrap_or(Point::ORIGIN);
        let coarsest = |px| self.coarsest_mm_per_px_at(px).unwrap_or(f64::INFINITY);
        Resolution {
            mm_per_px: self.mm_per_px_at(middle).unwrap_or(f64::INFINITY),
            coarsest_mm_per_px: points
                .iter()
                .chain([&middle])
                .map(|&px| coarsest(px))
                .fold(0.0, f64::max),
            scale_uncertainty: std::f64::consts::SQRT_2 * TAP_ERROR_PX / shortest_px,
        }
    }
}

fn map_point(map: &Homography, point: Point) -> Option<Point> {
    if !point.is_finite() {
        return None;
    }
    map.apply(point)
}

/// The corners in the order top-left, top-right, bottom-right,
/// bottom-left as the photo shows them (clockwise on the screen, starting
/// from the one nearest the photo's top-left).
fn sort_corners(corners: [Point; 4]) -> Result<[Point; 4], CalibrationError> {
    let middle = centroid(&corners).unwrap_or(Point::ORIGIN);
    let mut sorted = corners;
    // Pixels have y down, so a growing angle turns clockwise on screen.
    sorted.sort_by(|a, b| {
        let angle = |p: &Point| (p.y - middle.y).atan2(p.x - middle.x);
        angle(a).total_cmp(&angle(b))
    });
    check_convex(&sorted)?;
    let first = (0..4)
        .min_by(|&a, &b| {
            let rank = |i: usize| sorted[i].x + sorted[i].y;
            rank(a).total_cmp(&rank(b))
        })
        .unwrap_or(0);
    sorted.rotate_left(first);
    Ok(sorted)
}

/// Sizes and corners a rectangle can be made from: finite, positive and
/// far enough apart.
fn check_rectangle(
    corners: &[Point; 4],
    width_mm: f64,
    height_mm: f64,
) -> Result<(), CalibrationError> {
    if corners.iter().any(|corner| !corner.is_finite())
        || !width_mm.is_finite()
        || !height_mm.is_finite()
    {
        return Err(CalibrationError::NotFinite);
    }
    if width_mm <= 0.0 || height_mm <= 0.0 {
        return Err(CalibrationError::SizeNotPositive);
    }
    for (i, a) in corners.iter().enumerate() {
        if corners[i + 1..]
            .iter()
            .any(|&b| a.distance(b) < MIN_SPACING_PX)
        {
            return Err(CalibrationError::TooClose);
        }
    }
    Ok(())
}

/// The corners, in their order round the rectangle, turn the same way at
/// each corner and no three are on a line.
fn check_convex(corners: &[Point; 4]) -> Result<(), CalibrationError> {
    let min_sine = MIN_CORNER_ANGLE_DEGREES.to_radians().sin();
    let mut turns = [0.0; 4];
    for (i, turn) in turns.iter_mut().enumerate() {
        let before = corners[(i + 3) % 4];
        let corner = corners[i];
        let after = corners[(i + 1) % 4];
        let (incoming, outgoing) = (corner - before, after - corner);
        let sine = incoming.cross(outgoing) / (incoming.length() * outgoing.length());
        if sine.abs() < min_sine {
            return Err(CalibrationError::Collinear);
        }
        *turn = sine;
    }
    if turns.iter().any(|&t| t.signum() != turns[0].signum()) {
        return Err(CalibrationError::NotConvex);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 200.0;
    const HEIGHT: f64 = 150.0;

    /// A camera looking at the target from below left and at an angle:
    /// target mm (y up) to photo px (y down).
    const CAMERA: Homography = Homography([
        [10.0, 0.8, 500.0],
        [0.5, -9.0, 2500.0],
        [0.0004, -0.0003, 1.0],
    ]);

    fn photographed(mm: Point) -> Point {
        CAMERA.apply(mm).unwrap()
    }

    /// The photo turned a quarter turn clockwise, 3000 px tall before.
    fn turned(px: Point) -> Point {
        Point::new(3000.0 - px.y, px.x)
    }

    fn rectangle() -> [Point; 4] {
        [
            Point::new(0.0, 0.0),
            Point::new(WIDTH, 0.0),
            Point::new(WIDTH, HEIGHT),
            Point::new(0.0, HEIGHT),
        ]
    }

    fn target_points() -> Vec<Point> {
        let mut points = Vec::new();
        for i in 0..=8 {
            for j in 0..=6 {
                points.push(Point::new(-20.0 + 30.0 * i as f64, -15.0 + 30.0 * j as f64));
            }
        }
        points
    }

    fn assert_near(a: Point, b: Point, tolerance: f64) {
        assert!(
            a.distance(b) <= tolerance,
            "{a:?} is not within {tolerance} of {b:?}"
        );
    }

    #[test]
    fn two_points_scale_and_flip_y() {
        let calibration =
            Calibration::two_points(Point::new(100.0, 400.0), Point::new(500.0, 400.0), 100.0)
                .unwrap();
        assert_eq!(
            calibration.to_mm(Point::new(100.0, 400.0)),
            Some(Point::ORIGIN)
        );
        // 4 px per mm; up the photo is up the target.
        assert_near(
            calibration.to_mm(Point::new(140.0, 360.0)).unwrap(),
            Point::new(10.0, 10.0),
            1e-12,
        );
        assert_near(
            calibration.to_px(Point::new(-5.0, -2.5)).unwrap(),
            Point::new(80.0, 410.0),
            1e-12,
        );
        assert!((calibration.mm_per_px_at(Point::new(7.0, 9.0)).unwrap() - 0.25).abs() < 1e-15);
        let resolution = calibration.resolution();
        assert_eq!(resolution.mm_per_px, 0.25);
        assert_eq!(resolution.px_per_mm(), 4.0);
        assert!((resolution.coarsest_mm_per_px - 0.25).abs() < 1e-15);
        assert!(!resolution.is_too_coarse());
        // 400 px apart: one pixel of tap error is 0.35% of the scale.
        assert!((resolution.scale_uncertainty - 2f64.sqrt() / 400.0).abs() < 1e-15);
        assert!(!resolution.is_scale_uncertain());
        let coarse =
            Calibration::two_points(Point::new(0.0, 0.0), Point::new(30.0, 40.0), 100.0).unwrap();
        assert!(coarse.resolution().is_too_coarse());
        // 50 px apart: 2.8%, worth a warning.
        assert!(coarse.resolution().is_scale_uncertain());
    }

    #[test]
    fn two_points_refuse_bad_input() {
        let a = Point::new(10.0, 10.0);
        assert_eq!(
            Calibration::two_points(a, Point::new(39.0, 10.0), 50.0),
            Err(CalibrationError::TooClose)
        );
        assert!(Calibration::two_points(a, Point::new(40.0, 10.0), 50.0).is_ok());
        assert_eq!(
            Calibration::two_points(a, Point::new(f64::NAN, 10.0), 50.0),
            Err(CalibrationError::NotFinite)
        );
        assert_eq!(
            Calibration::two_points(a, Point::new(300.0, 10.0), f64::INFINITY),
            Err(CalibrationError::NotFinite)
        );
        assert_eq!(
            Calibration::two_points(a, Point::new(300.0, 10.0), 0.0),
            Err(CalibrationError::SizeNotPositive)
        );
        assert_eq!(
            Calibration::two_points(a, Point::new(300.0, 10.0), -4.0),
            Err(CalibrationError::SizeNotPositive)
        );
    }

    #[test]
    fn four_corners_straighten_a_photo_taken_at_an_angle() {
        let corners = rectangle().map(photographed);
        let calibration = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        let Reference::Rectangle {
            corners: sorted,
            width_mm,
            height_mm,
        } = calibration.reference()
        else {
            panic!("not a rectangle");
        };
        assert_eq!((width_mm, height_mm), (WIDTH, HEIGHT));
        // Top-left, top-right, bottom-right, bottom-left on the photo.
        assert_eq!(sorted, [corners[3], corners[2], corners[1], corners[0]]);
        for mm in target_points() {
            let px = photographed(mm);
            assert_near(calibration.to_mm(px).unwrap(), mm, 0.01);
            assert_near(calibration.to_px(mm).unwrap(), px, 1e-6);
            assert_near(
                calibration.to_px(calibration.to_mm(px).unwrap()).unwrap(),
                px,
                1e-6,
            );
        }
    }

    #[test]
    fn four_corners_take_any_tapping_order() {
        let corners = rectangle().map(photographed);
        let expected = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        let mut order = [0, 1, 2, 3];
        // Heap's algorithm: every permutation of the four.
        let mut counters = [0; 4];
        let mut seen = 1;
        let mut i = 1;
        while i < 4 {
            if counters[i] < i {
                let swap = if i % 2 == 0 { 0 } else { counters[i] };
                order.swap(swap, i);
                let tapped = order.map(|k| corners[k]);
                let calibration = Calibration::four_corners(tapped, WIDTH, HEIGHT).unwrap();
                assert_eq!(calibration.reference(), expected.reference());
                for mm in target_points() {
                    assert_near(calibration.to_mm(photographed(mm)).unwrap(), mm, 0.01);
                }
                seen += 1;
                counters[i] += 1;
                i = 1;
            } else {
                counters[i] = 0;
                i += 1;
            }
        }
        assert_eq!(seen, 24);
    }

    /// The largest error in the distance between any two of `points` (mm)
    /// after photographing them with `camera` and calibrating back.
    fn worst_distance_error(calibration: &Calibration, camera: impl Fn(Point) -> Point) -> f64 {
        let points = target_points();
        let mapped: Vec<Point> = points
            .iter()
            .map(|&mm| calibration.to_mm(camera(mm)).unwrap())
            .collect();
        let mut worst: f64 = 0.0;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let error = mapped[i].distance(mapped[j]) - points[i].distance(points[j]);
                worst = worst.max(error.abs());
            }
        }
        worst
    }

    #[test]
    fn four_corners_take_the_sizes_as_shown() {
        // The photo shown a quarter turn round: the target's height now
        // runs along the photo's top edge, so it is given as the width.
        let camera = |mm| turned(photographed(mm));
        let corners = rectangle().map(camera);
        let calibration = Calibration::four_corners(corners, HEIGHT, WIDTH).unwrap();
        let Reference::Rectangle {
            width_mm,
            height_mm,
            ..
        } = calibration.reference()
        else {
            panic!("not a rectangle");
        };
        assert_eq!((width_mm, height_mm), (HEIGHT, WIDTH));
        assert!(worst_distance_error(&calibration, camera) < 0.01);
        // Given the other way round, the sizes are taken as given, and the
        // distances come out wrong.
        let swapped = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        assert!(worst_distance_error(&swapped, camera) > 10.0);
    }

    #[test]
    fn a_near_square_target_photographed_from_the_side() {
        // 210 × 200 mm, seen from the right at 30° and turned 20°: the
        // photo shows it narrower than tall, yet the sides stay as given.
        let (w, h) = (210.0, 200.0);
        let (sin, cos) = 20f64.to_radians().sin_cos();
        let camera = |mm: Point| {
            let x = 30f64.to_radians().cos() * (mm.x - w / 2.0);
            let depth = 1.0 + 0.0015 * (mm.x - w / 2.0);
            let (x, y) = (x / depth, (mm.y - h / 2.0) / depth);
            Point::new(
                2000.0 + 10.0 * (cos * x - sin * y),
                1500.0 - 10.0 * (sin * x + cos * y),
            )
        };
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ]
        .map(camera);
        let [top_left, top_right, bottom_right, bottom_left] = sort_corners(corners).unwrap();
        let across = top_left.distance(top_right) + bottom_left.distance(bottom_right);
        let up = top_left.distance(bottom_left) + top_right.distance(bottom_right);
        assert!(across < up);
        let calibration = Calibration::four_corners(corners, w, h).unwrap();
        for i in 0..=7 {
            for j in 0..=6 {
                let mm = Point::new(30.0 * i as f64, 33.0 * j as f64);
                assert_near(calibration.to_mm(camera(mm)).unwrap(), mm, 0.01);
            }
        }
    }

    #[test]
    fn four_corners_scale_is_local() {
        let corners = rectangle().map(photographed);
        let calibration = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        for mm in target_points() {
            let px = photographed(mm);
            // The square of side h around px, mapped to the target.
            let h = 0.5;
            let a = calibration.to_mm(px + Point::new(-h, -h)).unwrap();
            let b = calibration.to_mm(px + Point::new(h, -h)).unwrap();
            let c = calibration.to_mm(px + Point::new(h, h)).unwrap();
            let d = calibration.to_mm(px + Point::new(-h, h)).unwrap();
            let area = 0.5 * ((c - a).cross(d - b)).abs();
            let expected = area.sqrt() / (2.0 * h);
            let scale = calibration.mm_per_px_at(px).unwrap();
            assert!((scale - expected).abs() < 1e-6 * expected);
        }
        let resolution = calibration.resolution();
        // About 9–10 px per mm here, coarser at some corners.
        assert!(resolution.mm_per_px > 0.09 && resolution.mm_per_px < 0.13);
        assert!(resolution.coarsest_mm_per_px > resolution.mm_per_px);
        assert!(!resolution.is_too_coarse());
        assert!(!resolution.is_scale_uncertain());
    }

    #[test]
    fn coarseness_counts_the_most_stretched_direction() {
        // 100 × 100 mm shown 4000 px wide but 100 px tall: 0.025 mm per px
        // across, 1 mm per px up; their geometric mean is a fine 0.16.
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(4000.0, 0.0),
            Point::new(4000.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let calibration = Calibration::four_corners(corners, 100.0, 100.0).unwrap();
        let resolution = calibration.resolution();
        assert!((resolution.mm_per_px - 0.158_113_883).abs() < 1e-8);
        assert!((resolution.coarsest_mm_per_px - 1.0).abs() < 1e-9);
        assert!(resolution.is_too_coarse());
        let middle = Point::new(2000.0, 50.0);
        assert!((calibration.coarsest_mm_per_px_at(middle).unwrap() - 1.0).abs() < 1e-9);
        // 100 px is the shortest side: 1.4% uncertain.
        assert!((resolution.scale_uncertainty - 2f64.sqrt() / 100.0).abs() < 1e-15);
        assert!(resolution.is_scale_uncertain());
    }

    #[test]
    fn beyond_the_horizon_has_no_target_point() {
        let corners = rectangle().map(photographed);
        let calibration = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        // The camera's vanishing line: 0.0004 x − 0.0003 y + 1 = 0 in mm.
        assert!(CAMERA.apply(Point::new(-3_000.0, 0.0)).is_none());
        assert!(calibration.to_px(Point::new(-3_000.0, 100.0)).is_none());
        assert!(calibration.to_px(Point::new(-2_000.0, 100.0)).is_some());
        assert!(calibration.to_mm(Point::new(f64::NAN, 0.0)).is_none());
    }

    #[test]
    fn four_corners_refuse_bad_shapes() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        assert!(Calibration::four_corners(square, 50.0, 50.0).is_ok());
        let mut close = square;
        close[2] = Point::new(5.0, 104.0);
        assert_eq!(
            Calibration::four_corners(close, 50.0, 50.0),
            Err(CalibrationError::TooClose)
        );
        let mut inside = square;
        inside[2] = Point::new(40.0, 20.0);
        assert_eq!(
            Calibration::four_corners(inside, 50.0, 50.0),
            Err(CalibrationError::NotConvex)
        );
        let mut straight = square;
        straight[2] = Point::new(50.0, 1.0);
        assert_eq!(
            Calibration::four_corners(straight, 50.0, 50.0),
            Err(CalibrationError::Collinear)
        );
        let line = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(200.0, 0.0),
            Point::new(300.0, 0.0),
        ];
        assert_eq!(
            Calibration::four_corners(line, 50.0, 50.0),
            Err(CalibrationError::Collinear)
        );
        let mut infinite = square;
        infinite[0] = Point::new(f64::INFINITY, 0.0);
        assert_eq!(
            Calibration::four_corners(infinite, 50.0, 50.0),
            Err(CalibrationError::NotFinite)
        );
        assert_eq!(
            Calibration::four_corners(square, 50.0, f64::NAN),
            Err(CalibrationError::NotFinite)
        );
        assert_eq!(
            Calibration::four_corners(square, 0.0, 50.0),
            Err(CalibrationError::SizeNotPositive)
        );
    }

    #[test]
    fn sheet_corners_keep_the_sheets_own_up() {
        // A 200 × 150 rectangle at 2 px/mm, upside down on the photo: its
        // top-left corner is at the photo's bottom right.
        let corners = [
            Point::new(500.0, 400.0),
            Point::new(100.0, 400.0),
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
        ];
        let sheet = Calibration::sheet_corners(corners, WIDTH, HEIGHT).unwrap();
        assert!(sheet.is_ordered());
        let top_left = sheet.to_mm(corners[0]).unwrap();
        assert!(top_left.distance(Point::new(0.0, HEIGHT)) < 1e-9);
        // Up the sheet is down the photo.
        let up = sheet.to_mm(Point::new(300.0, 230.0)).unwrap()
            - sheet.to_mm(Point::new(300.0, 250.0)).unwrap();
        assert!(up.distance(Point::new(0.0, -10.0)) < 1e-9, "{up:?}");
        let photo = Calibration::four_corners(corners, WIDTH, HEIGHT).unwrap();
        assert!(!photo.is_ordered());
        assert!(
            photo
                .to_mm(corners[0])
                .unwrap()
                .distance(Point::new(WIDTH, 0.0))
                < 1e-9
        );
        // Out of order is refused, not sorted.
        let crossed = [corners[0], corners[2], corners[1], corners[3]];
        assert_eq!(
            Calibration::sheet_corners(crossed, WIDTH, HEIGHT),
            Err(CalibrationError::NotConvex)
        );
    }
}

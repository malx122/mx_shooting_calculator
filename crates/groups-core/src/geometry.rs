//! Points on the target and in the photo.
//!
//! Two coordinate systems are used, and they share the [`Point`] type:
//!
//! - **Target millimetres**: the target plane, after the scale and any
//!   straightening. x grows to the right and y grows **up**, as on a
//!   graph. Every statistic in this crate is in these units.
//! - **Photo pixels**: x grows to the right and y grows **down**, as
//!   images are stored. Only [`Calibration`](crate::Calibration) takes or
//!   gives pixels; it converts between the two systems.
//!
//! A shot that hits above and right of the aim point therefore has an
//! impact with positive x and positive y in millimetres.

use std::ops::{Add, Mul, Neg, Sub};

/// A point or a displacement on the target (mm, y up) or in a photo
/// (px, y down); which one is said by the function that takes or gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// The length of this point taken as a displacement from the origin.
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// The distance to `other`.
    pub fn distance(self, other: Point) -> f64 {
        (self - other).length()
    }

    /// The 2-D cross product (z of the 3-D one): positive when `other` is
    /// turned anticlockwise from `self` in a y-up system.
    pub fn cross(self, other: Point) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn dot(self, other: Point) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// Whether both coordinates are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, other: Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, other: Point) -> Point {
        Point::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f64> for Point {
    type Output = Point;
    fn mul(self, factor: f64) -> Point {
        Point::new(self.x * factor, self.y * factor)
    }
}

impl Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

/// The mean of `points`, or `None` when there are none.
pub fn centroid(points: &[Point]) -> Option<Point> {
    if points.is_empty() {
        return None;
    }
    let sum = points.iter().fold(Point::ORIGIN, |sum, &point| sum + point);
    Some(sum * (1.0 / points.len() as f64))
}

/// The two points furthest apart, as their indices (the lower first) and
/// their distance, or `None` for fewer than two points. Ties keep the
/// first pair found in index order.
pub fn farthest_pair(points: &[Point]) -> Option<((usize, usize), f64)> {
    let mut best: Option<((usize, usize), f64)> = None;
    for (i, &a) in points.iter().enumerate() {
        for (j, &b) in points.iter().enumerate().skip(i + 1) {
            let distance = a.distance(b);
            if best.is_none_or(|(_, longest)| distance > longest) {
                best = Some(((i, j), distance));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centroid_and_farthest_pair() {
        let points = [
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(0.0, 4.0),
        ];
        assert_eq!(centroid(&points), Some(Point::new(1.0, 4.0 / 3.0)));
        assert_eq!(farthest_pair(&points), Some(((1, 2), 5.0)));
        assert_eq!(centroid(&[]), None);
        assert_eq!(farthest_pair(&points[..1]), None);
    }

    #[test]
    fn cross_is_anticlockwise_positive() {
        assert_eq!(Point::new(1.0, 0.0).cross(Point::new(0.0, 1.0)), 1.0);
        assert_eq!(Point::new(0.0, 1.0).cross(Point::new(1.0, 0.0)), -1.0);
    }
}

//! The chance that a shot lands in a circle or a rectangle, when shots
//! scatter as a normal distribution on the target.

use crate::distributions::{normal_cdf, normal_pdf};
use crate::geometry::Point;

/// A bivariate normal spread of shots on the target (mm, y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotModel {
    /// Where the shots centre.
    pub centre: Point,
    /// Standard deviation across, mm.
    pub sd_x: f64,
    /// Standard deviation up and down, mm.
    pub sd_y: f64,
    /// The correlation between across and up, −1 to 1: positive when the
    /// group leans from bottom-left to top-right.
    pub correlation: f64,
}

/// Standard deviations beyond which the spread is taken as empty.
const REACH: f64 = 9.0;

impl ShotModel {
    /// A round spread with standard deviation `sigma` (mm) on each axis.
    pub fn circular(centre: Point, sigma: f64) -> Self {
        ShotModel {
            centre,
            sd_x: sigma,
            sd_y: sigma,
            correlation: 0.0,
        }
    }

    fn is_circular(&self) -> bool {
        self.sd_x == self.sd_y && self.correlation == 0.0
    }

    /// Whether the centre and SDs are finite, the SDs not negative and
    /// the correlation within −1 to 1.
    pub fn is_valid(&self) -> bool {
        self.centre.is_finite()
            && [self.sd_x, self.sd_y]
                .iter()
                .all(|sd| sd.is_finite() && *sd >= 0.0)
            && (-1.0..=1.0).contains(&self.correlation)
    }

    /// The chance (0 to 1) that a shot lands within `radius` (mm) of
    /// `centre`. Exact for a round spread and a circle around the group's
    /// centre; otherwise integrated numerically, to about 1e-9. `NaN` for
    /// a model that is not [valid](ShotModel::is_valid).
    pub fn chance_in_circle(&self, centre: Point, radius: f64) -> f64 {
        if !self.is_valid() {
            return f64::NAN;
        }
        if radius.is_nan() || radius <= 0.0 || !centre.is_finite() {
            return 0.0;
        }
        if self.is_circular() && centre == self.centre {
            if self.sd_x == 0.0 {
                return 1.0;
            }
            return -(-0.5 * (radius / self.sd_x).powi(2)).exp_m1();
        }
        if radius.is_infinite() {
            return 1.0;
        }
        self.integrate(Region::Circle { centre, radius })
    }

    /// The chance (0 to 1) that a shot lands in the axis-aligned rectangle
    /// from `min` to `max` (mm; the corners may be given either way round,
    /// and may be infinite). `NaN` for a model that is not
    /// [valid](ShotModel::is_valid).
    pub fn chance_in_rectangle(&self, min: Point, max: Point) -> f64 {
        if !self.is_valid() {
            return f64::NAN;
        }
        if [min.x, min.y, max.x, max.y].iter().any(|v| v.is_nan()) {
            return 0.0;
        }
        let low = Point::new(min.x.min(max.x), min.y.min(max.y));
        let high = Point::new(min.x.max(max.x), min.y.max(max.y));
        if !(high.x > low.x && high.y > low.y) {
            return 0.0;
        }
        if self.correlation == 0.0 {
            let axis = |low: f64, high: f64, mean: f64, sd: f64| {
                if sd == 0.0 {
                    return if (low..=high).contains(&mean) {
                        1.0
                    } else {
                        0.0
                    };
                }
                normal_between((low - mean) / sd, (high - mean) / sd)
            };
            return axis(low.x, high.x, self.centre.x, self.sd_x)
                * axis(low.y, high.y, self.centre.y, self.sd_y);
        }
        self.integrate(Region::Rectangle { low, high })
    }

    /// The chance in `region`, by integrating across the axis with the
    /// larger spread, with the other axis's conditional normal in closed
    /// form.
    fn integrate(&self, region: Region) -> f64 {
        if self.sd_x < self.sd_y {
            return self.transposed().integrate(region.transposed());
        }
        let ShotModel {
            centre,
            sd_x,
            sd_y,
            correlation,
        } = *self;
        if sd_x == 0.0 {
            return if region.contains(centre) { 1.0 } else { 0.0 };
        }
        let slope = correlation * sd_y / sd_x;
        let conditional_sd = sd_y * (1.0 - correlation * correlation).sqrt();
        let (from, to) = (centre.x - REACH * sd_x, centre.x + REACH * sd_x);
        let slice = |x: f64, low: f64, high: f64| {
            let mean = centre.y + slope * (x - centre.x);
            let across = normal_pdf((x - centre.x) / sd_x) / sd_x;
            let up = if conditional_sd > 0.0 {
                normal_between(
                    (low - mean) / conditional_sd,
                    (high - mean) / conditional_sd,
                )
            } else if (low..=high).contains(&mean) {
                1.0
            } else {
                0.0
            };
            across * up
        };
        let chance = match region {
            Region::Circle { centre: c, radius } => {
                // x = c.x + r sin θ, so the half-chord is r cos θ.
                let angle = |x: f64| ((x - c.x) / radius).clamp(-1.0, 1.0).asin();
                integrate(
                    |theta| {
                        let half = radius * theta.cos();
                        slice(c.x + radius * theta.sin(), c.y - half, c.y + half) * half
                    },
                    angle(from),
                    angle(to),
                )
            }
            Region::Rectangle { low, high } => {
                integrate(|x| slice(x, low.y, high.y), low.x.max(from), high.x.min(to))
            }
        };
        chance.clamp(0.0, 1.0)
    }

    fn transposed(&self) -> ShotModel {
        ShotModel {
            centre: Point::new(self.centre.y, self.centre.x),
            sd_x: self.sd_y,
            sd_y: self.sd_x,
            correlation: self.correlation,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Region {
    Circle { centre: Point, radius: f64 },
    Rectangle { low: Point, high: Point },
}

impl Region {
    fn transposed(self) -> Region {
        let swap = |p: Point| Point::new(p.y, p.x);
        match self {
            Region::Circle { centre, radius } => Region::Circle {
                centre: swap(centre),
                radius,
            },
            Region::Rectangle { low, high } => Region::Rectangle {
                low: swap(low),
                high: swap(high),
            },
        }
    }

    fn contains(self, point: Point) -> bool {
        match self {
            Region::Circle { centre, radius } => point.distance(centre) <= radius,
            Region::Rectangle { low, high } => {
                (low.x..=high.x).contains(&point.x) && (low.y..=high.y).contains(&point.y)
            }
        }
    }
}

/// Φ(b) − Φ(a) for a ≤ b, taken in the nearer tail for precision.
fn normal_between(a: f64, b: f64) -> f64 {
    if a > 0.0 {
        normal_cdf(-a) - normal_cdf(-b)
    } else {
        normal_cdf(b) - normal_cdf(a)
    }
}

/// ∫ f from `a` to `b` by adaptive Simpson's rule on 32 starting panels;
/// `NaN` as soon as `f` or a partial sum is not finite.
fn integrate(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    const PANELS: usize = 32;
    const TOLERANCE: f64 = 1e-10;
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if b <= a {
        return 0.0;
    }
    let width = (b - a) / PANELS as f64;
    let mut total = 0.0;
    for k in 0..PANELS {
        let low = a + width * k as f64;
        let high = if k + 1 == PANELS { b } else { low + width };
        let (fa, fm, fb) = (f(low), f(0.5 * (low + high)), f(high));
        let whole = (high - low) / 6.0 * (fa + 4.0 * fm + fb);
        if !whole.is_finite() {
            return f64::NAN;
        }
        total += simpson(
            &f,
            low,
            high,
            [fa, fm, fb],
            whole,
            TOLERANCE / PANELS as f64,
            30,
        );
        if !total.is_finite() {
            return f64::NAN;
        }
    }
    total
}

fn simpson(
    f: &impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    [fa, fm, fb]: [f64; 3],
    whole: f64,
    tolerance: f64,
    depth: u32,
) -> f64 {
    let m = 0.5 * (a + b);
    let (left_m, right_m) = (0.5 * (a + m), 0.5 * (m + b));
    let (f_left, f_right) = (f(left_m), f(right_m));
    let left = (m - a) / 6.0 * (fa + 4.0 * f_left + fm);
    let right = (b - m) / 6.0 * (fm + 4.0 * f_right + fb);
    let error = left + right - whole;
    if !error.is_finite() {
        return f64::NAN;
    }
    if depth == 0 || error.abs() <= 15.0 * tolerance {
        return left + right + error / 15.0;
    }
    let first = simpson(f, a, m, [fa, f_left, fm], left, tolerance / 2.0, depth - 1);
    if first.is_nan() {
        return f64::NAN;
    }
    first
        + simpson(
            f,
            m,
            b,
            [fm, f_right, fb],
            right,
            tolerance / 2.0,
            depth - 1,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    #[test]
    fn round_spread_around_its_centre_is_rayleigh() {
        let model = ShotModel::circular(Point::new(3.0, -2.0), 4.0);
        for radius in [0.5_f64, 4.0, 4.0 * 1.177_410_022_515_475, 9.0, 30.0] {
            let exact = 1.0 - (-radius * radius / 32.0).exp();
            close(model.chance_in_circle(model.centre, radius), exact, 1e-15);
            // The numerical route agrees with the closed form.
            let region = Region::Circle {
                centre: model.centre,
                radius,
            };
            close(model.integrate(region), exact, 1e-9);
        }
        close(
            model.chance_in_circle(model.centre, 4.0 * 1.177_410_022_515_475),
            0.5,
            1e-12,
        );
        assert_eq!(model.chance_in_circle(model.centre, 0.0), 0.0);
        assert_eq!(model.chance_in_circle(model.centre, -1.0), 0.0);
    }

    #[test]
    fn offset_circle_matches_the_rice_distribution() {
        // σ = 1, offset d = 1.5, radius 2: 1 − Q₁(1.5, 2) (Marcum Q),
        // by a fine midpoint sum of the Rice density.
        let model = ShotModel::circular(Point::ORIGIN, 1.0);
        let (d, r) = (1.5_f64, 2.0_f64);
        let steps = 200_000;
        let mut expected = 0.0;
        for k in 0..steps {
            let x = r * (k as f64 + 0.5) / steps as f64;
            // I₀(x d) by its series.
            let z = x * d;
            let (mut term, mut bessel, mut j) = (1.0, 1.0, 1.0);
            while term > 1e-18 * bessel {
                term *= (z / (2.0 * j)).powi(2);
                bessel += term;
                j += 1.0;
            }
            expected += x * (-(x * x + d * d) / 2.0).exp() * bessel * r / steps as f64;
        }
        close(
            model.chance_in_circle(Point::new(d, 0.0), r),
            expected,
            1e-8,
        );
        close(
            model.chance_in_circle(Point::new(0.0, -d), r),
            expected,
            1e-8,
        );
        // Far from the group, nothing hits.
        assert!(model.chance_in_circle(Point::new(40.0, 0.0), 2.0) < 1e-12);
    }

    #[test]
    fn equal_spreads_with_a_tilt_are_not_round() {
        // σ = 2 both ways with ρ = 0.8 is the ellipse with SDs
        // 2√1.8 and 2√0.2 turned 45°; a circle round the centre sees
        // only the axes' lengths.
        let tilted = ShotModel {
            centre: Point::ORIGIN,
            sd_x: 2.0,
            sd_y: 2.0,
            correlation: 0.8,
        };
        let upright = ShotModel {
            centre: Point::ORIGIN,
            sd_x: 2.0 * 1.8f64.sqrt(),
            sd_y: 2.0 * 0.2f64.sqrt(),
            correlation: 0.0,
        };
        let radius = 2.0;
        let chance = tilted.chance_in_circle(Point::ORIGIN, radius);
        close(
            chance,
            upright.chance_in_circle(Point::ORIGIN, radius),
            1e-9,
        );
        assert!((chance - (1.0 - (-0.5f64).exp())).abs() > 0.05);
    }

    #[test]
    fn far_tails_keep_their_precision() {
        // Between 8σ and 9σ out: Φ(−8) − Φ(−9) = 6.2210e-16, which
        // Φ(9) − Φ(8) would lose to rounding.
        let model = ShotModel::circular(Point::ORIGIN, 1.0);
        let chance = model.chance_in_rectangle(
            Point::new(8.0, f64::NEG_INFINITY),
            Point::new(9.0, f64::INFINITY),
        );
        let expected = normal_cdf(-8.0) - normal_cdf(-9.0);
        close(
            expected,
            6.220_960_574_271_78e-16 - 1.128_588_405_953_03e-19,
            1e-25,
        );
        close(chance, expected, 1e-9 * expected);
    }

    #[test]
    fn rectangle_is_a_product_of_normals_without_correlation() {
        let model = ShotModel {
            centre: Point::new(1.0, 2.0),
            sd_x: 3.0,
            sd_y: 1.5,
            correlation: 0.0,
        };
        let (low, high) = (Point::new(-2.0, 0.5), Point::new(5.0, 3.0));
        let expected =
            (normal_cdf(4.0 / 3.0) - normal_cdf(-1.0)) * (normal_cdf(2.0 / 3.0) - normal_cdf(-1.0));
        close(model.chance_in_rectangle(low, high), expected, 1e-15);
        close(model.chance_in_rectangle(high, low), expected, 1e-15);
        close(
            model.integrate(Region::Rectangle { low, high }),
            expected,
            1e-9,
        );
        assert_eq!(model.chance_in_rectangle(low, Point::new(-2.0, 3.0)), 0.0);
    }

    #[test]
    fn correlated_spread_in_quadrants() {
        // P(X > 0, Y > 0) = 1/4 + asin(ρ) / 2π for a centred bivariate normal.
        for correlation in [-0.8, -0.3, 0.4, 0.95] {
            let model = ShotModel {
                centre: Point::ORIGIN,
                sd_x: 2.0,
                sd_y: 5.0,
                correlation,
            };
            let expected = 0.25 + correlation.asin() / (2.0 * std::f64::consts::PI);
            let chance = model.chance_in_rectangle(Point::ORIGIN, Point::new(100.0, 100.0));
            close(chance, expected, 1e-8);
        }
    }

    #[test]
    fn elliptical_spread_in_a_circle_is_a_chi_square_mixture() {
        // An axis-aligned ellipse: a circle round the centre of radius r
        // holds P(σx² Z₁² + σy² Z₂² ≤ r²); with σy → 0 it is the half-normal.
        let line = ShotModel {
            centre: Point::ORIGIN,
            sd_x: 2.0,
            sd_y: 0.0,
            correlation: 0.0,
        };
        for radius in [0.5, 2.0, 5.0] {
            let expected = 2.0 * normal_cdf(radius / 2.0) - 1.0;
            close(line.chance_in_circle(Point::ORIGIN, radius), expected, 1e-9);
        }
        // σx = 3, σy = 1: P(9 Z₁² + Z₂² ≤ 9) = ∫ over |z₁| < 1 of
        // P(|Z₂| ≤ 3 √(1 − z₁²)) φ(z₁) dz₁, by a fine midpoint sum.
        let ellipse = ShotModel {
            centre: Point::new(-4.0, 1.0),
            sd_x: 3.0,
            sd_y: 1.0,
            correlation: 0.0,
        };
        let steps = 100_000;
        let expected: f64 = (0..steps)
            .map(|k| {
                let z = -1.0 + 2.0 * (k as f64 + 0.5) / steps as f64;
                let half = 3.0 * (1.0 - z * z).sqrt();
                (2.0 * normal_cdf(half) - 1.0) * (-0.5 * z * z).exp()
                    / (2.0 * std::f64::consts::PI).sqrt()
                    * 2.0
                    / steps as f64
            })
            .sum();
        close(
            ellipse.chance_in_circle(ellipse.centre, 3.0),
            expected,
            1e-8,
        );
        let point = ShotModel::circular(Point::new(1.0, 1.0), 0.0);
        assert_eq!(point.chance_in_circle(Point::new(1.5, 1.0), 1.0), 1.0);
        assert_eq!(point.chance_in_circle(Point::new(3.0, 1.0), 1.0), 0.0);
        assert_eq!(
            point.chance_in_rectangle(Point::ORIGIN, Point::new(2.0, 2.0)),
            1.0
        );
    }
}

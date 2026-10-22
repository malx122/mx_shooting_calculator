//! The chance a shot lands in the zone, the furthest range where that
//! chance holds up, and the wind bracket: for the Calculate page.

use ballistics_core::dispersion::{self, Sensitivities, Spread, Uncertainty};

use crate::calculator::Aim;

/// Where a shot must land, around the aiming point, in metres (x across
/// to the right, y up).
#[derive(Clone, PartialEq)]
pub enum Zone {
    Circle(f64),
    /// A drawn hit area: the widest of the animal's zones.
    Shape(Vec<[f64; 2]>),
}

impl Zone {
    pub fn of(aim: &Aim) -> Option<Zone> {
        if let Some((_, points)) = aim.figure.as_ref().and_then(|figure| figure.zones.last()) {
            return Some(Zone::Shape(points.clone()));
        }
        aim.vital_diameter_cm.map(|cm| Zone::Circle(cm / 200.0))
    }

    pub fn contains(&self, [x, y]: [f64; 2]) -> bool {
        match self {
            Zone::Circle(radius) => x * x + y * y <= radius * radius,
            Zone::Shape(points) => ballistics_store::drawings::contains(points, [x, y]),
        }
    }

    /// How far the zone reaches across from the aiming point, on the
    /// narrower side: room for a wind error.
    pub fn half_width(&self) -> f64 {
        match self {
            Zone::Circle(radius) => *radius,
            Zone::Shape(points) => {
                let (mut left, mut right) = (f64::INFINITY, f64::INFINITY);
                for (i, &[x0, y0]) in points.iter().enumerate() {
                    let [x1, y1] = points[(i + 1) % points.len()];
                    if (y0 > 0.0) != (y1 > 0.0) {
                        let x = x0 + (x1 - x0) * (0.0 - y0) / (y1 - y0);
                        if x < 0.0 {
                            left = left.min(-x);
                        } else {
                            right = right.min(x);
                        }
                    }
                }
                left.min(right)
            }
        }
    }
}

/// What the Calculate page shows.
#[derive(Clone, Copy, PartialEq)]
pub struct Hits {
    /// The chance (0 to 1) of landing in the zone when holding as told.
    pub chance: f64,
    /// How the shots spread at the target range, metres.
    pub spread: Spread,
    /// The furthest range where the chance is at least the one asked for.
    pub limit: Option<f64>,
    pub bracket: Option<Bracket>,
}

/// Where the bullet goes across for the lowest and highest likely wind.
#[derive(Clone, Copy, PartialEq)]
pub struct Bracket {
    pub low_speed: f64,
    pub high_speed: f64,
    /// Drift in metres, to the right positive.
    pub low_drift: f64,
    pub high_drift: f64,
    /// Whether holding for the middle keeps both ends in the zone.
    pub fits: bool,
}

/// The chance at `range`, holding as told (the mean impact on the aiming
/// point).
pub fn chance_at(
    sensitivities: &Sensitivities,
    uncertainty: &Uncertainty,
    zone: &Zone,
    range: f64,
) -> Option<(f64, Spread)> {
    let spread = sensitivities.spread_at(range, uncertainty)?;
    Some((dispersion::hit_chance(spread, [0.0, 0.0], |p| zone.contains(p)), spread))
}

/// The hit chance at `range`, the furthest range with at least `needed`,
/// and the bracket for a wind of `wind` ± `wind_error` m/s.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    sensitivities: &Sensitivities,
    uncertainty: &Uncertainty,
    zone: &Zone,
    range: f64,
    needed: f64,
    max_range: f64,
    wind: f64,
    wind_error: f64,
) -> Option<Hits> {
    let (chance, spread) = chance_at(sensitivities, uncertainty, zone, range)?;
    let ok = |r: f64| chance_at(sensitivities, uncertainty, zone, r).is_some_and(|(p, _)| p >= needed);
    // Out in 25 m steps to the first range that falls short, then halve
    // the gap.
    let mut limit = None;
    let mut good = 0.0;
    let mut r = 25.0;
    while r <= max_range {
        if !ok(r) {
            let (mut low, mut high) = (good, r);
            for _ in 0..6 {
                let mid = (low + high) / 2.0;
                if ok(mid) {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            limit = (low > 0.0).then_some(low);
            break;
        }
        good = r;
        r += 25.0;
    }
    let bracket = (wind_error > 0.0).then(|| {
        let low_speed = (wind - wind_error).max(0.0);
        let high_speed = wind + wind_error;
        let low_drift = sensitivities.drift_at(range, low_speed)?;
        let high_drift = sensitivities.drift_at(range, high_speed)?;
        Some(Bracket {
            low_speed,
            high_speed,
            low_drift,
            high_drift,
            fits: (high_drift - low_drift).abs() / 2.0 <= zone.half_width(),
        })
    });
    Some(Hits {
        chance,
        spread,
        limit,
        bracket: bracket.flatten(),
    })
}

/// The uncertainties from the hunter's settings: a 5-shot group at 100 m
/// in centimetres, velocity spread, range, wind speed and direction (clock
/// hours), each about one standard deviation.
pub fn uncertainty(
    group_cm: f64,
    velocity_sd: f64,
    range_error: f64,
    wind_error: f64,
    wind_hours: f64,
) -> Uncertainty {
    Uncertainty {
        aim: Uncertainty::aim_from_group(group_cm / 100.0, 100.0),
        muzzle_velocity: velocity_sd,
        range: range_error,
        wind_speed: wind_error,
        wind_direction: wind_hours * 30.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_and_a_square_contain_and_measure() {
        let circle = Zone::Circle(0.15);
        assert!(circle.contains([0.1, 0.1]));
        assert!(!circle.contains([0.12, 0.12]));
        assert_eq!(circle.half_width(), 0.15);
        let shape = Zone::Shape(vec![[-0.2, -0.1], [0.3, -0.1], [0.3, 0.2], [-0.2, 0.2]]);
        assert!(shape.contains([0.25, 0.0]));
        assert!((shape.half_width() - 0.2).abs() < 1e-9);
    }
}

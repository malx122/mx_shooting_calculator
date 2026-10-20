//! Hunting answers built on top of the trajectory solver.

use super::solver::{self, RangeRow, Sample, Scenario, Solution};

/// Ranges over which a dead-on hold keeps the bullet inside a vital zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointBlank {
    /// First range where the bullet is inside the zone (non-zero only when
    /// the scope sits higher above the bore than the zone's radius).
    pub near_limit: f64,
    /// Furthest range of the dead-on hold.
    pub far_limit: f64,
}

/// The zero that gives the longest point-blank range, and what it gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptimalZero {
    pub zero_range: f64,
    pub peak_range: f64,
    pub point_blank: PointBlank,
    /// The bore's angle above the line of sight for this zero.
    pub elevation: f64,
}

/// How far the current zero lets you hold dead-on at a vital zone of the
/// given diameter. `None` if the bullet never enters the zone.
pub fn point_blank_with_zero(solution: &Solution, vital_diameter: f64) -> Option<PointBlank> {
    point_blank_range(&solution.samples, vital_diameter * 0.5)
}

/// Searches the zero range that maximises the point-blank range: the one
/// where the bullet's highest point is exactly the top of the vital zone.
/// Uses the shot's atmosphere, level and in calm air.
pub fn optimal_point_blank(scenario: &Scenario, vital_diameter: f64) -> Option<OptimalZero> {
    let radius = vital_diameter * 0.5;
    let fly = |elevation: f64| {
        let mut previous_drop = f64::NEG_INFINITY;
        solver::fly_level(scenario, elevation, move |sample| {
            let falling = sample.drop < previous_drop;
            previous_drop = sample.drop;
            !(falling && sample.drop < -radius)
        })
    };
    let peak = |samples: &[Sample]| {
        samples
            .iter()
            .copied()
            .max_by(|a, b| a.drop.total_cmp(&b.drop))
    };

    let (mut low, mut high) = (0.0, 0.02);
    if peak(&fly(high))?.drop < radius {
        return None;
    }
    for _ in 0..50 {
        let middle = 0.5 * (low + high);
        if peak(&fly(middle))?.drop > radius {
            high = middle;
        } else {
            low = middle;
        }
    }
    let samples = fly(low);
    let top = peak(&samples)?;
    let beyond_peak = samples.iter().position(|sample| sample.range > top.range)?;
    let zero_range = falling_crossing(&samples[beyond_peak - 1..], 0.0)?;
    Some(OptimalZero {
        zero_range,
        peak_range: top.range,
        point_blank: point_blank_range(&samples, radius)?,
        elevation: low,
    })
}

/// Where a level, calm shot with the bore `elevation` above the line of
/// sight is at `range`: above (positive) or below the line of sight. With
/// an `OptimalZero`'s elevation, that zero told at a reference range.
pub fn height_at(scenario: &Scenario, elevation: f64, range: f64) -> Option<f64> {
    let samples = solver::fly_level(scenario, elevation, |sample| sample.range < range);
    solver::interpolate(&samples, range).map(|sample| sample.drop)
}

/// A zero told the way it is shot in: how high the bullet is at a
/// reference range (say 100 m), and where it comes back down through the
/// line of sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZeroFigure {
    pub reference_range: f64,
    /// Above (positive) or below the line of sight at the reference range.
    pub height: f64,
    /// Where the falling bullet crosses the line of sight, if it does
    /// before `max_range`.
    pub far_zero: Option<f64>,
}

/// The rifle's zero told at `reference_range`, on the zero day.
pub fn zero_figure(
    scenario: &Scenario,
    reference_range: f64,
    max_range: f64,
) -> Result<ZeroFigure, solver::SolveError> {
    let samples = solver::zero_day_path(scenario, |sample| {
        sample.range < reference_range.max(max_range)
    })?;
    let height = solver::interpolate(&samples, reference_range)
        .ok_or(solver::SolveError::ZeroOutOfReach)?
        .drop;
    let peak = samples
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.drop.total_cmp(&b.1.drop))
        .map_or(0, |(index, _)| index);
    Ok(ZeroFigure {
        reference_range,
        height,
        far_zero: falling_crossing(&samples[peak..], 0.0),
    })
}

/// Furthest range at which the bullet still carries the energy and speed
/// the hunter requires. `None` means the limit lies beyond the computed path.
pub fn ethical_limit(solution: &Solution, min_energy_j: f64, min_velocity: f64) -> Option<f64> {
    let energy_velocity = (2.0 * min_energy_j.max(0.0) / solution.mass_kg).sqrt();
    let required = energy_velocity.max(min_velocity);
    let samples = &solution.samples;
    let below = samples
        .iter()
        .position(|sample| sample.velocity < required)?;
    if below == 0 {
        return Some(0.0);
    }
    let (a, b) = (samples[below - 1], samples[below]);
    Some(a.range + (b.range - a.range) * (a.velocity - required) / (a.velocity - b.velocity))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lead {
    /// How far the animal moves while the bullet is in the air.
    pub distance: f64,
    /// Lead as an angle, in radians.
    pub angle: f64,
}

/// Lead for an animal crossing at right angles to the line of fire.
pub fn moving_target_lead(row: &RangeRow, target_speed: f64) -> Lead {
    let distance = target_speed * row.time;
    Lead {
        distance,
        angle: distance.atan2(row.range),
    }
}

fn point_blank_range(samples: &[Sample], radius: f64) -> Option<PointBlank> {
    let inside = |sample: &Sample| sample.drop.abs() <= radius;
    let first_inside = samples.iter().position(inside)?;
    let near_limit = if first_inside == 0 {
        0.0
    } else {
        crossing(samples[first_inside - 1], samples[first_inside], -radius)
    };
    let far_limit = match samples[first_inside..].iter().position(|s| !inside(s)) {
        Some(offset) => {
            let (a, b) = (
                samples[first_inside + offset - 1],
                samples[first_inside + offset],
            );
            crossing(a, b, if b.drop > radius { radius } else { -radius })
        }
        None => samples.last()?.range,
    };
    Some(PointBlank {
        near_limit,
        far_limit,
    })
}

/// Range where the path first falls through `level`.
fn falling_crossing(samples: &[Sample], level: f64) -> Option<f64> {
    samples
        .windows(2)
        .find(|pair| pair[0].drop >= level && pair[1].drop < level)
        .map(|pair| crossing(pair[0], pair[1], level))
}

fn crossing(a: Sample, b: Sample, level: f64) -> f64 {
    a.range + (b.range - a.range) * (level - a.drop) / (b.drop - a.drop)
}

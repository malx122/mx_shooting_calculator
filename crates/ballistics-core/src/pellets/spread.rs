//! How a choke's pattern grows with range.
//!
//! A pattern is taken as a round normal spread about its centre, so the
//! share of pellets within a radius is 1 − exp(−r²/2σ²). That fits the
//! classic tables: Gough Thomas's share of the 30-inch pellets inside the
//! 20-inch circle, 46–62 % from cylinder to full, is within about a point
//! of it.
//!
//! The spread doesn't grow at a constant angle. A choke mostly delays the
//! start of the spread, and the pattern then opens about as fast as an
//! open barrel's. In Gough Thomas's table of percentages in the 30-inch
//! circle from 30 to 60 yards, improved cylinder is true cylinder moved 5
//! yards out, half choke improved cylinder moved 5 more, and full half
//! choke moved 5 more. So σ is σ₄₀ · x · (1 + q(x − 1)), with x the range
//! over 40 yards and q = 0.011 per point that the 40-yard percentage is
//! above 40. The 40-yard percentage is always exact. Against the whole
//! table (its rounded-up 100s read as at least 92) the error is 1.1 points
//! RMS and 2.6 at worst, where a constant angle errs by up to 8. The spread
//! it gives (the 85 % circle) matches Gough Thomas's spread table within
//! 3.1 inches at 20–40 yards. Below about 25 m it is a little tight for
//! open borings; beyond 55 m it is extrapolated, and no choke spreads wider
//! than a cylinder.

use super::{PATTERN_DISTANCE_M, PATTERN_RADIUS_M};
use crate::solver::SolveError;

/// The farthest range a pattern is worked out for.
pub const MAX_PATTERN_RANGE_M: f64 = 150.0;

/// Radius of the Nordic and German pattern test's circle: 75 cm across.
pub const NORDIC_PATTERN_RADIUS_M: f64 = 0.375;
/// Distance of the Nordic and German pattern test: 35 m.
pub const NORDIC_PATTERN_DISTANCE_M: f64 = 35.0;

/// How much later a tighter choke's pattern starts to open: per point of
/// 40-yard percentage above a true cylinder's 40.
const DELAY_PER_POINT: f64 = 0.011;
const CYLINDER_PERCENT: f64 = 40.0;

/// A choke by its name. Names map onto pattern percentages more reliably
/// than onto constrictions: the same 0.25 mm is an American improved
/// cylinder and a British quarter choke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Choke {
    /// No constriction; ★★★★★.
    Cylinder,
    /// Skeet, about 0.13 mm; ⅛.
    Skeet,
    /// About 0.25 mm in a 12 bore, as Americans make it.
    ImprovedCylinder,
    /// Quarter choke, ¼; ★★★★. In Europe often called improved cylinder.
    LightModified,
    /// Half choke, ½; ★★★.
    Modified,
    /// Three-quarter choke, ¾; ★★.
    ImprovedModified,
    /// Light full, ⅞.
    LightFull,
    /// Full choke, 1/1; ★.
    Full,
    /// Tighter than full, as turkey chokes are; its percentage is
    /// extrapolated.
    ExtraFull,
}

impl Choke {
    pub const ALL: [Choke; 9] = [
        Choke::Cylinder,
        Choke::Skeet,
        Choke::ImprovedCylinder,
        Choke::LightModified,
        Choke::Modified,
        Choke::ImprovedModified,
        Choke::LightFull,
        Choke::Full,
        Choke::ExtraFull,
    ];

    /// The choke's nominal percentage of a lead load in the 30-inch circle
    /// at 40 yards. Real guns and loads vary by several points either way;
    /// a measured pattern is better.
    pub fn nominal_percent(self) -> f64 {
        match self {
            Choke::Cylinder => 40.0,
            Choke::Skeet => 45.0,
            Choke::ImprovedCylinder => 50.0,
            Choke::LightModified => 55.0,
            Choke::Modified => 60.0,
            Choke::ImprovedModified => 65.0,
            Choke::LightFull => 67.0,
            Choke::Full => 70.0,
            Choke::ExtraFull => 75.0,
        }
    }

    /// Whether it is advised for steel shot: gunmakers and Svenska
    /// Jägareförbundet say no tighter than half choke (modified), and
    /// tighter chokes gain nothing with steel anyway. C.I.P. itself only
    /// limits high-performance steel and steel over 4 mm to under 0.5 mm of
    /// constriction.
    pub fn suits_steel(self) -> bool {
        self.nominal_percent() <= Choke::Modified.nominal_percent()
    }
}

/// The 40-yard percentage a 12-bore choke of `constriction_m` gives with
/// lead, on the American scale: 0 → 40 %, 0.25 mm → 50 %, 0.50 mm → 60 %,
/// 0.89 mm → 70 %. More constriction than that doesn't pattern tighter (it
/// can pattern more openly), so the percentage stays at 70. A 20 bore needs
/// about 10–20 % less constriction for the same pattern.
pub fn percent_from_constriction(constriction_m: f64) -> Result<f64, SolveError> {
    const SCALE: [(f64, f64); 8] = [
        (0.0, 40.0),
        (0.000_13, 45.0),
        (0.000_25, 50.0),
        (0.000_38, 55.0),
        (0.000_50, 60.0),
        (0.000_64, 65.0),
        (0.000_76, 67.0),
        (0.000_89, 70.0),
    ];
    if !(constriction_m.is_finite() && (0.0..=0.003).contains(&constriction_m)) {
        return Err(SolveError::InvalidInput(
            "choke constriction must be between 0 and 3 mm",
        ));
    }
    let upper = SCALE
        .iter()
        .position(|&(at, _)| at >= constriction_m)
        .unwrap_or(SCALE.len());
    Ok(match upper {
        0 => SCALE[0].1,
        n if n == SCALE.len() => SCALE[n - 1].1,
        n => {
            let ((c0, p0), (c1, p1)) = (SCALE[n - 1], SCALE[n]);
            p0 + (p1 - p0) * (constriction_m - c0) / (c1 - c0)
        }
    })
}

/// The 40-yard percentage of steel shot from a choke that gives
/// `lead_percent` with lead: steel patterns about 10 points tighter, but
/// gains little beyond about 72 %. An estimate; pattern the gun with the
/// load to know.
pub fn steel_percent(lead_percent: f64) -> Result<f64, SolveError> {
    check_percent(lead_percent)?;
    Ok((lead_percent + 10.0).min(lead_percent.max(72.0)))
}

/// The 40-yard percentage of a pattern that puts `percent` of its pellets in
/// the 75 cm circle at 35 m, the Nordic and German test.
pub fn percent_from_nordic(percent: f64) -> Result<f64, SolveError> {
    percent_from_test(percent, NORDIC_PATTERN_RADIUS_M, NORDIC_PATTERN_DISTANCE_M)
}

/// The 40-yard percentage of a pattern that put `percent` of its pellets in
/// a circle of `radius_m` at `range_m`: a pattern board the hunter shot and
/// counted.
pub fn percent_from_test(percent: f64, radius_m: f64, range_m: f64) -> Result<f64, SolveError> {
    check_percent(percent)?;
    share_inside(50.0, radius_m, range_m)?;
    if range_m == 0.0 {
        return Err(SolveError::InvalidInput("a pattern test needs a distance"));
    }
    let share = percent / 100.0;
    let at = |p40: f64| share_inside(p40, radius_m, range_m).unwrap_or(f64::NAN);
    // The share rises with the 40-yard percentage; bisect between the ends.
    let (mut low, mut high) = (1e-9, 100.0 - 1e-9);
    if !(at(low) <= share && share <= at(high)) {
        return Err(SolveError::InvalidInput(
            "pattern percentage is beyond what a 40-yard percentage can give",
        ));
    }
    for _ in 0..200 {
        let middle = (low + high) / 2.0;
        if at(middle) < share {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok((low + high) / 2.0)
}

/// One standard deviation, per axis, of a pattern's spread in metres at
/// `range_m`, for a choke and load that put `pattern_percent` of their
/// pellets in the 30-inch circle at 40 yards.
pub fn sigma_at(pattern_percent: f64, range_m: f64) -> Result<f64, SolveError> {
    check_percent(pattern_percent)?;
    if !(range_m.is_finite() && (0.0..=MAX_PATTERN_RANGE_M).contains(&range_m)) {
        return Err(SolveError::InvalidInput(
            "pattern range must be between 0 and 150 m",
        ));
    }
    let x = range_m / PATTERN_DISTANCE_M;
    let sigma = |percent: f64| {
        let at_40_yards = PATTERN_RADIUS_M / (-2.0 * (-percent / 100.0).ln_1p()).sqrt();
        let delay = DELAY_PER_POINT * (percent - CYLINDER_PERCENT).max(0.0);
        at_40_yards * x * (1.0 + delay * (x - 1.0))
    };
    // Far beyond the tables (about 90 m) a choke would otherwise outgrow a
    // cylinder's pattern, which no choke does.
    Ok(if pattern_percent > CYLINDER_PERCENT {
        sigma(pattern_percent).min(sigma(CYLINDER_PERCENT))
    } else {
        sigma(pattern_percent)
    })
}

/// The share (0–1) of a pattern's pellets within `radius_m` of its centre
/// at `range_m`.
pub fn share_inside(pattern_percent: f64, radius_m: f64, range_m: f64) -> Result<f64, SolveError> {
    if !(radius_m.is_finite() && radius_m >= 0.0) {
        return Err(SolveError::InvalidInput(
            "pattern circle radius must be finite and nonnegative",
        ));
    }
    let sigma = sigma_at(pattern_percent, range_m)?;
    if sigma == 0.0 {
        return Ok(1.0);
    }
    Ok(-(-(radius_m * radius_m) / (2.0 * sigma * sigma)).exp_m1())
}

/// The diameter of the circle holding `share` (0–1, ends excluded) of the
/// pellets at `range_m`. Pattern tables' "spread" is about the 85 % circle
/// at 25–40 m.
pub fn spread_diameter(pattern_percent: f64, share: f64, range_m: f64) -> Result<f64, SolveError> {
    if !(share > 0.0 && share < 1.0) {
        return Err(SolveError::InvalidInput(
            "share of the pattern must be above 0 and below 1",
        ));
    }
    Ok(2.0 * sigma_at(pattern_percent, range_m)? * (-2.0 * (-share).ln_1p()).sqrt())
}

/// `count` pellets' places across the line at `range_m`, [right, up] in
/// metres from the pattern's centre. Each pellet is drawn as in `pattern`
/// with the same seed, only spread to this range's size.
pub fn pattern_at(
    pattern_percent: f64,
    count: usize,
    seed: u64,
    range_m: f64,
) -> Result<Vec<[f64; 2]>, SolveError> {
    let sigma = sigma_at(pattern_percent, range_m)?;
    super::normal_pairs(count, seed, sigma)
}

/// A full choke's delay: its 70 % at 40 yards over a cylinder's 40.
const FULL_DELAY: f64 = DELAY_PER_POINT * 30.0;

/// How much of a pattern's spread grows with each pellet's flight time
/// rather than at a fixed angle (0–1): all of it for a full choke, whose
/// spread Compton found proportional to the flight time, none for a
/// cylinder, and the choke law's delay over a full choke's between.
pub(super) fn time_share(pattern_percent: f64) -> f64 {
    (DELAY_PER_POINT * (pattern_percent - CYLINDER_PERCENT) / FULL_DELAY).clamp(0.0, 1.0)
}

pub(super) fn check_percent(pattern_percent: f64) -> Result<(), SolveError> {
    if pattern_percent > 0.0 && pattern_percent < 100.0 {
        Ok(())
    } else {
        Err(SolveError::InvalidInput(
            "pattern percentage must be above 0 and below 100",
        ))
    }
}

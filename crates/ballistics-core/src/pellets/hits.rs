//! How many pellets reach the vitals, and the chance of enough of them.
//!
//! Each pellet of a round normal pattern lands on the vital area (taken as
//! a circle) independently, so the hits are binomial. With as many hits
//! expected as are needed, the chance of getting them is only about
//! 55–60 %: published minimum pattern counts are not sure kills. The
//! hunter's own aim error moves the pattern off the vitals; it is averaged
//! over as a round normal spread too.

use super::{Pellet, fly, sigma_at};
use crate::solver::{self, Conditions, SolveError};

/// The most hits a quarry may be said to need.
pub const MAX_HITS_NEEDED: u32 = 1_000;

/// Simpson's rule for `f` over `from..to` in an even number of `steps`.
fn simpson(from: f64, to: f64, steps: usize, f: impl Fn(f64) -> f64) -> f64 {
    let step = (to - from) / steps as f64;
    let sum: f64 = (0..=steps)
        .map(|i| {
            let weight = if i == 0 || i == steps {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            weight * f(from + i as f64 * step)
        })
        .sum();
    sum * step / 3.0
}

/// The share of a round normal spread of `sigma` per axis that falls on a
/// disc of `radius` whose centre is `offset` from the spread's centre (NaN
/// for a NaN input).
pub fn share_on_disc(sigma: f64, radius: f64, offset: f64) -> f64 {
    if sigma.is_nan() || radius.is_nan() || offset.is_nan() {
        return f64::NAN;
    }
    let offset = offset.abs();
    if radius <= 0.0 {
        return 0.0;
    }
    if sigma <= 0.0 {
        return if offset <= radius { 1.0 } else { 0.0 };
    }
    if offset == 0.0 {
        return -(-(radius * radius) / (2.0 * sigma * sigma)).exp_m1();
    }
    // The density of the distance from the disc's centre (a Rice
    // distribution), integrated by Simpson's rule out to the radius, where
    // there is any of it: within 8σ of the offset.
    let (from, to) = (
        (offset - 8.0 * sigma).max(0.0),
        (offset + 8.0 * sigma).min(radius),
    );
    if to <= from {
        return 0.0;
    }
    let variance = sigma * sigma;
    let density = |rho: f64| {
        rho / variance
            * (-(rho - offset).powi(2) / (2.0 * variance)).exp()
            * scaled_bessel_i0(rho * offset / variance)
    };
    simpson(from, to, 128, density).clamp(0.0, 1.0)
}

/// I₀(x)·e^(−x), the modified Bessel function of the first kind scaled so
/// it stays finite (Abramowitz and Stegun 9.8.1–2, error under 2·10⁻⁷).
fn scaled_bessel_i0(x: f64) -> f64 {
    let x = x.abs();
    if x <= 3.75 {
        let t = (x / 3.75).powi(2);
        let i0 = 1.0
            + t * (3.515_622_9
                + t * (3.089_942_4
                    + t * (1.206_749_2 + t * (0.265_973_2 + t * (0.036_076_8 + t * 0.004_581_3)))));
        i0 * (-x).exp()
    } else {
        let t = 3.75 / x;
        (0.398_942_28
            + t * (0.013_285_92
                + t * (0.002_253_19
                    + t * (-0.001_575_65
                        + t * (0.009_162_81
                            + t * (-0.020_577_06
                                + t * (0.026_355_37 + t * (-0.016_476_33 + t * 0.003_923_77))))))))
            / x.sqrt()
    }
}

/// The chance that at least `hits` of `pellets` land when each does,
/// independently, with the chance `share` (binomial). NaN for a NaN share.
pub fn chance_of_at_least(hits: u32, pellets: usize, share: f64) -> f64 {
    let (hits, n) = (hits as usize, pellets);
    if share.is_nan() {
        return f64::NAN;
    }
    if hits == 0 {
        return 1.0;
    }
    if hits > n || share <= 0.0 {
        return 0.0;
    }
    if share >= 1.0 {
        return 1.0;
    }
    // Sum the terms below `hits` in logarithms, so that neither (1 − p)ⁿ
    // nor the binomial coefficients overflow or vanish.
    let (log_p, log_q) = (share.ln(), (-share).ln_1p());
    let mut log_term = n as f64 * log_q;
    let (mut largest, mut scaled_sum) = (log_term, 1.0);
    for j in 1..hits {
        log_term += ((n - j + 1) as f64 / j as f64).ln() + log_p - log_q;
        if log_term > largest {
            scaled_sum = scaled_sum * (largest - log_term).exp() + 1.0;
            largest = log_term;
        } else {
            scaled_sum += (log_term - largest).exp();
        }
    }
    (1.0 - largest.exp() * scaled_sum).clamp(0.0, 1.0)
}

/// What a quarry needs from shot to be killed cleanly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quarry {
    /// The vital area as a circle: heart and lungs, or head and neck for a
    /// bird. The Swedish hunting test uses 9 cm for hare and fox and 18 cm
    /// for roe deer.
    pub vital_diameter_m: f64,
    /// Pellets needed in the vitals: Burrard and Åkerman give 3 below
    /// 0.5 kg, 4 to 1 kg and 5 above. At most `MAX_HITS_NEEDED`.
    pub min_hits: u32,
    /// The least energy a pellet needs there to count, joules: Eley's 0.68
    /// for snipe, 1.15 for grouse, 2.03 for geese.
    pub min_pellet_energy_j: f64,
}

/// The chance that at least `min_hits` of a load's `pellets` land on a
/// vital circle of `vital_diameter_m` at `range_m`, for a choke and load
/// that put `pattern_percent` in the 30-inch circle at 40 yards, when the
/// pattern's centre misses the aim point by a round normal error of
/// `aim_sigma_m` per axis there. Ignores the pellets' energy.
pub fn chance_of_hits(
    pellets: usize,
    pattern_percent: f64,
    range_m: f64,
    vital_diameter_m: f64,
    min_hits: u32,
    aim_sigma_m: f64,
) -> Result<f64, SolveError> {
    if !(vital_diameter_m.is_finite() && vital_diameter_m > 0.0 && vital_diameter_m < 2.0) {
        return Err(SolveError::InvalidInput(
            "vital diameter must be above 0 and below 2 m",
        ));
    }
    if !(aim_sigma_m.is_finite() && (0.0..=10.0).contains(&aim_sigma_m)) {
        return Err(SolveError::InvalidInput(
            "aim error must be between 0 and 10 m",
        ));
    }
    if pellets > super::MAX_PELLETS {
        return Err(SolveError::InvalidInput("too many pellets in the load"));
    }
    if min_hits > MAX_HITS_NEEDED {
        return Err(SolveError::InvalidInput(
            "a quarry can't need more than 1000 hits",
        ));
    }
    let sigma = sigma_at(pattern_percent, range_m)?;
    let radius = vital_diameter_m / 2.0;
    let at_offset =
        |offset: f64| chance_of_at_least(min_hits, pellets, share_on_disc(sigma, radius, offset));
    if aim_sigma_m == 0.0 {
        return Ok(at_offset(0.0));
    }
    // The miss distance r is Rayleigh; with u = r²/2s² its weight is e^−u.
    // Beyond 8σ of the pattern outside the vitals no pellet lands on them,
    // so the integral stops there (or where e^−u is gone).
    let reach = radius + 8.0 * sigma;
    let last_u = (reach * reach / (2.0 * aim_sigma_m * aim_sigma_m)).min(40.0);
    let chance = simpson(0.0, last_u, 256, |u| {
        (-u).exp() * at_offset(aim_sigma_m * (2.0 * u).sqrt())
    });
    Ok(chance.clamp(0.0, 1.0))
}

/// A shotgun load as it leaves the barrel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotLoad {
    /// One of its pellets.
    pub pellet: Pellet,
    /// The front pellets' velocity at the muzzle, m/s (`muzzle_velocity`
    /// finds it from a published one).
    pub muzzle_velocity: f64,
    /// The whole load's pellets.
    pub pellets: usize,
    /// This load from this choke: its percentage in the 30-inch circle at
    /// 40 yards.
    pub pattern_percent: f64,
}

/// What stops a load being enough beyond its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeLimit {
    /// The pellets have too little energy left.
    Energy,
    /// Too few pellets reach the vitals often enough.
    Hits,
}

/// How far a load is enough for a quarry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveRange {
    /// The farthest whole metre at which the load is still enough.
    pub farthest_m: f64,
    /// What ends the range beyond `farthest_m`; `None` when the load is
    /// still enough at 100 m, as far as it is checked.
    pub limit: Option<RangeLimit>,
}

/// The farthest a load is checked out to.
pub const MAX_EFFECTIVE_RANGE_M: f64 = 100.0;

/// How far, in whole metres out to 100, every pellet still has the
/// quarry's energy and the chance of its hits is at least `required_chance`
/// (0–1), with the hunter's aim error `aim_sigma_rad` (per axis, radians;
/// Kanstrup found good shots mostly miss the middle by under 30 cm) in
/// these conditions.
/// `None` if it isn't enough even at 1 m. An aim error that grows with the
/// range never makes a shot too close: the pattern's pellets crowd in
/// faster than it shrinks.
pub fn effective_range(
    load: &ShotLoad,
    quarry: &Quarry,
    conditions: &Conditions,
    aim_sigma_rad: f64,
    required_chance: f64,
) -> Result<Option<EffectiveRange>, SolveError> {
    if !(required_chance > 0.0 && required_chance < 1.0) {
        return Err(SolveError::InvalidInput(
            "required chance must be above 0 and below 1",
        ));
    }
    if !(quarry.min_pellet_energy_j.is_finite() && quarry.min_pellet_energy_j >= 0.0) {
        return Err(SolveError::InvalidInput(
            "pellet energy needed must be finite and nonnegative",
        ));
    }
    if !(aim_sigma_rad.is_finite() && (0.0..0.1).contains(&aim_sigma_rad)) {
        return Err(SolveError::InvalidInput(
            "aim error must be between 0 and 0.1 radians",
        ));
    }
    let path = fly(
        &load.pellet,
        load.muzzle_velocity,
        conditions,
        MAX_EFFECTIVE_RANGE_M,
    )?;
    let mut found: Option<EffectiveRange> = None;
    for metres in 1..=MAX_EFFECTIVE_RANGE_M as u32 {
        let range = metres as f64;
        let energy = solver::interpolate(&path, range)
            .map_or(0.0, |sample| load.pellet.energy_j(sample.velocity));
        let hits = chance_of_hits(
            load.pellets,
            load.pattern_percent,
            range,
            quarry.vital_diameter_m,
            quarry.min_hits,
            aim_sigma_rad * range,
        )? >= required_chance;
        let enough_energy = energy >= quarry.min_pellet_energy_j;
        match (&mut found, enough_energy && hits) {
            (None, true) => {
                found = Some(EffectiveRange {
                    farthest_m: range,
                    limit: None,
                })
            }
            (Some(effective), true) => effective.farthest_m = range,
            (Some(effective), false) => {
                effective.limit = Some(if enough_energy {
                    RangeLimit::Hits
                } else {
                    RangeLimit::Energy
                });
                break;
            }
            (None, false) => break,
        }
    }
    Ok(found)
}

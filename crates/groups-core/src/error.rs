//! Why an analysis of several groups or sessions can't be worked out.

use std::fmt;

use crate::group::StatsError;

/// Why a comparison, composite, zero check, ladder, temperature fit or
/// summary can't be worked out. Inputs are counted from 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnalysisError {
    /// There is nothing to analyse: no groups, sessions or values.
    Empty,
    /// One input (a group, session or rung) can't be used.
    Input { index: usize, error: StatsError },
    /// A value or setting is not a finite number.
    NotFinite,
    /// A setting must be more than zero.
    NotPositive(&'static str),
    /// A setting must not be negative.
    Negative(&'static str),
    /// Needs more of something: groups, shots, rungs or sessions.
    TooFew {
        what: &'static str,
        need: usize,
        have: usize,
    },
    /// Nothing measures the spread from shot to shot: every group has a
    /// single shot, and there are too few groups for their scatter to.
    NoSpreadEstimate,
    /// σ̂ is zero: the shots lie on top of each other.
    NoSpread,
    /// The input needs an aim point and has none.
    NoAim { index: usize },
    /// A cold-bore series takes one shot per group.
    NotOneShot { index: usize, shots: usize },
    /// One group has a distance and the other has none, so their sizes
    /// are in different units.
    MixedUnits,
    /// The sessions' temperatures span too little, °C.
    TooNarrow { need_c: f64, have_c: f64 },
    /// A session's mean velocity (m/s) is no velocity a chronograph can
    /// have read (see [`is_plausible`](crate::velocity::is_plausible)).
    ImplausibleMean { index: usize, mps: f64 },
    /// The statistics of the pooled shots can't be worked out.
    Stats(StatsError),
    /// The group has no aim point, so no impact.
    MissingAim,
    /// The group has no distance, so no angles.
    MissingDistance,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::Empty => formatter.write_str("there is nothing to analyse"),
            AnalysisError::Input { index, error } => {
                write!(formatter, "input {}: {error}", index + 1)
            }
            AnalysisError::NotFinite => formatter.write_str("a value is not a number"),
            AnalysisError::NotPositive(what) => {
                write!(formatter, "the {what} must be more than zero")
            }
            AnalysisError::Negative(what) => write!(formatter, "the {what} must not be negative"),
            AnalysisError::TooFew { what, need, have } => {
                write!(formatter, "needs {need} {what}, has {have}")
            }
            AnalysisError::NoSpreadEstimate => formatter
                .write_str("needs a group of 2 shots or more, or 3 groups, to measure the spread"),
            AnalysisError::NoSpread => formatter.write_str("the shots have no spread at all"),
            AnalysisError::NoAim { index } => {
                write!(formatter, "group {} has no aim point", index + 1)
            }
            AnalysisError::NotOneShot { index, shots } => write!(
                formatter,
                "group {} has {shots} shots; a cold-bore series takes one each",
                index + 1
            ),
            AnalysisError::MixedUnits => {
                formatter.write_str("one group has a distance and the other has none")
            }
            AnalysisError::TooNarrow { need_c, have_c } => write!(
                formatter,
                "the temperatures span {have_c:.1} °C; needs {need_c:.0} °C"
            ),
            AnalysisError::ImplausibleMean { index, mps } => write!(
                formatter,
                "session {}: a mean of {mps} m/s is not a velocity",
                index + 1
            ),
            AnalysisError::Stats(error) => write!(formatter, "the pooled shots: {error}"),
            AnalysisError::MissingAim => formatter.write_str("the group has no aim point"),
            AnalysisError::MissingDistance => formatter.write_str("the group has no distance"),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// The two-sided 95% Student's t factor for `degrees` degrees of freedom.
pub(crate) fn t95(degrees: f64) -> f64 {
    crate::distributions::t_quantile(0.5 + 0.5 * crate::group::CONFIDENCE, degrees)
}

/// The radial σ̂ pooled from groups given as (shots, σ̂): each contributes
/// 2(n − 1) degrees of freedom. `None` without a degree of freedom.
pub(crate) fn pool_sigmas(groups: impl Iterator<Item = (usize, f64)>) -> Option<crate::Sigma> {
    let (mut squares, mut degrees) = (0.0, 0.0);
    for (shots, sigma) in groups {
        if shots >= 2 {
            let df = 2.0 * (shots - 1) as f64;
            squares += df * sigma * sigma;
            degrees += df;
        }
    }
    (degrees > 0.0).then(|| crate::Sigma {
        value: (squares / degrees).sqrt(),
        degrees_of_freedom: degrees,
    })
}

/// Checks a per-group σ̂: finite and not negative, for a group of two
/// shots or more (a single shot has none, and its σ̂ is ignored).
pub(crate) fn check_sigma(index: usize, shots: usize, sigma: f64) -> Result<(), AnalysisError> {
    if shots == 0 {
        return Err(AnalysisError::Input {
            index,
            error: StatsError::NoShots,
        });
    }
    if shots >= 2 && !(sigma.is_finite() && sigma >= 0.0) {
        return Err(AnalysisError::Input {
            index,
            error: StatsError::NotFinite,
        });
    }
    Ok(())
}

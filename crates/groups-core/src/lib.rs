//! Shot-group analysis: calibrating a target photo, the statistics of a
//! group, and comparing and pooling groups.
//!
//! Shots are points in millimetres on the target plane, x to the right
//! and y up; photos are in pixels with y down. [`Calibration`] converts
//! between the two, [`GroupStats`] measures a group, [`flier_check`]
//! suggests a flier, and [`VelocityStats`] and [`VelocityVertical`] look
//! at the chronograph's readings. Several groups are compared and ranked
//! ([`mod@compare`]), pooled ([`Composite`]), checked against the zero
//! ([`ZeroCheck`]), read as a load ladder ([`Ladder`]) or fitted against
//! powder temperature ([`TemperatureSensitivity`]); [`BoxPlot`] summarises
//! values for trend charts. `docs/group-statistics.md` sets out every
//! definition and formula.

pub mod angles;
pub mod calibration;
pub mod compare;
pub mod composite;
pub mod distributions;
pub mod error;
pub mod geometry;
pub mod group;
pub mod hit;
pub mod ladder;
pub mod summary;
pub mod temperature;
pub mod velocity;
pub mod zero;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

pub use calibration::{Calibration, CalibrationError, Reference, Resolution};
pub use compare::{
    RankBy, RankedGroup, Ranking, SigmaComparison, SigmaVerdict, Unrankable, UnrankedGroup,
    compare, compare_groups, rank,
};
pub use composite::{Composite, CompositeFigures, CompositeMode, ShotGroup};
pub use error::AnalysisError;
pub use geometry::Point;
pub use group::{
    Estimate, ExtremeSpread, Flier, FlierTest, GroupStats, Interval, Options, RadiusMethod,
    Scatter, Shape, Sigma, Spread, StatsError, flier_check,
};
pub use hit::ShotModel;
pub use ladder::{
    Ladder, LadderGroup, LadderKey, LadderOptions, LadderRun, LadderRung, VelocitySummary,
};
pub use summary::BoxPlot;
pub use temperature::{TemperatureSensitivity, TemperatureSession};
pub use velocity::{VelocityStats, VelocityVertical, VelocityWarning, velocity_warnings};
pub use zero::{ZeroAxis, ZeroCheck, ZeroGroup, ZeroVerdict};

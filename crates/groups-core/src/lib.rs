//! Shot-group analysis: calibrating a target photo, the statistics of a
//! group, and comparing and pooling groups.
//!
//! Shots are points in millimetres on the target plane, x to the right
//! and y up; photos are in pixels with y down. [`Calibration`] converts
//! between the two, [`GroupStats`] measures a group, [`flier_check`]
//! suggests a flier, and [`VelocityStats`] and [`VelocityVertical`] look
//! at the chronograph's readings. `docs/group-statistics.md` sets out
//! every definition and formula.

pub mod angles;
pub mod calibration;
pub mod distributions;
pub mod geometry;
pub mod group;
pub mod hit;
pub mod velocity;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

pub use calibration::{Calibration, CalibrationError, Reference, Resolution};
pub use geometry::Point;
pub use group::{
    Estimate, ExtremeSpread, Flier, FlierTest, GroupStats, Interval, Options, RadiusMethod,
    Scatter, Shape, Sigma, Spread, StatsError, flier_check,
};
pub use hit::ShotModel;
pub use velocity::{VelocityStats, VelocityVertical, VelocityWarning, velocity_warnings};

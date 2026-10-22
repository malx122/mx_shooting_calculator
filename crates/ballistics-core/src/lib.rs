//! Exterior ballistics for hunting rifles: a point-mass trajectory solver
//! with standard drag tables, real atmosphere, wind, shot angle, spin drift
//! and Coriolis, plus hunting analyses on top of it.

pub mod analysis;
pub mod atmosphere;
pub mod dispersion;
pub mod drag;
pub mod solver;
pub mod stability;

#[cfg(test)]
mod tests;

pub use atmosphere::Atmosphere;
pub use drag::{DragCurve, DragModel};
pub use solver::{
    BcBands, Bullet, Conditions, Coriolis, Load, RangeRow, Rifle, Scenario, Solution, SolveError,
    Wind,
};

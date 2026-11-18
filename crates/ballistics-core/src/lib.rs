//! Exterior ballistics for hunting rifles and shotguns: a point-mass
//! trajectory solver with standard drag tables, real atmosphere, wind, shot
//! angle, spin drift and Coriolis, hunting analyses on top of it, and
//! shotgun pellets and patterns.

pub mod analysis;
pub mod atmosphere;
pub mod dispersion;
pub mod drag;
pub mod pellets;
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

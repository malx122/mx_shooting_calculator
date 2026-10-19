//! Storage for the ballistic calculator: the read-only base catalog of
//! calibers and bullets, the hunter's own rifles and bullet collection in a
//! local SQLite database, and JSON export/import of that personal data.
//!
//! Nothing here depends on the UI, so a mobile front end can reuse it.

pub mod catalog;
pub mod db;
pub mod model;
pub mod optics;
pub mod strelok;
pub mod targets;
pub mod transfer;

#[cfg(test)]
mod tests;

use std::fmt;

pub use catalog::{Caliber, Catalog, CatalogBullet, CatalogKind};
pub use db::Store;
pub use model::{
    BcBand, BcModel, Bullet, BulletKind, ClickUnit, Construction, DragPoint, LoadZero, Rifle,
    RifleLoad, TableColumns, TableSettings, VelocityModel, VelocityPoint, ZeroWeather,
};
pub use optics::{
    AngleUnit, FocalPlane, PLAIN_CROSSHAIR, Reticle, ReticleElement, Scope, UserScope,
};
pub use targets::{Silhouette, Target, TargetKind, TargetShape};
pub use transfer::ImportReport;

pub const GRAMS_PER_GRAIN: f64 = 0.064_798_91;

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Json(serde_json::Error),
    /// Data that breaks a rule, with a message for the user.
    Invalid(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Database(error) => write!(formatter, "database error: {error}"),
            StoreError::Json(error) => write!(formatter, "could not read the file: {error}"),
            StoreError::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Database(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        StoreError::Json(error)
    }
}

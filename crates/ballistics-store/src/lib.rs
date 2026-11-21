//! Storage for the ballistic calculator: the read-only base catalog of
//! calibers and bullets, the hunter's own rifles, bullet collection, shot
//! log and shot groups in a local SQLite database, and JSON export/import
//! of that personal data. Photo bytes are kept apart (see `photos`).
//!
//! Nothing here depends on the UI, so a mobile front end can reuse it.

pub mod barrels;
pub mod bundle;
pub mod cartridge;
pub mod catalog;
pub mod chrono;
pub mod db;
pub mod deletions;
pub mod drawings;
pub mod groups;
pub mod model;
pub mod optics;
pub mod photos;
pub mod print;
pub mod rules;
pub mod schema;
pub mod sessions;
pub mod shots;
pub mod strelok;
pub mod sun;
pub mod sync;
pub mod targets;
pub mod transfer;

#[cfg(test)]
mod tests;

use std::fmt;

pub use barrels::{Barrel, Clean, RoundCounts, RoundLogEntry, ZeroReset};
pub use cartridge::{Cartridge, Engraving, Material, Outline, Solid};
pub use catalog::{BcSource, Caliber, Catalog, CatalogBullet, CatalogKind, Popularity, Ranked};
pub use db::{GroupsData, Store};
pub use deletions::{Deletion, RecordKind};
pub use drawings::{Body, Drawing, DrawingPart, HitZone, Placement, Ring, TrainingRings};
pub use groups::{
    Analysis, AnalysisKind, Calibration, GridMarks, GridShot, Group, GroupView, Ladder,
    LadderVariable, Marks, PhotoMarks, PhotoShot,
};
pub use model::{
    BcBand, BcModel, Bullet, BulletKind, ClickUnit, Construction, DataAttribution, DragPoint,
    LoadZero, Recipe, Rifle, RifleLoad, ShotStats, TableColumns, TableSettings, Truing,
    TruingMethod, VelocityModel, VelocityPoint, ZeroWeather,
};
pub use optics::{
    AngleUnit, FocalPlane, PLAIN_CROSSHAIR, Reticle, ReticleElement, Scope, UserScope,
};
pub use photos::{MemoryPhotos, Pending, Photo, PhotoError, PhotoStore, photo_id};
pub use print::Print;
pub use rules::{Country, Load as RulesLoad, Rules};
pub use sessions::{Conditions, Session};
pub use shots::{
    AnimalSpot, GeoPoint, Outcome, ShooterPosition, Shot, ShotHold, ShotSolution, ShotWeather,
};
pub use sync::{SyncKey, SyncRound};
pub use targets::{Target, TargetGroup, TargetKind, TargetShape};
pub use transfer::ImportReport;

pub const GRAMS_PER_GRAIN: f64 = 0.064_798_91;

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Json(serde_json::Error),
    /// Data that breaks a rule, with a message for the user.
    Invalid(String),
    /// The database was written by a newer version of the app (its schema
    /// version): only an update can open it.
    Newer(i64),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Database(error) => write!(formatter, "database error: {error}"),
            StoreError::Json(error) => write!(formatter, "could not read the file: {error}"),
            StoreError::Invalid(message) => formatter.write_str(message),
            StoreError::Newer(schema) => write!(
                formatter,
                "the database was created by a newer version of the app (schema {schema}): \
                 update this program"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

impl StoreError {
    /// The file is damaged or isn't a database at all: no retry will open it.
    pub fn is_corrupt(&self) -> bool {
        matches!(
            self,
            StoreError::Database(rusqlite::Error::SqliteFailure(error, _))
                if matches!(
                    error.code,
                    rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
                )
        )
    }
}

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

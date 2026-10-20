//! Deleted records. A deletion is remembered (as a "tombstone") so that a
//! merge with another device's copy deletes the record there too, instead
//! of bringing it back.

use serde::{Deserialize, Serialize};

/// The kinds of the hunter's records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Rifle,
    Bullet,
    RifleLoad,
    Scope,
    Target,
}

impl RecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RecordKind::Rifle => "rifle",
            RecordKind::Bullet => "bullet",
            RecordKind::RifleLoad => "rifle_load",
            RecordKind::Scope => "scope",
            RecordKind::Target => "target",
        }
    }

    pub fn parse(text: &str) -> Option<RecordKind> {
        [
            RecordKind::Rifle,
            RecordKind::Bullet,
            RecordKind::RifleLoad,
            RecordKind::Scope,
            RecordKind::Target,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == text)
    }

    /// The table holding records of this kind.
    pub(crate) fn table(self) -> &'static str {
        match self {
            RecordKind::Rifle => "rifles",
            RecordKind::Bullet => "bullets",
            RecordKind::RifleLoad => "rifle_loads",
            RecordKind::Scope => "scopes",
            RecordKind::Target => "targets",
        }
    }
}

/// A record that was deleted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Deletion {
    /// What kind of record it was.
    pub kind: RecordKind,
    /// The deleted record's id.
    pub id: String,
    /// When it was deleted, in milliseconds since 1970 (UTC).
    pub deleted_at: i64,
}

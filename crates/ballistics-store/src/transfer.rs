//! JSON export and import of the hunter's personal data, for backups and for
//! moving data between devices. Import merges record by record:
//!
//! - a record that is new is added;
//! - one identical to the local copy (apart from its timestamp) is left
//!   alone, so importing the same file twice changes nothing;
//! - otherwise whichever copy was edited last wins.
//!
//! This is the same rule a later sync will use. Other importers (Strelok)
//! build the same `PersonalData` and go through `import_data`.

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::StoreError;
use crate::db::{
    Store, bullet_from_row, from_json_column, load_from_row, put_bullet, put_load, put_rifle,
    put_scope, put_target, rifle_from_row,
};
use crate::model::{Bullet, Rifle, RifleLoad, now_ms};
use crate::optics::UserScope;
use crate::targets::Target;

pub const FORMAT: &str = "mxsc/personal-data";
/// Backups made under the app's earlier names: Malx Hunting Calculator and,
/// before that, Ballistic Calculator. They still import.
const LEGACY_FORMATS: [&str; 2] = ["mhc/personal-data", "ballistic-calculator/personal-data"];
/// 2: rifle loads hold a list of velocities, an impact offset and zero
/// weather; rifles and bullets gained fields with defaults.
/// 3: custom scopes, and bullets with their own drag curve.
/// 4: custom targets.
pub const VERSION: u32 = 4;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersonalData {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    pub rifles: Vec<Rifle>,
    pub bullets: Vec<Bullet>,
    pub rifle_loads: Vec<RifleLoad>,
    #[serde(default)]
    pub scopes: Vec<UserScope>,
    #[serde(default)]
    pub targets: Vec<Target>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    /// Records identical to the local copy.
    pub unchanged: usize,
    /// Records where the local copy was edited more recently.
    pub kept_local: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Merge {
    Added,
    Updated,
    Unchanged,
    KeptLocal,
}

impl Merge {
    fn writes(self) -> bool {
        matches!(self, Merge::Added | Merge::Updated)
    }
}

impl ImportReport {
    fn count(&mut self, merge: Merge) {
        match merge {
            Merge::Added => self.added += 1,
            Merge::Updated => self.updated += 1,
            Merge::Unchanged => self.unchanged += 1,
            Merge::KeptLocal => self.kept_local += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.added + self.updated + self.unchanged + self.kept_local
    }
}

impl Store {
    pub fn export_json(&self) -> Result<String, StoreError> {
        let data = PersonalData {
            format: FORMAT.to_owned(),
            version: VERSION,
            exported_at: now_ms(),
            rifles: self.rifles()?,
            bullets: self.bullets()?,
            rifle_loads: self.rifle_loads()?,
            scopes: self.scopes()?,
            targets: self.targets()?,
        };
        Ok(serde_json::to_string_pretty(&data)?)
    }

    /// Merges an export into this database. Either the whole file is
    /// imported or, if any record is invalid, nothing is.
    pub fn import_json(&mut self, json: &str) -> Result<ImportReport, StoreError> {
        let data = read_export(json)?;
        self.import_data(&data, false)
    }

    /// Merges records into this database, all or nothing. With `dry_run`
    /// nothing is kept: the report says what an import would do.
    pub fn import_data(
        &mut self,
        data: &PersonalData,
        dry_run: bool,
    ) -> Result<ImportReport, StoreError> {
        let transaction = self.connection().transaction()?;
        let mut report = ImportReport::default();
        for scope in &data.scopes {
            scope.validate().map_err(StoreError::Invalid)?;
            let local = transaction
                .query_row(
                    "SELECT data FROM scopes WHERE id = ?1",
                    [&scope.id],
                    |row| from_json_column::<UserScope>(&row.get::<_, String>(0)?),
                )
                .optional()?;
            let merge = decide(local.as_ref(), scope, |s| s.updated_at);
            if merge.writes() {
                put_scope(&transaction, scope)?;
            }
            report.count(merge);
        }
        for target in &data.targets {
            target.validate().map_err(StoreError::Invalid)?;
            let local = transaction
                .query_row(
                    "SELECT data FROM targets WHERE id = ?1",
                    [&target.id],
                    |row| from_json_column::<Target>(&row.get::<_, String>(0)?),
                )
                .optional()?;
            let merge = decide(local.as_ref(), target, |t| t.updated_at);
            if merge.writes() {
                put_target(&transaction, target)?;
            }
            report.count(merge);
        }
        for rifle in &data.rifles {
            rifle.validate().map_err(StoreError::Invalid)?;
            let local = transaction
                .query_row(
                    "SELECT * FROM rifles WHERE id = ?1",
                    [&rifle.id],
                    rifle_from_row,
                )
                .optional()?;
            let merge = decide(local.as_ref(), rifle, |r| r.updated_at);
            if merge.writes() {
                put_rifle(&transaction, rifle)?;
            }
            report.count(merge);
        }
        for bullet in &data.bullets {
            bullet.validate().map_err(StoreError::Invalid)?;
            let local = transaction
                .query_row(
                    "SELECT * FROM bullets WHERE id = ?1",
                    [&bullet.id],
                    bullet_from_row,
                )
                .optional()?;
            let merge = decide(local.as_ref(), bullet, |b| b.updated_at);
            if merge.writes() {
                put_bullet(&transaction, bullet)?;
            }
            report.count(merge);
        }
        for load in &data.rifle_loads {
            load.validate().map_err(StoreError::Invalid)?;
            report.count(import_load(&transaction, load)?);
        }
        if dry_run {
            transaction.rollback()?;
        } else {
            transaction.commit()?;
        }
        Ok(report)
    }
}

/// Whether two records hold the same data, ignoring when they were saved.
/// Reads one of this app's exports, upgrading older formats.
pub fn read_export(json: &str) -> Result<PersonalData, StoreError> {
    let mut data: PersonalData = serde_json::from_str(json)?;
    data.rifle_loads
        .iter_mut()
        .for_each(RifleLoad::upgrade_legacy);
    if data.format != FORMAT && !LEGACY_FORMATS.contains(&data.format.as_str()) {
        return Err(StoreError::Invalid(
            "this is not an MX Shooting Calculator export".into(),
        ));
    }
    if data.version > VERSION {
        return Err(StoreError::Invalid(format!(
            "the file comes from a newer version of the app (format {})",
            data.version
        )));
    }
    Ok(data)
}

fn same_content<T: Serialize>(a: &T, b: &T) -> bool {
    let strip = |record: &T| {
        let mut value = serde_json::to_value(record).ok();
        if let Some(serde_json::Value::Object(fields)) = &mut value {
            fields.remove("updated_at");
        }
        value
    };
    strip(a) == strip(b)
}

fn decide<T: Serialize>(local: Option<&T>, incoming: &T, updated_at: impl Fn(&T) -> i64) -> Merge {
    match local {
        None => Merge::Added,
        Some(local) if same_content(local, incoming) => Merge::Unchanged,
        Some(local) if updated_at(incoming) > updated_at(local) => Merge::Updated,
        Some(_) => Merge::KeptLocal,
    }
}

/// A load is identified by its rifle and bullet as well as its id, since
/// two devices may each have measured the same pair.
fn import_load(transaction: &Transaction<'_>, load: &RifleLoad) -> Result<Merge, StoreError> {
    let exists = |table: &str, id: &str| -> Result<bool, StoreError> {
        Ok(transaction
            .query_row(
                &format!("SELECT 1 FROM {table} WHERE id = ?1"),
                [id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    };
    if !exists("rifles", &load.rifle_id)? || !exists("bullets", &load.bullet_id)? {
        return Err(StoreError::Invalid(
            "a measured velocity refers to a rifle or bullet missing from the file".into(),
        ));
    }
    let local = transaction
        .query_row(
            "SELECT * FROM rifle_loads WHERE rifle_id = ?1 AND bullet_id = ?2",
            params![load.rifle_id, load.bullet_id],
            load_from_row,
        )
        .optional()?;
    // Compare as if the local copy had the incoming id: the pair is the
    // identity that matters.
    let comparable = local.clone().map(|mut local| {
        local.id = load.id.clone();
        local
    });
    let merge = decide(comparable.as_ref(), load, |l| l.updated_at);
    if let (Merge::Updated, Some(local)) = (merge, &local) {
        transaction.execute("DELETE FROM rifle_loads WHERE id = ?1", [&local.id])?;
    }
    if merge.writes() {
        put_load(transaction, load)?;
    }
    Ok(merge)
}

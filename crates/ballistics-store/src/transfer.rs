//! JSON export and import of the hunter's personal data, for backups, for
//! moving data between devices and for sync. Import merges record by
//! record:
//!
//! - a record that is new is added;
//! - one identical to the local copy (apart from its timestamp) is left
//!   alone, so importing the same file twice changes nothing;
//! - otherwise whichever copy was edited last wins; on equal times the
//!   copies are compared, so every device picks the same one.
//!
//! Deletions travel too. In a sync a deletion and an edit of the same record
//! compete by time; importing a backup file brings back records deleted
//! since (that is what restoring a backup is for). Other importers
//! (Strelok) build the same `PersonalData` and go through `import_data`.

use std::cmp::Ordering;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::StoreError;
use crate::db::{
    Store, bullet_from_row, from_json_column, load_from_row, observe_on, put_bullet, put_load,
    put_rifle, put_scope, put_target, record_deletion, rifle_from_row, stamp_on,
};
use crate::deletions::{Deletion, RecordKind};
use crate::model::{Bullet, Rifle, RifleLoad, now_ms};
use crate::optics::UserScope;
use crate::targets::Target;

pub const FORMAT: &str = "mxsc/personal-data";
/// The demo data, also the development sample.
const DEMO_DATA: &str = include_str!("../../../dev-data/sample.json");
/// Backups made under the app's earlier names: Malx Hunting Calculator and,
/// before that, Ballistic Calculator. They still import.
const LEGACY_FORMATS: [&str; 2] = ["mhc/personal-data", "ballistic-calculator/personal-data"];
/// 2: rifle loads hold a list of velocities, an impact offset and zero
/// weather; rifles and bullets gained fields with defaults.
/// 3: custom scopes, and bullets with their own drag curve.
/// 4: custom targets.
/// 5: deleted records.
pub const VERSION: u32 = 5;

// This and the types in it are the documented export format: changing
// them changes docs/export-format.schema.json and the field reference in
// docs/export-format.md, which a test then asks to regenerate
// (`MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format`).
// Their doc comments are the published descriptions. Check the page's
// hand-written parts too, and raise `VERSION` when older apps couldn't read
// the new files.

/// A file of the hunter's own records: an export, a backup or a sync copy.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PersonalData {
    /// Always `mxsc/personal-data`. Files from the app's earlier names say
    /// `mhc/personal-data` or `ballistic-calculator/personal-data`.
    pub format: String,
    /// The format's version; this description is of version 5. A reader
    /// should refuse versions newer than it knows.
    pub version: u32,
    /// When the file was written, in milliseconds since 1970 (UTC).
    pub exported_at: i64,
    /// The hunter's rifles.
    pub rifles: Vec<Rifle>,
    /// The hunter's bullets: factory ammunition and handloads.
    pub bullets: Vec<Bullet>,
    /// What is particular to a bullet in a rifle: measured velocities,
    /// point of impact, zero weather and truing.
    pub rifle_loads: Vec<RifleLoad>,
    /// Scopes the hunter entered (catalog scopes are only named by id).
    #[serde(default)]
    pub scopes: Vec<UserScope>,
    /// Targets the hunter made or copied (catalog targets aren't included).
    #[serde(default)]
    pub targets: Vec<Target>,
    /// Records deleted, so merging the file deletes them elsewhere too.
    #[serde(default)]
    pub deleted: Vec<Deletion>,
    /// The catalog entries the records refer to by id, written out so the
    /// file can be read without the app. Only in exported files; importing
    /// ignores it.
    #[serde(default, skip_serializing_if = "CatalogDetails::is_empty")]
    pub catalog: CatalogDetails,
}

/// What the records name from the app's built-in catalog.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatalogDetails {
    /// The calibers named by rifles and bullets (their `caliber`).
    #[serde(default)]
    pub calibers: Vec<CaliberDetails>,
    /// The catalog scopes named by rifles (their `scope_id`).
    #[serde(default)]
    pub scopes: Vec<ScopeDetails>,
    /// The catalog reticles named by rifles and scopes (their `reticle_id`).
    #[serde(default)]
    pub reticles: Vec<ReticleDetails>,
}

impl CatalogDetails {
    pub fn is_empty(&self) -> bool {
        self.calibers.is_empty() && self.scopes.is_empty() && self.reticles.is_empty()
    }

    /// The catalog entries `data` refers to.
    pub fn for_data(data: &PersonalData) -> CatalogDetails {
        let catalog = crate::Catalog::builtin();
        let mut calibers: Vec<&str> = data
            .rifles
            .iter()
            .map(|rifle| rifle.caliber.as_str())
            .chain(data.bullets.iter().map(|bullet| bullet.caliber.as_str()))
            .collect();
        calibers.sort_unstable();
        calibers.dedup();
        let mut scopes: Vec<&str> = data
            .rifles
            .iter()
            .filter_map(|rifle| rifle.scope_id.as_deref())
            .collect();
        scopes.sort_unstable();
        scopes.dedup();
        let mut reticles: Vec<&str> = data
            .rifles
            .iter()
            .filter_map(|rifle| rifle.reticle_id.as_deref())
            .chain(
                data.scopes
                    .iter()
                    .filter_map(|scope| scope.reticle_id.as_deref()),
            )
            .collect();
        reticles.sort_unstable();
        reticles.dedup();
        CatalogDetails {
            calibers: calibers
                .into_iter()
                .filter_map(|key| catalog.caliber(key))
                .map(|caliber| CaliberDetails {
                    key: caliber.key.clone(),
                    name: caliber.name.clone(),
                    bullet_diameter_mm: caliber.bullet_diameter_mm,
                })
                .collect(),
            scopes: scopes
                .into_iter()
                .filter_map(|id| catalog.scope(id))
                .map(|scope| ScopeDetails {
                    id: scope.id.clone(),
                    manufacturer: scope.manufacturer.clone(),
                    model: scope.model.clone(),
                    magnification_min: scope.magnification_min,
                    magnification_max: scope.magnification_max,
                    objective_mm: scope.objective_mm,
                    tube_mm: scope.tube_mm,
                    focal_plane: scope.focal_plane,
                    reticle_reference_magnification: scope.reticle_reference_magnification,
                    click_unit: scope.click_unit,
                    click_value: scope.click_value,
                    elevation_travel: scope.elevation_travel,
                    fov_m_at_100m: scope.fov_m_at_100m,
                    illuminated: scope.illuminated,
                    source_url: scope.source_url.clone(),
                })
                .collect(),
            reticles: reticles
                .into_iter()
                .filter_map(|id| Some((id, catalog.reticle(id)?)))
                .map(|(id, reticle)| ReticleDetails {
                    id: id.to_owned(),
                    manufacturer: reticle.manufacturer.clone(),
                    name: reticle.name.clone(),
                    maker_reticle_name: reticle.maker_reticle_name.clone(),
                    unit: reticle.unit,
                    source_url: reticle.source_url.clone(),
                })
                .collect(),
        }
    }
}

/// A caliber, as a rifle's or bullet's `caliber` names it.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CaliberDetails {
    /// The key records use, e.g. `308-win`.
    pub key: String,
    /// Its name, e.g. `.308 Win`.
    pub name: String,
    /// The bullet diameter the caliber takes, mm.
    pub bullet_diameter_mm: f64,
}

/// A catalog scope, as a rifle's `scope_id` names it.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScopeDetails {
    /// The id a rifle's `scope_id` uses.
    pub id: String,
    /// The maker.
    pub manufacturer: String,
    /// The model.
    pub model: String,
    /// Lowest magnification.
    pub magnification_min: f64,
    /// Highest magnification.
    pub magnification_max: f64,
    /// Objective lens diameter, mm.
    pub objective_mm: f64,
    /// Tube diameter, mm.
    pub tube_mm: Option<f64>,
    /// Where the reticle sits.
    pub focal_plane: crate::optics::FocalPlane,
    /// For second-focal-plane scopes: the magnification at which the
    /// reticle's published dimensions are true.
    pub reticle_reference_magnification: Option<f64>,
    /// The unit of its clicks.
    pub click_unit: crate::optics::AngleUnit,
    /// Per click, in `click_unit`; `null` when the maker doesn't publish it.
    pub click_value: Option<f64>,
    /// Total elevation adjustment, in `click_unit`.
    pub elevation_travel: Option<f64>,
    /// Field of view in metres at 100 m, at the lowest and the highest
    /// magnification.
    pub fov_m_at_100m: Option<[f64; 2]>,
    /// Whether the reticle can be lit.
    pub illuminated: bool,
    /// Where the data was taken from.
    pub source_url: String,
}

/// A catalog reticle, as a `reticle_id` names it.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReticleDetails {
    /// The id the record uses.
    pub id: String,
    /// The maker.
    pub manufacturer: String,
    /// The app's name for it, with scope and unit.
    pub name: String,
    /// The maker's own name for the reticle.
    pub maker_reticle_name: String,
    /// The unit its marks are drawn in.
    pub unit: crate::optics::AngleUnit,
    /// Where its dimensions were taken from.
    pub source_url: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    /// Records identical to the local copy.
    pub unchanged: usize,
    /// Records where the local copy was edited (or deleted) more recently.
    pub kept_local: usize,
    /// Local records removed because the other copy deleted them later.
    pub deleted: usize,
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
        self.added + self.updated + self.unchanged + self.kept_local + self.deleted
    }

    /// Whether the merge changed anything on this device.
    pub fn changed(&self) -> bool {
        self.added + self.updated + self.deleted > 0
    }
}

/// How a merge treats a record this device has deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// A backup file: its records come back.
    File,
    /// Another device's copy: the later of the edit and the deletion wins.
    Sync,
}

/// What every kind of record has for merging.
trait Record: Serialize + Clone {
    const KIND: RecordKind;
    fn id(&self) -> &str;
    fn updated_at(&self) -> i64;
    fn set_updated_at(&mut self, at: i64);
}

macro_rules! record {
    ($type:ty, $kind:expr) => {
        impl Record for $type {
            const KIND: RecordKind = $kind;
            fn id(&self) -> &str {
                &self.id
            }
            fn updated_at(&self) -> i64 {
                self.updated_at
            }
            fn set_updated_at(&mut self, at: i64) {
                self.updated_at = at;
            }
        }
    };
}

record!(Rifle, RecordKind::Rifle);
record!(Bullet, RecordKind::Bullet);
record!(RifleLoad, RecordKind::RifleLoad);
record!(UserScope, RecordKind::Scope);
record!(Target, RecordKind::Target);

impl Store {
    /// Everything the hunter has made, with the deletions, as one export.
    pub fn export_data(&self) -> Result<PersonalData, StoreError> {
        Ok(PersonalData {
            format: FORMAT.to_owned(),
            version: VERSION,
            exported_at: now_ms(),
            rifles: self.rifles()?,
            bullets: self.bullets()?,
            rifle_loads: self.rifle_loads()?,
            scopes: self.scopes()?,
            targets: self.targets()?,
            deleted: self.deletions()?,
            // Sync copies stay small: only exported files carry it.
            catalog: CatalogDetails::default(),
        })
    }

    /// The export file: the records, with the catalog entries they name
    /// written out so other programs can read it (see docs/export-format.md).
    pub fn export_json(&self) -> Result<String, StoreError> {
        let mut data = self.export_data()?;
        data.catalog = CatalogDetails::for_data(&data);
        Ok(serde_json::to_string_pretty(&data)?)
    }

    /// Loads the demo data: a few rifles with bullets, loads, a scope and a
    /// target, for trying the app. Like any import, loading it again adds
    /// nothing and keeps demo records the hunter has changed.
    pub fn load_demo(&mut self) -> Result<ImportReport, StoreError> {
        self.import_json(DEMO_DATA)
    }

    /// Merges an export into this database. Either the whole file is
    /// imported or, if any record is invalid, nothing is.
    pub fn import_json(&mut self, json: &str) -> Result<ImportReport, StoreError> {
        let data = read_export(json)?;
        self.import_data(&data, false)
    }

    /// Merges records from a file into this database, all or nothing. With
    /// `dry_run` nothing is kept: the report says what an import would do.
    pub fn import_data(
        &mut self,
        data: &PersonalData,
        dry_run: bool,
    ) -> Result<ImportReport, StoreError> {
        self.merge(data, Mode::File, dry_run)
    }

    /// Merges another device's copy into this database, all or nothing.
    pub fn merge_sync(&mut self, data: &PersonalData) -> Result<ImportReport, StoreError> {
        self.merge(data, Mode::Sync, false)
    }

    fn merge(
        &mut self,
        data: &PersonalData,
        mode: Mode,
        dry_run: bool,
    ) -> Result<ImportReport, StoreError> {
        let data = &without_future(data);
        let transaction = self.connection().transaction()?;
        let mut report = ImportReport::default();
        let mut latest = 0;
        for deletion in &data.deleted {
            latest = latest.max(deletion.deleted_at);
            apply_deletion(&transaction, deletion, &mut report)?;
        }
        for scope in &data.scopes {
            scope.validate().map_err(StoreError::Invalid)?;
            latest = latest.max(scope.updated_at);
            merge_record(
                &transaction,
                scope,
                mode,
                &mut report,
                put_scope,
                |tx, s| {
                    Ok(tx
                        .query_row("SELECT data FROM scopes WHERE id = ?1", [&s.id], |row| {
                            from_json_column::<UserScope>(&row.get::<_, String>(0)?)
                        })
                        .optional()?)
                },
            )?;
        }
        for target in &data.targets {
            target.validate().map_err(StoreError::Invalid)?;
            latest = latest.max(target.updated_at);
            merge_record(
                &transaction,
                target,
                mode,
                &mut report,
                put_target,
                |tx, t| {
                    Ok(tx
                        .query_row("SELECT data FROM targets WHERE id = ?1", [&t.id], |row| {
                            from_json_column::<Target>(&row.get::<_, String>(0)?)
                        })
                        .optional()?)
                },
            )?;
        }
        for rifle in &data.rifles {
            rifle.validate().map_err(StoreError::Invalid)?;
            latest = latest.max(rifle.updated_at);
            merge_record(
                &transaction,
                rifle,
                mode,
                &mut report,
                put_rifle,
                |tx, r| {
                    Ok(tx
                        .query_row(
                            "SELECT * FROM rifles WHERE id = ?1",
                            [&r.id],
                            rifle_from_row,
                        )
                        .optional()?)
                },
            )?;
        }
        for bullet in &data.bullets {
            bullet.validate().map_err(StoreError::Invalid)?;
            latest = latest.max(bullet.updated_at);
            merge_record(
                &transaction,
                bullet,
                mode,
                &mut report,
                put_bullet,
                |tx, b| {
                    Ok(tx
                        .query_row(
                            "SELECT * FROM bullets WHERE id = ?1",
                            [&b.id],
                            bullet_from_row,
                        )
                        .optional()?)
                },
            )?;
        }
        for load in &data.rifle_loads {
            load.validate().map_err(StoreError::Invalid)?;
            latest = latest.max(load.updated_at);
            import_load(&transaction, load, mode, &mut report)?;
        }
        observe_on(&transaction, latest)?;
        if dry_run {
            transaction.rollback()?;
        } else {
            transaction.commit()?;
        }
        Ok(report)
    }
}

/// When this device deleted the record, if it did.
fn deleted_at(
    connection: &Connection,
    kind: RecordKind,
    id: &str,
) -> Result<Option<i64>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT deleted_at FROM deleted WHERE kind = ?1 AND id = ?2",
            params![kind.as_str(), id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Another copy's deletion: removes the local record unless it was edited
/// after the deletion, and remembers the deletion.
fn apply_deletion(
    transaction: &Transaction<'_>,
    deletion: &Deletion,
    report: &mut ImportReport,
) -> Result<(), StoreError> {
    let table = deletion.kind.table();
    let local: Option<i64> = transaction
        .query_row(
            &format!("SELECT updated_at FROM {table} WHERE id = ?1"),
            [&deletion.id],
            |row| row.get(0),
        )
        .optional()?;
    match local {
        Some(updated_at) if updated_at > deletion.deleted_at => {
            report.kept_local += 1;
            return Ok(());
        }
        Some(_) => {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE id = ?1"),
                [&deletion.id],
            )?;
            report.deleted += 1;
        }
        None => {}
    }
    record_deletion(
        transaction,
        deletion.kind,
        &deletion.id,
        deletion.deleted_at,
    )
}

/// The data with impossible times (more than a week ahead: a wrong
/// clock, a damaged file) made the oldest: such a time would otherwise win
/// every later merge, and a cap that moves with the clock would still beat
/// edits made since.
fn without_future(data: &PersonalData) -> PersonalData {
    let limit = crate::model::now_ms() + 7 * 24 * 60 * 60 * 1000;
    let fix = |at: &mut i64| {
        if *at > limit {
            *at = 0;
        }
    };
    let mut data = data.clone();
    data.rifles
        .iter_mut()
        .for_each(|record| fix(&mut record.updated_at));
    data.bullets
        .iter_mut()
        .for_each(|record| fix(&mut record.updated_at));
    data.rifle_loads
        .iter_mut()
        .for_each(|record| fix(&mut record.updated_at));
    data.scopes
        .iter_mut()
        .for_each(|record| fix(&mut record.updated_at));
    data.targets
        .iter_mut()
        .for_each(|record| fix(&mut record.updated_at));
    data.deleted
        .iter_mut()
        .for_each(|deletion| fix(&mut deletion.deleted_at));
    data
}

/// Settles a record this device deleted. Returns the record to merge, or
/// `None` when the deletion wins.
fn against_deletion<T: Record>(
    transaction: &Transaction<'_>,
    incoming: &T,
    mode: Mode,
) -> Result<Option<T>, StoreError> {
    let mut record = incoming.clone();
    let Some(deleted) = deleted_at(transaction, T::KIND, incoming.id())? else {
        return Ok(Some(record));
    };
    if deleted >= incoming.updated_at() {
        match mode {
            Mode::Sync => return Ok(None),
            // Restoring a backup: the record comes back as a new edit, so
            // the next sync brings it back elsewhere too.
            Mode::File => record.set_updated_at(stamp_on(transaction, deleted)?),
        }
    }
    transaction.execute(
        "DELETE FROM deleted WHERE kind = ?1 AND id = ?2",
        params![T::KIND.as_str(), incoming.id()],
    )?;
    Ok(Some(record))
}

fn merge_record<T: Record>(
    transaction: &Transaction<'_>,
    incoming: &T,
    mode: Mode,
    report: &mut ImportReport,
    put: fn(&Connection, &T) -> Result<(), StoreError>,
    local: impl FnOnce(&Transaction<'_>, &T) -> Result<Option<T>, StoreError>,
) -> Result<(), StoreError> {
    let Some(record) = against_deletion(transaction, incoming, mode)? else {
        report.count(Merge::KeptLocal);
        return Ok(());
    };
    let merge = decide(local(transaction, &record)?.as_ref(), &record, mode);
    if merge.writes() {
        put(transaction, &record)?;
    }
    report.count(merge);
    Ok(())
}

/// Whether two records hold the same data, ignoring when they were saved.
/// Reads one of this app's exports, upgrading older formats.
pub fn read_export(json: &str) -> Result<PersonalData, StoreError> {
    let mut data: PersonalData = serde_json::from_str(json)?;
    data.rifle_loads
        .iter_mut()
        .for_each(RifleLoad::upgrade_legacy);
    data.targets.iter_mut().for_each(Target::upgrade_legacy);
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

fn decide<T: Record>(local: Option<&T>, incoming: &T, mode: Mode) -> Merge {
    let Some(local) = local else {
        return Merge::Added;
    };
    if same_content(local, incoming) {
        return Merge::Unchanged;
    }
    let later = match incoming.updated_at().cmp(&local.updated_at()) {
        // Saved at the same moment on two devices: compare the content, so
        // both devices keep the same copy. A file never overrules an equal
        // local copy.
        Ordering::Equal if mode == Mode::Sync => {
            let text = |record: &T| serde_json::to_string(record).unwrap_or_default();
            text(incoming) > text(local)
        }
        order => order == Ordering::Greater,
    };
    if later {
        Merge::Updated
    } else {
        Merge::KeptLocal
    }
}

/// A load is identified by its rifle and bullet as well as its id, since
/// two devices may each have measured the same pair.
fn import_load(
    transaction: &Transaction<'_>,
    load: &RifleLoad,
    mode: Mode,
    report: &mut ImportReport,
) -> Result<(), StoreError> {
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
        // Its rifle or bullet was deleted here more recently: so is the load.
        if deleted_at(transaction, RecordKind::Rifle, &load.rifle_id)?.is_some()
            || deleted_at(transaction, RecordKind::Bullet, &load.bullet_id)?.is_some()
        {
            report.count(Merge::KeptLocal);
            return Ok(());
        }
        return Err(StoreError::Invalid(
            "a measured velocity refers to a rifle or bullet missing from the file".into(),
        ));
    }
    let Some(load) = against_deletion(transaction, load, mode)? else {
        report.count(Merge::KeptLocal);
        return Ok(());
    };
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
    let merge = decide(comparable.as_ref(), &load, mode);
    if let (Merge::Updated, Some(local)) = (merge, &local) {
        transaction.execute("DELETE FROM rifle_loads WHERE id = ?1", [&local.id])?;
    }
    if merge.writes() {
        put_load(transaction, &load)?;
    }
    report.count(merge);
    Ok(())
}

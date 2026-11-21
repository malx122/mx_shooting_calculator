//! Local SQLite storage for the hunter's own data. One file per device;
//! SQLite runs on every desktop and mobile platform.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use rusqlite::types::Value as SqlValue;

use crate::StoreError;
use crate::barrels::{Barrel, Clean, RoundLogEntry, ZeroReset};
use crate::deletions::{Deletion, RecordKind};
use crate::groups::{Analysis, Group, GroupView, Ladder};
use crate::model::{
    BcModel, Bullet, BulletKind, ClickUnit, Construction, Rifle, RifleLoad, TableSettings, now_ms,
};
use crate::optics::UserScope;
use crate::photos::Photo;
use crate::sessions::Session;
use crate::shots::Shot;
use crate::targets::Target;
use crate::transfer::Record;

/// Each entry upgrades the schema by one version; never edit a released one.
pub(crate) const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE rifles (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        caliber TEXT NOT NULL,
        barrel_length_mm REAL,
        twist_in REAL NOT NULL,
        right_hand_twist INTEGER NOT NULL,
        sight_height_cm REAL NOT NULL,
        zero_range_m REAL NOT NULL,
        zero_temperature_c REAL NOT NULL,
        zero_pressure_hpa REAL NOT NULL,
        click_unit TEXT NOT NULL,
        click_value REAL NOT NULL,
        notes TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE bullets (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        manufacturer TEXT NOT NULL,
        name TEXT NOT NULL,
        caliber TEXT NOT NULL,
        diameter_mm REAL NOT NULL,
        mass_g REAL NOT NULL,
        length_mm REAL,
        bc_g1 REAL,
        bc_g7 REAL,
        drag_model TEXT NOT NULL,
        muzzle_velocity_mps REAL NOT NULL,
        velocity_temperature_c REAL NOT NULL,
        velocity_per_degree REAL NOT NULL,
        test_barrel_mm REAL,
        construction TEXT NOT NULL,
        lead_free INTEGER NOT NULL,
        min_expansion_mps REAL,
        catalog_id TEXT,
        notes TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX bullets_by_caliber ON bullets (caliber);
    CREATE TABLE rifle_loads (
        id TEXT PRIMARY KEY,
        rifle_id TEXT NOT NULL REFERENCES rifles (id) ON DELETE CASCADE,
        bullet_id TEXT NOT NULL REFERENCES bullets (id) ON DELETE CASCADE,
        muzzle_velocity_mps REAL NOT NULL,
        velocity_temperature_c REAL NOT NULL,
        notes TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL,
        UNIQUE (rifle_id, bullet_id)
    );
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
"#,
    r#"
    ALTER TABLE rifles ADD COLUMN scope_id TEXT;
    ALTER TABLE rifles ADD COLUMN reticle_id TEXT;
"#,
    r#"
    ALTER TABLE bullets ADD COLUMN bc_bands TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE rifles ADD COLUMN zero_powder_temperature_c REAL;
    ALTER TABLE rifles ADD COLUMN zero_humidity_percent REAL NOT NULL DEFAULT 50;
    ALTER TABLE rifles ADD COLUMN table_settings TEXT;
    ALTER TABLE rifle_loads ADD COLUMN velocities TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE rifle_loads ADD COLUMN impact_offset_right_mrad REAL NOT NULL DEFAULT 0;
    ALTER TABLE rifle_loads ADD COLUMN impact_offset_up_mrad REAL NOT NULL DEFAULT 0;
    ALTER TABLE rifle_loads ADD COLUMN zero TEXT NOT NULL DEFAULT '{"mode":"rifle"}';
    UPDATE rifle_loads SET velocities = json_array(
        json_object('temperature_c', velocity_temperature_c, 'velocity_mps', muzzle_velocity_mps));
    ALTER TABLE rifle_loads DROP COLUMN muzzle_velocity_mps;
    ALTER TABLE rifle_loads DROP COLUMN velocity_temperature_c;
"#,
    r#"
    ALTER TABLE bullets ADD COLUMN drag_curve TEXT NOT NULL DEFAULT '[]';
    CREATE TABLE scopes (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
"#,
    r#"
    CREATE TABLE targets (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
"#,
    r#"
    CREATE TABLE deleted (
        kind TEXT NOT NULL,
        id TEXT NOT NULL,
        deleted_at INTEGER NOT NULL,
        PRIMARY KEY (kind, id)
    );
"#,
    r#"
    ALTER TABLE rifle_loads ADD COLUMN truing TEXT;
"#,
    r#"
    ALTER TABLE rifles ADD COLUMN zero_height_cm REAL NOT NULL DEFAULT 0;
"#,
    r#"
    CREATE TABLE shots (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        taken_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX shots_by_time ON shots (taken_at);
"#,
    r#"
    ALTER TABLE bullets ADD COLUMN data_attribution TEXT;
"#,
    r#"
    ALTER TABLE rifles ADD COLUMN fitted_barrel_id TEXT;
    ALTER TABLE rifles ADD COLUMN rounds_before_logging INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE bullets ADD COLUMN recipe TEXT;
    CREATE TABLE barrels (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        rifle_id TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX barrels_by_rifle ON barrels (rifle_id);
    CREATE TABLE round_log (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        rifle_id TEXT NOT NULL,
        at INTEGER NOT NULL,
        group_id TEXT,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX round_log_by_rifle ON round_log (rifle_id, at);
    CREATE INDEX round_log_by_group ON round_log (group_id);
    CREATE TABLE cleans (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        rifle_id TEXT NOT NULL,
        at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX cleans_by_rifle ON cleans (rifle_id, at);
    CREATE TABLE zero_resets (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        rifle_id TEXT NOT NULL,
        at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX zero_resets_by_rifle ON zero_resets (rifle_id, at);
    CREATE TABLE sessions (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX sessions_by_time ON sessions (started_at);
    CREATE TABLE photos (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE shot_groups (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        taken_at INTEGER NOT NULL,
        photo_id TEXT,
        rifle_id TEXT,
        session_id TEXT,
        ladder_id TEXT,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX shot_groups_by_time ON shot_groups (taken_at);
    CREATE INDEX shot_groups_by_photo ON shot_groups (photo_id);
    CREATE INDEX shot_groups_by_rifle ON shot_groups (rifle_id);
    CREATE INDEX shot_groups_by_session ON shot_groups (session_id);
    CREATE INDEX shot_groups_by_ladder ON shot_groups (ladder_id);
    CREATE TABLE ladders (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        rifle_id TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX ladders_by_rifle ON ladders (rifle_id);
    CREATE TABLE analyses (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE group_views (
        group_id TEXT PRIMARY KEY,
        view TEXT NOT NULL
    );
"#,
];

/// The schema version of the last released app, which every newer schema
/// must upgrade from.
#[cfg(test)]
pub(crate) const RELEASED_SCHEMA: usize = 10;

/// The setting holding the store's clock: the latest time it has stamped
/// or seen, so new stamps never go backwards (see `Store::stamp`).
const CLOCK: &str = "clock";

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Store, StoreError> {
        Store::init(Connection::open(path)?)
    }

    /// Opens the database only to read it, as it is: no upgrade, which an
    /// older app sharing it couldn't open afterwards. Refuses a database of
    /// another schema version than this program's.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Store, StoreError> {
        use rusqlite::OpenFlags;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let current = MIGRATIONS.len() as i64;
        if version < current {
            return Err(StoreError::Invalid(format!(
                "the database is from an older version of the app (schema {version}, this \
                 program reads {current}): start the app once to bring it up to date"
            )));
        }
        if version > current {
            return Err(StoreError::Newer(version));
        }
        Ok(Store { connection })
    }

    /// One setting read straight from a database file, whatever version of
    /// the app wrote it, changing nothing: for when the file can't be
    /// opened as a store, such as one from a newer version. None when it
    /// can't be read.
    pub fn peek_setting<T: DeserializeOwned>(path: impl AsRef<Path>, key: &str) -> Option<T> {
        use rusqlite::OpenFlags;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;
        let text: String = connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Moves a database that can't be used out of the way, so a new one
    /// can start at `path`: renamed, with its SQLite side files, to
    /// "<name>-reset-<UTC date and time>.<extension>" in the same folder,
    /// never deleted. Returns the new name.
    pub fn set_aside(path: &Path, now_ms: i64) -> Result<std::path::PathBuf, StoreError> {
        let date = crate::sun::Date::from_unix_ms(now_ms);
        let seconds = now_ms.div_euclid(1000).rem_euclid(86_400);
        let stamp = format!(
            "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
            date.year,
            date.month,
            date.day,
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        );
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("database");
        let name = match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) => format!("{stem}-reset-{stamp}.{extension}"),
            None => format!("{stem}-reset-{stamp}"),
        };
        let aside = path.with_file_name(name);
        if aside.exists() {
            return Err(StoreError::Invalid(format!(
                "{} is already there",
                aside.display()
            )));
        }
        let rename = |from: &Path, to: &Path| {
            std::fs::rename(from, to).map_err(|error| {
                StoreError::Invalid(format!(
                    "could not rename {} to {}: {error}",
                    from.display(),
                    to.display()
                ))
            })
        };
        // The journal and write-ahead log belong to the file: they go with
        // it, or, if one can't, everything goes back as it was.
        let with = |file: &Path, side: &str| {
            let mut name = file.as_os_str().to_owned();
            name.push(side);
            std::path::PathBuf::from(name)
        };
        let sides: Vec<&str> = ["-journal", "-wal", "-shm"]
            .into_iter()
            .filter(|side| with(path, side).exists())
            .collect();
        rename(path, &aside)?;
        for (done, side) in sides.iter().enumerate() {
            if let Err(error) = rename(&with(path, side), &with(&aside, side)) {
                for side in &sides[..done] {
                    let _ = std::fs::rename(with(&aside, side), with(path, side));
                }
                let _ = std::fs::rename(&aside, path);
                return Err(error);
            }
        }
        Ok(aside)
    }

    pub fn open_in_memory() -> Result<Store, StoreError> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(connection: Connection) -> Result<Store, StoreError> {
        connection.pragma_update(None, "foreign_keys", true)?;
        let mut store = Store { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), StoreError> {
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > MIGRATIONS.len() as i64 {
            return Err(StoreError::Newer(version));
        }
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", index as i64 + 1)?;
            transaction.commit()?;
        }
        Ok(())
    }

    pub(crate) fn connection(&mut self) -> &mut Connection {
        &mut self.connection
    }

    #[cfg(test)]
    pub(crate) fn connection_ref(&self) -> &Connection {
        &self.connection
    }

    // Rifles ---------------------------------------------------------------

    pub fn rifles(&self) -> Result<Vec<Rifle>, StoreError> {
        self.query_all(
            "SELECT * FROM rifles ORDER BY name COLLATE NOCASE",
            [],
            rifle_from_row,
        )
    }

    pub fn rifle(&self, id: &str) -> Result<Option<Rifle>, StoreError> {
        self.query_one("SELECT * FROM rifles WHERE id = ?1", [id], rifle_from_row)
    }

    /// Validates, stamps `updated_at` and inserts or updates the rifle.
    pub fn save_rifle(&self, rifle: &mut Rifle) -> Result<(), StoreError> {
        rifle.validate().map_err(StoreError::Invalid)?;
        rifle.updated_at = self.stamp(rifle.updated_at)?;
        self.put_undeleted(RecordKind::Rifle, &rifle.id, |connection| {
            put_rifle(connection, rifle)
        })
    }

    /// Deletes the rifle, remembering the deletion so a sync deletes it on
    /// other devices too. What belongs to it goes with it (the loads
    /// measured in it; its barrels, round log, cleans and zero resets)
    /// without being remembered on its own: if another device edits the
    /// rifle later, it comes back with all of them. Groups, ladders and
    /// shots keep naming it.
    pub fn delete_rifle(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::Rifle, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM rifles WHERE id = ?1", [id])?;
        sweep_orphans(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    // Bullets --------------------------------------------------------------

    pub fn bullets(&self) -> Result<Vec<Bullet>, StoreError> {
        self.query_all(
            "SELECT * FROM bullets ORDER BY manufacturer COLLATE NOCASE, name COLLATE NOCASE, mass_g",
            [],
            bullet_from_row,
        )
    }

    /// The collection's bullets loaded in this caliber: the only ones a
    /// rifle of that caliber can shoot.
    pub fn bullets_for_caliber(&self, caliber: &str) -> Result<Vec<Bullet>, StoreError> {
        self.query_all(
            "SELECT * FROM bullets WHERE caliber = ?1 \
             ORDER BY manufacturer COLLATE NOCASE, name COLLATE NOCASE, mass_g",
            [caliber],
            bullet_from_row,
        )
    }

    pub fn bullet(&self, id: &str) -> Result<Option<Bullet>, StoreError> {
        self.query_one("SELECT * FROM bullets WHERE id = ?1", [id], bullet_from_row)
    }

    pub fn save_bullet(&self, bullet: &mut Bullet) -> Result<(), StoreError> {
        bullet.validate().map_err(StoreError::Invalid)?;
        bullet.updated_at = self.stamp(bullet.updated_at)?;
        self.put_undeleted(RecordKind::Bullet, &bullet.id, |connection| {
            put_bullet(connection, bullet)
        })
    }

    /// Deletes the bullet and the loads measured with it; as with a rifle,
    /// only the bullet's deletion is remembered.
    pub fn delete_bullet(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::Bullet, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM bullets WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    // Custom scopes --------------------------------------------------------

    pub fn scopes(&self) -> Result<Vec<UserScope>, StoreError> {
        let mut scopes = self.query_all("SELECT data FROM scopes", [], |row| {
            from_json_column::<UserScope>(&row.get::<_, String>(0)?)
        })?;
        scopes.sort_by_key(|scope| scope.display_name().to_lowercase());
        Ok(scopes)
    }

    pub fn save_scope(&self, scope: &mut UserScope) -> Result<(), StoreError> {
        scope.validate().map_err(StoreError::Invalid)?;
        scope.updated_at = self.stamp(scope.updated_at)?;
        self.put_undeleted(RecordKind::Scope, &scope.id, |connection| {
            put_scope(connection, scope)
        })
    }

    /// Deletes a custom scope and takes it off the rifles that had it.
    pub fn delete_scope(&mut self, id: &str) -> Result<(), StoreError> {
        let at = self.stamp(0)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE rifles SET scope_id = NULL, reticle_id = NULL, updated_at = MAX(updated_at + 1, ?2)
             WHERE scope_id = ?1",
            params![id, at],
        )?;
        record_deletion(&transaction, RecordKind::Scope, id, at)?;
        transaction.execute("DELETE FROM scopes WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    // Custom targets -------------------------------------------------------

    pub fn targets(&self) -> Result<Vec<Target>, StoreError> {
        let mut targets = self.query_all("SELECT data FROM targets", [], |row| {
            from_json_column::<Target>(&row.get::<_, String>(0)?)
        })?;
        targets.iter_mut().for_each(Target::upgrade_legacy);
        targets.sort_by_key(|target| target.name.to_lowercase());
        Ok(targets)
    }

    pub fn save_target(&self, target: &mut Target) -> Result<(), StoreError> {
        target.upgrade_legacy();
        target.validate().map_err(StoreError::Invalid)?;
        target.updated_at = self.stamp(target.updated_at)?;
        self.put_undeleted(RecordKind::Target, &target.id, |connection| {
            put_target(connection, target)
        })
    }

    pub fn delete_target(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::Target, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM targets WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    // The shot log --------------------------------------------------------

    /// The logged shots, newest first.
    pub fn shots(&self) -> Result<Vec<Shot>, StoreError> {
        self.query_all(
            "SELECT data FROM shots ORDER BY taken_at DESC, id",
            [],
            |row| from_json_column::<Shot>(&row.get::<_, String>(0)?),
        )
    }

    pub fn shot(&self, id: &str) -> Result<Option<Shot>, StoreError> {
        self.query_one("SELECT data FROM shots WHERE id = ?1", [id], |row| {
            from_json_column::<Shot>(&row.get::<_, String>(0)?)
        })
    }

    /// Works out the animal's spot again, validates, stamps `updated_at`
    /// and inserts or updates the shot.
    pub fn save_shot(&self, shot: &mut Shot) -> Result<(), StoreError> {
        shot.update_spot();
        shot.validate().map_err(StoreError::Invalid)?;
        shot.updated_at = self.stamp(shot.updated_at)?;
        self.put_undeleted(RecordKind::Shot, &shot.id, |connection| {
            put_shot(connection, shot)
        })
    }

    pub fn delete_shot(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::Shot, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM shots WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    // Rifle loads ----------------------------------------------------------

    pub fn rifle_loads(&self) -> Result<Vec<RifleLoad>, StoreError> {
        self.query_all("SELECT * FROM rifle_loads", [], load_from_row)
    }

    pub fn rifle_load(
        &self,
        rifle_id: &str,
        bullet_id: &str,
    ) -> Result<Option<RifleLoad>, StoreError> {
        self.query_one(
            "SELECT * FROM rifle_loads WHERE rifle_id = ?1 AND bullet_id = ?2",
            [rifle_id, bullet_id],
            load_from_row,
        )
    }

    /// Saves the measured velocity for a rifle and bullet, replacing any
    /// earlier measurement of the same pair.
    pub fn save_rifle_load(&self, load: &mut RifleLoad) -> Result<(), StoreError> {
        load.validate().map_err(StoreError::Invalid)?;
        if let Some(existing) = self.rifle_load(&load.rifle_id, &load.bullet_id)? {
            load.id = existing.id;
        }
        load.updated_at = self.stamp(load.updated_at)?;
        self.put_undeleted(RecordKind::RifleLoad, &load.id, |connection| {
            put_load(connection, load)
        })
    }

    pub fn delete_rifle_load(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::RifleLoad, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM rifle_loads WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    // Barrels, the round log, cleans and zero resets -------------------------

    /// Every rifle's barrels, by name.
    pub fn barrels(&self) -> Result<Vec<Barrel>, StoreError> {
        let mut barrels: Vec<Barrel> = self.blobs("", [], "id")?;
        barrels.sort_by_key(|barrel| barrel.name.to_lowercase());
        Ok(barrels)
    }

    pub fn barrels_for_rifle(&self, rifle_id: &str) -> Result<Vec<Barrel>, StoreError> {
        let mut barrels: Vec<Barrel> = self.blobs("WHERE rifle_id = ?1", [rifle_id], "id")?;
        barrels.sort_by_key(|barrel| barrel.name.to_lowercase());
        Ok(barrels)
    }

    pub fn barrel(&self, id: &str) -> Result<Option<Barrel>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    pub fn save_barrel(&self, barrel: &mut Barrel) -> Result<(), StoreError> {
        self.save_blob(barrel)
    }

    /// Deletes the barrel, with its cleans and zero resets following it as a
    /// rifle's history follows the rifle, and takes it off the rifle it is
    /// fitted to. Its round log entries stay: the rounds went through the
    /// action too.
    pub fn delete_barrel(&self, id: &str) -> Result<(), StoreError> {
        let at = self.stamp(0)?;
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE rifles SET fitted_barrel_id = NULL, updated_at = MAX(updated_at + 1, ?2)
             WHERE fitted_barrel_id = ?1",
            params![id, at],
        )?;
        record_deletion(&transaction, RecordKind::Barrel, id, at)?;
        transaction.execute("DELETE FROM barrels WHERE id = ?1", [id])?;
        sweep_orphans(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// The round log, newest first.
    pub fn round_log(&self) -> Result<Vec<RoundLogEntry>, StoreError> {
        self.blobs("", [], "at DESC, id")
    }

    /// A rifle's round log, newest first.
    pub fn round_log_for_rifle(&self, rifle_id: &str) -> Result<Vec<RoundLogEntry>, StoreError> {
        self.blobs("WHERE rifle_id = ?1", [rifle_id], "at DESC, id")
    }

    pub fn save_round_log_entry(&self, entry: &mut RoundLogEntry) -> Result<(), StoreError> {
        self.save_blob(entry)
    }

    /// The entry logging a group's rounds, if one was made.
    pub fn round_log_entry_for_group(
        &self,
        group_id: &str,
    ) -> Result<Option<RoundLogEntry>, StoreError> {
        Ok(self
            .blobs("WHERE group_id = ?1", [group_id], "at DESC, id")?
            .into_iter()
            .next())
    }

    pub fn delete_round_log_entry(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::RoundLogEntry, id)
    }

    /// Every clean, newest first.
    pub fn cleans(&self) -> Result<Vec<Clean>, StoreError> {
        self.blobs("", [], "at DESC, id")
    }

    pub fn cleans_for_rifle(&self, rifle_id: &str) -> Result<Vec<Clean>, StoreError> {
        self.blobs("WHERE rifle_id = ?1", [rifle_id], "at DESC, id")
    }

    pub fn save_clean(&self, clean: &mut Clean) -> Result<(), StoreError> {
        self.save_blob(clean)
    }

    pub fn delete_clean(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::Clean, id)
    }

    /// Every zero reset, newest first.
    pub fn zero_resets(&self) -> Result<Vec<ZeroReset>, StoreError> {
        self.blobs("", [], "at DESC, id")
    }

    pub fn zero_resets_for_rifle(&self, rifle_id: &str) -> Result<Vec<ZeroReset>, StoreError> {
        self.blobs("WHERE rifle_id = ?1", [rifle_id], "at DESC, id")
    }

    pub fn save_zero_reset(&self, reset: &mut ZeroReset) -> Result<(), StoreError> {
        self.save_blob(reset)
    }

    pub fn delete_zero_reset(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::ZeroReset, id)
    }

    // Sessions, photos, groups, ladders and analyses --------------------------

    /// The range sessions, newest first.
    pub fn sessions(&self) -> Result<Vec<Session>, StoreError> {
        self.blobs("", [], "started_at DESC, id")
    }

    pub fn session(&self, id: &str) -> Result<Option<Session>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    pub fn save_session(&self, session: &mut Session) -> Result<(), StoreError> {
        self.save_blob(session)
    }

    /// Deletes the session; its groups keep their own conditions.
    pub fn delete_session(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::Session, id)
    }

    /// What is known about the photos; their bytes are in a `PhotoStore`.
    pub fn photos(&self) -> Result<Vec<Photo>, StoreError> {
        self.blobs("", [], "id")
    }

    pub fn photo(&self, id: &str) -> Result<Option<Photo>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    pub fn save_photo(&self, photo: &mut Photo) -> Result<(), StoreError> {
        self.save_blob(photo)
    }

    /// Deletes the photo's record unless a group is marked on it, and says
    /// whether no group is, so the photo's bytes can go too. Only this
    /// device forgets it: other devices keep their copy while their groups
    /// use it, and a merge brings it back with a group that does.
    pub fn delete_photo(&self, id: &str) -> Result<bool, StoreError> {
        let used = self
            .query_one(
                "SELECT 1 FROM shot_groups WHERE photo_id = ?1 LIMIT 1",
                [id],
                |_| Ok(()),
            )?
            .is_some();
        if !used {
            self.connection
                .execute("DELETE FROM photos WHERE id = ?1", [id])?;
        }
        Ok(!used)
    }

    /// Every photo this device's records name: the photo records and the
    /// photos groups are marked on. Bytes kept for any other photo are left
    /// over.
    pub fn photo_ids_in_use(&self) -> Result<std::collections::BTreeSet<String>, StoreError> {
        Ok(self
            .query_all(
                "SELECT id FROM photos
                 UNION SELECT photo_id FROM shot_groups WHERE photo_id IS NOT NULL",
                [],
                |row| row.get(0),
            )?
            .into_iter()
            .collect())
    }

    /// The groups, newest first.
    pub fn groups(&self) -> Result<Vec<Group>, StoreError> {
        self.blobs("", [], "taken_at DESC, id")
    }

    pub fn group(&self, id: &str) -> Result<Option<Group>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    /// The groups marked on a photo, newest first.
    pub fn groups_on_photo(&self, photo_id: &str) -> Result<Vec<Group>, StoreError> {
        self.blobs("WHERE photo_id = ?1", [photo_id], "taken_at DESC, id")
    }

    /// The groups shot in a session, newest first.
    pub fn groups_in_session(&self, session_id: &str) -> Result<Vec<Group>, StoreError> {
        self.blobs("WHERE session_id = ?1", [session_id], "taken_at DESC, id")
    }

    /// The groups shot with a rifle, newest first.
    pub fn groups_for_rifle(&self, rifle_id: &str) -> Result<Vec<Group>, StoreError> {
        self.blobs("WHERE rifle_id = ?1", [rifle_id], "taken_at DESC, id")
    }

    /// The rungs of a ladder, newest first.
    pub fn groups_in_ladder(&self, ladder_id: &str) -> Result<Vec<Group>, StoreError> {
        self.blobs("WHERE ladder_id = ?1", [ladder_id], "taken_at DESC, id")
    }

    pub fn save_group(&self, group: &mut Group) -> Result<(), StoreError> {
        self.save_blob(group)
    }

    /// Deletes the group. Round log entries made from it stay: the rounds
    /// were fired.
    pub fn delete_group(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::Group, id)?;
        self.connection
            .execute("DELETE FROM group_views WHERE group_id = ?1", [id])?;
        Ok(())
    }

    /// How this device last showed the group; views are kept per device,
    /// never exported or synced.
    pub fn group_view(&self, group_id: &str) -> Result<Option<GroupView>, StoreError> {
        let text: Option<String> = self.query_one(
            "SELECT view FROM group_views WHERE group_id = ?1",
            [group_id],
            |row| row.get(0),
        )?;
        Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
    }

    pub fn set_group_view(&self, group_id: &str, view: &GroupView) -> Result<(), StoreError> {
        view.validate().map_err(StoreError::Invalid)?;
        self.connection.execute(
            "INSERT INTO group_views (group_id, view) VALUES (?1, ?2)
             ON CONFLICT (group_id) DO UPDATE SET view = excluded.view",
            params![group_id, serde_json::to_string(view)?],
        )?;
        Ok(())
    }

    /// The ladders, by name.
    pub fn ladders(&self) -> Result<Vec<Ladder>, StoreError> {
        let mut ladders: Vec<Ladder> = self.blobs("", [], "id")?;
        ladders.sort_by_key(|ladder| ladder.name.to_lowercase());
        Ok(ladders)
    }

    pub fn ladder(&self, id: &str) -> Result<Option<Ladder>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    pub fn save_ladder(&self, ladder: &mut Ladder) -> Result<(), StoreError> {
        self.save_blob(ladder)
    }

    /// Deletes the ladder; its groups stay, naming it.
    pub fn delete_ladder(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::Ladder, id)
    }

    /// The saved analyses, by name.
    pub fn analyses(&self) -> Result<Vec<Analysis>, StoreError> {
        let mut analyses: Vec<Analysis> = self.blobs("", [], "id")?;
        analyses.sort_by_key(|analysis| analysis.name.to_lowercase());
        Ok(analyses)
    }

    pub fn analysis(&self, id: &str) -> Result<Option<Analysis>, StoreError> {
        blob_by_id(&self.connection, id)
    }

    pub fn save_analysis(&self, analysis: &mut Analysis) -> Result<(), StoreError> {
        self.save_blob(analysis)
    }

    pub fn delete_analysis(&self, id: &str) -> Result<(), StoreError> {
        self.delete_blob(RecordKind::Analysis, id)
    }

    /// Everything the Groups part shows besides rifles and bullets, read at
    /// once.
    pub fn groups_data(&self) -> Result<GroupsData, StoreError> {
        Ok(GroupsData {
            barrels: self.barrels()?,
            round_log: self.round_log()?,
            cleans: self.cleans()?,
            zero_resets: self.zero_resets()?,
            sessions: self.sessions()?,
            photos: self.photos()?,
            groups: self.groups()?,
            ladders: self.ladders()?,
            analyses: self.analyses()?,
        })
    }

    fn blobs<T: Blob, P: rusqlite::Params>(
        &self,
        condition: &str,
        parameters: P,
        order: &str,
    ) -> Result<Vec<T>, StoreError> {
        let sql = format!(
            "SELECT data FROM {} {condition} ORDER BY {order}",
            T::KIND.table()
        );
        self.query_all(&sql, parameters, |row| {
            from_json_column::<T>(&row.get::<_, String>(0)?)
        })
    }

    /// Validates, stamps `updated_at` and inserts or updates the record.
    fn save_blob<T: Blob>(&self, record: &mut T) -> Result<(), StoreError> {
        record.check().map_err(StoreError::Invalid)?;
        let at = self.stamp(record.updated_at())?;
        record.set_updated_at(at);
        self.put_undeleted(T::KIND, record.id(), |connection| {
            put_blob(connection, record)
        })
    }

    /// Writes a record with `put` and forgets that this device deleted it,
    /// together: a record saved after its deletion (an undone delete, an id
    /// made again) is a live record, and a merge must not set the old
    /// deletion against it.
    fn put_undeleted(
        &self,
        kind: RecordKind,
        id: &str,
        put: impl FnOnce(&Connection) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM deleted WHERE kind = ?1 AND id = ?2",
            params![kind.as_str(), id],
        )?;
        put(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    fn delete_blob(&self, kind: RecordKind, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, kind, id, self.stamp(0)?)?;
        transaction.execute(&format!("DELETE FROM {} WHERE id = ?1", kind.table()), [id])?;
        transaction.commit()?;
        Ok(())
    }

    /// The database file, for keeping things beside it; `None` in memory.
    pub fn file_path(&self) -> Option<std::path::PathBuf> {
        self.connection
            .path()
            .filter(|path| !path.is_empty())
            .map(std::path::PathBuf::from)
    }

    // Deletions and the clock ----------------------------------------------

    /// The records deleted on this device (or learnt from others), so a
    /// sync can pass the deletions on.
    pub fn deletions(&self) -> Result<Vec<Deletion>, StoreError> {
        self.query_all(
            "SELECT kind, id, deleted_at FROM deleted ORDER BY kind, id",
            [],
            |row| {
                let kind: String = row.get(0)?;
                Ok(Deletion {
                    kind: RecordKind::parse(&kind).unwrap_or(RecordKind::Rifle),
                    id: row.get(1)?,
                    deleted_at: row.get(2)?,
                })
            },
        )
    }

    /// A time for a save or deletion: after now, after the record's previous
    /// stamp and after every stamp this store has made or seen. A device
    /// whose clock is behind still stamps its edits later than the copies
    /// they were made from, so they win a merge.
    pub(crate) fn stamp(&self, previous: i64) -> Result<i64, StoreError> {
        stamp_on(&self.connection, previous)
    }

    // Settings -------------------------------------------------------------

    /// Device-local preferences, such as the last weather entered. Not part
    /// of the exported personal data.
    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StoreError> {
        let text: Option<String> =
            self.query_one("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })?;
        // A setting written by an older app version may no longer parse;
        // fall back to the default instead of failing.
        Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    fn query_all<T, P: rusqlite::Params>(
        &self,
        sql: &str,
        parameters: P,
        map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, StoreError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(parameters, map)?;
        Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
    }

    fn query_one<T, P: rusqlite::Params>(
        &self,
        sql: &str,
        parameters: P,
        map: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, StoreError> {
        Ok(self.connection.query_row(sql, parameters, map).optional()?)
    }
}

/// See `Store::stamp`; also works inside a transaction.
pub(crate) fn stamp_on(connection: &Connection, previous: i64) -> Result<i64, StoreError> {
    let at = now_ms()
        .max(previous.saturating_add(1))
        .max(clock(connection)?.saturating_add(1));
    set_clock(connection, at)?;
    Ok(at)
}

/// Moves the clock past a time seen in merged data.
pub(crate) fn observe_on(connection: &Connection, seen: i64) -> Result<(), StoreError> {
    if seen > clock(connection)? {
        set_clock(connection, seen)?;
    }
    Ok(())
}

fn clock(connection: &Connection) -> Result<i64, StoreError> {
    let text: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [CLOCK],
            |row| row.get(0),
        )
        .optional()?;
    Ok(text.and_then(|text| text.parse().ok()).unwrap_or(0))
}

fn set_clock(connection: &Connection, at: i64) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![CLOCK, at.to_string()],
    )?;
    Ok(())
}

/// Remembers that a record was deleted; a later deletion of the same id
/// keeps the later time.
pub(crate) fn record_deletion(
    connection: &Connection,
    kind: RecordKind,
    id: &str,
    at: i64,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO deleted (kind, id, deleted_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind, id) DO UPDATE SET deleted_at = MAX(deleted_at, excluded.deleted_at)",
        params![kind.as_str(), id, at],
    )?;
    Ok(())
}

/// Removes what belongs to a rifle or a barrel this device remembers as
/// deleted and doesn't have: the rifle's barrels, round log, cleans and
/// zero resets (but not those of a barrel still kept, which follow the
/// barrel), and the barrel's cleans and zero resets. Such records have no
/// deletions of their own, so they follow their parent's fate whichever
/// device last changed it. Also drops the views of groups that are gone.
/// Returns how many records went.
pub(crate) fn sweep_orphans(connection: &Connection) -> Result<usize, StoreError> {
    let gone = |kind: RecordKind, column: &str| {
        format!(
            "{column} IN (SELECT id FROM deleted WHERE kind = '{}')
             AND {column} NOT IN (SELECT id FROM {})",
            kind.as_str(),
            kind.table()
        )
    };
    let barrel = "json_extract(data, '$.barrel_id')";
    let rifle_gone = gone(RecordKind::Rifle, "rifle_id");
    let mut swept = connection.execute(&format!("DELETE FROM barrels WHERE {rifle_gone}"), [])?;
    for kind in [
        RecordKind::RoundLogEntry,
        RecordKind::Clean,
        RecordKind::ZeroReset,
    ] {
        swept += connection.execute(
            &format!(
                "DELETE FROM {} WHERE {rifle_gone}
                 AND ({barrel} IS NULL OR {barrel} NOT IN (SELECT id FROM barrels))",
                kind.table()
            ),
            [],
        )?;
    }
    for kind in [RecordKind::Clean, RecordKind::ZeroReset] {
        swept += connection.execute(
            &format!(
                "DELETE FROM {} WHERE {}",
                kind.table(),
                gone(RecordKind::Barrel, barrel)
            ),
            [],
        )?;
    }
    connection.execute(
        "DELETE FROM group_views WHERE group_id NOT IN (SELECT id FROM shot_groups)",
        [],
    )?;
    Ok(swept)
}

/// What the Groups part shows besides rifles and bullets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupsData {
    pub barrels: Vec<Barrel>,
    /// Newest first.
    pub round_log: Vec<RoundLogEntry>,
    /// Newest first.
    pub cleans: Vec<Clean>,
    /// Newest first.
    pub zero_resets: Vec<ZeroReset>,
    /// Newest first.
    pub sessions: Vec<Session>,
    pub photos: Vec<Photo>,
    /// Newest first.
    pub groups: Vec<Group>,
    pub ladders: Vec<Ladder>,
    pub analyses: Vec<Analysis>,
}

/// A record kept as JSON in a table of its own, beside the columns its
/// lists are sorted and filtered by.
pub(crate) trait Blob: Record + DeserializeOwned {
    /// Every column but `data`, with this record's values.
    fn columns(&self) -> Vec<(&'static str, SqlValue)>;
    fn check(&self) -> Result<(), String>;
}

macro_rules! blob {
    ($type:ty $(, $column:literal => |$record:ident| $value:expr)* $(,)?) => {
        impl Blob for $type {
            fn columns(&self) -> Vec<(&'static str, SqlValue)> {
                vec![
                    ("id", SqlValue::Text(self.id.clone())),
                    ("updated_at", SqlValue::Integer(self.updated_at)),
                    $(($column, {
                        let $record = self;
                        SqlValue::from($value)
                    }),)*
                ]
            }

            fn check(&self) -> Result<(), String> {
                self.validate()
            }
        }
    };
}

blob!(Barrel, "rifle_id" => |barrel| barrel.rifle_id.clone());
blob!(
    RoundLogEntry,
    "rifle_id" => |entry| entry.rifle_id.clone(),
    "at" => |entry| entry.at,
    "group_id" => |entry| entry.group_id.clone(),
);
blob!(
    Clean,
    "rifle_id" => |clean| clean.rifle_id.clone(),
    "at" => |clean| clean.at,
);
blob!(
    ZeroReset,
    "rifle_id" => |reset| reset.rifle_id.clone(),
    "at" => |reset| reset.at,
);
blob!(Session, "started_at" => |session| session.started_at);
blob!(Photo);
blob!(
    Group,
    "taken_at" => |group| group.taken_at,
    "photo_id" => |group| group.photo_id().map(str::to_owned),
    "rifle_id" => |group| group.rifle_id.clone(),
    "session_id" => |group| group.session_id.clone(),
    "ladder_id" => |group| group.ladder_id.clone(),
);
blob!(Ladder, "rifle_id" => |ladder| ladder.rifle_id.clone());
blob!(Analysis);

pub(crate) fn put_blob<T: Blob>(connection: &Connection, record: &T) -> Result<(), StoreError> {
    let mut columns = record.columns();
    columns.push(("data", SqlValue::Text(serde_json::to_string(record)?)));
    let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    let places: Vec<String> = (1..=columns.len()).map(|at| format!("?{at}")).collect();
    let updates: Vec<String> = names
        .iter()
        .filter(|name| **name != "id")
        .map(|name| format!("{name} = excluded.{name}"))
        .collect();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT (id) DO UPDATE SET {}",
        T::KIND.table(),
        names.join(", "),
        places.join(", "),
        updates.join(", ")
    );
    connection
        .prepare_cached(&sql)?
        .execute(rusqlite::params_from_iter(
            columns.into_iter().map(|(_, value)| value),
        ))?;
    Ok(())
}

pub(crate) fn blob_by_id<T: Blob>(
    connection: &Connection,
    id: &str,
) -> Result<Option<T>, StoreError> {
    Ok(connection
        .query_row(
            &format!("SELECT data FROM {} WHERE id = ?1", T::KIND.table()),
            [id],
            |row| from_json_column::<T>(&row.get::<_, String>(0)?),
        )
        .optional()?)
}

pub(crate) fn put_rifle(connection: &Connection, rifle: &Rifle) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO rifles (id, name, caliber, barrel_length_mm, twist_in, right_hand_twist,
             sight_height_cm, zero_range_m, zero_temperature_c, zero_pressure_hpa, click_unit,
             click_value, notes, updated_at, scope_id, reticle_id, zero_powder_temperature_c,
             zero_humidity_percent, table_settings, zero_height_cm, fitted_barrel_id,
             rounds_before_logging)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                 ?20, ?21, ?22)
         ON CONFLICT (id) DO UPDATE SET
             name = excluded.name, caliber = excluded.caliber,
             barrel_length_mm = excluded.barrel_length_mm, twist_in = excluded.twist_in,
             right_hand_twist = excluded.right_hand_twist, sight_height_cm = excluded.sight_height_cm,
             zero_range_m = excluded.zero_range_m, zero_temperature_c = excluded.zero_temperature_c,
             zero_pressure_hpa = excluded.zero_pressure_hpa, click_unit = excluded.click_unit,
             click_value = excluded.click_value, notes = excluded.notes,
             updated_at = excluded.updated_at, scope_id = excluded.scope_id,
             reticle_id = excluded.reticle_id,
             zero_powder_temperature_c = excluded.zero_powder_temperature_c,
             zero_humidity_percent = excluded.zero_humidity_percent,
             table_settings = excluded.table_settings,
             zero_height_cm = excluded.zero_height_cm,
             fitted_barrel_id = excluded.fitted_barrel_id,
             rounds_before_logging = excluded.rounds_before_logging",
        params![
            rifle.id,
            rifle.name,
            rifle.caliber,
            rifle.barrel_length_mm,
            rifle.twist_in,
            rifle.right_hand_twist,
            rifle.sight_height_cm,
            rifle.zero_range_m,
            rifle.zero_temperature_c,
            rifle.zero_pressure_hpa,
            rifle.click_unit.as_str(),
            rifle.click_value,
            rifle.notes,
            rifle.updated_at,
            rifle.scope_id,
            rifle.reticle_id,
            rifle.zero_powder_temperature_c,
            rifle.zero_humidity_percent,
            serde_json::to_string(&rifle.table)?,
            rifle.zero_height_cm,
            rifle.fitted_barrel_id,
            rifle.rounds_before_logging,
        ],
    )?;
    Ok(())
}

pub(crate) fn put_bullet(connection: &Connection, bullet: &Bullet) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO bullets (id, kind, manufacturer, name, caliber, diameter_mm, mass_g,
             length_mm, bc_g1, bc_g7, drag_model, muzzle_velocity_mps, velocity_temperature_c,
             velocity_per_degree, test_barrel_mm, construction, lead_free, min_expansion_mps,
             catalog_id, notes, updated_at, bc_bands, drag_curve, data_attribution, recipe)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
             ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
         ON CONFLICT (id) DO UPDATE SET
             kind = excluded.kind, manufacturer = excluded.manufacturer, name = excluded.name,
             caliber = excluded.caliber, diameter_mm = excluded.diameter_mm,
             mass_g = excluded.mass_g, length_mm = excluded.length_mm, bc_g1 = excluded.bc_g1,
             bc_g7 = excluded.bc_g7, drag_model = excluded.drag_model,
             muzzle_velocity_mps = excluded.muzzle_velocity_mps,
             velocity_temperature_c = excluded.velocity_temperature_c,
             velocity_per_degree = excluded.velocity_per_degree,
             test_barrel_mm = excluded.test_barrel_mm, construction = excluded.construction,
             lead_free = excluded.lead_free, min_expansion_mps = excluded.min_expansion_mps,
             catalog_id = excluded.catalog_id, notes = excluded.notes,
             updated_at = excluded.updated_at, bc_bands = excluded.bc_bands,
             drag_curve = excluded.drag_curve, data_attribution = excluded.data_attribution,
             recipe = excluded.recipe",
        params![
            bullet.id,
            match bullet.kind {
                BulletKind::Factory => "factory",
                BulletKind::Handload => "handload",
            },
            bullet.manufacturer,
            bullet.name,
            bullet.caliber,
            bullet.diameter_mm,
            bullet.mass_g,
            bullet.length_mm,
            bullet.bc_g1,
            bullet.bc_g7,
            match bullet.drag_model {
                BcModel::G1 => "G1",
                BcModel::G7 => "G7",
                BcModel::Custom => "Custom",
            },
            bullet.muzzle_velocity_mps,
            bullet.velocity_temperature_c,
            bullet.velocity_per_degree,
            bullet.test_barrel_mm,
            bullet.construction.as_str(),
            bullet.lead_free,
            bullet.min_expansion_mps,
            bullet.catalog_id,
            bullet.notes,
            bullet.updated_at,
            serde_json::to_string(&bullet.bc_bands)?,
            serde_json::to_string(&bullet.drag_curve)?,
            bullet
                .data_attribution
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            bullet
                .recipe
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        ],
    )?;
    Ok(())
}

pub(crate) fn put_load(connection: &Connection, load: &RifleLoad) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO rifle_loads (id, rifle_id, bullet_id, velocities, impact_offset_right_mrad,
             impact_offset_up_mrad, zero, notes, updated_at, truing)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (id) DO UPDATE SET
             rifle_id = excluded.rifle_id, bullet_id = excluded.bullet_id,
             velocities = excluded.velocities,
             impact_offset_right_mrad = excluded.impact_offset_right_mrad,
             impact_offset_up_mrad = excluded.impact_offset_up_mrad, zero = excluded.zero,
             notes = excluded.notes, updated_at = excluded.updated_at,
             truing = excluded.truing",
        params![
            load.id,
            load.rifle_id,
            load.bullet_id,
            serde_json::to_string(&load.velocities)?,
            load.impact_offset_mrad[0],
            load.impact_offset_mrad[1],
            serde_json::to_string(&load.zero)?,
            load.notes,
            load.updated_at,
            load.truing
                .map(|truing| serde_json::to_string(&truing))
                .transpose()?,
        ],
    )?;
    Ok(())
}

pub(crate) fn put_scope(connection: &Connection, scope: &UserScope) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO scopes (id, data, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
        params![scope.id, serde_json::to_string(scope)?, scope.updated_at],
    )?;
    Ok(())
}

pub(crate) fn put_target(connection: &Connection, target: &Target) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO targets (id, data, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
        params![target.id, serde_json::to_string(target)?, target.updated_at],
    )?;
    Ok(())
}

pub(crate) fn put_shot(connection: &Connection, shot: &Shot) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO shots (id, data, taken_at, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (id) DO UPDATE SET data = excluded.data, taken_at = excluded.taken_at,
             updated_at = excluded.updated_at",
        params![
            shot.id,
            serde_json::to_string(shot)?,
            shot.taken_at,
            shot.updated_at
        ],
    )?;
    Ok(())
}

/// Reads a JSON-encoded column into its Rust type.
pub(crate) fn from_json_column<T: serde::de::DeserializeOwned>(text: &str) -> rusqlite::Result<T> {
    serde_json::from_str(text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn invalid_text(column: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnType(
        0,
        format!("{column} = {value}"),
        rusqlite::types::Type::Text,
    )
}

pub(crate) fn rifle_from_row(row: &Row<'_>) -> rusqlite::Result<Rifle> {
    let click_unit: String = row.get("click_unit")?;
    Ok(Rifle {
        id: row.get("id")?,
        name: row.get("name")?,
        caliber: row.get("caliber")?,
        barrel_length_mm: row.get("barrel_length_mm")?,
        twist_in: row.get("twist_in")?,
        right_hand_twist: row.get("right_hand_twist")?,
        sight_height_cm: row.get("sight_height_cm")?,
        zero_range_m: row.get("zero_range_m")?,
        zero_height_cm: row.get("zero_height_cm")?,
        zero_temperature_c: row.get("zero_temperature_c")?,
        zero_pressure_hpa: row.get("zero_pressure_hpa")?,
        click_unit: ClickUnit::parse(&click_unit)
            .ok_or_else(|| invalid_text("click_unit", &click_unit))?,
        click_value: row.get("click_value")?,
        scope_id: row.get("scope_id")?,
        reticle_id: row.get("reticle_id")?,
        fitted_barrel_id: row.get("fitted_barrel_id")?,
        rounds_before_logging: row.get("rounds_before_logging")?,
        zero_powder_temperature_c: row.get("zero_powder_temperature_c")?,
        zero_humidity_percent: row.get("zero_humidity_percent")?,
        table: match row.get::<_, Option<String>>("table_settings")? {
            Some(text) => from_json_column(&text)?,
            None => TableSettings::default(),
        },
        notes: row.get("notes")?,
        updated_at: row.get("updated_at")?,
    })
}

pub(crate) fn bullet_from_row(row: &Row<'_>) -> rusqlite::Result<Bullet> {
    let kind: String = row.get("kind")?;
    let drag_model: String = row.get("drag_model")?;
    let construction: String = row.get("construction")?;
    Ok(Bullet {
        id: row.get("id")?,
        kind: match kind.as_str() {
            "factory" => BulletKind::Factory,
            "handload" => BulletKind::Handload,
            _ => return Err(invalid_text("kind", &kind)),
        },
        manufacturer: row.get("manufacturer")?,
        name: row.get("name")?,
        caliber: row.get("caliber")?,
        diameter_mm: row.get("diameter_mm")?,
        mass_g: row.get("mass_g")?,
        length_mm: row.get("length_mm")?,
        bc_g1: row.get("bc_g1")?,
        bc_g7: row.get("bc_g7")?,
        drag_model: match drag_model.as_str() {
            "G1" => BcModel::G1,
            "G7" => BcModel::G7,
            "Custom" => BcModel::Custom,
            _ => return Err(invalid_text("drag_model", &drag_model)),
        },
        bc_bands: from_json_column(&row.get::<_, String>("bc_bands")?)?,
        drag_curve: from_json_column(&row.get::<_, String>("drag_curve")?)?,
        muzzle_velocity_mps: row.get("muzzle_velocity_mps")?,
        velocity_temperature_c: row.get("velocity_temperature_c")?,
        velocity_per_degree: row.get("velocity_per_degree")?,
        test_barrel_mm: row.get("test_barrel_mm")?,
        construction: Construction::parse(&construction)
            .ok_or_else(|| invalid_text("construction", &construction))?,
        lead_free: row.get("lead_free")?,
        min_expansion_mps: row.get("min_expansion_mps")?,
        catalog_id: row.get("catalog_id")?,
        data_attribution: row
            .get::<_, Option<String>>("data_attribution")?
            .map(|text| from_json_column(&text))
            .transpose()?,
        recipe: row
            .get::<_, Option<String>>("recipe")?
            .map(|text| from_json_column(&text))
            .transpose()?,
        notes: row.get("notes")?,
        updated_at: row.get("updated_at")?,
    })
}

pub(crate) fn load_from_row(row: &Row<'_>) -> rusqlite::Result<RifleLoad> {
    let mut load = RifleLoad::new(
        &row.get::<_, String>("rifle_id")?,
        &row.get::<_, String>("bullet_id")?,
    );
    load.id = row.get("id")?;
    load.velocities = from_json_column(&row.get::<_, String>("velocities")?)?;
    load.impact_offset_mrad = [
        row.get("impact_offset_right_mrad")?,
        row.get("impact_offset_up_mrad")?,
    ];
    load.zero = from_json_column(&row.get::<_, String>("zero")?)?;
    load.notes = row.get("notes")?;
    load.updated_at = row.get("updated_at")?;
    load.truing = row
        .get::<_, Option<String>>("truing")?
        .map(|text| from_json_column(&text))
        .transpose()?;
    Ok(load)
}

//! Local SQLite storage for the hunter's own data. One file per device;
//! SQLite runs on every desktop and mobile platform.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::StoreError;
use crate::deletions::{Deletion, RecordKind};
use crate::model::{
    BcModel, Bullet, BulletKind, ClickUnit, Construction, Rifle, RifleLoad, TableSettings, now_ms,
};
use crate::optics::UserScope;
use crate::targets::Target;

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
];

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
            return Err(StoreError::Invalid(format!(
                "the database was created by a newer version of the app (schema {version})"
            )));
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
        put_rifle(&self.connection, rifle)
    }

    /// Deletes the rifle and the loads measured in it, remembering the
    /// deletions so a sync deletes them on other devices too.
    pub fn delete_rifle(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let at = self.stamp(0)?;
        let loads: Vec<String> = self.query_all(
            "SELECT id FROM rifle_loads WHERE rifle_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        for load in loads {
            record_deletion(&transaction, RecordKind::RifleLoad, &load, at)?;
        }
        record_deletion(&transaction, RecordKind::Rifle, id, at)?;
        transaction.execute("DELETE FROM rifles WHERE id = ?1", [id])?;
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
        put_bullet(&self.connection, bullet)
    }

    /// Deletes the bullet and any velocities measured with it.
    pub fn delete_bullet(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let at = self.stamp(0)?;
        let loads: Vec<String> = self.query_all(
            "SELECT id FROM rifle_loads WHERE bullet_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        for load in loads {
            record_deletion(&transaction, RecordKind::RifleLoad, &load, at)?;
        }
        record_deletion(&transaction, RecordKind::Bullet, id, at)?;
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
        put_scope(&self.connection, scope)
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
        put_target(&self.connection, target)
    }

    pub fn delete_target(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::Target, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM targets WHERE id = ?1", [id])?;
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
        put_load(&self.connection, load)
    }

    pub fn delete_rifle_load(&self, id: &str) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        record_deletion(&transaction, RecordKind::RifleLoad, id, self.stamp(0)?)?;
        transaction.execute("DELETE FROM rifle_loads WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
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
    let at = now_ms().max(previous + 1).max(clock(connection)? + 1);
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

pub(crate) fn put_rifle(connection: &Connection, rifle: &Rifle) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO rifles (id, name, caliber, barrel_length_mm, twist_in, right_hand_twist,
             sight_height_cm, zero_range_m, zero_temperature_c, zero_pressure_hpa, click_unit,
             click_value, notes, updated_at, scope_id, reticle_id, zero_powder_temperature_c,
             zero_humidity_percent, table_settings, zero_height_cm)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                 ?20)
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
             zero_height_cm = excluded.zero_height_cm",
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
        ],
    )?;
    Ok(())
}

pub(crate) fn put_bullet(connection: &Connection, bullet: &Bullet) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO bullets (id, kind, manufacturer, name, caliber, diameter_mm, mass_g,
             length_mm, bc_g1, bc_g7, drag_model, muzzle_velocity_mps, velocity_temperature_c,
             velocity_per_degree, test_barrel_mm, construction, lead_free, min_expansion_mps,
             catalog_id, notes, updated_at, bc_bands, drag_curve)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
             ?18, ?19, ?20, ?21, ?22, ?23)
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
             drag_curve = excluded.drag_curve",
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

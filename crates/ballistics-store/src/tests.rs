use crate::model::new_id;

fn measured(rifle: &str, bullet: &str, velocity_mps: f64, temperature_c: f64) -> RifleLoad {
    let mut load = RifleLoad::new(rifle, bullet);
    load.velocities.push(VelocityPoint {
        temperature_c,
        velocity_mps,
        shots: None,
    });
    load
}
use crate::*;

fn rifle(name: &str, caliber: &str) -> Rifle {
    Rifle::new(name, caliber, 10.0)
}

fn bullet(name: &str, caliber: &str) -> Bullet {
    let mut bullet = Bullet::new_manual(caliber, 7.82);
    bullet.manufacturer = "Test".into();
    bullet.name = name.into();
    bullet.mass_g = 11.7;
    bullet.bc_g1 = Some(0.45);
    bullet.muzzle_velocity_mps = 790.0;
    bullet
}

fn temporary_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ballistics-store-test-{}.sqlite", new_id()))
}

#[test]
fn data_survives_reopening_the_file() {
    let path = temporary_path();
    let mut saved = rifle("Tikka T3x", "308-win");
    Store::open(&path).unwrap().save_rifle(&mut saved).unwrap();
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.rifles().unwrap(), vec![saved]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unsafe_data_attribution_links_are_rejected_before_storage_or_import() {
    let source = DataAttribution {
        credit: "Example data collection".into(),
        source_url: "https://example.com/data".into(),
        source_revision: "1".into(),
        license: "CC-BY-SA-4.0".into(),
        license_url: "https://creativecommons.org/licenses/by-sa/4.0/".into(),
        changes: "Initial format conversion".into(),
    };
    for url in [
        "javascript:alert(1)",
        "data:text/html,test",
        "file:///tmp/test",
        "custom:run",
        "https:///no-host",
        "https://example.com/\n",
    ] {
        for license_link in [false, true] {
            let mut invalid = bullet("Attributed", "308-win");
            let mut attribution = source.clone();
            if license_link {
                attribution.license_url = url.into();
            } else {
                attribution.source_url = url.into();
            }
            invalid.data_attribution = Some(attribution);
            let mut store = Store::open_in_memory().unwrap();
            assert!(matches!(
                store.save_bullet(&mut invalid),
                Err(StoreError::Invalid(_))
            ));
            let data = transfer::PersonalData {
                format: transfer::FORMAT.into(),
                version: transfer::VERSION,
                bullets: vec![invalid],
                ..Default::default()
            };
            assert!(matches!(
                store.import_data(&data, false),
                Err(StoreError::Invalid(_))
            ));
            assert!(store.bullets().unwrap().is_empty());
        }
    }
}

#[test]
fn data_attribution_survives_storage_backups_and_sync_without_catalog_links() {
    let catalog = Catalog::builtin();
    let caliber = catalog.caliber("308-win").unwrap();
    let entry = catalog
        .bullets_for(caliber)
        .find(|entry| entry.data_attribution.is_some())
        .unwrap();
    let mut copied = entry.to_collection(caliber);
    let attribution = copied.data_attribution.clone();
    copied.muzzle_velocity_mps = 1.0; // A user-entered value is required for storage.
    copied.catalog_id = None;
    copied.notes.clear();
    let path = temporary_path();
    let source = Store::open(&path).unwrap();
    source.save_bullet(&mut copied).unwrap();
    drop(source);
    let source = Store::open(&path).unwrap();
    assert_eq!(
        source.bullet(&copied.id).unwrap().unwrap().data_attribution,
        attribution
    );
    let json = source.export_json().unwrap();
    let export: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(export["version"], transfer::VERSION);
    assert_eq!(
        export["bullets"][0]["data_attribution"]["license"],
        "CC-BY-SA-4.0"
    );
    let mut restored = Store::open_in_memory().unwrap();
    restored.import_json(&json).unwrap();
    assert_eq!(
        restored
            .bullet(&copied.id)
            .unwrap()
            .unwrap()
            .data_attribution,
        attribution
    );
    let mut synced = Store::open_in_memory().unwrap();
    synced.merge_sync(&source.export_data().unwrap()).unwrap();
    assert_eq!(
        synced.bullet(&copied.id).unwrap().unwrap().data_attribution,
        attribution
    );
    // A later personal edit must keep the attribution too.
    let mut changed = synced.bullet(&copied.id).unwrap().unwrap();
    changed.notes = "My own note".into();
    synced.save_bullet(&mut changed).unwrap();
    assert_eq!(
        synced.bullet(&copied.id).unwrap().unwrap().data_attribution,
        attribution
    );
    drop(source);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn the_database_before_data_attribution_upgrades_without_changing_bullets() {
    let path = temporary_path();
    let connection = rusqlite::Connection::open(&path).unwrap();
    let migrations = &crate::db::MIGRATIONS[..9];
    for migration in migrations {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 9).unwrap();
    connection
        .execute_batch(
            "INSERT INTO bullets (id, kind, manufacturer, name, caliber, diameter_mm, mass_g,
         bc_g1, drag_model, muzzle_velocity_mps, velocity_temperature_c, velocity_per_degree,
         construction, lead_free, notes, updated_at)
         VALUES ('old', 'handload', 'Test', 'Existing record', '308-win', 7.82, 1,
         0.1, 'G1', 1, 15, 0, 'other', 0, 'Existing note', 123);",
        )
        .unwrap();
    drop(connection);
    let upgraded = Store::open(&path).unwrap();
    let existing = upgraded.bullet("old").unwrap().unwrap();
    assert!(existing.data_attribution.is_none());
    assert_eq!(existing.name, "Existing record");
    assert_eq!(existing.notes, "Existing note");
    assert_eq!(existing.updated_at, 123);
    drop(upgraded);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn read_only_opening_reads_and_never_upgrades() {
    let path = temporary_path();
    let mut saved = rifle("Tikka T3x", "308-win");
    Store::open(&path).unwrap().save_rifle(&mut saved).unwrap();
    let reader = Store::open_read_only(&path).unwrap();
    assert_eq!(reader.rifles().unwrap(), vec![saved.clone()]);
    assert!(reader.save_rifle(&mut saved).is_err());
    drop(reader);
    // An older schema stays as it is.
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    drop(connection);
    assert!(matches!(
        Store::open_read_only(&path),
        Err(StoreError::Invalid(_))
    ));
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_database_set_aside_keeps_its_data_and_frees_its_place() {
    let folder = std::env::temp_dir().join(format!("mxsc-aside-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("mxsc.sqlite");
    let mut saved = rifle("Tikka T3x", "308-win");
    Store::open(&path).unwrap().save_rifle(&mut saved).unwrap();
    std::fs::write(folder.join("mxsc.sqlite-journal"), b"").unwrap();
    // 2026-11-13 17:59:12 UTC.
    let aside = Store::set_aside(&path, 1_794_592_752_000).unwrap();
    assert_eq!(aside, folder.join("mxsc-reset-2026-11-13-175912.sqlite"));
    assert!(!path.exists());
    assert!(
        folder
            .join("mxsc-reset-2026-11-13-175912.sqlite-journal")
            .exists()
    );
    assert!(!folder.join("mxsc.sqlite-journal").exists());
    std::fs::remove_file(folder.join("mxsc-reset-2026-11-13-175912.sqlite-journal")).unwrap();
    assert_eq!(Store::open(&aside).unwrap().rifles().unwrap(), vec![saved]);
    assert!(Store::open(&path).unwrap().rifles().unwrap().is_empty());
    // Never over an earlier one.
    assert!(Store::set_aside(&path, 1_794_592_752_000).is_err());
    assert!(path.exists());
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn only_a_damaged_file_counts_as_corrupt() {
    let path = temporary_path();
    std::fs::write(&path, vec![b'x'; 4096]).unwrap();
    let error = Store::open(&path).err().unwrap();
    assert!(error.is_corrupt(), "{error}");
    assert!(!StoreError::Newer(99).is_corrupt());
    assert!(!StoreError::Invalid("locked".into()).is_corrupt());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_newer_database_is_refused_but_its_settings_can_be_read() {
    let path = temporary_path();
    let store = Store::open(&path).unwrap();
    store.set_setting("check_for_updates", &true).unwrap();
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 999).unwrap();
    drop(connection);
    assert!(matches!(Store::open(&path), Err(StoreError::Newer(999))));
    assert!(matches!(
        Store::open_read_only(&path),
        Err(StoreError::Newer(999))
    ));
    assert_eq!(
        Store::peek_setting::<bool>(&path, "check_for_updates"),
        Some(true)
    );
    assert_eq!(Store::peek_setting::<bool>(&path, "missing"), None);
    // Peeking changes nothing.
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 999);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        Store::peek_setting::<bool>(&path, "check_for_updates"),
        None
    );
    assert!(!path.exists());
}

#[test]
fn saving_validates_and_stamps_records() {
    let store = Store::open_in_memory().unwrap();
    let mut unnamed = rifle(" ", "308-win");
    assert!(matches!(
        store.save_rifle(&mut unnamed),
        Err(StoreError::Invalid(_))
    ));

    let mut tikka = rifle("Tikka", "308-win");
    store.save_rifle(&mut tikka).unwrap();
    let first = tikka.updated_at;
    tikka.zero_range_m = 150.0;
    store.save_rifle(&mut tikka).unwrap();
    assert!(tikka.updated_at > first);
    assert_eq!(store.rifle(&tikka.id).unwrap().unwrap().zero_range_m, 150.0);
}

#[test]
fn only_bullets_in_the_rifles_caliber_are_offered() {
    let store = Store::open_in_memory().unwrap();
    for (name, caliber) in [
        ("Oryx", "308-win"),
        ("Vulcan", "308-win"),
        ("Ecostrike", "6-5x55-se"),
    ] {
        store.save_bullet(&mut bullet(name, caliber)).unwrap();
    }
    let names = |caliber| {
        store
            .bullets_for_caliber(caliber)
            .unwrap()
            .into_iter()
            .map(|b| b.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names("308-win"), ["Oryx", "Vulcan"]);
    assert_eq!(names("6-5x55-se"), ["Ecostrike"]);
    assert!(names("9-3x62").is_empty());
    assert_eq!(store.bullets().unwrap().len(), 3);
}

#[test]
fn a_bullet_needs_a_velocity_and_a_bc_for_its_drag_model() {
    let store = Store::open_in_memory().unwrap();
    let mut no_velocity = bullet("Oryx", "308-win");
    no_velocity.muzzle_velocity_mps = 0.0;
    assert!(store.save_bullet(&mut no_velocity).is_err());
    let mut wrong_model = bullet("Oryx", "308-win");
    wrong_model.drag_model = BcModel::G7;
    assert!(store.save_bullet(&mut wrong_model).is_err());
}

#[test]
fn measured_velocity_replaces_the_previous_one_and_follows_deletes() {
    let store = Store::open_in_memory().unwrap();
    let mut tikka = rifle("Tikka", "308-win");
    let mut oryx = bullet("Oryx", "308-win");
    store.save_rifle(&mut tikka).unwrap();
    store.save_bullet(&mut oryx).unwrap();

    store
        .save_rifle_load(&mut measured(&tikka.id, &oryx.id, 781.0, 12.0))
        .unwrap();
    store
        .save_rifle_load(&mut measured(&tikka.id, &oryx.id, 776.0, 8.0))
        .unwrap();
    let loads = store.rifle_loads().unwrap();
    assert_eq!(loads.len(), 1);
    assert_eq!(
        loads[0].velocities,
        [VelocityPoint {
            temperature_c: 8.0,
            velocity_mps: 776.0,
            shots: None
        }]
    );

    store.delete_bullet(&oryx.id).unwrap();
    assert!(store.rifle_loads().unwrap().is_empty());
    assert_eq!(store.rifles().unwrap().len(), 1);
}

#[test]
fn settings_round_trip_and_tolerate_old_shapes() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.setting::<f64>("temperature").unwrap(), None);
    store.set_setting("temperature", &-12.5).unwrap();
    store.set_setting("temperature", &-14.0).unwrap();
    assert_eq!(store.setting::<f64>("temperature").unwrap(), Some(-14.0));
    assert_eq!(store.setting::<Vec<String>>("temperature").unwrap(), None);
}

fn populated() -> (Store, Rifle, Bullet) {
    let store = Store::open_in_memory().unwrap();
    let mut tikka = rifle("Tikka", "308-win");
    let mut oryx = bullet("Oryx", "308-win");
    store.save_rifle(&mut tikka).unwrap();
    store.save_bullet(&mut oryx).unwrap();
    store
        .save_rifle_load(&mut measured(&tikka.id, &oryx.id, 781.0, 12.0))
        .unwrap();
    (store, tikka, oryx)
}

#[test]
fn export_then_import_recreates_the_data() {
    let (source, _, _) = populated();
    let json = source.export_json().unwrap();
    let mut target = Store::open_in_memory().unwrap();
    let report = target.import_json(&json).unwrap();
    assert_eq!(
        report,
        ImportReport {
            added: 3,
            updated: 0,
            unchanged: 0,
            kept_local: 0,
            deleted: 0
        }
    );
    assert_eq!(target.rifles().unwrap(), source.rifles().unwrap());
    assert_eq!(target.bullets().unwrap(), source.bullets().unwrap());
    assert_eq!(target.rifle_loads().unwrap(), source.rifle_loads().unwrap());

    let again = target.import_json(&json).unwrap();
    assert_eq!(
        again,
        ImportReport {
            added: 0,
            updated: 0,
            unchanged: 3,
            kept_local: 0,
            deleted: 0
        }
    );
}

#[test]
fn import_keeps_whichever_copy_was_edited_last() {
    let (phone, mut tikka, mut oryx) = populated();
    let mut laptop = Store::open_in_memory().unwrap();
    laptop.import_json(&phone.export_json().unwrap()).unwrap();

    // Edited on the laptop first, then the bullet on the phone.
    let mut laptop_rifle = laptop.rifle(&tikka.id).unwrap().unwrap();
    laptop_rifle.zero_range_m = 200.0;
    laptop.save_rifle(&mut laptop_rifle).unwrap();
    tikka.zero_range_m = 150.0;
    tikka.updated_at = laptop_rifle.updated_at - 1;
    oryx.muzzle_velocity_mps = 770.0;
    phone.save_bullet(&mut oryx).unwrap();
    phone.connection_for_tests_put_rifle(&tikka);

    let report = laptop.import_json(&phone.export_json().unwrap()).unwrap();
    assert_eq!(
        report,
        ImportReport {
            added: 0,
            updated: 1,
            unchanged: 1,
            kept_local: 1,
            deleted: 0
        }
    );
    assert_eq!(
        laptop.rifle(&tikka.id).unwrap().unwrap().zero_range_m,
        200.0
    );
    assert_eq!(
        laptop
            .bullet(&oryx.id)
            .unwrap()
            .unwrap()
            .muzzle_velocity_mps,
        770.0
    );
}

#[test]
fn a_bad_file_imports_nothing() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(matches!(
        store.import_json("{\"format\":\"other\"}"),
        Err(StoreError::Json(_))
    ));

    let (source, _, _) = populated();
    let mut data: serde_json::Value = serde_json::from_str(&source.export_json().unwrap()).unwrap();
    data["bullets"][0]["mass_g"] = serde_json::json!(0.0);
    assert!(matches!(
        store.import_json(&data.to_string()),
        Err(StoreError::Invalid(_))
    ));
    assert!(
        store.rifles().unwrap().is_empty(),
        "rifles were imported before the bad bullet"
    );

    data["format"] = serde_json::json!("ballistic-calculator/personal-data");
    data["bullets"][0]["mass_g"] = serde_json::json!(11.7);
    assert!(
        store.import_json(&data.to_string()).is_ok(),
        "backups from before the rename still import"
    );

    data["format"] = serde_json::json!("something-else");
    assert!(matches!(
        store.import_json(&data.to_string()),
        Err(StoreError::Invalid(_))
    ));
}

impl Store {
    /// Writes a rifle with its timestamp untouched, to simulate an edit made
    /// on another device at a given time.
    fn connection_for_tests_put_rifle(&self, rifle: &Rifle) {
        let mut copy = self.rifle(&rifle.id).unwrap().unwrap();
        copy.zero_range_m = rifle.zero_range_m;
        copy.updated_at = rifle.updated_at;
        crate::db::put_rifle(self.connection_ref(), &copy).unwrap();
    }
}

/// The development fixture (dev-data/sample.json, loaded by `cargo seed`)
/// must keep importing as the model changes, and stay useful: every rifle
/// has bullets and every catalog link resolves.
#[test]
fn development_fixture_imports_cleanly() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store
        .import_json(include_str!("../../../dev-data/sample.json"))
        .unwrap();
    assert_eq!(report.kept_local, 0);
    let catalog = Catalog::builtin();
    for rifle in store.rifles().unwrap() {
        assert!(
            catalog.caliber(&rifle.caliber).is_some(),
            "{}: unknown caliber",
            rifle.name
        );
        let scope = catalog
            .scope(rifle.scope_id.as_deref().unwrap())
            .expect("scope in the catalog");
        assert!(
            scope.reticles.contains(rifle.reticle_id.as_ref().unwrap()),
            "{}: reticle not offered",
            rifle.name
        );
        assert!(
            !store
                .bullets_for_caliber(&rifle.caliber)
                .unwrap()
                .is_empty(),
            "{}: no bullets",
            rifle.name
        );
    }
    for bullet in store.bullets().unwrap() {
        if let Some(id) = &bullet.catalog_id {
            assert!(
                catalog.bullet(id).is_some(),
                "{}: catalog entry {id} is gone",
                bullet.name
            );
        }
    }
    assert_eq!(store.rifle_loads().unwrap().len(), 2);
}

#[test]
fn a_database_from_the_previous_version_is_upgraded() {
    let path = temporary_path();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch(crate::db::MIGRATIONS[0]).unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    connection
        .execute(
            "INSERT INTO rifles VALUES ('r', 'Old rifle', '308-win', NULL, 10, 1, 4.5, 100, 15, 1013, 'mil', 0.1, '', 1)",
            [],
        )
        .unwrap();
    drop(connection);

    let store = Store::open(&path).unwrap();
    let mut rifle = store.rifle("r").unwrap().unwrap();
    assert_eq!(rifle.scope_id, None);
    rifle.scope_id = Some("some-scope".into());
    store.save_rifle(&mut rifle).unwrap();
    assert_eq!(
        store.rifle("r").unwrap().unwrap().scope_id.as_deref(),
        Some("some-scope")
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn new_fields_round_trip_through_the_database() {
    let store = Store::open_in_memory().unwrap();
    let mut rifle = rifle("Tikka", "308-win");
    rifle.zero_powder_temperature_c = Some(-5.0);
    rifle.zero_humidity_percent = 80.0;
    rifle.table = TableSettings {
        start_m: 100.0,
        end_m: 600.0,
        step_m: 25.0,
        columns: TableColumns {
            drop_moa: true,
            time: false,
            ..TableColumns::default()
        },
    };
    store.save_rifle(&mut rifle).unwrap();
    let mut oryx = bullet("Oryx", "308-win");
    oryx.bc_bands = vec![
        BcBand {
            velocity_mps: 800.0,
            bc: 0.36,
        },
        BcBand {
            velocity_mps: 500.0,
            bc: 0.33,
        },
    ];
    store.save_bullet(&mut oryx).unwrap();
    let mut load = measured(&rifle.id, &oryx.id, 781.0, 12.0);
    load.velocities.push(VelocityPoint {
        temperature_c: -10.0,
        velocity_mps: 768.0,
        shots: None,
    });
    load.impact_offset_mrad = [0.1, -0.2];
    load.zero = LoadZero::Recorded(ZeroWeather {
        temperature_c: 5.0,
        powder_temperature_c: 3.0,
        pressure_hpa: 990.0,
        humidity_percent: 60.0,
    });
    load.truing = Some(Truing {
        method: TruingMethod::Drag,
        range_m: 450.0,
        elevation_mrad: 4.6,
        inclination_degrees: -3.0,
        weather: ZeroWeather {
            temperature_c: -4.0,
            powder_temperature_c: -4.0,
            pressure_hpa: 1002.0,
            humidity_percent: 80.0,
        },
        factor: 1.06,
    });
    store.save_rifle_load(&mut load).unwrap();

    assert_eq!(store.rifle(&rifle.id).unwrap().unwrap(), rifle);
    assert_eq!(store.bullet(&oryx.id).unwrap().unwrap(), oryx);
    assert_eq!(store.rifle_loads().unwrap(), vec![load]);
    assert_eq!(rifle.table.ranges().collect::<Vec<_>>().len(), 21);
}

#[test]
fn invalid_bc_bands_and_table_settings_are_refused() {
    let store = Store::open_in_memory().unwrap();
    let mut duplicate = bullet("Oryx", "308-win");
    duplicate.bc_bands = vec![
        BcBand {
            velocity_mps: 800.0,
            bc: 0.36,
        },
        BcBand {
            velocity_mps: 800.0,
            bc: 0.33,
        },
    ];
    assert!(store.save_bullet(&mut duplicate).is_err());
    let mut backwards = rifle("Tikka", "308-win");
    backwards.table.end_m = 0.0;
    assert!(store.save_rifle(&mut backwards).is_err());
}

#[test]
fn velocity_model_uses_the_bullet_one_measurement_or_a_fitted_line() {
    let mut oryx = bullet("Oryx", "308-win");
    oryx.velocity_per_degree = 0.5;
    let mut load = RifleLoad::new("r", "b");
    assert_eq!(load.velocity_model(&oryx).velocity_at(15.0), 790.0);

    load.velocities.push(VelocityPoint {
        temperature_c: 20.0,
        velocity_mps: 800.0,
        shots: None,
    });
    let one = load.velocity_model(&oryx);
    assert_eq!(
        (one.velocity_at(20.0), one.velocity_at(0.0)),
        (800.0, 790.0)
    );

    load.velocities.push(VelocityPoint {
        temperature_c: 0.0,
        velocity_mps: 780.0,
        shots: None,
    });
    load.velocities.push(VelocityPoint {
        temperature_c: -20.0,
        velocity_mps: 760.0,
        shots: None,
    });
    let line = load.velocity_model(&oryx);
    assert!((line.per_degree - 1.0).abs() < 1e-12);
    assert!((line.velocity_at(10.0) - 790.0).abs() < 1e-9);
    assert!((line.percent_per_15c() - 15.0 / 780.0 * 100.0).abs() < 1e-9);
}

#[test]
fn measured_velocities_from_the_previous_schema_move_into_the_list() {
    let path = temporary_path();
    let connection = rusqlite::Connection::open(&path).unwrap();
    for migration in &crate::db::MIGRATIONS[..2] {
        connection.execute_batch(migration).unwrap();
    }
    connection.pragma_update(None, "user_version", 2).unwrap();
    connection.execute_batch(
        "INSERT INTO rifles VALUES ('r', 'Old', '308-win', NULL, 10, 1, 4.5, 100, 15, 1013, 'mil', 0.1, '', 1, NULL, NULL);
         INSERT INTO bullets VALUES ('b', 'factory', 'Norma', 'Oryx', '308-win', 7.82, 11.7, NULL, 0.354, NULL, 'G1',
             796, 15, 0.5, NULL, 'bonded', 0, NULL, NULL, '', 1);
         INSERT INTO rifle_loads VALUES ('l', 'r', 'b', 781, 12, '', 1);",
    )
    .unwrap();
    drop(connection);

    let store = Store::open(&path).unwrap();
    let load = &store.rifle_loads().unwrap()[0];
    assert_eq!(
        load.velocities,
        [VelocityPoint {
            temperature_c: 12.0,
            velocity_mps: 781.0,
            shots: None
        }]
    );
    assert_eq!(load.zero, LoadZero::Rifle);
    assert_eq!(
        store.rifle("r").unwrap().unwrap().table,
        TableSettings::default()
    );
    assert!(store.bullet("b").unwrap().unwrap().bc_bands.is_empty());
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn exports_in_the_first_format_still_import() {
    let mut store = Store::open_in_memory().unwrap();
    let report = store
        .import_json(
            r#"{"format":"ballistic-calculator/personal-data","version":1,"exported_at":1,
                "rifles":[{"id":"r","name":"Old","caliber":"308-win","barrel_length_mm":null,"twist_in":10.0,
                    "right_hand_twist":true,"sight_height_cm":4.5,"zero_range_m":100.0,"zero_temperature_c":15.0,
                    "zero_pressure_hpa":1013.25,"click_unit":"mil","click_value":0.1,"notes":"","updated_at":1}],
                "bullets":[{"id":"b","kind":"factory","manufacturer":"Norma","name":"Oryx","caliber":"308-win",
                    "diameter_mm":7.82,"mass_g":11.7,"length_mm":null,"bc_g1":0.354,"bc_g7":null,"drag_model":"G1",
                    "muzzle_velocity_mps":796.0,"velocity_temperature_c":15.0,"velocity_per_degree":0.5,
                    "test_barrel_mm":null,"construction":"bonded","lead_free":false,"min_expansion_mps":null,
                    "catalog_id":null,"notes":"","updated_at":1}],
                "rifle_loads":[{"id":"l","rifle_id":"r","bullet_id":"b","muzzle_velocity_mps":781.0,
                    "velocity_temperature_c":12.0,"notes":"","updated_at":1}]}"#,
        )
        .unwrap();
    assert_eq!(report.added, 3);
    let load = &store.rifle_loads().unwrap()[0];
    assert_eq!(
        load.velocities,
        [VelocityPoint {
            temperature_c: 12.0,
            velocity_mps: 781.0,
            shots: None
        }]
    );
    let exported = store.export_json().unwrap();
    assert!(
        !exported.contains("\"velocity_temperature_c\":12"),
        "legacy fields are not written back"
    );
}

#[test]
fn custom_scopes_save_export_import_and_leave_rifles_when_deleted() {
    let mut store = Store::open_in_memory().unwrap();
    let mut scope = UserScope::new();
    scope.manufacturer = "Zeiss".into();
    scope.model = "Diavari 6x42".into();
    scope.magnification_min = 6.0;
    scope.magnification_max = 6.0;
    scope.reticle_reference_magnification = Some(6.0);
    store.save_scope(&mut scope).unwrap();
    let mut tikka = rifle("Tikka", "308-win");
    tikka.scope_id = Some(scope.id.clone());
    tikka.reticle_id = Some(PLAIN_CROSSHAIR.into());
    store.save_rifle(&mut tikka).unwrap();
    assert_eq!(store.scopes().unwrap(), vec![scope.clone()]);

    let mut copy = Store::open_in_memory().unwrap();
    let report = copy.import_json(&store.export_json().unwrap()).unwrap();
    assert_eq!(report.added, 2);
    assert_eq!(copy.scopes().unwrap(), vec![scope.clone()]);

    store.delete_scope(&scope.id).unwrap();
    assert!(store.scopes().unwrap().is_empty());
    let rifle = store.rifle(&tikka.id).unwrap().unwrap();
    assert_eq!((rifle.scope_id, rifle.reticle_id), (None, None));
    assert!(
        rifle.updated_at > tikka.updated_at,
        "the change must win a later merge"
    );
}

#[test]
fn a_custom_drag_curve_replaces_the_bc() {
    let store = Store::open_in_memory().unwrap();
    let mut scenar = bullet("Scenar", "308-win");
    scenar.bc_g1 = None;
    scenar.drag_model = BcModel::Custom;
    assert!(
        store.save_bullet(&mut scenar.clone()).is_err(),
        "needs a curve"
    );
    scenar.drag_curve = vec![
        DragPoint {
            mach: 0.5,
            cd: 0.16,
        },
        DragPoint {
            mach: 1.0,
            cd: 0.38,
        },
        DragPoint {
            mach: 2.5,
            cd: 0.27,
        },
    ];
    store.save_bullet(&mut scenar).unwrap();
    assert_eq!(store.bullet(&scenar.id).unwrap().unwrap(), scenar);
}

#[test]
fn a_dry_run_reports_without_writing() {
    let (source, _, _) = populated();
    let mut target = Store::open_in_memory().unwrap();
    let data: crate::transfer::PersonalData =
        serde_json::from_str(&source.export_json().unwrap()).unwrap();
    let preview = target.import_data(&data, true).unwrap();
    assert_eq!(preview.added, 3);
    assert!(target.rifles().unwrap().is_empty() && target.bullets().unwrap().is_empty());
    assert_eq!(target.import_data(&data, false).unwrap(), preview);
}

#[test]
fn custom_targets_save_export_and_import_once() {
    let store = Store::open_in_memory().unwrap();
    let mut roe = Catalog::builtin().target("roe-deer").unwrap().to_custom();
    roe.name = "Roe buck, home forest".into();
    roe.vital_diameter_cm = 12.0;
    store.save_target(&mut roe).unwrap();
    assert_eq!(store.targets().unwrap(), vec![roe.clone()]);
    assert_eq!(roe.catalog_id.as_deref(), Some("roe-deer"));

    let mut nameless = Target::new_custom();
    assert!(store.save_target(&mut nameless).is_err());

    let export = store.export_json().unwrap();
    let mut copy = Store::open_in_memory().unwrap();
    assert_eq!(copy.import_json(&export).unwrap().added, 1);
    assert_eq!(copy.import_json(&export).unwrap().unchanged, 1);
    assert_eq!(copy.targets().unwrap(), vec![roe.clone()]);

    store.delete_target(&roe.id).unwrap();
    assert!(store.targets().unwrap().is_empty());
}

#[test]
fn catalog_drawings_are_sound() {
    let catalog = Catalog::builtin();
    let ids: std::collections::HashSet<_> = catalog.drawings.iter().map(|d| &d.id).collect();
    assert_eq!(ids.len(), catalog.drawings.len(), "duplicate drawing id");
    for drawing in &catalog.drawings {
        assert!(
            drawing.shoulder_height_m > 0.0 && drawing.length_m > 0.0,
            "{}",
            drawing.id
        );
        for part in &drawing.parts {
            assert!(part.points.len() >= 3, "{}", drawing.id);
            assert!(
                part.points.iter().all(|p| p[1] >= -1e-9),
                "{} below ground",
                drawing.id
            );
            assert!(
                part.fill.len() == 7 && part.fill.starts_with('#'),
                "{}: fill {}",
                drawing.id,
                part.fill
            );
        }
        // The aim point lies in the widest zone, and the training rings'
        // centre too; every zone is a real polygon.
        if let Some(widest) = drawing.zones.last() {
            assert!(
                crate::drawings::contains(&widest.points, drawing.vital),
                "{}: aim point outside the {}",
                drawing.id,
                widest.name
            );
            for zone in &drawing.zones {
                assert!(zone.points.len() >= 3, "{}: {}", drawing.id, zone.name);
            }
            if let Some(rings) = &drawing.rings {
                assert!(crate::drawings::contains(&widest.points, rings.center));
                assert!(
                    rings
                        .rings
                        .windows(2)
                        .all(|w| w[0].diameter_m < w[1].diameter_m)
                );
            }
        }
    }
    // Ids from before the drawings were data still find one.
    assert_eq!(
        catalog.drawing("no-such-drawing").id,
        catalog.drawings[0].id
    );
}

#[test]
fn catalog_bodies_are_closed_and_fit_their_drawings() {
    let catalog = Catalog::builtin();
    for body in &catalog.bodies {
        let drawing = catalog.drawing(&body.id);
        assert_eq!(drawing.id, body.id, "a body without a drawing");
        assert_eq!(body.triangles.len(), body.face_colour.len(), "{}", body.id);
        assert!(
            body.face_colour
                .iter()
                .all(|&c| (c as usize) < body.colours.len()),
            "{}",
            body.id
        );
        let n = body.vertices.len() as u32;
        assert!(
            body.triangles.iter().flatten().all(|&k| k < n),
            "{}",
            body.id
        );
        assert!(
            body.antler_faces
                .iter()
                .all(|&f| (f as usize) < body.triangles.len()),
            "{}",
            body.id
        );
        assert_eq!(
            drawing.has_antlers(),
            !body.antler_faces.is_empty(),
            "{}",
            body.id
        );
        // Closed: every edge is shared by two triangles running opposite
        // ways, so leaving out the faces turned away never shows a hole.
        let mut edges = std::collections::HashMap::new();
        for &[a, b, c] in &body.triangles {
            for edge in [(a, b), (b, c), (c, a)] {
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        let open = edges
            .iter()
            .filter(|&(&(a, b), &count)| count != 1 || edges.get(&(b, a)) != Some(&1))
            .count();
        assert_eq!(open, 0, "{}: {open} open or doubled edges", body.id);
        // Outward: positive volume.
        let volume: f64 = body
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|k| body.vertices[k as usize].map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum();
        assert!(volume > 0.0, "{}: inside out", body.id);
        // Placed, it is about its drawing's size (the 3D view stands it on
        // its own hooves) and holds the whole hit area.
        let placement = body.placement;
        assert!(
            (0.6..1.5).contains(&placement.scale),
            "{}: scale {}",
            body.id,
            placement.scale
        );
        for zone in &drawing.zones {
            let on = zone
                .points
                .iter()
                .filter(|&&point| {
                    body.near_side(placement.undo(drawing.vital, point))
                        .is_some_and(|z| z > 0.0)
                })
                .count();
            assert_eq!(
                on,
                zone.points.len(),
                "{}: {} off the body",
                body.id,
                zone.name
            );
        }
        // Without zones the app draws a vital circle round the aim point:
        // on the animal as the hunter sees it.
        assert!(
            body.near_side(placement.undo(drawing.vital, drawing.vital))
                .is_some(),
            "{}: the aim point is off the body",
            body.id
        );
    }
}

#[test]
fn catalog_targets_are_valid() {
    let catalog = Catalog::builtin();
    let ids: std::collections::HashSet<_> = catalog.targets.iter().map(|t| &t.id).collect();
    assert_eq!(ids.len(), catalog.targets.len(), "duplicate target id");
    assert!(catalog.target("moose").is_some());
    for target in &catalog.targets {
        target.validate().unwrap();
        // Game has an energy guideline; sport targets have none.
        assert_eq!(
            target.is_game(),
            target.min_energy_j.is_some(),
            "{}",
            target.name
        );
        // Every target sits in a list section.
        assert!(target.group.is_some(), "{}", target.name);
        match &target.shape {
            TargetShape::Animal { silhouette, .. } => {
                assert!(target.is_game(), "{}", target.name);
                assert!(
                    catalog.drawings.iter().any(|d| d.id == *silhouette),
                    "{}: no drawing {silhouette}",
                    target.name
                );
            }
            TargetShape::Rings { black_mm, .. } => {
                assert!(
                    (target.vital_diameter_cm * 10.0 - black_mm).abs() < 1e-9,
                    "{}",
                    target.name
                );
            }
            TargetShape::Plate { diameter_mm } => {
                assert!(
                    (target.vital_diameter_cm * 10.0 - diameter_mm).abs() < 1e-9,
                    "{}",
                    target.name
                );
            }
        }
    }
    // Swedish names find the species.
    let found: Vec<_> = catalog
        .targets
        .iter()
        .filter(|t| t.matches("tjäder"))
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(found, ["capercaillie"]);
    let issf50 = catalog.target("issf-50m-rifle").unwrap();
    let rings = issf50.shape.ring_diameters_mm();
    assert_eq!(rings.len(), 10);
    assert!((rings[9] - 154.4).abs() < 1e-9);
}

#[test]
fn backups_from_the_earlier_app_names_still_import() {
    let (store, _, _) = populated();
    let export = store.export_json().unwrap();
    assert!(export.contains("\"mxsc/personal-data\""));
    for old in ["mhc/personal-data", "ballistic-calculator/personal-data"] {
        let mut copy = Store::open_in_memory().unwrap();
        let renamed = export.replace("mxsc/personal-data", old);
        assert!(copy.import_json(&renamed).unwrap().added > 0, "{old}");
    }
    let mut copy = Store::open_in_memory().unwrap();
    assert!(
        copy.import_json(&export.replace("mxsc/personal-data", "other/app"))
            .is_err()
    );
}

#[test]
fn demo_data_loads_once() {
    let mut store = Store::open_in_memory().unwrap();
    let first = store.load_demo().unwrap();
    assert!(first.added >= 10, "{first:?}");
    let again = store.load_demo().unwrap();
    assert_eq!((again.added, again.updated), (0, 0));
    assert_eq!(store.rifles().unwrap().len(), 3);
}

// Sync ---------------------------------------------------------------------

/// The shared copy a server or a cloud file would hold.
#[derive(Default)]
struct Shared {
    data: Option<Vec<u8>>,
    writes: usize,
}

/// One round of sync for a device; returns what merging changed there.
fn sync(store: &mut Store, key: &SyncKey, shared: &mut Shared) -> ImportReport {
    let round = store.sync_round(Some(key), shared.data.as_deref()).unwrap();
    if let Some(upload) = round.upload {
        shared.data = Some(upload);
        shared.writes += 1;
    }
    round.report
}

/// The records two devices hold, without times or order.
fn same_data(a: &Store, b: &Store) -> bool {
    let records = |store: &Store| {
        let data = store.export_data().unwrap();
        let mut rifles: Vec<_> = data
            .rifles
            .iter()
            .map(|r| (r.id.clone(), r.name.clone(), r.updated_at))
            .collect();
        let mut bullets: Vec<_> = data
            .bullets
            .iter()
            .map(|b| (b.id.clone(), b.updated_at))
            .collect();
        let mut deleted: Vec<_> = data
            .deleted
            .iter()
            .map(|d| (d.kind.as_str(), d.id.clone()))
            .collect();
        rifles.sort();
        bullets.sort();
        deleted.sort();
        (rifles, bullets, deleted, data.rifle_loads.len())
    };
    records(a) == records(b)
}

#[test]
fn sync_keys_read_back_in_any_spelling() {
    let key = SyncKey::generate();
    let shown = key.to_string();
    assert_eq!(shown.len(), 26 + 6, "{shown}");
    assert_eq!(SyncKey::parse(&shown), Some(key.clone()));
    assert_eq!(
        SyncKey::parse(&shown.to_lowercase().replace('-', " ")),
        Some(key.clone())
    );
    let link = format!("https://example.com/app/#sync={}", key.compact());
    assert_eq!(SyncKey::parse(&link), Some(key.clone()));
    assert_ne!(SyncKey::generate(), key);
    assert_eq!(SyncKey::parse("not a key"), None);
    assert_eq!(SyncKey::parse(&shown[..20]), None);
    // The place the copy is kept doesn't show the key.
    assert_eq!(key.blob_id().len(), 64);
    assert!(!key.blob_id().contains(&key.compact().to_lowercase()));
}

#[test]
fn a_sealed_copy_opens_only_with_its_key() {
    let key = SyncKey::generate();
    let sealed = key.seal(b"rifles");
    assert_eq!(key.open(&sealed).unwrap(), b"rifles");
    assert_ne!(key.seal(b"rifles"), sealed, "a fresh nonce each time");
    assert!(SyncKey::generate().open(&sealed).is_err());
    let mut tampered = sealed.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(key.open(&tampered).is_err());
    assert!(key.open(b"MXS1").is_err());
}

/// A copy sealed by version 0.8.3 (chacha20poly1305 0.10, sha2 0.10) still
/// opens: crate updates mustn't change the format devices share.
#[test]
fn a_copy_sealed_by_an_earlier_version_still_opens() {
    let key = SyncKey::parse("Q3N2-19H2-MNSF-FQYB-SV0B-4WSQ-F8").unwrap();
    let hex = "4d585331dd3a197ca26748568a037c4495646bdb4c94172bc47a442d3ee018d30536645243e39b\
               d085f7c68fed4f60282d9b3df57b7c2ac760";
    let sealed: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    assert_eq!(key.open(&sealed).unwrap(), b"made by 0.8.3");
}

#[test]
fn two_devices_converge_and_deletions_stay_deleted() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let (mut phone, tikka, oryx) = populated();
    let mut desktop = Store::open_in_memory().unwrap();

    // The phone has data; the desktop starts empty and gets it.
    sync(&mut phone, &key, &mut shared);
    assert_eq!(shared.writes, 1);
    assert!(sync(&mut desktop, &key, &mut shared).changed());
    assert!(same_data(&phone, &desktop));
    assert_eq!(shared.writes, 1, "nothing new to write back");

    // The desktop renames the rifle, the phone deletes the bullet.
    let mut renamed = desktop.rifle(&tikka.id).unwrap().unwrap();
    renamed.name = "Tikka T3x Hunter".into();
    desktop.save_rifle(&mut renamed).unwrap();
    phone.delete_bullet(&oryx.id).unwrap();
    for _ in 0..2 {
        sync(&mut desktop, &key, &mut shared);
        sync(&mut phone, &key, &mut shared);
    }
    assert!(same_data(&phone, &desktop));
    assert_eq!(
        phone.rifle(&tikka.id).unwrap().unwrap().name,
        "Tikka T3x Hunter"
    );
    assert!(
        desktop.bullet(&oryx.id).unwrap().is_none(),
        "deleted everywhere"
    );
    assert!(desktop.rifle_loads().unwrap().is_empty(), "with its loads");

    // Rounds with nothing new write nothing.
    let writes = shared.writes;
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    assert_eq!(shared.writes, writes);
}

#[test]
fn plain_copies_sync_too() {
    // As in Google Drive: no key, plain JSON.
    let (mut phone, tikka, _) = populated();
    let mut desktop = Store::open_in_memory().unwrap();
    let round = phone.sync_round(None, None).unwrap();
    let copy = round.upload.unwrap();
    assert!(String::from_utf8(copy.clone()).unwrap().contains(&tikka.id));
    let round = desktop.sync_round(None, Some(&copy)).unwrap();
    assert!(round.report.changed());
    assert!(round.upload.is_none());
    assert!(same_data(&phone, &desktop));
}

#[test]
fn an_edit_after_a_deletion_keeps_the_record() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let (mut phone, tikka, _) = populated();
    sync(&mut phone, &key, &mut shared);
    let mut desktop = Store::open_in_memory().unwrap();
    sync(&mut desktop, &key, &mut shared);

    phone.delete_rifle(&tikka.id).unwrap();
    // The desktop hasn't heard, and edits the rifle afterwards (a deletion
    // and an edit in the same millisecond would go to the deletion).
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut edited = desktop.rifle(&tikka.id).unwrap().unwrap();
    edited.zero_range_m = 200.0;
    desktop.save_rifle(&mut edited).unwrap();

    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    assert_eq!(phone.rifle(&tikka.id).unwrap().unwrap().zero_range_m, 200.0);
    assert!(same_data(&phone, &desktop));
}

#[test]
fn restoring_a_backup_brings_deleted_records_back() {
    let (mut store, tikka, _) = populated();
    let backup = store.export_json().unwrap();
    store.delete_rifle(&tikka.id).unwrap();
    assert_eq!(
        store.deletions().unwrap().len(),
        1,
        "the rifle; its load goes with it"
    );
    let report = store.import_json(&backup).unwrap();
    assert!(report.added >= 1);
    let restored = store.rifle(&tikka.id).unwrap().unwrap();
    assert!(
        restored.updated_at > tikka.updated_at,
        "restored as a new edit"
    );
    assert!(store.deletions().unwrap().iter().all(|d| d.id != tikka.id));
}

#[test]
fn the_clock_never_runs_behind_merged_data() {
    let (mut store, tikka, _) = populated();
    let mut future = tikka.clone();
    future.name = "From a device a day ahead".into();
    future.updated_at = model::now_ms() + 86_400_000;
    let mut data = store.export_data().unwrap();
    data.rifles = vec![future.clone()];
    data.rifle_loads.clear();
    store.merge_sync(&data).unwrap();
    // An edit here, now, must still beat the copy it was made from.
    let mut edited = store.rifle(&tikka.id).unwrap().unwrap();
    edited.name = "Edited here".into();
    store.save_rifle(&mut edited).unwrap();
    assert!(edited.updated_at > future.updated_at);
    let mut other = rifle("Sako", "308-win");
    store.save_rifle(&mut other).unwrap();
    assert!(other.updated_at > future.updated_at);
}

#[test]
fn copies_saved_at_the_same_moment_settle_on_one() {
    let mut a = Store::open_in_memory().unwrap();
    let mut b = Store::open_in_memory().unwrap();
    let mut first = rifle("Tikka", "308-win");
    first.updated_at = 1_000;
    let mut second = first.clone();
    second.name = "Sako".into();
    let data = |rifle: &Rifle| transfer::PersonalData {
        format: transfer::FORMAT.into(),
        version: transfer::VERSION,
        rifles: vec![rifle.clone()],
        ..Default::default()
    };
    a.merge_sync(&data(&first)).unwrap();
    b.merge_sync(&data(&second)).unwrap();
    a.merge_sync(&b.export_data().unwrap()).unwrap();
    b.merge_sync(&a.export_data().unwrap()).unwrap();
    assert_eq!(a.rifles().unwrap(), b.rifles().unwrap());
}

#[test]
fn a_swedish_class_from_an_older_backup_becomes_the_swedish_level() {
    let mut store = Store::open_in_memory().unwrap();
    let old = r#"{"format":"mxsc/personal-data","version":4,"exported_at":0,
        "rifles":[],"bullets":[],"rifle_loads":[],"scopes":[],
        "targets":[{"id":"t1","name":"My roe","kind":"game","shape":"animal","silhouette":"roe-deer",
          "shoulder_height_cm":70,"body_length_cm":115,"antlers":true,"vital_diameter_cm":15,
          "min_energy_j":800,"swedish_class":2,"updated_at":5}]}"#;
    store.import_json(old).unwrap();
    let target = store.targets().unwrap().remove(0);
    assert_eq!(target.ammunition_level(Country::Sweden), Some(2));
    assert_eq!(target.ammunition_level(Country::Finland), None);
    let exported = store.export_json().unwrap();
    assert!(exported.contains(r#""sweden": 2"#), "{exported}");
    assert!(!exported.contains("swedish_class"));
}

#[test]
fn a_time_from_the_future_doesnt_beat_later_edits() {
    // Found by stress testing: a backup stamped at the end of time made
    // every later local edit lose to it.
    let store = Store::open_in_memory().unwrap();
    let mut x_bolt = rifle("Future X-Bolt", "308-win");
    store.save_rifle(&mut x_bolt).unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&store.export_json().unwrap()).unwrap();
    data["rifles"][0]["updated_at"] = serde_json::json!(i64::MAX);
    let future = data.to_string();

    let mut fresh = Store::open_in_memory().unwrap();
    fresh.import_json(&future).unwrap();
    let mut edited = fresh.rifles().unwrap().remove(0);
    edited.name = "Future X-Bolt v2".into();
    fresh.save_rifle(&mut edited).unwrap();
    fresh.import_json(&future).unwrap();
    assert_eq!(fresh.rifles().unwrap()[0].name, "Future X-Bolt v2");
}

#[test]
fn temperature_sensitivity_needs_a_spread_and_stays_believable() {
    // Found by stress testing: 0.5 °C apart gave 2900 m/s per °C.
    let bullet = bullet("Oryx", "308-win");
    let mut load = RifleLoad::new("rifle", &bullet.id);
    for (temperature_c, velocity_mps) in [(5.0, 50.0), (5.5, 1500.0)] {
        load.velocities.push(VelocityPoint {
            temperature_c,
            velocity_mps,
            shots: None,
        });
    }
    let model = load.velocity_model(&bullet);
    assert!(!model.fitted);
    assert_eq!(model.per_degree, bullet.velocity_per_degree);
    load.velocities[1].temperature_c = 15.0;
    let model = load.velocity_model(&bullet);
    assert!(model.fitted);
    assert_eq!(model.per_degree, crate::model::MAX_PER_DEGREE);
}

// The shot log ---------------------------------------------------------------

/// A shot at a moose at 150 m with the rifle and bullet, as Calculate logs it.
fn logged_shot(rifle: &Rifle, bullet: &Bullet, taken_at: i64) -> Shot {
    Shot {
        id: new_id(),
        taken_at,
        rifle_id: rifle.id.clone(),
        rifle_name: rifle.name.clone(),
        bullet_id: bullet.id.clone(),
        bullet_name: bullet.display_name(),
        target_id: Some("moose".into()),
        target_name: "Moose".into(),
        range_m: 150.0,
        inclination_degrees: 0.0,
        wind_speed_mps: 3.0,
        wind_clock: 3.0,
        weather: ShotWeather {
            temperature_c: 5.0,
            powder_temperature_c: 5.0,
            pressure_hpa: 1001.0,
            humidity_percent: 70.0,
        },
        hold: ShotHold {
            elevation_mrad: 0.6,
            windage_mrad: -0.3,
            click_unit: ClickUnit::Mil,
            click_value: 0.1,
            elevation_clicks: 6,
            windage_clicks: -3,
        },
        solution: ShotSolution {
            drop_cm: -9.0,
            windage_cm: 4.5,
            velocity_mps: 700.0,
            energy_j: 2860.0,
            time_of_flight_s: 0.2,
        },
        outcome: Outcome::NotRecorded,
        hit_where: String::new(),
        position: None,
        bearing_degrees: None,
        animal_spot: None,
        notes: String::new(),
        updated_at: 0,
    }
}

fn position(
    latitude_degrees: f64,
    longitude_degrees: f64,
    accuracy_m: Option<f64>,
) -> ShooterPosition {
    ShooterPosition {
        point: GeoPoint {
            latitude_degrees,
            longitude_degrees,
        },
        accuracy_m,
    }
}

#[test]
fn shots_are_kept_newest_first_with_the_animals_spot() {
    let (store, tikka, oryx) = populated();
    let mut first = logged_shot(&tikka, &oryx, 1_000);
    let mut second = logged_shot(&tikka, &oryx, 2_000);
    store.save_shot(&mut first).unwrap();
    second.outcome = Outcome::Hit;
    second.hit_where = "heart–lung".into();
    second.position = Some(position(64.75, 20.95, Some(5.0)));
    second.bearing_degrees = Some(90.0);
    second.inclination_degrees = 10.0;
    store.save_shot(&mut second).unwrap();
    assert!(second.updated_at > 0);
    assert_eq!(store.shots().unwrap(), vec![second.clone(), first.clone()]);
    assert_eq!(store.shot(&first.id).unwrap(), Some(first.clone()));

    // Saving worked out the spot: east, at the level distance.
    let spot = second.animal_spot.expect("a position and a bearing");
    let (distance, bearing) =
        shots::distance_and_bearing(second.position.unwrap().point, spot.point);
    let level = 150.0 * 10f64.to_radians().cos();
    assert!((distance - level).abs() < 0.01, "{distance}");
    assert!((bearing - 90.0).abs() < 0.01);
    assert!(spot.search_radius_m > 20.0 && spot.search_radius_m < 35.0);
    // Without the bearing the spot goes.
    second.bearing_degrees = None;
    store.save_shot(&mut second).unwrap();
    assert_eq!(store.shot(&second.id).unwrap().unwrap().animal_spot, None);

    // A rifle's deletion leaves its shots in the log.
    store.delete_rifle(&tikka.id).unwrap();
    assert_eq!(store.shots().unwrap().len(), 2);
    store.delete_shot(&first.id).unwrap();
    assert_eq!(store.shots().unwrap(), vec![second]);
    assert!(
        store
            .deletions()
            .unwrap()
            .iter()
            .any(|d| d.kind == RecordKind::Shot && d.id == first.id)
    );
}

#[test]
fn impossible_shots_are_refused() {
    let (store, tikka, oryx) = populated();
    let good = logged_shot(&tikka, &oryx, 1_000);
    let broken: [fn(&mut Shot); 7] = [
        |shot| shot.range_m = 0.0,
        |shot| shot.inclination_degrees = 95.0,
        |shot| shot.wind_clock = 13.0,
        |shot| shot.weather.pressure_hpa = -1.0,
        |shot| shot.solution.velocity_mps = f64::NAN,
        |shot| shot.bearing_degrees = Some(400.0),
        |shot| shot.position = Some(position(91.0, 0.0, None)),
    ];
    for (index, breaks) in broken.iter().enumerate() {
        let mut shot = good.clone();
        breaks(&mut shot);
        assert!(
            matches!(store.save_shot(&mut shot), Err(StoreError::Invalid(_))),
            "case {index}"
        );
    }
    assert!(store.shots().unwrap().is_empty());
    assert!(good.validate().is_ok());
}

#[test]
fn shots_export_import_and_match_the_schema() {
    let (store, tikka, oryx) = populated();
    let mut shot = logged_shot(&tikka, &oryx, 1_000);
    shot.position = Some(position(60.5, 15.25, None));
    shot.bearing_degrees = Some(200.0);
    shot.notes = "Ran 40 m into the spruce".into();
    store.save_shot(&mut shot).unwrap();

    let export = store.export_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&export).unwrap();
    assert_eq!(value["shots"][0]["position"]["latitude_degrees"], 60.5);
    let validator = jsonschema::validator_for(&schema::export_schema()).unwrap();
    let problems: Vec<String> = validator
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");

    let mut copy = Store::open_in_memory().unwrap();
    assert_eq!(copy.import_json(&export).unwrap().added, 4);
    assert_eq!(copy.import_json(&export).unwrap().unchanged, 4);
    assert_eq!(copy.shots().unwrap(), vec![shot]);
}

#[test]
fn backups_without_shots_still_import() {
    // A version 5 file, from before the shot log.
    let (old, _, _) = populated();
    let mut value: serde_json::Value = serde_json::from_str(&old.export_json().unwrap()).unwrap();
    value["version"] = 5.into();
    value.as_object_mut().unwrap().remove("shots");
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.import_json(&value.to_string()).unwrap().added, 3);
    assert!(store.shots().unwrap().is_empty());
    // A shot written by another program, with only what it must have.
    let minimal = r#"{"format":"mxsc/personal-data","version":6,"exported_at":0,
        "rifles":[],"bullets":[],"rifle_loads":[],
        "shots":[{"id":"s1","taken_at":1792473562000,"rifle_id":"r","bullet_id":"b",
          "range_m":120,"wind_speed_mps":0,"wind_clock":12,
          "weather":{"temperature_c":2,"powder_temperature_c":2,"pressure_hpa":990,
            "humidity_percent":80},
          "hold":{"elevation_mrad":0.2,"windage_mrad":0,"click_unit":"mil","click_value":0.1,
            "elevation_clicks":2,"windage_clicks":0},
          "solution":{"drop_cm":-2,"windage_cm":0,"velocity_mps":720,"energy_j":3000,
            "time_of_flight_s":0.16}}]}"#;
    store.import_json(minimal).unwrap();
    let shot = store.shot("s1").unwrap().unwrap();
    assert_eq!(shot.outcome, Outcome::NotRecorded);
    assert_eq!((shot.position, shot.target_id), (None, None));
}

#[test]
fn shots_sync_and_their_deletions_follow() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let (mut phone, tikka, oryx) = populated();
    let mut desktop = Store::open_in_memory().unwrap();
    let mut shot = logged_shot(&tikka, &oryx, model::now_ms());
    phone.save_shot(&mut shot).unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert_eq!(desktop.shots().unwrap(), vec![shot.clone()]);

    // The outcome written on the desktop reaches the phone.
    let mut edited = desktop.shot(&shot.id).unwrap().unwrap();
    edited.outcome = Outcome::Miss;
    desktop.save_shot(&mut edited).unwrap();
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    assert_eq!(
        phone.shot(&shot.id).unwrap().unwrap().outcome,
        Outcome::Miss
    );

    // Deleted on the phone, deleted on the desktop.
    phone.delete_shot(&shot.id).unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert!(desktop.shots().unwrap().is_empty());
    let writes = shared.writes;
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    assert_eq!(shared.writes, writes, "settled");
}

// Shot groups and what goes with them --------------------------------------

/// A day at the range for `rifle` and `bullet`: a fitted barrel, a session,
/// a photo with a group on it, a grid group that is a ladder's rung, the
/// round log, a clean, a zero reset and an analysis.
fn range_day(store: &Store, rifle: &Rifle, bullet: &Bullet) -> GroupsData {
    let mut factory = Barrel::new(&rifle.id, "Factory");
    factory.length_mm = Some(510.0);
    factory.twist_in = Some(11.0);
    factory.fitted_at = Some(1_000);
    factory.rounds_before_logging = 100;
    store.save_barrel(&mut factory).unwrap();
    let mut fitted = store.rifle(&rifle.id).unwrap().unwrap();
    fitted.fitted_barrel_id = Some(factory.id.clone());
    fitted.rounds_before_logging = 250;
    store.save_rifle(&mut fitted).unwrap();

    let mut session = Session::new(2_000);
    session.place = "Boden".into();
    session.conditions = Conditions {
        temperature_c: Some(6.0),
        pressure_hpa: Some(1011.0),
        wind_speed_mps: Some(2.0),
        wind_clock: Some(9.0),
        ..Conditions::default()
    };
    store.save_session(&mut session).unwrap();
    let mut photo = Photo::new(photo_id(b"the photo's bytes"), 4000, 3000);
    photo.taken_at = Some(2_100);
    photo.calibration = Some(Calibration::FourCorner {
        corners_px: [
            [100.0, 120.0],
            [3900.0, 100.0],
            [3880.0, 2900.0],
            [120.0, 2880.0],
        ],
        width_mm: 297.0,
        height_mm: 210.0,
        ordered: true,
    });
    store.save_photo(&mut photo).unwrap();

    let mut on_photo = Group::on_photo(&photo.id, 2_200);
    let Marks::Photo(marks) = &mut on_photo.marks else {
        unreachable!()
    };
    marks.aim_px = Some([2000.0, 1500.0]);
    marks.shots = vec![
        PhotoShot {
            x_px: 2010.5,
            y_px: 1490.0,
            cold_bore: true,
            called_flier: false,
            velocity_mps: Some(823.0),
        },
        PhotoShot {
            x_px: 2030.0,
            y_px: 1505.25,
            cold_bore: false,
            called_flier: true,
            velocity_mps: None,
        },
    ];
    on_photo.rifle_id = Some(rifle.id.clone());
    on_photo.barrel_id = Some(factory.id.clone());
    on_photo.bullet_id = Some(bullet.id.clone());
    on_photo.distance_m = Some(100.0);
    on_photo.session_id = Some(session.id.clone());
    on_photo.conditions.temperature_c = Some(4.0);
    on_photo.dialled_offset_mrad = [0.1, 0.5];
    on_photo.tags = vec!["practice".into()];
    store.save_group(&mut on_photo).unwrap();
    store
        .set_group_view(
            &on_photo.id,
            &GroupView {
                zoom: 3.0,
                centre: [2000.0, 1500.0],
            },
        )
        .unwrap();
    let mut entry = RoundLogEntry::for_group(&on_photo, 2).unwrap();
    store.save_round_log_entry(&mut entry).unwrap();
    store
        .save_clean(&mut Clean::new(&rifle.id, Some(&factory.id), 2_300))
        .unwrap();
    store
        .save_zero_reset(&mut ZeroReset::new(&rifle.id, Some(&factory.id), 1_500))
        .unwrap();

    let mut ladder = Ladder::new(&rifle.id, &bullet.id, LadderVariable::Charge);
    ladder.name = "N150 charge".into();
    store.save_ladder(&mut ladder).unwrap();
    let mut rung = Group::on_grid(300.0, 300.0, 2_400);
    let Marks::Grid(marks) = &mut rung.marks else {
        unreachable!()
    };
    marks.shots = vec![
        GridShot {
            x_mm: -4.0,
            y_mm: 6.5,
            cold_bore: false,
            called_flier: false,
            velocity_mps: Some(820.0),
        },
        GridShot {
            x_mm: 3.0,
            y_mm: -1.0,
            cold_bore: false,
            called_flier: false,
            velocity_mps: Some(826.0),
        },
    ];
    rung.extra_velocities_mps = vec![818.0];
    rung.rifle_id = Some(rifle.id.clone());
    rung.ladder_id = Some(ladder.id.clone());
    rung.rung_value = Some(2.85);
    rung.include_in_zero = false;
    store.save_group(&mut rung).unwrap();
    store
        .save_round_log_entry(&mut RoundLogEntry::new(
            &rifle.id,
            Some(&factory.id),
            2,
            2_400,
        ))
        .unwrap();

    let mut analysis = Analysis::new(
        AnalysisKind::Compare,
        "Before and after",
        vec![on_photo.id.clone(), rung.id.clone()],
    );
    analysis.settings = serde_json::json!({ "rank_by": "mean_radius" });
    store.save_analysis(&mut analysis).unwrap();
    store.groups_data().unwrap()
}

#[test]
fn range_records_save_read_back_and_give_round_counts() {
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    assert_eq!(data.barrels.len(), 1);
    assert_eq!(data.round_log.len(), 2);
    assert_eq!(data.round_log[0].at, 2_400, "newest first");
    assert_eq!(data.groups.len(), 2);
    assert_eq!(data.groups[0].rung_value, Some(2.85), "newest first");
    assert!(data.groups.iter().all(|group| group.updated_at > 0));

    let on_photo = &data.groups[1];
    assert_eq!(store.group(&on_photo.id).unwrap().as_ref(), Some(on_photo));
    let photo_id = on_photo.photo_id().unwrap();
    assert_eq!(
        store.groups_on_photo(photo_id).unwrap(),
        vec![on_photo.clone()]
    );
    assert_eq!(store.photo(photo_id).unwrap(), Some(data.photos[0].clone()));
    let session = &data.sessions[0];
    assert_eq!(
        store.groups_in_session(&session.id).unwrap(),
        vec![on_photo.clone()]
    );
    assert_eq!(store.groups_for_rifle(&tikka.id).unwrap().len(), 2);
    assert_eq!(
        store.groups_in_ladder(&data.ladders[0].id).unwrap(),
        vec![data.groups[0].clone()]
    );
    assert_eq!(
        on_photo.conditions.or(session.conditions).temperature_c,
        Some(4.0)
    );
    assert_eq!(
        on_photo.conditions.or(session.conditions).wind_clock,
        Some(9.0)
    );

    let rifle = store.rifle(&tikka.id).unwrap().unwrap();
    let barrel = store
        .barrel(rifle.fitted_barrel_id.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        store.barrels_for_rifle(&rifle.id).unwrap(),
        vec![barrel.clone()]
    );
    let counts = RoundCounts::derive(&rifle, Some(&barrel), &data.round_log, &data.cleans);
    assert_eq!(
        counts,
        RoundCounts {
            action: 254,
            barrel: Some(104),
            since_clean: Some(2),
            last_clean_at: Some(2_300),
        }
    );
}

#[test]
fn impossible_range_records_are_refused() {
    let (store, tikka, oryx) = populated();
    let invalid = |result: Result<(), StoreError>| matches!(result, Err(StoreError::Invalid(_)));

    let mut group = Group::on_photo("IMG_0412.JPG", 0);
    assert!(invalid(store.save_group(&mut group)));
    let mut photo = Photo::new(photo_id(b"x"), 100, 100);
    photo.calibration = Some(Calibration::TwoPoint {
        a_px: [10.0, 10.0],
        b_px: [10.0, 10.0],
        distance_mm: 25.0,
    });
    assert!(invalid(store.save_photo(&mut photo)));
    photo.calibration = Some(Calibration::FourCorner {
        corners_px: [[0.0, 0.0], [90.0, 0.0], [90.0, 90.0], [90.0, 90.0]],
        width_mm: 210.0,
        height_mm: 297.0,
        ordered: false,
    });
    assert!(invalid(store.save_photo(&mut photo)));
    let mut group = Group::on_photo(&photo_id(b"x"), 0);
    let mut fouled = group.clone();
    fouled.extra_velocities_mps = vec![820.0, 28.5];
    assert!(invalid(store.save_group(&mut fouled)));
    let Marks::Photo(marks) = &mut group.marks else {
        unreachable!()
    };
    marks.shots = vec![
        PhotoShot {
            x_px: f64::NAN,
            y_px: 0.0,
            cold_bore: false,
            called_flier: false,
            velocity_mps: None,
        };
        2
    ];
    assert!(invalid(store.save_group(&mut group.clone())));
    let mut slow = Group::on_grid(300.0, 300.0, 0);
    let Marks::Grid(marks) = &mut slow.marks else {
        unreachable!()
    };
    marks.shots.push(GridShot {
        x_mm: 0.0,
        y_mm: 0.0,
        cold_bore: false,
        called_flier: false,
        velocity_mps: Some(2850.0),
    });
    assert!(invalid(store.save_group(&mut slow)));
    let mut tagged = Group::on_grid(300.0, 300.0, 0);
    tagged.tags = vec![" practice".into()];
    assert!(invalid(store.save_group(&mut tagged)));
    tagged.tags = vec!["practice".into()];
    tagged.distance_m = Some(0.0);
    assert!(invalid(store.save_group(&mut tagged)));
    tagged.distance_m = Some(100.0);
    store.save_group(&mut tagged).unwrap();

    let mut session = Session::new(0);
    session.conditions.humidity_percent = Some(120.0);
    assert!(invalid(store.save_session(&mut session)));
    let mut analysis = Analysis::new(AnalysisKind::Composite, "All", Vec::new());
    analysis.settings = serde_json::json!([1, 2]);
    assert!(invalid(store.save_analysis(&mut analysis)));
    assert!(invalid(store.save_ladder(&mut Ladder::new(
        &tikka.id,
        "",
        LadderVariable::Seating
    ))));
    let mut handload = store.bullet(&oryx.id).unwrap().unwrap();
    handload.recipe = Some(Recipe {
        charge_g: Some(45.0),
        ..Recipe::default()
    });
    assert!(invalid(store.save_bullet(&mut handload)));
    let mut rifle = store.rifle(&tikka.id).unwrap().unwrap();
    rifle.rounds_before_logging = 2_000_000;
    assert!(invalid(store.save_rifle(&mut rifle)));
    assert_eq!(store.groups().unwrap().len(), 1, "only the valid group");
}

#[test]
fn a_handload_recipe_round_trips() {
    let (store, _, oryx) = populated();
    let mut handload = store.bullet(&oryx.id).unwrap().unwrap();
    assert_eq!(handload.recipe, None);
    let recipe = Recipe {
        powder: "Vihtavuori N150".into(),
        charge_g: Some(2.85),
        primer: "CCI BR-2".into(),
        brass: "Lapua, 3rd firing".into(),
        coal_mm: Some(71.1),
        cbto_mm: Some(57.2),
        notes: "Seated 0.5 mm off".into(),
    };
    handload.recipe = Some(recipe.clone());
    store.save_bullet(&mut handload).unwrap();
    assert_eq!(
        store.bullet(&oryx.id).unwrap().unwrap().recipe,
        Some(recipe.clone())
    );
    let export: serde_json::Value = serde_json::from_str(&store.export_json().unwrap()).unwrap();
    assert_eq!(export["bullets"][0]["recipe"]["charge_g"], 2.85);
    let mut copy = Store::open_in_memory().unwrap();
    copy.import_json(&export.to_string()).unwrap();
    assert_eq!(copy.bullet(&oryx.id).unwrap().unwrap().recipe, Some(recipe));
    // A factory bullet has no recipe, and writes none.
    let mut factory = bullet("Factory", "308-win");
    factory.kind = BulletKind::Factory;
    store.save_bullet(&mut factory).unwrap();
    let export: serde_json::Value = serde_json::from_str(&store.export_json().unwrap()).unwrap();
    let factory = export["bullets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bullet| bullet["kind"] == "factory")
        .unwrap();
    assert!(factory.get("recipe").is_none());
}

#[test]
fn range_records_export_import_and_match_the_schema() {
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    let export = store.export_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&export).unwrap();
    assert_eq!(value["version"], 8);
    let validator = jsonschema::validator_for(&schema::export_schema()).unwrap();
    let problems: Vec<String> = validator
        .iter_errors(&value)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");
    // The shapes the format describes.
    let on_photo = &value["groups"][1];
    assert_eq!(on_photo["marks"]["on"], "photo");
    assert_eq!(value["photos"][0]["calibration"]["kind"], "four_corner");
    assert!(
        on_photo["marks"].get("view").is_none(),
        "views stay on the device"
    );
    assert_eq!(
        on_photo["conditions"],
        serde_json::json!({ "temperature_c": 4.0 })
    );
    assert_eq!(value["groups"][0]["extra_velocities_mps"][0], 818.0);
    assert_eq!(on_photo["marks"]["shots"][0]["x_px"], 2010.5);
    assert_eq!(value["groups"][0]["marks"]["on"], "grid");
    assert_eq!(value["groups"][0]["marks"]["shots"][0]["y_mm"], 6.5);
    assert_eq!(value["sessions"][0]["temperature_c"], 6.0);
    assert_eq!(value["rifles"][0]["rounds_before_logging"], 250);
    assert_eq!(value["ladders"][0]["variable"], "charge");

    let mut copy = Store::open_in_memory().unwrap();
    let report = copy.import_json(&export).unwrap();
    // A rifle, a bullet and a load, then the range day's eleven records.
    assert_eq!((report.added, report.total()), (14, 14));
    assert_eq!(copy.import_json(&export).unwrap().unchanged, 14);
    assert_eq!(copy.groups_data().unwrap(), data);
    assert_eq!(copy.rifles().unwrap(), store.rifles().unwrap());
}

#[test]
fn files_from_before_groups_still_import_and_newer_ones_are_refused() {
    let (old, _, _) = populated();
    let mut value: serde_json::Value = serde_json::from_str(&old.export_json().unwrap()).unwrap();
    value["version"] = 7.into();
    let fields = value.as_object_mut().unwrap();
    for list in [
        "barrels",
        "round_log",
        "cleans",
        "zero_resets",
        "sessions",
        "photos",
        "groups",
        "ladders",
        "analyses",
    ] {
        assert!(fields.remove(list).is_some(), "{list} is written");
    }
    let rifle = fields["rifles"][0].as_object_mut().unwrap();
    rifle.remove("fitted_barrel_id");
    rifle.remove("rounds_before_logging");
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.import_json(&value.to_string()).unwrap().added, 3);
    assert_eq!(store.rifles().unwrap(), old.rifles().unwrap());
    assert_eq!(store.groups_data().unwrap(), GroupsData::default());

    value["version"] = (transfer::VERSION + 1).into();
    assert!(matches!(
        store.import_json(&value.to_string()),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn range_records_sync_and_their_deletions_follow() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let (mut phone, tikka, oryx) = populated();
    let data = range_day(&phone, &tikka, &oryx);
    let mut desktop = Store::open_in_memory().unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert_eq!(desktop.groups_data().unwrap(), data);

    // Notes written on the desktop reach the phone; the phone's later edit
    // of the same group wins over the desktop's earlier one.
    let mut edited = desktop.group(&data.groups[0].id).unwrap().unwrap();
    edited.notes = "Flat spot?".into();
    desktop.save_group(&mut edited).unwrap();
    let mut session = phone.session(&data.sessions[0].id).unwrap().unwrap();
    session.notes = "Gusty after noon".into();
    phone.save_session(&mut session).unwrap();
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert_eq!(
        phone.group(&edited.id).unwrap().unwrap().notes,
        "Flat spot?"
    );
    assert_eq!(
        desktop.session(&session.id).unwrap().unwrap().notes,
        "Gusty after noon"
    );

    // Deleted on the phone, deleted on the desktop; the photo goes with
    // its last group.
    let photo = &data.photos[0].id;
    assert!(
        !phone.delete_photo(photo).unwrap(),
        "a group is marked on it"
    );
    assert!(phone.photo(photo).unwrap().is_some());
    phone.delete_group(&edited.id).unwrap();
    phone.delete_group(&data.groups[1].id).unwrap();
    assert!(phone.delete_photo(photo).unwrap());
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert_eq!(desktop.group(&edited.id).unwrap(), None);
    assert!(desktop.photos().unwrap().is_empty());
    assert!(
        desktop
            .deletions()
            .unwrap()
            .iter()
            .all(|d| d.kind != RecordKind::Photo),
        "photo records are never remembered as deleted"
    );
    assert!(
        desktop
            .deletions()
            .unwrap()
            .iter()
            .any(|d| d.kind == RecordKind::Group && d.id == edited.id)
    );
    let writes = shared.writes;
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    assert!(phone.photos().unwrap().is_empty());
    assert_eq!(shared.writes, writes, "settled");
}

#[test]
fn deleting_a_barrel_or_a_rifle_takes_its_history_along() {
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    let barrel = &data.barrels[0];
    let before = store.rifle(&tikka.id).unwrap().unwrap();
    store.delete_barrel(&barrel.id).unwrap();
    let after = store.rifle(&tikka.id).unwrap().unwrap();
    assert_eq!(after.fitted_barrel_id, None);
    assert!(after.updated_at > before.updated_at, "the unfitting syncs");
    assert!(store.barrels().unwrap().is_empty());
    assert!(store.cleans().unwrap().is_empty());
    assert!(store.zero_resets().unwrap().is_empty());
    assert_eq!(store.round_log().unwrap().len(), 2, "the action fired them");
    let kinds = |store: &Store| -> Vec<RecordKind> {
        store
            .deletions()
            .unwrap()
            .iter()
            .map(|deletion| deletion.kind)
            .collect()
    };
    assert_eq!(kinds(&store), vec![RecordKind::Barrel], "only the barrel's");

    // A rifle with its barrel, history and loads.
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    store
        .save_clean(&mut Clean::new(&tikka.id, None, 2_500))
        .unwrap();
    store.delete_rifle(&tikka.id).unwrap();
    let left = store.groups_data().unwrap();
    assert!(left.barrels.is_empty(), "barrels");
    assert!(left.round_log.is_empty(), "round log");
    assert!(left.cleans.is_empty(), "cleans");
    assert!(left.zero_resets.is_empty(), "zero resets");
    assert!(store.rifle_loads().unwrap().is_empty(), "loads");
    assert_eq!(kinds(&store), vec![RecordKind::Rifle], "only the rifle's");
    assert_eq!(left.groups.len(), 2, "groups stay");
    assert_eq!(left.ladders.len(), 1, "ladders stay");
    assert_eq!(left.photos, data.photos);
}

/// The phone and the desktop, in step, with a range day for the phone's
/// rifle.
fn two_devices_with_a_range_day() -> (Store, Store, SyncKey, Shared, Rifle, GroupsData) {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let (mut phone, tikka, oryx) = populated();
    let data = range_day(&phone, &tikka, &oryx);
    let mut desktop = Store::open_in_memory().unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    (phone, desktop, key, shared, tikka, data)
}

/// When `store` deleted the record of `id`.
fn deleted_at(store: &Store, id: &str) -> i64 {
    store
        .deletions()
        .unwrap()
        .iter()
        .find(|deletion| deletion.id == id)
        .unwrap()
        .deleted_at
}

#[test]
fn a_rifle_deleted_on_one_device_and_edited_later_on_another_comes_back_whole() {
    let (mut phone, mut desktop, key, mut shared, tikka, data) = two_devices_with_a_range_day();
    phone.delete_rifle(&tikka.id).unwrap();
    let mut rifle = desktop.rifle(&tikka.id).unwrap().unwrap();
    rifle.notes = "Kept after all".into();
    rifle.updated_at = deleted_at(&phone, &tikka.id) + 1_000;
    desktop.save_rifle(&mut rifle).unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    for store in [&phone, &desktop] {
        let rifle = store.rifle(&tikka.id).unwrap().unwrap();
        assert_eq!(rifle.notes, "Kept after all");
        let kept = store.groups_data().unwrap();
        assert_eq!(kept, data);
        assert_eq!(rifle.fitted_barrel_id, Some(kept.barrels[0].id.clone()));
        assert_eq!(store.rifle_loads().unwrap().len(), 1, "its load too");
    }
}

#[test]
fn a_barrel_deleted_on_one_device_and_edited_later_on_another_comes_back_whole() {
    let (mut phone, mut desktop, key, mut shared, _, data) = two_devices_with_a_range_day();
    let barrel = &data.barrels[0];
    phone.delete_barrel(&barrel.id).unwrap();
    let mut edited = desktop.barrel(&barrel.id).unwrap().unwrap();
    edited.notes = "Still shooting".into();
    edited.updated_at = deleted_at(&phone, &barrel.id) + 1_000;
    desktop.save_barrel(&mut edited).unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    for store in [&phone, &desktop] {
        let kept = store.groups_data().unwrap();
        assert_eq!(kept.barrels[0].notes, "Still shooting");
        assert_eq!((kept.cleans.len(), kept.zero_resets.len()), (1, 1));
        assert_eq!(kept.round_log.len(), 2);
    }
}

#[test]
fn history_added_elsewhere_after_its_rifle_was_deleted_goes_with_it() {
    let (mut phone, mut desktop, key, mut shared, tikka, data) = two_devices_with_a_range_day();
    phone.delete_rifle(&tikka.id).unwrap();
    // The desktop hasn't heard, and logs more for the rifle and its barrel.
    let later = deleted_at(&phone, &tikka.id) + 1_000;
    let barrel = &data.barrels[0].id;
    let mut spare = Barrel::new(&tikka.id, "Spare");
    spare.updated_at = later;
    desktop.save_barrel(&mut spare).unwrap();
    for mut clean in [
        Clean::new(&tikka.id, None, later),
        Clean::new(&tikka.id, Some(barrel), later),
    ] {
        clean.updated_at = later;
        desktop.save_clean(&mut clean).unwrap();
    }
    let mut entry = RoundLogEntry::new(&tikka.id, Some(barrel), 5, later);
    entry.updated_at = later;
    desktop.save_round_log_entry(&mut entry).unwrap();
    let mut reset = ZeroReset::new(&tikka.id, None, later);
    reset.updated_at = later;
    desktop.save_zero_reset(&mut reset).unwrap();
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    for store in [&phone, &desktop] {
        assert_eq!(store.rifle(&tikka.id).unwrap(), None);
        let left = store.groups_data().unwrap();
        assert!(left.barrels.is_empty());
        assert!(left.round_log.is_empty());
        assert!(left.cleans.is_empty());
        assert!(left.zero_resets.is_empty());
        assert!(store.rifle_loads().unwrap().is_empty());
    }
    let writes = shared.writes;
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    assert_eq!(shared.writes, writes, "settled");
}

#[test]
fn a_barrel_moved_to_another_rifle_keeps_its_history_when_the_old_rifle_goes() {
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    let mut sako = rifle("Sako", "308-win");
    store.save_rifle(&mut sako).unwrap();
    let mut moved = data.barrels[0].clone();
    moved.rifle_id = sako.id.clone();
    store.save_barrel(&mut moved).unwrap();
    store.delete_rifle(&tikka.id).unwrap();
    let left = store.groups_data().unwrap();
    assert_eq!(left.barrels.len(), 1);
    assert_eq!((left.round_log.len(), left.cleans.len()), (2, 1));
    let counts = RoundCounts::derive(&sako, Some(&moved), &left.round_log, &left.cleans);
    assert_eq!(counts.barrel, Some(104));
}

#[test]
fn a_photo_deleted_on_one_device_stays_for_a_group_made_on_another() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let mut phone = Store::open_in_memory().unwrap();
    let mut photo = Photo::new(photo_id(b"target"), 400, 300);
    phone.save_photo(&mut photo).unwrap();
    let mut first = Group::on_photo(&photo.id, 1_000);
    phone.save_group(&mut first).unwrap();
    let mut desktop = Store::open_in_memory().unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);

    phone.delete_group(&first.id).unwrap();
    assert!(phone.delete_photo(&photo.id).unwrap());
    let mut second = Group::on_photo(&photo.id, 2_000);
    desktop.save_group(&mut second).unwrap();
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    for store in [&phone, &desktop] {
        assert_eq!(
            store.photo(&photo.id).unwrap().map(|p| p.id),
            Some(photo.id.clone())
        );
        assert_eq!(store.groups().unwrap().len(), 1);
    }
    // Imported again after it went, nothing remembers it as deleted.
    assert!(phone.deletions().unwrap().iter().all(|d| d.id != photo.id));
    phone.delete_group(&second.id).unwrap();
    assert!(phone.delete_photo(&photo.id).unwrap());
    let mut again = photo.clone();
    phone.save_photo(&mut again).unwrap();
    assert!(phone.photo(&photo.id).unwrap().is_some());
    assert!(phone.deletions().unwrap().iter().all(|d| d.id != photo.id));
}

#[test]
fn a_photo_forgotten_here_isnt_brought_back_without_a_group() {
    let key = SyncKey::generate();
    let mut shared = Shared::default();
    let mut phone = Store::open_in_memory().unwrap();
    let mut desktop = Store::open_in_memory().unwrap();
    let mut photo = Photo::new(photo_id(b"unused"), 400, 300);
    desktop.save_photo(&mut photo).unwrap();
    sync(&mut desktop, &key, &mut shared);
    let report = sync(&mut phone, &key, &mut shared);
    assert_eq!(phone.photo(&photo.id).unwrap(), None);
    assert_eq!(report.added, 0);
    assert!(
        desktop.photo(&photo.id).unwrap().is_some(),
        "the desktop's own"
    );
    assert_eq!(
        desktop.photo_ids_in_use().unwrap(),
        std::collections::BTreeSet::from([photo.id.clone()])
    );
}

#[test]
fn a_view_stays_on_its_device() {
    let (store, tikka, oryx) = populated();
    let data = range_day(&store, &tikka, &oryx);
    let group = &data.groups[1].id;
    assert_eq!(
        store.group_view(group).unwrap(),
        Some(GroupView {
            zoom: 3.0,
            centre: [2000.0, 1500.0]
        })
    );
    assert!(!store.export_json().unwrap().contains("\"zoom\""));
    let mut copy = Store::open_in_memory().unwrap();
    copy.import_json(&store.export_json().unwrap()).unwrap();
    assert_eq!(copy.group_view(group).unwrap(), None);
    let bad = GroupView {
        zoom: 0.0,
        centre: [0.0, 0.0],
    };
    assert!(matches!(
        store.set_group_view(group, &bad),
        Err(StoreError::Invalid(_))
    ));
    store.delete_group(group).unwrap();
    assert_eq!(store.group_view(group).unwrap(), None);
}

#[test]
fn a_group_saved_on_two_devices_is_counted_once() {
    let (mut phone, mut desktop, key, mut shared, tikka, data) = two_devices_with_a_range_day();
    let group = &data.groups[1];
    let logged = phone.round_log_entry_for_group(&group.id).unwrap().unwrap();
    assert_eq!(logged.id, RoundLogEntry::id_for_group(&group.id));
    assert_eq!(
        (logged.rifle_id.as_str(), logged.rounds),
        (tikka.id.as_str(), 2)
    );
    // Both devices log the same group again, as saving it would.
    for store in [&phone, &desktop] {
        let mut entry = RoundLogEntry::for_group(group, 3).unwrap();
        store.save_round_log_entry(&mut entry).unwrap();
    }
    sync(&mut phone, &key, &mut shared);
    sync(&mut desktop, &key, &mut shared);
    sync(&mut phone, &key, &mut shared);
    for store in [&phone, &desktop] {
        let log = store.round_log().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(
            log.iter()
                .filter(|entry| entry.group_id.as_deref() == Some(group.id.as_str()))
                .count(),
            1
        );
    }
    assert_eq!(phone.round_log_entry_for_group("nothing").unwrap(), None);
    assert_eq!(
        RoundLogEntry::for_group(&Group::on_grid(100.0, 100.0, 0), 5),
        None
    );
}

#[test]
fn imported_groups_are_checked_and_their_times_kept_sane() {
    let (store, tikka, oryx) = populated();
    range_day(&store, &tikka, &oryx);
    let export: serde_json::Value = serde_json::from_str(&store.export_json().unwrap()).unwrap();

    // A group that breaks a rule stops the whole file.
    let mut broken = export.clone();
    broken["groups"][0]["distance_m"] = 0.into();
    let mut fresh = Store::open_in_memory().unwrap();
    assert!(matches!(
        fresh.import_json(&broken.to_string()),
        Err(StoreError::Invalid(_))
    ));
    assert!(fresh.rifles().unwrap().is_empty());

    // A group stamped years ahead is taken as old: a later edit wins.
    let mut ahead = export.clone();
    let id = ahead["groups"][0]["id"].as_str().unwrap().to_owned();
    ahead["groups"][0]["updated_at"] = (model::now_ms() + 3 * 365 * 86_400_000).into();
    fresh.import_json(&ahead.to_string()).unwrap();
    assert_eq!(fresh.group(&id).unwrap().unwrap().updated_at, 0);

    // A group stamped a little ahead moves the clock: edits after it are
    // stamped later still.
    let soon = model::now_ms() + 86_400_000;
    let mut close = export.clone();
    close["groups"][0]["updated_at"] = soon.into();
    let mut other = Store::open_in_memory().unwrap();
    other.import_json(&close.to_string()).unwrap();
    let mut session = Session::new(0);
    other.save_session(&mut session).unwrap();
    assert!(session.updated_at > soon);
}

#[test]
fn a_file_from_a_newer_app_is_refused_as_newer_before_its_records_are_read() {
    let mut store = Store::open_in_memory().unwrap();
    let newer = format!(
        r#"{{"format":"mxsc/personal-data","version":{},"exported_at":0,
            "rifles":[],"bullets":[],"rifle_loads":[],
            "deleted":[{{"kind":"something_new","id":"x","deleted_at":1}}]}}"#,
        transfer::VERSION + 1
    );
    match store.import_json(&newer) {
        Err(StoreError::Invalid(message)) => assert!(message.contains("newer"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn sync_copies_are_compressed_and_older_ones_still_open() {
    let key = SyncKey::generate();
    let (mut store, tikka, oryx) = populated();
    range_day(&store, &tikka, &oryx);
    let round = store.sync_round(Some(&key), None).unwrap();
    let sealed = round.upload.unwrap();
    assert_eq!(&sealed[..4], b"MXS2");
    let json = serde_json::to_vec(&store.export_data().unwrap()).unwrap();
    assert!(
        sealed.len() * 2 < json.len(),
        "{} of {}",
        sealed.len(),
        json.len()
    );
    assert_eq!(
        transfer::read_export(std::str::from_utf8(&key.open(&sealed).unwrap()).unwrap())
            .unwrap()
            .groups
            .len(),
        2
    );
    // A copy from a newer app says so, not that the key is wrong.
    let mut newer = sealed.clone();
    newer[3] = b'9';
    match key.open(&newer) {
        Err(StoreError::Invalid(message)) => assert!(message.contains("newer"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_released_database_upgrades_with_its_data_intact() {
    let path = temporary_path();
    let connection = rusqlite::Connection::open(&path).unwrap();
    for migration in &crate::db::MIGRATIONS[..crate::db::RELEASED_SCHEMA] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .pragma_update(None, "user_version", crate::db::RELEASED_SCHEMA as i64)
        .unwrap();
    let mut tikka = rifle("Tikka T3x", "308-win");
    tikka.id = "r1".into();
    let mut oryx = bullet("Oryx", "308-win");
    oryx.id = "b1".into();
    let mut shot = logged_shot(&tikka, &oryx, 1_500);
    shot.updated_at = 1_600;
    connection
        .execute_batch(
            "INSERT INTO rifles (id, name, caliber, barrel_length_mm, twist_in,
                 right_hand_twist, sight_height_cm, zero_range_m, zero_temperature_c,
                 zero_pressure_hpa, click_unit, click_value, notes, updated_at, scope_id,
                 reticle_id, zero_powder_temperature_c, zero_humidity_percent, table_settings,
                 zero_height_cm)
             VALUES ('r1', 'Tikka T3x', '308-win', 510, 11, 1, 4.5, 100, 15, 1013.25, 'mil',
                 0.1, 'Old rifle', 100, NULL, NULL, NULL, 50, NULL, 4);
             INSERT INTO bullets (id, kind, manufacturer, name, caliber, diameter_mm, mass_g,
                 bc_g1, drag_model, muzzle_velocity_mps, velocity_temperature_c,
                 velocity_per_degree, construction, lead_free, notes, updated_at)
             VALUES ('b1', 'handload', 'Test', 'Oryx', '308-win', 7.82, 11.7, 0.45, 'G1', 790,
                 15, 0.5, 'other', 0, 'Old bullet', 110);
             INSERT INTO rifle_loads (id, rifle_id, bullet_id, notes, updated_at, velocities)
             VALUES ('l1', 'r1', 'b1', '', 120,
                 '[{\"temperature_c\": 12.0, \"velocity_mps\": 781.0}]');
             INSERT INTO deleted (kind, id, deleted_at) VALUES ('scope', 'gone', 90);
             INSERT INTO settings (key, value) VALUES ('units', '\"metric\"');",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO shots (id, data, taken_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                shot.id,
                serde_json::to_string(&shot).unwrap(),
                shot.taken_at,
                shot.updated_at
            ],
        )
        .unwrap();
    drop(connection);

    let store = Store::open(&path).unwrap();
    let version: i64 = store
        .connection_ref()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, crate::db::MIGRATIONS.len() as i64);
    let rifle = store.rifle("r1").unwrap().unwrap();
    assert_eq!(
        (rifle.name.as_str(), rifle.notes.as_str(), rifle.updated_at),
        ("Tikka T3x", "Old rifle", 100)
    );
    assert_eq!(
        (rifle.zero_height_cm, rifle.barrel_length_mm),
        (4.0, Some(510.0))
    );
    assert_eq!(
        (rifle.fitted_barrel_id, rifle.rounds_before_logging),
        (None, 0)
    );
    let bullet = store.bullet("b1").unwrap().unwrap();
    assert_eq!(
        (bullet.notes.as_str(), bullet.updated_at),
        ("Old bullet", 110)
    );
    assert_eq!(bullet.recipe, None);
    let load = store.rifle_load("r1", "b1").unwrap().unwrap();
    assert_eq!(load.velocities[0].velocity_mps, 781.0);
    assert_eq!(store.shots().unwrap(), vec![shot]);
    assert_eq!(store.deletions().unwrap().len(), 1);
    assert_eq!(
        store.setting::<String>("units").unwrap().as_deref(),
        Some("metric")
    );
    assert_eq!(
        store.file_path().map(|file| file.canonicalize().unwrap()),
        Some(path.canonicalize().unwrap())
    );

    // The new tables take records, and the whole lot exports.
    let mut barrel = Barrel::new("r1", "Factory");
    store.save_barrel(&mut barrel).unwrap();
    let mut fitted = store.rifle("r1").unwrap().unwrap();
    fitted.fitted_barrel_id = Some(barrel.id.clone());
    store.save_rifle(&mut fitted).unwrap();
    let export = store.export_data().unwrap();
    assert_eq!((export.rifles.len(), export.barrels.len()), (1, 1));
    let mut copy = Store::open_in_memory().unwrap();
    assert_eq!(copy.import_data(&export, false).unwrap().added, 5);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_record_saved_again_after_its_deletion_keeps_later_edits_against_an_old_backup() {
    let mut store = Store::open_in_memory().unwrap();
    let mut rifle = rifle("Tikka", "308-win");
    store.save_rifle(&mut rifle).unwrap();
    let mut group = Group::on_grid(200.0, 200.0, 1_000);
    group.rifle_id = Some(rifle.id.clone());
    group.notes = "first".into();
    store.save_group(&mut group).unwrap();
    let mut entry = RoundLogEntry::for_group(&group, 5).unwrap();
    store.save_round_log_entry(&mut entry).unwrap();
    let old_backup = store.export_json().unwrap();

    // Deleted, then saved again (Undo, or rounds counted again).
    store.delete_group(&group.id).unwrap();
    store.delete_round_log_entry(&entry.id).unwrap();
    store.delete_rifle(&rifle.id).unwrap();
    store.save_rifle(&mut rifle).unwrap();
    store.save_group(&mut group).unwrap();
    store.save_round_log_entry(&mut entry).unwrap();

    rifle.name = "Tikka T3x".into();
    store.save_rifle(&mut rifle).unwrap();
    group.notes = "edited".into();
    store.save_group(&mut group).unwrap();
    entry.rounds = 7;
    store.save_round_log_entry(&mut entry).unwrap();

    store.import_json(&old_backup).unwrap();
    assert_eq!(store.rifle(&rifle.id).unwrap().unwrap().name, "Tikka T3x");
    assert_eq!(store.group(&group.id).unwrap().unwrap().notes, "edited");
    assert_eq!(
        store
            .round_log_entry_for_group(&group.id)
            .unwrap()
            .unwrap()
            .rounds,
        7
    );
    assert!(
        store.deletions().unwrap().is_empty(),
        "saving took back the deletions"
    );
}

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
fn newer_schema_is_refused() {
    let path = temporary_path();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    assert!(matches!(Store::open(&path), Err(StoreError::Invalid(_))));
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
        2,
        "the rifle and its load"
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

use crate::model::new_id;

fn measured(rifle: &str, bullet: &str, velocity_mps: f64, temperature_c: f64) -> RifleLoad {
    let mut load = RifleLoad::new(rifle, bullet);
    load.velocities.push(VelocityPoint {
        temperature_c,
        velocity_mps,
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
            velocity_mps: 776.0
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
            kept_local: 0
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
            kept_local: 0
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
            kept_local: 1
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
    });
    load.impact_offset_mrad = [0.1, -0.2];
    load.zero = LoadZero::Recorded(ZeroWeather {
        temperature_c: 5.0,
        powder_temperature_c: 3.0,
        pressure_hpa: 990.0,
        humidity_percent: 60.0,
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
    });
    let one = load.velocity_model(&oryx);
    assert_eq!(
        (one.velocity_at(20.0), one.velocity_at(0.0)),
        (800.0, 790.0)
    );

    load.velocities.push(VelocityPoint {
        temperature_c: 0.0,
        velocity_mps: 780.0,
    });
    load.velocities.push(VelocityPoint {
        temperature_c: -20.0,
        velocity_mps: 760.0,
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
            velocity_mps: 781.0
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
            velocity_mps: 781.0
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
        match &target.shape {
            TargetShape::Animal { .. } => assert!(target.is_game(), "{}", target.name),
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

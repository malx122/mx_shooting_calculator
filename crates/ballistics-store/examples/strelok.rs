//! Shows what importing a Strelok Pro backup would do, without writing.
//!
//!     cargo run -p ballistics-store --example strelok -- backup.srl [database.sqlite [--apply]]
//!
//! With a database, the counts say what would be added, updated or left
//! alone in it; without one, against an empty database. `--apply` imports
//! for real (into the given database only).

use ballistics_store::Store;
use ballistics_store::strelok::PlanOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let backup = arguments
        .next()
        .ok_or("usage: strelok <backup.srl> [database.sqlite]")?;
    let xml = std::fs::read_to_string(&backup)?;
    let mut store = match arguments.next() {
        Some(path) => Store::open(path)?,
        None => Store::open_in_memory()?,
    };
    let modified = std::fs::metadata(&backup)?.modified()?;
    let updated_at = modified.duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64;
    let plan = store.plan_strelok(
        &xml,
        &PlanOptions {
            updated_at,
            ..Default::default()
        },
    )?;

    for rifle in &plan.rifles {
        println!(
            "{:32} {:14} {:?}, {} bullet(s)",
            rifle.strelok_name, rifle.caliber, rifle.caliber_source, rifle.bullets
        );
    }
    println!();
    for bullet in &plan.data.bullets {
        let bc = bullet.ballistic_coefficient().unwrap_or_default();
        println!(
            "  {:12} {:12} {:34} {:6.1} gr {:?} {bc:.3}{} {:4.0} m/s",
            bullet.caliber,
            bullet.manufacturer,
            bullet.name,
            bullet.mass_g / ballistics_store::GRAMS_PER_GRAIN,
            bullet.drag_model,
            if bullet.bc_bands.is_empty() {
                String::new()
            } else {
                format!(" ({} bands)", bullet.bc_bands.len())
            },
            bullet.muzzle_velocity_mps,
        );
    }
    println!();
    for warning in &plan.warnings {
        println!("! {warning}");
    }
    let apply = arguments.next().as_deref() == Some("--apply");
    let report = store.import_data(&plan.data, !apply)?;
    println!(
        "\n{} add {}, update {}, leave {} unchanged and keep {} edited locally.",
        if apply { "Did" } else { "Would" },
        report.added,
        report.updated,
        report.unchanged,
        report.kept_local
    );
    Ok(())
}

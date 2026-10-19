//! Rebuilds the development database from `dev-data/sample.json`.
//!
//!     cargo seed                      # target/dev.sqlite
//!     cargo seed -- other.sqlite      # another file (must not exist, or pass --force)
//!
//! Then run the app on it with `cargo dev`.

use std::path::PathBuf;

use ballistics_store::Store;

const SAMPLE: &str = include_str!("../../../dev-data/sample.json");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut force = false;
    let mut custom = None;
    for argument in std::env::args_os().skip(1) {
        if argument == "--force" {
            force = true;
        } else {
            custom = Some(PathBuf::from(argument));
        }
    }
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/dev.sqlite");
    // Only the default dev database is replaced without asking, so a typo
    // can never wipe a real database.
    if let Some(path) = &custom
        && path.exists()
        && !force
    {
        return Err(format!("{} exists; pass --force to replace it", path.display()).into());
    }
    let path = custom.unwrap_or(default);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        match std::fs::remove_file(&file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }

    let mut store = Store::open(&path)?;
    let report = store.import_json(SAMPLE)?;
    let path = path.canonicalize()?;
    println!(
        "Seeded {} with {} records ({} rifles, {} bullets).",
        path.display(),
        report.added,
        store.rifles()?.len(),
        store.bullets()?.len()
    );
    println!(
        "Start the app on it with `cargo dev`, or MXSC_DB=\"{}\" cargo run",
        path.display()
    );
    Ok(())
}

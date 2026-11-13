// A released Windows build opens no console window beside the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bullets;
mod calculator;
mod cards;
mod cartridge3d;
mod changelog;
mod figure;
mod files;
mod flags;
mod google;
mod hits;
mod http;
mod hunting;
mod i18n;
mod import;
mod loads;
mod mobile;
mod number_field;
#[cfg(target_arch = "wasm32")]
mod relaxed_idb;
mod rifles;
mod sensors;
mod shot_log;
mod sight_picture;
mod startup;
mod sync;
mod targets;
mod theme;
mod trajectory3d;
mod units;
#[cfg(not(target_arch = "wasm32"))]
mod updates;
mod watch;
mod weather;
#[cfg(target_arch = "wasm32")]
mod web;
mod widgets;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use ballistics_store::Store;
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;
use startup::StartupError;

pub const APP_NAME: &str = "MX Shooting Calculator";
/// The app's version (from Cargo.toml) and the git commit it was built from.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("MXSC_COMMIT");

/// The database lives in the platform's app-data folder, unless `--db <file>`
/// or `MXSC_DB` points elsewhere (for development data or a portable install).
#[cfg(not(target_arch = "wasm32"))]
fn database_path() -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--db" {
            return arguments
                .next()
                .map(PathBuf::from)
                .ok_or("--db needs a file path".to_owned());
        }
    }
    if let Some(path) = std::env::var_os("MXSC_DB") {
        return Ok(PathBuf::from(path));
    }
    let directories = directories::ProjectDirs::from("", "", APP_NAME)
        .ok_or("could not find a folder for app data")?;
    let folder = directories.data_dir();
    std::fs::create_dir_all(folder)
        .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    let path = folder.join("mxsc.sqlite");
    adopt_previous_database(&path)?;
    Ok(path)
}

/// The app's earlier names and their database files, newest first.
#[cfg(not(target_arch = "wasm32"))]
const PREVIOUS_DATABASES: [(&str, &str); 2] = [
    ("Malx Hunting Calculator", "mhc.sqlite"),
    ("Ballistic Calculator", "ballistics.sqlite"),
];

/// The app had other names before; on first start, copy the newest
/// database found under one of them so nothing is lost. The old file is
/// left in place.
#[cfg(not(target_arch = "wasm32"))]
fn adopt_previous_database(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let Some(old) = PREVIOUS_DATABASES.iter().find_map(|(name, file)| {
        directories::ProjectDirs::from("", "", name)
            .map(|directories| directories.data_dir().join(file))
            .filter(|old| old.exists())
    }) else {
        return Ok(());
    };
    std::fs::copy(&old, path).map(|_| ()).map_err(|error| {
        format!(
            "could not copy {} to {}: {error}",
            old.display(),
            path.display()
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let store = match database_path() {
        Ok(path) => Store::open(&path).map_err(|error| StartupError::opening(path, &error)),
        Err(message) => Err(StartupError::new(message)),
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: theme::window_icon().into_iter().fold(
            egui::ViewportBuilder::default()
                .with_title(APP_NAME)
                .with_app_id("mxsc")
                .with_inner_size([1360.0, 880.0])
                .with_min_inner_size([900.0, 600.0]),
            |viewport, icon| viewport.with_icon(icon),
        ),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, app_creator(store))
}

/// In the browser the data is kept in the browser's own storage.
#[cfg(target_arch = "wasm32")]
fn main() {
    web::start(|store| app_creator(store.map_err(StartupError::new)));
}

/// Builds the app once eframe is ready, or a page explaining why the
/// database could not be opened.
fn app_creator(store: Result<Store, StartupError>) -> eframe::AppCreator<'static> {
    Box::new(|creation_context| {
        theme::apply(&creation_context.egui_ctx);
        cards::fonts::install(&creation_context.egui_ctx);
        match store {
            Ok(store) => Ok(Box::new(app::App::new(store)) as Box<dyn eframe::App>),
            Err(error) => Ok(Box::new(error)),
        }
    })
}

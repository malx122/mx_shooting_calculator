//! The page shown when the database can't be opened. On the desktop it can
//! still update the app, the way to open data saved by a newer version, and
//! start over with an empty database, the old file kept under a new name.

use eframe::egui;

use crate::i18n::tr;

pub struct StartupError {
    message: String,
    /// The data was saved by a newer version of the app.
    newer: bool,
    #[cfg(not(target_arch = "wasm32"))]
    desktop: Desktop,
}

/// What the desktop can do about it.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Desktop {
    /// The database file, when it was found.
    path: Option<std::path::PathBuf>,
    /// The file is from a newer version or damaged: offer to start over.
    can_reset: bool,
    /// Check for updates at once: the hunter turned that on.
    check_at_start: bool,
    updates: crate::updates::Updater,
    /// When the reset dialog was opened, and whether the hunter has ticked
    /// that they understand.
    asked_to_reset: Option<(f64, bool)>,
    /// What went wrong resetting.
    reset_failed: Option<String>,
    /// The app, started on a new database after a reset.
    started: Option<Box<crate::app::App>>,
}

impl StartupError {
    pub fn new(message: String) -> StartupError {
        crate::i18n::set(crate::i18n::Language::system());
        StartupError {
            message,
            newer: false,
            #[cfg(not(target_arch = "wasm32"))]
            desktop: Desktop::default(),
        }
    }

    /// The database at `path` failed to open with `error`. Whether to check
    /// for updates is read from the file itself, without opening it as a
    /// store.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn opening(path: std::path::PathBuf, error: &ballistics_store::StoreError) -> StartupError {
        let mut page = StartupError::new(format!("could not open {}: {error}", path.display()));
        // In the hunter's language, read like the setting below.
        let language = ballistics_store::Store::peek_setting::<Option<crate::i18n::Language>>(
            &path,
            crate::app::LANGUAGE_SETTING,
        )
        .flatten();
        crate::i18n::set(crate::i18n::Language::chosen(language));
        page.newer = matches!(error, ballistics_store::StoreError::Newer(_));
        // Starting over only helps when no retry will open the file, not
        // when it is locked or can't be read just now.
        page.desktop.can_reset = page.newer || error.is_corrupt();
        page.desktop.check_at_start =
            ballistics_store::Store::peek_setting(&path, crate::updates::SETTING).unwrap_or(false);
        page.desktop.path = Some(path);
        page
    }
}

impl eframe::App for StartupError {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(app) = &mut self.desktop.started {
                return app.ui(ui, frame);
            }
            if std::mem::take(&mut self.desktop.check_at_start) {
                self.desktop.updates.check(ui.ctx());
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = frame;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(tr("The database could not be opened"));
                if self.newer {
                    ui.label(tr(
                        "Your data was saved by a newer version of the app. Update this one to open it.",
                    ));
                }
                ui.colored_label(ui.visuals().error_fg_color, &self.message);
                #[cfg(not(target_arch = "wasm32"))]
                self.desktop.show(ui, self.newer);
            });
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Desktop {
    fn show(&mut self, ui: &mut egui::Ui, newer: bool) {
        ui.add_space(16.0);
        ui.label(crate::i18n::trf!("This is version {}.", crate::VERSION));
        let busy = matches!(
            self.updates.state(),
            crate::updates::State::Checking | crate::updates::State::Downloading(_)
        );
        if ui
            .add_enabled(!busy, egui::Button::new(tr("Check now")))
            .clicked()
        {
            self.updates.check(ui.ctx());
        }
        ui.weak(tr(
            "This asks GitHub, where the app's releases are kept, for the latest version. \
             Nothing else is sent.",
        ));
        self.updates.show(ui);

        let Some(path) = self.path.clone().filter(|_| self.can_reset) else {
            return;
        };
        ui.add_space(24.0);
        ui.separator();
        ui.strong(tr("Start over"));
        ui.label(if newer {
            tr(
                "If you can't update, the app can start over with an empty database. \
                The current file is kept under a new name, so you can open it again after \
                updating.",
            )
        } else {
            tr(
                "The app can start over with an empty database. The current file is kept, \
                under a new name, so it can still be repaired.",
            )
        });
        if ui.button(tr("Reset the database…")).clicked() {
            self.asked_to_reset = Some((ui.input(|input| input.time), false));
            self.reset_failed = None;
        }
        if let Some(message) = &self.reset_failed {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
        if self.asked_to_reset.is_some() {
            self.reset_dialog(ui.ctx(), &path);
        }
    }

    /// Starting over gets a dialog of its own, with a box to tick and a
    /// moment's wait, not a second click on the same spot.
    fn reset_dialog(&mut self, context: &egui::Context, path: &std::path::Path) {
        let Some((asked_at, mut understood)) = self.asked_to_reset else {
            return;
        };
        let width = (context.content_rect().width() - 48.0).min(440.0);
        let mut reset = false;
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("reset-database")).show(context, |ui| {
            ui.set_width(width);
            ui.heading(tr("Reset the database?"));
            ui.label(tr(
                "The app starts with no rifles, bullets, scopes, targets or shot log, \
                 with its settings reset and sync off. Nothing is deleted: the current file is \
                 renamed and stays in its folder:",
            ));
            ui.monospace(path.display().to_string());
            ui.label(tr(
                "To open it again, close the app, move the new file away and give the old one \
                 back its name: the app then uses it again.",
            ));
            ui.add_space(8.0);
            ui.checkbox(
                &mut understood,
                tr("I understand that the app starts empty"),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
                let ready = crate::widgets::confirm_ready(ui, asked_at);
                if ui
                    .add_enabled(
                        ready && understood,
                        egui::Button::new(
                            egui::RichText::new(tr("Reset the database"))
                                .color(ui.visuals().error_fg_color),
                        ),
                    )
                    .clicked()
                {
                    reset = true;
                }
            });
        });
        self.asked_to_reset = Some((asked_at, understood));
        if close || modal.should_close() {
            self.asked_to_reset = None;
        }
        if reset {
            self.asked_to_reset = None;
            self.reset(path);
        }
    }

    /// Sets the file aside and starts the app on a new one.
    fn reset(&mut self, path: &std::path::Path) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64);
        let started = ballistics_store::Store::set_aside(path, now)
            .and_then(|aside| Ok((aside, ballistics_store::Store::open(path)?)));
        match started {
            Ok((aside, store)) => {
                let mut app = crate::app::App::new(store);
                app.keep_message(crate::i18n::trf!(
                    "Started over with an empty database. The old one is kept as {}.",
                    aside.display()
                ));
                self.started = Some(Box::new(app));
            }
            Err(error) => self.reset_failed = Some(error.to_string()),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use ballistics_store::{Store, StoreError};

    #[test]
    fn a_damaged_database_can_be_set_aside_and_the_app_started() {
        let folder = std::env::temp_dir().join(format!("mxsc-startup-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("mxsc.sqlite");
        std::fs::write(&path, vec![b'x'; 4096]).unwrap();
        let error = Store::open(&path).err().unwrap();
        let mut page = StartupError::opening(path.clone(), &error);
        assert!(page.desktop.can_reset && !page.newer);

        page.desktop.reset(&path);
        assert!(page.desktop.reset_failed.is_none());
        assert!(page.desktop.started.is_some());
        let kept: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("mxsc-reset-") && name.ends_with(".sqlite"))
            .collect();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!(
            std::fs::read(folder.join(&kept[0])).unwrap(),
            vec![b'x'; 4096]
        );
        assert!(Store::open(&path).unwrap().rifles().unwrap().is_empty());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn starting_over_is_offered_only_when_no_retry_helps() {
        let path = std::env::temp_dir().join("mxsc-startup-not-there.sqlite");
        let newer = StartupError::opening(path.clone(), &StoreError::Newer(99));
        assert!(newer.newer && newer.desktop.can_reset);
        let locked = StartupError::opening(path, &StoreError::Invalid("locked".into()));
        assert!(!locked.desktop.can_reset);
    }
}

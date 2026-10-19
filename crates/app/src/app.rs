//! The application shell: navigation, the cached copy of the database, and
//! applying what the pages ask for. Pages never touch the store directly;
//! they return `Action`s, which keeps them easy to reuse on mobile.

use ballistics_store::{
    Bullet, Catalog, PLAIN_CROSSHAIR, Reticle, Rifle, RifleLoad, Scope, Store, StoreError, Target,
    UserScope,
};
use eframe::egui::{self, RichText};

use crate::bullets::BulletsPage;
use crate::calculator::{CalculatorPage, Inputs};
use crate::import::{self, Decision, ImportPreview};
use crate::rifles::RiflesPage;
use crate::targets::TargetsPage;
use crate::theme;
use crate::units::{self, UnitPrefs};

const CALCULATOR_SETTINGS: &str = "calculator";
const UNIT_SETTINGS: &str = "units";

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Calculate,
    Rifles,
    Bullets,
    Targets,
}

pub enum Action {
    Goto(Page),
    ShowBulletsFor(String),
    /// Open a rifle in the Rifles page editor.
    EditRifle(String),
    SaveRifle(Rifle),
    DeleteRifle(String),
    SaveBullet(Bullet),
    DeleteBullet(String),
    SaveLoad(RifleLoad),
    DeleteLoad(String),
    SaveScope(UserScope),
    DeleteScope(String),
    SaveTarget(Target),
    DeleteTarget(String),
}

/// The hunter's records, reloaded from the store after every change.
#[derive(Default)]
pub struct Data {
    pub rifles: Vec<Rifle>,
    pub bullets: Vec<Bullet>,
    pub loads: Vec<RifleLoad>,
    pub scopes: Vec<UserScope>,
    pub targets: Vec<Target>,
}

impl Data {
    /// A rifle's scope: one of the hunter's own, or from the catalog.
    pub fn scope(&self, id: &str) -> Option<Scope> {
        match self.scopes.iter().find(|scope| scope.id == id) {
            Some(custom) => Some(custom.to_scope()),
            None => Catalog::builtin().scope(id).cloned(),
        }
    }

    /// A target: one of the hunter's own, or from the catalog.
    pub fn target(&self, id: &str) -> Option<Target> {
        match self.targets.iter().find(|target| target.id == id) {
            Some(custom) => Some(custom.clone()),
            None => Catalog::builtin().target(id).cloned(),
        }
    }

    /// The rifle's scope and reticle, if both can be found.
    pub fn optics(&self, rifle: &Rifle) -> Option<(Scope, Reticle)> {
        let scope = self.scope(rifle.scope_id.as_deref()?)?;
        let reticle = match rifle.reticle_id.as_deref()? {
            PLAIN_CROSSHAIR => Reticle::plain_crosshair(),
            id => Catalog::builtin().reticle(id)?.clone(),
        };
        Some((scope, reticle))
    }
}

struct Status {
    text: String,
    error: bool,
}

pub struct App {
    store: Store,
    data: Data,
    page: Page,
    calculator: CalculatorPage,
    rifles: RiflesPage,
    bullets: BulletsPage,
    targets: TargetsPage,
    saved_inputs: Inputs,
    units: UnitPrefs,
    status: Option<Status>,
    /// An import being previewed.
    import: Option<ImportPreview>,
    /// A backup picked in the browser, waiting to be previewed.
    #[cfg(target_arch = "wasm32")]
    picked: crate::web::PickedSlot,
}

impl App {
    pub fn new(store: Store) -> Self {
        let inputs: Inputs = store
            .setting(CALCULATOR_SETTINGS)
            .ok()
            .flatten()
            .unwrap_or_default();
        let units: UnitPrefs = store
            .setting(UNIT_SETTINGS)
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut app = Self {
            store,
            data: Data::default(),
            page: Page::Calculate,
            calculator: CalculatorPage::new(inputs.clone()),
            rifles: RiflesPage::default(),
            bullets: BulletsPage::default(),
            targets: TargetsPage::default(),
            saved_inputs: inputs,
            units,
            status: None,
            import: None,
            #[cfg(target_arch = "wasm32")]
            picked: Default::default(),
        };
        app.reload();
        if app.data.rifles.is_empty() {
            app.page = Page::Rifles;
        }
        app
    }

    fn reload(&mut self) {
        let loaded = (|| {
            Ok::<_, StoreError>(Data {
                rifles: self.store.rifles()?,
                bullets: self.store.bullets()?,
                loads: self.store.rifle_loads()?,
                scopes: self.store.scopes()?,
                targets: self.store.targets()?,
            })
        })();
        match loaded {
            Ok(data) => self.data = data,
            Err(error) => self.report(Err(error)),
        }
    }

    fn report(&mut self, result: Result<String, StoreError>) {
        self.status = Some(match result {
            Ok(text) => Status { text, error: false },
            Err(error) => Status {
                text: error.to_string(),
                error: true,
            },
        });
    }

    fn apply(&mut self, action: Action) {
        let result = match action {
            Action::Goto(page) => {
                self.page = page;
                return;
            }
            Action::EditRifle(id) => {
                if let Some(rifle) = self.data.rifles.iter().find(|rifle| rifle.id == id) {
                    self.rifles.edit(rifle);
                }
                self.page = Page::Rifles;
                return;
            }
            Action::ShowBulletsFor(caliber) => {
                self.bullets.show_caliber(&caliber);
                self.page = Page::Bullets;
                return;
            }
            Action::SaveRifle(mut rifle) => self.store.save_rifle(&mut rifle).map(|()| {
                self.rifles.saved(&rifle);
                format!("Saved {}", rifle.name)
            }),
            Action::DeleteRifle(id) => self.store.delete_rifle(&id).map(|()| {
                self.rifles.deleted(&id);
                "Rifle deleted".to_owned()
            }),
            Action::SaveBullet(mut bullet) => self.store.save_bullet(&mut bullet).map(|()| {
                self.bullets.saved(&bullet);
                format!("Saved {}", bullet.display_name())
            }),
            Action::DeleteBullet(id) => self.store.delete_bullet(&id).map(|()| {
                self.bullets.deleted(&id);
                "Bullet deleted".to_owned()
            }),
            Action::SaveLoad(mut load) => self
                .store
                .save_rifle_load(&mut load)
                .map(|()| "Saved this bullet's data for the rifle".to_owned()),
            Action::SaveScope(mut scope) => self.store.save_scope(&mut scope).map(|()| {
                self.rifles.scope_saved(&scope);
                format!("Saved {}", scope.display_name())
            }),
            Action::DeleteScope(id) => self.store.delete_scope(&id).map(|()| {
                self.rifles.scope_deleted(&id);
                "Scope deleted; rifles that had it now have none".to_owned()
            }),
            Action::SaveTarget(mut target) => self.store.save_target(&mut target).map(|()| {
                self.targets.saved(&target);
                format!("Saved {}", target.name)
            }),
            Action::DeleteTarget(id) => self.store.delete_target(&id).map(|()| {
                self.targets.deleted(&id);
                "Target deleted".to_owned()
            }),
            Action::DeleteLoad(id) => self.store.delete_rifle_load(&id).map(|()| {
                self.calculator.forget_load_draft();
                "Using the bullet's own data again".to_owned()
            }),
        };
        self.report(result);
        self.reload();
    }

    fn export(&mut self) {
        let json = match self.store.export_json() {
            Ok(json) => json,
            Err(error) => return self.report(Err(error)),
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .set_file_name("mxsc-backup.json")
                .add_filter("JSON", &["json"])
                .save_file()
            else {
                return;
            };
            let result = std::fs::write(&path, json)
                .map(|()| format!("Exported to {}", path.display()))
                .map_err(|error| {
                    StoreError::Invalid(format!("could not write {}: {error}", path.display()))
                });
            self.report(result);
        }
        #[cfg(target_arch = "wasm32")]
        self.report(
            crate::web::download("mxsc-backup.json", &json)
                .map(|()| "Exported: saved as mxsc-backup.json in your downloads".to_owned())
                .map_err(StoreError::Invalid),
        );
    }

    /// Opens a backup for the import window: this app's JSON export or a
    /// Strelok Pro backup.
    #[cfg(not(target_arch = "wasm32"))]
    fn import(&mut self, _context: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Backups", &["json", "srl"])
            .add_filter("MX Shooting Calculator (JSON)", &["json"])
            .add_filter("Strelok Pro (.srl)", &["srl"])
            .pick_file()
        else {
            return;
        };
        match ImportPreview::open(&mut self.store, &path) {
            Ok(preview) => self.import = Some(preview),
            Err(error) => self.report(Err(error)),
        }
    }

    /// In the browser the picker answers later: the file is collected by
    /// `collect_picked_file` on a following frame.
    #[cfg(target_arch = "wasm32")]
    fn import(&mut self, context: &egui::Context) {
        crate::web::pick_backup(self.picked.clone(), context.clone());
    }

    #[cfg(target_arch = "wasm32")]
    fn collect_picked_file(&mut self) {
        let Some(file) = self.picked.borrow_mut().take() else {
            return;
        };
        match ImportPreview::from_text(
            &mut self.store,
            file.name,
            file.text,
            Some(file.modified_ms),
        ) {
            Ok(preview) => self.import = Some(preview),
            Err(error) => self.report(Err(error)),
        }
    }

    fn import_window(&mut self, context: &egui::Context) {
        let Some(preview) = &mut self.import else {
            return;
        };
        match preview.show(context, &mut self.store) {
            Ok(Decision::Undecided) => {}
            Ok(Decision::Cancel) => self.import = None,
            Ok(Decision::Imported(report, selected)) => {
                self.import = None;
                self.reload();
                if let Some((rifle_id, bullet_id)) = selected {
                    self.calculator.inputs.rifle_id = Some(rifle_id);
                    self.calculator.inputs.bullet_id = bullet_id;
                    self.page = Page::Calculate;
                }
                self.report(Ok(format!("Imported: {}", import::summary(&report))));
            }
            Err(error) => {
                self.import = None;
                self.report(Err(error));
            }
        }
    }

    /// Saves the calculator inputs when they change, but not mid-drag.
    fn persist_inputs(&mut self, context: &egui::Context) {
        let inputs = &self.calculator.inputs;
        if *inputs != self.saved_inputs && !context.input(|input| input.pointer.any_down()) {
            match self.store.set_setting(CALCULATOR_SETTINGS, inputs) {
                Ok(()) => self.saved_inputs = inputs.clone(),
                Err(error) => self.report(Err(error)),
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.units.install(context);
        let header = egui::Frame::new()
            .fill(theme::FOREST_900)
            .inner_margin(egui::Margin::symmetric(14, 8))
            .stroke(theme::hairline(theme::FOREST_600));
        egui::TopBottomPanel::top("navigation")
            .frame(header)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    brand(ui);
                    ui.add_space(28.0);
                    for (page, label) in [
                        (Page::Calculate, "Calculate"),
                        (Page::Rifles, "Rifles"),
                        (Page::Bullets, "My bullets"),
                        (Page::Targets, "Targets"),
                    ] {
                        ui.selectable_value(&mut self.page, page, RichText::new(label).size(15.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut changed = false;
                        ui.menu_button("Units", |ui| changed = units::menu(ui, &mut self.units));
                        if changed {
                            self.units.install(context);
                            if let Err(error) = self.store.set_setting(UNIT_SETTINGS, &self.units) {
                                self.report(Err(error));
                            }
                        }
                        if ui
                            .button("Import…")
                            .on_hover_text("Merge a backup into this device's data")
                            .clicked()
                        {
                            self.import(context);
                        }
                        if ui
                            .button("Export…")
                            .on_hover_text("Save your rifles and bullets as JSON")
                            .clicked()
                        {
                            self.export();
                        }
                    });
                });
            });
        if let Some(status) = &self.status {
            let mut dismiss = false;
            egui::TopBottomPanel::bottom("status").show(context, |ui| {
                ui.horizontal(|ui| {
                    let text = RichText::new(&status.text);
                    if status.error {
                        ui.colored_label(ui.visuals().error_fg_color, text);
                    } else {
                        ui.label(text);
                    }
                    dismiss = ui.small_button("✖").clicked();
                });
            });
            if dismiss {
                self.status = None;
            }
        }

        let actions = match self.page {
            Page::Calculate => self.calculator.show(context, &self.data),
            Page::Rifles => self.rifles.show(context, &self.data),
            Page::Bullets => self.bullets.show(context, &self.data),
            Page::Targets => self.targets.show(context, &self.data),
        };
        for action in actions {
            self.apply(action);
        }
        #[cfg(target_arch = "wasm32")]
        self.collect_picked_file();
        self.import_window(context);
        self.persist_inputs(context);
    }
}

/// The logo in the header: icon, MX wordmark and tagline, all vector.
fn brand(ui: &mut egui::Ui) {
    ui.add(egui::Image::new(theme::icon()).fit_to_exact_size(egui::vec2(36.0, 36.0)));
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add(egui::Image::new(theme::wordmark()).fit_to_exact_size(egui::vec2(34.0, 19.0)));
        ui.label(
            RichText::new("SHOOTING CALCULATOR")
                .size(9.5)
                .strong()
                .color(theme::BLAZE)
                .extra_letter_spacing(1.6),
        );
    });
}

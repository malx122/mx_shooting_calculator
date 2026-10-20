//! The application shell: navigation, the cached copy of the database, and
//! applying what the pages ask for. Pages never touch the store directly;
//! they return `Action`s, which keeps them easy to reuse on mobile.

use ballistics_store::{
    Bullet, Catalog, Country, PLAIN_CROSSHAIR, Reticle, Rifle, RifleLoad, Scope, Store, StoreError,
    Target, UserScope,
};
use eframe::egui::{self, RichText};

use crate::bullets::BulletsPage;
use crate::calculator::{CalculatorPage, Inputs};
use crate::cards::{CardSettings, CardsPage};
use crate::import::{self, Decision, ImportPreview};
use crate::mobile;
use crate::rifles::RiflesPage;
use crate::sync::{self, Sync};
use crate::targets::TargetsPage;
use crate::theme;
use crate::units::{self, UnitPrefs};

const CALCULATOR_SETTINGS: &str = "calculator";
/// The card choices and the hunter's stands.
const CARD_SETTINGS: &str = "cards";
/// Whose hunting rules to check: a country, or `null` for none. Absent
/// until the hunter chooses; then the device's language decides.
const COUNTRY_SETTING: &str = "country";
/// The version the hunter last used, to mention an update once.
const SEEN_VERSION: &str = "seen_version";
const UNIT_SETTINGS: &str = "units";

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Calculate,
    Rifles,
    Bullets,
    Targets,
    Cards,
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
    /// Load the built-in demo rifles, bullets, scope and target.
    LoadDemo,
}

/// The hunter's records, reloaded from the store after every change.
#[derive(Default)]
pub struct Data {
    pub rifles: Vec<Rifle>,
    pub bullets: Vec<Bullet>,
    pub loads: Vec<RifleLoad>,
    pub scopes: Vec<UserScope>,
    pub targets: Vec<Target>,
    /// Whose hunting rules to check, if any.
    pub country: Option<Country>,
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

/// The phone menu's sheets.
#[derive(Clone, Copy, PartialEq)]
enum MenuSheet {
    Main,
    Units,
    Country,
    Sync,
    WhatsNew,
    /// How to put the app on the home screen by hand.
    Install,
}

/// What a tap in the phone menu asked for, done once the sheet is drawn.
enum MenuChoice {
    Units,
    Country,
    Sync,
    WhatsNew,
    Import,
    Export,
    Demo,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Install,
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
    cards: CardsPage,
    saved_cards: CardSettings,
    saved_inputs: Inputs,
    units: UnitPrefs,
    status: Option<Status>,
    /// An import being previewed.
    import: Option<ImportPreview>,
    /// The phone menu, when open.
    menu: Option<MenuSheet>,
    sync: Sync,
    country: Option<Country>,
    /// The desktop's What's new window is open.
    whats_new_open: bool,
    /// The hunter's data changed this frame: sync it.
    data_changed: bool,
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
        let cards: CardSettings = store
            .setting(CARD_SETTINGS)
            .ok()
            .flatten()
            .unwrap_or_default();
        #[allow(unused_mut)]
        let mut sync = Sync::new(&store);
        #[cfg(target_arch = "wasm32")]
        {
            sync.offered = crate::web::take_sync_link();
        }
        let country = match store.setting::<Option<Country>>(COUNTRY_SETTING) {
            Ok(Some(chosen)) => chosen,
            _ => sys_locale::get_locale()
                .as_deref()
                .and_then(Country::from_locale),
        };
        let mut app = Self {
            sync,
            country,
            whats_new_open: false,
            data_changed: false,
            store,
            data: Data::default(),
            page: Page::Calculate,
            calculator: CalculatorPage::new(inputs.clone()),
            rifles: RiflesPage::default(),
            bullets: BulletsPage::default(),
            targets: TargetsPage::default(),
            cards: CardsPage::new(cards.clone()),
            saved_cards: cards,
            saved_inputs: inputs,
            units,
            status: None,
            import: None,
            menu: None,
            #[cfg(target_arch = "wasm32")]
            picked: Default::default(),
        };
        app.reload();
        app.mention_update();
        if app.data.rifles.is_empty() {
            app.page = Page::Rifles;
        }
        app
    }

    /// Says once that the app was updated (not on the very first start).
    fn mention_update(&mut self) {
        let seen: Option<String> = self.store.setting(SEEN_VERSION).ok().flatten();
        if seen.as_deref() == Some(crate::VERSION) {
            return;
        }
        let used_before = seen.is_some() || !self.data.rifles.is_empty();
        if used_before {
            self.status = Some(Status {
                text: format!(
                    "Updated to version {}. What's new: click the version number (on a phone, \
                     in the menu).",
                    crate::VERSION
                ),
                error: false,
            });
        }
        let _ = self.store.set_setting(SEEN_VERSION, &crate::VERSION);
    }

    fn reload(&mut self) {
        let loaded = (|| {
            Ok::<_, StoreError>(Data {
                rifles: self.store.rifles()?,
                bullets: self.store.bullets()?,
                loads: self.store.rifle_loads()?,
                scopes: self.store.scopes()?,
                targets: self.store.targets()?,
                country: self.country,
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
            Action::LoadDemo => self.store.load_demo().map(|report| {
                self.page = Page::Calculate;
                format!("Demo data: {}", import::summary(&report))
            }),
            Action::DeleteLoad(id) => self.store.delete_rifle_load(&id).map(|()| {
                self.calculator.forget_load_draft();
                "Using the bullet's own data again".to_owned()
            }),
        };
        self.data_changed |= result.is_ok();
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
                self.data_changed = true;
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
        let dragging = context.input(|input| input.pointer.any_down());
        if *inputs != self.saved_inputs && !dragging {
            match self.store.set_setting(CALCULATOR_SETTINGS, inputs) {
                Ok(()) => self.saved_inputs = inputs.clone(),
                Err(error) => self.report(Err(error)),
            }
        }
        let cards = &self.cards.settings;
        if *cards != self.saved_cards && !dragging {
            match self.store.set_setting(CARD_SETTINGS, cards) {
                Ok(()) => self.saved_cards = cards.clone(),
                Err(error) => self.report(Err(error)),
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.units.install(context);
        mobile::apply_style(context);
        let narrow = mobile::is_narrow(context);
        if narrow {
            self.phone_header(context);
            self.tab_bar(context);
        } else {
            self.desktop_header(context);
        }
        if let Some(status) = &self.status {
            let mut dismiss = false;
            egui::TopBottomPanel::bottom("status").show(context, |ui| {
                ui.horizontal_wrapped(|ui| {
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
            Page::Cards => {
                let subject = self.calculator.card_subject(&self.data);
                self.cards.show(context, subject)
            }
        };
        for action in actions {
            self.apply(action);
        }
        if narrow {
            self.menu_sheet(context);
        } else {
            self.menu = None;
        }
        #[cfg(target_arch = "wasm32")]
        self.collect_picked_file();
        self.import_window(context);
        self.persist_inputs(context);
        self.run_sync(context);
    }
}

impl App {
    /// Syncs with the hunter's other devices, and shows the sync window
    /// and any pairing offer.
    fn run_sync(&mut self, context: &egui::Context) {
        if std::mem::take(&mut self.data_changed) {
            self.sync.changed(context.input(|input| input.time));
        }
        match self.sync.poll(&mut self.store, context) {
            Some(sync::Event::Merged(report)) => {
                self.reload();
                let changes = report.added + report.updated + report.deleted;
                self.report(Ok(format!(
                    "Synced: {changes} change{} from your other devices",
                    if changes == 1 { "" } else { "s" }
                )));
            }
            Some(sync::Event::Stopped(message)) => self.report(Ok(message)),
            None => {}
        }
        let mut open = self.whats_new_open;
        egui::Window::new("What's new")
            .open(&mut open)
            .collapsible(false)
            .default_size([520.0, 560.0])
            .show(context, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, crate::changelog::show);
            });
        self.whats_new_open = open;
        let mut open = self.sync.window_open;
        egui::Window::new("Sync")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            // Under the Sync button, where it stays when its content changes.
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 60.0))
            .show(context, |ui| self.sync.panel(ui, &self.store));
        self.sync.window_open = open;
        self.sync.offer_window(context, &self.store);
    }

    fn desktop_header(&mut self, context: &egui::Context) {
        let header = egui::Frame::new()
            .fill(theme::FOREST_900)
            .inner_margin(egui::Margin::symmetric(14, 8))
            .stroke(theme::hairline(theme::FOREST_600));
        egui::TopBottomPanel::top("navigation")
            .frame(header)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    if brand(ui) {
                        self.whats_new_open = !self.whats_new_open;
                    }
                    ui.add_space(28.0);
                    for (page, label) in [
                        (Page::Calculate, "Calculate"),
                        (Page::Rifles, "Rifles"),
                        (Page::Bullets, "My bullets"),
                        (Page::Targets, "Targets"),
                        (Page::Cards, "Cards"),
                    ] {
                        ui.selectable_value(&mut self.page, page, RichText::new(label).size(15.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut changed = false;
                        ui.menu_button("Units", |ui| changed = units::menu(ui, &mut self.units));
                        if changed {
                            self.units_changed(context);
                        }
                        let mut chosen = None;
                        let button = ui
                            .menu_button(country_label(self.country), |ui| {
                                chosen = country_menu(ui, self.country);
                            })
                            .response
                            .on_hover_text("Whose hunting rules to check ammunition against");
                        if let Some(country) = self.country {
                            crate::flags::paint(
                                ui.painter(),
                                button.rect.left_center() + egui::vec2(8.0, 0.0),
                                country,
                                11.0,
                            );
                        }
                        if let Some(country) = chosen {
                            self.set_country(country);
                        }
                        let now = context.input(|input| input.time);
                        let sync = ui.button("     Sync").on_hover_text(self.sync.status(now));
                        // The font has no dot, so paint one.
                        ui.painter().circle_filled(
                            sync.rect.left_center() + egui::vec2(13.0, 0.0),
                            4.0,
                            self.sync.colour(),
                        );
                        if sync.clicked() {
                            self.sync.window_open = !self.sync.window_open;
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
    }

    fn set_country(&mut self, country: Option<Country>) {
        self.country = country;
        self.data.country = country;
        if let Err(error) = self.store.set_setting(COUNTRY_SETTING, &country) {
            self.report(Err(error));
        }
    }

    fn units_changed(&mut self, context: &egui::Context) {
        self.units.install(context);
        if let Err(error) = self.store.set_setting(UNIT_SETTINGS, &self.units) {
            self.report(Err(error));
        }
    }

    /// The phone's header: the logo and the menu button.
    fn phone_header(&mut self, context: &egui::Context) {
        let header = egui::Frame::new()
            .fill(theme::FOREST_900)
            .inner_margin(egui::Margin::symmetric(12, 6))
            .stroke(theme::hairline(theme::FOREST_600));
        egui::TopBottomPanel::top("phone-header")
            .frame(header)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(theme::icon()).fit_to_exact_size(egui::vec2(30.0, 30.0)),
                    );
                    ui.add(
                        egui::Image::new(theme::wordmark())
                            .fit_to_exact_size(egui::vec2(32.0, 18.0)),
                    );
                    ui.label(
                        RichText::new(format!("v{}", crate::VERSION))
                            .small()
                            .color(theme::BONE_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let menu = ui.add(
                            egui::Button::new(RichText::new("•••").color(theme::BONE))
                                .min_size(egui::vec2(mobile::TOUCH, mobile::TOUCH)),
                        );
                        if menu.on_hover_text("Menu").clicked() {
                            self.menu = Some(MenuSheet::Main);
                        }
                    });
                });
            });
    }

    /// The phone's tab bar: one tab per page.
    fn tab_bar(&mut self, context: &egui::Context) {
        let frame = egui::Frame::new()
            .fill(theme::FOREST_900)
            .inner_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 4,
                bottom: 10,
            })
            .stroke(theme::hairline(theme::FOREST_600));
        egui::TopBottomPanel::bottom("tabs")
            .frame(frame)
            .show(context, |ui| {
                let tabs = [
                    (Page::Calculate, "Calculate"),
                    (Page::Rifles, "Rifles"),
                    (Page::Bullets, "Bullets"),
                    (Page::Targets, "Targets"),
                    (Page::Cards, "Cards"),
                ];
                let width = ui.available_width() / tabs.len() as f32;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (page, label) in tabs {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(width, 54.0), egui::Sense::click());
                        let colour = if self.page == page {
                            theme::BLAZE
                        } else {
                            theme::BONE_MUTED
                        };
                        tab_icon(
                            ui.painter(),
                            rect.center() - egui::vec2(0.0, 9.0),
                            page,
                            colour,
                        );
                        ui.painter().text(
                            rect.center() + egui::vec2(0.0, 15.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(12.0),
                            colour,
                        );
                        if response.clicked() {
                            self.page = page;
                        }
                    }
                });
            });
    }

    /// The phone menu: units, backups and what version this is.
    fn menu_sheet(&mut self, context: &egui::Context) {
        let Some(sheet) = self.menu else {
            return;
        };
        match sheet {
            MenuSheet::Main => {
                let shown = mobile::sheet(context, "menu", "Menu", |ui| {
                    let mut choice = None;
                    if mobile::list_row(ui, "Units", "Metric or imperial, per quantity", false)
                        .clicked()
                    {
                        choice = Some(MenuChoice::Units);
                    }
                    let row = mobile::list_row(
                        ui,
                        "Hunting rules",
                        self.country.map_or("None: no legal checks", Country::name),
                        false,
                    );
                    if let Some(country) = self.country {
                        flag_in_row(ui, &row, country);
                    }
                    if row.clicked() {
                        choice = Some(MenuChoice::Country);
                    }
                    let status = self.sync.status(ui.ctx().input(|input| input.time));
                    if mobile::list_row(ui, "Sync", &status, false).clicked() {
                        choice = Some(MenuChoice::Sync);
                    }
                    if mobile::list_row(ui, "Import backup…", "From this app or Strelok Pro", false)
                        .clicked()
                    {
                        choice = Some(MenuChoice::Import);
                    }
                    if mobile::list_row(
                        ui,
                        "Export backup",
                        "Your rifles, bullets, scopes and targets",
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Export);
                    }
                    if mobile::list_row(
                        ui,
                        "Load demo data",
                        "Three rifles with bullets, a scope and a target",
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Demo);
                    }
                    #[cfg(target_arch = "wasm32")]
                    if crate::web::install_state() != crate::web::Install::Installed
                        && mobile::list_row(
                            ui,
                            "Install on home screen",
                            "Opens like an app and works offline",
                            false,
                        )
                        .clicked()
                    {
                        choice = Some(MenuChoice::Install);
                    }
                    if mobile::list_row(
                        ui,
                        "What's new",
                        &format!("Version {} and earlier", crate::VERSION),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::WhatsNew);
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new(crate::APP_NAME).strong());
                    let version = if crate::COMMIT.is_empty() {
                        format!("Version {}", crate::VERSION)
                    } else {
                        format!("Version {} ({})", crate::VERSION, crate::COMMIT)
                    };
                    ui.weak(version);
                    choice
                });
                if shown.closed {
                    self.menu = None;
                }
                match shown.inner {
                    Some(MenuChoice::Units) => self.menu = Some(MenuSheet::Units),
                    Some(MenuChoice::Country) => self.menu = Some(MenuSheet::Country),
                    Some(MenuChoice::Sync) => self.menu = Some(MenuSheet::Sync),
                    Some(MenuChoice::WhatsNew) => self.menu = Some(MenuSheet::WhatsNew),
                    Some(MenuChoice::Import) => {
                        self.menu = None;
                        self.import(context);
                    }
                    Some(MenuChoice::Demo) => {
                        self.menu = None;
                        self.apply(Action::LoadDemo);
                    }
                    Some(MenuChoice::Export) => {
                        self.menu = None;
                        self.export();
                    }
                    Some(MenuChoice::Install) => {
                        #[cfg(target_arch = "wasm32")]
                        if crate::web::install_state() == crate::web::Install::Prompt {
                            crate::web::prompt_install();
                            self.menu = None;
                            return;
                        }
                        self.menu = Some(MenuSheet::Install);
                    }
                    None => {}
                }
            }
            MenuSheet::Install => {
                let shown = mobile::sheet(context, "install", "Install on home screen", |ui| {
                    ui.label(RichText::new("iPhone and iPad (Safari)").strong());
                    ui.label("Tap the Share button, then Add to Home Screen.");
                    ui.add_space(8.0);
                    ui.label(RichText::new("Android (Chrome)").strong());
                    ui.label("Open the menu (⋮), then Install app or Add to Home screen.");
                    ui.add_space(8.0);
                    ui.weak(
                        "The app then opens full screen from its icon, works without a \
                         connection and keeps your data on this device. Export a backup now \
                         and then.",
                    );
                });
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Sync => {
                let shown = mobile::sheet(context, "sync", "Sync", |ui| {
                    self.sync.panel(ui, &self.store)
                });
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Country => {
                let current = self.country;
                let shown = mobile::sheet(context, "country", "Hunting rules", |ui| {
                    let mut chosen = None;
                    for country in Country::ALL.map(Some).into_iter().chain([None]) {
                        let (title, detail) = country_choice(country);
                        let row = mobile::list_row(ui, title, detail, current == country);
                        if let Some(country) = country {
                            flag_in_row(ui, &row, country);
                        }
                        if row.clicked() {
                            chosen = Some(country);
                        }
                    }
                    chosen
                });
                if let Some(country) = shown.inner {
                    self.set_country(country);
                    self.menu = None;
                } else if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::WhatsNew => {
                let shown =
                    mobile::sheet(context, "whats-new", "What's new", crate::changelog::show);
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Units => {
                let shown = mobile::sheet(context, "units", "Units", |ui| {
                    units::menu(ui, &mut self.units)
                });
                if shown.inner {
                    self.units_changed(context);
                }
                if shown.closed {
                    self.menu = None;
                }
            }
        }
    }
}

/// The header's country menu button; room is left for the flag.
fn country_label(country: Option<Country>) -> String {
    match country {
        Some(country) => format!("        {}", country.name()),
        None => "Rules: none".to_owned(),
    }
}

/// A flag at the right of a phone list row, before its chevron.
fn flag_in_row(ui: &egui::Ui, row: &egui::Response, country: Country) {
    let height = 16.0;
    let left = row.rect.right() - 34.0 - crate::flags::width(country, height);
    crate::flags::paint(
        ui.painter(),
        egui::pos2(left, row.rect.center().y),
        country,
        height,
    );
}

/// A country choice's name and what it means.
fn country_choice(country: Option<Country>) -> (&'static str, &'static str) {
    match country {
        Some(country) => (country.name(), country.rules().law),
        None => ("None", "No legal checks, just the ethical guidelines"),
    }
}

/// The countries to choose from; returns the choice.
fn country_menu(ui: &mut egui::Ui, current: Option<Country>) -> Option<Option<Country>> {
    let mut chosen = None;
    ui.label(RichText::new("Check ammunition against the hunting rules of").small());
    for country in Country::ALL.map(Some).into_iter().chain([None]) {
        let (title, detail) = country_choice(country);
        let clicked = ui
            .horizontal(|ui| {
                match country {
                    Some(country) => {
                        crate::flags::show(ui, country, 11.0);
                    }
                    None => ui.add_space(crate::flags::width(Country::Sweden, 11.0)),
                }
                ui.selectable_label(current == country, title)
                    .on_hover_text(detail)
                    .clicked()
            })
            .inner;
        if clicked {
            chosen = Some(country);
            ui.close_menu();
        }
    }
    chosen
}

/// A tab's icon, drawn in a 24-point box around `center`.
fn tab_icon(painter: &egui::Painter, center: egui::Pos2, page: Page, colour: egui::Color32) {
    let stroke = egui::Stroke::new(1.8_f32, colour);
    let at = |x: f32, y: f32| center + egui::vec2(x - 12.0, y - 12.0);
    let line = |points: &[(f32, f32)]| {
        egui::Shape::line(points.iter().map(|&(x, y)| at(x, y)).collect(), stroke)
    };
    match page {
        Page::Calculate => {
            painter.circle_stroke(center, 8.0, stroke);
            for points in [
                [(12.0, 2.0), (12.0, 8.0)],
                [(12.0, 16.0), (12.0, 22.0)],
                [(2.0, 12.0), (8.0, 12.0)],
                [(16.0, 12.0), (22.0, 12.0)],
            ] {
                painter.add(line(&points));
            }
        }
        Page::Rifles => {
            painter.add(line(&[
                (2.0, 14.0),
                (15.0, 14.0),
                (18.0, 11.0),
                (22.0, 11.0),
            ]));
            painter.add(line(&[(5.0, 14.0), (4.0, 19.0), (8.0, 19.0), (9.0, 14.0)]));
            painter.add(line(&[(10.0, 11.0), (14.0, 11.0)]));
        }
        Page::Bullets => {
            painter.add(line(&[
                (9.0, 21.0),
                (9.0, 9.0),
                (10.5, 5.0),
                (12.0, 2.0),
                (13.5, 5.0),
                (15.0, 9.0),
                (15.0, 21.0),
                (9.0, 21.0),
            ]));
            painter.add(line(&[(9.0, 15.0), (15.0, 15.0)]));
        }
        Page::Targets => {
            painter.circle_stroke(center, 9.0, stroke);
            painter.circle_stroke(center, 5.0, stroke);
            painter.circle_filled(center, 1.5, colour);
        }
        Page::Cards => {
            // A card with a table's lines.
            painter.add(line(&[
                (4.0, 5.0),
                (20.0, 5.0),
                (20.0, 19.0),
                (4.0, 19.0),
                (4.0, 5.0),
            ]));
            for y in [9.5, 14.0] {
                painter.add(line(&[(7.0, y), (17.0, y)]));
            }
            painter.add(line(&[(10.0, 7.5), (10.0, 17.0)]));
        }
    }
}

/// The logo in the header: icon, MX wordmark and tagline, all vector.
/// Returns true when the version number was clicked (for What's new).
fn brand(ui: &mut egui::Ui) -> bool {
    let mut clicked = false;
    ui.add(egui::Image::new(theme::icon()).fit_to_exact_size(egui::vec2(36.0, 36.0)));
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(theme::wordmark()).fit_to_exact_size(egui::vec2(34.0, 19.0)));
            let version = ui
                .add(
                    egui::Label::new(
                        RichText::new(format!("v{}", crate::VERSION))
                            .size(10.0)
                            .color(theme::BONE_MUTED),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let hover = if crate::COMMIT.is_empty() {
                format!("Version {}. Click for what's new.", crate::VERSION)
            } else {
                format!(
                    "Version {}, built from commit {}. Click for what's new.",
                    crate::VERSION,
                    crate::COMMIT
                )
            };
            clicked = version.on_hover_text(hover).clicked();
        });
        ui.label(
            RichText::new("SHOOTING CALCULATOR")
                .size(9.5)
                .strong()
                .color(theme::BLAZE)
                .extra_letter_spacing(1.6),
        );
    });
    clicked
}

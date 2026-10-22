//! The application shell: navigation, the cached copy of the database, and
//! applying what the pages ask for. Pages never touch the store directly;
//! they return `Action`s, which keeps them easy to reuse on mobile.

use ballistics_store::{
    Bullet, Catalog, Country, PLAIN_CROSSHAIR, Reticle, Rifle, RifleLoad, Scope, Shot, Store,
    StoreError, Target, UserScope,
};
use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};

use crate::bullets::BulletsPage;
use crate::calculator::{CalculatorPage, Inputs};
use crate::cards::{CardSettings, CardsPage};
use crate::i18n::{self, Language, tr, trf};
use crate::import::{self, Decision, ImportPreview};
use crate::mobile;
use crate::rifles::RiflesPage;
use crate::shot_log::ShotLogPage;
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
/// The language the app is shown in, or `null` for the device's.
const LANGUAGE_SETTING: &str = "language";
/// What the app is used for: hunting or target shooting. Absent until asked.
const PURPOSE_SETTING: &str = "purpose";
/// The version the hunter last used, to mention an update once.
const SEEN_VERSION: &str = "seen_version";
const UNIT_SETTINGS: &str = "units";

/// What the app is used for. Target shooting leaves out everything about
/// hunting: ethical limits, legal ammunition, seasons and shooting light.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Hunting,
    Shooting,
}

impl Purpose {
    pub fn name(self) -> &'static str {
        match self {
            Purpose::Hunting => tr("Hunting"),
            Purpose::Shooting => tr("Target shooting"),
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Purpose::Hunting => tr("Ethical range, legal ammo, seasons, light"),
            Purpose::Shooting => tr("Just the ballistics: holds, tables, cards"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Calculate,
    Rifles,
    Bullets,
    Targets,
    Cards,
    /// Reached from the menu and from Calculate, not from a tab.
    ShotLog,
}

pub enum Action {
    Goto(Page),
    ShowBulletsFor(String),
    /// Open a bullet's setup in a rifle (rifle id, bullet id).
    EditLoad(String, String),
    SaveRifle(Rifle),
    /// Save a rifle whose range table changed, without a message.
    SaveTable(Rifle),
    DeleteRifle(String),
    SaveBullet(Bullet),
    DeleteBullet(String),
    SaveLoad(RifleLoad),
    DeleteLoad(String),
    SaveScope(UserScope),
    DeleteScope(String),
    SaveTarget(Target),
    DeleteTarget(String),
    /// Save a shot from Calculate in the log and open it there.
    LogShot(Box<Shot>),
    SaveShot(Box<Shot>),
    DeleteShot(String),
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
    /// The shot log, newest first.
    pub shots: Vec<Shot>,
    /// Whose hunting rules to check, if any (none when target shooting).
    pub country: Option<Country>,
    /// Hunting: ethical limits and the rules apply. Off for target shooting.
    pub hunting: bool,
    /// The hunter's county or region (chosen in Calculate), for seasons.
    pub region: Option<String>,
    /// The weather and shot set on Calculate: a truing shot's weather.
    pub today: ballistics_shot::Conditions,
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
    Language,
    Country,
    Purpose,
    Sync,
    WhatsNew,
    /// How to put the app on the home screen by hand.
    Install,
}

/// What a tap in the phone menu asked for, done once the sheet is drawn.
enum MenuChoice {
    Units,
    Language,
    Country,
    Purpose,
    Sync,
    ShotLog,
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

/// How long a message that isn't an error stays, seconds.
const STATUS_SECONDS: f64 = 10.0;

pub struct App {
    store: Store,
    data: Data,
    page: Page,
    calculator: CalculatorPage,
    rifles: RiflesPage,
    bullets: BulletsPage,
    targets: TargetsPage,
    shot_log: ShotLogPage,
    cards: CardsPage,
    saved_cards: CardSettings,
    saved_inputs: Inputs,
    units: UnitPrefs,
    /// The language chosen in the menu; `None` follows the device.
    language: Option<Language>,
    status: Option<Status>,
    /// An import being previewed.
    import: Option<ImportPreview>,
    /// The phone menu, when open.
    menu: Option<MenuSheet>,
    sync: Sync,
    country: Option<Country>,
    /// Hunting or target shooting; `None` until the first-run question is
    /// answered.
    purpose: Option<Purpose>,
    /// The first-run question has moved on to the hunter's country.
    asking_country: bool,
    /// The message shown and since when (seconds, egui time).
    status_seen: Option<(String, f64)>,
    /// The desktop's What's new window is open.
    whats_new_open: bool,
    /// The desktop's About window is open.
    about_open: bool,
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
        let purpose = store.setting::<Purpose>(PURPOSE_SETTING).ok().flatten();
        let language = store
            .setting::<Option<Language>>(LANGUAGE_SETTING)
            .ok()
            .flatten()
            .flatten();
        i18n::set(Language::chosen(language));
        let mut app = Self {
            sync,
            country,
            purpose,
            asking_country: false,
            whats_new_open: false,
            about_open: false,
            status_seen: None,
            data_changed: false,
            store,
            data: Data::default(),
            page: Page::Calculate,
            calculator: CalculatorPage::new(inputs.clone()),
            rifles: RiflesPage::default(),
            bullets: BulletsPage::default(),
            targets: TargetsPage::default(),
            shot_log: ShotLogPage::default(),
            cards: CardsPage::new(cards.clone()),
            saved_cards: cards,
            saved_inputs: inputs,
            units,
            language,
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
                text: trf!(
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
                shots: self.store.shots()?,
                country: self.rules_country(),
                hunting: self.hunting(),
                region: None,
                today: self.calculator.inputs.conditions(),
            })
        })();
        match loaded {
            Ok(data) => self.data = data,
            Err(error) => self.report(Err(error)),
        }
    }

    fn hunting(&self) -> bool {
        self.purpose != Some(Purpose::Shooting)
    }

    /// The country whose rules apply: none when target shooting.
    fn rules_country(&self) -> Option<Country> {
        self.country.filter(|_| self.hunting())
    }

    fn set_purpose(&mut self, purpose: Purpose) {
        self.purpose = Some(purpose);
        self.data.hunting = self.hunting();
        self.data.country = self.rules_country();
        if let Err(error) = self.store.set_setting(PURPOSE_SETTING, &purpose) {
            self.report(Err(error));
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
            Action::EditLoad(rifle_id, bullet_id) => {
                if let Some(rifle) = self.data.rifles.iter().find(|rifle| rifle.id == rifle_id) {
                    self.rifles.edit_load(rifle, &bullet_id);
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
                trf!("Saved {}", rifle.name)
            }),
            Action::SaveTable(mut rifle) => {
                let result = self.store.save_rifle(&mut rifle);
                match result {
                    Ok(()) => {
                        self.rifles.table_saved(&rifle);
                        self.data_changed = true;
                    }
                    Err(error) => self.report(Err(error)),
                }
                self.reload();
                return;
            }
            Action::DeleteRifle(id) => self.store.delete_rifle(&id).map(|()| {
                self.rifles.deleted(&id);
                tr("Rifle deleted").to_owned()
            }),
            Action::SaveBullet(mut bullet) => self.store.save_bullet(&mut bullet).map(|()| {
                self.bullets.saved(&bullet);
                trf!("Saved {}", bullet.display_name())
            }),
            Action::DeleteBullet(id) => self.store.delete_bullet(&id).map(|()| {
                self.bullets.deleted(&id);
                tr("Bullet deleted").to_owned()
            }),
            Action::SaveLoad(mut load) => self
                .store
                .save_rifle_load(&mut load)
                .map(|()| tr("Saved this bullet's data for the rifle").to_owned()),
            Action::SaveScope(mut scope) => self.store.save_scope(&mut scope).map(|()| {
                self.rifles.scope_saved(&scope);
                trf!("Saved {}", scope.display_name())
            }),
            Action::DeleteScope(id) => self.store.delete_scope(&id).map(|()| {
                self.rifles.scope_deleted(&id);
                tr("Scope deleted; rifles that had it now have none").to_owned()
            }),
            Action::SaveTarget(mut target) => self.store.save_target(&mut target).map(|()| {
                self.targets.saved(&target);
                trf!("Saved {}", target.name)
            }),
            Action::DeleteTarget(id) => self.store.delete_target(&id).map(|()| {
                self.targets.deleted(&id);
                tr("Target deleted").to_owned()
            }),
            Action::LogShot(mut shot) => {
                if let Some(bullet) = self.data.bullets.iter().find(|b| b.id == shot.bullet_id) {
                    shot.bullet_name = bullet.display_name();
                }
                self.store.save_shot(&mut shot).map(|()| {
                    self.shot_log.open(&shot);
                    self.page = Page::ShotLog;
                    "Shot logged: add what came of it, and where the animal stood".to_owned()
                })
            }
            Action::SaveShot(mut shot) => self.store.save_shot(&mut shot).map(|()| {
                self.shot_log.saved(&shot);
                "Saved the shot".to_owned()
            }),
            Action::DeleteShot(id) => self.store.delete_shot(&id).map(|()| {
                self.shot_log.deleted(&id);
                "Shot deleted".to_owned()
            }),
            Action::LoadDemo => self.store.load_demo().map(|report| {
                self.page = Page::Calculate;
                trf!("Demo data: {}", import::summary(&report))
            }),
            Action::DeleteLoad(id) => self.store.delete_rifle_load(&id).map(|()| {
                self.rifles.forget_load_draft();
                tr("Using the bullet's own data again").to_owned()
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
                .map(|()| trf!("Exported to {}", path.display()))
                .map_err(|error| {
                    StoreError::Invalid(trf!("could not write {}: {}", path.display(), error))
                });
            self.report(result);
        }
        #[cfg(target_arch = "wasm32")]
        self.report(
            crate::web::download("mxsc-backup.json", &json)
                .map(|()| tr("Exported: saved as mxsc-backup.json in your downloads").to_owned())
                .map_err(StoreError::Invalid),
        );
    }

    /// Opens a backup for the import window: this app's JSON export or a
    /// Strelok Pro backup.
    #[cfg(not(target_arch = "wasm32"))]
    fn import(&mut self, _context: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr("Backups"), &["json", "srl"])
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
                self.report(Ok(trf!("Imported: {}", import::summary(&report))));
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
        // Messages go away by themselves after a while; errors stay.
        let now = context.input(|input| input.time);
        match &self.status {
            Some(status) if !status.error => {
                if self
                    .status_seen
                    .as_ref()
                    .is_none_or(|(text, _)| *text != status.text)
                {
                    self.status_seen = Some((status.text.clone(), now));
                }
                let since = self.status_seen.as_ref().map_or(now, |(_, since)| *since);
                if now - since > STATUS_SECONDS {
                    self.status = None;
                } else {
                    context.request_repaint_after(std::time::Duration::from_secs(1));
                }
            }
            _ => self.status_seen = None,
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

        self.data.region = self.calculator.inputs.region.clone();
        self.data.today = self.calculator.inputs.conditions();
        let actions = match self.page {
            Page::Calculate => self.calculator.show(context, &self.data),
            Page::Rifles => self.rifles.show(context, &self.data),
            Page::Bullets => self.bullets.show(context, &self.data),
            Page::Targets => self.targets.show(context, &self.data),
            Page::ShotLog => {
                let place = self.calculator.inputs.place.as_ref();
                self.shot_log.show(context, &self.data, place)
            }
            Page::Cards => {
                let subject = self.calculator.card_subject(&self.data);
                self.cards.show(context, subject, self.data.hunting)
            }
        };
        for action in actions {
            self.apply(action);
        }
        if self.page != Page::ShotLog {
            self.shot_log.leave();
        }
        // The phone menu's sheets; on wider screens the desktop menu opens
        // the ones it has no room for.
        self.menu_sheet(context);
        #[cfg(target_arch = "wasm32")]
        self.collect_picked_file();
        self.import_window(context);
        self.welcome(context);
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
                self.report(Ok(if changes == 1 {
                    tr("Synced: 1 change from your other devices").to_owned()
                } else {
                    trf!("Synced: {} changes from your other devices", changes)
                }));
            }
            Some(sync::Event::Stopped(message)) => self.report(Ok(message)),
            None => {}
        }
        let mut open = self.whats_new_open;
        egui::Window::new(tr("What's new"))
            .id(egui::Id::new("whats-new"))
            .open(&mut open)
            .collapsible(false)
            .default_size([520.0, 560.0])
            .show(context, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, crate::changelog::show);
            });
        self.whats_new_open = open;
        let mut open = self.about_open;
        egui::Window::new(tr("About"))
            .id(egui::Id::new("about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(context, |ui| {
                ui.label(RichText::new(crate::APP_NAME).strong().size(16.0));
                ui.label(if crate::COMMIT.is_empty() {
                    trf!("Version {}", crate::VERSION)
                } else {
                    trf!("Version {} ({})", crate::VERSION, crate::COMMIT)
                });
                ui.add_space(4.0);
                ui.label(tr("Free software under the MIT or Apache-2.0 licence."));
                ui.weak(tr(
                    "Your rifles, bullets, targets and shot log stay on this device, and on \
                     your other devices if you sync them.",
                ));
            });
        self.about_open = open;
        let mut open = self.sync.window_open;
        egui::Window::new(tr("Sync"))
            .id(egui::Id::new("sync"))
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
                        (Page::Calculate, tr("Calculate")),
                        (Page::Rifles, tr("Rifles")),
                        (Page::Bullets, tr("Bullets")),
                        (Page::Targets, tr("Targets")),
                        (Page::Cards, tr("Cards")),
                    ] {
                        ui.selectable_value(&mut self.page, page, RichText::new(label).size(15.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.desktop_menu(ui, context);
                        let now = context.input(|input| input.time);
                        // The font has no dot, so paint one in a slot of its own
                        // before the status.
                        let galley = ui.painter().layout_no_wrap(
                            self.sync.short_status().to_owned(),
                            egui::FontId::proportional(12.0),
                            egui::Color32::PLACEHOLDER,
                        );
                        let dot = 14.0;
                        let (rect, sync) = ui.allocate_exact_size(
                            egui::vec2(dot + galley.size().x, galley.size().y.max(20.0)),
                            egui::Sense::click(),
                        );
                        let colour = if sync.hovered() {
                            theme::BONE
                        } else {
                            theme::BONE_MUTED
                        };
                        ui.painter().circle_filled(
                            rect.left_center() + egui::vec2(4.0, 0.0),
                            4.0,
                            self.sync.colour(),
                        );
                        ui.painter().galley(
                            egui::pos2(rect.left() + dot, rect.center().y - galley.size().y / 2.0),
                            galley,
                            colour,
                        );
                        let sync = sync
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(self.sync.status(now));
                        if sync.clicked() {
                            self.sync.window_open = !self.sync.window_open;
                        }
                        if let Some(country) = self.rules_country() {
                            let height = 13.0;
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(crate::flags::width(country, height), height),
                                egui::Sense::hover(),
                            );
                            crate::flags::paint(ui.painter(), rect.left_center(), country, height);
                            response.on_hover_text(trf!(
                                "{} hunting rules: legal ammunition, seasons and shooting light",
                                i18n::country_adjective(country)
                            ));
                        }
                    });
                });
            });
    }

    /// The desktop's one menu: settings, backups and about.
    fn desktop_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let mut sync = false;
        let mut import = false;
        let mut export = false;
        let mut demo = false;
        ui.menu_button(RichText::new("•••").size(14.0), |ui| {
            ui.set_min_width(230.0);
            if ui
                .button("Shot log")
                .on_hover_text("The shots you logged from Calculate")
                .clicked()
            {
                self.page = Page::ShotLog;
                ui.close_menu();
            }
            ui.separator();
            // Choices open a sheet: submenus would have no room to open at
            // the window's right edge and would cover this menu instead.
            let purpose = self.purpose.unwrap_or(Purpose::Hunting).name();
            let language = self.language.map_or(tr("System"), Language::own_name);
            let mut rows = vec![
                (tr("Units…").to_owned(), MenuSheet::Units),
                (trf!("Language: {}…", language), MenuSheet::Language),
                (trf!("Use for: {}…", purpose), MenuSheet::Purpose),
            ];
            if self.hunting() {
                let country = self.country.map_or(tr("none"), i18n::country_name);
                rows.push((trf!("Hunting rules: {}…", country), MenuSheet::Country));
            }
            for (label, sheet) in rows {
                if ui.button(label).clicked() {
                    self.menu = Some(sheet);
                    ui.close_menu();
                }
            }
            let now = ui.ctx().input(|input| input.time);
            if ui
                .button(tr("Sync…"))
                .on_hover_text(self.sync.status(now))
                .clicked()
            {
                sync = true;
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button(tr("Import backup…"))
                .on_hover_text(tr(
                    "From this app or Strelok Pro; merged into this device's data",
                ))
                .clicked()
            {
                import = true;
                ui.close_menu();
            }
            if ui
                .button(tr("Export backup"))
                .on_hover_text(tr("Your rifles, bullets, scopes, targets and shot log, as JSON"))
                .clicked()
            {
                export = true;
                ui.close_menu();
            }
            if ui
                .button(tr("Load demo data"))
                .on_hover_text(tr("Three rifles with bullets, a scope and a target"))
                .clicked()
            {
                demo = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button(tr("What's new")).clicked() {
                self.whats_new_open = true;
                ui.close_menu();
            }
            if ui.button(tr("About")).clicked() {
                self.about_open = true;
                ui.close_menu();
            }
        })
        .response
        .on_hover_text(tr("Menu"));
        if sync {
            self.sync.window_open = true;
        }
        if import {
            self.import(context);
        }
        if export {
            self.export();
        }
        if demo {
            self.apply(Action::LoadDemo);
        }
    }

    /// Asks once what the app is for, and a hunter where they hunt.
    fn welcome(&mut self, context: &egui::Context) {
        if self.purpose.is_some() && !self.asking_country {
            return;
        }
        let mut chosen_purpose = None;
        let mut chosen_country = None;
        let width = (context.screen_rect().width() - 48.0).min(440.0);
        let modal = egui::Modal::new(egui::Id::new("welcome")).show(context, |ui| {
            ui.set_width(width);
            if self.asking_country {
                ui.heading(tr("Where do you hunt?"));
                ui.label(tr(
                    "For the legal ammunition, the seasons and the shooting light. You can \
                     change it later in the menu.",
                ));
                ui.add_space(8.0);
                for country in Country::ALL.map(Some).into_iter().chain([None]) {
                    let (title, detail) = match country {
                        Some(country) => (i18n::country_name(country), country.rules().law),
                        None => (
                            tr("Elsewhere"),
                            tr("No legal checks, just the ethical limits"),
                        ),
                    };
                    let row = mobile::list_row(ui, title, detail, self.country == country);
                    if let Some(country) = country {
                        flag_in_row(ui, &row, country);
                    }
                    if row.clicked() {
                        chosen_country = Some(country);
                    }
                }
            } else {
                ui.heading(trf!("Welcome to {}", crate::APP_NAME));
                ui.label(tr(
                    "What will you use it for? You can change it later in the menu.",
                ));
                ui.add_space(8.0);
                for purpose in [Purpose::Hunting, Purpose::Shooting] {
                    if mobile::list_row(ui, purpose.name(), purpose.detail(), false).clicked() {
                        chosen_purpose = Some(purpose);
                    }
                }
            }
        });
        if let Some(purpose) = chosen_purpose {
            self.set_purpose(purpose);
            self.asking_country = purpose == Purpose::Hunting;
        } else if let Some(country) = chosen_country {
            self.set_country(country);
            self.asking_country = false;
        } else if modal.should_close() {
            // Closed without an answer: hunting, with the rules as they were.
            if self.purpose.is_none() {
                self.set_purpose(Purpose::Hunting);
            }
            self.asking_country = false;
        }
    }

    fn set_country(&mut self, country: Option<Country>) {
        self.country = country;
        self.data.country = self.rules_country();
        if let Err(error) = self.store.set_setting(COUNTRY_SETTING, &country) {
            self.report(Err(error));
        }
    }

    /// Shows the app in `language`, or in the device's for `None`.
    fn set_language(&mut self, language: Option<Language>) {
        self.language = language;
        i18n::set(Language::chosen(language));
        if let Err(error) = self.store.set_setting(LANGUAGE_SETTING, &language) {
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
                        if menu.on_hover_text(tr("Menu")).clicked() {
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
                    (Page::Calculate, tr("Calculate")),
                    (Page::Rifles, tr("Rifles")),
                    (Page::Bullets, tr("Bullets")),
                    (Page::Targets, tr("Targets")),
                    (Page::Cards, tr("Cards")),
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
                let shots = self.data.shots.len();
                let shown = mobile::sheet(context, "menu", tr("Menu"), |ui| {
                    let mut choice = None;
                    let logged = match shots {
                        0 => tr("Log shots from Calculate").to_owned(),
                        1 => tr("1 shot").to_owned(),
                        count => trf!("{} shots", count),
                    };
                    if mobile::list_row(ui, tr("Shot log"), &logged, false).clicked() {
                        choice = Some(MenuChoice::ShotLog);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Units"),
                        tr("Metric or imperial, per quantity"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Units);
                    }
                    let language = self.language.map_or(tr("System"), Language::own_name);
                    if mobile::list_row(ui, tr("Language"), language, false).clicked() {
                        choice = Some(MenuChoice::Language);
                    }
                    let purpose = self.purpose.unwrap_or(Purpose::Hunting);
                    if mobile::list_row(ui, tr("Use for"), purpose.name(), false).clicked() {
                        choice = Some(MenuChoice::Purpose);
                    }
                    if self.hunting() {
                        let row = mobile::list_row(
                            ui,
                            tr("Hunting rules"),
                            self.country
                                .map_or(tr("None: no legal checks"), i18n::country_name),
                            false,
                        );
                        if let Some(country) = self.country {
                            flag_in_row(ui, &row, country);
                        }
                        if row.clicked() {
                            choice = Some(MenuChoice::Country);
                        }
                    }
                    let status = self.sync.status(ui.ctx().input(|input| input.time));
                    if mobile::list_row(ui, tr("Sync"), &status, false).clicked() {
                        choice = Some(MenuChoice::Sync);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Import backup…"),
                        tr("From this app or Strelok Pro"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Import);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Export backup"),
                        tr("Your rifles, bullets, scopes, targets and shots"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Export);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Load demo data"),
                        tr("Three rifles with bullets, a scope and a target"),
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
                            tr("Install on home screen"),
                            tr("Opens like an app and works offline"),
                            false,
                        )
                        .clicked()
                    {
                        choice = Some(MenuChoice::Install);
                    }
                    if mobile::list_row(
                        ui,
                        tr("What's new"),
                        &trf!("Version {} and earlier", crate::VERSION),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::WhatsNew);
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new(crate::APP_NAME).strong());
                    let version = if crate::COMMIT.is_empty() {
                        trf!("Version {}", crate::VERSION)
                    } else {
                        trf!("Version {} ({})", crate::VERSION, crate::COMMIT)
                    };
                    ui.weak(version);
                    choice
                });
                if shown.closed {
                    self.menu = None;
                }
                match shown.inner {
                    Some(MenuChoice::Units) => self.menu = Some(MenuSheet::Units),
                    Some(MenuChoice::Language) => self.menu = Some(MenuSheet::Language),
                    Some(MenuChoice::Country) => self.menu = Some(MenuSheet::Country),
                    Some(MenuChoice::Purpose) => self.menu = Some(MenuSheet::Purpose),
                    Some(MenuChoice::Sync) => self.menu = Some(MenuSheet::Sync),
                    Some(MenuChoice::WhatsNew) => self.menu = Some(MenuSheet::WhatsNew),
                    Some(MenuChoice::ShotLog) => {
                        self.menu = None;
                        self.page = Page::ShotLog;
                    }
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
                let shown = mobile::sheet(context, "install", tr("Install on home screen"), |ui| {
                    ui.label(RichText::new(tr("iPhone and iPad (Safari)")).strong());
                    ui.label(tr("Tap the Share button, then Add to Home Screen."));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Android (Chrome)").strong());
                    ui.label(tr(
                        "Open the menu (⋮), then Install app or Add to Home screen.",
                    ));
                    ui.add_space(8.0);
                    ui.weak(tr(
                        "The app then opens full screen from its icon, works without a \
                         connection and keeps your data on this device. Export a backup now \
                         and then.",
                    ));
                });
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Sync => {
                let shown = mobile::sheet(context, "sync", tr("Sync"), |ui| {
                    self.sync.panel(ui, &self.store)
                });
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Country => {
                let current = self.country;
                let shown = mobile::sheet(context, "country", tr("Hunting rules"), |ui| {
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
            MenuSheet::Purpose => {
                let current = self.purpose.unwrap_or(Purpose::Hunting);
                let shown = mobile::sheet(context, "purpose", tr("Use for"), |ui| {
                    let mut chosen = None;
                    for purpose in [Purpose::Hunting, Purpose::Shooting] {
                        if mobile::list_row(
                            ui,
                            purpose.name(),
                            purpose.detail(),
                            current == purpose,
                        )
                        .clicked()
                        {
                            chosen = Some(purpose);
                        }
                    }
                    chosen
                });
                if let Some(purpose) = shown.inner {
                    self.set_purpose(purpose);
                    self.menu = None;
                } else if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::WhatsNew => {
                let shown = mobile::sheet(
                    context,
                    "whats-new",
                    tr("What's new"),
                    crate::changelog::show,
                );
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Units => {
                let shown = mobile::sheet(context, "units", tr("Units"), |ui| {
                    units::menu(ui, &mut self.units)
                });
                if shown.inner {
                    self.units_changed(context);
                }
                if shown.closed {
                    self.menu = None;
                }
            }
            MenuSheet::Language => {
                let current = self.language;
                let shown = mobile::sheet(context, "language", tr("Language"), |ui| {
                    let mut chosen = None;
                    let system = Language::system().own_name();
                    if mobile::list_row(ui, tr("System"), system, current.is_none()).clicked() {
                        chosen = Some(None);
                    }
                    for language in Language::ALL {
                        let row = mobile::list_row(
                            ui,
                            language.own_name(),
                            "",
                            current == Some(language),
                        );
                        if row.clicked() {
                            chosen = Some(Some(language));
                        }
                    }
                    chosen
                });
                if let Some(language) = shown.inner {
                    self.set_language(language);
                    self.menu = None;
                } else if shown.closed {
                    self.menu = None;
                }
            }
        }
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
        Some(country) => (i18n::country_name(country), country.rules().law),
        None => (
            tr("None"),
            tr("No legal checks, just the ethical guidelines"),
        ),
    }
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
        // Not a tab.
        Page::ShotLog => {}
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
                trf!("Version {}. Click for what's new.", crate::VERSION)
            } else {
                trf!(
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

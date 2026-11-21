//! The application shell: navigation, the cached copy of the database, and
//! applying what the pages ask for. Pages never touch the store directly;
//! they return `Action`s, which keeps them easy to reuse on mobile.

use std::collections::HashMap;
use std::rc::Rc;

use ballistics_store::{
    Bullet, Catalog, Country, Group, GroupView, GroupsData, PLAIN_CROSSHAIR, Pending, Photo,
    PhotoStore, Reticle, Rifle, RifleLoad, RoundLogEntry, Scope, Shot, Store, StoreError, Target,
    UserScope,
};
use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};

use crate::bullets::BulletsPage;
use crate::calculator::{CalculatorPage, Inputs};
use crate::cards::{CardSettings, CardsPage};
use crate::groups::rounds::RoundSync;
use crate::groups::{GroupSave, GroupSettings, GroupsPage};
use crate::i18n::{self, Language, tr, trf};
use crate::import::{self, Decision, ImportPreview};
use crate::mobile;
use crate::rifles::RiflesPage;
use crate::sessions::SessionsPage;
use crate::shot_log::ShotLogPage;
use crate::sync::{self, Sync};
use crate::targets::TargetsPage;
use crate::theme::{self, ThemeChoice};
use crate::units::{self, UnitPrefs};

const CALCULATOR_SETTINGS: &str = "calculator";
/// The card choices and the hunter's stands.
const CARD_SETTINGS: &str = "cards";
/// Whose hunting rules to check: a country, or `null` for none. Absent
/// until the hunter chooses; then the device's language decides.
const COUNTRY_SETTING: &str = "country";
/// The language the app is shown in, or `null` for the device's.
pub(crate) const LANGUAGE_SETTING: &str = "language";
/// What the app is used for: hunting or target shooting. Absent until asked.
const PURPOSE_SETTING: &str = "purpose";
/// The version the hunter last used, to mention an update once.
const SEEN_VERSION: &str = "seen_version";
/// Dark, Sun or the device's theme.
const THEME_SETTING: &str = "theme";
/// After how many rounds each rifle is due a clean, by rifle id.
const CLEAN_AFTER_SETTING: &str = "clean_after";
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
    Groups,
    Rifles,
    Bullets,
    Targets,
    /// Reached from the menu on a phone, from a tab on a computer.
    Cards,
    /// Reached from the menu and from Calculate, not from a tab.
    ShotLog,
    /// Range sessions, reached from the menu.
    Sessions,
}

pub enum Action {
    Goto(Page),
    /// Back one view, as the ‹ links and the browser's Back do.
    Back,
    ShowBulletsFor(String),
    /// Open a bullet's setup in a rifle (rifle id, bullet id).
    EditLoad(String, String),
    SaveRifle(Rifle),
    /// Save a rifle whose range table changed, without a message.
    SaveTable(Rifle),
    DeleteRifle(String),
    SaveBullet(Box<Bullet>),
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
    /// Load the built-in demo rifles, bullets, scope, target and groups.
    LoadDemo,
    /// Keep a photo just brought in: its bytes and thumbnail in the photo
    /// store. Its record is saved with the first group on it; until then
    /// the bytes go when the app next starts.
    KeepPhoto(Box<groups_vision::ImportedPhoto>),
    /// Save a group, then what goes with it: its session, its photo's
    /// record, its round log entry and its framing.
    SaveGroup(Box<GroupSave>),
    /// Delete a group with its round log entry, offering Undo.
    DeleteGroup(String),
    /// A change to a rifle's history, a session or a load's velocities.
    Record(Box<crate::records::Change>),
    /// Clean a rifle after this many rounds (rifle id), kept on this device.
    SetCleanAfter(String, u32),
    /// Open a group's page in Groups.
    OpenGroup(String),
    /// Save a file the app made on the device.
    SaveFile(Box<crate::files::FileOut>),
    /// Say that something couldn't be done.
    Problem(String),
    /// Delete several groups at once; Undo puts them all back.
    DeleteGroups(Vec<String>),
    /// Save a comparison or composite of groups, to open again.
    SaveAnalysis(ballistics_store::Analysis),
    DeleteAnalysis(String),
    SaveLadder(ballistics_store::Ladder),
    /// Make a ladder's run a load: the handload first, then its data in the
    /// rifle and the ladder naming it, only once the handload is saved.
    PromoteLadder(Box<crate::groups::analyse::pool::Promotion>),
    /// Mark that a rifle's zero was set again: zero checks start from it.
    SaveZeroReset(ballistics_store::ZeroReset),
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
    /// Groups, sessions, photos, barrels and the round log.
    pub groups: GroupsData,
    /// How each group was last framed while marking, on this device.
    pub group_views: HashMap<String, GroupView>,
    /// After how many rounds a rifle is due a clean, by rifle id, where the
    /// hunter changed it; kept on this device.
    pub clean_after: HashMap<String, u32>,
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
    Theme,
    Country,
    Purpose,
    Sync,
    /// The rifles on the Garmin watch app.
    Watch,
    WhatsNew,
    /// How to put the app on the home screen by hand.
    Install,
    /// Groups or their shots as CSV.
    GroupsCsv,
    #[cfg(not(target_arch = "wasm32"))]
    Updates,
}

/// What a tap in the phone menu asked for, done once the sheet is drawn.
enum MenuChoice {
    Cards,
    Sessions,
    ExportWithPhotos,
    GroupsCsv,
    Units,
    Language,
    Theme,
    Country,
    Purpose,
    Sync,
    Watch,
    ShotLog,
    WhatsNew,
    Import,
    Export,
    Demo,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Install,
    #[cfg(not(target_arch = "wasm32"))]
    Updates,
}

/// A photo or thumbnail being written to the photo store.
enum PhotoWrite {
    Photo(Pending<String>),
    Thumbnail(Pending<()>),
}

impl PhotoWrite {
    /// How the write went, once the store answers.
    fn take(&self) -> Option<Result<(), ballistics_store::PhotoError>> {
        match self {
            PhotoWrite::Photo(pending) => pending.take().map(|answer| answer.map(|_| ())),
            PhotoWrite::Thumbnail(pending) => pending.take(),
        }
    }
}

struct Status {
    text: String,
    error: bool,
    /// A deleted group Undo puts back.
    undo: Option<Box<DeletedGroup>>,
}

/// A deleted group, with what went with it.
struct DeletedGroup {
    group: Group,
    rounds: Option<RoundLogEntry>,
    view: Option<GroupView>,
    /// Its photo's record, when no other group was marked on it.
    photo: Option<Photo>,
    /// Deleted with it, from a selection.
    with: Vec<DeletedGroup>,
}

/// How long a message that isn't an error stays, seconds.
const STATUS_SECONDS: f64 = 10.0;
/// How long Undo is offered after a group is deleted, seconds.
const UNDO_SECONDS: f64 = 10.0;
/// The phone's Undo button in the status bar.
const UNDO_BUTTON_WIDTH: f32 = 72.0;

pub struct App {
    store: Store,
    data: Data,
    page: Page,
    calculator: CalculatorPage,
    rifles: RiflesPage,
    bullets: BulletsPage,
    targets: TargetsPage,
    shot_log: ShotLogPage,
    groups: GroupsPage,
    sessions: SessionsPage,
    saved_group_settings: GroupSettings,
    /// Where photos are kept; made on the first frame (the browser's needs
    /// the context).
    photos: Option<Rc<dyn PhotoStore>>,
    /// Photo writes the store hasn't answered yet.
    photo_writes: Vec<PhotoWrite>,
    /// A backup with photos waiting for the photos to be read.
    photo_export: Option<crate::backup::PhotoExport>,
    /// Photos from an imported backup, put back one a frame.
    photo_restore: crate::backup::PhotoRestore,
    cards: CardsPage,
    saved_cards: CardSettings,
    saved_inputs: Inputs,
    units: UnitPrefs,
    /// The language chosen in the menu; `None` follows the device.
    language: Option<Language>,
    theme: ThemeChoice,
    /// The theme last handed to egui.
    theme_shown: Option<ThemeChoice>,
    /// The theme the web page around the app was last told about.
    #[cfg(target_arch = "wasm32")]
    page_theme: Option<(ThemeChoice, egui::Theme)>,
    status: Option<Status>,
    /// An import being previewed.
    import: Option<ImportPreview>,
    /// The phone menu, when open.
    menu: Option<MenuSheet>,
    sync: Sync,
    watch: crate::watch::WatchSync,
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
    /// When Back was asked for with unsaved changes: the dialog is open.
    discard_asked: Option<f64>,
    /// This frame's time (egui's clock).
    now: f64,
    /// The hunter's data changed this frame: sync it.
    data_changed: bool,
    /// A backup picked in the browser, waiting to be previewed.
    #[cfg(target_arch = "wasm32")]
    picked: crate::web::PickedSlot,
    #[cfg(not(target_arch = "wasm32"))]
    updates: crate::updates::Updater,
    /// Look for a new version when the app starts: off until the hunter
    /// turns it on.
    #[cfg(not(target_arch = "wasm32"))]
    check_updates: bool,
    /// The check at start was made (or wasn't wanted).
    #[cfg(not(target_arch = "wasm32"))]
    updates_started: bool,
}

impl App {
    /// A message that stays until the hunter closes it.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn keep_message(&mut self, text: String) {
        self.status = Some(Status {
            text,
            error: true,
            undo: None,
        });
    }

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
        let group_settings: GroupSettings = store
            .setting(crate::groups::SETTINGS)
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
        let theme = store
            .setting(THEME_SETTING)
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut app = Self {
            sync,
            watch: crate::watch::WatchSync::load(&store),
            country,
            purpose,
            asking_country: false,
            whats_new_open: false,
            about_open: false,
            discard_asked: None,
            now: 0.0,
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
            groups: GroupsPage::new(group_settings.clone()),
            sessions: SessionsPage::default(),
            saved_group_settings: group_settings,
            photos: None,
            photo_writes: Vec::new(),
            photo_export: None,
            photo_restore: Default::default(),
            cards: CardsPage::new(cards.clone()),
            saved_cards: cards,
            saved_inputs: inputs,
            units,
            language,
            theme,
            theme_shown: None,
            #[cfg(target_arch = "wasm32")]
            page_theme: None,
            status: None,
            import: None,
            menu: None,
            #[cfg(target_arch = "wasm32")]
            picked: Default::default(),
            #[cfg(not(target_arch = "wasm32"))]
            updates: Default::default(),
            #[cfg(not(target_arch = "wasm32"))]
            check_updates: false,
            #[cfg(not(target_arch = "wasm32"))]
            updates_started: false,
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.check_updates = app
                .store
                .setting(crate::updates::SETTING)
                .ok()
                .flatten()
                .unwrap_or(false);
        }
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
                undo: None,
            });
        }
        let _ = self.store.set_setting(SEEN_VERSION, &crate::VERSION);
    }

    fn reload(&mut self) {
        let loaded = (|| {
            let groups = self.store.groups_data()?;
            let mut group_views = HashMap::new();
            for group in &groups.groups {
                if let Some(view) = self.store.group_view(&group.id)? {
                    group_views.insert(group.id.clone(), view);
                }
            }
            Ok::<_, StoreError>(Data {
                groups,
                group_views,
                clean_after: self.store.setting(CLEAN_AFTER_SETTING)?.unwrap_or_default(),
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
            Ok(text) => Status {
                text,
                error: false,
                undo: None,
            },
            Err(error) => Status {
                text: error.to_string(),
                error: true,
                undo: None,
            },
        });
    }

    /// Deletes a group with its round log entry, and its photo's record when
    /// no other group is marked on it, offering Undo for a while. Once the
    /// group is gone Undo is offered even if what goes with it couldn't be.
    fn delete_group(&mut self, id: &str) {
        self.delete_groups(&[id.to_owned()]);
    }

    /// Deletes groups with their round log entries, and their photos'
    /// records where no other group is marked on them, offering Undo.
    fn delete_groups(&mut self, ids: &[String]) {
        let mut deleted: Vec<DeletedGroup> = Vec::new();
        let mut problem = None;
        for id in ids {
            let Some(group) = self
                .data
                .groups
                .groups
                .iter()
                .find(|g| &g.id == id)
                .cloned()
            else {
                continue;
            };
            let rounds = crate::groups::rounds::entry_for(&self.data.groups.round_log, id).cloned();
            let view = self.data.group_views.get(id).copied();
            let photo = crate::groups::photo_of(&group, &self.data).cloned();
            if let Err(error) = self.store.delete_group(id) {
                problem = Some(error);
                break;
            }
            let rest = (|| {
                if let Some(entry) = &rounds {
                    self.store.delete_round_log_entry(&entry.id)?;
                }
                Ok::<_, StoreError>(match &photo {
                    Some(photo) if self.store.delete_photo(&photo.id)? => Some(photo.clone()),
                    _ => None,
                })
            })();
            let photo = match rest {
                Ok(photo) => photo,
                Err(error) => {
                    problem = Some(error);
                    photo
                }
            };
            self.groups.deleted(id);
            deleted.push(DeletedGroup {
                group,
                rounds,
                view,
                photo,
                with: Vec::new(),
            });
        }
        if deleted.is_empty() {
            if let Some(error) = problem {
                self.report(Err(error));
            }
            return;
        }
        let text = match (&problem, deleted.len()) {
            (None, 1) => tr("Group deleted").to_owned(),
            (None, count) => trf!("{} groups deleted", count),
            (Some(error), _) => trf!("Group deleted, but not all that went with it: {}", error),
        };
        let mut first = deleted.remove(0);
        first.with = deleted;
        self.data_changed = true;
        self.status = Some(Status {
            text,
            error: problem.is_some(),
            undo: Some(Box::new(first)),
        });
        self.reload();
    }

    /// Puts a deleted group back as it was.
    fn undo_delete(&mut self, deleted: DeletedGroup) {
        let DeletedGroup {
            mut group,
            rounds,
            view,
            photo,
            with,
        } = deleted;
        let several = !with.is_empty();
        let mut result = (|| {
            if let Some(mut photo) = photo {
                self.store.save_photo(&mut photo)?;
            }
            self.store.save_group(&mut group)?;
            if let Some(mut entry) = rounds {
                self.store.save_round_log_entry(&mut entry)?;
            }
            if let Some(view) = view {
                self.store.set_group_view(&group.id, &view)?;
            }
            Ok::<_, StoreError>(())
        })();
        for other in with {
            if result.is_ok() {
                result = self.put_back(other);
            }
        }
        self.data_changed |= result.is_ok();
        self.report(result.map(|()| {
            if several {
                tr("The groups are back").to_owned()
            } else {
                tr("The group is back").to_owned()
            }
        }));
        self.reload();
        if several {
            self.page = Page::Groups;
        } else if let Some(group) = self.data.groups.groups.iter().find(|g| g.id == group.id) {
            self.groups.open(group);
            self.page = Page::Groups;
        }
    }

    /// Saves a deleted group and what went with it again.
    fn put_back(&mut self, deleted: DeletedGroup) -> Result<(), StoreError> {
        let DeletedGroup {
            mut group,
            rounds,
            view,
            photo,
            ..
        } = deleted;
        if let Some(mut photo) = photo {
            self.store.save_photo(&mut photo)?;
        }
        self.store.save_group(&mut group)?;
        if let Some(mut entry) = rounds {
            self.store.save_round_log_entry(&mut entry)?;
        }
        if let Some(view) = view {
            self.store.set_group_view(&group.id, &view)?;
        }
        Ok(())
    }

    /// Keeps a photo just brought in, in the photo store; the browser's
    /// answers later.
    fn keep_photo(&mut self, imported: groups_vision::ImportedPhoto) {
        let Some(photos) = &self.photos else {
            return;
        };
        let groups_vision::ImportedPhoto {
            id,
            jpeg,
            thumbnail,
            ..
        } = imported;
        self.photo_writes.push(PhotoWrite::Photo(photos.put(jpeg)));
        self.photo_writes
            .push(PhotoWrite::Thumbnail(photos.put_thumbnail(&id, thumbnail)));
        self.collect_photo_writes();
    }

    /// Says plainly when a photo couldn't be kept, as the store answers.
    fn collect_photo_writes(&mut self) {
        let mut failed = None;
        self.photo_writes.retain(|write| match write.take() {
            Some(Err(error)) => {
                failed = Some(error);
                false
            }
            Some(Ok(())) => false,
            None => true,
        });
        if let Some(error) = failed {
            self.report(Err(StoreError::Invalid(trf!(
                "The photo couldn't be kept on this device ({}). The storage may be full, or \
                 the browser in a private window that keeps nothing: free some space or use a \
                 normal window, then bring the photo in again.",
                error
            ))));
        }
    }

    /// Saves a group, then what goes with it, which is skipped if the group
    /// couldn't be saved.
    fn save_group(&mut self, save: GroupSave) {
        let GroupSave {
            mut group,
            session,
            photo,
            rounds,
            view,
        } = save;
        let new = !self.data.groups.groups.iter().any(|g| g.id == group.id);
        if let Err(error) = self.store.save_group(&mut group) {
            self.report(Err(error));
            return;
        }
        let rest = (|| {
            if let Some(mut session) = session {
                self.store.save_session(&mut session)?;
            }
            if let Some(mut photo) = photo {
                self.store.save_photo(&mut photo)?;
            }
            match rounds {
                RoundSync::Save(mut entry) => self.store.save_round_log_entry(&mut entry)?,
                RoundSync::Delete(id) => self.store.delete_round_log_entry(&id)?,
                RoundSync::Nothing => {}
            }
            if let Some(view) = view {
                self.store.set_group_view(&group.id, &view)?;
            }
            Ok::<_, StoreError>(())
        })();
        self.data_changed = true;
        self.reload();
        self.groups.saved(&group);
        self.report(rest.map(|()| {
            if new {
                tr("Saved the group").to_owned()
            } else {
                tr("Saved").to_owned()
            }
        }));
    }

    /// Where photos are kept: beside the database on a computer, in the
    /// browser's storage on the web, in memory for a database in memory.
    fn photo_store(&mut self, context: &egui::Context) -> Rc<dyn PhotoStore> {
        if let Some(photos) = &self.photos {
            return photos.clone();
        }
        #[cfg(not(target_arch = "wasm32"))]
        let photos: Rc<dyn PhotoStore> = match self.store.file_path() {
            Some(database) => Rc::new(crate::photos::desktop::FilePhotos::beside(&database)),
            None => Rc::new(ballistics_store::MemoryPhotos::default()),
        };
        #[cfg(target_arch = "wasm32")]
        let photos: Rc<dyn PhotoStore> =
            Rc::new(crate::photos::web::BrowserPhotos::new(context.clone()));
        #[cfg(not(target_arch = "wasm32"))]
        let _ = context;
        self.photos = Some(photos.clone());
        photos
    }

    /// A photo dropped on the window starts a group.
    fn take_dropped_files(&mut self, context: &egui::Context) {
        let dropped = context.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            let name = file
                .path()
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            if !crate::groups::import::is_photo_name(&name) {
                continue;
            }
            self.page = Page::Groups;
            #[cfg(not(target_arch = "wasm32"))]
            match file.bytes() {
                Ok(bytes) => {
                    self.groups.drop_file((name, bytes));
                }
                Err(error) => self.report(Err(StoreError::Invalid(error))),
            }
            #[cfg(target_arch = "wasm32")]
            {
                let slot = self.groups.photo_slot();
                let context = context.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(bytes) = file.bytes_async().await {
                        slot.put((name, bytes));
                        context.request_repaint();
                    }
                });
            }
            break;
        }
    }

    fn apply(&mut self, action: Action) {
        let result = match action {
            Action::Goto(page) => {
                self.page = page;
                return;
            }
            Action::Back => {
                self.back();
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
                    tr("Shot logged: add what came of it, and where the animal stood").to_owned()
                })
            }
            Action::SaveShot(mut shot) => self.store.save_shot(&mut shot).map(|()| {
                self.shot_log.saved(&shot);
                tr("Saved the shot").to_owned()
            }),
            Action::DeleteShot(id) => self.store.delete_shot(&id).map(|()| {
                self.shot_log.deleted(&id);
                tr("Shot deleted").to_owned()
            }),
            Action::LoadDemo => self.store.load_demo().and_then(|report| {
                crate::groups::demo::load(&self.store)?;
                crate::rifle_history::load_demo(&self.store)?;
                self.page = Page::Calculate;
                Ok(trf!("Demo data: {}", import::summary(&report)))
            }),
            Action::DeleteLoad(id) => self.store.delete_rifle_load(&id).map(|()| {
                self.rifles.forget_load_draft();
                tr("Using the bullet's own data again").to_owned()
            }),
            Action::SaveGroup(save) => {
                self.save_group(*save);
                return;
            }
            Action::DeleteGroup(id) => {
                self.delete_group(&id);
                return;
            }
            Action::DeleteGroups(ids) => {
                self.delete_groups(&ids);
                return;
            }
            Action::SaveAnalysis(mut analysis) => self
                .store
                .save_analysis(&mut analysis)
                .map(|()| trf!("Saved {}", crate::groups::analyse::analysis_name(&analysis))),
            Action::DeleteAnalysis(id) => self
                .store
                .delete_analysis(&id)
                .map(|()| tr("Analysis deleted; its groups are kept").to_owned()),
            Action::SaveLadder(mut ladder) => self
                .store
                .save_ladder(&mut ladder)
                .map(|()| tr("Saved the ladder").to_owned()),
            Action::PromoteLadder(promotion) => {
                let crate::groups::analyse::pool::Promotion {
                    mut bullet,
                    mut ladder,
                    load,
                } = *promotion;
                self.store.save_bullet(&mut bullet).and_then(|()| {
                    self.bullets.saved(&bullet);
                    if let Some(mut load) = load {
                        self.store.save_rifle_load(&mut load)?;
                    }
                    ladder.promoted_bullet_id = Some(bullet.id.clone());
                    self.store.save_ladder(&mut ladder)?;
                    Ok(trf!("{} is your load now", bullet.display_name()))
                })
            }
            Action::SaveZeroReset(mut reset) => self
                .store
                .save_zero_reset(&mut reset)
                .map(|()| tr("Zero reset marked: zero checks start from now").to_owned()),
            Action::KeepPhoto(imported) => {
                self.keep_photo(*imported);
                return;
            }
            Action::Record(change) => self.apply_record(*change),
            Action::SetCleanAfter(rifle_id, rounds) => {
                self.data.clean_after.insert(rifle_id, rounds);
                if let Err(error) = self
                    .store
                    .set_setting(CLEAN_AFTER_SETTING, &self.data.clean_after)
                {
                    self.report(Err(error));
                }
                return;
            }
            Action::OpenGroup(id) => {
                if let Some(group) = self.data.groups.groups.iter().find(|g| g.id == id) {
                    self.groups.open(group);
                    self.page = Page::Groups;
                }
                return;
            }
            Action::SaveFile(file) => {
                match crate::files::save(&file) {
                    Ok(Some(said)) => self.report(Ok(said)),
                    Ok(None) => {}
                    Err(error) => self.report(Err(StoreError::Invalid(error))),
                }
                return;
            }
            Action::Problem(text) => {
                self.report(Err(StoreError::Invalid(text)));
                return;
            }
        };
        self.data_changed |= result.is_ok();
        self.report(result);
        self.reload();
    }

    /// Applies a change from the Rifles or Sessions page or a group's
    /// velocities, and keeps the open editors in step with it.
    fn apply_record(&mut self, change: crate::records::Change) -> Result<String, StoreError> {
        let applied = crate::records::apply(&self.store, &self.data, change)?;
        if let Some(rifle) = &applied.rifle {
            self.rifles.history_changed(rifle);
        }
        if applied.load {
            self.rifles.forget_load_draft();
        }
        if let Some(session) = &applied.session {
            self.sessions.saved(session);
        }
        if let Some(id) = &applied.session_deleted {
            self.sessions.deleted(id);
        }
        Ok(applied.message)
    }

    /// Starts the backup with photos: the photos are read first.
    fn export_with_photos(&mut self, context: &egui::Context) {
        let json = match self.store.export_json() {
            Ok(json) => json,
            Err(error) => return self.report(Err(error)),
        };
        let ids = self
            .data
            .groups
            .photos
            .iter()
            .map(|photo| photo.id.clone())
            .collect();
        match crate::backup::PhotoExport::start(&json, ids) {
            Ok(export) => {
                self.photo_export = Some(export);
                self.report(Ok(tr("Gathering the photos…").to_owned()));
            }
            Err(error) => self.report(Err(StoreError::Invalid(error))),
        }
        self.collect_photo_export(context);
    }

    /// Adds the photos to the backup with photos as they are read, and
    /// saves it once they are in.
    fn collect_photo_export(&mut self, context: &egui::Context) {
        if self.photo_export.is_none() {
            return;
        }
        let photos = self.photo_store(context);
        let Some(done) = self
            .photo_export
            .as_mut()
            .and_then(|export| export.poll(photos.as_ref()))
        else {
            return;
        };
        self.photo_export = None;
        match done {
            Ok((file, said)) => match crate::files::save(&file) {
                Ok(Some(saved)) => self.report(Ok(format!("{saved}. {said}"))),
                Ok(None) => self.status = None,
                Err(error) => self.report(Err(StoreError::Invalid(error))),
            },
            Err(error) => self.report(Err(StoreError::Invalid(error))),
        }
    }

    /// Puts back the next photo a backup with photos brought, with a new
    /// thumbnail: one a frame, so a phone isn't held up.
    fn restore_photos(&mut self, context: &egui::Context) {
        let photos = self.photo_store(context);
        if let Some((photo, thumbnail)) = self.photo_restore.next(photos.as_ref()) {
            self.photo_writes.push(PhotoWrite::Photo(photo));
            if let Some(thumbnail) = thumbnail {
                self.photo_writes.push(PhotoWrite::Thumbnail(thumbnail));
            }
            self.collect_photo_writes();
            context.request_repaint();
        }
    }

    /// The sheet that saves the groups as CSV: per group or per shot.
    fn groups_csv_sheet(&mut self, context: &egui::Context) {
        use crate::groups::details::export;
        let groups: Vec<&Group> = self.data.groups.groups.iter().collect();
        let shown = mobile::sheet(context, "groups-csv", tr("Groups as CSV"), |ui| {
            ui.weak(trf!(
                "{} groups, in metric units, for a spreadsheet.",
                groups.len()
            ));
            let mut file = None;
            if mobile::list_row(
                ui,
                tr("One row per group"),
                tr("With its load and weather"),
                false,
            )
            .clicked()
            {
                file = Some(("mxsc-groups.csv", export::groups_csv(&groups, &self.data)));
            }
            if mobile::list_row(
                ui,
                tr("One row per shot"),
                tr("mm and mrad, from the aim and from the centre"),
                false,
            )
            .clicked()
            {
                file = Some(("mxsc-shots.csv", export::shots_csv(&groups, &self.data)));
            }
            file
        });
        if let Some((name, csv)) = shown.inner {
            self.menu = None;
            self.apply(Action::SaveFile(Box::new(crate::files::FileOut {
                name: name.to_owned(),
                bytes: csv.into_bytes(),
                mime: "text/csv",
            })));
        } else if shown.closed {
            self.menu = None;
        }
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
            .add_filter(tr("Backups"), &["json", "zip", "srl"])
            .add_filter("MX Shooting Calculator (JSON)", &["json"])
            .add_filter(tr("Backup with photos (ZIP)"), &["zip"])
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
        match ImportPreview::from_bytes(
            &mut self.store,
            file.name,
            file.bytes,
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
                let photos = self
                    .import
                    .take()
                    .map(|mut preview| preview.take_photos())
                    .unwrap_or_default();
                let restored = crate::backup::restored_text(photos.len());
                self.photo_restore.add(photos);
                self.reload();
                self.data_changed = true;
                if let Some((rifle_id, bullet_id)) = selected {
                    self.calculator.inputs.rifle_id = Some(rifle_id);
                    self.calculator.inputs.bullet_id = bullet_id;
                    self.page = Page::Calculate;
                }
                let mut said = trf!("Imported: {}", import::summary(&report));
                if !restored.is_empty() {
                    said = format!("{said}. {restored}");
                }
                self.report(Ok(said));
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
        let groups = &self.groups.settings;
        if *groups != self.saved_group_settings && !dragging {
            match self.store.set_setting(crate::groups::SETTINGS, groups) {
                Ok(()) => self.saved_group_settings = groups.clone(),
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = &ui.ctx().clone();
        self.show_theme(context);
        self.units.install(context);
        mobile::apply_style(context);
        let narrow = mobile::is_narrow(context);
        self.now = context.input(|input| input.time);
        #[cfg(target_arch = "wasm32")]
        for _ in 0..crate::web::take_backs() {
            self.back();
        }
        if narrow {
            self.phone_header(ui);
            self.tab_bar(ui);
        } else {
            self.desktop_header(ui);
        }
        self.status_bar(ui);
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !std::mem::replace(&mut self.updates_started, true) && self.check_updates {
                self.updates.check(context);
            }
            self.updates.notice(ui);
        }

        self.data.region = self.calculator.inputs.region.clone();
        self.data.today = self.calculator.inputs.conditions();
        let photos = self.photo_store(context);
        self.take_dropped_files(context);
        self.collect_photo_writes();
        let actions = match self.page {
            Page::Calculate => self.calculator.show(ui, &self.data),
            Page::Groups => self.groups.show_with_tools(ui, &self.data, photos.as_ref()),
            Page::Sessions => self.sessions.show(ui, &self.data),
            Page::Rifles => self.rifles.show(ui, &self.data),
            Page::Bullets => self.bullets.show(ui, &self.data),
            Page::Targets => self.targets.show(ui, &self.data),
            Page::ShotLog => {
                let place = self.calculator.inputs.place.as_ref();
                self.shot_log.show(ui, &self.data, place)
            }
            Page::Cards => {
                let subject = self.calculator.card_subject(&self.data);
                self.cards.show(ui, subject, self.data.hunting)
            }
        };
        for action in actions {
            self.apply(action);
        }
        if self.page != Page::ShotLog {
            self.shot_log.leave();
        }
        if self.page != Page::Groups {
            self.groups.leave();
        }
        crate::groups::drop_hint(context);
        self.collect_photo_export(context);
        self.restore_photos(context);
        // The phone menu's sheets; on wider screens the desktop menu opens
        // the ones it has no room for.
        self.menu_sheet(context);
        #[cfg(target_arch = "wasm32")]
        self.collect_picked_file();
        self.import_window(context);
        self.welcome(context);
        self.discard_dialog(context);
        self.persist_inputs(context);
        self.run_sync(context);
        self.run_watch(context);
        #[cfg(target_arch = "wasm32")]
        crate::web::navigation(context, self.depth(context), self.unsaved());
    }
}

impl App {
    #[cfg(target_arch = "wasm32")]
    /// Views open over the page on a phone, each one a step for Back.
    fn depth(&self, context: &egui::Context) -> usize {
        if !mobile::is_narrow(context) {
            return 0;
        }
        let overlays = usize::from(self.menu.is_some())
            + usize::from(self.import.is_some())
            + usize::from(self.sync.dialog_open());
        overlays
            + match self.page {
                Page::Calculate => self.calculator.depth(),
                Page::Groups => self.groups.depth(),
                Page::Rifles => self.rifles.depth(),
                Page::Bullets => self.bullets.depth(),
                Page::Targets => self.targets.depth(),
                // Over Calculate, from the menu.
                Page::Cards => 1,
                // Over Calculate, where its ‹ link goes.
                Page::ShotLog => 1 + self.shot_log.depth(),
                // Over Calculate, from the menu.
                Page::Sessions => 1 + self.sessions.depth(),
            }
    }

    /// Back one view: an open dialog or the menu first. Asks first when
    /// that would drop unsaved changes.
    fn back(&mut self) {
        if self.discard_asked.is_some() {
            return;
        }
        if self.sync.dialog_open() {
            self.sync.close_dialog();
            return;
        }
        if self.import.take().is_some() {
            return;
        }
        if self.menu.take().is_some() {
            return;
        }
        let unsaved = match self.page {
            Page::Groups => self.groups.unsaved(&self.data),
            Page::Rifles => self.rifles.unsaved(&self.data),
            Page::Bullets => self.bullets.unsaved(&self.data),
            Page::Targets => self.targets.unsaved(&self.data),
            Page::ShotLog => self.shot_log.unsaved(&self.data),
            Page::Sessions => self.sessions.unsaved(&self.data),
            Page::Calculate | Page::Cards => false,
        };
        if unsaved {
            self.discard_asked = Some(self.now);
        } else {
            self.go_back();
        }
    }

    fn go_back(&mut self) {
        match self.page {
            Page::Calculate => self.calculator.back(),
            Page::Groups => self.groups.back(&self.data),
            Page::Rifles => self.rifles.back(),
            Page::Bullets => self.bullets.back(),
            Page::Targets => self.targets.back(),
            Page::Cards => self.page = Page::Calculate,
            Page::ShotLog if self.shot_log.depth() > 0 => self.shot_log.back(),
            Page::ShotLog => self.page = Page::Calculate,
            Page::Sessions if self.sessions.depth() > 0 => self.sessions.back(),
            Page::Sessions => self.page = Page::Calculate,
        }
    }

    #[cfg(target_arch = "wasm32")]
    /// Some editor has changes that aren't saved.
    fn unsaved(&self) -> bool {
        self.groups.has_unsaved(&self.data)
            || self.rifles.unsaved(&self.data)
            || self.bullets.unsaved(&self.data)
            || self.targets.unsaved(&self.data)
            || self.shot_log.unsaved(&self.data)
            || self.sessions.unsaved(&self.data)
    }

    /// "Discard your changes?", when Back would drop them.
    fn discard_dialog(&mut self, context: &egui::Context) {
        let Some(asked_at) = self.discard_asked else {
            return;
        };
        let width = (context.content_rect().width() - 48.0).min(400.0);
        let mut keep = false;
        let mut discard = false;
        let modal = egui::Modal::new(egui::Id::unique("discard-changes")).show(context, |ui| {
            ui.set_width(width);
            ui.heading(tr("Discard your changes?"));
            ui.label(tr("They aren't saved yet."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                keep = ui.button(tr("Keep editing")).clicked();
                let ready = crate::widgets::confirm_ready(ui, asked_at);
                discard = ui
                    .add_enabled(
                        ready,
                        egui::Button::new(
                            egui::RichText::new(tr("Discard")).color(ui.visuals().error_fg_color),
                        ),
                    )
                    .clicked();
            });
        });
        if keep || discard || modal.should_close() {
            self.discard_asked = None;
        }
        if discard {
            self.go_back();
        }
    }
}

impl App {
    /// Keeps the Garmin watch's rifles up to date.
    fn run_watch(&mut self, context: &egui::Context) {
        let calculator = &self.calculator;
        let data = &self.data;
        self.watch.poll(
            &self.store,
            context,
            || calculator.watch_subjects(data),
            &self.units,
        );
    }

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
            .id(egui::Id::unique("whats-new"))
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
            .id(egui::Id::unique("about"))
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
                egui::CollapsingHeader::new(tr("Catalog data attribution")).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            ui.label(ballistics_store::catalog::CHAMBERSIM_DATA_NOTICE);
                            ui.hyperlink_to(
                                "ChamberSim Reloaded project database",
                                "https://github.com/Schildkroet/ChamberSim-Reloaded",
                            );
                            ui.hyperlink_to(
                                "CC BY-SA 4.0",
                                "https://creativecommons.org/licenses/by-sa/4.0/",
                            );
                        });
                });
                ui.weak(tr(
                    "Your rifles, bullets, targets and shot log stay on this device, and on \
                     your other devices if you sync them.",
                ));
            });
        self.about_open = open;
        let mut open = self.sync.window_open;
        egui::Window::new(tr("Sync"))
            .id(egui::Id::unique("sync"))
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

    fn desktop_header(&mut self, ui: &mut egui::Ui) {
        let context = &ui.ctx().clone();
        let palette = theme::palette(context);
        let header = egui::Frame::new()
            .fill(palette.chrome)
            .inner_margin(egui::Margin::symmetric(14, 8))
            .stroke(theme::hairline(palette.stroke));
        egui::Panel::top("navigation").frame(header).show(ui, |ui| {
            ui.horizontal(|ui| {
                if brand(ui) {
                    self.whats_new_open = !self.whats_new_open;
                }
                ui.add_space(28.0);
                for (page, label) in [
                    (Page::Calculate, tr("Calculate")),
                    (Page::Groups, tr("Groups")),
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
                        palette.text
                    } else {
                        palette.text_muted
                    };
                    ui.painter().circle_filled(
                        rect.left_center() + egui::vec2(4.0, 0.0),
                        4.0,
                        self.sync.colour(palette),
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
        let mut export_photos = false;
        let mut demo = false;
        ui.menu_button(RichText::new("•••").size(14.0), |ui| {
            ui.set_min_width(230.0);
            if ui
                .button(tr("Shot log"))
                .on_hover_text(tr("The shots you logged from Calculate"))
                .clicked()
            {
                self.page = Page::ShotLog;
                ui.close();
            }
            if ui
                .button(tr("Sessions"))
                .on_hover_text(tr("Days at the range, with their weather and groups"))
                .clicked()
            {
                self.page = Page::Sessions;
                ui.close();
            }
            ui.separator();
            // Choices open a sheet: submenus would have no room to open at
            // the window's right edge and would cover this menu instead.
            let purpose = self.purpose.unwrap_or(Purpose::Hunting).name();
            let language = self.language.map_or(tr("System"), Language::own_name);
            let mut rows = vec![
                (tr("Units…").to_owned(), MenuSheet::Units),
                (trf!("Language: {}…", language), MenuSheet::Language),
                (trf!("Theme: {}…", self.theme.name()), MenuSheet::Theme),
                (trf!("Use for: {}…", purpose), MenuSheet::Purpose),
            ];
            if self.hunting() {
                let country = self.country.map_or(tr("none"), i18n::country_name);
                rows.push((trf!("Hunting rules: {}…", country), MenuSheet::Country));
            }
            for (label, sheet) in rows {
                if ui.button(label).clicked() {
                    self.menu = Some(sheet);
                    ui.close();
                }
            }
            let now = ui.ctx().input(|input| input.time);
            if ui
                .button(tr("Sync…"))
                .on_hover_text(self.sync.status(now))
                .clicked()
            {
                sync = true;
                ui.close();
            }
            if ui
                .button(tr("Garmin watch…"))
                .on_hover_text(tr("Every rifle and load on a Garmin watch"))
                .clicked()
            {
                self.menu = Some(MenuSheet::Watch);
                ui.close();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button(tr("Updates…")).clicked() {
                self.menu = Some(MenuSheet::Updates);
                ui.close();
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
                ui.close();
            }
            if ui
                .button(tr("Export backup"))
                .on_hover_text(tr(
                    "Your rifles, bullets, scopes, targets and shot log, as JSON",
                ))
                .clicked()
            {
                export = true;
                ui.close();
            }
            if ui
                .button(tr("Export backup with photos"))
                .on_hover_text(tr(
                    "The backup and the photos your groups are marked on, as one ZIP file",
                ))
                .clicked()
            {
                export_photos = true;
                ui.close();
            }
            if ui
                .button(tr("Groups as CSV…"))
                .on_hover_text(tr("For a spreadsheet: one row per group, or per shot"))
                .clicked()
            {
                self.menu = Some(MenuSheet::GroupsCsv);
                ui.close();
            }
            if ui
                .button(tr("Load demo data"))
                .on_hover_text(tr(
                    "Three rifles with bullets, a scope, a target and groups",
                ))
                .clicked()
            {
                demo = true;
                ui.close();
            }
            ui.separator();
            if ui.button(tr("What's new")).clicked() {
                self.whats_new_open = true;
                ui.close();
            }
            if ui.button(tr("About")).clicked() {
                self.about_open = true;
                ui.close();
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
        if export_photos {
            self.export_with_photos(context);
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
        let width = (context.content_rect().width() - 48.0).min(440.0);
        let modal = egui::Modal::new(egui::Id::unique("welcome")).show(context, |ui| {
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

    /// Hands the chosen theme to egui (and to the web page) when it changed.
    pub fn show_theme(&mut self, context: &egui::Context) {
        if self.theme_shown != Some(self.theme) {
            theme::choose(context, self.theme);
            self.theme_shown = Some(self.theme);
            context.request_repaint();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let shown = (self.theme, context.theme());
            // Before the first frame egui doesn't know the system's theme yet.
            let known = self.theme != ThemeChoice::System || context.system_theme().is_some();
            if known && self.page_theme != Some(shown) {
                crate::web::show_theme(self.theme, theme::palette(context));
                self.page_theme = Some(shown);
            }
        }
    }

    fn set_theme(&mut self, context: &egui::Context, choice: ThemeChoice) {
        self.theme = choice;
        self.show_theme(context);
        if let Err(error) = self.store.set_setting(THEME_SETTING, &choice) {
            self.report(Err(error));
        }
    }

    fn units_changed(&mut self, context: &egui::Context) {
        self.units.install(context);
        if let Err(error) = self.store.set_setting(UNIT_SETTINGS, &self.units) {
            self.report(Err(error));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
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
                let shown_for = if status.undo.is_some() {
                    UNDO_SECONDS
                } else {
                    STATUS_SECONDS
                };
                if now - since > shown_for {
                    self.status = None;
                } else {
                    context.request_repaint_after(std::time::Duration::from_secs(1));
                }
            }
            _ => self.status_seen = None,
        }
        if let Some(status) = &self.status {
            let mut dismiss = false;
            let mut undo = false;
            if mobile::is_narrow(&context) {
                let frame = egui::Frame::side_top_panel(ui.style());
                let undo_width = if status.undo.is_some() {
                    UNDO_BUTTON_WIDTH + ui.spacing().item_spacing.x
                } else {
                    0.0
                };
                let text_width = (ui.available_width()
                    - frame.total_margin().sum().x
                    - mobile::TOUCH
                    - ui.spacing().item_spacing.x
                    - undo_width)
                    .max(1.0);
                let colour = if status.error {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().text_color()
                };
                let text = ui.painter().layout(
                    status.text.clone(),
                    egui::TextStyle::Body.resolve(ui.style()),
                    colour,
                    text_width,
                );
                // Bottom panels otherwise start with an estimated height and can
                // paint their wrapped contents underneath the navigation bar.
                let height = text.size().y.max(mobile::TOUCH)
                    + frame.total_margin().sum().y
                    + ui.visuals().widgets.noninteractive.bg_stroke.width.round();
                egui::Panel::bottom("status")
                    .frame(frame)
                    .exact_size(height)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.add(egui::Label::new(text));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                dismiss = ui
                                    .add_sized(
                                        egui::vec2(mobile::TOUCH, mobile::TOUCH),
                                        egui::Button::new("✖").small(),
                                    )
                                    .clicked();
                                if status.undo.is_some() {
                                    undo =
                                        ui.add(egui::Button::new(tr("Undo")).min_size(egui::vec2(
                                            UNDO_BUTTON_WIDTH,
                                            mobile::TOUCH,
                                        )))
                                        .clicked();
                                }
                            });
                        });
                    });
            } else {
                egui::Panel::bottom("status").show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let text = RichText::new(&status.text);
                        if status.error {
                            ui.colored_label(ui.visuals().error_fg_color, text);
                        } else {
                            ui.label(text);
                        }
                        if status.undo.is_some() {
                            undo = ui.button(tr("Undo")).clicked();
                        }
                        dismiss = ui.small_button("✖").clicked();
                    });
                });
            }
            if undo && let Some(deleted) = self.status.take().and_then(|status| status.undo) {
                self.undo_delete(*deleted);
            } else if dismiss {
                self.status = None;
            }
        }
    }

    /// The phone's header: the logo and the menu button.
    fn phone_header(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.ctx());
        let header = egui::Frame::new()
            .fill(palette.chrome)
            .inner_margin(egui::Margin::symmetric(12, 6))
            .stroke(theme::hairline(palette.stroke));
        egui::Panel::top("phone-header")
            .frame(header)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(theme::icon()).fit_to_exact_size(egui::vec2(30.0, 30.0)),
                    );
                    ui.add(
                        egui::Image::new(theme::wordmark(palette))
                            .fit_to_exact_size(egui::vec2(32.0, 18.0)),
                    );
                    ui.label(
                        RichText::new(format!("v{}", crate::VERSION))
                            .small()
                            .color(palette.text_muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let menu = ui.add(
                            egui::Button::new(RichText::new("•••").color(palette.text))
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
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.ctx());
        let frame = egui::Frame::new()
            .fill(palette.chrome)
            .inner_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 4,
                bottom: 10,
            })
            .stroke(theme::hairline(palette.stroke));
        let height = 54.0
            + frame.total_margin().sum().y
            + ui.visuals().widgets.noninteractive.bg_stroke.width.round();
        let panel = egui::Panel::bottom("tabs").frame(frame).exact_size(height);
        panel.show(ui, |ui| {
            let tabs = [
                (Page::Calculate, tr("Calculate")),
                (Page::Groups, tr("Groups")),
                (Page::Rifles, tr("Rifles")),
                (Page::Bullets, tr("Bullets")),
                (Page::Targets, tr("Targets")),
            ];
            let width = ui.available_width() / tabs.len() as f32;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (page, label) in tabs {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(width, 54.0), egui::Sense::click());
                    let colour = if self.page == page {
                        palette.accent
                    } else {
                        palette.text_muted
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
                    let sessions = match self.data.groups.sessions.len() {
                        0 => tr("Days at the range, with their weather").to_owned(),
                        1 => tr("1 session").to_owned(),
                        count => trf!("{} sessions", count),
                    };
                    if mobile::list_row(ui, tr("Sessions"), &sessions, false).clicked() {
                        choice = Some(MenuChoice::Sessions);
                    }
                    if mobile::list_row(ui, tr("Cards"), tr("Range cards to print"), false)
                        .clicked()
                    {
                        choice = Some(MenuChoice::Cards);
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
                    if mobile::list_row(ui, tr("Theme"), self.theme.name(), false).clicked() {
                        choice = Some(MenuChoice::Theme);
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
                    if mobile::list_row(ui, tr("Garmin watch"), self.watch.short_status(), false)
                        .clicked()
                    {
                        choice = Some(MenuChoice::Watch);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if mobile::list_row(
                        ui,
                        tr("Updates"),
                        if self.check_updates {
                            tr("Checked when the app starts")
                        } else {
                            tr("Off")
                        },
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::Updates);
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
                        tr("Export backup with photos"),
                        tr("The backup and your groups' photos, as one ZIP"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::ExportWithPhotos);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Groups as CSV"),
                        tr("For a spreadsheet: per group or per shot"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(MenuChoice::GroupsCsv);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Load demo data"),
                        tr("Three rifles with bullets, a scope, a target and groups"),
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
                    Some(MenuChoice::Theme) => self.menu = Some(MenuSheet::Theme),
                    Some(MenuChoice::Country) => self.menu = Some(MenuSheet::Country),
                    Some(MenuChoice::Purpose) => self.menu = Some(MenuSheet::Purpose),
                    Some(MenuChoice::Sync) => self.menu = Some(MenuSheet::Sync),
                    Some(MenuChoice::Watch) => self.menu = Some(MenuSheet::Watch),
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(MenuChoice::Updates) => self.menu = Some(MenuSheet::Updates),
                    Some(MenuChoice::WhatsNew) => self.menu = Some(MenuSheet::WhatsNew),
                    Some(MenuChoice::ShotLog) => {
                        self.menu = None;
                        self.page = Page::ShotLog;
                    }
                    Some(MenuChoice::Cards) => {
                        self.menu = None;
                        self.page = Page::Cards;
                    }
                    Some(MenuChoice::Sessions) => {
                        self.menu = None;
                        self.page = Page::Sessions;
                    }
                    Some(MenuChoice::ExportWithPhotos) => {
                        self.menu = None;
                        self.export_with_photos(context);
                    }
                    Some(MenuChoice::GroupsCsv) => self.menu = Some(MenuSheet::GroupsCsv),
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
            MenuSheet::GroupsCsv => self.groups_csv_sheet(context),
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
            MenuSheet::Watch => {
                let subjects = self.calculator.watch_subjects(&self.data);
                let units = &self.units;
                let watch = &mut self.watch;
                let store = &self.store;
                let shown = mobile::sheet(context, "watch", tr("Garmin watch"), |ui| {
                    watch.panel(ui, store, &subjects, units)
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
            #[cfg(not(target_arch = "wasm32"))]
            MenuSheet::Updates => {
                let mut enabled = self.check_updates;
                let mut check = false;
                let updates = &mut self.updates;
                let shown = mobile::sheet(context, "updates", tr("Updates"), |ui| {
                    ui.checkbox(&mut enabled, tr("Check for updates when the app starts"));
                    ui.weak(tr(
                        "The app then asks GitHub, where its releases are kept, for the latest \
                         version. Nothing else is sent. An update is installed only when you \
                         say so, and only if it is signed by the app's maker.",
                    ));
                    ui.add_space(8.0);
                    ui.label(trf!("This is version {}.", crate::VERSION));
                    let busy = matches!(
                        updates.state(),
                        crate::updates::State::Checking | crate::updates::State::Downloading(_)
                    );
                    if ui
                        .add_enabled(!busy, egui::Button::new(tr("Check now")))
                        .clicked()
                    {
                        check = true;
                    }
                    updates.show(ui);
                });
                if enabled != self.check_updates {
                    self.check_updates = enabled;
                    let _ = self.store.set_setting(crate::updates::SETTING, &enabled);
                    check |= enabled;
                }
                if check {
                    self.updates.check(context);
                }
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
            MenuSheet::Theme => {
                let current = self.theme;
                let shown = mobile::sheet(context, "theme", tr("Theme"), |ui| {
                    let mut chosen = None;
                    for choice in ThemeChoice::ALL {
                        if mobile::list_row(ui, choice.name(), choice.detail(), current == choice)
                            .clicked()
                        {
                            chosen = Some(choice);
                        }
                    }
                    chosen
                });
                if let Some(choice) = shown.inner {
                    self.set_theme(context, choice);
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
        Page::Groups => {
            // A sheet of paper with a group of holes on it.
            painter.add(line(&[
                (4.0, 3.0),
                (20.0, 3.0),
                (20.0, 21.0),
                (4.0, 21.0),
                (4.0, 3.0),
            ]));
            for (x, y) in [(9.5, 10.0), (13.5, 8.5), (14.5, 13.5), (10.0, 15.0)] {
                painter.circle_filled(at(x, y), 1.7, colour);
            }
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
        Page::ShotLog | Page::Sessions => {}
    }
}

/// The logo in the header: icon, MX wordmark and tagline, all vector.
/// Returns true when the version number was clicked (for What's new).
fn brand(ui: &mut egui::Ui) -> bool {
    let palette = theme::palette(ui.ctx());
    let mut clicked = false;
    ui.add(egui::Image::new(theme::icon()).fit_to_exact_size(egui::vec2(36.0, 36.0)));
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(theme::wordmark(palette))
                    .fit_to_exact_size(egui::vec2(34.0, 19.0)),
            );
            let version = ui
                .add(
                    egui::Label::new(
                        RichText::new(format!("v{}", crate::VERSION))
                            .size(10.0)
                            .color(palette.text_muted),
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
                .color(palette.accent)
                .extra_letter_spacing(1.6),
        );
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_update_notice_fits_above_navigation_from_the_first_frame() {
        for language in Language::ALL {
            for (width, height) in [(320.0, 568.0), (390.0, 844.0), (600.0, 800.0)] {
                let context = egui::Context::default();
                theme::apply(&context);
                let store = Store::open_in_memory().unwrap();
                store.set_setting(SEEN_VERSION, &"0.8.7").unwrap();
                store
                    .set_setting(LANGUAGE_SETTING, &Some(language))
                    .unwrap();
                let mut app = App::new(store);
                let message = app.status.as_ref().unwrap().text.clone();
                for time in [0.0, 1.0, STATUS_SECONDS + 0.1] {
                    let expired = time > STATUS_SECONDS;
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            time: Some(time),
                            ..Default::default()
                        },
                        |ui| {
                            mobile::apply_style(&context);
                            app.phone_header(ui);
                            app.tab_bar(ui);
                            let above_tabs = ui.available_rect_before_wrap();
                            app.status_bar(ui);
                            let tabs = egui::containers::panel::PanelState::load(
                                &context,
                                ui.scope_id().with_salt(egui::IdSalt::new("tabs")),
                            )
                            .unwrap()
                            .outer_rect;
                            assert!(
                                (tabs.bottom() - height).abs() <= 0.5,
                                "navigation must stay on screen from the first frame"
                            );
                            if expired {
                                assert_eq!(
                                    ui.available_rect_before_wrap(),
                                    above_tabs,
                                    "expired notice must release all of its space"
                                );
                            } else {
                                let status = egui::containers::panel::PanelState::load(
                                    &context,
                                    ui.scope_id().with_salt(egui::IdSalt::new("status")),
                                )
                                .unwrap()
                                .outer_rect;
                                assert!(
                                    status.bottom() <= tabs.top() + 0.5,
                                    "notice overlaps the navigation"
                                );
                            }
                        },
                    );
                    let rendered = output.shapes.iter().find_map(|shape| {
                        if let egui::epaint::Shape::Text(text) = &shape.shape
                            && text.galley.job.text == message
                        {
                            return Some((
                                shape.clip_rect,
                                egui::Rect::from_min_size(text.pos, text.galley.size()),
                            ));
                        }
                        None
                    });
                    if expired {
                        assert!(app.status.is_none());
                        assert!(rendered.is_none());
                    } else {
                        let (clip, text) = rendered.expect("the update message must be painted");
                        assert!(
                            clip.expand(0.5).contains_rect(text),
                            "message is clipped: {language:?}, {width}x{height}, {time}: text={text:?}, clip={clip:?}"
                        );
                    }
                    output.textures_delta.clear();
                }
            }
        }
    }

    /// Every text the frame painted.
    fn painted(output: &egui::FullOutput) -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rifle_history_sessions_and_velocities_draw_in_both_themes() {
        for choice in [ThemeChoice::Dark, ThemeChoice::Sun] {
            for width in [1200.0, 390.0] {
                let context = egui::Context::default();
                theme::apply(&context);
                let store = Store::open_in_memory().unwrap();
                store.set_setting(THEME_SETTING, &choice).unwrap();
                store
                    .set_setting(PURPOSE_SETTING, &Purpose::Shooting)
                    .unwrap();
                let mut app = App::new(store);
                app.apply(Action::LoadDemo);
                let mut frame = eframe::Frame::_new_kittest();
                let mut run = |app: &mut App| {
                    let mut texts = Vec::new();
                    for _ in 0..3 {
                        let mut output = context.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(width, 2400.0),
                                )),
                                ..Default::default()
                            },
                            |ui| eframe::App::ui(app, ui, &mut frame),
                        );
                        texts = painted(&output);
                        output.textures_delta.clear();
                    }
                    texts
                };
                let rifle = app
                    .data
                    .rifles
                    .iter()
                    .find(|rifle| rifle.fitted_barrel_id.is_some())
                    .cloned()
                    .expect("the demo has a rifle with barrels");
                app.rifles.edit_load(&rifle, "");
                app.page = Page::Rifles;
                let texts = run(&mut app);
                if width < 640.0 {
                    let right: f32 = context
                        .data(|memory| {
                            memory.get_temp(egui::Id::unique(crate::rifles::EDITOR_RIGHT))
                        })
                        .expect("the editor was drawn");
                    assert!(right <= width + 0.5, "the rifle editor ends at {right}");
                }
                for expected in ["Round counts", "Since clean", "Barrels", "Recent rounds"] {
                    assert!(texts.iter().any(|t| t == expected), "{expected} at {width}");
                }

                let session = app.data.groups.sessions[0].clone();
                app.page = Page::Sessions;
                app.sessions.open(&session, &app.data);
                let texts = run(&mut app);
                assert!(texts.iter().any(|t| t == "Edit session"), "at {width}");

                let group = app
                    .data
                    .groups
                    .groups
                    .iter()
                    .find(|group| {
                        crate::groups::details::velocity::shot_velocities(&group.marks)
                            .iter()
                            .flatten()
                            .count()
                            >= 5
                    })
                    .cloned()
                    .expect("a demo group has velocities");
                app.apply(Action::OpenGroup(group.id.clone()));
                assert!(app.page == Page::Groups);
                let texts = run(&mut app);
                assert!(
                    texts.iter().any(|t| t == "Velocity × vertical"),
                    "the chart at {width}"
                );
            }
        }
    }

    #[test]
    fn every_page_draws_in_both_themes_on_a_desktop_and_a_phone() {
        for choice in [ThemeChoice::Dark, ThemeChoice::Sun] {
            for width in [1200.0, 390.0] {
                let context = egui::Context::default();
                theme::apply(&context);
                let store = Store::open_in_memory().unwrap();
                store.set_setting(THEME_SETTING, &choice).unwrap();
                store
                    .set_setting(PURPOSE_SETTING, &Purpose::Hunting)
                    .unwrap();
                let mut app = App::new(store);
                app.apply(Action::LoadDemo);
                assert!(!app.data.rifles.is_empty(), "the demo data loads");
                let mut frame = eframe::Frame::_new_kittest();
                let views = [
                    (Page::Calculate, None),
                    (Page::Groups, None),
                    (Page::Rifles, None),
                    (Page::Bullets, None),
                    (Page::Targets, None),
                    (Page::Cards, None),
                    (Page::ShotLog, None),
                    (Page::Sessions, None),
                    (Page::Calculate, Some(MenuSheet::GroupsCsv)),
                    (Page::Calculate, Some(MenuSheet::Main)),
                    (Page::Calculate, Some(MenuSheet::Theme)),
                ];
                for (page, menu) in views {
                    app.page = page;
                    app.menu = menu;
                    for _ in 0..2 {
                        let mut output = context.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(width, 844.0),
                                )),
                                ..Default::default()
                            },
                            |ui| eframe::App::ui(&mut app, ui, &mut frame),
                        );
                        output.textures_delta.clear();
                    }
                }
                let expected = match choice {
                    ThemeChoice::Sun => &theme::SUN,
                    _ => &theme::DARK,
                };
                assert_eq!(theme::palette(&context), expected, "{choice:?}");
                assert_eq!(context.global_style().visuals.dark_mode, expected.dark);
            }
        }
    }

    #[test]
    fn the_theme_is_kept_on_the_device() {
        let context = egui::Context::default();
        theme::apply(&context);
        let store = Store::open_in_memory().unwrap();
        let mut app = App::new(store);
        assert_eq!(app.theme, ThemeChoice::Dark, "Dark until another is chosen");
        app.set_theme(&context, ThemeChoice::Sun);
        assert_eq!(theme::palette(&context), &theme::SUN);
        let saved: Option<ThemeChoice> = app.store.setting(THEME_SETTING).unwrap();
        assert_eq!(saved, Some(ThemeChoice::Sun));
        app.set_theme(&context, ThemeChoice::System);
        assert_eq!(
            theme::palette(&context),
            &theme::DARK,
            "no system theme: Dark"
        );
    }

    #[test]
    fn a_deleted_group_comes_back_with_undo_and_its_rounds_follow_it() {
        use crate::groups::rounds::{self, RoundSync};
        let mut app = App::new(Store::open_in_memory().unwrap());
        app.apply(Action::LoadDemo);
        assert_eq!(
            app.data.groups.groups.len(),
            crate::groups::demo::GROUP_COUNT,
            "the demo groups"
        );
        let mut group = app.data.groups.groups[0].clone();
        group.count_rounds = true;
        let entry = rounds::sync(&group, None, true);
        assert!(
            matches!(entry, RoundSync::Save(_)),
            "a counted group logs its rounds"
        );
        app.apply(Action::SaveGroup(Box::new(GroupSave {
            group: group.clone(),
            session: None,
            photo: None,
            rounds: entry,
            view: None,
        })));
        let logged = rounds::entry_for(&app.data.groups.round_log, &group.id).cloned();
        assert_eq!(
            logged.as_ref().map(|entry| entry.rounds as usize),
            Some(group.shot_count())
        );

        app.apply(Action::DeleteGroup(group.id.clone()));
        assert!(app.data.groups.groups.iter().all(|g| g.id != group.id));
        assert!(rounds::entry_for(&app.data.groups.round_log, &group.id).is_none());
        let status = app.status.take().expect("Undo is offered");
        assert!(!status.error);
        app.undo_delete(*status.undo.expect("with the group"));
        let back = app.data.groups.groups.iter().find(|g| g.id == group.id);
        assert_eq!(back.map(|g| &g.marks), Some(&group.marks));
        assert_eq!(
            rounds::entry_for(&app.data.groups.round_log, &group.id).map(|e| e.rounds),
            logged.map(|entry| entry.rounds)
        );
        assert!(app.page == Page::Groups, "the group is shown again");
    }

    #[test]
    fn a_failed_group_save_saves_nothing_that_goes_with_it() {
        use crate::groups::rounds;
        let mut app = App::new(Store::open_in_memory().unwrap());
        app.apply(Action::LoadDemo);
        let logged = app.data.groups.round_log.clone();
        let mut group = app.data.groups.groups[0].clone();
        group.id = String::new();
        group.count_rounds = true;
        let entry = rounds::sync(&group, None, true);
        app.apply(Action::SaveGroup(Box::new(GroupSave {
            group,
            session: Some(ballistics_store::Session::new(1_000)),
            photo: None,
            rounds: entry,
            view: None,
        })));
        assert!(app.status.as_ref().is_some_and(|status| status.error));
        assert_eq!(
            app.data.groups.round_log, logged,
            "no orphan round log entry"
        );
        assert_eq!(app.data.groups.sessions.len(), 1, "only the demo's session");
    }

    #[test]
    fn a_photo_keeps_its_record_only_with_a_group_on_it() {
        let context = egui::Context::default();
        let mut app = App::new(Store::open_in_memory().unwrap());
        let photos = app.photo_store(&context);
        let jpeg = b"not really a jpeg".to_vec();
        let id = ballistics_store::photo_id(&jpeg);
        app.apply(Action::KeepPhoto(Box::new(groups_vision::ImportedPhoto {
            id: id.clone(),
            jpeg: jpeg.clone(),
            width_px: 40,
            height_px: 30,
            thumbnail: b"small".to_vec(),
            taken: None,
        })));
        assert!(app.data.groups.photos.is_empty(), "no record while marking");
        assert_eq!(photos.get(&id).take(), Some(Ok(Some(jpeg))));
        assert_eq!(
            photos.get_thumbnail(&id).take(),
            Some(Ok(Some(b"small".to_vec())))
        );

        let mut photo = Photo::new(id.clone(), 40, 30);
        photo.taken_at = Some(1_000);
        app.apply(Action::SaveGroup(Box::new(GroupSave {
            group: Group::on_photo(&id, 1_000),
            session: None,
            photo: Some(photo),
            rounds: RoundSync::Nothing,
            view: None,
        })));
        let kept = app.data.groups.photos.iter().find(|photo| photo.id == id);
        assert_eq!(kept.map(|photo| photo.taken_at), Some(Some(1_000)));

        // A record left without a group goes at the next start.
        let mut orphan = Photo::new(ballistics_store::photo_id(b"other"), 10, 10);
        app.store.save_photo(&mut orphan).unwrap();
        crate::groups::forget_unmarked_photos(&app.store);
        let left: Vec<String> = app
            .store
            .photos()
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(left, [id]);
    }

    #[test]
    fn mobile_errors_stay_visible_after_the_notice_timeout() {
        let context = egui::Context::default();
        theme::apply(&context);
        let mut app = App::new(Store::open_in_memory().unwrap());
        app.status = Some(Status {
            text: "An error that needs attention".into(),
            error: true,
            undo: None,
        });
        for time in [0.0, STATUS_SECONDS + 1.0] {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(390.0, 844.0),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    mobile::apply_style(&context);
                    app.phone_header(ui);
                    app.tab_bar(ui);
                    app.status_bar(ui);
                },
            );
            assert!(app.status.as_ref().is_some_and(|status| status.error));
            output.textures_delta.clear();
        }
    }
}

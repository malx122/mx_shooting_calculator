//! The Groups part: photographed targets, marked shots and their
//! statistics. The library lists the groups; a group's page shows its
//! drawing and numbers; marking (in `marking`) places the shots, and the
//! save sheet says what they were of. Like every page it never writes to
//! the store: it returns `Action`s.

pub mod analyse;
pub mod demo;
pub mod details;
pub mod drawing;
pub mod form;
pub mod import;
pub mod library;
pub mod marking;
pub mod measure;
pub mod rounds;
#[cfg(test)]
mod tests;
pub mod textures;

use std::collections::HashMap;
use std::rc::Rc;

use ballistics_store::model::now_ms;
use ballistics_store::{
    Calibration, Country, Group, GroupView, Marks, Photo, PhotoStore, Session, Store,
};
use eframe::egui::{self, RichText, vec2};
use serde::{Deserialize, Serialize};

use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity};
use details::{NumbersEvent, NumbersState, Subject};
use form::Form;
use import::PhotoImport;
use library::{Filters, Selection};
use marking::{Marking, MarkingContext, MarkingOutcome};
use measure::{MeasureError, Measured};
use textures::Textures;

/// The Groups settings, kept on this device.
pub const SETTINGS: &str = "groups";

/// The size the library ranks groups by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankBy {
    MeanRadius,
    ExtremeSpread,
}

/// Choices for the Groups part, kept on this device only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupSettings {
    /// Keep photos as they come instead of reducing them to 4096 pixels.
    pub keep_original_photos: bool,
    pub rank_by: RankBy,
    /// Whether a new group counts its rounds on the barrel.
    pub count_rounds: bool,
    /// The last blank grid's width and height, mm.
    pub grid_mm: [f64; 2],
    /// The grid square sizes photos were last scaled by.
    pub grid_sizes: marking::GridSizes,
}

impl Default for GroupSettings {
    fn default() -> Self {
        GroupSettings {
            keep_original_photos: false,
            rank_by: RankBy::MeanRadius,
            count_rounds: true,
            grid_mm: [200.0, 200.0],
            grid_sizes: marking::GridSizes::default(),
        }
    }
}

/// `unix_ms` on the device's clock where it says (in the browser), else in
/// the hunting rules' country, else UTC.
pub fn local_ms(unix_ms: i64, country: Option<Country>) -> i64 {
    if let Some(offset) = crate::sensors::utc_offset_minutes(unix_ms) {
        return unix_ms + (offset * 60_000.0) as i64;
    }
    if let Some(country) = country {
        let (date, minutes) = country.hunting().local_now(unix_ms);
        return date.days() * 86_400_000 + (minutes * 60_000.0) as i64;
    }
    unix_ms
}

/// The photo a group is marked on, if this device knows it.
pub fn photo_of<'a>(group: &Group, data: &'a Data) -> Option<&'a Photo> {
    let id = group.photo_id()?;
    data.groups.photos.iter().find(|photo| photo.id == id)
}

/// Measures a saved group with its photo's scale and its bullet.
pub fn measure_group(group: &Group, data: &Data) -> Result<Measured, MeasureError> {
    measure::measure(
        &group.marks,
        photo_of(group, data).and_then(|photo| photo.calibration.as_ref()),
        form::calibre_of(group.bullet_id.as_deref(), data),
        group.distance_m,
    )
}

pub(crate) fn problem_text(error: &MeasureError) -> String {
    match error {
        MeasureError::NoScale => tr("no scale yet").to_owned(),
        MeasureError::Scale(error) => trf!("the scale can't be used: {}", error),
        MeasureError::OffTarget => tr("a mark is off the target").to_owned(),
        MeasureError::Stats(error) => trf!("can't be measured: {}", error),
    }
}

/// Why a group being marked has no numbers yet.
fn waiting_text(error: &MeasureError) -> String {
    match error {
        MeasureError::NoScale => tr("The numbers appear once the photo has a scale.").to_owned(),
        other => problem_text(other),
    }
}

/// A group as the library lists it.
pub struct Summary {
    pub title: String,
    pub subtitle: String,
    pub measured: Option<Rc<Measured>>,
    /// Why it can't be measured.
    pub problem: Option<String>,
}

/// "ELD-M 140 gr · rung 41.2 g", or "Group", with a rung in metric units.
pub fn title_of(group: &Group, data: &Data) -> String {
    title_in(group, data, &units::UnitPrefs::default())
}

/// "ELD-M 140 gr · rung 45.0 gr", or "Group", a rung in the hunter's units.
pub fn title_in(group: &Group, data: &Data, u: &units::UnitPrefs) -> String {
    let bullet = group
        .bullet_id
        .as_ref()
        .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id));
    let mut title = bullet.map_or_else(|| tr("Group").to_owned(), |b| b.display_name());
    if let Some(rung) = group.rung_value {
        let ladder = group
            .ladder_id
            .as_ref()
            .and_then(|id| data.groups.ladders.iter().find(|ladder| &ladder.id == id));
        let value = match ladder {
            Some(ladder) => u.format(analyse::ladder_quantity(ladder.variable), rung),
            None => format!("{rung:.2}"),
        };
        title = trf!("{} · rung {}", title, value);
    }
    title
}

/// "5 shots · 100 m · Tikka T3x".
fn subtitle_of(group: &Group, data: &Data, u: &units::UnitPrefs) -> String {
    let count = group.shot_count();
    let mut parts = vec![if count == 1 {
        tr("1 shot").to_owned()
    } else {
        trf!("{} shots", count)
    }];
    if let Some(distance) = group.distance_m {
        parts.push(u.format(Quantity::Distance, distance));
    }
    if let Some(rifle) = group
        .rifle_id
        .as_ref()
        .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
    {
        parts.push(rifle.name.clone());
    }
    let cold = match &group.marks {
        Marks::Photo(marks) => marks.shots.iter().any(|shot| shot.cold_bore),
        Marks::Grid(marks) => marks.shots.iter().any(|shot| shot.cold_bore),
    };
    if cold {
        parts.push(tr("cold bore").to_owned());
    }
    parts.join(" · ")
}

type Stamp = (i64, i64, u64, u64);

/// The library's measured groups, worked out again only when a group, its
/// photo's scale, its bullet or its distance changes.
#[derive(Default)]
pub struct Summaries(HashMap<String, (Stamp, Result<Rc<Measured>, String>)>);

impl Summaries {
    pub fn of(&mut self, group: &Group, data: &Data, u: &units::UnitPrefs) -> Summary {
        let stamp = (
            group.updated_at,
            photo_of(group, data).map_or(-1, |photo| photo.updated_at),
            form::calibre_of(group.bullet_id.as_deref(), data).map_or(0, f64::to_bits),
            group.distance_m.map_or(0, f64::to_bits),
        );
        let measure = || {
            measure_group(group, data)
                .map(Rc::new)
                .map_err(|error| problem_text(&error))
        };
        let entry = self
            .0
            .entry(group.id.clone())
            .or_insert_with(|| (stamp, measure()));
        if entry.0 != stamp {
            *entry = (stamp, measure());
        }
        Summary {
            title: title_in(group, data, u),
            subtitle: subtitle_of(group, data, u),
            measured: entry.1.as_ref().ok().cloned(),
            problem: entry.1.as_ref().err().cloned(),
        }
    }
}

/// A group to save, with what goes with it. The app saves the group first,
/// and the rest only if the group was saved.
pub struct GroupSave {
    pub group: Group,
    /// A session made for it.
    pub session: Option<Session>,
    /// Its photo's record: new with the first group on the photo, or with a
    /// new scale.
    pub photo: Option<Photo>,
    pub rounds: rounds::RoundSync,
    /// How it was framed while marking, kept on this device.
    pub view: Option<GroupView>,
}

impl GroupSave {
    /// Just the group, its round log entry kept in step.
    fn of(group: Group, data: &Data) -> GroupSave {
        let rounds = rounds_of(&group, data);
        GroupSave {
            group,
            session: None,
            photo: None,
            rounds,
            view: None,
        }
    }
}

/// Forgets the photo records no group is marked on, as when a photo was
/// brought in but no group saved on it; their bytes go with the photo
/// store's sweep.
pub fn forget_unmarked_photos(store: &Store) {
    if let Ok(photos) = store.photos() {
        for photo in photos {
            let _ = store.delete_photo(&photo.id);
        }
    }
}

/// A saved group open on its page.
struct Open {
    id: String,
    /// Its details as edited.
    form: Form,
    /// On a phone: the details sheet is open.
    editing: bool,
    /// The photo is shown full size.
    photo: bool,
    /// The drawing is shown full size.
    enlarged: bool,
}

impl Open {
    fn new(group: &Group) -> Open {
        Open {
            id: group.id.clone(),
            form: Form::of(group),
            editing: false,
            photo: false,
            enlarged: false,
        }
    }
}

/// What a marking session makes.
enum Purpose {
    /// A new group, saved from the save sheet. `photo`: the record of a
    /// photo just brought in, saved with the group. `from`: the group whose
    /// photo it shares, to go back to.
    New {
        group: Box<Group>,
        photo: Option<Box<Photo>>,
        from: Option<String>,
    },
    /// New shots for a saved group, saved when marking is done.
    Edit(String),
}

/// Marking a group, then saying what it was of.
struct Flow {
    marking: Marking,
    purpose: Purpose,
    form: Form,
    /// Marking is done: the save sheet (phone) or pane (desktop) is up.
    saving: bool,
    /// The group handed to the app to save, waiting for it.
    pending: Option<String>,
}

impl Flow {
    fn from(&self) -> Option<&str> {
        match &self.purpose {
            Purpose::New { from, .. } => from.as_deref(),
            Purpose::Edit(id) => Some(id),
        }
    }

    /// The record of the photo being marked: just brought in, or this
    /// device's.
    fn photo<'a>(&'a self, data: &'a Data) -> Option<&'a Photo> {
        let id = match &self.purpose {
            Purpose::New {
                photo: Some(photo), ..
            } => return Some(photo),
            Purpose::New { group, .. } => group.photo_id()?.to_owned(),
            Purpose::Edit(id) => find(data, id)?.photo_id()?.to_owned(),
        };
        data.groups.photos.iter().find(|photo| photo.id == id)
    }
}

enum View {
    Library,
    Group(Box<Open>),
    Marking(Box<Flow>),
    /// Zero, trend, compare, ladder or a composite.
    Analyse,
}

/// The library's sheets.
#[derive(Clone, Copy, PartialEq)]
enum Sheet {
    /// Take or choose a photo, or a blank grid.
    NewGroup,
    /// A blank grid's size.
    Grid,
}

/// Whether this device has a photo's picture.
enum PhotoHere {
    Ready(egui::TextureId, egui::Vec2),
    Loading,
    Missing,
    Failed(String),
}

pub struct GroupsPage {
    view: View,
    pub settings: GroupSettings,
    filters: Filters,
    /// The groups picked in the library.
    selection: Selection,
    analyse: analyse::Analyse,
    sheet: Option<Sheet>,
    import: PhotoImport,
    textures: Textures,
    summaries: Summaries,
    numbers: NumbersState,
    /// Why the last photo couldn't be brought in.
    notice: Option<String>,
    /// A phone sheet was up last frame. A sheet opened by a tap waits a
    /// frame, so the same tap doesn't land on a field under it.
    sheet_up: bool,
    /// A full photo was drawn this frame, so its texture stays.
    photo_shown: bool,
}

impl GroupsPage {
    pub fn new(settings: GroupSettings) -> Self {
        GroupsPage {
            view: View::Library,
            settings,
            filters: Filters::default(),
            selection: Selection::default(),
            analyse: analyse::Analyse::default(),
            sheet: None,
            import: PhotoImport::default(),
            textures: Textures::default(),
            summaries: Summaries::default(),
            numbers: NumbersState::default(),
            notice: None,
            sheet_up: false,
            photo_shown: false,
        }
    }

    /// Views open over the library on a phone, each one a step for Back.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn depth(&self) -> usize {
        let sheet = usize::from(self.sheet.is_some());
        sheet
            + match &self.view {
                View::Library => 0,
                View::Group(open) => {
                    1 + usize::from(open.editing)
                        + usize::from(open.photo)
                        + usize::from(open.enlarged)
                }
                View::Marking(flow) => {
                    usize::from(flow.from().is_some())
                        + flow.marking.depth()
                        + usize::from(flow.saving)
                }
                View::Analyse => 1 + self.analyse.depth(),
            }
    }

    /// Back one view: a sheet, the photo or drawing, the save sheet, a
    /// marking step.
    pub fn back(&mut self, data: &Data) {
        if self.sheet.take().is_some() {
            return;
        }
        match &mut self.view {
            View::Library => {}
            View::Group(open) => {
                if open.photo {
                    open.photo = false;
                } else if open.enlarged {
                    open.enlarged = false;
                } else if open.editing {
                    open.editing = false;
                    if let Some(group) = find(data, &open.id) {
                        open.form = Form::of(group);
                    }
                } else {
                    self.view = View::Library;
                }
            }
            View::Marking(flow) => {
                if flow.saving {
                    flow.saving = false;
                } else if !flow.marking.back() {
                    self.close_marking(data);
                }
            }
            View::Analyse => {
                if !self.analyse.back() {
                    self.view = View::Library;
                }
            }
        }
    }

    /// Whether Back now would drop changes that aren't saved.
    pub fn unsaved(&self, data: &Data) -> bool {
        if self.sheet.is_some() {
            return false;
        }
        match &self.view {
            View::Library => false,
            View::Group(open) => {
                !open.photo
                    && !open.enlarged
                    && open.editing
                    && find(data, &open.id).is_some_and(|group| open.form.changed(group))
            }
            View::Marking(flow) => {
                !flow.saving && flow.marking.depth() <= 1 && flow.marking.unsaved()
            }
            View::Analyse => false,
        }
    }

    /// Whether anything at all isn't saved, for leaving the page.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn has_unsaved(&self, data: &Data) -> bool {
        match &self.view {
            View::Library => false,
            View::Group(open) => find(data, &open.id).is_some_and(|group| open.form.changed(group)),
            View::Marking(flow) => flow.saving || flow.marking.unsaved(),
            View::Analyse => false,
        }
    }

    /// The app saved a group.
    pub fn saved(&mut self, group: &Group) {
        match &mut self.view {
            View::Marking(flow) if flow.pending.as_deref() == Some(group.id.as_str()) => {
                self.view = View::Group(Box::new(Open::new(group)));
                self.textures.release_photo();
            }
            View::Group(open) if open.id == group.id => {
                if matches!(open.form.session, form::SessionChoice::New { .. }) {
                    open.form = Form::of(group);
                }
            }
            _ => {}
        }
    }

    /// The app deleted a group.
    pub fn deleted(&mut self, id: &str) {
        if matches!(&self.view, View::Group(open) if open.id == id) {
            self.view = View::Library;
        }
    }

    /// Opens a group's page.
    pub fn open(&mut self, group: &Group) {
        self.view = View::Group(Box::new(Open::new(group)));
    }

    /// The hunter went to another page: the full photo goes unless it is
    /// being marked.
    pub fn leave(&mut self) {
        if !matches!(self.view, View::Marking(_)) {
            self.textures.release_photo();
        }
    }

    /// Where a photo read in the background lands, to be brought in.
    #[cfg(target_arch = "wasm32")]
    pub fn photo_slot(&self) -> crate::files::Picked {
        self.import.slot()
    }

    /// A file dropped on the window: brought in if it is a photo.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn drop_file(&mut self, file: crate::files::File) -> bool {
        let photo = self.import.dropped(file);
        if photo {
            self.sheet = None;
        }
        photo
    }

    /// Closes marking without saving; a photo just brought in keeps no
    /// record, so its bytes go when the app next starts.
    fn close_marking(&mut self, data: &Data) {
        let View::Marking(flow) = std::mem::replace(&mut self.view, View::Library) else {
            return;
        };
        self.textures.release_photo();
        if let Some(group) = flow.from().and_then(|id| find(data, id)) {
            self.view = View::Group(Box::new(Open::new(group)));
        }
    }

    fn start_marking(&mut self, marking: Marking, purpose: Purpose, form: Form) {
        self.sheet = None;
        self.view = View::Marking(Box::new(Flow {
            marking,
            purpose,
            form,
            saving: false,
            pending: None,
        }));
    }

    fn start_grid(&mut self, data: &Data) {
        let [width, height] = self.settings.grid_mm;
        let group = Group::on_grid(width, height, now_ms());
        self.start_marking(
            Marking::new_grid(width, height),
            Purpose::New {
                group: Box::new(group),
                photo: None,
                from: None,
            },
            Form::for_new(data, self.settings.count_rounds),
        );
    }

    /// A photo is in: keep its bytes and start marking on it. Its record is
    /// saved with the group.
    fn start_photo(
        &mut self,
        context: &egui::Context,
        (imported, pixels): (groups_vision::ImportedPhoto, groups_vision::Thumbnail),
        data: &Data,
        actions: &mut Vec<Action>,
    ) {
        let size_px = [imported.width_px, imported.height_px];
        self.textures.keep_photo(textures::from_pixels(
            context,
            &imported.id,
            &pixels,
            size_px,
            true,
        ));
        let known = data
            .groups
            .photos
            .iter()
            .find(|photo| photo.id == imported.id);
        let offset = crate::sensors::utc_offset_minutes(now_ms()).unwrap_or(0.0) as i32;
        let taken_at = imported.taken.and_then(|taken| taken.unix_ms(offset));
        let record = match known {
            Some(_) => None,
            None => {
                let mut photo = Photo::new(imported.id.clone(), size_px[0], size_px[1]);
                photo.mime = groups_vision::MIME.to_owned();
                photo.taken_at = taken_at;
                Some(Box::new(photo))
            }
        };
        let group = Group::on_photo(&imported.id, taken_at.unwrap_or_else(now_ms));
        let marking = Marking::new_photo(
            imported.id.clone(),
            known.and_then(|photo| photo.calibration),
        )
        .with_grid_sizes(self.settings.grid_sizes);
        actions.push(Action::KeepPhoto(Box::new(imported)));
        self.start_marking(
            marking,
            Purpose::New {
                group: Box::new(group),
                photo: record,
                from: None,
            },
            Form::for_new(data, self.settings.count_rounds),
        );
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Data, photos: &dyn PhotoStore) -> Vec<Action> {
        let context = &ui.ctx().clone();
        let mut actions = Vec::new();
        self.photo_shown = false;
        let long_edge = textures::display_long_edge(context);
        if let Some(result) =
            self.import
                .poll(context, self.settings.keep_original_photos, long_edge)
        {
            match result {
                Ok(imported) => {
                    self.notice = None;
                    self.start_photo(context, imported, data, &mut actions);
                }
                Err(message) => self.notice = Some(message),
            }
        }
        if let View::Group(open) = &self.view
            && find(data, &open.id).is_none()
        {
            self.view = View::Library;
        }
        let phone = mobile::is_narrow(context);
        if self.import.preparing() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.spinner();
                    ui.heading(tr("Preparing photo…"));
                    ui.weak(tr("Turning it upright and taking out its location."));
                });
            });
            return actions;
        }
        if phone {
            self.phone(ui, data, photos, &mut actions);
        } else {
            self.desktop(ui, data, photos, &mut actions);
        }
        self.sheets(context, data);
        if !matches!(&self.view, View::Marking(_)) && !self.photo_shown {
            self.textures.release_photo();
        }
        actions
    }

    fn library(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn PhotoStore,
        phone: bool,
        actions: &mut Vec<Action>,
    ) {
        let palette = theme::palette(ui.ctx());
        if let Some(notice) = &self.notice {
            let mut dismiss = false;
            egui::Frame::new()
                .fill(palette.alarm_tint)
                .corner_radius(8)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.add(
                            egui::Label::new(RichText::new(notice).color(palette.alarm_on_tint))
                                .wrap(),
                        );
                        dismiss = ui.small_button("✖").clicked();
                    });
                });
            if dismiss {
                self.notice = None;
            }
        }
        let selected = match &self.view {
            View::Group(open) => Some(open.id.clone()),
            View::Marking(flow) => flow.from().map(str::to_owned),
            View::Library | View::Analyse => None,
        };
        let marking = matches!(self.view, View::Marking(_));
        let event = library::show(
            ui,
            data,
            &mut self.filters,
            &mut self.settings,
            &mut self.summaries,
            (&mut self.textures, photos),
            &mut self.selection,
            selected.as_deref(),
            !marking,
            phone,
        );
        self.library_event(event, ui.ctx(), data, phone, actions);
    }

    /// The picked groups' bar, above the library's list.
    fn picked_bar(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        phone: bool,
        actions: &mut Vec<Action>,
    ) {
        if matches!(self.view, View::Marking(_)) {
            return;
        }
        let event = library::selection_bar(ui, &mut self.selection, data, phone);
        if event.is_some() {
            self.library_event(event, ui.ctx(), data, phone, actions);
        }
    }

    fn library_event(
        &mut self,
        event: Option<library::Event>,
        context: &egui::Context,
        data: &Data,
        phone: bool,
        actions: &mut Vec<Action>,
    ) {
        match event {
            Some(library::Event::Open(id)) => {
                if let Some(group) = find(data, &id) {
                    self.open(group);
                }
            }
            Some(library::Event::NewGroup) => {
                if phone {
                    self.sheet = Some(Sheet::NewGroup);
                } else {
                    self.import.choose(context, false);
                }
            }
            Some(library::Event::Grid) => self.sheet = Some(Sheet::Grid),
            Some(library::Event::Analyse) => self.view = View::Analyse,
            Some(library::Event::OpenAnalysis(id)) => {
                if let Some(analysis) = data.groups.analyses.iter().find(|a| a.id == id) {
                    self.analyse.open_saved(analysis, data);
                    self.view = View::Analyse;
                }
            }
            Some(library::Event::Compare(ids)) => {
                self.analyse.compare(ids);
                self.view = View::Analyse;
            }
            Some(library::Event::Composite(ids)) => {
                self.analyse.composite(ids);
                self.view = View::Analyse;
            }
            Some(library::Event::Archive(ids, archive)) => {
                for id in ids {
                    if let Some(group) = find(data, &id)
                        && group.archived != archive
                    {
                        let mut group = group.clone();
                        group.archived = archive;
                        actions.push(Action::SaveGroup(Box::new(GroupSave::of(group, data))));
                    }
                }
            }
            Some(library::Event::Delete(ids)) => actions.push(Action::DeleteGroups(ids)),
            None => {}
        }
    }

    fn phone(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn PhotoStore,
        actions: &mut Vec<Action>,
    ) {
        egui::CentralPanel::default().show(ui, |ui| match &mut self.view {
            View::Library => {
                self.picked_bar(ui, data, true, actions);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.library(ui, data, photos, true, actions));
            }
            View::Group(_) => self.group_page(ui, data, photos, actions, true),
            View::Marking(flow) if flow.saving => {
                if mobile::back_button(ui, tr("Shots")) {
                    actions.push(Action::Back);
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.marked(ui, data, true));
            }
            View::Marking(_) => self.mark(ui, data, photos, actions),
            View::Analyse => {
                if self.analyse.depth() == 0 {
                    ui.horizontal(|ui| {
                        if mobile::back_button(ui, tr("Groups")) {
                            actions.push(Action::Back);
                        }
                        ui.heading(tr("Analyse"));
                    });
                }
                self.analyse.tabs(ui, true);
                let rank_by = self.settings.rank_by;
                egui::ScrollArea::vertical()
                    .id_salt(self.analyse.view_key())
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.analyse.show(ui, data, rank_by, true, actions));
            }
        });
        self.save_sheet(ui.ctx(), data, actions);
    }

    fn desktop(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn PhotoStore,
        actions: &mut Vec<Action>,
    ) {
        egui::Panel::left("groups-library")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| {
                let marking = matches!(self.view, View::Marking(_));
                let mut analysing = matches!(self.view, View::Analyse);
                ui.add_space(4.0);
                ui.add_enabled_ui(!marking, |ui| {
                    if analyse::segmented(
                        ui,
                        &mut analysing,
                        &[(false, tr("Library")), (true, tr("Analyse"))],
                        false,
                    ) {
                        self.view = if analysing {
                            View::Analyse
                        } else {
                            View::Library
                        };
                    }
                });
                ui.add_space(4.0);
                if !analysing {
                    self.picked_bar(ui, data, false, actions);
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if analysing {
                            self.analyse.menu(ui, data);
                        } else {
                            self.library(ui, data, photos, false, actions);
                        }
                    });
            });
        if !matches!(self.view, View::Library | View::Analyse) {
            egui::Panel::right("groups-numbers")
                .resizable(true)
                .default_size(380.0)
                .size_range(300.0..=560.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.right_pane(ui, data, actions));
                });
        }
        if matches!(self.view, View::Analyse) {
            let rank_by = self.settings.rank_by;
            egui::CentralPanel::default().show(ui, |ui| {
                ui.scope(|ui| {
                    ui.set_max_width(ui.available_width().min(980.0));
                    self.analyse.tabs(ui, false);
                });
                egui::ScrollArea::vertical()
                    .id_salt(self.analyse.view_key())
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width().min(980.0));
                        self.analyse.show(ui, data, rank_by, false, actions);
                    });
            });
            return;
        }
        egui::CentralPanel::default().show(ui, |ui| match &self.view {
            View::Library | View::Analyse => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.heading(tr("Choose a group, or start a new one"));
                    ui.weak(tr(
                        "New group opens a photo of the target; you can also drop one onto \
                         the window. Grid is for a target you didn't photograph.",
                    ));
                });
            }
            View::Group(_) => self.group_page(ui, data, photos, actions, false),
            View::Marking(flow) if flow.saving => {
                ui.heading(tr("Save the group"));
                self.marked(ui, data, false);
            }
            View::Marking(_) => self.mark(ui, data, photos, actions),
        });
    }

    /// The marking view, and what came of it.
    fn mark(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn PhotoStore,
        actions: &mut Vec<Action>,
    ) {
        let View::Marking(flow) = &mut self.view else {
            return;
        };
        let state = flow
            .photo(data)
            .map(|record| self.textures.photo(ui.ctx(), photos, record, true));
        let image = match &state {
            Some(textures::State::Ready(image)) => Some(*image),
            Some(textures::State::Missing) => {
                ui.colored_label(
                    theme::palette(ui.ctx()).warning,
                    tr("This photo isn't on this device: photos stay on the device they were taken on."),
                );
                None
            }
            Some(textures::State::Failed(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, *error);
                None
            }
            _ => None,
        };
        let marking_context = MarkingContext {
            photo: image,
            calibre_mm: flow.form.calibre_mm(data),
            distance_m: Some(flow.form.distance_m),
        };
        let outcome = flow.marking.show(ui, &marking_context);
        match outcome {
            MarkingOutcome::Working => {}
            MarkingOutcome::Cancelled => self.close_marking(data),
            MarkingOutcome::Finished => self.finish_marking(data, actions),
        }
    }

    fn finish_marking(&mut self, data: &Data, actions: &mut Vec<Action>) {
        let View::Marking(flow) = &mut self.view else {
            return;
        };
        let Purpose::Edit(id) = &flow.purpose else {
            flow.saving = true;
            return;
        };
        let Some(stored) = find(data, id) else {
            self.view = View::Library;
            return;
        };
        let mut group = stored.clone();
        group.marks = flow.marking.marks().clone();
        let mut save = GroupSave::of(group, data);
        save.photo = scale_record(&save.group, flow.marking.calibration(), data, None);
        save.view = flow.marking.view();
        flow.pending = Some(save.group.id.clone());
        actions.push(Action::SaveGroup(Box::new(save)));
    }

    /// Saves the group just marked with the save sheet's details.
    fn save_new(&mut self, data: &Data, actions: &mut Vec<Action>) {
        let View::Marking(flow) = &mut self.view else {
            return;
        };
        let Purpose::New { group, photo, .. } = &flow.purpose else {
            return;
        };
        let mut group = (**group).clone();
        group.marks = flow.marking.marks().clone();
        let session = flow.form.apply(&mut group, data);
        let photo = scale_record(&group, flow.marking.calibration(), data, photo.as_deref());
        let mut save = GroupSave::of(group, data);
        save.session = session;
        save.photo = photo;
        save.view = flow.marking.view();
        self.settings.count_rounds = flow.form.count_rounds;
        self.settings.grid_sizes = flow.marking.grid_sizes();
        flow.pending = Some(save.group.id.clone());
        actions.push(Action::SaveGroup(Box::new(save)));
    }

    /// The group just marked, before it is saved: its drawing and numbers.
    fn marked(&mut self, ui: &mut egui::Ui, data: &Data, phone: bool) {
        let View::Marking(flow) = &self.view else {
            return;
        };
        let calibre = flow.form.calibre_mm(data);
        let distance = Some(flow.form.distance_m);
        let measured = measure::measure(
            flow.marking.marks(),
            flow.marking.calibration(),
            calibre,
            distance,
        );
        match measured {
            Ok(measured) => {
                let side = drawing_side(ui, phone);
                let stats = if self.numbers.with_called_fliers {
                    measured
                        .with_called_fliers
                        .as_ref()
                        .or(measured.stats.as_ref())
                } else {
                    measured.stats.as_ref()
                };
                drawing::group(ui, &measured, stats, calibre, side);
                drawing::legend(ui, &measured, stats.is_some());
                if phone {
                    let rifle = flow
                        .form
                        .rifle_id
                        .as_ref()
                        .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id));
                    let subject = Subject {
                        measured: &measured,
                        rifle,
                        calibre_mm: calibre,
                        distance_m: distance,
                        velocities: velocities(flow.marking.marks(), &[]),
                    };
                    ui.add_space(6.0);
                    let _ = details::show(ui, &subject, &mut self.numbers);
                }
            }
            Err(error) => {
                ui.weak(waiting_text(&error));
            }
        }
    }

    /// The phone's sheets over a group: its details, or the save sheet
    /// after marking. Done saves.
    fn save_sheet(&mut self, context: &egui::Context, data: &Data, actions: &mut Vec<Action>) {
        let wanted = match &self.view {
            View::Group(open) => open.editing,
            View::Marking(flow) => flow.saving && flow.pending.is_none(),
            View::Library | View::Analyse => false,
        };
        let waited = std::mem::replace(&mut self.sheet_up, wanted);
        if wanted && !waited {
            context.request_repaint();
            return;
        }
        self.draw_sheet(context, data, actions);
    }

    fn draw_sheet(&mut self, context: &egui::Context, data: &Data, actions: &mut Vec<Action>) {
        let View::Marking(flow) = &mut self.view else {
            if let View::Group(open) = &mut self.view
                && open.editing
            {
                let Some(stored) = find(data, &open.id) else {
                    return;
                };
                let shots = stored.shot_count();
                let shown = mobile::sheet(context, "group-details", tr("Details"), |ui| {
                    form::show(ui, &mut open.form, data, shots);
                });
                if shown.closed {
                    open.editing = false;
                    if open.form.changed(stored) {
                        save_details(open, stored, data, actions);
                    }
                }
            }
            return;
        };
        if !flow.saving || flow.pending.is_some() {
            return;
        }
        let shots = shot_count(flow.marking.marks());
        let shown = mobile::sheet(context, "group-save", tr("Save group"), |ui| {
            if shots == 0 {
                ui.label(tr("Mark at least one shot first."));
            }
            form::show(ui, &mut flow.form, data, shots);
        });
        if shown.closed {
            if shots == 0 {
                flow.saving = false;
            } else {
                self.save_new(data, actions);
            }
        }
    }

    /// The desktop's right pane: the numbers, and the details form.
    fn right_pane(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        match &mut self.view {
            View::Library | View::Analyse => {}
            View::Group(open) => {
                let Some(stored) = find(data, &open.id) else {
                    return;
                };
                match measure_group(stored, data) {
                    Ok(measured) => {
                        let subject = subject_of(stored, &measured, data);
                        if let Some(NumbersEvent::CallFlier(shot)) =
                            details::show(ui, &subject, &mut self.numbers)
                        {
                            let group = called_flier(stored, shot);
                            actions.push(Action::SaveGroup(Box::new(GroupSave::of(group, data))));
                        }
                    }
                    Err(error) => {
                        ui.weak(waiting_text(&error));
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                ui.label(RichText::new(tr("Details")).strong());
                let shots = stored.shot_count();
                form::show(ui, &mut open.form, data, shots);
                ui.horizontal(|ui| {
                    let changed = open.form.changed(stored);
                    if ui
                        .add_enabled(changed, egui::Button::new(tr("Save")))
                        .clicked()
                    {
                        save_details(open, stored, data, actions);
                    }
                    if changed {
                        if ui.button(tr("Undo changes")).clicked() {
                            open.form = Form::of(stored);
                        }
                        ui.weak(tr("Not saved yet"));
                    }
                });
                ui.add_space(12.0);
                manage(ui, stored, data, actions);
            }
            View::Marking(flow) => {
                if flow.saving {
                    let shots = shot_count(flow.marking.marks());
                    ui.label(RichText::new(tr("What was shot")).strong());
                    form::show(ui, &mut flow.form, data, shots);
                    let mut save = false;
                    ui.horizontal(|ui| {
                        let palette = theme::palette(ui.ctx());
                        save = ui
                            .add_enabled(
                                shots > 0 && flow.pending.is_none(),
                                egui::Button::new(
                                    RichText::new(tr("Save group")).color(palette.on_accent),
                                )
                                .fill(palette.accent),
                            )
                            .clicked();
                        if ui.button(tr("‹ Back to the shots")).clicked() {
                            flow.saving = false;
                        }
                    });
                    if shots == 0 {
                        ui.weak(tr("Mark at least one shot first."));
                    }
                    ui.add_space(10.0);
                    ui.separator();
                    if save {
                        self.save_new(data, actions);
                        return;
                    }
                }
                let View::Marking(flow) = &self.view else {
                    return;
                };
                let calibre = flow.form.calibre_mm(data);
                let distance = Some(flow.form.distance_m);
                match measure::measure(
                    flow.marking.marks(),
                    flow.marking.calibration(),
                    calibre,
                    distance,
                ) {
                    Ok(measured) => {
                        let rifle = flow
                            .form
                            .rifle_id
                            .as_ref()
                            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id));
                        let subject = Subject {
                            measured: &measured,
                            rifle,
                            calibre_mm: calibre,
                            distance_m: distance,
                            velocities: velocities(flow.marking.marks(), &[]),
                        };
                        let _ = details::show(ui, &subject, &mut self.numbers);
                    }
                    Err(error) => {
                        ui.weak(waiting_text(&error));
                    }
                }
            }
        }
    }

    /// A saved group's page. On a phone: the drawing beside its size, then
    /// where it landed, the rest of the numbers and the details; on a
    /// computer: the drawing, with the numbers in the right pane.
    fn group_page(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn PhotoStore,
        actions: &mut Vec<Action>,
        phone: bool,
    ) {
        let View::Group(open) = &mut self.view else {
            return;
        };
        let Some(stored) = find(data, &open.id) else {
            return;
        };
        let photo = photo_of(stored, data);
        let here = photo.map(
            |photo| match self.textures.thumbnail(ui.ctx(), photos, photo) {
                textures::State::Ready(image) => {
                    PhotoHere::Ready(image.texture.id(), image.texture.size_vec2())
                }
                textures::State::Loading => PhotoHere::Loading,
                textures::State::Missing => PhotoHere::Missing,
                textures::State::Failed(error) => PhotoHere::Failed(error.to_owned()),
            },
        );
        let measured = measure_group(stored, data);
        let calibre = form::calibre_of(stored.bullet_id.as_deref(), data);
        let stats = |measured: &Measured, with_fliers: bool| {
            if with_fliers {
                measured.with_called_fliers.or(measured.stats)
            } else {
                measured.stats
            }
        };

        if open.photo || open.enlarged {
            let back = if open.photo {
                tr("‹ Back to the drawing")
            } else {
                tr("‹ Back")
            };
            let pressed = if phone {
                mobile::back_button(ui, tr("Group"))
            } else {
                ui.button(back).clicked()
            };
            if pressed {
                if phone {
                    actions.push(Action::Back);
                } else {
                    open.photo = false;
                    open.enlarged = false;
                }
                return;
            }
            if open.photo {
                if let Some(photo) = photo {
                    photo_view(ui, stored, &mut self.textures, photos, photo);
                    self.photo_shown = true;
                }
            } else if let Ok(measured) = &measured {
                let shown = stats(measured, self.numbers.with_called_fliers);
                let side = ui.available_width().min(ui.available_height() - 80.0);
                drawing::group(ui, measured, shown.as_ref(), calibre, side);
                drawing::legend(ui, measured, shown.is_some());
            }
            return;
        }
        if phone && mobile::back_button(ui, tr("Groups")) {
            actions.push(Action::Back);
            return;
        }

        let missing = matches!(here, Some(PhotoHere::Missing));
        let no_scale = matches!(measured, Err(MeasureError::NoScale));
        let mut start = None;
        let mut header = |ui: &mut egui::Ui, open: &mut Open| {
            ui.horizontal_top(|ui| {
                if let Some(PhotoHere::Ready(texture, size)) = &here {
                    let side = if phone { 56.0 } else { 64.0 };
                    let picture = egui::Image::new((*texture, *size))
                        .fit_to_exact_size(*size * (side / size.x.max(size.y)));
                    if ui
                        .add(egui::Button::image(picture))
                        .on_hover_text(tr("Show the photo"))
                        .clicked()
                    {
                        open.photo = true;
                    }
                }
                ui.vertical(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(title_in(stored, data, &units::prefs(ui.ctx())))
                                .heading(),
                        )
                        .wrap(),
                    );
                    ui.weak(subtitle_of(stored, data, &units::prefs(ui.ctx())));
                });
            });
            ui.horizontal_wrapped(|ui| {
                let edit = if no_scale {
                    tr("Set the scale")
                } else {
                    tr("Edit shots")
                };
                if ui.add_enabled(!missing, egui::Button::new(edit)).clicked() {
                    start = Some(false);
                }
                if photo.is_some() {
                    if ui
                        .add_enabled(
                            !missing,
                            egui::Button::new(tr("Mark another group on this photo")),
                        )
                        .clicked()
                    {
                        start = Some(true);
                    }
                    let ready = matches!(here, Some(PhotoHere::Ready(..)));
                    if ui
                        .add_enabled(ready, egui::Button::new(tr("Show the photo")))
                        .clicked()
                    {
                        open.photo = true;
                    }
                }
            });
            match &here {
                Some(PhotoHere::Missing) => {
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr(
                                "The photo is on another device: photos stay on the device they \
                                 were brought in on, so its shots can't be changed here.",
                            ))
                            .color(theme::palette(ui.ctx()).warning),
                        )
                        .wrap(),
                    );
                }
                Some(PhotoHere::Failed(error)) => {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                _ => {}
            }
        };

        if phone {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    header(ui, open);
                    ui.add_space(4.0);
                    match &measured {
                        Ok(measured) => {
                            let shown = stats(measured, self.numbers.with_called_fliers);
                            let subject = subject_of(stored, measured, data);
                            let side = (ui.available_width() * 0.44).clamp(140.0, 180.0);
                            ui.horizontal_top(|ui| {
                                let drawing =
                                    drawing::group(ui, measured, shown.as_ref(), calibre, side);
                                if drawing.on_hover_cursor(egui::CursorIcon::ZoomIn).clicked() {
                                    open.enlarged = true;
                                }
                                ui.vertical(|ui| {
                                    if let Some(stats) = &shown {
                                        details::heading(ui, stats, subject.distance_m);
                                        let width = ui.available_width();
                                        details::size_tiles(ui, &subject, stats, width, true);
                                    } else {
                                        ui.weak(tr(
                                            "No shot counts yet: cold-bore shots and called \
                                             fliers are left out.",
                                        ));
                                    }
                                });
                            });
                            if let Some(stats) = &shown {
                                details::rough(ui, stats);
                                let width = ui.available_width();
                                details::aim_tiles(ui, &subject, stats, width);
                                details::fliers_switch(ui, measured, &mut self.numbers);
                                if let Some(NumbersEvent::CallFlier(shot)) =
                                    details::more(ui, &subject, stats, &mut self.numbers)
                                {
                                    let group = called_flier(stored, shot);
                                    actions.push(Action::SaveGroup(Box::new(GroupSave::of(
                                        group, data,
                                    ))));
                                }
                            }
                            ui.add_space(6.0);
                            drawing::legend(ui, measured, shown.is_some());
                        }
                        Err(error) => {
                            ui.weak(waiting_text(error));
                        }
                    }
                    ui.add_space(8.0);
                    let summary = details_summary(stored, data);
                    if mobile::list_row(ui, tr("Details"), &summary, false).clicked() {
                        open.editing = true;
                    }
                    ui.add_space(12.0);
                    manage(ui, stored, data, actions);
                });
        } else {
            header(ui, open);
            ui.add_space(4.0);
            match &measured {
                Ok(measured) => {
                    let shown = stats(measured, self.numbers.with_called_fliers);
                    let side = drawing_side(ui, false);
                    drawing::group(ui, measured, shown.as_ref(), calibre, side);
                    ui.allocate_ui(vec2(side, 60.0), |ui| {
                        drawing::legend(ui, measured, shown.is_some());
                    });
                }
                Err(MeasureError::NoScale) if photo.is_some() && !missing => {
                    if let Some(photo) = photo {
                        photo_view(ui, stored, &mut self.textures, photos, photo);
                        self.photo_shown = true;
                    }
                }
                Err(error) => {
                    ui.weak(waiting_text(error));
                }
            }
        }
        match start {
            Some(false) => {
                let calibration = photo.and_then(|photo| photo.calibration);
                let marking = Marking::edit(
                    stored,
                    calibration.as_ref(),
                    data.group_views.get(&stored.id).copied(),
                );
                let form = Form::of(stored);
                self.start_marking(marking, Purpose::Edit(stored.id.clone()), form);
            }
            Some(true) => {
                if let Some(photo) = photo {
                    let taken_at = photo.taken_at.unwrap_or(stored.taken_at);
                    let group = Group::on_photo(&photo.id, taken_at);
                    let form = Form::like(stored);
                    self.start_marking(
                        Marking::new_photo(photo.id.clone(), photo.calibration),
                        Purpose::New {
                            group: Box::new(group),
                            photo: None,
                            from: Some(stored.id.clone()),
                        },
                        form,
                    );
                }
            }
            None => {}
        }
    }

    /// The library's sheets: a new group's photo, and a grid's size.
    fn sheets(&mut self, context: &egui::Context, data: &Data) {
        match self.sheet {
            None => {}
            Some(Sheet::NewGroup) => {
                let mut choice = None;
                let shown = mobile::sheet(context, "groups-new", tr("New group"), |ui| {
                    if mobile::list_row(
                        ui,
                        tr("Take photo"),
                        tr("Of the target, square on, the whole sheet in view"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(0);
                    }
                    if mobile::list_row(ui, tr("Choose photo"), tr("From the photo library"), false)
                        .clicked()
                    {
                        choice = Some(1);
                    }
                    if mobile::list_row(
                        ui,
                        tr("Blank grid"),
                        tr("For a target you didn't photograph"),
                        false,
                    )
                    .clicked()
                    {
                        choice = Some(2);
                    }
                    ui.add_space(4.0);
                    ui.weak(tr(
                        "Photos stay on this device. Their location is taken out as they come \
                         in.",
                    ));
                });
                match choice {
                    Some(0) | Some(1) => {
                        self.sheet = None;
                        self.import.choose(context, choice == Some(0));
                    }
                    Some(_) => self.sheet = Some(Sheet::Grid),
                    None if shown.closed => self.sheet = None,
                    None => {}
                }
            }
            Some(Sheet::Grid) => {
                let mut start = false;
                let size = &mut self.settings.grid_mm;
                let shown = mobile::sheet(context, "groups-grid", tr("Blank grid"), |ui| {
                    ui.label(tr("The grid is drawn at true scale. Its size:"));
                    ui.horizontal_wrapped(|ui| {
                        for (label, [width, height]) in [
                            ("100 × 100 mm", [100.0, 100.0]),
                            ("200 × 200 mm", [200.0, 200.0]),
                            ("300 × 300 mm", [300.0, 300.0]),
                            ("A4 210 × 297 mm", [210.0, 297.0]),
                            ("500 × 500 mm", [500.0, 500.0]),
                        ] {
                            ui.selectable_value(size, [width, height], label);
                        }
                    });
                    crate::widgets::grid(ui, "grid-size", |ui| {
                        units::quantity(
                            ui,
                            tr("Width"),
                            &mut size[0],
                            Quantity::GroupSize,
                            10.0..=2000.0,
                            1.0,
                        );
                        units::quantity(
                            ui,
                            tr("Height"),
                            &mut size[1],
                            Quantity::GroupSize,
                            10.0..=2000.0,
                            1.0,
                        );
                    });
                    ui.add_space(6.0);
                    let palette = theme::palette(ui.ctx());
                    start = ui
                        .add(
                            egui::Button::new(
                                RichText::new(tr("Start marking")).color(palette.on_accent),
                            )
                            .fill(palette.accent)
                            .min_size(vec2(160.0, mobile::TOUCH)),
                        )
                        .clicked();
                });
                if start {
                    self.start_grid(data);
                } else if shown.closed {
                    self.sheet = None;
                }
            }
        }
    }
}

fn find<'a>(data: &'a Data, id: &str) -> Option<&'a Group> {
    data.groups.groups.iter().find(|group| group.id == id)
}

fn shot_count(marks: &Marks) -> usize {
    match marks {
        Marks::Photo(marks) => marks.shots.len(),
        Marks::Grid(marks) => marks.shots.len(),
    }
}

/// Every chronograph reading: the marked shots', then the others.
pub(crate) fn velocities(marks: &Marks, extra: &[f64]) -> Vec<f64> {
    let marked: Vec<f64> = match marks {
        Marks::Photo(marks) => marks.shots.iter().filter_map(|s| s.velocity_mps).collect(),
        Marks::Grid(marks) => marks.shots.iter().filter_map(|s| s.velocity_mps).collect(),
    };
    marked.into_iter().chain(extra.iter().copied()).collect()
}

fn subject_of<'a>(group: &Group, measured: &'a Measured, data: &'a Data) -> Subject<'a> {
    Subject {
        measured,
        rifle: group
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id)),
        calibre_mm: form::calibre_of(group.bullet_id.as_deref(), data),
        distance_m: group.distance_m,
        velocities: velocities(&group.marks, &group.extra_velocities_mps),
    }
}

/// The group with `shot` called a flier.
fn called_flier(group: &Group, shot: usize) -> Group {
    let mut group = group.clone();
    match &mut group.marks {
        Marks::Photo(marks) => {
            if let Some(shot) = marks.shots.get_mut(shot) {
                shot.called_flier = true;
            }
        }
        Marks::Grid(marks) => {
            if let Some(shot) = marks.shots.get_mut(shot) {
                shot.called_flier = true;
            }
        }
    }
    group
}

/// The photo's record to save with a group: a new photo's, or this
/// device's when marking changed its scale.
fn scale_record(
    group: &Group,
    calibration: Option<&Calibration>,
    data: &Data,
    new: Option<&Photo>,
) -> Option<Photo> {
    if let Some(new) = new {
        let mut photo = new.clone();
        photo.calibration = calibration.copied();
        return Some(photo);
    }
    let photo = photo_of(group, data)?;
    (photo.calibration.as_ref() != calibration).then(|| {
        let mut photo = photo.clone();
        photo.calibration = calibration.copied();
        photo
    })
}

/// The change that keeps the group's round log entry in step with it.
fn rounds_of(group: &Group, data: &Data) -> rounds::RoundSync {
    let rifle_known = group
        .rifle_id
        .as_ref()
        .is_some_and(|id| data.rifles.iter().any(|rifle| &rifle.id == id));
    rounds::sync(
        group,
        rounds::entry_for(&data.groups.round_log, &group.id),
        rifle_known,
    )
}

fn save_details(open: &Open, stored: &Group, data: &Data, actions: &mut Vec<Action>) {
    let mut group = stored.clone();
    let session = open.form.apply(&mut group, data);
    let mut save = GroupSave::of(group, data);
    save.session = session;
    actions.push(Action::SaveGroup(Box::new(save)));
}

/// "Browning X-Bolt · Oryx · Luleå range · practice".
fn details_summary(group: &Group, data: &Data) -> String {
    let mut parts = Vec::new();
    if let Some(rifle) = group
        .rifle_id
        .as_ref()
        .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
    {
        parts.push(rifle.name.clone());
    }
    if let Some(session) = group.session_id.as_ref().and_then(|id| {
        data.groups
            .sessions
            .iter()
            .find(|session| &session.id == id)
    }) && !session.place.is_empty()
    {
        parts.push(session.place.clone());
    }
    parts.extend(group.tags.iter().cloned());
    if parts.is_empty() {
        tr("Rifle, bullet, distance, session, tags").to_owned()
    } else {
        parts.join(" · ")
    }
}

/// Archive, and apart from it, Delete (which offers Undo).
fn manage(ui: &mut egui::Ui, group: &Group, data: &Data, actions: &mut Vec<Action>) {
    let palette = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        let archive = if group.archived {
            tr("Bring back from the archive")
        } else {
            tr("Archive")
        };
        if ui
            .button(archive)
            .on_hover_text(tr(
                "Archived groups are left out of the library until you show them",
            ))
            .clicked()
        {
            let mut group = group.clone();
            group.archived = !group.archived;
            actions.push(Action::SaveGroup(Box::new(GroupSave::of(group, data))));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let delete = egui::Button::new(RichText::new(tr("Delete")).color(palette.alarm))
                .stroke(egui::Stroke::new(1.0_f32, palette.alarm));
            if ui.add(delete).clicked() {
                actions.push(Action::DeleteGroup(group.id.clone()));
            }
        });
    });
}

/// The drawing's side: as wide as the page allows, and on a computer short
/// enough to leave room for the title and the legend.
fn drawing_side(ui: &egui::Ui, phone: bool) -> f32 {
    let width = ui.available_width();
    if phone {
        width.min(460.0)
    } else {
        width.min(ui.available_height() - 110.0).clamp(220.0, 720.0)
    }
}

/// The photo at full size with the shots ringed where they were marked.
fn photo_view(
    ui: &mut egui::Ui,
    group: &Group,
    textures: &mut Textures,
    photos: &dyn PhotoStore,
    photo: &Photo,
) {
    match textures.photo(ui.ctx(), photos, photo, false) {
        textures::State::Ready(image) => {
            let size = egui::vec2(image.size_px[0] as f32, image.size_px[1] as f32);
            let room = ui.available_size();
            let scale = (room.x / size.x).min(room.y / size.y).min(4.0);
            let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
            ui.painter().image(
                image.texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            if let Marks::Photo(marks) = &group.marks {
                let palette = theme::palette(ui.ctx());
                let at = |[x, y]: [f64; 2]| {
                    rect.left_top() + egui::vec2(x as f32 * scale, y as f32 * scale)
                };
                for shot in &marks.shots {
                    ui.painter().circle_stroke(
                        at([shot.x_px, shot.y_px]),
                        9.0,
                        egui::Stroke::new(2.0_f32, palette.accent),
                    );
                }
                if let Some(aim) = marks.aim_px {
                    let centre = at(aim);
                    let stroke = egui::Stroke::new(2.0_f32, palette.alarm);
                    ui.painter()
                        .line_segment([centre - vec2(10.0, 0.0), centre + vec2(10.0, 0.0)], stroke);
                    ui.painter()
                        .line_segment([centre - vec2(0.0, 10.0), centre + vec2(0.0, 10.0)], stroke);
                }
            }
        }
        textures::State::Loading => {
            ui.spinner();
        }
        textures::State::Missing => {
            ui.weak(tr(
                "This photo isn't on this device: photos stay on the device they were taken on.",
            ));
        }
        textures::State::Failed(error) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}

/// Shows a hint while a file is dragged over the window.
pub fn drop_hint(context: &egui::Context) {
    if context.input(|input| input.raw.hovered_files.is_empty()) {
        return;
    }
    let palette = theme::palette(context);
    let screen = context.content_rect();
    egui::Area::new(egui::Id::unique("groups-drop-hint"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(false)
        .show(context, |ui| {
            ui.painter()
                .rect_filled(screen, 0.0, palette.background.gamma_multiply(0.85));
            ui.painter().text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                tr("Drop the photo to start a group"),
                egui::FontId::proportional(24.0),
                palette.accent,
            );
        });
}

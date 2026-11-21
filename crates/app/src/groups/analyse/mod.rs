//! Analyse: several groups at once. Zero checks whether the zero holds,
//! Trend follows a size or velocity over time and load, Compare sets
//! groups side by side and tests two against each other, Ladder reads a
//! load development, and a composite pools groups into one. What counts
//! is worked out in `pool`; these pages draw it and, like every page,
//! return `Action`s instead of writing to the store.

mod compare;
mod composite;
mod ladder;
pub use ladder::{name as ladder_name, quantity as ladder_quantity};
pub mod pool;
#[cfg(test)]
mod tests;
mod trend;
mod zero;

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use ballistics_store::{Analysis, AnalysisKind, Rifle};
use eframe::egui::{self, Color32, RichText, vec2};
use groups_core::Point;

use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::theme;
use crate::units::{Quantity, UnitPrefs};

/// The analyses a hunter switches between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Zero,
    Trend,
    Compare,
    Ladder,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Zero, Tab::Trend, Tab::Compare, Tab::Ladder];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Zero => tr("Zero"),
            Tab::Trend => tr("Trend"),
            Tab::Compare => tr("Compare"),
            Tab::Ladder => tr("Ladder"),
        }
    }
}

/// The Analyse pages' state, kept while the hunter looks elsewhere.
#[derive(Default)]
pub struct Analyse {
    pub tab: Tab,
    pub compare: compare::State,
    /// A composite shown over the tab.
    pub composite: Option<composite::State>,
    pub zero: zero::State,
    pub trend: trend::State,
    pub ladder: ladder::State,
}

impl Analyse {
    /// Views open over the tab, each one a step for Back.
    pub fn depth(&self) -> usize {
        usize::from(self.composite.is_some())
    }

    /// What is shown, for keeping each view's scroll position apart.
    pub fn view_key(&self) -> (&'static str, Tab, bool) {
        ("analyse", self.tab, self.composite.is_some())
    }

    /// Closes the view on top; false when there was none.
    pub fn back(&mut self) -> bool {
        self.composite.take().is_some()
    }

    /// Compares the groups of `ids`.
    pub fn compare(&mut self, ids: Vec<String>) {
        self.tab = Tab::Compare;
        self.compare = compare::State::of(ids, None);
        self.composite = None;
    }

    /// Pools the groups of `ids` into a composite.
    pub fn composite(&mut self, ids: Vec<String>) {
        self.composite = Some(composite::State::of(ids, None, None, None));
    }

    /// Opens a saved analysis, rebuilt from the groups there are now.
    pub fn open_saved(&mut self, analysis: &Analysis, data: &Data) {
        let rebuilt = pool::rebuild(analysis, data);
        match analysis.kind {
            AnalysisKind::Compare => {
                self.tab = Tab::Compare;
                self.compare = compare::State::of(rebuilt.ids, Some(analysis.id.clone()));
                self.compare.missing = rebuilt.missing;
                self.composite = None;
            }
            AnalysisKind::Composite => {
                let mut state = composite::State::of(
                    rebuilt.ids,
                    Some(analysis.id.clone()),
                    rebuilt.mode,
                    rebuilt.distance_m,
                );
                state.missing = rebuilt.missing;
                self.composite = Some(state);
            }
            AnalysisKind::Ladder => {
                self.tab = Tab::Ladder;
                self.composite = None;
                let ladder = rebuilt.ids.iter().find_map(|id| {
                    data.groups
                        .groups
                        .iter()
                        .find(|group| &group.id == id)?
                        .ladder_id
                        .clone()
                });
                if let Some(ladder) = ladder {
                    self.ladder.open = Some(ladder);
                }
            }
        }
    }

    /// The page: the switch between the analyses, and the one chosen (or
    /// a composite over it). `rank_by` is the library's choice.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        rank_by: super::RankBy,
        phone: bool,
        actions: &mut Vec<Action>,
    ) {
        let stamp = stamp(data);
        if let Some(state) = &mut self.composite {
            let open = composite::show(ui, state, data, stamp, phone, actions);
            match open {
                composite::Outcome::Stay => {}
                composite::Outcome::Close => {
                    if phone {
                        actions.push(Action::Back);
                    } else {
                        self.composite = None;
                    }
                }
            }
            return;
        }
        match self.tab {
            Tab::Zero => zero::show(ui, &mut self.zero, data, stamp, phone, actions),
            Tab::Trend => trend::show(ui, &mut self.trend, data, stamp, phone, actions),
            Tab::Compare => {
                match compare::show(ui, &mut self.compare, data, stamp, rank_by, phone, actions) {
                    Some(compare::Next::Composite(ids)) => {
                        self.composite = Some(composite::State::of(ids, None, None, None));
                    }
                    Some(compare::Next::Open(id)) => {
                        if let Some(analysis) = data
                            .groups
                            .analyses
                            .iter()
                            .find(|analysis| analysis.id == id)
                        {
                            self.open_saved(&analysis.clone(), data);
                        }
                    }
                    None => {}
                }
            }
            Tab::Ladder => {
                if let Some(ids) = ladder::show(ui, &mut self.ladder, data, stamp, phone, actions) {
                    self.composite = Some(composite::State::of(
                        ids,
                        None,
                        Some(groups_core::CompositeMode::AboutOwnCentres),
                        None,
                    ));
                }
            }
        }
    }

    /// The switch between the analyses, above the page; none over a
    /// composite.
    pub fn tabs(&mut self, ui: &mut egui::Ui, phone: bool) {
        if self.composite.is_none() {
            segmented(
                ui,
                &mut self.tab,
                &Tab::ALL.map(|tab| (tab, tab.name())),
                phone,
            );
            ui.add_space(6.0);
        }
    }

    /// The desktop's left pane in Analyse: the saved analyses and the
    /// ladders, to open.
    pub fn menu(&mut self, ui: &mut egui::Ui, data: &Data) {
        let palette = theme::palette(ui.ctx());
        ui.label(
            RichText::new(tr("Saved analyses"))
                .strong()
                .color(palette.text_muted),
        );
        if data.groups.analyses.is_empty() {
            ui.weak(tr(
                "None yet. Select groups in the library and compare or pool them, then save.",
            ));
        }
        let mut open = None;
        for analysis in &data.groups.analyses {
            let selected = match analysis.kind {
                AnalysisKind::Composite => self
                    .composite
                    .as_ref()
                    .is_some_and(|state| state.saved.as_ref() == Some(&analysis.id)),
                AnalysisKind::Compare => {
                    self.composite.is_none()
                        && self.tab == Tab::Compare
                        && self.compare.saved.as_ref() == Some(&analysis.id)
                }
                AnalysisKind::Ladder => false,
            };
            let detail = analysis_detail(analysis, data);
            if mobile::list_row(ui, &analysis_name(analysis), &detail, selected).clicked() {
                open = Some(analysis.clone());
            }
        }
        if let Some(analysis) = open {
            self.open_saved(&analysis, data);
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(tr("Ladders"))
                .strong()
                .color(palette.text_muted),
        );
        if data.groups.ladders.is_empty() {
            ui.weak(tr("None yet: start one on the Ladder tab."));
        }
        for ladder in &data.groups.ladders {
            let selected = self.composite.is_none()
                && self.tab == Tab::Ladder
                && self.ladder.open.as_ref() == Some(&ladder.id);
            let rungs = data
                .groups
                .groups
                .iter()
                .filter(|group| group.ladder_id.as_ref() == Some(&ladder.id))
                .count();
            let detail = if rungs == 1 {
                tr("1 group").to_owned()
            } else {
                trf!("{} groups", rungs)
            };
            if mobile::list_row(ui, &ladder::name(ladder, data), &detail, selected).clicked() {
                self.tab = Tab::Ladder;
                self.composite = None;
                self.ladder.open = Some(ladder.id.clone());
            }
        }
    }
}

/// A saved analysis's name, or what it is.
pub fn analysis_name(analysis: &Analysis) -> String {
    if !analysis.name.trim().is_empty() {
        return analysis.name.clone();
    }
    match analysis.kind {
        AnalysisKind::Compare => tr("Comparison").to_owned(),
        AnalysisKind::Composite => tr("Composite").to_owned(),
        AnalysisKind::Ladder => tr("Ladder").to_owned(),
    }
}

/// "Composite of 3 groups", "Comparison of 2 groups".
pub fn analysis_detail(analysis: &Analysis, data: &Data) -> String {
    let have = pool::groups_of(data, &analysis.group_ids).len();
    match analysis.kind {
        AnalysisKind::Compare => trf!("comparison of {} groups", have),
        AnalysisKind::Composite => trf!("composite of {} groups", have),
        AnalysisKind::Ladder => trf!("ladder of {} groups", have),
    }
}

/// A key that changes whenever a record the analyses read changes.
pub fn stamp(data: &Data) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    let groups = &data.groups;
    for group in &groups.groups {
        (&group.id, group.updated_at).hash(&mut hasher);
    }
    for photo in &groups.photos {
        (&photo.id, photo.updated_at).hash(&mut hasher);
    }
    for bullet in &data.bullets {
        (&bullet.id, bullet.updated_at).hash(&mut hasher);
    }
    for rifle in &data.rifles {
        (&rifle.id, rifle.updated_at).hash(&mut hasher);
    }
    for load in &data.loads {
        (&load.id, load.updated_at).hash(&mut hasher);
    }
    for session in &groups.sessions {
        (&session.id, session.updated_at).hash(&mut hasher);
    }
    for ladder in &groups.ladders {
        (&ladder.id, ladder.updated_at).hash(&mut hasher);
    }
    for reset in &groups.zero_resets {
        (&reset.id, reset.updated_at).hash(&mut hasher);
    }
    for barrel in &groups.barrels {
        (&barrel.id, barrel.updated_at).hash(&mut hasher);
    }
    for entry in &groups.round_log {
        (&entry.id, entry.updated_at).hash(&mut hasher);
    }
    for clean in &groups.cleans {
        (&clean.id, clean.updated_at).hash(&mut hasher);
    }
    hasher.finish()
}

/// The last result worked out, kept until its key changes.
pub struct Memo<T> {
    key: Option<u64>,
    value: Option<Rc<T>>,
}

impl<T> Default for Memo<T> {
    fn default() -> Self {
        Memo {
            key: None,
            value: None,
        }
    }
}

impl<T> Memo<T> {
    pub fn get(&mut self, key: u64, make: impl FnOnce() -> T) -> Rc<T> {
        match &self.value {
            Some(value) if self.key == Some(key) => value.clone(),
            _ => {
                let value = Rc::new(make());
                self.key = Some(key);
                self.value = Some(value.clone());
                value
            }
        }
    }
}

/// `stamp` and whatever else a result depends on, as one key.
pub fn key_of(stamp: u64, settings: impl Hash) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    (stamp, settings).hash(&mut hasher);
    hasher.finish()
}

/// A row of choices drawn as one control, the chosen one filled.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    choices: &[(T, &str)],
    phone: bool,
) -> bool {
    let palette = theme::palette(ui.ctx());
    let mut changed = false;
    let height = if phone { mobile::TOUCH - 4.0 } else { 28.0 };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let width =
            (ui.available_width() - 2.0 * (choices.len() as f32 - 1.0)) / choices.len() as f32;
        for (choice, label) in choices {
            let chosen = *value == *choice;
            let text = if chosen {
                RichText::new(*label).color(palette.on_accent).strong()
            } else {
                RichText::new(*label).color(palette.text)
            };
            let button = egui::Button::new(text)
                .fill(if chosen {
                    palette.accent
                } else {
                    palette.raised
                })
                .min_size(vec2(width, height));
            if ui.add(button).clicked() && !chosen {
                *value = *choice;
                changed = true;
            }
        }
    });
    changed
}

/// Small choices in a wrapping row.
pub fn chips<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, choices: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        for (choice, label) in choices {
            ui.selectable_value(value, *choice, *label);
        }
    });
}

/// How a verdict reads.
#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Good,
    Bad,
    Unsure,
}

/// A verdict in a card: a word in capitals and what it means.
pub fn verdict(ui: &mut egui::Ui, tone: Tone, word: &str, lines: &[String]) {
    let palette = theme::palette(ui.ctx());
    let colour = match tone {
        Tone::Good => palette.good,
        Tone::Bad => palette.warning,
        Tone::Unsure => palette.text_muted,
    };
    egui::Frame::new()
        .fill(palette.raised)
        .stroke(egui::Stroke::new(1.5_f32, colour))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(word.to_uppercase())
                    .size(20.0)
                    .strong()
                    .color(colour),
            );
            for line in lines {
                ui.add(egui::Label::new(RichText::new(line).color(palette.text)).wrap());
            }
        });
}

/// A note in a tinted box, for what the numbers can't say.
pub fn note(ui: &mut egui::Ui, text: &str) {
    let palette = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(palette.warning_tint)
        .corner_radius(8)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(text).color(palette.text)).wrap());
        });
}

/// A wrapped line in the muted colour.
pub fn weak(ui: &mut egui::Ui, text: &str) {
    let muted = theme::palette(ui.ctx()).text_muted;
    ui.add(egui::Label::new(RichText::new(text).color(muted)).wrap());
}

/// An angle in the hunter's group unit: "0.12 mil".
pub fn angle(u: &UnitPrefs, mrad: f64) -> String {
    u.format(Quantity::GroupAngle, mrad)
}

/// "→ 0.17 · ↓ 0.06 mil": an impact as arrows, or "On the aim".
pub fn arrows(u: &UnitPrefs, impact: Point) -> String {
    let unit = u.unit(Quantity::GroupAngle);
    let number = |mrad: f64| format!("{:.*}", unit.decimals, unit.show(mrad.abs()));
    let zero = number(0.0);
    let mut parts = Vec::new();
    let across = number(impact.x);
    if across != zero {
        let arrow = if impact.x > 0.0 { "→" } else { "←" };
        parts.push(format!("{arrow} {across}"));
    }
    let up = number(impact.y);
    if up != zero {
        let arrow = if impact.y > 0.0 { "↑" } else { "↓" };
        parts.push(format!("{arrow} {up}"));
    }
    if parts.is_empty() {
        tr("On the aim").to_owned()
    } else {
        format!("{} {}", parts.join(" · "), unit.label)
    }
}

/// What to dial in the rifle's clicks: "2 clicks left, 1 click up".
pub fn dial_text(rifle: &Rifle, correction_mrad: Point) -> String {
    let clicks = |mrad: f64| -> i64 {
        let angle = ballistics_shot::from_radians(rifle.click_unit, mrad / 1000.0);
        if rifle.click_value > 0.0 {
            (angle / rifle.click_value).round() as i64
        } else {
            0
        }
    };
    let (across, up) = (clicks(correction_mrad.x), clicks(correction_mrad.y));
    let mut parts = Vec::new();
    if across != 0 {
        let way = if across > 0 { tr("right") } else { tr("left") };
        parts.push(format!(
            "{} {way}",
            crate::calculator::clicks_text(across.abs())
        ));
    }
    if up != 0 {
        let way = if up > 0 { tr("up") } else { tr("down") };
        parts.push(format!(
            "{} {way}",
            crate::calculator::clicks_text(up.abs())
        ));
    }
    if parts.is_empty() {
        tr("Nothing: it's on").to_owned()
    } else {
        parts.join(", ")
    }
}

/// "12 Nov" on the device's clock.
pub fn date(ms: i64, data: &Data) -> String {
    let day = super::local_ms(ms, data.country).div_euclid(86_400_000);
    let today =
        super::local_ms(ballistics_store::model::now_ms(), data.country).div_euclid(86_400_000);
    super::library::day_name(day, today)
}

/// "A", "B", … "Z", "AA"…: a group's letter in a comparison.
pub fn letter(index: usize) -> String {
    let mut text = String::new();
    let mut n = index;
    loop {
        text.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    text
}

/// Colours for lines and groups, distinct on paper and in both themes.
const COLOURS: [Color32; 8] = [
    Color32::from_rgb(0xE0, 0x6C, 0x1F),
    Color32::from_rgb(0x2F, 0x7E, 0xD8),
    Color32::from_rgb(0x2E, 0x9E, 0x5B),
    Color32::from_rgb(0x9B, 0x51, 0xC9),
    Color32::from_rgb(0xD2, 0x3B, 0x4A),
    Color32::from_rgb(0x1A, 0x9E, 0xA6),
    Color32::from_rgb(0x9C, 0x6B, 0x30),
    Color32::from_rgb(0xB0, 0x9A, 0x1E),
];

pub fn colour(index: usize) -> Color32 {
    COLOURS[index % COLOURS.len()]
}

/// The rifles' names for a picker.
pub fn rifle_name(data: &Data, id: Option<&str>) -> String {
    id.and_then(|id| data.rifles.iter().find(|rifle| rifle.id == id))
        .map_or_else(|| tr("No rifle").to_owned(), |rifle| rifle.name.clone())
}

/// A bullet's name, or "No bullet".
pub fn bullet_name(data: &Data, id: Option<&str>) -> String {
    id.and_then(|id| data.bullets.iter().find(|bullet| bullet.id == id))
        .map_or_else(
            || tr("No bullet given").to_owned(),
            |bullet| bullet.display_name(),
        )
}

/// A picker of `options` (id, name) with "All" first.
pub fn pick(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Option<String>,
    all: &str,
    options: &[(String, String)],
    width: f32,
) {
    let shown = value
        .as_ref()
        .and_then(|chosen| options.iter().find(|(id, _)| id == chosen))
        .map_or_else(|| all.to_owned(), |(_, name)| name.clone());
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, all);
            for (key, name) in options {
                ui.selectable_value(value, Some(key.clone()), name);
            }
        });
}

/// A button that asks first, saying what it will do in `question`; true
/// once confirmed. The confirming button takes clicks only after a moment,
/// so a double click can't confirm.
pub fn confirm(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    question: &str,
    destructive: bool,
) -> bool {
    let palette = theme::palette(ui.ctx());
    let id = ui.make_persistent_id(id);
    let colour = if destructive {
        palette.alarm
    } else {
        palette.text
    };
    let Some(shown_at) = ui.data(|data| data.get_temp::<f64>(id)) else {
        let mut button = egui::Button::new(RichText::new(label).color(colour));
        if destructive {
            button = button.stroke(egui::Stroke::new(1.0_f32, palette.alarm));
        }
        if ui.add(button).clicked() {
            let now = ui.input(|input| input.time);
            ui.data_mut(|data| data.insert_temp(id, now));
        }
        return false;
    };
    let mut confirmed = false;
    egui::Frame::new()
        .fill(palette.raised)
        .stroke(egui::Stroke::new(1.0_f32, colour))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(question).color(palette.text)).wrap());
            ui.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    ui.data_mut(|data| data.remove::<f64>(id));
                }
                ui.add_space(16.0);
                let ready = crate::widgets::confirm_ready(ui, shown_at);
                let button = if destructive {
                    egui::Button::new(RichText::new(label).color(palette.alarm))
                        .stroke(egui::Stroke::new(1.0_f32, palette.alarm))
                } else {
                    egui::Button::new(RichText::new(label).color(palette.on_accent))
                        .fill(palette.accent)
                };
                if ui.add_enabled(ready, button).clicked() {
                    confirmed = true;
                    ui.data_mut(|data| data.remove::<f64>(id));
                }
            });
        });
    confirmed
}

/// A drawn button in the accent colour.
pub fn primary(ui: &mut egui::Ui, label: &str) -> bool {
    let palette = theme::palette(ui.ctx());
    ui.add(egui::Button::new(RichText::new(label).color(palette.on_accent)).fill(palette.accent))
        .clicked()
}

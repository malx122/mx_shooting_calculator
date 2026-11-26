//! Shotguns, as rifles are: the gun on the Guns page with its chokes and the
//! pattern boards counted with each shell; its shells on the Ammunition
//! page; and on Calculate, a shell range by range, its pattern on the target
//! at true size, what the law says, what hunting bodies advise, and how far
//! it carries.
//!
//! The app says what a load does and what rules and named bodies say; it
//! doesn't judge how many pellets an animal needs, since the sources for
//! that are too uncertain.

use std::sync::Arc;

use ballistics_core::pellets::{self, Swarm};
use ballistics_shot::shotgun::{self as shells, BarrelPattern, PatternFrom, ShotTable};
use ballistics_store::rules::shotgun::{
    self as law, HUNTING_TEST_RANGE_M, ShotVerdict, SlugVerdict,
};
use ballistics_store::{
    Catalog, Choke, ChokeName, Country, PatternTest, Payload, Shell, ShotMaterial, Shotgun,
    ShotgunBarrel, ShotgunLoad, Target, TargetGroup, VelocityAt,
};
use eframe::egui::{self, Color32, Pos2, RichText, Stroke, pos2, vec2};

use crate::app::{Action, Data};
use crate::body3d;
use crate::figure::Figure;
use crate::i18n::{self, target_name, tr, trf};
use crate::mobile;
use crate::number_field::NumberField;
use crate::theme;
use crate::units::{self, Quantity, quantity};
use crate::widgets::{
    confirm_button, fold_with, grid, shotgun_caliber_name, shotgun_caliber_picker, text,
};

const SKY: Color32 = Color32::from_rgb(0xC9, 0xD2, 0xBE);
const FIELD: Color32 = Color32::from_rgb(0xA9, 0xB8, 0x98);
const PELLET: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
const ZONE: Color32 = Color32::from_rgba_premultiplied(0x50, 0x8C, 0x50, 0x70);
/// The ranges of a shell's table.
const TABLE_RANGES: [f64; 9] = [10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0];
const SLUG_RANGES: [f64; 6] = [25.0, 50.0, 75.0, 100.0, 125.0, 150.0];
/// Pellets drawn to estimate the average hits on a hit area that isn't a
/// circle.
const AVERAGE_DRAWS: usize = 4000;

/// What a shell's numbers were worked out from, so they are only worked out
/// again when it changes.
#[derive(Clone, PartialEq)]
struct Key {
    shotgun: Shotgun,
    shell: Shell,
    load: Option<ShotgunLoad>,
    barrel: usize,
    conditions: ballistics_shot::Conditions,
}

enum Worked {
    Shot(Box<ShotTable>),
    Slug(f64, Vec<ballistics_core::RangeRow>),
    Failed(String),
}

/// A shotgun on the Guns page: the gun, and the shells of its gauge with
/// the pattern boards counted with each, as a rifle has its loads.
#[derive(Default)]
pub struct ShotgunEditor {
    /// The gun open in the editor: a copy, saved with Save.
    editing: Option<Box<Shotgun>>,
    /// The shell whose pattern boards are open.
    shell: Option<String>,
    /// Its boards being edited.
    boards: Option<ShotgunLoad>,
    /// Barrels taken off the open gun since it was saved, in order: its
    /// boards follow when it is saved.
    removed: Vec<usize>,
}

impl ShotgunEditor {
    /// Closes the open gun, dropping its unsaved changes.
    pub fn back(&mut self) {
        self.editing = None;
        self.boards = None;
        self.removed.clear();
    }

    pub fn is_open(&self) -> bool {
        self.editing.is_some()
    }

    pub fn open_id(&self) -> Option<&str> {
        self.editing.as_ref().map(|editing| editing.id.as_str())
    }

    /// The open gun or its boards have changes that aren't saved.
    pub fn unsaved(&self, data: &Data) -> bool {
        let Some(shotgun) = &self.editing else {
            return false;
        };
        let gun = match data.shotguns.iter().find(|stored| stored.id == shotgun.id) {
            Some(stored) => stored != &**shotgun,
            None => !shotgun.name.trim().is_empty(),
        };
        gun || self.boards_unsaved(data)
    }

    fn boards_unsaved(&self, data: &Data) -> bool {
        let Some(boards) = &self.boards else {
            return false;
        };
        match stored_load(data, &boards.shotgun_id, &boards.shell_id) {
            Some(stored) => stored.patterns != boards.patterns,
            None => !boards.patterns.is_empty(),
        }
    }

    /// Keeps the editor on the gun that was just saved.
    pub fn saved(&mut self, shotgun: &Shotgun) {
        if self.open_id() == Some(shotgun.id.as_str()) {
            self.editing = Some(Box::new(shotgun.clone()));
            self.removed.clear();
        }
    }

    /// Keeps the boards just saved (or reset) open.
    pub fn boards_saved(&mut self) {
        self.boards = None;
    }

    pub fn deleted(&mut self, id: &str) {
        if self.open_id() == Some(id) {
            self.back();
        }
    }

    /// Opens a gun, with a shell's boards open if one is given. The gun
    /// already open keeps its unsaved changes.
    pub fn open(&mut self, shotgun: Shotgun, shell: Option<&str>) {
        if self.open_id() != Some(shotgun.id.as_str()) {
            self.editing = Some(Box::new(shotgun));
            self.removed.clear();
            self.boards = None;
        }
        if shell.is_some() && self.shell.as_deref() != shell {
            self.shell = shell.map(str::to_owned);
            self.boards = None;
        }
    }

    /// The open gun's editor, then its shells with their pattern boards.
    pub fn detail(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        let Some(shotgun) = self.editing.as_deref_mut() else {
            return;
        };
        let stored = data.shotguns.iter().find(|stored| stored.id == shotgun.id);
        ui.heading(if stored.is_some() {
            tr("Edit shotgun")
        } else {
            tr("New shotgun")
        });
        ui.add_space(4.0);
        // The caliber's shell beside the fields when there is room, else
        // below them, as a rifle's cartridge is.
        let wide = ui.available_width() >= 760.0;
        let mut removed = None;
        ui.horizontal_top(|ui| {
            removed = gun_fields(ui, shotgun);
            if wide && let Some(cartridge) = shotgun.caliber().and_then(|c| c.cartridge.as_ref()) {
                ui.add_space(32.0);
                let width = ui.available_width().min(1100.0);
                ui.vertical(|ui| {
                    crate::cartridge_drawing::with_3d(
                        ui,
                        cartridge,
                        width,
                        170.0,
                        150.0,
                        "shotgun",
                        |_| {},
                    );
                });
            }
        });
        if !wide && let Some(cartridge) = shotgun.caliber().and_then(|c| c.cartridge.as_ref()) {
            crate::cartridge_drawing::with_3d(
                ui,
                cartridge,
                ui.available_width().min(360.0),
                64.0,
                64.0,
                "shotgun",
                |_| {},
            );
        }
        if let Some(removed) = removed {
            self.removed.push(removed);
        }
        ui.add_space(6.0);
        let problem = shotgun.validate().err();
        let changed = stored != Some(&*shotgun);
        let mut close = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    problem.is_none() && changed,
                    egui::Button::new(RichText::new(tr("Save")).strong()),
                )
                .clicked()
            {
                actions.push(Action::SaveShotgun(Box::new(shotgun.clone())));
                // Boards counted with a barrel that was taken off go, and
                // the others keep pointing at their own barrel.
                for load in data
                    .shotgun_loads
                    .iter()
                    .filter(|load| load.shotgun_id == shotgun.id)
                {
                    let mut moved = load.clone();
                    for &removed in &self.removed {
                        moved.patterns.retain(|test| test.barrel != removed);
                        for test in &mut moved.patterns {
                            if test.barrel > removed {
                                test.barrel -= 1;
                            }
                        }
                    }
                    if moved != *load {
                        actions.push(Action::SaveShotgunLoad(Box::new(moved)));
                    }
                }
            }
            if let Some(stored) = stored {
                if changed && ui.button(tr("Undo changes")).clicked() {
                    *shotgun = stored.clone();
                    self.removed.clear();
                }
                if confirm_button(ui, "delete-shotgun", tr("Delete")) {
                    actions.push(Action::DeleteShotgun(shotgun.id.clone()));
                }
            }
            close = ui.button(tr("Close")).clicked();
        });
        if let Some(problem) = problem
            && !shotgun.name.trim().is_empty()
        {
            ui.colored_label(theme::ALARM, problem);
        } else if changed && stored.is_some() {
            ui.weak(tr("Unsaved changes"));
        }
        if close {
            self.back();
            return;
        }
        ui.add_space(10.0);
        ui.separator();
        ui.heading(tr("Shells in this gun"));
        // Boards are counted against the gun as it is saved.
        let Some(shotgun) = stored.filter(|_| !changed) else {
            ui.weak(tr("Save the gun first."));
            return;
        };
        let shells: Vec<&Shell> = data
            .shells
            .iter()
            .filter(|shell| shotgun.takes(shell))
            .collect();
        if shells.is_empty() {
            ui.weak(trf!(
                "No shells for {} yet.",
                shotgun_caliber_name(&shotgun.caliber)
            ));
            if ui.button(tr("Go to Ammunition")).clicked() {
                actions.push(Action::ShowShellsFor(shotgun.caliber.clone()));
            }
            return;
        }
        ui.weak(tr(
            "Your pattern boards with a shell in this gun replace its chokes' nominal patterns.",
        ));
        let boards_unsaved = self.boards_unsaved(data);
        for shell in &shells {
            let boards =
                stored_load(data, &shotgun.id, &shell.id).map_or(0, |load| load.patterns.len());
            let detail = match boards {
                0 => tr("nominal pattern").to_owned(),
                count => boards_text(count),
            };
            let open = self.shell.as_deref() == Some(shell.id.as_str());
            let response = ui.add_enabled(
                open || !boards_unsaved,
                egui::Button::selectable(open, format!("{}  ·  {}", shell.display_name(), detail)),
            );
            if response.clicked() && !open {
                self.shell = Some(shell.id.clone());
                self.boards = None;
            }
        }
        if boards_unsaved {
            ui.weak(tr(
                "Save or undo the open boards before choosing another shell.",
            ));
        }
        let Some(shell) = self
            .shell
            .as_deref()
            .and_then(|id| shells.iter().find(|shell| shell.id == id))
        else {
            return;
        };
        ui.add_space(6.0);
        if ui
            .button(trf!("Calculate with {}", shell.display_name()))
            .clicked()
        {
            actions.push(Action::CalculateWith(shotgun.id.clone(), shell.id.clone()));
        }
        if shell.is_slug() {
            return;
        }
        let stored_boards = stored_load(data, &shotgun.id, &shell.id);
        let boards = self.boards.get_or_insert_with(|| {
            stored_boards
                .cloned()
                .unwrap_or_else(|| ShotgunLoad::new(&shotgun.id, &shell.id))
        });
        pattern_boards(ui, boards, shell, &barrel_names(shotgun));
        let problem = boards.validate(Some(shotgun.barrels.len())).err();
        let changed = stored_boards.map_or(!boards.patterns.is_empty(), |stored| {
            stored.patterns != boards.patterns
        });
        let mut undo = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    changed && problem.is_none(),
                    egui::Button::new(tr("Save the boards")),
                )
                .clicked()
            {
                actions.push(Action::SaveShotgunLoad(Box::new(boards.clone())));
            }
            if changed && ui.button(tr("Undo changes")).clicked() {
                undo = true;
            }
            if let Some(stored) = stored_boards
                && confirm_button(ui, "delete-boards", tr("Remove the boards"))
            {
                actions.push(Action::DeleteShotgunLoad(stored.id.clone()));
            }
        });
        if let Some(problem) = problem {
            ui.colored_label(theme::ALARM, problem);
        }
        if undo {
            self.boards = None;
        }
    }
}

/// The stored boards of a shell in a gun, if any.
pub fn stored_load<'a>(data: &'a Data, shotgun: &str, shell: &str) -> Option<&'a ShotgunLoad> {
    data.shotgun_loads
        .iter()
        .find(|load| load.shotgun_id == shotgun && load.shell_id == shell)
}

/// A shotgun shell's results on Calculate: its table range by range, what
/// the law and named bodies say for the target, the pattern on the target
/// at true size, and how far it carries.
pub struct ShotgunResults {
    worked: Option<(Key, Worked)>,
    /// The row at the range chosen on Calculate, when it isn't one of the
    /// table's own: worked out alone, so changing the range is cheap.
    chosen: Option<(Key, f64, Chosen)>,
    seen: Option<(SeenKey, Seen)>,
    seed: u64,
    /// Counts the times the table was worked out, so a shot's pellets are
    /// drawn again from a new swarm.
    generation: u64,
}

#[derive(Clone)]
enum Chosen {
    Shot(Option<shells::ShotRow>),
    Slug(Option<ballistics_core::RangeRow>),
}

impl Default for ShotgunResults {
    fn default() -> Self {
        Self {
            worked: None,
            chosen: None,
            seen: None,
            seed: 1,
            generation: 0,
        }
    }
}

/// The farthest range a shot table and a slug table go to.
const SHOT_TABLE_LIMIT_M: f64 = pellets::MAX_PATTERN_RANGE_M;
const SLUG_TABLE_LIMIT_M: f64 = 300.0;

impl ShotgunResults {
    /// The results for a shell from a barrel of the gun, at the day's
    /// conditions, on `target` at `range_m`.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        shotgun: &Shotgun,
        shell: &Shell,
        barrel: usize,
        target: Option<&Target>,
        range_m: f64,
        target_speed: f64,
        conditions: &ballistics_shot::Conditions,
    ) {
        let load = stored_load(data, &shotgun.id, &shell.id);
        // Shot is flown level, without a rifle's spin or jump: those don't
        // change it, so changing them mustn't work the pellets out again.
        let conditions = if shell.is_slug() {
            conditions.clone()
        } else {
            ballistics_shot::Conditions {
                inclination_degrees: 0.0,
                cant_degrees: 0.0,
                spin_drift: false,
                aerodynamic_jump: false,
                ..conditions.clone()
            }
        };
        let key = Key {
            shotgun: shotgun.clone(),
            shell: shell.clone(),
            load: load.cloned(),
            barrel,
            conditions,
        };
        self.work(&key);
        let chosen = self.chosen(&key, range_m);
        let worked = &self.worked.as_ref().expect("just worked out").1;
        let (pattern, pellets, swarm) = match (worked, chosen) {
            (Worked::Failed(problem), _) => {
                ui.heading(tr("No solution"));
                ui.colored_label(theme::ALARM, problem.clone());
                return;
            }
            (Worked::Shot(table), chosen) => {
                let mut table = (**table).clone();
                if let Some(Chosen::Shot(Some(row))) = chosen {
                    table.rows.push(row);
                    table.rows.sort_by(|a, b| a.range_m.total_cmp(&b.range_m));
                }
                shot_results(ui, shotgun, shell, &table, barrel, range_m, data.country);
                (
                    Some(table.pattern),
                    table.pellets,
                    Some(table.swarm.clone()),
                )
            }
            (Worked::Slug(muzzle, rows), chosen) => {
                let mut rows = rows.clone();
                if let Some(Chosen::Slug(Some(row))) = chosen {
                    rows.push(row);
                    rows.sort_by(|a, b| a.range.total_cmp(&b.range));
                }
                slug_results(ui, shotgun, *muzzle, &rows, range_m, target_speed);
                (None, 0, None)
            }
        };
        let Some(target) = target else {
            return;
        };
        ui.add_space(10.0);
        ui.separator();
        ui.heading(target_name(target));
        // The rules know the catalog's species, and copies of them.
        let species = target
            .catalog_id
            .as_deref()
            .or(Some(target.id.as_str()))
            .filter(|id| Catalog::builtin().target(id).is_some())
            .map(str::to_owned);
        if data.hunting
            && target.is_game()
            && let Some(country) = data.country
        {
            let allowed = legality(ui, shotgun, shell, target, species.as_deref(), country);
            if let Some(species) = &species
                && allowed
            {
                advice(ui, shell, species, country);
            }
        }
        if let (Some(pattern), Some(swarm)) = (pattern, swarm) {
            ui.add_space(6.0);
            if range_m > SHOT_TABLE_LIMIT_M {
                ui.colored_label(
                    theme::ALARM,
                    trf!(
                        "{} m is too far for shot: the pattern on the animal is worked out to \
                         {} m at most. Choose a shorter range to see it.",
                        range_m.round(),
                        SHOT_TABLE_LIMIT_M
                    ),
                );
                return;
            }
            ui.horizontal(|ui| {
                if ui.button(tr("New shot")).clicked() {
                    self.seed = self.seed.wrapping_add(1);
                }
                ui.weak(tr(
                    "Another shot: its own pellets, and a pattern a little wider or tighter, as \
                     shots are.",
                ));
            });
            pattern_on_game(
                ui,
                &mut self.seen,
                Shooting {
                    target,
                    species: species.as_deref().filter(|_| data.hunting),
                    swarm: &swarm,
                    generation: self.generation,
                    percent: pattern.percent,
                    pellets,
                    range_m,
                    speed: target_speed,
                    seed: self.seed,
                },
                data.country,
            );
        }
    }

    /// The shell's table at its own ranges, worked out again only when
    /// something it depends on changed (not the chosen range).
    fn work(&mut self, key: &Key) {
        if self.worked.as_ref().is_some_and(|(known, _)| known == key) {
            return;
        }
        let Key {
            shotgun,
            shell,
            load,
            barrel,
            conditions,
        } = key;
        let worked = match shell.payload {
            Payload::Shot { .. } => {
                match shells::shot_table(
                    shotgun,
                    *barrel,
                    shell,
                    load.as_ref(),
                    conditions,
                    &TABLE_RANGES,
                ) {
                    Ok(Some(table)) => Worked::Shot(Box::new(table)),
                    Ok(None) => Worked::Failed(tr("No pattern for this barrel.").to_owned()),
                    Err(error) => Worked::Failed(error.to_string()),
                }
            }
            Payload::Slug { .. } => {
                match shells::slug_table(shotgun, shell, conditions, &SLUG_RANGES) {
                    Ok(Some((muzzle, rows))) => Worked::Slug(muzzle, rows),
                    Ok(None) => Worked::Failed(String::new()),
                    Err(error) => Worked::Failed(error.to_string()),
                }
            }
        };
        self.worked = Some((key.clone(), worked));
        self.generation = self.generation.wrapping_add(1);
    }

    /// The row at the chosen range, when the table hasn't one there.
    fn chosen(&mut self, key: &Key, range_m: f64) -> Option<Chosen> {
        let (own, limit) = match key.shell.payload {
            Payload::Shot { .. } => (&TABLE_RANGES[..], SHOT_TABLE_LIMIT_M),
            Payload::Slug { .. } => (&SLUG_RANGES[..], SLUG_TABLE_LIMIT_M),
        };
        if !(range_m > 0.0 && range_m <= limit)
            || own.iter().any(|range| (range - range_m).abs() < 0.01)
        {
            return None;
        }
        let fresh = self
            .chosen
            .as_ref()
            .is_some_and(|(known, range, _)| known == key && *range == range_m);
        if !fresh {
            let Key {
                shotgun,
                shell,
                conditions,
                ..
            } = key;
            let row = match self.worked.as_ref().map(|(_, worked)| worked) {
                Some(Worked::Shot(table)) => {
                    let row = shells::pellet(shell).and_then(|pellet| {
                        shells::shot_rows(
                            &pellet,
                            &table.pattern,
                            shell,
                            table.muzzle_velocity_mps,
                            conditions,
                            &[range_m],
                            Some(&table.swarm),
                        )
                        .ok()?
                        .pop()
                    });
                    Chosen::Shot(row)
                }
                Some(Worked::Slug(muzzle, _)) => Chosen::Slug(
                    shells::slug_rows(shotgun, shell, conditions, *muzzle, &[range_m])
                        .ok()
                        .and_then(|mut rows| rows.pop()),
                ),
                _ => return None,
            };
            self.chosen = Some((key.clone(), range_m, row));
        }
        self.chosen.as_ref().map(|(_, _, row)| row.clone())
    }
}

fn barrel_names(shotgun: &Shotgun) -> Vec<String> {
    shotgun
        .barrels
        .iter()
        .enumerate()
        .map(|(number, barrel)| {
            if barrel.name.trim().is_empty() {
                trf!("Barrel {}", number + 1)
            } else {
                barrel.name.clone()
            }
        })
        .collect()
}

/// The gun's fields; returns the barrel taken off, if one was.
fn gun_fields(ui: &mut egui::Ui, shotgun: &mut Shotgun) -> Option<usize> {
    let narrow = mobile::is_narrow(ui.ctx());
    let mut removed = None;
    grid(ui, "shotgun-fields", |ui| {
        text(ui, tr("Name"), &mut shotgun.name);
        ui.label(tr("Gauge"));
        shotgun_caliber_picker(ui, "shotgun-caliber", &mut shotgun.caliber);
        ui.end_row();
        let count = shotgun.barrels.len();
        let mut remove = None;
        for (number, barrel) in shotgun.barrels.iter_mut().enumerate() {
            ui.label(trf!("Barrel {}", number + 1));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut barrel.name)
                        .hint_text(tr("Under, over…"))
                        .desired_width(if narrow { 60.0 } else { 90.0 }),
                );
                choke_picker(ui, number, &mut barrel.choke, narrow);
                if count > 1 && ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                    remove = Some(number);
                }
            });
            ui.end_row();
        }
        if let Some(number) = remove {
            shotgun.barrels.remove(number);
            removed = Some(number);
        }
        ui.label("");
        if shotgun.barrels.len() < ballistics_store::shotguns::MAX_BARRELS
            && ui.button(tr("➕ Barrel")).clicked()
        {
            shotgun.barrels.push(ShotgunBarrel {
                name: String::new(),
                choke: Choke::Name {
                    name: ChokeName::Modified,
                },
            });
        }
        ui.end_row();
        ui.label(tr("Sights, for slugs")).on_hover_text(tr(
            "How high the sights or scope are above the bore; empty for a bead.",
        ));
        optional_cm(ui, &mut shotgun.sight_height_cm, 5.0);
        ui.end_row();
        ui.label(tr("Slug zero"))
            .on_hover_text(tr("The range the sights are set for, for slugs."));
        ui.horizontal(|ui| {
            let mut known = shotgun.slug_zero_m.is_some();
            if ui.checkbox(&mut known, "").changed() {
                shotgun.slug_zero_m = known.then_some(shells::DEFAULT_SLUG_ZERO_M);
            }
            match &mut shotgun.slug_zero_m {
                Some(zero) => {
                    units::drag(ui, zero, Quantity::Distance, 5.0..=200.0, 1.0);
                }
                None => {
                    ui.weak(trf!("{} m", shells::DEFAULT_SLUG_ZERO_M));
                }
            }
        });
        ui.end_row();
        ui.label(tr("Notes"));
        ui.text_edit_multiline(&mut shotgun.notes);
        ui.end_row();
    });
    removed
}

fn optional_cm(ui: &mut egui::Ui, value: &mut Option<f64>, default: f64) {
    ui.horizontal(|ui| {
        let mut known = value.is_some();
        if ui.checkbox(&mut known, "").changed() {
            *value = known.then_some(default);
        }
        match value {
            Some(height) => {
                units::drag(ui, height, Quantity::SightHeight, 0.0..=20.0, 0.1);
            }
            None => {
                ui.weak(tr("a bead"));
            }
        }
    });
}

fn choke_label(choke: Choke) -> String {
    match choke {
        Choke::Name { name } => i18n::tr_data(name.label()).to_owned(),
        Choke::Constriction { mm } => trf!("{:.2} mm constriction", mm),
    }
}

/// A named choke for the picker: its label, and under it, smaller, its
/// other names and markings, so a hunter can find it by what is stamped on
/// the tube.
fn choke_choice(ui: &egui::Ui, name: ChokeName) -> egui::text::LayoutJob {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    job.append(
        i18n::tr_data(name.label()),
        0.0,
        egui::TextFormat::simple(body.clone(), ui.visuals().text_color()),
    );
    job.append(
        &format!("\n{}", i18n::tr_data(name.other_names())),
        0.0,
        egui::TextFormat::simple(
            egui::FontId::proportional(body.size * 0.85),
            ui.visuals().weak_text_color(),
        ),
    );
    job
}

fn choke_picker(ui: &mut egui::Ui, number: usize, choke: &mut Choke, narrow: bool) {
    let response = egui::ComboBox::from_id_salt(("choke", number))
        .selected_text(choke_label(*choke))
        .width(if narrow { 150.0 } else { 210.0 })
        .show_ui(ui, |ui| {
            ui.set_min_width(if narrow { 220.0 } else { 300.0 });
            for name in ChokeName::ALL {
                let job = choke_choice(ui, name);
                ui.selectable_value(choke, Choke::Name { name }, job);
            }
            if !matches!(choke, Choke::Constriction { .. })
                && ui
                    .selectable_label(false, tr("By its constriction…"))
                    .clicked()
            {
                *choke = Choke::Constriction { mm: 0.5 };
            }
        })
        .response;
    if let Choke::Name { name } = choke {
        response.on_hover_text(i18n::tr_data(name.other_names()));
    }
    if let Choke::Constriction { mm } = choke {
        ui.add(
            NumberField::new(mm)
                .range(0.0..=3.0)
                .speed(0.01)
                .max_decimals(2)
                .suffix(" mm"),
        );
    }
}

/// The sizes to pick a pellet by: Swedish US numbers, larger American
/// sizes and English numbers, each with its millimetres.
fn pellet_sizes() -> Vec<(String, f64)> {
    let mut sizes: Vec<(String, f64)> = ["1", "2", "3", "4", "5", "6", "7", "7½", "8", "9"]
        .iter()
        .filter_map(|number| {
            pellets::nordic_diameter(number).map(|diameter| (format!("US {number}"), diameter))
        })
        .collect();
    sizes.extend(
        pellets::US_SIZES[11..]
            .iter()
            .map(|&(label, diameter)| (trf!("{} (American)", label), diameter)),
    );
    sizes.extend(
        pellets::ENGLISH_SIZES
            .iter()
            .map(|&(label, diameter)| (trf!("English {}", label), diameter)),
    );
    sizes
}

/// "US 5 · 3.0 mm", or just the millimetres when no number matches.
pub fn pellet_name(millimetres: f64) -> String {
    let number = ["1", "2", "3", "4", "5", "6", "7", "7½", "8", "9"]
        .iter()
        .find(|number| {
            pellets::nordic_diameter(number)
                .is_some_and(|diameter| (diameter * 1000.0 - millimetres).abs() < 0.03)
        });
    match number {
        Some(number) => format!("US {number} · {millimetres:.2} mm"),
        None => format!("{millimetres:.2} mm"),
    }
}

/// The fields of a shotgun shell, for the Ammunition page.
pub fn shell_fields(ui: &mut egui::Ui, shell: &mut Shell, country: Option<Country>) {
    let bore_mm = shell.gauge().bore_mm();
    grid(ui, "shell-fields", |ui| {
        text(ui, tr("Manufacturer"), &mut shell.manufacturer);
        ui.label(tr("Name"));
        ui.add(
            egui::TextEdit::singleline(&mut shell.name)
                .hint_text(tr("Name and load, as on the box"))
                .desired_width(240.0),
        );
        ui.end_row();
        ui.label(tr("Gauge"));
        shotgun_caliber_picker(ui, "shell-caliber", &mut shell.caliber).on_hover_text(tr(
            "A shell fits guns of its gauge with a chamber as long or longer: a 12/70 shell \
             fits a 12/76, never the other way round.",
        ));
        ui.end_row();
        ui.label(tr("Loaded with"));
        ui.horizontal(|ui| {
            let is_shot = matches!(shell.payload, Payload::Shot { .. });
            if ui.selectable_label(is_shot, tr("Shot")).clicked() && !is_shot {
                shell.payload = Shell::new(&shell.caliber).payload;
            }
            if ui.selectable_label(!is_shot, tr("Slug")).clicked() && is_shot {
                shell.payload = Payload::Slug {
                    mass_g: 28.0,
                    diameter_mm: bore_mm,
                    bc_g1: 0.07,
                    lead_free: false,
                };
            }
        });
        ui.end_row();
        match &mut shell.payload {
            Payload::Shot {
                material,
                pellet_mm,
                load_g,
            } => {
                ui.label(tr("Shot"));
                egui::ComboBox::from_id_salt("shot-material")
                    .selected_text(i18n::tr_data(material.label()))
                    .show_ui(ui, |ui| {
                        for option in ShotMaterial::ALL {
                            ui.selectable_value(material, option, i18n::tr_data(option.label()));
                        }
                    });
                ui.end_row();
                ui.label(tr("Pellet size"));
                ui.horizontal(|ui| {
                    ui.add(
                        NumberField::new(pellet_mm)
                            .range(1.0..=10.0)
                            .speed(0.05)
                            .max_decimals(2)
                            .suffix(" mm"),
                    );
                    egui::ComboBox::from_id_salt("pellet-number")
                        .selected_text(pellet_name(*pellet_mm))
                        .height(320.0)
                        .show_ui(ui, |ui| {
                            for (label, diameter) in pellet_sizes() {
                                let millimetres = diameter * 1000.0;
                                if ui
                                    .selectable_label(
                                        (millimetres - *pellet_mm).abs() < 1e-6,
                                        format!("{label} · {millimetres:.2} mm"),
                                    )
                                    .clicked()
                                {
                                    *pellet_mm = (millimetres * 100.0).round() / 100.0;
                                }
                            }
                        });
                })
                .response
                .on_hover_text(tr(
                    "The number on a box stands for different sizes in different countries: \
                     go by the millimetres. Swedish boxes use US numbers (US 5 is 3.0 mm).",
                ));
                ui.end_row();
                quantity(
                    ui,
                    tr("Load of shot"),
                    load_g,
                    Quantity::BulletMass,
                    1.0..=80.0,
                    0.5,
                );
            }
            Payload::Slug {
                mass_g,
                diameter_mm,
                bc_g1,
                lead_free,
            } => {
                quantity(
                    ui,
                    tr("Slug weight"),
                    mass_g,
                    Quantity::BulletMass,
                    3.0..=70.0,
                    0.5,
                );
                quantity(
                    ui,
                    tr("Slug diameter"),
                    diameter_mm,
                    Quantity::BulletDiameter,
                    6.0..=25.0,
                    0.1,
                );
                ui.label(tr("G1 BC")).on_hover_text(tr(
                    "As the maker gives it; Brenneke's tables fit about 0.07.",
                ));
                ui.add(
                    NumberField::new(bc_g1)
                        .range(0.01..=1.0)
                        .speed(0.001)
                        .max_decimals(3),
                );
                ui.end_row();
                ui.label(tr("Lead-free"));
                ui.checkbox(lead_free, "");
                ui.end_row();
            }
        }
        quantity(
            ui,
            tr("Velocity"),
            &mut shell.velocity_mps,
            Quantity::Speed,
            50.0..=1000.0,
            1.0,
        );
        ui.label(tr("Measured"));
        egui::ComboBox::from_id_salt("velocity-at")
            .selected_text(i18n::tr_data(shell.velocity_at.label()))
            .show_ui(ui, |ui| {
                for option in VelocityAt::ALL {
                    ui.selectable_value(
                        &mut shell.velocity_at,
                        option,
                        i18n::tr_data(option.label()),
                    );
                }
            })
            .response
            .on_hover_text(tr(
                "Where the box's velocity is measured. Shot slows fast: a V2.5 is 20–35 m/s \
                 below the velocity at the muzzle.",
            ));
        ui.end_row();
        ui.label(tr("Notes"));
        ui.text_edit_multiline(&mut shell.notes);
        ui.end_row();
    });
    if let Payload::Shot {
        material,
        pellet_mm,
        load_g,
    } = shell.payload
    {
        if let Some(count) = shells::pellet_count(shell) {
            let pellet = shells::pellet(shell).expect("shot has a pellet");
            ui.weak(trf!(
                "About {} pellets of {:.2} g each.",
                count,
                pellet.mass_kg() * 1000.0
            ));
        }
        steel_notes(ui, material, pellet_mm, load_g, country);
    }
}

/// What Naturvårdsverket and Svenska Jägareförbundet advise when going
/// from lead to steel, and their warnings for steel.
fn steel_notes(
    ui: &mut egui::Ui,
    material: ShotMaterial,
    pellet_mm: f64,
    load_g: f64,
    country: Option<Country>,
) {
    let max = country.and_then(|country| country.shotgun_law().max_pellet_mm);
    if material.is_lead() {
        let steel = pellets::steel_for_lead(pellet_mm / 1000.0) * 1000.0;
        let mut text = trf!(
            "Instead in steel: {} (0.5 mm, two US numbers, larger, as Naturvårdsverket and \
             Svenska Jägareförbundet advise).",
            pellet_name(steel)
        );
        if let Some(max) = max
            && steel > max + 1e-9
        {
            text.push(' ');
            text.push_str(&trf!(
                "That is larger than the {} mm the law allows: take {} mm steel, or denser \
                 shot.",
                max,
                max
            ));
        }
        ui.weak(text);
    } else if material.is_hard() {
        let mut lines =
            vec![tr("Hard shot: no tighter than half choke (Svenska Jägareförbundet).").to_owned()];
        if (material == ShotMaterial::Steel && load_g >= 32.0) || pellet_mm > 3.25 {
            lines.push(
                tr(
                    "32 g or more of steel, or hard shot larger than US 4 (3.25 mm): only in \
                    a gun its maker says can take it (Svenska Jägareförbundet).",
                )
                .to_owned(),
            );
        }
        ui.weak(lines.join("\n"));
    }
}

/// The hunter's pattern boards with the shell, and what each says the
/// barrel throws.
fn pattern_boards(ui: &mut egui::Ui, load: &mut ShotgunLoad, shell: &Shell, barrels: &[String]) {
    let summary = match load.patterns.len() {
        0 => tr("none: the choke's nominal pattern is used").to_owned(),
        count => trf!("{} counted", count),
    };
    let in_load = shells::pellet_count(shell);
    fold_with(
        ui,
        "pattern-boards",
        tr("Pattern boards"),
        &summary,
        true,
        |ui| {
            ui.weak(tr(
                "Shoot at a large sheet from a measured distance (35 m is the Nordic test), \
             draw a circle (75 cm) round the pattern's densest part and count the pellets \
             inside. Your own count replaces the choke's nominal pattern for that barrel.",
            ));
            let mut remove = None;
            for (number, test) in load.patterns.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt(("board-barrel", number))
                        .selected_text(barrels.get(test.barrel).cloned().unwrap_or_default())
                        .show_ui(ui, |ui| {
                            for (index, name) in barrels.iter().enumerate() {
                                ui.selectable_value(&mut test.barrel, index, name);
                            }
                        });
                    units::drag(
                        ui,
                        &mut test.distance_m,
                        Quantity::Distance,
                        5.0..=100.0,
                        0.5,
                    );
                    ui.label(tr("circle"));
                    ui.add(
                        NumberField::new(&mut test.circle_cm)
                            .range(10.0..=300.0)
                            .speed(1.0)
                            .suffix(" cm"),
                    );
                    ui.label(tr("pellets inside"));
                    ui.add(NumberField::new(&mut test.pellets_inside).range(0..=100_000));
                    let all = test
                        .pellets_in_load
                        .or(in_load.map(|count| count as u32))
                        .filter(|&all| all > 0);
                    if test.pellets_inside == 0 {
                        ui.weak(tr("enter the count: left out until then"));
                    } else if let Some(all) = all
                        && test.pellets_inside > all
                    {
                        ui.colored_label(
                            theme::ALARM,
                            if test.pellets_in_load.is_some() {
                                trf!("more than the {} you counted in the load", all)
                            } else {
                                trf!("more than the {} the load holds: left out", all)
                            },
                        );
                    } else if let Some(all) = all {
                        let percent = 100.0 * test.pellets_inside as f64 / all as f64;
                        let at_40 = pellets::percent_from_test(
                            percent.clamp(0.5, 99.5),
                            test.circle_cm / 200.0,
                            test.distance_m,
                        );
                        ui.weak(match at_40 {
                            Ok(at_40) => {
                                trf!("{:.0} % of {}: {:.0} % at 40 yards", percent, all, at_40)
                            }
                            Err(_) => trf!("{:.0} % of {}", percent, all),
                        });
                    }
                    if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                        remove = Some(number);
                    }
                });
                ui.horizontal(|ui| {
                    let mut counted = test.pellets_in_load.is_some();
                    if ui
                        .checkbox(&mut counted, tr("I counted the whole load"))
                        .changed()
                    {
                        test.pellets_in_load =
                            counted.then(|| in_load.map_or(200, |count| count as u32));
                    }
                    if let Some(all) = &mut test.pellets_in_load {
                        ui.add(NumberField::new(all).range(1..=100_000));
                    }
                });
            }
            if let Some(number) = remove {
                load.patterns.remove(number);
            }
            if load.patterns.len() < ballistics_store::shotguns::MAX_PATTERN_TESTS
                && ui.button(tr("➕ Add a pattern board")).clicked()
            {
                load.patterns.push(PatternTest {
                    barrel: 0,
                    distance_m: 35.0,
                    circle_cm: 75.0,
                    pellets_inside: 0,
                    pellets_in_load: None,
                });
            }
        },
    );
}

fn pattern_source(pattern: &BarrelPattern, shotgun: &Shotgun, barrel: usize) -> String {
    match pattern.from {
        PatternFrom::Boards(1) => tr("from your pattern board").to_owned(),
        PatternFrom::Boards(count) => trf!("from your {} pattern boards", count),
        PatternFrom::Choke { hard_shot } => {
            let choke = shotgun
                .barrels
                .get(barrel)
                .map(|barrel| choke_label(barrel.choke))
                .unwrap_or_default();
            if hard_shot {
                trf!("{}, nominal, about 10 points tighter for hard shot", choke)
            } else {
                trf!("{}, nominal", choke)
            }
        }
    }
}

/// One row of a table; the chosen range's stands out.
fn table_row<const N: usize>(ui: &mut egui::Ui, cells: [String; N], chosen: bool) {
    for cell in cells {
        if chosen {
            ui.label(RichText::new(cell).strong().color(theme::BLAZE));
        } else {
            ui.label(cell);
        }
    }
    ui.end_row();
}

/// "1 pattern board", "3 pattern boards".
pub fn boards_text(count: usize) -> String {
    if count == 1 {
        tr("1 pattern board").to_owned()
    } else {
        trf!("{} pattern boards", count)
    }
}

fn shot_results(
    ui: &mut egui::Ui,
    shotgun: &Shotgun,
    shell: &Shell,
    table: &ShotTable,
    barrel: usize,
    chosen_m: f64,
    country: Option<Country>,
) {
    let prefs = units::prefs(ui.ctx());
    ui.label(
        RichText::new(trf!(
            "{} pellets, {} at the muzzle. Pattern: {:.0} % in the 30-inch circle at 40 yards ({}).",
            table.pellets,
            prefs.format(Quantity::Speed, table.muzzle_velocity_mps),
            table.pattern.percent,
            pattern_source(&table.pattern, shotgun, barrel)
        ))
        .strong(),
    );
    ui.weak(tr(
        "In today's weather and wind, from Calculate, shot level. Drop is below the bore's line; \
         a gun is regulated to make up for some of it, and the pattern below is drawn centred \
         where you point.",
    ));
    ui.weak(tr(
        "The pellets don't arrive together: velocity and energy are the front pellets'. The \
         string is how long the swarm is front to back, 80 % of its pellets; the energy at the \
         back is a pellet's that trails nine in ten.",
    ));
    ui.add_space(4.0);
    egui::ScrollArea::horizontal()
        .id_salt("shot-table-scroll")
        .show(ui, |ui| {
            egui::Grid::new("shot-table")
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for heading in [
                        trf!("Range ({})", prefs.label(Quantity::Distance)),
                        trf!("Velocity ({})", prefs.label(Quantity::Speed)),
                        tr("Energy a pellet (J)").to_owned(),
                        trf!("Spread, 85 % ({})", prefs.label(Quantity::ShortLength)),
                        tr("In 75 cm").to_owned(),
                        tr("Pellets per dm² in the middle").to_owned(),
                        trf!("Drop ({})", prefs.label(Quantity::ShortLength)),
                        trf!("Wind drift ({})", prefs.label(Quantity::ShortLength)),
                        trf!("String, 80 % ({})", prefs.label(Quantity::Distance)),
                        tr("Energy at the back (J)").to_owned(),
                    ] {
                        ui.label(RichText::new(heading).strong());
                    }
                    ui.end_row();
                    for row in &table.rows {
                        let cells = [
                            prefs.number(Quantity::Distance, row.range_m, false),
                            prefs.number(Quantity::Speed, row.velocity_mps, false),
                            format!("{:.1}", row.pellet_energy_j),
                            prefs.number(Quantity::ShortLength, row.spread_cm, false),
                            format!("{:.0} %", row.in_75_cm_percent),
                            format!("{:.1}", row.middle_per_dm2),
                            prefs.number(Quantity::ShortLength, row.drop_cm, false),
                            prefs.number(Quantity::ShortLength, row.drift_cm, true),
                            row.string_80_m.map_or_else(String::new, |length| {
                                format!("{:.1}", prefs.show(Quantity::Distance, length))
                            }),
                            row.back_pellet_energy_j
                                .map_or_else(String::new, |energy| format!("{energy:.1}")),
                        ];
                        table_row(ui, cells, (row.range_m - chosen_m).abs() < 0.01);
                    }
                });
        });
    ui.add_space(6.0);
    let mut danger = match table.max_range {
        Some(farthest) => trf!(
            "How far it carries: up to {} at {:.0}° elevation in today's air, farther with \
             the wind behind.",
            prefs.format(Quantity::Distance, farthest.distance_m),
            farthest.elevation_degrees
        ),
        None => String::new(),
    };
    if country == Some(Country::Sweden) {
        danger.push(' ');
        danger.push_str(tr(
            "Swedish range-safety rules (SäkB 2020) take 250 m for US 6–9 and 300 m for US 1–5 on \
             clay ranges.",
        ));
    }
    if let Payload::Shot { material, .. } = shell.payload
        && material == ShotMaterial::Steel
    {
        danger.push(' ');
        danger.push_str(tr("Steel ricochets off stone, ice and water."));
    }
    ui.label(danger);
}

fn slug_results(
    ui: &mut egui::Ui,
    shotgun: &Shotgun,
    muzzle: f64,
    rows: &[ballistics_core::RangeRow],
    chosen_m: f64,
    target_speed: f64,
) {
    let prefs = units::prefs(ui.ctx());
    ui.label(
        RichText::new(trf!(
            "{} at the muzzle; the sights set for {}.",
            prefs.format(Quantity::Speed, muzzle),
            prefs.format(
                Quantity::Distance,
                shotgun.slug_zero_m.unwrap_or(shells::DEFAULT_SLUG_ZERO_M)
            )
        ))
        .strong(),
    );
    egui::ScrollArea::horizontal()
        .id_salt("slug-table-scroll")
        .show(ui, |ui| {
            egui::Grid::new("slug-table")
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for heading in [
                        trf!("Range ({})", prefs.label(Quantity::Distance)),
                        trf!("Path ({})", prefs.label(Quantity::ShortLength)),
                        trf!("Velocity ({})", prefs.label(Quantity::Speed)),
                        trf!("Energy ({})", prefs.label(Quantity::Energy)),
                        trf!("Wind drift ({})", prefs.label(Quantity::ShortLength)),
                    ] {
                        ui.label(RichText::new(heading).strong());
                    }
                    ui.end_row();
                    for row in rows {
                        let cells = [
                            prefs.number(Quantity::Distance, row.range, false),
                            prefs.number(Quantity::ShortLength, row.drop * 100.0, true),
                            prefs.number(Quantity::Speed, row.velocity, false),
                            prefs.number(Quantity::Energy, row.energy_j, false),
                            prefs.number(Quantity::ShortLength, row.windage * 100.0, true),
                        ];
                        table_row(ui, cells, (row.range - chosen_m).abs() < 0.01);
                    }
                });
        });
    if target_speed > 0.0
        && let Some(row) = rows.iter().find(|row| (row.range - chosen_m).abs() < 0.01)
    {
        let lead = ballistics_core::analysis::moving_target_lead(row, target_speed);
        ui.label(trf!(
            "Lead for a crossing animal at {}: {} ahead of the vital zone",
            prefs.format(Quantity::Distance, chosen_m),
            prefs.format(Quantity::ShortLength, lead.distance * 100.0)
        ));
    }
    ui.label(tr(
        "Swedish range-safety rules (SäkB 2020) give a slug a danger area 800 m long, 1200 m \
         over frozen ground, ice or water.",
    ));
}

/// What the country's law says about the shell, the gun and the species.
/// Returns whether the shell may be used on it (true too when the app
/// doesn't know the species).
fn legality(
    ui: &mut egui::Ui,
    shotgun: &Shotgun,
    shell: &Shell,
    target: &Target,
    species: Option<&str>,
    country: Country,
) -> bool {
    let law = country.shotgun_law();
    let name = target_name(target).to_lowercase();
    let gauge = shotgun.gauge().label();
    let source = format!("{} ({})", law.law, law.source_url);
    let rules = trf!("{} rules:", i18n::country_adjective(country));
    let line = |ui: &mut egui::Ui, allowed: bool, text: String| {
        ui.horizontal_wrapped(|ui| {
            crate::flags::show(ui, country, 11.0);
            ui.label(RichText::new(&rules).strong());
            ui.colored_label(if allowed { theme::MEADOW } else { theme::ALARM }, text)
                .on_hover_text(&source);
        });
    };
    let catalog = species.and_then(|id| Catalog::builtin().target(id));
    let Some((species, catalog)) = species.zip(catalog) else {
        ui.horizontal_wrapped(|ui| {
            crate::flags::show(ui, country, 11.0);
            ui.label(RichText::new(&rules).strong());
            ui.label(trf!(
                "the app knows them only for the catalog's animals and copies of them: check {}.",
                law.law
            ));
        });
        return true;
    };
    if !catalog.ammunition.contains_key(&country) {
        ui.horizontal_wrapped(|ui| {
            crate::flags::show(ui, country, 11.0);
            ui.label(RichText::new(&rules).strong());
            let note = catalog.rules_notes.get(&country).map_or_else(
                || trf!("{} isn't game there.", name),
                |note| i18n::tr_data(note).to_owned(),
            );
            ui.label(note);
        });
        return false;
    }
    let mut allowed = match shell.payload {
        Payload::Shot { pellet_mm, .. } => {
            let verdict = law.shot(species, shotgun.gauge());
            let full = law
                .full_gauges()
                .iter()
                .map(|gauge| gauge.label())
                .collect::<Vec<_>>()
                .join(", ");
            let (allowed, text) = match verdict {
                ShotVerdict::Allowed => (
                    true,
                    trf!("Shot is allowed for {} from a {} bore.", name, gauge),
                ),
                ShotVerdict::GaugeTooSmall => (
                    false,
                    trf!(
                        "Not from a {} bore: {} needs a bore of {}.",
                        gauge,
                        name,
                        full
                    ),
                ),
                ShotVerdict::GaugeNotAllowed => {
                    (false, trf!("A {} bore isn't allowed for hunting.", gauge))
                }
                ShotVerdict::NotAllowed => (false, trf!("Shot isn't allowed for {}.", name)),
            };
            line(ui, allowed, text);
            let too_large = law.max_pellet_mm.filter(|max| pellet_mm > max + 1e-9);
            if let Some(max) = too_large {
                line(
                    ui,
                    false,
                    trf!(
                        "Shot larger than {} mm isn't allowed (it is when tracking wounded game, \
                         and in some protective hunting).",
                        max
                    ),
                );
            }
            allowed && too_large.is_none()
        }
        Payload::Slug { .. } => {
            let (allowed, text) = match law.slug(species, shotgun.gauge(), shotgun.barrels.len()) {
                SlugVerdict::Allowed {
                    rifle_class: Some(class),
                } => (
                    true,
                    trf!(
                        "A slug is allowed for {} from a single barrel with sights, for a hunter \
                         qualified for class {} rifles.",
                        name,
                        class
                    ),
                ),
                SlugVerdict::Allowed { rifle_class: None } => {
                    (true, trf!("A slug is allowed for {}.", name))
                }
                SlugVerdict::OnlySingleBarrel => (
                    false,
                    tr("A slug only from a gun with a single barrel.").to_owned(),
                ),
                SlugVerdict::GaugeNotAllowed => (false, trf!("Not a slug from a {} bore.", gauge)),
                SlugVerdict::NotAllowed => (false, trf!("A slug isn't allowed for {}.", name)),
            };
            line(ui, allowed, text);
            if allowed && country == Country::Finland {
                ui.weak(tr(
                    "Whether a slug from a rifled barrel counts as a rifle (with its shooting \
                     test and expanding-bullet rules), the decree doesn't settle.",
                ));
            }
            allowed
        }
    };
    if shell.has_lead() && matches!(shell.payload, Payload::Shot { .. }) {
        if law.no_lead_for_waterfowl && catalog.group() == TargetGroup::Waterfowl {
            allowed = false;
            ui.colored_label(
                theme::ALARM,
                trf!(
                    "Lead shot isn't allowed for {}: no lead for waterfowl.",
                    name
                ),
            )
            .on_hover_text("Metsästyslaki 615/1993, 33 §");
        }
        ui.weak(tr(
            "Lead shot: not in or within 100 m of wetlands (EU, since 15 February 2023), and \
             not for any hunting from 7 December 2033 (EU 2026/2530).",
        ));
    }
    allowed
}

/// What named hunting bodies advise for the species, beside the shell.
fn advice(ui: &mut egui::Ui, shell: &Shell, species: &str, country: Country) {
    let Payload::Shot {
        material,
        pellet_mm,
        ..
    } = shell.payload
    else {
        return;
    };
    let prefs = units::prefs(ui.ctx());
    for advice in law::advice_for(country, species) {
        let mut parts = Vec::new();
        if let Some(range) = advice.max_range_m {
            parts.push(trf!(
                "no farther than {}",
                prefs.format(Quantity::Distance, range)
            ));
        }
        let sizes = if let Some(any) = advice.any_mm {
            Some((tr("shot"), any))
        } else if material == ShotMaterial::Steel {
            advice.steel_mm.map(|sizes| (tr("steel"), sizes))
        } else if material.is_lead() {
            advice.lead_mm.map(|sizes| (tr("lead"), sizes))
        } else {
            None
        };
        if let Some((kind, (low, high))) = sizes {
            let this = if pellet_mm < low - 0.01 {
                tr("this shell's is smaller")
            } else if pellet_mm > high + 0.01 {
                tr("this shell's is larger")
            } else {
                tr("this shell's is among them")
            };
            parts.push(trf!(
                "{} {}–{} mm ({})",
                kind,
                format!("{low:.2}"),
                format!("{high:.2}"),
                this
            ));
        }
        if parts.is_empty() {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}:", advice.by)).strong());
            ui.label(parts.join("; ")).on_hover_text(advice.source_url);
        });
    }
}

/// What a pattern on an animal was worked out for.
#[derive(Clone, PartialEq)]
pub struct SeenKey {
    target: Target,
    generation: u64,
    percent: f64,
    pellets: usize,
    range_m: f64,
    speed: f64,
    seed: u64,
}

/// A shot's pellets on the animal, and how many land on its hit area,
/// this shot and on average.
pub struct Seen {
    /// [ahead, up] on the animal, m.
    places: Vec<[f64; 2]>,
    on_zone: Vec<bool>,
    hits: usize,
    average: f64,
    /// The shot's pellets from the side: [along the range, up], m, as the
    /// middle of the swarm reaches the animal.
    side: Vec<[f64; 2]>,
    string: Option<pellets::ShotString>,
    pattern_scale: f64,
}

/// A shot at the animal: what the pattern on it is drawn from.
struct Shooting<'a> {
    target: &'a Target,
    species: Option<&'a str>,
    swarm: &'a Arc<Swarm>,
    generation: u64,
    percent: f64,
    pellets: usize,
    range_m: f64,
    /// The animal's speed across the line of fire, towards its head, m/s.
    speed: f64,
    seed: u64,
}

/// The pattern at the range on the animal at true size, with the pellets
/// that land on its hit area counted, this shot and on average. A crossing
/// animal is led by where it is when the middle of the swarm arrives; the
/// pellets that arrive before the middle land ahead of that, towards its
/// head, and those after it behind.
fn pattern_on_game(
    ui: &mut egui::Ui,
    seen: &mut Option<(SeenKey, Seen)>,
    shooting: Shooting<'_>,
    country: Option<Country>,
) {
    let Shooting {
        target,
        species,
        swarm,
        generation,
        percent,
        pellets,
        range_m,
        speed,
        seed,
    } = shooting;
    let figure = Figure::new(target, target.vital_diameter_cm);
    let vital_radius = target.vital_diameter_cm / 200.0;
    let key = SeenKey {
        target: target.clone(),
        generation,
        percent,
        pellets,
        range_m,
        speed,
        seed,
    };
    if seen.as_ref().is_none_or(|(known, _)| *known != key) {
        let on_zone = |point: [f64; 2]| {
            if figure.zones.is_empty() {
                point[0].hypot(point[1]) <= vital_radius
            } else {
                figure.hit(point).zone.is_some()
            }
        };
        let string = swarm.string_at(range_m);
        let middle = string.map_or(0.0, |string| string.middle_time_s);
        // Where each pellet lands on the animal, which has moved on by the
        // time it arrives.
        let on_animal = |pellet: &pellets::Arrival| {
            [
                pellet.place[0] - speed * (pellet.time_s - middle),
                pellet.place[1],
            ]
        };
        let shot = swarm.shot(pellets, seed, true).ok();
        let arrivals = shot
            .as_ref()
            .and_then(|shot| swarm.arrivals(shot, percent, range_m).ok())
            .unwrap_or_default();
        let places: Vec<[f64; 2]> = arrivals.iter().map(on_animal).collect();
        let side = arrivals
            .iter()
            .map(|pellet| {
                [
                    range_m + (middle - pellet.time_s) * pellet.velocity_mps,
                    pellet.place[1],
                ]
            })
            .collect();
        let marks: Vec<bool> = places.iter().map(|&point| on_zone(point)).collect();
        let hits = marks.iter().filter(|&&on| on).count();
        // Its own seed and the load's own pattern, so the average stays put
        // when only the shot changes.
        let draws = swarm
            .shot(AVERAGE_DRAWS, 0x5EED, false)
            .ok()
            .and_then(|shot| swarm.arrivals(&shot, percent, range_m).ok())
            .unwrap_or_default();
        let on = draws
            .iter()
            .filter(|pellet| on_zone(on_animal(pellet)))
            .count();
        let average = pellets as f64 * on as f64 / AVERAGE_DRAWS as f64;
        *seen = Some((
            key,
            Seen {
                places,
                on_zone: marks,
                hits,
                average,
                side,
                string,
                pattern_scale: shot.map_or(1.0, |shot| shot.pattern_scale),
            },
        ));
    }
    let Some((_, seen)) = seen.as_ref() else {
        return;
    };
    let test = species
        .filter(|_| country == Some(Country::Sweden))
        .and_then(law::hunting_test_figure);

    let prefs = units::prefs(ui.ctx());
    let width = ui.available_width().min(760.0);
    let [low, high] = figure.bounds;
    let (span_x, span_y) = ((high[0] - low[0]) * 1.15, (high[1] - low[1]) * 1.2);
    let height = (width as f64 * span_y / span_x).clamp(160.0, 460.0) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let scale = (rect.width() as f64 / span_x).min(rect.height() as f64 / span_y);
    let middle = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
    let at = |[x, y]: [f64; 2]| -> Pos2 {
        rect.center()
            + vec2(
                ((x - middle[0]) * scale) as f32,
                (-(y - middle[1]) * scale) as f32,
            )
    };
    painter.rect_filled(rect, 6.0, SKY);
    let ground = figure
        .body
        .as_ref()
        .map_or(-figure.aim_height, |body| body.ground());
    let feet = at([0.0, ground]).y;
    if feet < rect.bottom() {
        painter.rect_filled(
            egui::Rect::from_min_max(pos2(rect.left(), feet.max(rect.top())), rect.max),
            0.0,
            FIELD,
        );
    }
    let mut parts = &figure.parts[..];
    if let Some(body) = &figure.body {
        let corners: Vec<[f32; 3]> = body
            .body
            .vertices
            .iter()
            .map(|&corner| body.place(corner).map(|axis| axis as f32))
            .collect();
        painter.add(body3d::mesh(body, &corners, |[x, y, _]| {
            at([f64::from(x), f64::from(y)])
        }));
        parts = &figure.parts[figure.animal_parts..];
    }
    for part in parts {
        painter.add(part.shape(part.points.iter().map(|&point| at(point)).collect()));
    }
    let outline = Stroke::new(1.5_f32, theme::BONE);
    if figure.zones.is_empty() {
        painter.circle(at([0.0, 0.0]), (vital_radius * scale) as f32, ZONE, outline);
    } else {
        for (_, points) in &figure.zones {
            let points: Vec<Pos2> = points.iter().map(|&point| at(point)).collect();
            painter.add(egui::Shape::closed_line(points, outline));
        }
    }
    if let Some(test) = test {
        let radius = test.circle_cm / 200.0;
        let points: Vec<Pos2> = (0..=64)
            .map(|i| {
                let angle = i as f64 / 64.0 * std::f64::consts::TAU;
                at([radius * angle.cos(), radius * angle.sin()])
            })
            .collect();
        painter.extend(egui::Shape::dashed_line(
            &points,
            Stroke::new(1.5_f32, theme::BLAZE),
            4.0,
            3.0,
        ));
    }
    let dot = (0.0015 * scale).clamp(1.0, 3.0) as f32;
    for (&point, &on) in seen.places.iter().zip(&seen.on_zone) {
        painter.circle_filled(at(point), dot, if on { theme::BLAZE } else { PELLET });
    }
    painter.text(
        rect.left_top() + vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        trf!(
            "{} · {} pellets",
            prefs.format(Quantity::Distance, range_m),
            pellets
        ),
        egui::FontId::proportional(13.0),
        theme::FOREST_900,
    );
    ui.label(trf!(
        "This shot: {} pellets on the hit area; on average {:.0}. Spread (85 %): {}, this \
         shot {}.",
        seen.hits,
        seen.average,
        prefs.format(
            Quantity::ShortLength,
            pellets::spread_diameter(percent, 0.85, range_m).unwrap_or(0.0) * 100.0
        ),
        prefs.format(
            Quantity::ShortLength,
            pellets::spread_diameter(percent, 0.85, range_m).unwrap_or(0.0)
                * seen.pattern_scale
                * 100.0
        )
    ));
    if let Some(string) = seen.string {
        if speed > 0.0 {
            ui.label(trf!(
                "Crossing at {}: lead it by {}, to where it is when the middle of the swarm \
                 arrives, {:.0} ms after the shot. 80 % of the swarm passes in {:.0} ms, while \
                 the animal moves {}.",
                prefs.format(Quantity::WindSpeed, speed),
                prefs.format(Quantity::ShortLength, speed * string.middle_time_s * 100.0),
                string.middle_time_s * 1000.0,
                string.time_80_s * 1000.0,
                prefs.format(Quantity::ShortLength, speed * string.time_80_s * 100.0)
            ));
        }
        swarm_from_the_side(ui, &seen.side, range_m, string, &prefs);
    }
    if let Some(test) = test {
        ui.weak(trf!(
            "The dashed circle is the Swedish hunting test's (NFS 2015:10), a test of aim: \
             running at {}, with US 7 shot, a hit is at least {} pellets in its {} cm. It \
             isn't a measure of what a load kills.",
            prefs.format(Quantity::Distance, HUNTING_TEST_RANGE_M),
            test.pellets,
            test.circle_cm
        ))
        .on_hover_text(law::HUNTING_TEST_URL);
    }
    ui.weak(tr(
        "The app shows what the pattern does; it doesn't judge how many pellets an animal \
         needs. Shoot only at ranges you know you master.",
    ));
}

/// The shot's pellets seen from the side, at true proportions, as the
/// middle of the swarm reaches the animal: the string, with the front
/// pellets already past and the slow, dented ones trailing.
fn swarm_from_the_side(
    ui: &mut egui::Ui,
    side: &[[f64; 2]],
    range_m: f64,
    string: pellets::ShotString,
    prefs: &units::UnitPrefs,
) {
    if side.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(tr("The swarm from the side")).strong());
    let (mut back, mut front, mut tallest) = (f64::MAX, f64::MIN, 0.0_f64);
    for &[along, up] in side {
        back = back.min(along);
        front = front.max(along);
        tallest = tallest.max(up.abs());
    }
    let span_x = (front - back).max(0.5) + 0.6;
    let span_y = 2.0 * tallest.max(0.1) * 1.15;
    let width = ui.available_width().min(760.0);
    let scale = (width as f64 / span_x).min(220.0 / span_y);
    let height = (span_y * scale).max(70.0) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, SKY);
    let middle_x = (back + front) / 2.0;
    let at = |[along, up]: [f64; 2]| -> Pos2 {
        rect.center() + vec2(((along - middle_x) * scale) as f32, (-up * scale) as f32)
    };
    let line = Stroke::new(1.0_f32, theme::FOREST_900.gamma_multiply(0.5));
    painter.extend(egui::Shape::dashed_line(
        &[
            pos2(rect.left(), rect.center().y),
            pos2(rect.right(), rect.center().y),
        ],
        line,
        6.0,
        4.0,
    ));
    let plane = at([range_m, 0.0]).x;
    painter.line_segment(
        [pos2(plane, rect.top()), pos2(plane, rect.bottom())],
        Stroke::new(1.5_f32, theme::BLAZE),
    );
    let dot = (0.0015 * scale).clamp(1.0, 2.5) as f32;
    for &point in side {
        painter.circle_filled(at(point), dot, PELLET);
    }
    painter.text(
        pos2(plane + 4.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        prefs.format(Quantity::Distance, range_m),
        egui::FontId::proportional(12.0),
        theme::FOREST_900,
    );
    painter.text(
        rect.right_top() + vec2(-6.0, 4.0),
        egui::Align2::RIGHT_TOP,
        tr("flying this way →"),
        egui::FontId::proportional(12.0),
        theme::FOREST_900,
    );
    ui.weak(trf!(
        "As the middle of the swarm reaches the target (the line). Its front arrives after {:.0} \
         ms at {}; 80 % of it is within {} and passes in {:.0} ms. The trailing pellets started \
         slower or were dented in the barrel and drag more: one in ten is below {}.",
        string.front_time_s * 1000.0,
        prefs.format(Quantity::Speed, string.front_velocity_mps),
        format!(
            "{:.1} {}",
            prefs.show(Quantity::Distance, string.length_80_m),
            prefs.label(Quantity::Distance)
        ),
        string.time_80_s * 1000.0,
        prefs.format(Quantity::Speed, string.back_velocity_mps)
    ));
}

//! The Targets page: the catalog's game animals and sport targets, and the
//! hunter's own, with an editor and a drawing to scale.

use ballistics_store::{Catalog, Country, Drawing, Target, TargetGroup, TargetKind, TargetShape};
use eframe::egui::{self, Color32, Pos2, RichText, pos2, vec2};

use crate::app::{Action, Data};
use crate::figure::Figure;
use crate::i18n::{self, target_name, tr, tr_data, trf};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity, optional_quantity, quantity};
use crate::widgets::{confirm_button, grid, number, text};

const SKY: Color32 = Color32::from_rgb(0xC9, 0xD2, 0xBE);
const FIELD: Color32 = Color32::from_rgb(0xA9, 0xB8, 0x98);

enum Showing {
    Nothing,
    Catalog(String),
    /// One of the hunter's targets (or a new one) being edited.
    Custom(Box<Target>),
}

pub struct TargetsPage {
    showing: Showing,
    search: String,
}

impl Default for TargetsPage {
    fn default() -> Self {
        Self {
            showing: Showing::Nothing,
            search: String::new(),
        }
    }
}

impl TargetsPage {
    /// Keeps the editor on the target that was just saved.
    pub fn saved(&mut self, target: &Target) {
        if matches!(&self.showing, Showing::Custom(editing) if editing.id == target.id) {
            self.showing = Showing::Custom(Box::new(target.clone()));
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if matches!(&self.showing, Showing::Custom(editing) if editing.id == id) {
            self.showing = Showing::Nothing;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Data) -> Vec<Action> {
        let context = &ui.ctx().clone();
        let mut actions = Vec::new();
        if mobile::is_narrow(context) {
            // On a phone: the list, or one target with a way back.
            egui::CentralPanel::default().show(ui, |ui| {
                if matches!(self.showing, Showing::Nothing) {
                    egui::ScrollArea::vertical().show(ui, |ui| self.phone_list(ui, data));
                } else {
                    if mobile::back_button(ui, tr("Targets")) {
                        self.showing = Showing::Nothing;
                        return;
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| self.detail(ui, data, &mut actions));
                }
            });
            return actions;
        }
        egui::Panel::left("target-list")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.detail(ui, data, &mut actions));
        });
        actions
    }

    /// The chosen target: a catalog one to look at, or the hunter's own to
    /// edit.
    fn detail(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        match &mut self.showing {
            Showing::Nothing => {
                ui.weak(tr("Choose a target, or make your own."));
            }
            Showing::Catalog(id) => {
                if let Some(target) = Catalog::builtin().target(id)
                    && catalog_view(ui, target, data, data.region.as_deref())
                {
                    self.showing = Showing::Custom(Box::new(target.to_custom()));
                }
            }
            Showing::Custom(target) => {
                if editor(ui, target, data, actions) {
                    self.showing = Showing::Nothing;
                }
            }
        }
    }

    /// The phone's list: own targets, then the catalog's by group.
    fn phone_list(&mut self, ui: &mut egui::Ui, data: &Data) {
        search_field(ui, &mut self.search);
        ui.heading(tr("My targets"));
        if ui.button(tr("+ New target")).clicked() {
            self.showing = Showing::Custom(Box::new(Target::new_custom()));
        }
        for target in data.targets.iter().filter(|t| t.matches(&self.search)) {
            if mobile::list_row(ui, &target.name, &summary(ui, target, data.hunting), false)
                .clicked()
            {
                self.showing = Showing::Custom(Box::new(target.clone()));
            }
        }
        if data.targets.is_empty() {
            ui.weak(tr("None yet: copy one below or make a new one."));
        }
        ui.add_space(10.0);
        let mut chosen = None;
        catalog_groups(ui, &self.search, |ui, target| {
            if mobile::list_row(
                ui,
                target_name(target),
                &summary(ui, target, data.hunting),
                false,
            )
            .clicked()
            {
                chosen = Some(target.id.clone());
            }
        });
        if let Some(id) = chosen {
            self.showing = Showing::Catalog(id);
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, data: &Data) {
        search_field(ui, &mut self.search);
        ui.heading(tr("My targets"));
        if ui.button(tr("➕ New target")).clicked() {
            self.showing = Showing::Custom(Box::new(Target::new_custom()));
        }
        ui.add_space(4.0);
        for target in data.targets.iter().filter(|t| t.matches(&self.search)) {
            let selected =
                matches!(&self.showing, Showing::Custom(editing) if editing.id == target.id);
            if ui.selectable_label(selected, &target.name).clicked() {
                self.showing = Showing::Custom(Box::new(target.clone()));
            }
        }
        if data.targets.is_empty() {
            ui.weak(tr("None yet: copy one below or make a new one."));
        }
        ui.separator();
        let showing = match &self.showing {
            Showing::Catalog(id) => Some(id.clone()),
            _ => None,
        };
        let mut chosen = None;
        catalog_groups(ui, &self.search, |ui, target| {
            let selected = showing.as_deref() == Some(target.id.as_str());
            if ui.selectable_label(selected, target_name(target)).clicked() {
                chosen = Some(target.id.clone());
            }
        });
        if let Some(id) = chosen {
            self.showing = Showing::Catalog(id);
        }
    }
}

fn search_field(ui: &mut egui::Ui, search: &mut String) {
    ui.add(
        egui::TextEdit::singleline(search)
            .hint_text(tr("Search, also Swedish or Finnish names"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
}

/// The catalog's targets that match the search, in folding sections by
/// group; all open while searching.
pub fn catalog_groups(
    ui: &mut egui::Ui,
    search: &str,
    mut row: impl FnMut(&mut egui::Ui, &Target),
) {
    let searching = !search.trim().is_empty();
    let mut found = false;
    for group in TargetGroup::ALL {
        let targets: Vec<&Target> = Catalog::builtin()
            .targets
            .iter()
            .filter(|target| target.group() == group && target.matches(search))
            .collect();
        if targets.is_empty() {
            continue;
        }
        found = true;
        egui::CollapsingHeader::new(
            RichText::new(format!("{} ({})", tr_data(group.label()), targets.len())).strong(),
        )
        .id_salt(("target-group", group.label()))
        .open(searching.then_some(true))
        .show(ui, |ui| {
            for target in targets {
                row(ui, target);
            }
        });
    }
    if !found {
        ui.weak(tr("Nothing in the catalog matches."));
    }
}

/// A target's zone and energy, for list rows.
fn summary(ui: &egui::Ui, target: &Target, hunting: bool) -> String {
    let u = units::prefs(ui.ctx());
    let zone = u.format(Quantity::ShortLength, target.vital_diameter_cm);
    match target.min_energy_j {
        Some(energy) if target.is_game() && hunting => {
            format!("{zone} · {}", u.format(Quantity::Energy, energy))
        }
        _ => zone,
    }
}

/// Whether the details and the drawing fit side by side.
fn wide(ui: &egui::Ui) -> bool {
    ui.available_width() >= 760.0
}

/// The drawing's size: a fixed box when side by side, else the width.
fn preview_size(ui: &egui::Ui) -> egui::Vec2 {
    if wide(ui) {
        vec2(360.0, 260.0)
    } else {
        let width = ui.available_width().min(420.0);
        vec2(width, width * 0.72)
    }
}

/// A catalog target, read-only. Returns true when the hunter asks for
/// their own copy.
fn catalog_view(ui: &mut egui::Ui, target: &Target, data: &Data, region: Option<&str>) -> bool {
    let country = data.country;
    ui.heading(target_name(target));
    ui.weak(match target.kind {
        TargetKind::Game => tr("Game, from the catalog"),
        TargetKind::Sport => tr("Sport target, from the catalog"),
    });
    ui.add_space(6.0);
    let size = preview_size(ui);
    let details = |ui: &mut egui::Ui| {
        facts(ui, target, country, data.hunting);
        ui.add_space(6.0);
        if !target.notes.is_empty() {
            ui.label(&target.notes);
            ui.add_space(6.0);
        }
        if target.is_game()
            && let Some(country) = country
        {
            crate::hunting::species_section(ui, country, &target.id, region);
            ui.add_space(6.0);
        }
        ui.button(tr("Make my own copy"))
            .on_hover_text(tr("An editable copy under My targets"))
            .clicked()
    };
    if wide(ui) {
        ui.horizontal_top(|ui| {
            let copy = ui
                .vertical(|ui| {
                    ui.set_max_width(360.0);
                    details(ui)
                })
                .inner;
            ui.add_space(16.0);
            ui.vertical(|ui| preview(ui, target, size));
            copy
        })
        .inner
    } else {
        preview(ui, target, size);
        ui.add_space(8.0);
        details(ui)
    }
}

fn facts(ui: &mut egui::Ui, target: &Target, country: Option<Country>, hunting: bool) {
    let u = units::prefs(ui.ctx());
    let short = |cm: f64| u.format(Quantity::ShortLength, cm);
    grid(ui, "target-facts", |ui| {
        if !target.other_names.is_empty() {
            ui.label(tr("Also called"));
            ui.label(target.other_names.join(", "));
            ui.end_row();
        }
        ui.label(tr("Group"));
        ui.label(tr_data(target.group().label()));
        ui.end_row();
        match &target.shape {
            TargetShape::Animal {
                silhouette,
                shoulder_height_cm,
                body_length_cm,
                ..
            } => {
                let drawing = Catalog::builtin().drawing(silhouette);
                ui.label(tr("Drawn as"));
                ui.label(&drawing.name);
                ui.end_row();
                let (height_label, length_label) = size_labels(drawing);
                ui.label(height_label);
                ui.label(short(*shoulder_height_cm));
                ui.end_row();
                ui.label(length_label);
                ui.label(short(*body_length_cm));
                ui.end_row();
            }
            TargetShape::Rings {
                count, black_mm, ..
            } => {
                let rings = target.shape.ring_diameters_mm();
                ui.label(tr("Rings"));
                ui.label(trf!(
                    "{}, from {} mm to {} mm",
                    count,
                    rings.first().copied().unwrap_or(0.0),
                    rings.last().copied().unwrap_or(0.0)
                ));
                ui.end_row();
                ui.label(tr("Black"));
                ui.label(format!("{black_mm} mm"));
                ui.end_row();
            }
            TargetShape::Plate { diameter_mm } => {
                ui.label(tr("Plate"));
                ui.label(format!("{diameter_mm} mm"));
                ui.end_row();
            }
        }
        ui.label(if target.is_game() && hunting {
            tr("Vital zone")
        } else {
            tr("Hit zone")
        });
        ui.label(short(target.vital_diameter_cm));
        ui.end_row();
        if target.is_game() && hunting {
            ui.label(tr("Minimum energy"));
            ui.label(target.min_energy_j.map_or(tr("none").to_owned(), |energy| {
                u.format(Quantity::Energy, energy)
            }));
            ui.end_row();
            ui.label(tr("Minimum velocity"));
            ui.label(match target.min_velocity_mps {
                Some(velocity) if velocity > 0.0 => u.format(Quantity::Speed, velocity),
                Some(_) => tr("none (no expansion needed)").to_owned(),
                None => tr("the bullet's or your general one").to_owned(),
            });
            ui.end_row();
            if let Some(country) = country {
                let rules = country.rules();
                ui.horizontal(|ui| {
                    crate::flags::show(ui, country, 11.0);
                    ui.label(trf!("{} rules", i18n::country_adjective(country)))
                        .on_hover_text(i18n::rules_help(rules));
                });
                let mut text = match target.ammunition_level(country) {
                    Some(level) => {
                        trf!(
                            "Rifle: {}",
                            i18n::requirement_text(rules, level).to_lowercase()
                        )
                    }
                    None => tr("No rifle level known").to_owned(),
                };
                if let Some(notes) = target.rules_notes.get(&country) {
                    text.push_str(". ");
                    text.push_str(notes);
                }
                ui.add(egui::Label::new(text).wrap())
                    .on_hover_text(i18n::rules_help(rules));
                ui.end_row();
            }
        }
    });
}

/// The editor for the hunter's own target. Returns true to close it.
fn editor(ui: &mut egui::Ui, target: &mut Target, data: &Data, actions: &mut Vec<Action>) -> bool {
    let stored = data.targets.iter().any(|stored| stored.id == target.id);
    ui.heading(if stored {
        tr("Edit target")
    } else {
        tr("New target")
    });
    if let Some(source) = target
        .catalog_id
        .as_deref()
        .and_then(|id| Catalog::builtin().target(id))
    {
        ui.weak(trf!("Copied from the catalog's {}", target_name(source)));
    }
    ui.add_space(4.0);
    let size = preview_size(ui);
    if wide(ui) {
        ui.horizontal_top(|ui| {
            let close = ui
                .vertical(|ui| {
                    ui.set_max_width(420.0);
                    editor_fields(ui, target, stored, data, actions)
                })
                .inner;
            ui.add_space(16.0);
            ui.vertical(|ui| preview(ui, target, size));
            close
        })
        .inner
    } else {
        preview(ui, target, size);
        ui.add_space(8.0);
        editor_fields(ui, target, stored, data, actions)
    }
}

/// The editor's fields and buttons. Returns true to close the editor.
fn editor_fields(
    ui: &mut egui::Ui,
    target: &mut Target,
    stored: bool,
    data: &Data,
    actions: &mut Vec<Action>,
) -> bool {
    fields(ui, target, data.country, data.hunting);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(tr("Save")).clicked() {
            actions.push(Action::SaveTarget(target.clone()));
        }
        if stored && confirm_button(ui, "delete-target", tr("Delete")) {
            actions.push(Action::DeleteTarget(target.id.clone()));
        }
        ui.button(tr("Close")).clicked()
    })
    .inner
}

fn fields(ui: &mut egui::Ui, target: &mut Target, country: Option<Country>, hunting: bool) {
    grid(ui, "target-editor", |ui| {
        text(ui, tr("Name"), &mut target.name);
        ui.label(tr("Kind"));
        ui.horizontal(|ui| {
            let before = target.kind;
            ui.selectable_value(&mut target.kind, TargetKind::Game, tr("Game"));
            ui.selectable_value(&mut target.kind, TargetKind::Sport, tr("Sport"));
            if target.kind != before {
                target.min_energy_j = match target.kind {
                    TargetKind::Game => Some(1400.0),
                    TargetKind::Sport => None,
                };
            }
        });
        ui.end_row();

        ui.label(tr("Group"));
        let mut group = target.group();
        egui::ComboBox::from_id_salt("target-group")
            .selected_text(tr_data(group.label()))
            .show_ui(ui, |ui| {
                for option in TargetGroup::ALL {
                    ui.selectable_value(&mut group, option, tr_data(option.label()));
                }
            });
        target.group = Some(group);
        ui.end_row();

        ui.label(tr("Shape"));
        ui.horizontal(|ui| {
            let current = shape_name(&target.shape);
            for (name, label) in [
                ("Animal", tr("Animal")),
                ("Rings", tr("Rings")),
                ("Plate", tr("Plate")),
            ] {
                if ui.selectable_label(current == name, label).clicked() && current != name {
                    target.shape = default_shape(name);
                    if let TargetShape::Plate { diameter_mm } = target.shape {
                        target.vital_diameter_cm = diameter_mm / 10.0;
                    }
                }
            }
        });
        ui.end_row();

        match &mut target.shape {
            TargetShape::Animal {
                silhouette,
                shoulder_height_cm,
                body_length_cm,
                antlers,
            } => {
                let drawing = Catalog::builtin().drawing(silhouette);
                ui.label(tr("Drawn as"));
                egui::ComboBox::from_id_salt("silhouette")
                    .selected_text(&drawing.name)
                    .height(320.0)
                    .show_ui(ui, |ui| {
                        let mut drawings: Vec<&Drawing> =
                            Catalog::builtin().drawings.iter().collect();
                        drawings.sort_by(|a, b| a.name.cmp(&b.name));
                        for option in drawings {
                            if ui
                                .selectable_label(option.id == drawing.id, &option.name)
                                .clicked()
                            {
                                *silhouette = option.id.clone();
                            }
                        }
                    });
                ui.end_row();
                let (height_label, length_label) = size_labels(drawing);
                quantity(
                    ui,
                    height_label,
                    shoulder_height_cm,
                    Quantity::ShortLength,
                    10.0..=400.0,
                    1.0,
                );
                quantity(
                    ui,
                    length_label,
                    body_length_cm,
                    Quantity::ShortLength,
                    10.0..=600.0,
                    1.0,
                );
                if drawing.has_antlers() {
                    ui.label(tr("Antlers"));
                    ui.checkbox(antlers, "");
                    ui.end_row();
                }
            }
            TargetShape::Rings {
                inner_mm,
                step_mm,
                count,
                black_mm,
            } => {
                number(ui, tr("Innermost ring"), inner_mm, 0.1..=2000.0, 0.1, " mm");
                number(
                    ui,
                    tr("Each ring wider by"),
                    step_mm,
                    0.0..=1000.0,
                    0.1,
                    " mm",
                );
                ui.label(tr("Rings"));
                ui.add(egui::DragValue::new(count).range(1..=20));
                ui.end_row();
                number(ui, tr("Black"), black_mm, 0.0..=5000.0, 0.5, " mm");
            }
            TargetShape::Plate { diameter_mm } => {
                let mut diameter_cm = *diameter_mm / 10.0;
                quantity(
                    ui,
                    tr("Plate diameter"),
                    &mut diameter_cm,
                    Quantity::ShortLength,
                    1.0..=200.0,
                    0.5,
                );
                *diameter_mm = diameter_cm * 10.0;
                // A plate is hit anywhere on it.
                target.vital_diameter_cm = diameter_cm;
            }
        }

        if !matches!(target.shape, TargetShape::Plate { .. }) {
            quantity(
                ui,
                if target.is_game() && hunting {
                    tr("Vital zone diameter")
                } else {
                    tr("Hit zone diameter")
                },
                &mut target.vital_diameter_cm,
                Quantity::ShortLength,
                1.0..=200.0,
                0.5,
            );
        }
        if target.is_game() && hunting {
            optional_quantity(
                ui,
                tr("Minimum energy"),
                &mut target.min_energy_j,
                1400.0,
                Quantity::Energy,
                0.0..=8000.0,
                10.0,
            );
            optional_quantity(
                ui,
                tr("Own minimum velocity"),
                &mut target.min_velocity_mps,
                0.0,
                Quantity::Speed,
                0.0..=1500.0,
                5.0,
            );
            if let Some(country) = country {
                let rules = country.rules();
                ui.horizontal(|ui| {
                    crate::flags::show(ui, country, 11.0);
                    ui.label(trf!("{} rifle level", i18n::country_adjective(country)))
                        .on_hover_text(i18n::rules_help(rules));
                });
                let mut level = target.ammunition_level(country);
                egui::ComboBox::from_id_salt("ammunition-level")
                    .selected_text(level.map_or(tr("None").to_owned(), |level| {
                        i18n::requirement_text(rules, level)
                    }))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut level, None, tr("None"));
                        for option in rules.levels {
                            ui.selectable_value(
                                &mut level,
                                Some(option.number),
                                i18n::requirement_text(rules, option.number),
                            )
                            .on_hover_text(tr_data(option.needs));
                        }
                    });
                if level != target.ammunition_level(country) {
                    target.set_ammunition_level(country, level);
                }
                ui.end_row();
            }
        }
        ui.label(tr("Notes"));
        ui.text_edit_multiline(&mut target.notes);
        ui.end_row();
    });
}

/// What an animal's two sizes measure.
fn size_labels(drawing: &Drawing) -> (&'static str, &'static str) {
    if drawing.bird {
        (tr("Height to the back"), tr("Beak to tail"))
    } else if drawing.shoulder_height_m < 0.45 {
        (tr("Height to the back"), tr("Body length"))
    } else {
        (tr("Shoulder height"), tr("Body length"))
    }
}

fn shape_name(shape: &TargetShape) -> &'static str {
    match shape {
        TargetShape::Animal { .. } => "Animal",
        TargetShape::Rings { .. } => "Rings",
        TargetShape::Plate { .. } => "Plate",
    }
}

fn default_shape(name: &str) -> TargetShape {
    match name {
        "Rings" => TargetShape::Rings {
            inner_mm: 10.0,
            step_mm: 10.0,
            count: 10,
            black_mm: 60.0,
        },
        "Plate" => TargetShape::Plate { diameter_mm: 200.0 },
        _ => TargetShape::Animal {
            silhouette: "roe-deer".to_owned(),
            shoulder_height_cm: 100.0,
            body_length_cm: 170.0,
            antlers: true,
        },
    }
}

/// The target drawn to scale on a patch of ground, with its height.
fn preview(ui: &mut egui::Ui, target: &Target, size: egui::Vec2) {
    let figure = Figure::new(target, target.vital_diameter_cm);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, SKY);
    let [low, high] = figure.bounds;
    let (width, height) = ((high[0] - low[0]).max(1e-3), (high[1] - low[1]).max(1e-3));
    let margin = 24.0;
    let scale = ((rect.width() - 2.0 * margin) as f64 / width)
        .min((rect.height() - 2.0 * margin) as f64 / height);
    let origin = pos2(
        rect.center().x - ((low[0] + high[0]) / 2.0 * scale) as f32,
        rect.center().y + ((low[1] + high[1]) / 2.0 * scale) as f32,
    );
    let at = |[x, y]: [f64; 2]| -> Pos2 {
        pos2(origin.x + (x * scale) as f32, origin.y - (y * scale) as f32)
    };
    let ground = at([0.0, -figure.aim_height]).y;
    painter.rect_filled(
        egui::Rect::from_min_max(pos2(rect.left(), ground), rect.max),
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        },
        FIELD,
    );
    for part in &figure.parts {
        painter.add(part.shape(part.points.iter().map(|&point| at(point)).collect()));
    }
    let u = units::prefs(ui.ctx());
    painter.text(
        rect.left_top() + vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        trf!(
            "{} tall · drawn to scale",
            u.format(Quantity::ShortLength, height * 100.0)
        ),
        egui::FontId::proportional(11.5),
        theme::FOREST_900,
    );
    ui.add_space(2.0);
    let key = match (figure.zones.is_empty(), figure.rings.is_some()) {
        (true, _) => tr("Green: where a shot must land."),
        (false, true) => {
            tr("Green: the hit area, where a shot must land, with the moose test figure's rings.")
        }
        (false, false) => tr("Green: the hit area, where a shot must land."),
    };
    ui.label(RichText::new(key).small());
}

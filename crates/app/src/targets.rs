//! The Targets page: the catalog's game animals and sport targets, and the
//! hunter's own, with an editor and a drawing to scale.

use ballistics_store::{Catalog, Silhouette, Target, TargetKind, TargetShape};
use eframe::egui::{self, Color32, Pos2, RichText, Shape, Stroke, pos2, vec2};

use crate::app::{Action, Data};
use crate::figure::{Figure, has_antlers};
use crate::theme;
use crate::units::{self, Quantity, optional_quantity, quantity};
use crate::widgets::{confirm_button, grid, number, text};

const SKY: Color32 = Color32::from_rgb(0xC9, 0xD2, 0xBE);
const FIELD: Color32 = Color32::from_rgb(0xA9, 0xB8, 0x98);

enum Showing {
    Nothing,
    Catalog(String),
    /// One of the hunter's targets (or a new one) being edited.
    Custom(Target),
}

pub struct TargetsPage {
    showing: Showing,
}

impl Default for TargetsPage {
    fn default() -> Self {
        Self {
            showing: Showing::Nothing,
        }
    }
}

impl TargetsPage {
    /// Keeps the editor on the target that was just saved.
    pub fn saved(&mut self, target: &Target) {
        if matches!(&self.showing, Showing::Custom(editing) if editing.id == target.id) {
            self.showing = Showing::Custom(target.clone());
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if matches!(&self.showing, Showing::Custom(editing) if editing.id == id) {
            self.showing = Showing::Nothing;
        }
    }

    pub fn show(&mut self, context: &egui::Context, data: &Data) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::SidePanel::left("target-list")
            .resizable(true)
            .default_width(260.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data));
            });
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match &mut self.showing {
                Showing::Nothing => {
                    ui.weak("Choose a target, or make your own.");
                }
                Showing::Catalog(id) => {
                    if let Some(target) = Catalog::builtin().target(id)
                        && catalog_view(ui, target)
                    {
                        self.showing = Showing::Custom(target.to_custom());
                    }
                }
                Showing::Custom(target) => {
                    if editor(ui, target, data, &mut actions) {
                        self.showing = Showing::Nothing;
                    }
                }
            });
        });
        actions
    }

    fn list(&mut self, ui: &mut egui::Ui, data: &Data) {
        ui.heading("My targets");
        if ui.button("➕ New target").clicked() {
            self.showing = Showing::Custom(Target::new_custom());
        }
        ui.add_space(4.0);
        for target in &data.targets {
            let selected =
                matches!(&self.showing, Showing::Custom(editing) if editing.id == target.id);
            if ui.selectable_label(selected, &target.name).clicked() {
                self.showing = Showing::Custom(target.clone());
            }
        }
        if data.targets.is_empty() {
            ui.weak("None yet: copy one below or make a new one.");
        }
        for (title, kind) in [("Game", TargetKind::Game), ("Sport", TargetKind::Sport)] {
            ui.separator();
            ui.heading(title);
            for target in Catalog::builtin().targets.iter().filter(|t| t.kind == kind) {
                let selected = matches!(&self.showing, Showing::Catalog(id) if *id == target.id);
                if ui.selectable_label(selected, &target.name).clicked() {
                    self.showing = Showing::Catalog(target.id.clone());
                }
            }
        }
    }
}

/// A catalog target, read-only. Returns true when the hunter asks for
/// their own copy.
fn catalog_view(ui: &mut egui::Ui, target: &Target) -> bool {
    ui.heading(&target.name);
    ui.weak(match target.kind {
        TargetKind::Game => "Game, from the catalog",
        TargetKind::Sport => "Sport target, from the catalog",
    });
    ui.add_space(6.0);
    let mut copy = false;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(360.0);
            facts(ui, target);
            ui.add_space(6.0);
            if !target.notes.is_empty() {
                ui.label(&target.notes);
                ui.add_space(6.0);
            }
            copy = ui
                .button("Make my own copy")
                .on_hover_text("An editable copy under My targets")
                .clicked();
        });
        ui.add_space(16.0);
        ui.vertical(|ui| preview(ui, target, vec2(360.0, 260.0)));
    });
    copy
}

fn facts(ui: &mut egui::Ui, target: &Target) {
    let u = units::prefs(ui.ctx());
    let short = |cm: f64| u.format(Quantity::ShortLength, cm);
    grid(ui, "target-facts", |ui| {
        match &target.shape {
            TargetShape::Animal {
                silhouette,
                shoulder_height_cm,
                body_length_cm,
                ..
            } => {
                ui.label("Drawn as");
                ui.label(silhouette.label());
                ui.end_row();
                let (height_label, length_label) = size_labels(*silhouette);
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
                ui.label("Rings");
                ui.label(format!(
                    "{count}, from {} mm to {} mm",
                    rings.first().copied().unwrap_or(0.0),
                    rings.last().copied().unwrap_or(0.0)
                ));
                ui.end_row();
                ui.label("Black");
                ui.label(format!("{black_mm} mm"));
                ui.end_row();
            }
            TargetShape::Plate { diameter_mm } => {
                ui.label("Plate");
                ui.label(format!("{diameter_mm} mm"));
                ui.end_row();
            }
        }
        ui.label(if target.is_game() {
            "Vital zone"
        } else {
            "Hit zone"
        });
        ui.label(short(target.vital_diameter_cm));
        ui.end_row();
        if target.is_game() {
            ui.label("Minimum energy");
            ui.label(target.min_energy_j.map_or("none".to_owned(), |energy| {
                u.format(Quantity::Energy, energy)
            }));
            ui.end_row();
            ui.label("Minimum velocity");
            ui.label(match target.min_velocity_mps {
                Some(velocity) if velocity > 0.0 => u.format(Quantity::Speed, velocity),
                Some(_) => "none (no expansion needed)".to_owned(),
                None => "the bullet's or your general one".to_owned(),
            });
            ui.end_row();
        }
    });
}

/// The editor for the hunter's own target. Returns true to close it.
fn editor(ui: &mut egui::Ui, target: &mut Target, data: &Data, actions: &mut Vec<Action>) -> bool {
    let stored = data.targets.iter().any(|stored| stored.id == target.id);
    ui.heading(if stored { "Edit target" } else { "New target" });
    if let Some(source) = target
        .catalog_id
        .as_deref()
        .and_then(|id| Catalog::builtin().target(id))
    {
        ui.weak(format!("Copied from the catalog's {}", source.name));
    }
    ui.add_space(4.0);
    let mut close = false;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(420.0);
            fields(ui, target);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    actions.push(Action::SaveTarget(target.clone()));
                }
                if stored && confirm_button(ui, "delete-target", "Delete") {
                    actions.push(Action::DeleteTarget(target.id.clone()));
                }
                close = ui.button("Close").clicked();
            });
        });
        ui.add_space(16.0);
        ui.vertical(|ui| preview(ui, target, vec2(360.0, 260.0)));
    });
    close
}

fn fields(ui: &mut egui::Ui, target: &mut Target) {
    grid(ui, "target-editor", |ui| {
        text(ui, "Name", &mut target.name);
        ui.label("Kind");
        ui.horizontal(|ui| {
            let before = target.kind;
            ui.selectable_value(&mut target.kind, TargetKind::Game, "Game");
            ui.selectable_value(&mut target.kind, TargetKind::Sport, "Sport");
            if target.kind != before {
                target.min_energy_j = match target.kind {
                    TargetKind::Game => Some(1400.0),
                    TargetKind::Sport => None,
                };
            }
        });
        ui.end_row();

        ui.label("Shape");
        ui.horizontal(|ui| {
            let current = shape_name(&target.shape);
            for name in ["Animal", "Rings", "Plate"] {
                if ui.selectable_label(current == name, name).clicked() && current != name {
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
                ui.label("Drawn as");
                egui::ComboBox::from_id_salt("silhouette")
                    .selected_text(silhouette.label())
                    .show_ui(ui, |ui| {
                        for option in Silhouette::ALL {
                            ui.selectable_value(silhouette, option, option.label());
                        }
                    });
                ui.end_row();
                let (height_label, length_label) = size_labels(*silhouette);
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
                if has_antlers(*silhouette) {
                    ui.label("Antlers");
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
                number(ui, "Innermost ring", inner_mm, 0.1..=2000.0, 0.1, " mm");
                number(ui, "Each ring wider by", step_mm, 0.0..=1000.0, 0.1, " mm");
                ui.label("Rings");
                ui.add(egui::DragValue::new(count).range(1..=20));
                ui.end_row();
                number(ui, "Black", black_mm, 0.0..=5000.0, 0.5, " mm");
            }
            TargetShape::Plate { diameter_mm } => {
                let mut diameter_cm = *diameter_mm / 10.0;
                quantity(
                    ui,
                    "Plate diameter",
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
                if target.is_game() {
                    "Vital zone diameter"
                } else {
                    "Hit zone diameter"
                },
                &mut target.vital_diameter_cm,
                Quantity::ShortLength,
                1.0..=200.0,
                0.5,
            );
        }
        if target.is_game() {
            optional_quantity(
                ui,
                "Minimum energy",
                &mut target.min_energy_j,
                1400.0,
                Quantity::Energy,
                0.0..=8000.0,
                10.0,
            );
            optional_quantity(
                ui,
                "Own minimum velocity",
                &mut target.min_velocity_mps,
                0.0,
                Quantity::Speed,
                0.0..=1500.0,
                5.0,
            );
        }
        ui.label("Notes");
        ui.text_edit_multiline(&mut target.notes);
        ui.end_row();
    });
}

/// What an animal's two sizes measure.
fn size_labels(silhouette: Silhouette) -> (&'static str, &'static str) {
    match silhouette {
        bird if bird.is_bird() => ("Height to the back", "Beak to tail"),
        Silhouette::Hare => ("Height to the back", "Body length"),
        _ => ("Shoulder height", "Body length"),
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
            silhouette: Silhouette::Deer,
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
        let points = part.points.iter().map(|&point| at(point)).collect();
        let outline = part
            .stroke
            .map_or(Stroke::NONE, |colour| Stroke::new(1.0_f32, colour));
        painter.add(Shape::convex_polygon(points, part.fill, outline));
    }
    let u = units::prefs(ui.ctx());
    painter.text(
        rect.left_top() + vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{} tall · drawn to scale",
            u.format(Quantity::ShortLength, height * 100.0)
        ),
        egui::FontId::proportional(11.5),
        theme::FOREST_900,
    );
    ui.add_space(2.0);
    ui.label(RichText::new("Green: where a shot must land.").small());
}

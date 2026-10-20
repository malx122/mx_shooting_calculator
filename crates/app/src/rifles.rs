//! The Rifles page: the hunter's rifles and own scopes, with an editor for
//! each.

use ballistics_store::{
    AngleUnit, Catalog, ClickUnit, FocalPlane, PLAIN_CROSSHAIR, Reticle, Rifle, TableSettings,
    UserScope,
};
use eframe::egui;

use crate::app::{Action, Data};
use crate::mobile;
use crate::sight_picture::reticle_preview;
use crate::units::{self, Quantity, optional_quantity, quantity};
use crate::widgets::{
    ScopeChoice, caliber_name, caliber_picker, confirm_button, grid, number, optional_number,
    reticle_name, scope_picker, scope_summary, text,
};

enum Editing {
    Nothing,
    Rifle(Rifle),
    Scope(UserScope),
}

pub struct RiflesPage {
    editing: Editing,
    caliber_search: String,
    scope_search: String,
}

impl Default for RiflesPage {
    fn default() -> Self {
        Self {
            editing: Editing::Nothing,
            caliber_search: String::new(),
            scope_search: String::new(),
        }
    }
}

impl RiflesPage {
    pub fn edit(&mut self, rifle: &Rifle) {
        self.editing = Editing::Rifle(rifle.clone());
    }

    /// Keeps the editor on the record that was just saved.
    pub fn saved(&mut self, rifle: &Rifle) {
        if matches!(&self.editing, Editing::Rifle(editing) if editing.id == rifle.id) {
            self.editing = Editing::Rifle(rifle.clone());
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if matches!(&self.editing, Editing::Rifle(editing) if editing.id == id) {
            self.editing = Editing::Nothing;
        }
    }

    pub fn scope_saved(&mut self, scope: &UserScope) {
        if matches!(&self.editing, Editing::Scope(editing) if editing.id == scope.id) {
            self.editing = Editing::Scope(scope.clone());
        }
    }

    pub fn scope_deleted(&mut self, id: &str) {
        match &mut self.editing {
            Editing::Scope(editing) if editing.id == id => self.editing = Editing::Nothing,
            // An open rifle draft loses the scope too, as the saved rifles did.
            Editing::Rifle(rifle) if rifle.scope_id.as_deref() == Some(id) => {
                rifle.scope_id = None;
                rifle.reticle_id = None;
            }
            _ => {}
        }
    }

    pub fn show(&mut self, context: &egui::Context, data: &Data) -> Vec<Action> {
        let mut actions = Vec::new();
        if mobile::is_narrow(context) {
            // On a phone: the list, or one editor with a way back.
            egui::CentralPanel::default().show(context, |ui| {
                if matches!(self.editing, Editing::Nothing) {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| self.phone_list(ui, data, &mut actions));
                } else {
                    let back = mobile::back_button(ui, "Rifles");
                    let cancel = egui::ScrollArea::vertical()
                        .show(ui, |ui| self.editor(ui, data, &mut actions))
                        .inner;
                    if back || cancel {
                        self.editing = Editing::Nothing;
                    }
                }
            });
            return actions;
        }
        egui::SidePanel::left("rifle-list")
            .resizable(true)
            .default_width(280.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data, &mut actions));
            });
        egui::CentralPanel::default().show(context, |ui| {
            let cancel = egui::ScrollArea::vertical()
                .show(ui, |ui| self.editor(ui, data, &mut actions))
                .inner;
            if cancel {
                self.editing = Editing::Nothing;
            }
        });
        actions
    }

    /// The open editor. Returns true when the hunter cancels a new record.
    fn editor(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) -> bool {
        match &mut self.editing {
            Editing::Nothing => {
                ui.weak("Choose a rifle or scope, or add a new one.");
                false
            }
            Editing::Rifle(rifle) => rifle_editor(
                ui,
                rifle,
                data,
                &mut self.caliber_search,
                &mut self.scope_search,
                actions,
            ),
            Editing::Scope(scope) => scope_editor(ui, scope, data, actions),
        }
    }

    /// The phone's list: big rows for rifles and own scopes.
    fn phone_list(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        ui.heading("Rifles");
        if ui.button("+ New rifle").clicked() {
            self.new_rifle();
        }
        for rifle in &data.rifles {
            let scope = rifle
                .scope_id
                .as_deref()
                .and_then(|id| data.scope(id))
                .map(|scope| format!(" · {}", scope.display_name()))
                .unwrap_or_default();
            let detail = format!("{}{scope}", caliber_name(&rifle.caliber));
            if mobile::list_row(ui, &rifle.name, &detail, false).clicked() {
                self.editing = Editing::Rifle(rifle.clone());
            }
        }
        if data.rifles.is_empty() {
            ui.weak("No rifles yet.");
            if ui
                .button("Load demo data")
                .on_hover_text("Three rifles with bullets, a scope and a target, to try the app")
                .clicked()
            {
                actions.push(Action::LoadDemo);
            }
        }
        ui.add_space(12.0);
        ui.heading("My scopes");
        ui.weak("For scopes that aren't in the catalog.");
        if ui.button("+ New scope").clicked() {
            self.editing = Editing::Scope(UserScope::new());
        }
        for scope in &data.scopes {
            if mobile::list_row(ui, &scope.display_name(), "", false).clicked() {
                self.editing = Editing::Scope(scope.clone());
            }
        }
    }

    fn new_rifle(&mut self) {
        let caliber = Catalog::builtin()
            .caliber("308-win")
            .expect("308 Win is in the catalog");
        self.editing = Editing::Rifle(Rifle::new("", &caliber.key, caliber.typical_twist_in));
    }

    fn list(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        ui.heading("Rifles");
        if ui.button("➕ New rifle").clicked() {
            self.new_rifle();
        }
        ui.add_space(4.0);
        for rifle in &data.rifles {
            let selected =
                matches!(&self.editing, Editing::Rifle(editing) if editing.id == rifle.id);
            let label = format!("{}\n{}", rifle.name, caliber_name(&rifle.caliber));
            if ui.selectable_label(selected, label).clicked() {
                self.editing = Editing::Rifle(rifle.clone());
            }
        }
        if data.rifles.is_empty() {
            ui.weak("No rifles yet.");
            if ui
                .button("Load demo data")
                .on_hover_text("Three rifles with bullets, a scope and a target, to try the app")
                .clicked()
            {
                actions.push(Action::LoadDemo);
            }
        }

        ui.separator();
        ui.heading("My scopes");
        ui.weak("For scopes that aren't in the catalog.");
        if ui.button("➕ New scope").clicked() {
            self.editing = Editing::Scope(UserScope::new());
        }
        ui.add_space(4.0);
        for scope in &data.scopes {
            let selected =
                matches!(&self.editing, Editing::Scope(editing) if editing.id == scope.id);
            if ui
                .selectable_label(selected, scope.display_name())
                .clicked()
            {
                self.editing = Editing::Scope(scope.clone());
            }
        }
    }
}

/// The rifle editor. Returns true when the hunter cancels a new rifle.
fn rifle_editor(
    ui: &mut egui::Ui,
    rifle: &mut Rifle,
    data: &Data,
    caliber_search: &mut String,
    scope_search: &mut String,
    actions: &mut Vec<Action>,
) -> bool {
    let stored = data.rifles.iter().find(|stored| stored.id == rifle.id);
    ui.heading(if stored.is_some() {
        "Edit rifle"
    } else {
        "New rifle"
    });
    ui.add_space(4.0);
    rifle_fields(ui, rifle, data, caliber_search, scope_search);
    ui.add_space(8.0);

    let problem = rifle.validate().err();
    let changed = stored != Some(&*rifle);
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(problem.is_none() && changed, egui::Button::new("💾 Save"))
            .clicked()
        {
            actions.push(Action::SaveRifle(rifle.clone()));
        }
        if let Some(stored) = stored {
            if changed && ui.button("Undo changes").clicked() {
                *rifle = stored.clone();
            }
            if confirm_button(ui, "delete-rifle", "Delete") {
                actions.push(Action::DeleteRifle(stored.id.clone()));
            }
        } else if ui.button("Cancel").clicked() {
            cancel = true;
        }
    });
    if let Some(problem) = problem {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
    } else if changed {
        ui.weak("Unsaved changes");
    }

    if stored.is_some() {
        ui.separator();
        let count = data
            .bullets
            .iter()
            .filter(|b| b.caliber == rifle.caliber)
            .count();
        ui.horizontal(|ui| {
            ui.label(format!(
                "{count} bullet(s) in your collection for {}.",
                caliber_name(&rifle.caliber)
            ));
            if ui.button("Show bullets").clicked() {
                actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
            }
        });
    }
    cancel
}

fn rifle_fields(
    ui: &mut egui::Ui,
    rifle: &mut Rifle,
    data: &Data,
    caliber_search: &mut String,
    scope_search: &mut String,
) {
    grid(ui, "rifle-editor", |ui| {
        text(ui, "Name", &mut rifle.name);
        ui.label("Caliber");
        if let Some(caliber) = caliber_picker(ui, "rifle-caliber", &rifle.caliber, caliber_search) {
            rifle.caliber = caliber.key.clone();
            rifle.twist_in = caliber.typical_twist_in;
        }
        ui.end_row();
        optional_quantity(
            ui,
            "Barrel length",
            &mut rifle.barrel_length_mm,
            560.0,
            Quantity::BarrelLength,
            200.0..=1000.0,
            1.0,
        );
        quantity(
            ui,
            "Twist, 1 turn in",
            &mut rifle.twist_in,
            Quantity::Twist,
            3.0..=40.0,
            0.05,
        );
        ui.label("Twist direction");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut rifle.right_hand_twist, true, "Right");
            ui.selectable_value(&mut rifle.right_hand_twist, false, "Left");
        });
        ui.end_row();
        quantity(
            ui,
            "Sight height",
            &mut rifle.sight_height_cm,
            Quantity::SightHeight,
            0.0..=15.0,
            0.1,
        );
        quantity(
            ui,
            "Zero range",
            &mut rifle.zero_range_m,
            Quantity::Distance,
            10.0..=1000.0,
            1.0,
        );
        quantity(
            ui,
            "  hitting there",
            &mut rifle.zero_height_cm,
            Quantity::ShortLength,
            -50.0..=50.0,
            0.1,
        );
        ui.label("");
        ui.weak("0 on the crosshair; + high, − low (\"4 cm high at 100 m\")");
        ui.end_row();
        quantity(
            ui,
            "Zeroed at",
            &mut rifle.zero_temperature_c,
            Quantity::Temperature,
            -40.0..=50.0,
            0.5,
        );
        quantity(
            ui,
            "  station pressure",
            &mut rifle.zero_pressure_hpa,
            Quantity::Pressure,
            500.0..=1100.0,
            1.0,
        );
        number(
            ui,
            "  humidity",
            &mut rifle.zero_humidity_percent,
            0.0..=100.0,
            1.0,
            " %",
        );
        ui.label("  powder temperature");
        ui.horizontal(|ui| {
            let mut same = rifle.zero_powder_temperature_c.is_none();
            if ui.checkbox(&mut same, "same as air").changed() {
                rifle.zero_powder_temperature_c = (!same).then_some(rifle.zero_temperature_c);
            }
            if let Some(powder) = &mut rifle.zero_powder_temperature_c {
                units::drag(ui, powder, Quantity::Temperature, -45.0..=60.0, 0.5);
            }
        });
        ui.end_row();
        scope_rows(ui, rifle, data, scope_search);
        ui.label("Scope clicks");
        ui.horizontal(|ui| {
            let before = rifle.click_unit;
            ui.selectable_value(&mut rifle.click_unit, ClickUnit::Mil, "mil");
            ui.selectable_value(&mut rifle.click_unit, ClickUnit::Moa, "MOA");
            if before != rifle.click_unit {
                rifle.click_value = match rifle.click_unit {
                    ClickUnit::Mil => 0.1,
                    ClickUnit::Moa => 0.25,
                };
            }
            ui.add(
                egui::DragValue::new(&mut rifle.click_value)
                    .range(0.01..=1.0)
                    .speed(0.005)
                    .prefix("per click "),
            );
        });
        ui.end_row();
        ui.label("Notes");
        ui.text_edit_multiline(&mut rifle.notes);
        ui.end_row();
    });
    ui.small(
        "The twist is set to a common value for the caliber; check your barrel's actual twist.",
    );
    table_settings(ui, &mut rifle.table);
}

fn table_settings(ui: &mut egui::Ui, table: &mut TableSettings) {
    ui.add_space(8.0);
    ui.strong("Dope table");
    grid(ui, "table-range", |ui| {
        quantity(
            ui,
            "From",
            &mut table.start_m,
            Quantity::Distance,
            0.0..=2900.0,
            5.0,
        );
        quantity(
            ui,
            "To",
            &mut table.end_m,
            Quantity::Distance,
            10.0..=3000.0,
            10.0,
        );
        quantity(
            ui,
            "Every",
            &mut table.step_m,
            Quantity::Distance,
            1.0..=500.0,
            1.0,
        );
    });
    let columns = &mut table.columns;
    ui.horizontal_wrapped(|ui| {
        ui.label("Height:")
            .on_hover_text("Where a dead-on hold hits: + above the aim, − below");
        ui.checkbox(&mut columns.drop_length, "cm");
        ui.checkbox(&mut columns.drop_mil, "mil");
        ui.checkbox(&mut columns.drop_moa, "MOA");
        ui.checkbox(&mut columns.drop_clicks, "clicks");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Wind:");
        ui.checkbox(&mut columns.wind_length, "cm");
        ui.checkbox(&mut columns.wind_mil, "mil");
        ui.checkbox(&mut columns.wind_moa, "MOA");
        ui.checkbox(&mut columns.wind_clicks, "clicks");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Also:");
        ui.checkbox(&mut columns.velocity, "velocity");
        ui.checkbox(&mut columns.energy, "energy");
        ui.checkbox(&mut columns.time, "time of flight");
    });
}

/// Scope and reticle rows of the rifle editor. Choosing a scope also takes
/// over its click unit and value.
fn scope_rows(ui: &mut egui::Ui, rifle: &mut Rifle, data: &Data, search: &mut String) {
    ui.label("Scope");
    match scope_picker(
        ui,
        "rifle-scope",
        rifle.scope_id.as_deref(),
        search,
        &data.scopes,
    ) {
        Some(ScopeChoice::Mounted(scope)) => {
            rifle.scope_id = Some(scope.id.clone());
            rifle.reticle_id = scope.reticles.first().cloned();
            if let Some(clicks) = scope.rifle_clicks() {
                (rifle.click_unit, rifle.click_value) = clicks;
            }
        }
        Some(ScopeChoice::Removed) => {
            rifle.scope_id = None;
            rifle.reticle_id = None;
        }
        None => {}
    }
    ui.end_row();
    let Some(scope) = rifle.scope_id.as_deref().and_then(|id| data.scope(id)) else {
        return;
    };
    ui.label("");
    ui.weak(scope_summary(&scope));
    ui.end_row();
    ui.label("Reticle");
    egui::ComboBox::from_id_salt("rifle-reticle")
        .selected_text(
            rifle
                .reticle_id
                .as_deref()
                .map_or("Choose reticle".to_owned(), reticle_name),
        )
        .show_ui(ui, |ui| {
            for id in &scope.reticles {
                let selected = rifle
                    .reticle_id
                    .as_deref()
                    .is_some_and(|chosen| Catalog::builtin().reticle_id(chosen) == id);
                let Some(reticle) = reticle_by_id(Some(id)) else {
                    continue;
                };
                if previewed_choice(ui, selected, &reticle).clicked() {
                    rifle.reticle_id = Some(id.clone());
                }
            }
        });
    ui.end_row();
}

/// The custom scope editor. Returns true when the hunter cancels a new one.
fn scope_editor(
    ui: &mut egui::Ui,
    scope: &mut UserScope,
    data: &Data,
    actions: &mut Vec<Action>,
) -> bool {
    let stored = data.scopes.iter().find(|stored| stored.id == scope.id);
    ui.heading(if stored.is_some() {
        "Edit scope"
    } else {
        "New scope"
    });
    ui.add_space(4.0);
    grid(ui, "scope-editor", |ui| {
        text(ui, "Maker", &mut scope.manufacturer);
        text(ui, "Model", &mut scope.model);
        ui.label("Magnification");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut scope.magnification_min)
                    .range(1.0..=100.0)
                    .speed(0.1)
                    .suffix("×"),
            );
            ui.label("to");
            ui.add(
                egui::DragValue::new(&mut scope.magnification_max)
                    .range(1.0..=100.0)
                    .speed(0.1)
                    .suffix("×"),
            );
        });
        ui.end_row();
        optional_number(
            ui,
            "Objective",
            &mut scope.objective_mm,
            50.0,
            10.0..=80.0,
            1.0,
            " mm",
        );
        ui.label("Reticle position");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut scope.focal_plane,
                FocalPlane::Sfp,
                "Second focal plane",
            );
            ui.selectable_value(&mut scope.focal_plane, FocalPlane::Ffp, "First focal plane");
        });
        ui.end_row();
        if scope.focal_plane == FocalPlane::Sfp {
            ui.label("  marks true at");
            let reference = scope
                .reticle_reference_magnification
                .get_or_insert(scope.magnification_max);
            ui.add(
                egui::DragValue::new(reference)
                    .range(1.0..=100.0)
                    .speed(0.1)
                    .suffix("×"),
            );
            ui.end_row();
        }
        ui.label("Clicks");
        ui.horizontal(|ui| {
            for unit in [AngleUnit::CmAt100m, AngleUnit::Mrad, AngleUnit::Moa] {
                ui.selectable_value(&mut scope.click_unit, unit, unit.label());
            }
            ui.add(
                egui::DragValue::new(&mut scope.click_value)
                    .range(0.01..=5.0)
                    .speed(0.01)
                    .prefix("per click "),
            );
        });
        ui.end_row();
        let travel_unit = format!(" {}", scope.click_unit.label());
        optional_number(
            ui,
            "Elevation travel",
            &mut scope.elevation_travel,
            20.0,
            0.1..=500.0,
            0.5,
            &travel_unit,
        );
        ui.label("Field of view at 100 m");
        ui.horizontal(|ui| {
            let mut known = scope.fov_m_at_100m.is_some();
            if ui.checkbox(&mut known, "").changed() {
                scope.fov_m_at_100m = known.then_some([10.0, 3.0]);
            }
            match &mut scope.fov_m_at_100m {
                Some([wide, narrow]) => {
                    ui.add(
                        egui::DragValue::new(wide)
                            .range(0.1..=60.0)
                            .speed(0.1)
                            .suffix(" m"),
                    );
                    ui.label("to");
                    ui.add(
                        egui::DragValue::new(narrow)
                            .range(0.1..=60.0)
                            .speed(0.1)
                            .suffix(" m"),
                    );
                }
                None => {
                    ui.weak("unknown");
                }
            }
        });
        ui.end_row();
        ui.label("Illuminated");
        ui.checkbox(&mut scope.illuminated, "");
        ui.end_row();
        ui.label("Reticle drawing");
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt("custom-scope-reticle")
                .width(280.0)
                .height(420.0)
                .selected_text(
                    scope
                        .reticle_id
                        .as_deref()
                        .map_or("Plain crosshair".to_owned(), reticle_name),
                )
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    let search_id = ui.id().with("reticle-search");
                    let mut search: String =
                        ui.data(|data| data.get_temp(search_id)).unwrap_or_default();
                    ui.add(
                        egui::TextEdit::singleline(&mut search)
                            .hint_text("Search any reticle name"),
                    )
                    .request_focus();
                    let plain = Reticle::plain_crosshair();
                    if search.trim().is_empty()
                        && previewed_choice(ui, scope.reticle_id.is_none(), &plain).clicked()
                    {
                        scope.reticle_id = None;
                    }
                    let mut chosen_one = false;
                    for reticle in Catalog::builtin()
                        .reticles
                        .iter()
                        .filter(|reticle| reticle.matches(&search))
                    {
                        let selected = scope.reticle_id.as_deref().is_some_and(|chosen| {
                            Catalog::builtin().reticle_id(chosen) == reticle.id
                        });
                        if previewed_choice(ui, selected, reticle).clicked() {
                            scope.reticle_id = Some(reticle.id.clone());
                            chosen_one = true;
                        }
                    }
                    if chosen_one {
                        search.clear();
                        // A combo box popup, not a menu: close_menu leaves it open.
                        ui.memory_mut(|memory| memory.close_popup());
                    }
                    ui.data_mut(|data| data.insert_temp(search_id, search));
                });
            if let Some(reticle) = reticle_by_id(scope.reticle_id.as_deref()) {
                ui.add_space(4.0);
                reticle_preview(ui, &reticle, 200.0, scope.illuminated);
                ui.weak(preview_caption(&reticle));
            }
        });
        ui.end_row();
        ui.label("Notes");
        ui.text_edit_multiline(&mut scope.notes);
        ui.end_row();
    });
    ui.small(
        "A catalog reticle is drawn with the dimensions its maker published for that scope. On a second-focal-plane \
         scope they are taken to be true at the magnification above.",
    );
    ui.add_space(8.0);

    let problem = scope.validate().err();
    let changed = stored != Some(&*scope);
    let in_use = data
        .rifles
        .iter()
        .filter(|rifle| rifle.scope_id.as_deref() == Some(scope.id.as_str()))
        .count();
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(problem.is_none() && changed, egui::Button::new("💾 Save"))
            .clicked()
        {
            actions.push(Action::SaveScope(scope.clone()));
        }
        if let Some(stored) = stored {
            if changed && ui.button("Undo changes").clicked() {
                *scope = stored.clone();
            }
            if confirm_button(ui, "delete-scope", "Delete") {
                actions.push(Action::DeleteScope(stored.id.clone()));
            }
        } else if ui.button("Cancel").clicked() {
            cancel = true;
        }
    });
    if let Some(problem) = problem {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
    } else if changed {
        ui.weak("Unsaved changes");
    }
    if in_use > 0 {
        ui.weak(format!("Mounted on {in_use} rifle(s)."));
    }
    cancel
}

/// A reticle by id; no id (or the built-in id) is the plain crosshair.
fn reticle_by_id(id: Option<&str>) -> Option<Reticle> {
    match id {
        None | Some(PLAIN_CROSSHAIR) => Some(Reticle::plain_crosshair()),
        Some(id) => Catalog::builtin().reticle(id).cloned(),
    }
}

/// A reticle in a drop-down list, with a drawing of it on hover.
fn previewed_choice(ui: &mut egui::Ui, selected: bool, reticle: &Reticle) -> egui::Response {
    ui.selectable_label(selected, &reticle.name)
        .on_hover_ui(|ui| {
            reticle_preview(ui, reticle, 160.0, false);
            ui.weak(preview_caption(reticle));
            if !reticle.also_known_as.is_empty() {
                ui.weak(format!(
                    "Also sold as: {}",
                    reticle.also_known_as.join(", ")
                ));
            }
        })
}

fn preview_caption(reticle: &Reticle) -> String {
    if reticle.id == PLAIN_CROSSHAIR {
        "A fine crosshair without marks.".to_owned()
    } else {
        format!(
            "Shown at its published size, {:.1} mrad from the centre to its outermost mark.",
            reticle.inner_extent_mrad()
        )
    }
}

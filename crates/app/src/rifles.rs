//! The Rifles page: the hunter's rifles and own scopes, with an editor for
//! each, and for each rifle the bullets shot in it (their loads).

use ballistics_store::{
    AngleUnit, Catalog, ClickUnit, FocalPlane, PLAIN_CROSSHAIR, Reticle, Rifle, UserScope,
};
use eframe::egui::{self, RichText};

use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::loads::LoadEditor;
use crate::mobile;
use crate::sight_picture::reticle_preview;
use crate::units::{self, Quantity, optional_quantity, quantity};
use crate::widgets::{
    ScopeChoice, caliber_name, caliber_picker, confirm_button, fold, grid, number, optional_number,
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
    /// The bullet whose load in the open rifle is shown.
    load_bullet: Option<String>,
    loads: LoadEditor,
}

impl Default for RiflesPage {
    fn default() -> Self {
        Self {
            editing: Editing::Nothing,
            caliber_search: String::new(),
            scope_search: String::new(),
            load_bullet: None,
            loads: LoadEditor::default(),
        }
    }
}

impl RiflesPage {
    /// Opens a rifle with one of its bullets' setup showing.
    pub fn edit_load(&mut self, rifle: &Rifle, bullet_id: &str) {
        if !matches!(&self.editing, Editing::Rifle(editing) if editing.id == rifle.id) {
            self.editing = Editing::Rifle(rifle.clone());
        }
        self.load_bullet = Some(bullet_id.to_owned());
    }

    /// Drops unsaved load edits, e.g. after the load was reset.
    pub fn forget_load_draft(&mut self) {
        self.loads.forget();
    }

    /// Takes a range table saved from Calculate into an open draft.
    pub fn table_saved(&mut self, rifle: &Rifle) {
        if let Editing::Rifle(editing) = &mut self.editing
            && editing.id == rifle.id
        {
            editing.table = rifle.table;
        }
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

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Data) -> Vec<Action> {
        let context = &ui.ctx().clone();
        let mut actions = Vec::new();
        if mobile::is_narrow(context) {
            // On a phone: the list, or one editor with a way back.
            egui::CentralPanel::default().show(ui, |ui| {
                if matches!(self.editing, Editing::Nothing) {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| self.phone_list(ui, data, &mut actions));
                } else {
                    let back = mobile::back_button(ui, tr("Rifles"));
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
        egui::Panel::left("rifle-list")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data, &mut actions));
            });
        egui::CentralPanel::default().show(ui, |ui| {
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
                ui.weak(tr("Choose a rifle or scope, or add a new one."));
                false
            }
            Editing::Rifle(rifle) => rifle_editor(
                ui,
                rifle,
                data,
                &mut self.caliber_search,
                &mut self.scope_search,
                &mut self.load_bullet,
                &mut self.loads,
                actions,
            ),
            Editing::Scope(scope) => scope_editor(ui, scope, data, actions),
        }
    }

    /// The phone's list: big rows for rifles and own scopes.
    fn phone_list(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        ui.heading(tr("Rifles"));
        if ui.button(tr("+ New rifle")).clicked() {
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
            ui.weak(tr("No rifles yet."));
            if ui
                .button(tr("Load demo data"))
                .on_hover_text(tr(
                    "Three rifles with bullets, a scope and a target, to try the app",
                ))
                .clicked()
            {
                actions.push(Action::LoadDemo);
            }
        }
        ui.add_space(12.0);
        ui.heading(tr("My scopes"));
        ui.weak(tr("For scopes that aren't in the catalog."));
        if ui.button(tr("+ New scope")).clicked() {
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
        ui.heading(tr("Rifles"));
        if ui.button(tr("+ New rifle")).clicked() {
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
            ui.weak(tr("No rifles yet."));
            if ui
                .button(tr("Load demo data"))
                .on_hover_text(tr(
                    "Three rifles with bullets, a scope and a target, to try the app",
                ))
                .clicked()
            {
                actions.push(Action::LoadDemo);
            }
        }

        ui.separator();
        ui.heading(tr("My scopes"));
        ui.weak(tr("For scopes that aren't in the catalog."));
        if ui.button(tr("+ New scope")).clicked() {
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
#[allow(clippy::too_many_arguments)]
fn rifle_editor(
    ui: &mut egui::Ui,
    rifle: &mut Rifle,
    data: &Data,
    caliber_search: &mut String,
    scope_search: &mut String,
    load_bullet: &mut Option<String>,
    loads: &mut LoadEditor,
    actions: &mut Vec<Action>,
) -> bool {
    let stored = data.rifles.iter().find(|stored| stored.id == rifle.id);
    ui.heading(if stored.is_some() {
        tr("Edit rifle")
    } else {
        tr("New rifle")
    });
    ui.add_space(4.0);
    rifle_fields(ui, rifle, data, caliber_search, scope_search);
    ui.add_space(8.0);

    let problem = rifle.validate().err();
    let changed = stored != Some(&*rifle);
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                problem.is_none() && changed,
                egui::Button::new(RichText::new(tr("Save")).strong()),
            )
            .clicked()
        {
            actions.push(Action::SaveRifle(rifle.clone()));
        }
        if let Some(stored) = stored {
            if changed && ui.button(tr("Undo changes")).clicked() {
                *rifle = stored.clone();
            }
            if confirm_button(ui, "delete-rifle", tr("Delete")) {
                actions.push(Action::DeleteRifle(stored.id.clone()));
            }
        } else if ui.button(tr("Cancel")).clicked() {
            cancel = true;
        }
    });
    if let Some(problem) = problem {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
    } else if changed {
        ui.weak(tr("Unsaved changes"));
    }

    if let Some(stored) = stored {
        ui.separator();
        loads_section(ui, stored, data, load_bullet, loads, actions);
    }
    cancel
}

/// The bullets for the rifle's caliber, each with its setup in this rifle:
/// velocities, point of impact, zero weather and truing.
fn loads_section(
    ui: &mut egui::Ui,
    rifle: &Rifle,
    data: &Data,
    load_bullet: &mut Option<String>,
    loads: &mut LoadEditor,
    actions: &mut Vec<Action>,
) {
    ui.heading(tr("Loads in this rifle"));
    let bullets: Vec<_> = data
        .bullets
        .iter()
        .filter(|bullet| bullet.caliber == rifle.caliber)
        .collect();
    if bullets.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak(trf!(
                "No {} bullets in your collection yet.",
                caliber_name(&rifle.caliber)
            ));
            if ui.button(tr("Add bullets")).clicked() {
                actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
            }
        });
        return;
    }
    if load_bullet
        .as_ref()
        .is_none_or(|id| !bullets.iter().any(|bullet| &bullet.id == id))
    {
        *load_bullet = Some(bullets[0].id.clone());
    }
    let load_of = |bullet_id: &str| {
        data.loads
            .iter()
            .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet_id)
    };
    ui.horizontal_wrapped(|ui| {
        for bullet in &bullets {
            let selected = load_bullet.as_deref() == Some(bullet.id.as_str());
            let set_up = load_of(&bullet.id).is_some();
            let text = if set_up {
                format!("{} ✔", bullet.display_name())
            } else {
                bullet.display_name()
            };
            if ui
                .add(egui::Button::new(text).selected(selected))
                .on_hover_text(if set_up {
                    tr("Set up for this rifle")
                } else {
                    tr("Using the bullet's own data")
                })
                .clicked()
            {
                *load_bullet = Some(bullet.id.clone());
            }
        }
        if ui.small_button(tr("+ Add bullets")).clicked() {
            actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
        }
    });
    ui.add_space(4.0);
    let Some(bullet) = bullets
        .iter()
        .find(|bullet| load_bullet.as_deref() == Some(bullet.id.as_str()))
    else {
        return;
    };
    loads.show(ui, rifle, bullet, load_of(&bullet.id), &data.today, actions);
}

fn rifle_fields(
    ui: &mut egui::Ui,
    rifle: &mut Rifle,
    data: &Data,
    caliber_search: &mut String,
    scope_search: &mut String,
) {
    let u = units::prefs(ui.ctx());
    grid(ui, "rifle-editor", |ui| {
        text(ui, tr("Name"), &mut rifle.name);
        ui.label(tr("Caliber"));
        if let Some(caliber) = caliber_picker(ui, "rifle-caliber", &rifle.caliber, caliber_search) {
            rifle.caliber = caliber.key.clone();
            rifle.twist_in = caliber.typical_twist_in;
        }
        ui.end_row();
        scope_rows(ui, rifle, data, scope_search);
        click_rows(ui, rifle, data);
        quantity(
            ui,
            tr("Sight height"),
            &mut rifle.sight_height_cm,
            Quantity::SightHeight,
            0.0..=15.0,
            0.1,
        );
        ui.label(tr("Zero"));
        ui.horizontal(|ui| {
            units::drag(
                ui,
                &mut rifle.zero_range_m,
                Quantity::Distance,
                10.0..=1000.0,
                1.0,
            );
            ui.label(tr("hitting"));
            units::drag(
                ui,
                &mut rifle.zero_height_cm,
                Quantity::ShortLength,
                -50.0..=50.0,
                0.1,
            )
            .on_hover_text(tr(
                "0 on the crosshair; + high, − low (\"4 cm high at 100 m\")",
            ));
            ui.weak(tr("above the aim"));
        });
        ui.end_row();
    });
    ui.add_space(4.0);

    let short = |quantity: Quantity, value: f64| {
        let number = format!("{:.2}", u.show(quantity, value));
        let number = number.trim_end_matches('0').trim_end_matches('.');
        format!("{number} {}", u.label(quantity))
    };
    let barrel = format!(
        "{} · 1:{} {}",
        rifle
            .barrel_length_mm
            .map_or(tr("length unknown").to_owned(), |mm| short(
                Quantity::BarrelLength,
                mm
            )),
        short(Quantity::Twist, rifle.twist_in),
        if rifle.right_hand_twist {
            tr("right twist")
        } else {
            tr("left twist")
        }
    );
    fold(ui, "rifle-barrel", tr("Barrel"), &barrel, |ui| {
        grid(ui, "rifle-barrel-grid", |ui| {
            optional_quantity(
                ui,
                tr("Barrel length"),
                &mut rifle.barrel_length_mm,
                560.0,
                Quantity::BarrelLength,
                200.0..=1000.0,
                1.0,
            );
            quantity(
                ui,
                tr("Twist, 1 turn in"),
                &mut rifle.twist_in,
                Quantity::Twist,
                3.0..=40.0,
                0.05,
            );
            ui.label("");
            ui.weak(tr(
                "Set to a common value for the caliber; check your barrel's.",
            ));
            ui.end_row();
            ui.label(tr("Twist direction"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut rifle.right_hand_twist, true, tr("Right"));
                ui.selectable_value(&mut rifle.right_hand_twist, false, tr("Left"));
            });
            ui.end_row();
        });
    });
    let weather = format!(
        "{} · {} · {} %",
        u.format(Quantity::Temperature, rifle.zero_temperature_c),
        u.format(Quantity::Pressure, rifle.zero_pressure_hpa),
        rifle.zero_humidity_percent.round()
    );
    fold(
        ui,
        "rifle-zero-weather",
        tr("Zero conditions"),
        &weather,
        |ui| {
            ui.weak(tr("The weather the rifle was zeroed in."));
            grid(ui, "rifle-zero-grid", |ui| {
                quantity(
                    ui,
                    tr("Temperature"),
                    &mut rifle.zero_temperature_c,
                    Quantity::Temperature,
                    -40.0..=50.0,
                    0.5,
                );
                quantity(
                    ui,
                    tr("Barometer at the range"),
                    &mut rifle.zero_pressure_hpa,
                    Quantity::Pressure,
                    500.0..=1100.0,
                    1.0,
                );
                number(
                    ui,
                    tr("Humidity"),
                    &mut rifle.zero_humidity_percent,
                    0.0..=100.0,
                    1.0,
                    " %",
                );
                ui.label(tr("Powder temperature"));
                ui.horizontal(|ui| {
                    let mut same = rifle.zero_powder_temperature_c.is_none();
                    if ui.checkbox(&mut same, tr("same as air")).changed() {
                        rifle.zero_powder_temperature_c =
                            (!same).then_some(rifle.zero_temperature_c);
                    }
                    if let Some(powder) = &mut rifle.zero_powder_temperature_c {
                        units::drag(ui, powder, Quantity::Temperature, -45.0..=60.0, 0.5);
                    }
                });
                ui.end_row();
            });
        },
    );
    let notes = rifle.notes.lines().next().unwrap_or_default().to_owned();
    fold(
        ui,
        "rifle-notes",
        tr("Notes"),
        &crate::calculator::shortened(&notes, 50),
        |ui| {
            ui.text_edit_multiline(&mut rifle.notes);
        },
    );
}

/// The scope's clicks: taken from the scope when it publishes them, with a
/// way to set them by hand.
fn click_rows(ui: &mut egui::Ui, rifle: &mut Rifle, data: &Data) {
    let from_scope = rifle
        .scope_id
        .as_deref()
        .and_then(|id| data.scope(id))
        .and_then(|scope| scope.rifle_clicks());
    let own_id = ui.scope_id().with("own-clicks");
    let own = ui
        .data(|data| data.get_temp::<bool>(own_id))
        .unwrap_or(false);
    ui.label(tr("Clicks"));
    match from_scope {
        Some(clicks) if !own && clicks == (rifle.click_unit, rifle.click_value) => {
            ui.horizontal(|ui| {
                ui.label(trf!(
                    "{} {} a click, from the scope",
                    rifle.click_value,
                    ballistics_shot::unit_label(rifle.click_unit)
                ));
                if ui.small_button(tr("Change")).clicked() {
                    ui.data_mut(|data| data.insert_temp(own_id, true));
                }
            });
        }
        _ => {
            ui.horizontal_wrapped(|ui| {
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
                        .suffix(tr(" a click")),
                );
                if let Some(clicks) = from_scope
                    && clicks != (rifle.click_unit, rifle.click_value)
                    && ui
                        .small_button(tr("Use the scope's"))
                        .on_hover_text(trf!(
                            "{} {} a click",
                            clicks.1,
                            ballistics_shot::unit_label(clicks.0)
                        ))
                        .clicked()
                {
                    (rifle.click_unit, rifle.click_value) = clicks;
                    ui.data_mut(|data| data.insert_temp(own_id, false));
                }
            });
        }
    }
    ui.end_row();
}

/// Scope and reticle rows of the rifle editor. Choosing a scope also takes
/// over its click unit and value.
fn scope_rows(ui: &mut egui::Ui, rifle: &mut Rifle, data: &Data, search: &mut String) {
    ui.label(tr("Scope"));
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
    ui.label(tr("Reticle"));
    egui::ComboBox::from_id_salt("rifle-reticle")
        .selected_text(
            rifle
                .reticle_id
                .as_deref()
                .map_or(tr("Choose reticle").to_owned(), reticle_name),
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
        tr("Edit scope")
    } else {
        tr("New scope")
    });
    ui.add_space(4.0);
    grid(ui, "scope-editor", |ui| {
        text(ui, tr("Maker"), &mut scope.manufacturer);
        text(ui, tr("Model"), &mut scope.model);
        ui.label(tr("Magnification"));
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut scope.magnification_min)
                    .range(1.0..=100.0)
                    .speed(0.1)
                    .suffix("×"),
            );
            ui.label(tr("to"));
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
            tr("Objective"),
            &mut scope.objective_mm,
            50.0,
            10.0..=80.0,
            1.0,
            " mm",
        );
        ui.label(tr("Reticle position"));
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut scope.focal_plane,
                FocalPlane::Sfp,
                tr("Second focal plane"),
            );
            ui.selectable_value(
                &mut scope.focal_plane,
                FocalPlane::Ffp,
                tr("First focal plane"),
            );
        });
        ui.end_row();
        if scope.focal_plane == FocalPlane::Sfp {
            ui.label(tr("  marks true at"));
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
        ui.label(tr("Clicks"));
        ui.horizontal(|ui| {
            for unit in [AngleUnit::CmAt100m, AngleUnit::Mrad, AngleUnit::Moa] {
                ui.selectable_value(&mut scope.click_unit, unit, unit.label());
            }
            ui.add(
                egui::DragValue::new(&mut scope.click_value)
                    .range(0.01..=5.0)
                    .speed(0.01)
                    .prefix(tr("per click ")),
            );
        });
        ui.end_row();
        let travel_unit = format!(" {}", scope.click_unit.label());
        optional_number(
            ui,
            tr("Elevation travel"),
            &mut scope.elevation_travel,
            20.0,
            0.1..=500.0,
            0.5,
            &travel_unit,
        );
        ui.label(tr("Field of view at 100 m"));
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
                    ui.label(tr("to"));
                    ui.add(
                        egui::DragValue::new(narrow)
                            .range(0.1..=60.0)
                            .speed(0.1)
                            .suffix(" m"),
                    );
                }
                None => {
                    ui.weak(tr("unknown"));
                }
            }
        });
        ui.end_row();
        ui.label(tr("Illuminated"));
        ui.checkbox(&mut scope.illuminated, "");
        ui.end_row();
        ui.label(tr("Reticle drawing"));
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt("custom-scope-reticle")
                .width(280.0)
                .height(420.0)
                .selected_text(
                    scope
                        .reticle_id
                        .as_deref()
                        .map_or(tr("Plain crosshair").to_owned(), reticle_name),
                )
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    let search_id = ui.scope_id().with("reticle-search");
                    let mut search: String =
                        ui.data(|data| data.get_temp(search_id)).unwrap_or_default();
                    ui.add(
                        egui::TextEdit::singleline(&mut search)
                            .hint_text(tr("Search any reticle name")),
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
                        // Closes the combo box's popup.
                        ui.close();
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
        ui.label(tr("Notes"));
        ui.text_edit_multiline(&mut scope.notes);
        ui.end_row();
    });
    ui.small(tr(
        "A catalog reticle is drawn with the dimensions its maker published for that scope. On a second-focal-plane \
         scope they are taken to be true at the magnification above.",
    ));
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
            .add_enabled(problem.is_none() && changed, egui::Button::new(tr("Save")))
            .clicked()
        {
            actions.push(Action::SaveScope(scope.clone()));
        }
        if let Some(stored) = stored {
            if changed && ui.button(tr("Undo changes")).clicked() {
                *scope = stored.clone();
            }
            if confirm_button(ui, "delete-scope", tr("Delete")) {
                actions.push(Action::DeleteScope(stored.id.clone()));
            }
        } else if ui.button(tr("Cancel")).clicked() {
            cancel = true;
        }
    });
    if let Some(problem) = problem {
        ui.colored_label(ui.visuals().warn_fg_color, problem);
    } else if changed {
        ui.weak(tr("Unsaved changes"));
    }
    if in_use > 0 {
        ui.weak(if in_use == 1 {
            tr("Mounted on 1 rifle.").to_owned()
        } else {
            trf!("Mounted on {} rifles.", in_use)
        });
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
                ui.weak(trf!("Also sold as: {}", reticle.also_known_as.join(", ")));
            }
        })
}

fn preview_caption(reticle: &Reticle) -> String {
    if reticle.id == PLAIN_CROSSHAIR {
        tr("A fine crosshair without marks.").to_owned()
    } else {
        trf!(
            "Shown at its published size, {:.1} mrad from the centre to its outermost mark.",
            reticle.inner_extent_mrad()
        )
    }
}

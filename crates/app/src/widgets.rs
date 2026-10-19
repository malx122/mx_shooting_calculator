//! Small input helpers shared by the pages.

use std::ops::RangeInclusive;

use ballistics_store::{
    Caliber, Catalog, FocalPlane, PLAIN_CROSSHAIR, Reticle, Scope, Target, TargetGroup, UserScope,
};
use eframe::egui::{self, RichText};

pub fn section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(true)
        .show(ui, contents);
}

pub fn grid(ui: &mut egui::Ui, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, contents);
}

/// A labelled number in a two-column grid row.
pub fn number(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: RangeInclusive<f64>,
    speed: f64,
    suffix: &str,
) {
    ui.label(label);
    ui.add(
        egui::DragValue::new(value)
            .range(range)
            .speed(speed)
            .suffix(suffix),
    );
    ui.end_row();
}

/// A number that may be unknown: a checkbox switches it on with `default`.
pub fn optional_number(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    default: f64,
    range: RangeInclusive<f64>,
    speed: f64,
    suffix: &str,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut known = value.is_some();
        if ui.checkbox(&mut known, "").changed() {
            *value = known.then_some(default);
        }
        match value {
            Some(number) => {
                ui.add(
                    egui::DragValue::new(number)
                        .range(range)
                        .speed(speed)
                        .suffix(suffix),
                );
            }
            None => {
                ui.weak("unknown");
            }
        }
    });
    ui.end_row();
}

pub fn text(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.text_edit_singleline(value);
    ui.end_row();
}

pub fn caliber_name(key: &str) -> String {
    Catalog::builtin()
        .caliber(key)
        .map_or_else(|| key.to_owned(), |caliber| caliber.name.clone())
}

/// A drop-down of all calibers with a search box. Returns the newly chosen
/// caliber when the selection changes.
pub fn caliber_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &str,
    search: &mut String,
) -> Option<&'static Caliber> {
    let catalog = Catalog::builtin();
    let mut chosen = None;
    let label = if selected.is_empty() {
        "Choose caliber".to_owned()
    } else {
        caliber_name(selected)
    };
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(label)
        .width(220.0)
        .height(360.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            let field =
                ui.add(egui::TextEdit::singleline(search).hint_text("Search, e.g. 6.5 swe"));
            field.request_focus();
            for caliber in catalog.search_calibers(search) {
                let response = ui.selectable_label(caliber.key == selected, &caliber.name);
                let response = if caliber.aliases.is_empty() {
                    response
                } else {
                    response.on_hover_text(caliber.aliases.join(", "))
                };
                if response.clicked() {
                    chosen = Some(caliber);
                    search.clear();
                    ui.close_menu();
                }
            }
        });
    chosen
}

/// A drop-down of the hunter's own targets, then the catalog's game and
/// sport targets. Returns true when the choice changed.
pub fn target_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    data: &crate::app::Data,
    selected: &mut Option<String>,
) -> bool {
    let current = match selected {
        None => "No target".to_owned(),
        Some(id) => data
            .target(id)
            .map_or_else(|| "Choose a target".to_owned(), |target| target.name),
    };
    let before = selected.clone();
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(current)
        .width(260.0)
        .height(420.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "No target");
            let mut group =
                |ui: &mut egui::Ui, title: &str, targets: &mut dyn Iterator<Item = &Target>| {
                    let mut first = true;
                    for target in targets {
                        if first {
                            ui.label(RichText::new(title).small().strong());
                            first = false;
                        }
                        ui.selectable_value(selected, Some(target.id.clone()), &target.name);
                    }
                };
            let catalog = &Catalog::builtin().targets;
            group(ui, "My targets", &mut data.targets.iter());
            for kind in TargetGroup::ALL {
                group(
                    ui,
                    kind.label(),
                    &mut catalog.iter().filter(|t| t.group() == kind),
                );
            }
        });
    *selected != before
}

/// A button that needs a second click to confirm. Returns true on confirm.
pub fn confirm_button(ui: &mut egui::Ui, id: &str, label: &str) -> bool {
    let id = ui.id().with(id);
    let armed = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if armed {
        let confirmed = ui
            .button(
                RichText::new(format!("Really {}?", label.to_lowercase()))
                    .color(ui.visuals().error_fg_color),
            )
            .clicked();
        if ui.button("Cancel").clicked() || confirmed {
            ui.data_mut(|data| data.insert_temp(id, false));
        }
        confirmed
    } else {
        if ui.button(label).clicked() {
            ui.data_mut(|data| data.insert_temp(id, true));
        }
        false
    }
}

/// What the scope picker changed: a scope was chosen, or the scope removed.
pub enum ScopeChoice {
    Mounted(Box<Scope>),
    Removed,
}

/// A drop-down of the hunter's own scopes and the catalog's, with a search
/// box.
pub fn scope_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: Option<&str>,
    search: &mut String,
    own: &[UserScope],
) -> Option<ScopeChoice> {
    let catalog = Catalog::builtin();
    let label = match selected {
        None => "No scope".to_owned(),
        Some(id) => own
            .iter()
            .find(|scope| scope.id == id)
            .map(UserScope::display_name)
            .or_else(|| catalog.scope(id).map(Scope::display_name))
            .unwrap_or_else(|| "Unknown scope".to_owned()),
    };
    let mut choice = None;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(label)
        .width(260.0)
        .height(360.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(search).hint_text("Search maker, model or reticle"))
                .request_focus();
            if ui
                .selectable_label(selected.is_none(), "No scope")
                .clicked()
            {
                choice = Some(ScopeChoice::Removed);
            }
            let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
            let matches = |name: &str| {
                let name = name.to_lowercase();
                words.iter().all(|word| name.contains(word.as_str()))
            };
            let mine: Vec<_> = own
                .iter()
                .filter(|scope| matches(&scope.display_name()))
                .collect();
            if !mine.is_empty() {
                ui.separator();
                ui.weak("My scopes");
                for scope in mine {
                    if ui
                        .selectable_label(selected == Some(scope.id.as_str()), scope.display_name())
                        .clicked()
                    {
                        choice = Some(ScopeChoice::Mounted(Box::new(scope.to_scope())));
                    }
                }
                ui.separator();
                ui.weak("Catalog");
            }
            // Catalog scopes are also found by any name of their reticles.
            let by_reticle = |scope: &Scope| {
                !search.trim().is_empty()
                    && scope.reticles.iter().any(|id| {
                        catalog
                            .reticle(id)
                            .is_some_and(|reticle| reticle.matches(search))
                    })
            };
            for scope in catalog
                .scopes
                .iter()
                .filter(|scope| matches(&scope.display_name()) || by_reticle(scope))
            {
                if ui
                    .selectable_label(selected == Some(scope.id.as_str()), scope.display_name())
                    .clicked()
                {
                    choice = Some(ScopeChoice::Mounted(Box::new(scope.clone())));
                }
            }
            if choice.is_some() {
                search.clear();
                ui.close_menu();
            }
        });
    choice
}

/// A reticle's name from its id: a catalog reticle or the plain crosshair.
pub fn reticle_name(id: &str) -> String {
    if id == PLAIN_CROSSHAIR {
        return Reticle::plain_crosshair().name;
    }
    Catalog::builtin()
        .reticle(id)
        .map_or_else(|| id.to_owned(), |reticle| reticle.name.clone())
}

/// "2.5–15×56 · first focal plane · 0.1 mrad clicks · illuminated"
pub fn scope_summary(scope: &Scope) -> String {
    let plane = match scope.focal_plane {
        FocalPlane::Ffp => "first focal plane".to_owned(),
        FocalPlane::Sfp => match scope.reticle_reference_magnification {
            Some(reference) => format!("second focal plane (marks true at {reference}×)"),
            None => "second focal plane".to_owned(),
        },
    };
    let mut parts = vec![
        if scope.objective_mm > 0.0 {
            format!(
                "{}–{}×{}",
                scope.magnification_min, scope.magnification_max, scope.objective_mm
            )
        } else {
            format!("{}–{}×", scope.magnification_min, scope.magnification_max)
        },
        plane,
        match scope.click_value {
            Some(value) => format!("{value} {} clicks", scope.click_unit.label()),
            None => "click value not published".to_owned(),
        },
    ];
    if let Some(travel) = scope.elevation_travel {
        parts.push(format!("{travel} {} elevation", scope.click_unit.label()));
    }
    if scope.illuminated {
        parts.push("illuminated".to_owned());
    }
    parts.join(" · ")
}

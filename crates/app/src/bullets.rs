//! The Bullets page: the hunter's collection with an editor, and the base
//! catalog to copy ready-made bullets from.

use std::collections::HashMap;

use ballistics_store::{
    BcBand, BcModel, Bullet, BulletKind, Catalog, CatalogBullet, CatalogKind, Construction,
    Country, DragPoint, GRAMS_PER_GRAIN,
};
use eframe::egui::{self, RichText};
use egui_plot::{Line, Plot, PlotPoints};

use crate::app::{Action, Data};
use crate::i18n::{tr, tr_data, trf};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity, optional_quantity, quantity};
use crate::widgets::{
    caliber_name, caliber_picker, confirm_button, fold, grid, optional_number, text,
};

#[derive(Clone, Copy, PartialEq)]
enum View {
    Collection,
    Catalog,
}

pub struct BulletsPage {
    view: View,
    /// Caliber shown in the collection list and the catalog; empty for all.
    caliber: String,
    caliber_search: String,
    editor_caliber_search: String,
    /// Text pasted into the drag curve editor.
    curve_paste: String,
    editing: Option<Bullet>,
    catalog_text: String,
    catalog_kind: Option<CatalogKind>,
    lead_free_only: bool,
    /// The legal level of catalog bullets, worked out once each.
    levels: HashMap<(String, String, Country), Option<u8>>,
}

impl Default for BulletsPage {
    fn default() -> Self {
        Self {
            view: View::Collection,
            caliber: String::new(),
            caliber_search: String::new(),
            editor_caliber_search: String::new(),
            curve_paste: String::new(),
            editing: None,
            catalog_text: String::new(),
            catalog_kind: None,
            lead_free_only: false,
            levels: HashMap::new(),
        }
    }
}

impl BulletsPage {
    pub fn show_caliber(&mut self, caliber: &str) {
        self.caliber = caliber.to_owned();
        self.view = View::Collection;
    }

    pub fn saved(&mut self, bullet: &Bullet) {
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.id == bullet.id)
        {
            self.editing = Some(bullet.clone());
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.id == id)
        {
            self.editing = None;
        }
    }

    pub fn show(&mut self, context: &egui::Context, data: &Data) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::TopBottomPanel::top("bullets-toolbar").show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.view, View::Collection, tr("My bullets"));
                ui.selectable_value(&mut self.view, View::Catalog, tr("Catalog"));
                if self.view == View::Catalog {
                    ui.separator();
                    ui.label(tr("Caliber"));
                    if let Some(caliber) = caliber_picker(
                        ui,
                        "bullets-caliber",
                        &self.caliber,
                        &mut self.caliber_search,
                    ) {
                        self.caliber = caliber.key.clone();
                    }
                }
            });
        });
        match self.view {
            View::Collection => self.collection(context, data, &mut actions),
            View::Catalog => self.catalog(context, data),
        }
        actions
    }

    fn collection(&mut self, context: &egui::Context, data: &Data, actions: &mut Vec<Action>) {
        if mobile::is_narrow(context) {
            // On a phone: the list, or one bullet with a way back.
            egui::CentralPanel::default().show(context, |ui| {
                if self.editing.is_none() {
                    self.collection_list(ui, data, true);
                } else if mobile::back_button(ui, tr("My bullets")) {
                    self.editing = None;
                } else {
                    self.collection_editor(ui, data, actions);
                }
            });
            return;
        }
        egui::SidePanel::left("bullet-list")
            .resizable(true)
            .default_width(320.0)
            .show(context, |ui| self.collection_list(ui, data, false));
        egui::CentralPanel::default().show(context, |ui| self.collection_editor(ui, data, actions));
    }

    /// The hunter's bullets by caliber, with ways to add more. On a phone
    /// each is a big row.
    fn collection_list(&mut self, ui: &mut egui::Ui, data: &Data, phone: bool) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(tr("+ Add from the catalog"))
                            .strong()
                            .color(theme::FOREST_900),
                    )
                    .fill(theme::BLAZE),
                )
                .on_hover_text(tr(
                    "Factory ammunition and component bullets, with their data",
                ))
                .clicked()
            {
                self.view = View::Catalog;
            }
            if ui.button(tr("Add by hand")).clicked() {
                let caliber = Catalog::builtin().caliber(&self.caliber);
                self.editing = Some(match caliber {
                    Some(caliber) => Bullet::new_manual(&caliber.key, caliber.bullet_diameter_mm),
                    None => Bullet::new_manual("", 0.0),
                });
            }
        });
        if !self.caliber.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak(trf!("Showing {} only", caliber_name(&self.caliber)));
                if ui.small_button(tr("Show all")).clicked() {
                    self.caliber.clear();
                }
            });
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut shown = 0;
            let mut heading = String::new();
            let mut bullets: Vec<&Bullet> = data
                .bullets
                .iter()
                .filter(|bullet| self.caliber.is_empty() || bullet.caliber == self.caliber)
                .collect();
            bullets.sort_by_key(|bullet| caliber_name(&bullet.caliber));
            for bullet in bullets {
                let caliber = caliber_name(&bullet.caliber);
                if caliber != heading {
                    ui.add_space(4.0);
                    ui.strong(&caliber);
                    heading = caliber;
                }
                let selected = self
                    .editing
                    .as_ref()
                    .is_some_and(|editing| editing.id == bullet.id);
                let clicked = if phone {
                    let detail = match bullet.kind {
                        BulletKind::Factory => tr("Factory ammunition"),
                        BulletKind::Handload => tr("Handload"),
                    };
                    mobile::list_row(ui, &bullet.display_name(), detail, selected).clicked()
                } else {
                    ui.selectable_label(selected, bullet.display_name())
                        .clicked()
                };
                if clicked {
                    self.editing = Some(bullet.clone());
                }
                shown += 1;
            }
            if shown == 0 {
                ui.weak(tr(
                    "No bullets here yet. Add one from the catalog or by hand.",
                ));
            }
        });
    }

    /// The open bullet's editor, with save, undo and delete.
    fn collection_editor(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        let Some(bullet) = &mut self.editing else {
            ui.weak(tr("Choose a bullet, or add one."));
            return;
        };
        let stored = data.bullets.iter().find(|stored| stored.id == bullet.id);
        let mut cancel = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(match (stored, &bullet.catalog_id) {
                (Some(_), _) => tr("Edit bullet"),
                (None, Some(_)) => tr("New from catalog: check the data and save"),
                (None, None) => tr("New bullet"),
            });
            ui.add_space(4.0);
            editor(
                ui,
                bullet,
                &mut self.editor_caliber_search,
                &mut self.curve_paste,
                data.hunting,
            );
            ui.add_space(8.0);

            let problem = bullet.validate().err();
            let changed = stored != Some(&*bullet);
            ui.horizontal_wrapped(|ui| {
                let save = ui.add_enabled(
                    problem.is_none() && changed,
                    egui::Button::new(RichText::new(tr("Save")).strong()),
                );
                if save.clicked() {
                    actions.push(Action::SaveBullet(bullet.clone()));
                }
                if let Some(stored) = stored {
                    if changed && ui.button(tr("Undo changes")).clicked() {
                        *bullet = stored.clone();
                    }
                    if confirm_button(ui, "delete-bullet", tr("Delete")) {
                        actions.push(Action::DeleteBullet(stored.id.clone()));
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
            if let Some(source) = bullet
                .catalog_id
                .as_deref()
                .and_then(|id| Catalog::builtin().bullet(id))
            {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.weak(trf!(
                        "Copied from the catalog (data checked {}).",
                        source.verified_on
                    ));
                    ui.hyperlink_to(tr("Source"), &source.source_url)
                        .on_hover_text(&source.source_url);
                });
            }
        });
        if cancel {
            self.editing = None;
        }
    }

    fn catalog(&mut self, context: &egui::Context, data: &Data) {
        let catalog = Catalog::builtin();
        let u = units::prefs(context);
        egui::CentralPanel::default().show(context, |ui| {
            let Some(caliber) = catalog.caliber(&self.caliber) else {
                ui.heading(tr("Choose a caliber"));
                ui.label(tr("The catalog shows factory ammunition in that caliber and component bullets that fit it."));
                let mut own: Vec<&str> = data.rifles.iter().map(|rifle| rifle.caliber.as_str()).collect();
                own.dedup();
                if !own.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr("Your rifles:"));
                        for key in own {
                            if ui.button(caliber_name(key)).clicked() {
                                self.caliber = key.to_owned();
                            }
                        }
                    });
                }
                return;
            };

            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.catalog_text).hint_text(tr("Search maker or product")));
                ui.selectable_value(&mut self.catalog_kind, None, tr("All"));
                ui.selectable_value(&mut self.catalog_kind, Some(CatalogKind::Ammunition), tr("Factory ammunition"));
                ui.selectable_value(&mut self.catalog_kind, Some(CatalogKind::Component), tr("Component bullets"));
                ui.checkbox(&mut self.lead_free_only, tr("Lead-free only"));
            });
            ui.separator();

            let query = self.catalog_text.to_lowercase();
            let entries: Vec<&CatalogBullet> = catalog
                .bullets_for(caliber)
                .filter(|entry| self.catalog_kind.is_none_or(|kind| entry.kind == kind))
                .filter(|entry| !self.lead_free_only || entry.lead_free)
                .filter(|entry| {
                    query.is_empty()
                        || format!("{} {}", entry.manufacturer, entry.product).to_lowercase().contains(&query)
                })
                .collect();
            if entries.is_empty() {
                ui.weak(trf!("Nothing in the catalog for {} with these filters.", caliber.name));
                return;
            }
            let mut chosen = None;
            let country = data.country;
            let levels = &mut self.levels;
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("catalog").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    let class = country.map(|country| trf!("{} class", country.code()));
                    for header in ["", tr("Maker"), tr("Product"), tr("Type"), tr("Mass"), "BC G1", "BC G7", tr("Velocity")] {
                        ui.strong(header);
                    }
                    if let Some(class) = &class {
                        ui.strong(class).on_hover_text(
                            country.map_or(String::new(), |country| crate::i18n::rules_help(country.rules())),
                        );
                    }
                    for header in [tr("Construction"), "", tr("Source")] {
                        ui.strong(header);
                    }
                    ui.end_row();
                    for entry in entries {
                        let owned = data.bullets.iter().any(|b| {
                            b.catalog_id.as_deref() == Some(entry.id.as_str()) && b.caliber == caliber.key
                        });
                        if owned {
                            ui.weak(tr("✔ added"));
                        } else if ui.button(tr("Add")).clicked() {
                            chosen = Some(entry);
                        }
                        ui.label(&entry.manufacturer);
                        ui.label(&entry.product);
                        ui.label(match entry.kind {
                            CatalogKind::Ammunition => tr("Factory"),
                            CatalogKind::Component => tr("Component"),
                        });
                        let grains = entry.mass_gr.unwrap_or(entry.mass_g / GRAMS_PER_GRAIN);
                        ui.label(format!("{:.1} g / {grains:.0} gr", entry.mass_g));
                        ui.label(entry.bc_g1.map_or("–".to_owned(), |bc| format!("{bc:.3}")));
                        ui.label(entry.bc_g7.map_or("–".to_owned(), |bc| format!("{bc:.3}")));
                        ui.label(entry.muzzle_velocity_mps.map_or(tr("your load").to_owned(), |v| u.format(Quantity::Speed, v)));
                        if let Some(country) = country {
                            let level = *levels
                                .entry((entry.id.clone(), caliber.key.clone(), country))
                                .or_insert_with(|| {
                                    ballistics_shot::level_of(&entry.to_collection(caliber), country)
                                });
                            ui.label(match level {
                                Some(level) => level.to_string(),
                                None if entry.muzzle_velocity_mps.is_none() => "–".to_owned(),
                                None => tr("none").to_owned(),
                            })
                            .on_hover_text(match level {
                                Some(level) => trf!(
                                    "Meets {} out of a typical barrel; the load and barrel decide",
                                    crate::i18n::level_name(country.rules(), level).to_lowercase()
                                ),
                                None => tr("No level known").to_owned(),
                            });
                        }
                        ui.label(tr_data(entry.construction.label()));
                        ui.label(if entry.lead_free { tr("lead-free") } else { "" });
                        ui.hyperlink_to(tr("page"), &entry.source_url).on_hover_text(&entry.source_url);
                        ui.end_row();
                    }
                });
            });
            if let Some(entry) = chosen {
                self.editing = Some(entry.to_collection(caliber));
                self.view = View::Collection;
            }
        });
    }
}

fn editor(
    ui: &mut egui::Ui,
    bullet: &mut Bullet,
    caliber_search: &mut String,
    curve_paste: &mut String,
    hunting: bool,
) {
    let u = units::prefs(ui.ctx());
    grid(ui, "bullet-editor", |ui| {
        text(ui, tr("Manufacturer"), &mut bullet.manufacturer);
        text(ui, tr("Name"), &mut bullet.name);
        ui.label(tr("Type"));
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut bullet.kind,
                BulletKind::Factory,
                tr("Factory ammunition"),
            );
            ui.selectable_value(&mut bullet.kind, BulletKind::Handload, tr("Handload"));
        });
        ui.end_row();
        ui.label(tr("Loaded in"));
        if let Some(caliber) = caliber_picker(ui, "bullet-caliber", &bullet.caliber, caliber_search)
        {
            bullet.caliber = caliber.key.clone();
            bullet.diameter_mm = caliber.bullet_diameter_mm;
        }
        ui.end_row();
        quantity(
            ui,
            tr("Diameter"),
            &mut bullet.diameter_mm,
            Quantity::BulletDiameter,
            1.0..=16.0,
            0.01,
        );
        quantity(
            ui,
            tr("Mass"),
            &mut bullet.mass_g,
            Quantity::BulletMass,
            0.1..=100.0,
            0.05,
        );
        optional_quantity(
            ui,
            tr("Length"),
            &mut bullet.length_mm,
            30.0,
            Quantity::BulletLength,
            5.0..=80.0,
            0.1,
        );
        quantity(
            ui,
            tr("Muzzle velocity"),
            &mut bullet.muzzle_velocity_mps,
            Quantity::Speed,
            0.0..=1600.0,
            1.0,
        );
    });
    if bullet.length_mm.is_none() {
        ui.weak(tr(
            "Without a length, stability, spin drift and aerodynamic jump are skipped.",
        ));
    }
    ui.add_space(4.0);

    let drag = match bullet.drag_model {
        BcModel::Custom => trf!("own drag curve, {} points", bullet.drag_curve.len()),
        model => {
            let bc = bullet
                .ballistic_coefficient()
                .map_or(tr("no BC").to_owned(), |bc| format!("{bc:.3}"));
            if bullet.bc_bands.is_empty() {
                format!("{model:?} {bc}")
            } else {
                trf!(
                    "{} {}, {} bands",
                    format!("{model:?}"),
                    bc,
                    bullet.bc_bands.len()
                )
            }
        }
    };
    fold(ui, "bullet-drag", tr("Drag model"), &drag, |ui| {
        grid(ui, "bullet-drag-grid", |ui| {
            optional_number(ui, "BC G1", &mut bullet.bc_g1, 0.4, 0.05..=1.2, 0.001, "");
            optional_number(ui, "BC G7", &mut bullet.bc_g7, 0.2, 0.05..=1.2, 0.001, "");
            ui.label(tr("Calculate with"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut bullet.drag_model, BcModel::G1, "G1");
                ui.selectable_value(&mut bullet.drag_model, BcModel::G7, "G7");
                ui.selectable_value(
                    &mut bullet.drag_model,
                    BcModel::Custom,
                    tr("Own drag curve"),
                );
            });
            ui.end_row();
            if bullet.drag_model == BcModel::Custom {
                drag_curve_rows(ui, bullet, curve_paste);
            } else {
                bc_band_rows(ui, bullet);
            }
        });
    });
    let velocity = trf!(
        "measured at {} · {}",
        u.format(Quantity::Temperature, bullet.velocity_temperature_c),
        u.format(Quantity::Sensitivity, bullet.velocity_per_degree)
    );
    fold(
        ui,
        "bullet-velocity",
        tr("Velocity details"),
        &velocity,
        |ui| {
            grid(ui, "bullet-velocity-grid", |ui| {
                quantity(
                    ui,
                    tr("Measured at"),
                    &mut bullet.velocity_temperature_c,
                    Quantity::Temperature,
                    -40.0..=50.0,
                    0.5,
                );
                quantity(
                    ui,
                    tr("Temperature sensitivity"),
                    &mut bullet.velocity_per_degree,
                    Quantity::Sensitivity,
                    0.0..=3.0,
                    0.05,
                );
                optional_quantity(
                    ui,
                    tr("Test barrel"),
                    &mut bullet.test_barrel_mm,
                    610.0,
                    Quantity::BarrelLength,
                    200.0..=1000.0,
                    1.0,
                );
            });
            ui.weak(trf!(
                "A typical {} is filled in; measure your own for cold-weather accuracy.",
                u.format(Quantity::Sensitivity, 0.5)
            ));
        },
    );
    let construction = |ui: &mut egui::Ui, bullet: &mut Bullet| {
        grid(ui, "bullet-construction", |ui| {
            ui.label(tr("Construction"));
            egui::ComboBox::from_id_salt("construction")
                .selected_text(tr_data(bullet.construction.label()))
                .show_ui(ui, |ui| {
                    for construction in Construction::ALL {
                        ui.selectable_value(
                            &mut bullet.construction,
                            construction,
                            tr_data(construction.label()),
                        );
                    }
                });
            ui.end_row();
            ui.label(tr("Lead-free"));
            ui.checkbox(&mut bullet.lead_free, "");
            ui.end_row();
            optional_quantity(
                ui,
                tr("Min. expansion velocity"),
                &mut bullet.min_expansion_mps,
                550.0,
                Quantity::Speed,
                100.0..=900.0,
                5.0,
            );
        });
    };
    if hunting {
        // They decide the legal class: always in view for a hunter.
        ui.add_space(4.0);
        construction(ui, bullet);
    } else {
        fold(
            ui,
            "bullet-construction-fold",
            tr("Construction"),
            tr_data(bullet.construction.label()),
            |ui| construction(ui, bullet),
        );
    }
    let notes = bullet.notes.lines().next().unwrap_or_default().to_owned();
    fold(
        ui,
        "bullet-notes",
        tr("Notes"),
        &crate::calculator::shortened(&notes, 50),
        |ui| {
            ui.text_edit_multiline(&mut bullet.notes);
        },
    );
}

/// Optional BCs by velocity for the selected drag model, as some makers
/// publish them. They replace the single BC in the calculation.
fn bc_band_rows(ui: &mut egui::Ui, bullet: &mut Bullet) {
    ui.label(tr("BC by velocity"));
    ui.vertical(|ui| {
        let mut remove = None;
        for (index, band) in bullet.bc_bands.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut band.bc)
                        .range(0.05..=1.2)
                        .speed(0.001)
                        .prefix("BC "),
                );
                ui.label(tr("at"));
                units::drag(
                    ui,
                    &mut band.velocity_mps,
                    Quantity::Speed,
                    50.0..=1500.0,
                    1.0,
                );
                if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            bullet.bc_bands.remove(index);
        }
        if bullet.bc_bands.len() < 5 && ui.small_button(tr("+ Add band")).clicked() {
            let bc = bullet.ballistic_coefficient().unwrap_or(0.4);
            let velocity_mps = bullet
                .bc_bands
                .last()
                .map_or(bullet.muzzle_velocity_mps.max(300.0), |band| {
                    (band.velocity_mps - 100.0).max(100.0)
                })
                .round();
            bullet.bc_bands.push(BcBand { velocity_mps, bc });
        }
        if bullet.bc_bands.is_empty() {
            ui.weak(tr("none: one BC for all velocities"));
        }
    });
    ui.end_row();
}

/// The bullet's own drag curve: a plot, the points, and a box to paste a
/// whole table (for example a maker's Doppler-radar data).
fn drag_curve_rows(ui: &mut egui::Ui, bullet: &mut Bullet, paste: &mut String) {
    ui.label(tr("Drag curve"));
    ui.vertical(|ui| {
        ui.weak(trf!(
            "{} points. With its own curve the bullet needs no BC: mass and diameter are used instead.",
            bullet.drag_curve.len()
        ));
        if bullet.drag_curve.len() >= 2 {
            let points: PlotPoints<'_> = bullet.drag_curve.iter().map(|point| [point.mach, point.cd]).collect();
            Plot::new("drag-curve")
                .height(140.0)
                .width(360.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .x_axis_label("Mach")
                .y_axis_label("Cd")
                .show(ui, |plot_ui| plot_ui.line(Line::new(points).color(theme::BLAZE)));
        }
        egui::CollapsingHeader::new(tr("Points")).id_salt("drag-curve-points").show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                let mut remove = None;
                for (index, point) in bullet.drag_curve.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut point.mach).range(0.0..=10.0).speed(0.005).prefix("Mach "));
                        ui.add(egui::DragValue::new(&mut point.cd).range(0.001..=2.999).speed(0.001).prefix("Cd "));
                        if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    bullet.drag_curve.remove(index);
                }
            });
            if ui.small_button(tr("+ Add point")).clicked() {
                let mach = bullet.drag_curve.last().map_or(0.5, |point| point.mach + 0.1);
                let cd = bullet.drag_curve.last().map_or(0.3, |point| point.cd);
                bullet.drag_curve.push(DragPoint { mach, cd });
            }
        });
        egui::CollapsingHeader::new(tr("Paste a table")).id_salt("drag-curve-paste").show(ui, |ui| {
            ui.weak(tr("One point per line: Mach and drag coefficient, separated by spaces, tabs, commas or semicolons."));
            ui.add(egui::TextEdit::multiline(paste).desired_rows(6).hint_text("0.50 0.231\n0.80 0.238\n1.00 0.392"));
            ui.horizontal(|ui| match parse_drag_curve(paste) {
                Ok(points) => {
                    if ui.button(trf!("Use these {} points", points.len())).clicked() {
                        bullet.drag_curve = points;
                        paste.clear();
                    }
                }
                Err(problem) if !paste.trim().is_empty() => {
                    ui.colored_label(ui.visuals().warn_fg_color, problem);
                }
                Err(_) => {}
            });
        });
    });
    ui.end_row();
}

/// Reads "Mach Cd" pairs, one per line. Lines that don't start with two
/// numbers (headers, comments) are skipped. Decimal commas are understood
/// when the columns are separated by spaces, tabs or semicolons.
fn parse_drag_curve(text: &str) -> Result<Vec<DragPoint>, String> {
    let mut points = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ';')
            .filter(|f| !f.is_empty())
            .collect();
        let fields: Vec<String> = if fields.len() >= 2 {
            fields.iter().map(|field| field.replace(',', ".")).collect()
        } else {
            line.split(',')
                .map(|field| field.trim().to_owned())
                .collect()
        };
        let numbers: Vec<f64> = fields
            .iter()
            .take(2)
            .filter_map(|field| field.parse().ok())
            .collect();
        if let [mach, cd] = numbers[..] {
            points.push(DragPoint { mach, cd });
        }
    }
    if points.len() < 2 {
        return Err(tr("Found fewer than two points.").to_owned());
    }
    points.sort_by(|a, b| a.mach.total_cmp(&b.mach));
    if points.windows(2).any(|pair| pair[0].mach == pair[1].mach) {
        return Err(tr("Two points have the same Mach number.").to_owned());
    }
    if points
        .iter()
        .any(|point| !(0.0..=10.0).contains(&point.mach) || !(point.cd > 0.0 && point.cd < 3.0))
    {
        return Err(tr("Mach must be 0–10 and the drag coefficient between 0 and 3.").to_owned());
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_drag_tables_are_read_in_common_formats() {
        let spaces = "Mach Cd\n0.50 0.231\n1.00 0.392\n0.80 0.238\n";
        let points = parse_drag_curve(spaces).unwrap();
        assert_eq!(
            points.iter().map(|p| p.mach).collect::<Vec<_>>(),
            [0.5, 0.8, 1.0]
        );
        let csv = "mach,cd\n0.5,0.231\n1.0,0.392";
        assert_eq!(parse_drag_curve(csv).unwrap().len(), 2);
        let european = "0,5;0,231\n1,0;0,392";
        assert_eq!(
            parse_drag_curve(european).unwrap()[1],
            DragPoint {
                mach: 1.0,
                cd: 0.392
            }
        );
        assert!(parse_drag_curve("0.5 0.2").is_err());
        assert!(parse_drag_curve("0.5 0.2\n0.5 0.3").is_err());
        assert!(parse_drag_curve("0.5 0.2\n1.0 7.0").is_err());
    }
}

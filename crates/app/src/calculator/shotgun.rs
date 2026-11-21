//! Calculate with a shotgun: the same page as for a rifle, with the gun and
//! its shell (and barrel) chosen at the top, the same range, angle, wind,
//! target and weather, and the shell's results instead of a bullet's.

use ballistics_store::{Payload, Shell, Shotgun};
use eframe::egui::{self, Ui};

use super::{Aim, CalculatorPage, phone, shortened};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::shotguns::{pellet_name, stored_load};
use crate::units::{self, Quantity};
use crate::widgets::grid;

impl CalculatorPage {
    pub(super) fn show_shotgun(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        shotgun: &Shotgun,
        actions: &mut Vec<Action>,
    ) {
        let context = &ui.ctx().clone();
        let shells: Vec<&Shell> = data
            .shells
            .iter()
            .filter(|shell| shotgun.takes(shell))
            .collect();
        let shell = self
            .inputs
            .bullet_id
            .as_ref()
            .and_then(|id| shells.iter().find(|shell| &shell.id == id))
            .or(shells.first())
            .map(|shell| (*shell).clone());
        self.inputs.bullet_id = shell.as_ref().map(|shell| shell.id.clone());
        self.inputs.barrel = self
            .inputs
            .barrel
            .min(shotgun.barrels.len().saturating_sub(1));
        // The rifle's limits don't apply to the range picker.
        self.limits = (None, None);
        let aim = Aim::new(&self.inputs, data);
        self.remember_target();

        if mobile::is_narrow(context) {
            egui::CentralPanel::default().show(ui, |ui| match self.phone_view {
                phone::PhoneView::Conditions => {
                    if mobile::back_button(ui, tr("Calculate")) {
                        actions.push(Action::Back);
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.shotgun_equipment(ui, data, shotgun, &shells, shell.as_ref(), actions);
                        if shell.is_some() {
                            ui.add_space(6.0);
                            self.conditions_for(ui, data, &aim, None, true, actions);
                        }
                    });
                }
                phone::PhoneView::Results => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let title = format!("{} · {}", shotgun.name, gauge_text(shotgun));
                        let detail = shell.as_ref().map_or_else(
                            || tr("Choose a shell").to_owned(),
                            |shell| shell.display_name(),
                        );
                        if mobile::list_row(ui, &title, &detail, false).clicked() {
                            self.phone_view = phone::PhoneView::Conditions;
                        }
                        let Some(shell) = &shell else {
                            no_shells(ui, shotgun, actions);
                            return;
                        };
                        self.tiles(ui, &aim);
                        self.shotgun_results(ui, data, shotgun, shell, &aim);
                    });
                }
            });
            self.picker_sheet(context, data, None);
            return;
        }

        egui::Panel::left("calculator-inputs")
            .resizable(true)
            .default_size(370.0)
            .max_size(560.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.shotgun_equipment(ui, data, shotgun, &shells, shell.as_ref(), actions);
                    if shell.is_some() {
                        ui.add_space(6.0);
                        self.tiles(ui, &aim);
                        ui.add_space(4.0);
                        self.conditions_for(ui, data, &aim, None, true, actions);
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| match &shell {
            None => no_shells(ui, shotgun, actions),
            Some(shell) => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.shotgun_results(ui, data, shotgun, shell, &aim)
                    });
            }
        });
        self.picker_sheet(context, data, None);
    }

    fn shotgun_results(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        shotgun: &Shotgun,
        shell: &Shell,
        aim: &Aim,
    ) {
        let conditions = self.inputs.conditions();
        self.shotgun.show(
            ui,
            data,
            shotgun,
            shell,
            self.inputs.barrel,
            aim.target.as_ref(),
            self.inputs.target_range_m,
            self.inputs.target_speed,
            &conditions,
        );
    }

    /// The gun, its shell and barrel, as the rifle and bullet are chosen.
    fn shotgun_equipment(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        shotgun: &Shotgun,
        shells: &[&Shell],
        shell: Option<&Shell>,
        actions: &mut Vec<Action>,
    ) {
        let u = units::prefs(ui.ctx());
        ui.heading(tr("Calculate"));
        ui.add_space(4.0);
        grid(ui, "shotgun-equipment", |ui| {
            ui.label(tr("Gun"));
            egui::ComboBox::from_id_salt("rifle")
                .width(240.0)
                .selected_text(shortened(&shotgun.name, 40))
                .show_ui(ui, |ui| self.gun_choices(ui, data))
                .response
                .on_hover_text(gauge_text(shotgun));
            ui.end_row();
            ui.label(tr("Shell"));
            egui::ComboBox::from_id_salt("bullet")
                .width(240.0)
                .selected_text(shell.map_or(tr("None").to_owned(), |shell| {
                    shortened(&shell.display_name(), 40)
                }))
                .show_ui(ui, |ui| {
                    for candidate in shells {
                        let selected = shell.is_some_and(|shell| shell.id == candidate.id);
                        if ui
                            .selectable_label(selected, candidate.display_name())
                            .clicked()
                        {
                            self.inputs.bullet_id = Some(candidate.id.clone());
                        }
                    }
                });
            ui.end_row();
            if shotgun.barrels.len() > 1 && shell.is_some_and(|shell| !shell.is_slug()) {
                ui.label(tr("Barrel"));
                ui.horizontal_wrapped(|ui| {
                    for (number, barrel) in shotgun.barrels.iter().enumerate() {
                        let name = if barrel.name.trim().is_empty() {
                            trf!("Barrel {}", number + 1)
                        } else {
                            barrel.name.clone()
                        };
                        ui.selectable_value(&mut self.inputs.barrel, number, name);
                    }
                });
                ui.end_row();
            }
        });
        let Some(shell) = shell else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let boards =
                stored_load(data, &shotgun.id, &shell.id).map_or(0, |load| load.patterns.len());
            let what = match shell.payload {
                Payload::Shot {
                    material,
                    pellet_mm,
                    load_g,
                } => trf!(
                    "{} {} · {} · {}",
                    u.format(Quantity::BulletMass, load_g),
                    crate::i18n::tr_data(material.label()).to_lowercase(),
                    pellet_name(pellet_mm),
                    match boards {
                        0 => tr("nominal pattern").to_owned(),
                        count => crate::shotguns::boards_text(count),
                    }
                ),
                Payload::Slug { mass_g, .. } => {
                    trf!("{} slug", u.format(Quantity::BulletMass, mass_g))
                }
            };
            ui.weak(what);
            if ui
                .link(tr("Gun setup…"))
                .on_hover_text(tr(
                    "The gun's chokes, and the pattern boards counted with this shell in it",
                ))
                .clicked()
            {
                actions.push(Action::EditShotgunLoad(
                    shotgun.id.clone(),
                    shell.id.clone(),
                ));
            }
        });
    }
}

fn gauge_text(shotgun: &Shotgun) -> String {
    crate::widgets::shotgun_caliber_name(&shotgun.caliber)
}

fn no_shells(ui: &mut Ui, shotgun: &Shotgun, actions: &mut Vec<Action>) {
    ui.heading(trf!(
        "No shells for {} in your collection",
        crate::widgets::shotgun_caliber_name(&shotgun.caliber)
    ));
    ui.label(tr("Add the shells you shoot on the Ammunition page."));
    if ui.button(tr("Go to Ammunition")).clicked() {
        actions.push(Action::ShowShellsFor(shotgun.caliber.clone()));
    }
}

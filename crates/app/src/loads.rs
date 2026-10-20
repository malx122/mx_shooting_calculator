//! A bullet in a rifle, set up once at the range: measured velocities (by
//! hand or from a chronograph file), where it hits against the rifle's zero
//! load, the weather it was zeroed in, and truing to a long shot. Shown on
//! the Rifles page; Calculate shows one line about it.

use ballistics_shot::{Conditions, from_radians, to_radians, unit_label};
use ballistics_store::{
    Bullet, BulletKind, LoadZero, Rifle, RifleLoad, Truing, TruingMethod, VelocityPoint,
};
use eframe::egui::{self, RichText};

use crate::app::Action;
use crate::units::{self, Quantity, quantity};
use crate::widgets::{confirm_button, grid, number};

/// The load being edited, and the forms that go with it.
#[derive(Default)]
pub struct LoadEditor {
    /// The saved load or a new empty one, with unsaved edits.
    draft: Option<RifleLoad>,
    truing_form: TruingForm,
    /// The truing form is showing.
    truing_open: bool,
    /// A chronograph file being picked, and one being looked at.
    chrono_file: crate::files::Picked,
    chrono: Option<ChronoImport>,
    /// Why the last chronograph file couldn't be read.
    chrono_error: Option<String>,
}

/// A chronograph file read, before its string is added to the load.
struct ChronoImport {
    file: String,
    sessions: Vec<ballistics_store::chrono::Session>,
    chosen: usize,
    /// The cartridges' temperature when they were shot.
    temperature_c: f64,
}

/// A long shot to true the calculation to, as entered.
struct TruingForm {
    range_m: f64,
    /// In the rifle's click unit, up positive.
    elevation: f64,
    method: Option<TruingMethod>,
    error: Option<String>,
}

impl Default for TruingForm {
    fn default() -> Self {
        TruingForm {
            range_m: 300.0,
            elevation: 0.0,
            method: None,
            error: None,
        }
    }
}

/// The powder's temperature in these conditions.
fn powder_c(conditions: &Conditions) -> f64 {
    conditions
        .powder_temperature_c
        .unwrap_or(conditions.temperature_c)
}

/// The load in a line, for Calculate: "786 m/s at 3.6 °C, from the box".
pub fn summary(
    u: &units::UnitPrefs,
    bullet: &Bullet,
    load: Option<&RifleLoad>,
    conditions: &Conditions,
) -> String {
    let empty;
    let load = match load {
        Some(load) => load,
        None => {
            empty = RifleLoad::new("", &bullet.id);
            &empty
        }
    };
    let powder = powder_c(conditions);
    let model = load.velocity_model(bullet);
    let factor = match load.truing {
        Some(Truing {
            method: TruingMethod::Velocity,
            factor,
            ..
        }) => factor,
        _ => 1.0,
    };
    let source = match (load.velocities.len(), bullet.kind) {
        (0, BulletKind::Factory) => "from the box".to_owned(),
        (0, BulletKind::Handload) => "from your load data".to_owned(),
        (1, _) => "measured".to_owned(),
        (count, _) => format!("measured {count} times"),
    };
    let mut parts = vec![format!(
        "{} at {}, {source}",
        u.format(Quantity::Speed, model.velocity_at(powder) * factor),
        u.format(Quantity::Temperature, powder),
    )];
    if load.truing.is_some() {
        parts.push("trued".to_owned());
    }
    if load.impact_offset_mrad != [0.0, 0.0] {
        parts.push("own point of impact".to_owned());
    }
    parts.join(" · ")
}

impl LoadEditor {
    /// Drops unsaved edits, e.g. after the load was reset.
    pub fn forget(&mut self) {
        self.draft = None;
    }

    /// The load of `bullet` in `rifle`: velocities, point of impact, zero
    /// weather and truing, with save. `today` is the weather set on
    /// Calculate, which a truing shot is taken to have been fired in.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        rifle: &Rifle,
        bullet: &Bullet,
        saved: Option<&RifleLoad>,
        today: &Conditions,
        actions: &mut Vec<Action>,
    ) {
        if self
            .draft
            .as_ref()
            .is_none_or(|draft| draft.rifle_id != rifle.id || draft.bullet_id != bullet.id)
        {
            self.draft = Some(
                saved
                    .cloned()
                    .unwrap_or_else(|| RifleLoad::new(&rifle.id, &bullet.id)),
            );
            self.truing_form = TruingForm::default();
            self.truing_open = false;
            self.chrono = None;
            self.chrono_error = None;
        }
        let powder = powder_c(today);
        let air = today.temperature_c;
        // A truing shot is taken level: the cant is the shooter's, not the load's.
        let conditions = Conditions {
            cant_degrees: 0.0,
            ..today.clone()
        };
        if let Some((file, bytes)) = self.chrono_file.take() {
            self.chrono_error = None;
            match ballistics_store::chrono::parse(&bytes) {
                Ok(sessions) => {
                    self.chrono = Some(ChronoImport {
                        file,
                        sessions,
                        chosen: 0,
                        temperature_c: air.round(),
                    })
                }
                Err(error) => {
                    self.chrono = None;
                    self.chrono_error = Some(format!("{file}: {error}"));
                }
            }
        }
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        let u = units::prefs(ui.ctx());
        ui.label(summary(&u, bullet, Some(draft), today))
            .on_hover_text("The muzzle velocity in the weather set on Calculate");
        ui.add_space(4.0);

        let model = draft.velocity_model(bullet);
        panel(ui, "Measured velocities", |ui| {
            ui.weak(
                "Chronograph averages. Two or more at different temperatures give this rifle's \
                 own temperature sensitivity.",
            );
            let mut remove = None;
            egui::Grid::new("velocities")
                .num_columns(3)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    for (index, point) in draft.velocities.iter_mut().enumerate() {
                        units::drag(
                            ui,
                            &mut point.temperature_c,
                            Quantity::Temperature,
                            -60.0..=70.0,
                            0.5,
                        );
                        units::drag(
                            ui,
                            &mut point.velocity_mps,
                            Quantity::Speed,
                            50.0..=1500.0,
                            1.0,
                        );
                        ui.horizontal(|ui| {
                            if let Some(shots) = point.shots {
                                ui.weak(format!(
                                    "{} shots, SD {}, ES {}",
                                    shots.count,
                                    spread_text(&u, shots.sd_mps),
                                    spread_text(&u, shots.es_mps)
                                ));
                            }
                            if ui.small_button("✖").on_hover_text("Remove").clicked() {
                                remove = Some(index);
                            }
                        });
                        ui.end_row();
                    }
                });
            if let Some(index) = remove {
                draft.velocities.remove(index);
            }
            ui.horizontal_wrapped(|ui| {
                if draft.velocities.len() < 10 && ui.button("+ Add a reading").clicked() {
                    let velocity_mps = model.velocity_at(air).round();
                    draft.velocities.push(VelocityPoint {
                        temperature_c: air.round(),
                        velocity_mps,
                        shots: None,
                    });
                }
                if draft.velocities.len() < 10
                    && ui
                        .button("From a chronograph file…")
                        .on_hover_text(
                            "Garmin Xero (ShotView CSV), LabRadar (SR…Report.csv) or \
                             MagnetoSpeed (LOG.CSV)",
                        )
                        .clicked()
                {
                    self.chrono_file
                        .pick(ui.ctx(), "Chronograph files", &["csv", "CSV"]);
                }
            });
            if let Some(error) = self.chrono_error.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                    if ui.small_button("✖").clicked() {
                        self.chrono_error = None;
                    }
                });
            }
            let mut close = false;
            if let Some(import) = self.chrono.as_mut() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    import.chosen = import.chosen.min(import.sessions.len() - 1);
                    let session = &import.sessions[import.chosen];
                    ui.strong(format!("{} · {}", session.device.name(), import.file));
                    if import.sessions.len() > 1 {
                        egui::ComboBox::from_id_salt("chrono-session")
                            .selected_text(session.name.clone().unwrap_or_default())
                            .show_ui(ui, |ui| {
                                for (index, session) in import.sessions.iter().enumerate() {
                                    let label = format!(
                                        "{} ({} shots)",
                                        session.name.clone().unwrap_or_default(),
                                        session.velocities_mps.len()
                                    );
                                    ui.selectable_value(&mut import.chosen, index, label);
                                }
                            });
                    } else if let Some(name) = &session.name {
                        ui.weak(name);
                    }
                    let session = &import.sessions[import.chosen];
                    let stats = session.stats();
                    ui.label(format!(
                        "{} shot{}{}: average {}, SD {}, ES {}",
                        stats.count,
                        if stats.count == 1 { "" } else { "s" },
                        if session.missed > 0 {
                            format!(" ({} not read, left out)", session.missed)
                        } else {
                            String::new()
                        },
                        u.format(Quantity::Speed, session.average_mps()),
                        spread_text(&u, stats.sd_mps),
                        spread_text(&u, stats.es_mps),
                    ))
                    .on_hover_text(
                        "Worked out from the shots; the SD is a sample's (n − 1), a little \
                         larger than some chronograph apps show.",
                    );
                    let expected = model.velocity_at(import.temperature_c);
                    if (session.average_mps() / expected - 1.0).abs() > 0.15 {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            format!(
                                "That is far from the {} expected for this bullet: is it the \
                                 right file?",
                                u.format(Quantity::Speed, expected)
                            ),
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.label("The cartridges were at");
                        units::drag(
                            ui,
                            &mut import.temperature_c,
                            Quantity::Temperature,
                            -60.0..=70.0,
                            0.5,
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Add the average").clicked() {
                            draft.velocities.push(VelocityPoint {
                                temperature_c: import.temperature_c,
                                velocity_mps: session.average_mps(),
                                shots: Some(stats),
                            });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            if close {
                self.chrono = None;
            }
            ui.weak(format!(
                "Temperature sensitivity {} ({:+.1} % per 15 °C), {}.",
                u.format_signed(Quantity::Sensitivity, model.per_degree),
                model.percent_per_15c(),
                if model.fitted {
                    "from your measurements"
                } else if draft.velocities.len() >= 2 {
                    "from the bullet data (your measurements need to be at least 2 °C apart)"
                } else {
                    "from the bullet data"
                }
            ));
        });

        panel(ui, "In this rifle", |ui| {
            ui.strong("Point of impact at the zero range");
            ui.weak("Where this load hits compared with the load the rifle was zeroed with.");
            let unit = rifle.click_unit;
            grid(ui, "impact-offset", |ui| {
                for (label, index) in [("Up", 1), ("Right", 0)] {
                    ui.label(label);
                    let mut value = from_radians(unit, draft.impact_offset_mrad[index] / 1000.0);
                    let response = ui.add(
                        egui::DragValue::new(&mut value)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix(format!(" {}", unit_label(unit))),
                    );
                    if response.changed() {
                        draft.impact_offset_mrad[index] =
                            (to_radians(unit, value) * 1000.0).clamp(-49.0, 49.0);
                    }
                    // 1 mrad is 10 cm at 100 m.
                    let centimetres = draft.impact_offset_mrad[index] * rifle.zero_range_m / 10.0;
                    ui.weak(format!(
                        "{} at {}",
                        u.format_signed(Quantity::ShortLength, centimetres),
                        u.format(Quantity::Distance, rifle.zero_range_m)
                    ));
                    ui.end_row();
                }
            });

            ui.add_space(4.0);
            ui.strong("Zeroed in");
            let mut mode = match draft.zero {
                LoadZero::Rifle => 0,
                LoadZero::SameAsShot => 1,
                LoadZero::Recorded(_) => 2,
            };
            ui.radio_value(&mut mode, 0, "The rifle's zero weather");
            ui.radio_value(&mut mode, 1, "The same weather as the shot");
            ui.radio_value(&mut mode, 2, "This weather:");
            draft.zero = match (mode, draft.zero) {
                (0, _) => LoadZero::Rifle,
                (1, _) => LoadZero::SameAsShot,
                (_, LoadZero::Recorded(weather)) => LoadZero::Recorded(weather),
                _ => LoadZero::Recorded(rifle.zero_weather()),
            };
            if let LoadZero::Recorded(weather) = &mut draft.zero {
                grid(ui, "load-zero", |ui| {
                    quantity(
                        ui,
                        "Air",
                        &mut weather.temperature_c,
                        Quantity::Temperature,
                        -45.0..=50.0,
                        0.5,
                    );
                    quantity(
                        ui,
                        "Powder",
                        &mut weather.powder_temperature_c,
                        Quantity::Temperature,
                        -45.0..=60.0,
                        0.5,
                    );
                    quantity(
                        ui,
                        "Barometer at the range",
                        &mut weather.pressure_hpa,
                        Quantity::Pressure,
                        500.0..=1100.0,
                        1.0,
                    );
                    number(
                        ui,
                        "Humidity",
                        &mut weather.humidity_percent,
                        0.0..=100.0,
                        1.0,
                        " %",
                    );
                });
            }

            ui.add_space(4.0);
            self.truing_open |= draft.truing.is_some();
            if !self.truing_open {
                if ui
                    .button("Shot long and missed? True the calculation…")
                    .on_hover_text(
                        "Enter what it took to hit at a long range, and the calculation is \
                         adjusted to match",
                    )
                    .clicked()
                {
                    self.truing_open = true;
                }
            } else {
                truing(
                    ui,
                    &mut self.truing_form,
                    draft,
                    rifle,
                    bullet,
                    powder,
                    &conditions,
                );
            }
        });

        ui.add_space(4.0);
        let problem = draft.validate().err();
        let changed = match saved {
            Some(saved) => !same_content(saved, draft),
            None => !draft.is_empty(),
        };
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    changed && problem.is_none(),
                    egui::Button::new(RichText::new("Save").strong()),
                )
                .clicked()
            {
                actions.push(Action::SaveLoad(draft.clone()));
            }
            if changed && ui.button("Undo changes").clicked() {
                *draft = saved
                    .cloned()
                    .unwrap_or_else(|| RifleLoad::new(&rifle.id, &bullet.id));
            }
            if let Some(saved) = saved
                && confirm_button(ui, "remove-load", "Reset to the bullet's data")
            {
                actions.push(Action::DeleteLoad(saved.id.clone()));
            }
        });
        if let Some(problem) = problem {
            ui.colored_label(ui.visuals().warn_fg_color, problem);
        } else if changed {
            ui.weak("Unsaved changes: Calculate uses this once saved.");
        }
    }
}

/// A framed group with a heading.
fn panel(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(14.0));
            contents(ui);
        });
    ui.add_space(6.0);
}

/// The truing form: a long shot, what it took, and the result.
fn truing(
    ui: &mut egui::Ui,
    form: &mut TruingForm,
    draft: &mut RifleLoad,
    rifle: &Rifle,
    bullet: &Bullet,
    powder: f64,
    conditions: &Conditions,
) {
    let u = units::prefs(ui.ctx());
    let model = draft.velocity_model(bullet);
    ui.strong("Truing");
    ui.weak(format!(
        "Shot far and found the calculation off? Enter what it took to hit centre. The shot is \
         taken to be in the weather set on Calculate ({}, {}). The longest range you've shot \
         works best (300 m or more).",
        u.format(Quantity::Temperature, conditions.temperature_c),
        match conditions.pressure {
            ballistics_shot::Pressure::Station(hpa) => u.format(Quantity::Pressure, hpa),
            ballistics_shot::Pressure::SeaLevel { hpa, .. } =>
                format!("{} at sea level", u.format(Quantity::Pressure, hpa)),
        }
    ));
    if let Some(truing) = draft.truing {
        let what = match truing.method {
            TruingMethod::Velocity => format!(
                "velocity ×{:.3} ({} instead of {})",
                truing.factor,
                u.format(Quantity::Speed, model.velocity_at(powder) * truing.factor),
                u.format(Quantity::Speed, model.velocity_at(powder)),
            ),
            TruingMethod::Drag => format!(
                "drag ×{:.3}{}",
                truing.factor,
                bullet
                    .ballistic_coefficient()
                    .map_or(String::new(), |bc| format!(
                        " (BC {:.3} instead of {bc:.3})",
                        bc / truing.factor
                    )),
            ),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Trued to {}, {:.2} {} up, at {}: {what}.",
                u.format(Quantity::Distance, truing.range_m),
                from_radians(rifle.click_unit, truing.elevation_mrad / 1000.0),
                unit_label(rifle.click_unit),
                u.format(Quantity::Temperature, truing.weather.temperature_c),
            ));
            if ui.small_button("Remove").clicked() {
                draft.truing = None;
            }
        });
        if (truing.factor - 1.0).abs() > 0.1 {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "That's a large correction. Check the range, the zero and the clicks before \
                 trusting it.",
            );
        }
    }
    let measured = !draft.velocities.is_empty();
    let method = form.method.unwrap_or(if measured {
        TruingMethod::Drag
    } else {
        TruingMethod::Velocity
    });
    let unit = rifle.click_unit;
    grid(ui, "truing", |ui| {
        quantity(
            ui,
            "Range",
            &mut form.range_m,
            Quantity::Distance,
            50.0..=2000.0,
            5.0,
        );
        ui.label("It took");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut form.elevation)
                    .speed(0.05)
                    .fixed_decimals(2)
                    .suffix(format!(" {}", unit_label(unit))),
            );
            ui.label("up to hit centre");
            ui.weak(format!(
                "({})",
                crate::calculator::clicks_text((form.elevation / rifle.click_value).round() as i64)
            ));
        });
        ui.end_row();
        ui.label("Adjust");
        ui.vertical(|ui| {
            let mut chosen = method;
            ui.radio_value(&mut chosen, TruingMethod::Velocity, "the velocity")
                .on_hover_text("When the velocity is from the box or a guess: no chronograph");
            ui.radio_value(&mut chosen, TruingMethod::Drag, "the drag (BC)")
                .on_hover_text("When the velocity was measured, so the BC is what's uncertain");
            if chosen != method {
                form.method = Some(chosen);
            }
            ui.weak(if measured {
                "Your velocity is measured, so the drag is the likely culprit."
            } else {
                "Without a measured velocity, the velocity is the likely culprit."
            });
        });
        ui.end_row();
    });
    if ui.button("Calculate the truing").clicked() {
        let mut base = draft.clone();
        base.truing = None;
        let elevation_mrad = to_radians(unit, form.elevation) * 1000.0;
        match ballistics_shot::true_load(
            rifle,
            bullet,
            &base,
            method,
            form.range_m,
            elevation_mrad,
            conditions,
        ) {
            Ok(truing) => {
                draft.truing = Some(truing);
                form.error = None;
            }
            Err(error) => form.error = Some(error.to_string()),
        }
    }
    if let Some(error) = &form.error {
        ui.colored_label(ui.visuals().warn_fg_color, error);
    }
}

/// A velocity spread (SD, ES) with a decimal: they are small numbers.
fn spread_text(u: &units::UnitPrefs, mps: f64) -> String {
    format!(
        "{:.1} {}",
        u.show(Quantity::Speed, mps),
        u.label(Quantity::Speed)
    )
}

/// Whether two rifle loads hold the same data, ignoring when they were saved.
fn same_content(a: &RifleLoad, b: &RifleLoad) -> bool {
    a.velocities == b.velocities
        && a.impact_offset_mrad == b.impact_offset_mrad
        && a.zero == b.zero
        && a.notes == b.notes
        && a.truing == b.truing
}

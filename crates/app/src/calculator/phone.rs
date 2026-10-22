//! The calculator on a phone: results first, tiles for range, shot angle,
//! wind and target, and the inputs on a page of their own. The pickers
//! slide up as sheets (see `mobile.rs`); on a desktop they are dialogs.

use ballistics_core::Solution;
use ballistics_core::analysis::OptimalZero;
use ballistics_store::{Bullet, ClickUnit, Reticle, Rifle, RifleLoad, Scope, Target};
use eframe::egui::{
    self, Align2, FontId, Rect, RichText, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use ballistics_shot::{from_radians, unit_label};

use super::{
    Aim, CalculatorPage, PressureSource, RAD_PER_MOA, ResultsView, SOLVE_RANGE_M, ViewRequests,
    sight_impact, wind_description,
};
use crate::app::{Action, Data, Page};
use crate::figure::Figure;
use crate::mobile::{self, TOUCH};
use crate::sight_picture::{SightPicture, magnification_slider, magnification_text};
use crate::theme;
use crate::trajectory3d;
use crate::units::{self, Quantity};
use crate::widgets::caliber_name;
use crate::widgets::fold;

/// Which phone view of the calculator is showing.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum PhoneView {
    #[default]
    Results,
    /// All inputs, on their own page.
    Conditions,
}

/// The quick pickers, as sheets over the results.
#[derive(Clone, Copy, PartialEq)]
pub enum Picker {
    Range,
    Angle,
    Wind,
    Target,
}

/// What the calculator worked out this frame, handed to the phone view.
pub(super) struct PhoneFrame<'a> {
    pub rifle: Option<&'a Rifle>,
    pub bullets: &'a [&'a Bullet],
    pub bullet: Option<&'a Bullet>,
    pub load: Option<&'a RifleLoad>,
    pub optics: Option<&'a (Scope, Reticle)>,
    pub aim: &'a Aim,
    pub country: Option<ballistics_store::Country>,
}

/// Preset wind speeds (m/s) and what they feel like.
const WIND_PRESETS: [(f64, &str); 5] = [
    (0.0, "calm"),
    (2.0, "light"),
    (3.0, "gentle"),
    (6.0, "fresh"),
    (10.0, "strong"),
];

const ANGLE_PRESETS: [f64; 7] = [-30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0];

impl CalculatorPage {
    pub(super) fn show_phone(
        &mut self,
        context: &egui::Context,
        data: &Data,
        frame: PhoneFrame<'_>,
        actions: &mut Vec<Action>,
    ) {
        egui::CentralPanel::default().show(context, |ui| match self.phone_view {
            PhoneView::Conditions => {
                if mobile::back_button(ui, "Calculate") {
                    self.phone_view = PhoneView::Results;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.equipment(
                        ui,
                        data,
                        frame.rifle,
                        frame.bullets,
                        frame.bullet,
                        frame.load,
                        actions,
                    );
                    if frame.rifle.is_some() && frame.bullet.is_some() {
                        ui.add_space(6.0);
                        self.conditions(ui, data, frame.aim, frame.bullet, actions);
                    }
                });
            }
            PhoneView::Results => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.phone_results(ui, &frame, actions);
                });
            }
        });
        self.picker_sheet(context, data, frame.rifle);
    }

    /// The open picker's sheet, if any.
    pub(super) fn picker_sheet(
        &mut self,
        context: &egui::Context,
        data: &Data,
        rifle: Option<&Rifle>,
    ) {
        match self.picker {
            Some(Picker::Range) => {
                let note = self.range_note(context);
                self.range_sheet(context, rifle, note)
            }
            Some(Picker::Angle) => self.angle_sheet(context),
            Some(Picker::Wind) => self.wind_sheet(context),
            Some(Picker::Target) => self.target_sheet(context, data),
            None => {}
        }
    }

    /// For the range picker: how the range stands against the limits, from
    /// the last calculation.
    fn range_note(&self, context: &egui::Context) -> Option<String> {
        let u = units::prefs(context);
        let (ethical, point_blank) = self.limits;
        match (ethical, point_blank) {
            (Some(limit), _) if self.inputs.target_range_m > limit => Some(format!(
                "Beyond the ethical range ({})",
                u.format(Quantity::Distance, limit)
            )),
            (Some(limit), Some(blank)) => Some(format!(
                "Inside the ethical range ({}) · dead-on hold to {}",
                u.format(Quantity::Distance, limit),
                u.format(Quantity::Distance, blank)
            )),
            (Some(limit), None) => Some(format!(
                "Inside the ethical range ({})",
                u.format(Quantity::Distance, limit)
            )),
            (None, Some(blank)) => Some(format!(
                "Dead-on hold to {}",
                u.format(Quantity::Distance, blank)
            )),
            (None, None) => None,
        }
    }

    /// The results page.
    fn phone_results(&mut self, ui: &mut Ui, frame: &PhoneFrame<'_>, actions: &mut Vec<Action>) {
        let u = units::prefs(ui.ctx());
        // The setup: tapping it opens all the inputs.
        let title = frame.rifle.map_or_else(
            || "Choose a rifle".to_owned(),
            |rifle| format!("{} · {}", rifle.name, caliber_name(&rifle.caliber)),
        );
        let pressure = match self.inputs.pressure_source {
            PressureSource::Station => self.inputs.station_pressure_hpa,
            PressureSource::SeaLevelAndAltitude => self.inputs.sea_level_pressure_hpa,
        };
        let detail = format!(
            "{}{} · {}",
            frame
                .bullet
                .map(|bullet| format!("{} · ", bullet.display_name()))
                .unwrap_or_default(),
            u.format(Quantity::Temperature, self.inputs.temperature_c),
            u.format(Quantity::Pressure, pressure),
        );
        if mobile::list_row(ui, &title, &detail, false).clicked() {
            self.phone_view = PhoneView::Conditions;
        }

        let (Some(rifle), Some(bullet)) = (frame.rifle, frame.bullet) else {
            ui.add_space(8.0);
            match frame.rifle {
                None => {
                    ui.heading("Add a rifle to start");
                    if ui.button("Go to Rifles").clicked() {
                        actions.push(Action::Goto(Page::Rifles));
                    }
                    if ui.button("Load demo data").clicked() {
                        actions.push(Action::LoadDemo);
                    }
                }
                Some(rifle) => {
                    ui.heading(format!(
                        "No {} bullets in your collection",
                        caliber_name(&rifle.caliber)
                    ));
                    if ui.button("Go to Bullets").clicked() {
                        actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
                    }
                }
            }
            return;
        };

        self.tiles(ui, frame.aim);

        let table = self.table_for(rifle);
        let scenario = self.scenario(rifle, bullet, frame.load);
        self.refresh(
            ui.ctx(),
            scenario,
            frame.aim.vital_diameter_cm,
            table
                .end_m
                .max(SOLVE_RANGE_M)
                .max(self.inputs.target_range_m + 10.0),
        );
        let hits = self.hits_for(frame.aim, frame.load);
        let requests = {
            let computed = &self.cache.as_ref().expect("refresh fills the cache").1;
            match &computed.solution {
                Ok(solution) => {
                    let view = ResultsView {
                        hits,
                        inputs: &self.inputs,
                        rifle,
                        table,
                        optics: frame.optics,
                        aim: frame.aim,
                        min_velocity: frame.aim.min_velocity_mps.unwrap_or_else(|| {
                            ballistics_shot::min_velocity(bullet, self.inputs.min_velocity)
                        }),
                        assessment: frame
                            .country
                            .map(|country| ballistics_shot::assess(bullet, solution, country)),
                        zeros: computed.zeros,
                    };
                    let (ethical, point_blank) = view.limits_of(solution);
                    self.limits = (ethical, point_blank.map(|blank| blank.far_limit));
                    view.show_phone(ui, solution, computed.optimal.as_ref())
                }
                Err(error) => {
                    ui.heading("No solution");
                    ui.colored_label(ui.visuals().error_fg_color, error.to_string());
                    ViewRequests::default()
                }
            }
        };
        self.apply_requests(requests, rifle, ui.ctx(), actions);
    }

    fn range_sheet(
        &mut self,
        context: &egui::Context,
        rifle: Option<&Rifle>,
        note: Option<String>,
    ) {
        let inputs = &mut self.inputs;
        let shown = mobile::sheet(context, "range-picker", "Range", |ui| {
            let u = units::prefs(ui.ctx());
            let unit = u.unit(Quantity::Distance);
            ui.vertical_centered(|ui| {
                ui.scope(|ui| {
                    ui.style_mut().override_font_id = Some(FontId::proportional(40.0));
                    units::drag(
                        ui,
                        &mut inputs.target_range_m,
                        Quantity::Distance,
                        5.0..=super::MAX_RANGE_M,
                        1.0,
                    );
                });
                ui.weak("Tap the number to type an exact range");
                if let Some(note) = &note {
                    let colour = if note.starts_with("Beyond") {
                        ui.visuals().warn_fg_color
                    } else {
                        theme::MEADOW
                    };
                    ui.colored_label(colour, note);
                }
            });
            ui.add_space(6.0);
            ruler(ui, &mut inputs.target_range_m, unit);
            ui.add_space(6.0);
            let steps = [-25.0, -5.0, 5.0, 25.0];
            let width = (ui.available_width() - 3.0 * 8.0) / 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for step in steps {
                    let label = format!("{step:+.0}");
                    if ui
                        .add(egui::Button::new(label).min_size(vec2(width, TOUCH)))
                        .clicked()
                    {
                        let shown = unit.show(inputs.target_range_m) + step;
                        inputs.target_range_m = unit.base(shown.max(5.0));
                    }
                }
            });
            ui.add_space(6.0);
            // Distances from the rifle's dope table, and recent ranges.
            let mut presets: Vec<f64> = rifle
                .map(|rifle| {
                    let table = rifle.table;
                    let step = if table.end_m - table.start_m > 300.0 {
                        50.0
                    } else {
                        25.0
                    };
                    let mut range = (table.start_m / step).ceil() * step;
                    let mut presets = Vec::new();
                    while range <= table.end_m + 0.5 && presets.len() < 8 {
                        presets.push(range);
                        range += step;
                    }
                    presets
                })
                .unwrap_or_default();
            for recent in &inputs.recent_ranges {
                if !presets.iter().any(|preset| (preset - recent).abs() < 0.5) {
                    presets.push(*recent);
                }
            }
            ui.weak("From your range table, and recent");
            ui.horizontal_wrapped(|ui| {
                for preset in presets {
                    let selected = (preset - inputs.target_range_m).abs() < 0.5;
                    let label = format!("{:.0}", unit.show(preset));
                    if ui
                        .add(
                            egui::Button::new(label)
                                .selected(selected)
                                .min_size(vec2(56.0, 40.0)),
                        )
                        .clicked()
                    {
                        inputs.target_range_m = preset;
                    }
                }
            });
        });
        if shown.closed {
            // Remember an odd range for next time.
            let range = (self.inputs.target_range_m * 10.0).round() / 10.0;
            let recent = &mut self.inputs.recent_ranges;
            recent.retain(|other| (other - range).abs() > 0.5);
            recent.insert(0, range);
            recent.truncate(3);
            self.picker = None;
        }
    }

    fn angle_sheet(&mut self, context: &egui::Context) {
        let inputs = &mut self.inputs;
        let shown = mobile::sheet(context, "angle-picker", "Shot angle", |ui| {
            ui.horizontal(|ui| {
                inclinometer(ui, &mut inputs.inclination_degrees);
                ui.vertical(|ui| {
                    ui.weak("Angle to the target");
                    ui.scope(|ui| {
                        ui.style_mut().override_font_id = Some(FontId::proportional(36.0));
                        ui.add(
                            egui::DragValue::new(&mut inputs.inclination_degrees)
                                .range(-80.0..=80.0)
                                .speed(0.5)
                                .max_decimals(1)
                                .suffix("°"),
                        );
                    });
                    let angle = inputs.inclination_degrees;
                    let (text, colour) = if angle > 0.5 {
                        ("uphill", theme::AMBER)
                    } else if angle < -0.5 {
                        ("downhill", theme::AMBER)
                    } else {
                        ("level", theme::BONE_MUTED)
                    };
                    ui.colored_label(colour, text);
                    ui.weak("The holds take the angle into account.");
                });
            });
            #[cfg(target_arch = "wasm32")]
            measure_with_phone(ui, &mut inputs.inclination_degrees);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Cant");
                ui.add(
                    egui::DragValue::new(&mut inputs.cant_degrees)
                        .range(-30.0..=30.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix("°"),
                );
                ui.weak("rifle tilted; + with the scope's top to the right");
            });
            ui.add_space(6.0);
            let width = (ui.available_width() - 6.0 * 6.0) / 7.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.spacing_mut().button_padding.x = 2.0;
                for preset in ANGLE_PRESETS {
                    let selected = (inputs.inclination_degrees - preset).abs() < 0.25;
                    if ui
                        .add(
                            egui::Button::new(format!("{preset:+.0}"))
                                .selected(selected)
                                .min_size(vec2(width, TOUCH)),
                        )
                        .clicked()
                    {
                        inputs.inclination_degrees = preset;
                    }
                }
            });
        });
        if shown.closed {
            self.picker = None;
        }
    }

    fn wind_sheet(&mut self, context: &egui::Context) {
        let inputs = &mut self.inputs;
        let report = self.weather.report.clone();
        let shown = mobile::sheet(context, "wind-picker", "Wind", |ui| {
            let u = units::prefs(ui.ctx());
            ui.vertical_centered(|ui| {
                ui.weak("Tap where the wind comes from · 12 is towards the target");
                wind_clock(ui, &mut inputs.wind_clock);
                ui.label(
                    RichText::new(format!(
                        "From {} · {}",
                        clock_text(inputs.wind_clock),
                        wind_description(inputs.wind_clock)
                    ))
                    .strong(),
                );
            });
            ui.add_space(8.0);
            let unit = u.unit(Quantity::WindSpeed);
            ui.horizontal(|ui| {
                let side = vec2(56.0, 52.0);
                let step = unit.base(unit.show(0.0) + 0.5) - unit.base(unit.show(0.0));
                if ui
                    .add(egui::Button::new(RichText::new("−").size(24.0)).min_size(side))
                    .clicked()
                {
                    inputs.wind_speed = (inputs.wind_speed - step).max(0.0);
                }
                let middle = ui.available_width() - side.x - 10.0;
                ui.allocate_ui_with_layout(
                    vec2(middle, side.y),
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            RichText::new(u.format(Quantity::WindSpeed, inputs.wind_speed))
                                .size(30.0)
                                .strong(),
                        );
                    },
                );
                if ui
                    .add(egui::Button::new(RichText::new("+").size(24.0)).min_size(side))
                    .clicked()
                {
                    inputs.wind_speed = (inputs.wind_speed + step).min(30.0);
                }
            });
            ui.add_space(6.0);
            let width = (ui.available_width() - 4.0 * 6.0) / 5.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (speed, feel) in WIND_PRESETS {
                    let selected = (inputs.wind_speed - speed).abs() < 0.05;
                    let label = format!("{:.0}\n{feel}", unit.show(speed));
                    if ui
                        .add(
                            egui::Button::new(label)
                                .selected(selected)
                                .min_size(vec2(width, 52.0)),
                        )
                        .clicked()
                    {
                        inputs.wind_speed = speed;
                    }
                }
            });
            if let Some(report) = &report {
                ui.add_space(8.0);
                let speed = crate::weather::shooting_height_speed(report);
                let clock =
                    crate::weather::wind_clock(report.wind_from_degrees, inputs.azimuth_degrees);
                ui.weak(format!(
                    "Forecast: {} from {} at {}, shooting towards {:.0}° {} (set under Conditions)",
                    u.format(Quantity::WindSpeed, report.wind_speed_mps),
                    crate::weather::compass_point(report.wind_from_degrees),
                    report.place.name,
                    inputs.azimuth_degrees,
                    crate::weather::compass_point(inputs.azimuth_degrees),
                ));
                if ui
                    .add(
                        egui::Button::new(format!(
                            "Use the forecast: {} from {}",
                            u.format(Quantity::WindSpeed, speed),
                            clock_text(clock)
                        ))
                        .min_size(vec2(ui.available_width(), 44.0)),
                    )
                    .clicked()
                {
                    inputs.wind_speed = (speed * 10.0).round() / 10.0;
                    inputs.wind_clock = clock;
                }
            }
        });
        if shown.closed {
            self.picker = None;
        }
    }

    fn target_sheet(&mut self, context: &egui::Context, data: &Data) {
        let mut chosen: Option<Option<String>> = None;
        let current = self.inputs.target_id.clone();
        let recent = self.inputs.recent_targets.clone();
        let search = &mut self.target_search;
        let shown = mobile::sheet(context, "target-picker", "Target", |ui| {
            ui.add(
                egui::TextEdit::singleline(search)
                    .hint_text("Search: roe, rådjur, fox, ISSF…")
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, 40.0)),
            );
            ui.add_space(4.0);
            let row = |ui: &mut Ui, target: &Target| {
                mobile::list_row(
                    ui,
                    &target.name,
                    &target_summary(ui, target, data.hunting),
                    current.as_deref() == Some(target.id.as_str()),
                )
                .clicked()
            };
            let query = search.trim().to_lowercase();
            if query.is_empty() {
                if mobile::list_row(
                    ui,
                    "No target",
                    "Just the reticle and the impact point",
                    current.is_none(),
                )
                .clicked()
                {
                    chosen = Some(None);
                }
                let recent: Vec<Target> = recent.iter().filter_map(|id| data.target(id)).collect();
                if !recent.is_empty() {
                    ui.add_space(6.0);
                    ui.weak("RECENT");
                    for target in &recent {
                        if row(ui, target) {
                            chosen = Some(Some(target.id.clone()));
                        }
                    }
                }
            }
            let mine: Vec<&Target> = data.targets.iter().filter(|t| t.matches(&query)).collect();
            if !mine.is_empty() {
                ui.add_space(6.0);
                ui.weak("MY TARGETS");
                for target in mine {
                    if row(ui, target) {
                        chosen = Some(Some(target.id.clone()));
                    }
                }
            }
            ui.add_space(6.0);
            crate::targets::catalog_groups(ui, &query, |ui, target| {
                if row(ui, target) {
                    chosen = Some(Some(target.id.clone()));
                }
            });
        });
        if let Some(target) = chosen {
            if target != self.inputs.target_id {
                self.inputs.target_id = target;
                // A new target brings its own zone and energy.
                self.inputs.min_energy_override_j = None;
                self.inputs.min_velocity_override_mps = None;
            }
            self.target_search.clear();
            self.picker = None;
        } else if shown.closed {
            self.target_search.clear();
            self.picker = None;
        }
    }
}

impl ResultsView<'_> {
    /// The results on a phone: the hold, the go/no-go line, the scope view,
    /// and the rest folded away.
    fn show_phone(
        &self,
        ui: &mut Ui,
        solution: &Solution,
        optimal: Option<&OptimalZero>,
    ) -> ViewRequests {
        let mut requests = ViewRequests::default();
        let (ethical, point_blank) = self.limits_of(solution);

        ui.add_space(4.0);
        match solution.at(self.inputs.target_range_m) {
            Some(row) => self.hold_card(ui, &row, ethical),
            None => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "The bullet does not reach this range.",
                );
            }
        }
        if let Some(statuses) = self.statuses() {
            // One line; open it for the pills and the region.
            let mut job = egui::text::LayoutJob::default();
            for (index, pill) in statuses.pills.iter().enumerate() {
                let (mark, colour) = match pill.tone {
                    crate::hunting::Tone::Open => ("✔", theme::MEADOW),
                    crate::hunting::Tone::Closed => ("✖", theme::ALARM),
                    crate::hunting::Tone::Warn => ("!", ui.visuals().warn_fg_color),
                    crate::hunting::Tone::Plain => ("·", theme::BONE_MUTED),
                };
                job.append(
                    &format!("{mark} {}", pill.what),
                    if index == 0 { 0.0 } else { 14.0 },
                    egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), colour),
                );
            }
            egui::CollapsingHeader::new(job)
                .id_salt("phone-status")
                .show(ui, |ui| self.go_no_go(ui, &statuses, &mut requests));
        }
        self.warnings(ui, solution);
        ui.add_space(4.0);
        self.phone_scope(ui, solution, &mut requests);
        ui.add_space(6.0);

        let limits_title = if self.aim.is_game() {
            "Hunting limits"
        } else {
            "Limits"
        };
        let u = units::prefs(ui.ctx());
        let summary = match (ethical, point_blank) {
            (Some(limit), _) => format!("{} max", u.format(Quantity::Distance, limit)),
            (None, Some(blank)) => format!(
                "dead-on to {}",
                u.format(Quantity::Distance, blank.far_limit)
            ),
            (None, None) => String::new(),
        };
        if let Some(height) = fold(ui, "phone-limits", limits_title, &summary, |ui| {
            self.limits(ui, solution, optimal, ethical, point_blank)
        })
        .flatten()
        {
            requests.zero_height = Some(height);
        }
        egui::CollapsingHeader::new(RichText::new("Range table").strong())
            .id_salt("phone-table")
            .show(ui, |ui| {
                self.table_heading_phone(ui, &mut requests);
                let all_id = ui.id().with("all-columns");
                let all = ui
                    .data(|data| data.get_temp::<bool>(all_id))
                    .unwrap_or(false);
                let left_out = egui::ScrollArea::horizontal()
                    .id_salt("phone-table-scroll")
                    .show(ui, |ui| {
                        self.table(ui, solution, ethical, (!all).then_some(PHONE_COLUMNS))
                    })
                    .inner;
                if left_out || all {
                    let label = if all {
                        "Fewer columns"
                    } else {
                        "Show all columns"
                    };
                    if ui.button(label).clicked() {
                        ui.data_mut(|data| data.insert_temp(all_id, !all));
                    }
                }
            });
        egui::CollapsingHeader::new(RichText::new("Trajectory · 2D / 3D").strong())
            .id_salt("phone-trajectory")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let three_d = self.inputs.trajectory_3d;
                    if ui.selectable_label(!three_d, "2D").clicked() {
                        requests.trajectory_3d = Some(false);
                    }
                    if ui.selectable_label(three_d, "3D").clicked() {
                        requests.trajectory_3d = Some(true);
                    }
                });
                if self.inputs.trajectory_3d {
                    let scene = trajectory3d::Scene {
                        samples: &solution.samples,
                        target_range: self.inputs.target_range_m,
                        inclination_degrees: self.inputs.inclination_degrees,
                        figure: self.aim.figure.as_ref(),
                        wind_speed: self.inputs.wind_speed,
                        wind_from_degrees: self.inputs.wind_clock * 30.0,
                    };
                    trajectory3d::show(ui, &scene, 320.0);
                } else {
                    self.plot(
                        ui,
                        solution,
                        self.aim.vital_diameter_cm.map(|cm| cm / 200.0),
                        ethical,
                    );
                }
            });
        requests
    }

    /// The phone table's ranges, and the button for its columns.
    fn table_heading_phone(&self, ui: &mut Ui, requests: &mut ViewRequests) {
        let u = units::prefs(ui.ctx());
        let table = self.table;
        let id = ui.id().with("columns-open");
        ui.horizontal(|ui| {
            ui.weak(format!(
                "{}–{} every {}",
                u.format(Quantity::Distance, table.start_m),
                u.format(Quantity::Distance, table.end_m),
                u.format(Quantity::Distance, table.step_m)
            ));
            if ui.button("Columns…").clicked() {
                ui.data_mut(|data| data.insert_temp(id, true));
            }
        });
        if ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false) {
            let mut changed = table;
            let shown = mobile::sheet(ui.ctx(), "table-columns", "Range table", |ui| {
                super::table_settings(ui, &mut changed, self.rifle.click_unit);
            });
            if changed != table {
                requests.table = Some(changed);
            }
            if shown.closed {
                ui.data_mut(|data| data.insert_temp(id, false));
            }
        }
    }

    /// The holds in two big columns, with speed and energy under them.
    fn hold_card(&self, ui: &mut Ui, row: &ballistics_core::RangeRow, ethical: Option<f64>) {
        let unit = self.unit();
        let elevation = from_radians(unit, row.elevation_correction);
        let windage = from_radians(unit, row.windage_correction);
        let u = units::prefs(ui.ctx());
        egui::Frame::new()
            .fill(theme::FOREST_700)
            .corner_radius(12)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(2, |columns| {
                    let hold = |ui: &mut Ui, arrow: &str, value: f64, clicks: i64, way: &str| {
                        ui.label(
                            RichText::new(format!(
                                "{arrow} {:.2} {}",
                                value.abs(),
                                unit_label(unit)
                            ))
                            .size(24.0)
                            .strong(),
                        );
                        ui.weak(format!("{} {way}", super::clicks_text(clicks.abs())));
                    };
                    hold(
                        &mut columns[0],
                        if elevation >= 0.0 { "⬆" } else { "⬇" },
                        elevation,
                        self.clicks(elevation),
                        if elevation >= 0.0 { "up" } else { "down" },
                    );
                    columns[1].with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        hold(
                            ui,
                            if windage >= 0.0 { "➡" } else { "⬅" },
                            windage,
                            self.clicks(windage),
                            if windage >= 0.0 { "right" } else { "left" },
                        );
                    });
                });
                ui.weak(format!(
                    "{} · {} · {:.2} s",
                    u.format(Quantity::Speed, row.velocity),
                    u.format(Quantity::Energy, row.energy_j),
                    row.time
                ));
                if self.inputs.target_speed > 0.0 {
                    let lead = ballistics_core::analysis::moving_target_lead(
                        row,
                        self.inputs.target_speed,
                    );
                    ui.label(format!(
                        "Lead {} ({:.2} {})",
                        u.format(Quantity::ShortLength, lead.distance * 100.0),
                        from_radians(unit, lead.angle),
                        unit_label(unit)
                    ));
                }
                if ethical.is_some_and(|limit| row.range > limit) {
                    ui.colored_label(ui.visuals().error_fg_color, "⚠ Beyond your ethical range");
                }
            });
    }

    /// The scope view with its magnification and width beside it, the
    /// slider, and whether the shot is dialled or held.
    fn phone_scope(&self, ui: &mut Ui, solution: &Solution, requests: &mut ViewRequests) {
        let Some((scope, reticle)) = self.optics else {
            ui.weak(
                "Choose a scope and reticle for this rifle (Rifles) to see the view through it.",
            );
            return;
        };
        let Some(row) = solution.at(self.inputs.target_range_m) else {
            return;
        };
        let magnification =
            scope.clamp_magnification(self.inputs.magnification.unwrap_or(scope.magnification_max));
        let click_mrad = match self.rifle.click_unit {
            ClickUnit::Mil => self.rifle.click_value,
            ClickUnit::Moa => self.rifle.click_value * RAD_PER_MOA * 1000.0,
        };
        let (impact_mrad, dialled_clicks) =
            sight_impact(&row, click_mrad, self.inputs.dial_elevation);

        // The picture centred, as wide as the screen allows.
        let width = ui.available_width();
        let size = width.min(340.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, size), Sense::hover());
        let picture_rect = Rect::from_center_size(rect.center(), vec2(size, size));
        let mut picture_ui = ui.new_child(UiBuilder::new().max_rect(picture_rect));
        let drawn = SightPicture {
            scope,
            reticle,
            magnification,
            impact_mrad,
            figure: self.aim.figure.as_ref(),
            range_m: row.range,
            illumination: self.inputs.illumination,
            spread_mrad: self.hits.map(|hits| {
                let per_metre = 1000.0 / row.range.max(1.0);
                [hits.spread.across * per_metre, hits.spread.up * per_metre]
            }),
        }
        .show(&mut picture_ui, size);

        let mut chosen = magnification;
        let margin = ((width - size) / 2.0 - ui.spacing().item_spacing.x).max(0.0);
        ui.horizontal(|ui| {
            ui.add_space(margin);
            if magnification_slider(ui, &mut chosen, scope, size).changed() {
                requests.magnification = Some(chosen);
            }
        });
        // The magnification and the view's width, under the slider.
        ui.vertical_centered(|ui| {
            ui.small(format!(
                "{} · {:.1} m across at 100 m",
                magnification_text(magnification),
                drawn.field_of_view_mrad / 10.0
            ));
        });
        super::view_toggles(ui, self.inputs, scope, requests);
        if self.inputs.dial_elevation {
            ui.small(format!(
                "Dial {} {}, then hold the orange dot on the target.",
                super::clicks_text(dialled_clicks.abs() as i64),
                if dialled_clicks >= 0.0 { "up" } else { "down" }
            ));
        } else {
            ui.small("Hold the orange dot on the target.");
        }
        if let Some(hits) = self.hits {
            ui.label(format!(
                "Hit chance {} in the {}",
                super::percent(hits.chance),
                self.zone_word()
            ));
        }
        self.bracket_line(ui);
        if !drawn.impact_in_view {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The impact is outside the view: zoom out or dial.",
            );
        }
    }
}

/// The most columns the phone's range table shows besides the range,
/// unless all are asked for.
const PHONE_COLUMNS: usize = 3;

/// The small drawing at the right of a quick field, following its value.
pub(super) enum Glyph<'a> {
    /// A scale with the range marked, as a share of the scale (0 to 1).
    Range(f32),
    /// A horizon with the line of fire at this inclination, degrees.
    Angle(f32),
    /// A clock face with the wind arrow: the clock direction, and the
    /// arrow's length as a share (0 to 1).
    Wind { clock: f32, strength: f32 },
    /// The target at a glance; none without a target.
    Figure(Option<&'a Figure>),
}

impl Glyph<'_> {
    fn width(&self) -> f32 {
        match self {
            Glyph::Range(_) => 56.0,
            Glyph::Angle(_) => 44.0,
            Glyph::Wind { .. } => 40.0,
            Glyph::Figure(_) => 60.0,
        }
    }

    fn paint(&self, painter: &egui::Painter, rect: egui::Rect) {
        let thin = Stroke::new(1.5_f32, theme::FOREST_500);
        let accent = Stroke::new(2.5_f32, theme::BLAZE);
        match *self {
            Glyph::Range(share) => {
                let y = rect.center().y + 6.0;
                let (x0, x1) = (rect.left() + 2.0, rect.right() - 2.0);
                painter.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], thin);
                for tick in 0..=4 {
                    let x = x0 + (x1 - x0) * tick as f32 / 4.0;
                    let height = if tick % 2 == 0 { 5.0 } else { 3.0 };
                    painter.line_segment([egui::pos2(x, y - height), egui::pos2(x, y)], thin);
                }
                let x = x0 + (x1 - x0) * share.clamp(0.0, 1.0);
                painter.line_segment([egui::pos2(x0, y), egui::pos2(x, y)], accent);
                painter.circle_filled(egui::pos2(x, y), 4.0, theme::BLAZE);
                painter.circle_filled(egui::pos2(x, y), 1.5, theme::FOREST_700);
            }
            Glyph::Angle(degrees) => {
                let pivot = egui::pos2(rect.left() + 3.0, rect.center().y + 8.0);
                let reach = rect.width() - 6.0;
                dashed(painter, pivot, pivot + vec2(reach, 0.0), thin, 3.0, 3.0);
                let angle = degrees.clamp(-60.0, 60.0).to_radians();
                let tip = pivot + vec2(angle.cos(), -angle.sin()) * reach;
                painter.line_segment([pivot, tip], accent);
                painter.circle_filled(pivot, 2.5, theme::BLAZE);
            }
            Glyph::Wind { clock, strength } => {
                let centre = rect.center();
                let radius = rect.height() / 2.0 - 1.0;
                painter.circle_stroke(centre, radius, thin);
                for hour in 0..12 {
                    let a = hour as f32 / 12.0 * std::f32::consts::TAU;
                    let dir = vec2(a.sin(), -a.cos());
                    let inner = if hour % 3 == 0 { 5.0 } else { 2.5 };
                    let stroke = if hour % 3 == 0 {
                        Stroke::new(1.5_f32, theme::BONE_MUTED)
                    } else {
                        thin
                    };
                    painter.line_segment(
                        [centre + dir * (radius - inner), centre + dir * radius],
                        stroke,
                    );
                }
                // The wind blows from its clock position towards the shooter.
                let a = clock / 12.0 * std::f32::consts::TAU;
                let from = vec2(a.sin(), -a.cos());
                let length = radius * (0.35 + 0.65 * strength.clamp(0.0, 1.0));
                let tail = centre + from * radius * 0.9;
                let head = tail - from * length;
                painter.line_segment([tail, head], accent);
                let side = vec2(-from.y, from.x) * 3.5;
                let back = from * 4.5;
                painter.add(egui::Shape::convex_polygon(
                    vec![head, head + back + side, head + back - side],
                    theme::BLAZE,
                    Stroke::NONE,
                ));
                painter.circle_filled(centre, 2.0, theme::BONE);
            }
            Glyph::Figure(figure) => {
                let Some(figure) = figure else { return };
                let [low, high] = figure.bounds;
                let (w, h) = (high[0] - low[0], high[1] - low[1]);
                if w <= 0.0 || h <= 0.0 {
                    return;
                }
                let scale = (rect.width() as f64 / w).min(rect.height() as f64 / h);
                let centre = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
                let at = |[x, y]: [f64; 2]| {
                    rect.center() + vec2(((x - centre[0]) * scale) as f32, -((y - centre[1]) * scale) as f32)
                };
                for part in &figure.parts {
                    painter.add(part.shape(part.points.iter().map(|&p| at(p)).collect()));
                }
            }
        }
    }
}

fn dashed(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, stroke: Stroke, dash: f32, gap: f32) {
    let length = (to - from).length();
    let dir = (to - from) / length.max(1e-6);
    let mut t = 0.0;
    while t < length {
        let end = (t + dash).min(length);
        painter.line_segment([from + dir * t, from + dir * end], stroke);
        t += dash + gap;
    }
}

/// A quick field: a small label over a big value, the size of a thumb,
/// with a glyph that follows the value: at the right on a wide tile, in a
/// strip under the value on a narrow one (a phone), which is taller.
pub(super) fn quick_field(
    ui: &mut Ui,
    width: f32,
    label: &str,
    value: &str,
    glyph: Glyph<'_>,
) -> egui::Response {
    let strip = width < 150.0;
    let height = if strip { 84.0 } else { 58.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        theme::FOREST_600
    } else {
        theme::FOREST_700
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        10,
        fill,
        Stroke::new(1.0_f32, theme::FOREST_600),
        egui::StrokeKind::Inside,
    );
    let text_width = |text: &str, size: f32| {
        painter
            .layout_no_wrap(text.to_owned(), FontId::proportional(size), theme::BONE)
            .size()
            .x
    };
    let glyph_rect = if strip {
        // Scales stretch across; a clock or an animal keeps its own width.
        let w = match glyph {
            Glyph::Range(_) | Glyph::Angle(_) => rect.width() - 20.0,
            Glyph::Wind { .. } | Glyph::Figure(_) => glyph.width().min(rect.width() - 20.0),
        };
        Some(egui::Rect::from_min_size(
            egui::pos2(rect.left() + 10.0, rect.bottom() - 9.0 - 22.0),
            vec2(w, 22.0),
        ))
    } else {
        // Beside the text when it leaves room, shrunk to the room there is.
        let room = rect.width() - 26.0 - text_width(label, 12.0).max(text_width(value, 19.0));
        (room >= 32.0).then(|| {
            let w = glyph.width().min(room);
            egui::Rect::from_min_size(
                egui::pos2(rect.right() - 10.0 - w, rect.top() + 9.0),
                vec2(w, 40.0),
            )
        })
    };
    if let Some(glyph_rect) = glyph_rect {
        glyph.paint(&painter.with_clip_rect(glyph_rect.expand(1.0)), glyph_rect);
    }
    let painter = painter.with_clip_rect(rect.shrink(4.0));
    // A label too long for the tile keeps its first part: the glyph shows
    // the rest ("Wind · 3 o'clock" becomes "Wind").
    let label = match label.split_once(" · ") {
        Some((head, _)) if text_width(label, 12.0) > rect.width() - 20.0 => head,
        _ => label,
    };
    painter.text(
        rect.left_top() + vec2(10.0, 9.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(12.0),
        theme::BONE_MUTED,
    );
    let value_bottom = if strip { rect.bottom() - 9.0 - 22.0 - 4.0 } else { rect.bottom() - 9.0 };
    painter.text(
        egui::pos2(rect.left() + 10.0, value_bottom),
        Align2::LEFT_BOTTOM,
        value,
        FontId::proportional(19.0),
        theme::BONE,
    );
    response
}

/// "3 o'clock", "2:30".
pub(super) fn clock_text(clock: f64) -> String {
    let hour = clock.rem_euclid(12.0);
    let hour = if hour < 0.25 { 12.0 } else { hour };
    if (hour - hour.round()).abs() < 0.01 {
        format!("{:.0} o'clock", hour.round())
    } else {
        format!(
            "{}:{:02.0}",
            hour.floor() as i64,
            (hour.fract() * 60.0).round()
        )
    }
}

fn target_summary(ui: &Ui, target: &Target, hunting: bool) -> String {
    let u = units::prefs(ui.ctx());
    let zone = u.format(Quantity::ShortLength, target.vital_diameter_cm);
    match target.min_energy_j {
        Some(energy) if target.is_game() && hunting => {
            format!("{zone} · {}", u.format(Quantity::Energy, energy))
        }
        _ if target.is_game() => zone,
        _ => format!("{zone} · sport"),
    }
}

/// A drag ruler for the range: drag sideways in steps of 5.
fn ruler(ui: &mut Ui, metres: &mut f64, unit: units::Unit) {
    const PIXELS_PER_UNIT: f32 = 3.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 74.0), Sense::drag());
    let id = response.id.with("ruler");
    // The unsnapped value while dragging, so small moves add up.
    if response.drag_started() {
        ui.data_mut(|data| data.insert_temp(id, unit.show(*metres)));
    }
    if response.dragged() {
        let mut value: f64 = ui
            .data(|data| data.get_temp(id))
            .unwrap_or(unit.show(*metres));
        value -= f64::from(response.drag_delta().x / PIXELS_PER_UNIT);
        value = value.max(5.0);
        ui.data_mut(|data| data.insert_temp(id, value));
        *metres = unit.base((value / 5.0).round() * 5.0);
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 12, theme::FOREST_900);
    let shown = unit.show(*metres);
    let center = rect.center().x;
    let first = ((shown - 80.0) / 5.0).floor() as i64;
    for index in first..first + 33 {
        let value = index as f64 * 5.0;
        if value < 0.0 {
            continue;
        }
        let x = center + (value - shown) as f32 * PIXELS_PER_UNIT;
        if x < rect.left() || x > rect.right() {
            continue;
        }
        let major = index % 5 == 0;
        let (top, colour, width) = if major {
            (rect.top() + 26.0, theme::BONE, 2.0_f32)
        } else {
            (rect.top() + 34.0, theme::BONE_MUTED, 1.2_f32)
        };
        painter.line_segment(
            [pos2(x, top), pos2(x, rect.top() + 46.0)],
            Stroke::new(width, colour),
        );
        if major && (x - center).abs() > 24.0 {
            painter.text(
                pos2(x, rect.bottom() - 10.0),
                Align2::CENTER_BOTTOM,
                format!("{value:.0}"),
                FontId::proportional(12.0),
                theme::BONE_MUTED,
            );
        }
    }
    painter.line_segment(
        [
            pos2(center, rect.top() + 6.0),
            pos2(center, rect.top() + 48.0),
        ],
        Stroke::new(2.5_f32, theme::BLAZE),
    );
    painter.add(Shape::convex_polygon(
        vec![
            pos2(center - 7.0, rect.top() + 6.0),
            pos2(center + 7.0, rect.top() + 6.0),
            pos2(center, rect.top() + 16.0),
        ],
        theme::BLAZE,
        Stroke::NONE,
    ));
    let badge = Rect::from_center_size(pos2(center, rect.bottom() - 17.0), vec2(40.0, 20.0));
    painter.rect_filled(badge, 4, theme::BLAZE);
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        format!("{shown:.0}"),
        FontId::proportional(13.0),
        theme::FOREST_900,
    );
}

/// A side-view dial for the shot angle: drag the needle up or down.
fn inclinometer(ui: &mut Ui, degrees: &mut f64) {
    let (rect, response) = ui.allocate_exact_size(vec2(180.0, 220.0), Sense::click_and_drag());
    let center = pos2(rect.left() + 24.0, rect.center().y);
    let radius = (rect.height() / 2.0 - 20.0).min(rect.width() - 50.0);
    if let Some(pointer) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let angle = (-(pointer.y - center.y))
            .atan2(pointer.x - center.x)
            .to_degrees();
        *degrees = (f64::from(angle) * 2.0).round() / 2.0;
        *degrees = degrees.clamp(-60.0, 60.0);
    }
    let painter = ui.painter_at(rect.expand(4.0));
    let at = |angle: f32, r: f32| {
        let radians = angle.to_radians();
        center + vec2(radians.cos(), -radians.sin()) * r
    };
    let mut fan = vec![center];
    for step in -12..=12 {
        fan.push(at(step as f32 * 5.0, radius));
    }
    painter.add(Shape::convex_polygon(
        fan,
        theme::FOREST_900,
        Stroke::new(1.5_f32, theme::FOREST_600),
    ));
    for step in -6..=6 {
        let angle = step as f32 * 10.0;
        painter.line_segment(
            [at(angle, radius - 12.0), at(angle, radius)],
            Stroke::new(1.5_f32, theme::BONE_MUTED),
        );
        if step % 2 == 0 {
            painter.text(
                at(angle, radius + 14.0),
                Align2::CENTER_CENTER,
                format!("{angle:+.0}"),
                FontId::proportional(11.0),
                theme::BONE_MUTED,
            );
        }
    }
    painter.line_segment(
        [center, at(0.0, radius)],
        Stroke::new(1.0_f32, theme::FOREST_600),
    );
    let tip = at(*degrees as f32, radius - 6.0);
    painter.line_segment([center, tip], Stroke::new(4.0_f32, theme::BLAZE));
    painter.circle(tip, 9.0, theme::BONE, Stroke::new(2.5_f32, theme::BLAZE));
    painter.circle_filled(center, 7.0, theme::BONE);
}

/// Measure the angle with the phone's tilt sensor: hold the phone flat,
/// screen up, and point its top edge at the target.
#[cfg(target_arch = "wasm32")]
fn measure_with_phone(ui: &mut Ui, degrees: &mut f64) {
    if !crate::web::tilt_supported() {
        return;
    }
    ui.add_space(6.0);
    match crate::web::tilt() {
        None => {
            let button = egui::Button::new(
                "Measure with the phone\nHold it flat, screen up, top edge towards the target",
            )
            .min_size(vec2(ui.available_width(), 56.0));
            if ui.add(button).clicked() {
                crate::web::start_tilt(ui.ctx().clone());
            }
        }
        Some(tilt) => {
            let tilt = (tilt * 2.0).round() / 2.0;
            let button = egui::Button::new(format!("Use {tilt:+.1}° (measured now)"))
                .fill(theme::BLAZE_DEEP)
                .min_size(vec2(ui.available_width(), 56.0));
            if ui.add(button).clicked() {
                *degrees = tilt.clamp(-80.0, 80.0);
            }
        }
    }
}

/// A clock face around the shooter for the wind's direction: tap or drag
/// to the hour the wind comes from (half hours too).
fn wind_clock(ui: &mut Ui, clock: &mut f64) {
    let size = ui.available_width().min(240.0);
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let center = rect.center();
    let radius = size / 2.0 - 4.0;
    if let Some(pointer) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let offset = pointer - center;
        if offset.length() > 20.0 {
            let angle = offset.x.atan2(-offset.y);
            let hours = (f64::from(angle.to_degrees()) / 30.0).rem_euclid(12.0);
            let snapped = (hours * 2.0).round() / 2.0;
            *clock = if snapped < 0.25 { 12.0 } else { snapped };
        }
    }
    let painter = ui.painter_at(rect.expand(2.0));
    painter.circle(
        center,
        radius,
        theme::FOREST_900,
        Stroke::new(2.0_f32, theme::FOREST_600),
    );
    let at = |hours: f64, r: f32| {
        let angle = (hours * 30.0).to_radians() as f32;
        center + vec2(angle.sin(), -angle.cos()) * r
    };
    let number_radius = radius - 22.0;
    let wind = clock.rem_euclid(12.0);
    for hour in 1..=12 {
        let position = at(f64::from(hour), number_radius);
        let selected =
            (wind - f64::from(hour % 12)).abs() < 0.01 || (hour == 12 && wind.abs() < 0.01);
        let (fill, text) = if selected {
            (theme::BLAZE, theme::FOREST_900)
        } else if hour == 12 {
            (theme::FOREST_800, theme::MEADOW)
        } else {
            (theme::FOREST_800, theme::BONE)
        };
        painter.circle_filled(position, 17.0, fill);
        painter.text(
            position,
            Align2::CENTER_CENTER,
            hour.to_string(),
            FontId::proportional(15.0),
            text,
        );
    }
    // The shooter in the middle, facing 12.
    painter.circle_filled(center, 8.0, theme::BONE);
    painter.arrow(
        center - vec2(0.0, 10.0),
        vec2(0.0, -26.0),
        Stroke::new(2.5_f32, theme::BONE),
    );
    // The wind, blowing in from its hour.
    let from = at(*clock, number_radius - 24.0);
    let to = at(*clock, 18.0);
    painter.arrow(from, to - from, Stroke::new(4.0_f32, theme::BLAZE));
}

//! The calculation page: pick a rifle and a bullet from your collection,
//! enter the weather and the shot, read holds, limits and the dope table.

use ballistics_core::analysis::{self, OptimalZero, PointBlank};
use ballistics_core::atmosphere::STANDARD_SEA_LEVEL_PRESSURE_HPA;
use ballistics_core::{Coriolis, RangeRow, Scenario, Solution, SolveError, solver};
use ballistics_store::{
    BcModel, Bullet, BulletKind, Catalog, ClickUnit, LoadZero, PLAIN_CROSSHAIR, Reticle, Rifle,
    RifleLoad, Scope, Target, Truing, TruingMethod, VelocityPoint,
};

use eframe::egui::{self, RichText};
use egui_plot::{HLine, Legend, Line, Plot, PlotBounds, PlotPoints, VLine};
use serde::{Deserialize, Serialize};

use ballistics_shot::{from_radians, to_radians, unit_label};

use crate::app::{Action, Data, Page};
use crate::figure::Figure;
use crate::mobile;
use crate::sight_picture::{SightPicture, magnification_slider, magnification_text};
use crate::theme;
use crate::trajectory3d;
use crate::units::{self, Quantity, UnitPrefs, quantity};
use crate::widgets::{caliber_name, confirm_button, grid, number, section, target_picker};

mod phone;

const RAD_PER_MOA: f64 = std::f64::consts::PI / (180.0 * 60.0);
/// Shortest range the engine flies, so the ethical limit can be found even
/// when it lies past the table.
const SOLVE_RANGE_M: f64 = 1200.0;
/// The target until the hunter picks another, and when theirs is deleted.
const DEFAULT_TARGET: &str = "moose";

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PressureSource {
    Station,
    SeaLevelAndAltitude,
}

/// Everything entered on this page. Saved on the device between sessions.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Inputs {
    pub rifle_id: Option<String>,
    pub bullet_id: Option<String>,
    pub temperature_c: f64,
    pub pressure_source: PressureSource,
    pub station_pressure_hpa: f64,
    pub sea_level_pressure_hpa: f64,
    pub altitude_m: f64,
    pub humidity_percent: f64,
    pub powder_follows_air: bool,
    pub powder_temperature_c: f64,
    pub wind_speed: f64,
    pub wind_clock: f64,
    pub inclination_degrees: f64,
    pub coriolis: bool,
    pub latitude_degrees: f64,
    pub azimuth_degrees: f64,
    pub spin_drift: bool,
    pub target_range_m: f64,
    /// The target: one of the hunter's own or the catalog's; `None` for
    /// no target (just the reticle and the shot).
    pub target_id: Option<String>,
    /// The targets picked lately, newest first, for quick choices.
    pub recent_targets: Vec<String>,
    /// Ranges picked lately (metres), newest first.
    pub recent_ranges: Vec<f64>,
    /// The hunter's own vital (or hit) zone instead of the target's.
    pub vital_override_cm: Option<f64>,
    /// The hunter's own minimum energy instead of the target's.
    pub min_energy_override_j: Option<f64>,
    /// The hunter's own minimum velocity for a target that sets one.
    pub min_velocity_override_mps: Option<f64>,
    pub target_speed: f64,
    pub min_velocity: f64,
    /// Scope magnification for the sight picture; `None` is the scope's highest.
    pub magnification: Option<f64>,
    /// Show the sight picture with the elevation dialled instead of held.
    pub dial_elevation: bool,
    /// Show the sight picture zoomed in around the aiming point.
    /// Show the reticle's illuminated parts lit.
    pub illumination: bool,
    /// Show the trajectory in 3D instead of as a graph.
    pub trajectory_3d: bool,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            rifle_id: None,
            bullet_id: None,
            temperature_c: 5.0,
            pressure_source: PressureSource::SeaLevelAndAltitude,
            station_pressure_hpa: 1000.0,
            sea_level_pressure_hpa: STANDARD_SEA_LEVEL_PRESSURE_HPA,
            altitude_m: 100.0,
            humidity_percent: 70.0,
            powder_follows_air: true,
            powder_temperature_c: 5.0,
            wind_speed: 3.0,
            wind_clock: 3.0,
            inclination_degrees: 0.0,
            coriolis: false,
            latitude_degrees: 60.0,
            azimuth_degrees: 0.0,
            spin_drift: true,
            target_range_m: 150.0,
            target_id: Some(DEFAULT_TARGET.to_owned()),
            recent_targets: Vec::new(),
            recent_ranges: Vec::new(),
            vital_override_cm: None,
            min_energy_override_j: None,
            min_velocity_override_mps: None,
            target_speed: 0.0,
            min_velocity: 550.0,
            magnification: None,
            dial_elevation: false,
            illumination: false,
            trajectory_3d: false,
        }
    }
}

impl Inputs {
    /// The weather and the shot, for the engine.
    pub fn conditions(&self) -> ballistics_shot::Conditions {
        ballistics_shot::Conditions {
            temperature_c: self.temperature_c,
            pressure: match self.pressure_source {
                PressureSource::Station => {
                    ballistics_shot::Pressure::Station(self.station_pressure_hpa)
                }
                PressureSource::SeaLevelAndAltitude => ballistics_shot::Pressure::SeaLevel {
                    hpa: self.sea_level_pressure_hpa,
                    altitude_m: self.altitude_m,
                },
            },
            humidity_percent: self.humidity_percent,
            powder_temperature_c: (!self.powder_follows_air).then_some(self.powder_temperature_c),
            wind_speed_mps: self.wind_speed,
            wind_clock: self.wind_clock,
            inclination_degrees: self.inclination_degrees,
            coriolis: self.coriolis.then_some(Coriolis {
                latitude_degrees: self.latitude_degrees,
                azimuth_degrees: self.azimuth_degrees,
            }),
            spin_drift: self.spin_drift,
        }
    }
}

/// The target and what it asks of the shot, with the hunter's changes.
/// Without a target there is no zone and no ethical limit.
pub struct Aim {
    pub target: Option<Target>,
    /// The zone a shot must land in, across.
    pub vital_diameter_cm: Option<f64>,
    /// `None` for sport targets and no target: no ethical limits.
    pub min_energy_j: Option<f64>,
    /// The game's own minimum velocity, when it sets one.
    pub min_velocity_mps: Option<f64>,
    pub figure: Option<Figure>,
}

impl Aim {
    fn new(inputs: &Inputs, data: &Data) -> Aim {
        let Some(id) = &inputs.target_id else {
            return Aim {
                target: None,
                vital_diameter_cm: None,
                min_energy_j: None,
                min_velocity_mps: None,
                figure: None,
            };
        };
        let target = data
            .target(id)
            .or_else(|| Catalog::builtin().target(DEFAULT_TARGET).cloned())
            .expect("the default target is in the catalog");
        let vital_diameter_cm = inputs.vital_override_cm.unwrap_or(target.vital_diameter_cm);
        let min_energy_j = target.is_game().then(|| {
            inputs
                .min_energy_override_j
                .or(target.min_energy_j)
                .unwrap_or(0.0)
        });
        let min_velocity_mps = target
            .min_velocity_mps
            .map(|velocity| inputs.min_velocity_override_mps.unwrap_or(velocity));
        let figure = Figure::new(&target, vital_diameter_cm);
        Aim {
            target: Some(target),
            vital_diameter_cm: Some(vital_diameter_cm),
            min_energy_j,
            min_velocity_mps,
            figure: Some(figure),
        }
    }

    pub fn is_game(&self) -> bool {
        self.target.as_ref().is_some_and(Target::is_game)
    }

    /// The target's name, or "No target".
    pub fn name(&self) -> &str {
        self.target
            .as_ref()
            .map_or("No target", |target| target.name.as_str())
    }

    /// "vital zone" on game, "hit zone" on sport targets.
    pub fn zone_name(&self) -> &'static str {
        if self.target.as_ref().is_some_and(|target| !target.is_game()) {
            "hit zone"
        } else {
            "vital zone"
        }
    }
}

#[derive(Clone, PartialEq)]
struct CacheKey {
    scenario: Scenario,
    vital_diameter: Option<f64>,
    max_range: f64,
}

struct Computed {
    solution: Result<Solution, SolveError>,
    optimal: Option<OptimalZero>,
}

pub struct CalculatorPage {
    pub inputs: Inputs,
    /// The rifle-load being edited: the saved one, or a new empty one.
    load_draft: Option<RifleLoad>,
    cache: Option<(CacheKey, Computed)>,
    /// On a phone: the results or the inputs page.
    phone_view: phone::PhoneView,
    /// On a phone: the picker sheet that is open.
    picker: Option<phone::Picker>,
    /// The target picker's search text.
    target_search: String,
    /// The weather where the hunter is, when asked for.
    pub weather: crate::weather::Weather,
    /// The truing shot being entered.
    truing_form: TruingForm,
}

/// A long shot to true the calculation to, as entered.
struct TruingForm {
    range_m: f64,
    /// In the rifle's click unit, up positive.
    elevation: f64,
    method: Option<ballistics_store::TruingMethod>,
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

impl CalculatorPage {
    pub fn new(inputs: Inputs) -> Self {
        Self {
            inputs,
            load_draft: None,
            cache: None,
            phone_view: phone::PhoneView::default(),
            picker: None,
            target_search: String::new(),
            weather: crate::weather::Weather::default(),
            truing_form: TruingForm::default(),
        }
    }

    /// What a printed card is for: the chosen rifle, bullet (with its saved
    /// data for the rifle) and target, in today's weather.
    pub fn card_subject(&self, data: &Data) -> Option<crate::cards::Subject> {
        let rifle = self
            .inputs
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
            .or(data.rifles.first())?;
        let fits = |bullet: &&Bullet| bullet.caliber == rifle.caliber;
        let bullet = self
            .inputs
            .bullet_id
            .as_ref()
            .and_then(|id| {
                data.bullets
                    .iter()
                    .filter(fits)
                    .find(|bullet| &bullet.id == id)
            })
            .or_else(|| data.bullets.iter().find(fits))?;
        let aim = Aim::new(&self.inputs, data);
        Some(crate::cards::Subject {
            load: data
                .loads
                .iter()
                .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
                .cloned(),
            target: aim.target.as_ref().map(|target| target.name.clone()),
            vital_cm: aim.vital_diameter_cm,
            min_energy_j: aim.min_energy_j,
            min_velocity_mps: aim
                .min_velocity_mps
                .unwrap_or_else(|| ballistics_shot::min_velocity(bullet, self.inputs.min_velocity)),
            conditions: self.inputs.conditions(),
            rifle: rifle.clone(),
            bullet: bullet.clone(),
        })
    }

    /// Drops unsaved rifle-load edits, e.g. after the load was reset.
    pub fn forget_load_draft(&mut self) {
        self.load_draft = None;
    }

    pub fn show(&mut self, context: &egui::Context, data: &Data) -> Vec<Action> {
        if let Some(report) = self.weather.poll(context) {
            crate::weather::apply(&report, &mut self.inputs);
        }
        let mut actions = Vec::new();
        let rifle = self.selected_rifle(data).cloned();
        let bullets: Vec<&Bullet> = rifle
            .as_ref()
            .map(|rifle| {
                data.bullets
                    .iter()
                    .filter(|b| b.caliber == rifle.caliber)
                    .collect()
            })
            .unwrap_or_default();
        let bullet = self
            .inputs
            .bullet_id
            .as_ref()
            .and_then(|id| bullets.iter().find(|bullet| &bullet.id == id))
            .or(bullets.first())
            .map(|bullet| (*bullet).clone());
        self.inputs.bullet_id = bullet.as_ref().map(|bullet| bullet.id.clone());
        let load = match (&rifle, &bullet) {
            (Some(rifle), Some(bullet)) => data
                .loads
                .iter()
                .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
                .cloned(),
            _ => None,
        };
        // Keep the draft on the selected rifle and bullet.
        let draft_matches = self.load_draft.as_ref().is_some_and(|draft| {
            rifle
                .as_ref()
                .is_some_and(|rifle| rifle.id == draft.rifle_id)
                && bullet
                    .as_ref()
                    .is_some_and(|bullet| bullet.id == draft.bullet_id)
        });
        if !draft_matches {
            self.load_draft = match (&rifle, &bullet) {
                (Some(rifle), Some(bullet)) => Some(
                    load.clone()
                        .unwrap_or_else(|| RifleLoad::new(&rifle.id, &bullet.id)),
                ),
                _ => None,
            };
        }
        let optics = rifle.as_ref().and_then(|rifle| data.optics(rifle));
        let aim = Aim::new(&self.inputs, data);
        // Unsaved edits count as soon as they are valid.
        let effective_load = self
            .load_draft
            .clone()
            .filter(|draft| draft.validate().is_ok() && !draft.is_empty())
            .or(load.clone());
        self.remember_target();

        if mobile::is_narrow(context) {
            self.show_phone(
                context,
                data,
                phone::PhoneFrame {
                    rifle: rifle.as_ref(),
                    bullets: &bullets,
                    bullet: bullet.as_ref(),
                    load: load.as_ref(),
                    optics: optics.as_ref(),
                    aim: &aim,
                    effective_load: effective_load.as_ref(),
                    country: data.country,
                },
                &mut actions,
            );
            return actions;
        }
        self.picker = None;

        egui::SidePanel::left("calculator-inputs")
            .resizable(true)
            .default_width(360.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.equipment(
                        ui,
                        data,
                        rifle.as_ref(),
                        &bullets,
                        bullet.as_ref(),
                        load.as_ref(),
                        &mut actions,
                    );
                    if rifle.is_some() && bullet.is_some() {
                        self.conditions(
                            ui,
                            data,
                            &aim,
                            rifle.as_ref(),
                            bullet.as_ref(),
                            optics.as_ref(),
                            &mut actions,
                        );
                    }
                });
            });

        egui::CentralPanel::default().show(context, |ui| match (&rifle, &bullet) {
            (None, _) => {
                ui.heading("Add a rifle to start");
                ui.label("Rifles and the bullets you shoot in them are kept in your own database.");
                ui.horizontal(|ui| {
                    if ui.button("Go to Rifles").clicked() {
                        actions.push(Action::Goto(Page::Rifles));
                    }
                    if ui
                        .button("Load demo data")
                        .on_hover_text(
                            "Three rifles with bullets, a scope and a target, to try the app",
                        )
                        .clicked()
                    {
                        actions.push(Action::LoadDemo);
                    }
                });
            }
            (Some(rifle), None) => {
                ui.heading(format!(
                    "No {} bullets in your collection",
                    caliber_name(&rifle.caliber)
                ));
                ui.label(
                    "Add factory ammunition or a handload from the catalog, or enter one by hand.",
                );
                if ui.button("Go to My bullets").clicked() {
                    actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
                }
            }
            (Some(rifle), Some(bullet)) => {
                let scenario = self.scenario(rifle, bullet, effective_load.as_ref());
                self.refresh(
                    scenario,
                    aim.vital_diameter_cm,
                    rifle.table.end_m.max(SOLVE_RANGE_M),
                );
                let computed = &self.cache.as_ref().expect("refresh fills the cache").1;
                match &computed.solution {
                    Ok(solution) => {
                        let view = ResultsView {
                            inputs: &self.inputs,
                            rifle,
                            optics: optics.as_ref(),
                            aim: &aim,
                            min_velocity: aim.min_velocity_mps.unwrap_or_else(|| {
                                ballistics_shot::min_velocity(bullet, self.inputs.min_velocity)
                            }),
                            assessment: data
                                .country
                                .map(|country| ballistics_shot::assess(bullet, solution, country)),
                        };
                        let requested = egui::ScrollArea::vertical()
                            .show(ui, |ui| view.show(ui, solution, computed.optimal.as_ref()))
                            .inner;
                        if let Some(magnification) = requested.magnification {
                            self.inputs.magnification = Some(magnification);
                        }
                        if let Some(three_d) = requested.trajectory_3d {
                            self.inputs.trajectory_3d = three_d;
                        }
                    }
                    Err(error) => {
                        ui.heading("No solution");
                        ui.colored_label(ui.visuals().error_fg_color, error.to_string());
                    }
                }
            }
        });
        actions
    }

    /// Keeps the target at the front of the recent ones.
    fn remember_target(&mut self) {
        let Some(id) = &self.inputs.target_id else {
            return;
        };
        let recent = &mut self.inputs.recent_targets;
        if recent.first() != Some(id) {
            recent.retain(|other| other != id);
            recent.insert(0, id.clone());
            recent.truncate(3);
        }
    }

    fn selected_rifle<'a>(&mut self, data: &'a Data) -> Option<&'a Rifle> {
        let rifle = self
            .inputs
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
            .or(data.rifles.first());
        self.inputs.rifle_id = rifle.map(|rifle| rifle.id.clone());
        rifle
    }

    fn refresh(&mut self, scenario: Scenario, vital_diameter_cm: Option<f64>, max_range: f64) {
        let key = CacheKey {
            scenario,
            vital_diameter: vital_diameter_cm.map(|cm| cm / 100.0),
            max_range,
        };
        if self.cache.as_ref().is_none_or(|(cached, _)| *cached != key) {
            let computed = Computed {
                solution: solver::solve(&key.scenario, key.max_range),
                optimal: key
                    .vital_diameter
                    .and_then(|diameter| analysis::optimal_point_blank(&key.scenario, diameter)),
            };
            self.cache = Some((key, computed));
        }
    }

    fn scenario(&self, rifle: &Rifle, bullet: &Bullet, load: Option<&RifleLoad>) -> Scenario {
        ballistics_shot::scenario(rifle, bullet, load, &self.inputs.conditions())
    }

    fn powder_temperature_c(&self) -> f64 {
        if self.inputs.powder_follows_air {
            self.inputs.temperature_c
        } else {
            self.inputs.powder_temperature_c
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn equipment(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        rifle: Option<&Rifle>,
        bullets: &[&Bullet],
        bullet: Option<&Bullet>,
        load: Option<&RifleLoad>,
        actions: &mut Vec<Action>,
    ) {
        ui.heading("Calculate");
        ui.add_space(4.0);
        section(ui, "Rifle and bullet", |ui| {
            grid(ui, "equipment", |ui| {
                ui.label("Rifle");
                egui::ComboBox::from_id_salt("rifle")
                    .width(230.0)
                    .selected_text(rifle.map_or("None".to_owned(), |rifle| rifle.name.clone()))
                    .show_ui(ui, |ui| {
                        for candidate in &data.rifles {
                            let text = format!(
                                "{} ({})",
                                candidate.name,
                                caliber_name(&candidate.caliber)
                            );
                            let selected = rifle.is_some_and(|rifle| rifle.id == candidate.id);
                            if ui.selectable_label(selected, text).clicked() {
                                self.inputs.rifle_id = Some(candidate.id.clone());
                                self.inputs.bullet_id = None;
                                self.load_draft = None;
                            }
                        }
                    });
                ui.end_row();
                ui.label("Bullet");
                egui::ComboBox::from_id_salt("bullet")
                    .width(230.0)
                    .selected_text(bullet.map_or("None".to_owned(), Bullet::display_name))
                    .show_ui(ui, |ui| {
                        for candidate in bullets {
                            let selected = bullet.is_some_and(|bullet| bullet.id == candidate.id);
                            if ui
                                .selectable_label(selected, candidate.display_name())
                                .clicked()
                            {
                                self.inputs.bullet_id = Some(candidate.id.clone());
                                self.load_draft = None;
                            }
                        }
                    });
                ui.end_row();
            });
            let (Some(rifle), Some(bullet)) = (rifle, bullet) else {
                return;
            };
            let u = units::prefs(ui.ctx());
            ui.weak(format!(
                "{} · zero {} · sight {} · 1:{} {} twist · clicks {} {}",
                caliber_name(&rifle.caliber),
                u.format(Quantity::Distance, rifle.zero_range_m),
                u.format(Quantity::SightHeight, rifle.sight_height_cm),
                u.format(Quantity::Twist, rifle.twist_in),
                if rifle.right_hand_twist {
                    "right"
                } else {
                    "left"
                },
                rifle.click_value,
                unit_label(rifle.click_unit),
            ));
            let drag = match bullet.drag_model {
                BcModel::Custom => format!("own drag curve ({} points)", bullet.drag_curve.len()),
                model => format!(
                    "BC {model:?} {:.3}",
                    bullet.ballistic_coefficient().unwrap_or_default()
                ),
            };
            ui.weak(format!(
                "{drag} · {}",
                bullet.length_mm.map_or("length unknown".to_owned(), |mm| u
                    .format(Quantity::BulletLength, mm)
                    + " long"),
            ));
            ui.add_space(4.0);
            self.load_panel(ui, rifle, bullet, load, actions);
        });
    }

    /// Velocity, impact offset and zero weather of this bullet in this
    /// rifle. Edits apply to the calculation at once and are kept once saved.
    fn load_panel(
        &mut self,
        ui: &mut egui::Ui,
        rifle: &Rifle,
        bullet: &Bullet,
        saved: Option<&RifleLoad>,
        actions: &mut Vec<Action>,
    ) {
        let powder = self.powder_temperature_c();
        let today = self.inputs.temperature_c;
        let conditions = self.inputs.conditions();
        let form = &mut self.truing_form;
        let Some(draft) = self.load_draft.as_mut() else {
            return;
        };
        let model = draft.velocity_model(bullet);
        let source = match (draft.velocities.len(), bullet.kind) {
            (0, BulletKind::Factory) => "from the box".to_owned(),
            (0, BulletKind::Handload) => "from your load data".to_owned(),
            (1, _) => "from your measurement".to_owned(),
            (count, _) => format!("fitted to {count} measurements"),
        };
        let u = units::prefs(ui.ctx());
        let velocity_factor = match draft.truing {
            Some(Truing {
                method: TruingMethod::Velocity,
                factor,
                ..
            }) => factor,
            _ => 1.0,
        };
        ui.label(format!(
            "Muzzle velocity {} with the powder at {}, {source}{}",
            u.format(Quantity::Speed, model.velocity_at(powder) * velocity_factor),
            u.format(Quantity::Temperature, powder),
            if velocity_factor != 1.0 {
                ", trued"
            } else {
                ""
            },
        ));

        egui::CollapsingHeader::new("This bullet in this rifle").id_salt("rifle-load").show(ui, |ui| {
            ui.strong("Measured velocities");
            ui.weak("Chronograph averages. Two or more at different temperatures give this rifle's own temperature sensitivity.");
            let mut remove = None;
            egui::Grid::new("velocities").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                for (index, point) in draft.velocities.iter_mut().enumerate() {
                    units::drag(ui, &mut point.temperature_c, Quantity::Temperature, -60.0..=70.0, 0.5);
                    units::drag(ui, &mut point.velocity_mps, Quantity::Speed, 50.0..=1500.0, 1.0);
                    if ui.small_button("✖").on_hover_text("Remove").clicked() {
                        remove = Some(index);
                    }
                    ui.end_row();
                }
            });
            if let Some(index) = remove {
                draft.velocities.remove(index);
            }
            if draft.velocities.len() < 10 && ui.button("➕ Add measurement").clicked() {
                let velocity_mps = model.velocity_at(today).round();
                draft.velocities.push(VelocityPoint { temperature_c: today.round(), velocity_mps });
            }
            ui.weak(format!(
                "Temperature sensitivity {} ({:+.1} % per 15 °C), {}.",
                u.format_signed(Quantity::Sensitivity, model.per_degree),
                model.percent_per_15c(),
                if draft.velocities.len() >= 2 { "from your measurements" } else { "from the bullet data" }
            ));

            ui.separator();
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
                        draft.impact_offset_mrad[index] = (to_radians(unit, value) * 1000.0).clamp(-49.0, 49.0);
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

            ui.separator();
            ui.strong("Zeroed in");
            let mut mode = match draft.zero {
                LoadZero::Rifle => 0,
                LoadZero::SameAsShot => 1,
                LoadZero::Recorded(_) => 2,
            };
            ui.radio_value(&mut mode, 0, "The rifle's zero weather");
            ui.radio_value(&mut mode, 1, "The same weather as today");
            ui.radio_value(&mut mode, 2, "This weather:");
            draft.zero = match (mode, draft.zero) {
                (0, _) => LoadZero::Rifle,
                (1, _) => LoadZero::SameAsShot,
                (_, LoadZero::Recorded(weather)) => LoadZero::Recorded(weather),
                _ => LoadZero::Recorded(rifle.zero_weather()),
            };
            if let LoadZero::Recorded(weather) = &mut draft.zero {
                grid(ui, "load-zero", |ui| {
                    quantity(ui, "Air", &mut weather.temperature_c, Quantity::Temperature, -45.0..=50.0, 0.5);
                    quantity(ui, "Powder", &mut weather.powder_temperature_c, Quantity::Temperature, -45.0..=60.0, 0.5);
                    quantity(ui, "Station pressure", &mut weather.pressure_hpa, Quantity::Pressure, 500.0..=1100.0, 1.0);
                    number(ui, "Humidity", &mut weather.humidity_percent, 0.0..=100.0, 1.0, " %");
                });
            }

            ui.separator();
            ui.strong("Truing");
            ui.weak(
                "Shot far and found the calculation off? Enter what it took to hit centre, in today's \
                 weather above, and the calculation is adjusted to match. The longest range you've \
                 shot works best (300 m or more).",
            );
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
                        bullet.ballistic_coefficient().map_or(String::new(), |bc| format!(
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
                        "That's a large correction. Check the range, the zero and the clicks \
                         before trusting it.",
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
                quantity(ui, "Range", &mut form.range_m, Quantity::Distance, 50.0..=2000.0, 5.0);
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
                        "({} clicks)",
                        (form.elevation / rifle.click_value).round()
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
                    &conditions,
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

            ui.separator();
            let problem = draft.validate().err();
            let changed = match saved {
                Some(saved) => !same_content(saved, draft),
                None => !draft.is_empty(),
            };
            ui.horizontal(|ui| {
                if ui.add_enabled(changed && problem.is_none(), egui::Button::new("💾 Save for this rifle")).clicked() {
                    actions.push(Action::SaveLoad(draft.clone()));
                }
                if changed && ui.button("Undo changes").clicked() {
                    *draft = saved.cloned().unwrap_or_else(|| RifleLoad::new(&rifle.id, &bullet.id));
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
                ui.weak("Used in the calculation now; save to keep it.");
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn conditions(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        aim: &Aim,
        rifle: Option<&Rifle>,
        bullet: Option<&Bullet>,
        optics: Option<&(Scope, Reticle)>,
        actions: &mut Vec<Action>,
    ) {
        let inputs = &mut self.inputs;
        let weather = &mut self.weather;
        section(ui, "Weather", |ui| {
            weather.panel(ui, inputs, data.country);
            ui.add_space(4.0);
            grid(ui, "weather", |ui| {
                quantity(
                    ui,
                    "Temperature",
                    &mut inputs.temperature_c,
                    Quantity::Temperature,
                    -45.0..=50.0,
                    0.5,
                );
                ui.label("Pressure from");
                ui.vertical(|ui| {
                    ui.radio_value(
                        &mut inputs.pressure_source,
                        PressureSource::SeaLevelAndAltitude,
                        "Weather report + altitude",
                    );
                    ui.radio_value(
                        &mut inputs.pressure_source,
                        PressureSource::Station,
                        "Barometer (station)",
                    );
                });
                ui.end_row();
                match inputs.pressure_source {
                    PressureSource::Station => {
                        quantity(
                            ui,
                            "Station pressure",
                            &mut inputs.station_pressure_hpa,
                            Quantity::Pressure,
                            500.0..=1100.0,
                            1.0,
                        );
                    }
                    PressureSource::SeaLevelAndAltitude => {
                        quantity(
                            ui,
                            "Sea-level pressure",
                            &mut inputs.sea_level_pressure_hpa,
                            Quantity::Pressure,
                            900.0..=1100.0,
                            1.0,
                        );
                        quantity(
                            ui,
                            "Altitude",
                            &mut inputs.altitude_m,
                            Quantity::Altitude,
                            -400.0..=5000.0,
                            10.0,
                        );
                    }
                }
                number(
                    ui,
                    "Humidity",
                    &mut inputs.humidity_percent,
                    0.0..=100.0,
                    1.0,
                    " %",
                );
                ui.label("Powder temperature");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut inputs.powder_follows_air, "same as air");
                    if !inputs.powder_follows_air {
                        units::drag(
                            ui,
                            &mut inputs.powder_temperature_c,
                            Quantity::Temperature,
                            -45.0..=60.0,
                            0.5,
                        );
                    }
                });
                ui.end_row();
                quantity(
                    ui,
                    "Wind speed",
                    &mut inputs.wind_speed,
                    Quantity::WindSpeed,
                    0.0..=30.0,
                    0.1,
                );
                ui.label("Wind from");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut inputs.wind_clock)
                            .range(0.0..=12.0)
                            .speed(0.05)
                            .fixed_decimals(1)
                            .suffix(" o'clock"),
                    );
                    ui.weak(wind_description(inputs.wind_clock));
                });
                ui.end_row();
            });
        });

        section(ui, "Shot", |ui| {
            grid(ui, "shot", |ui| {
                quantity(
                    ui,
                    "Range (rangefinder)",
                    &mut inputs.target_range_m,
                    Quantity::Distance,
                    5.0..=1000.0,
                    1.0,
                );
                number(
                    ui,
                    "Shot angle",
                    &mut inputs.inclination_degrees,
                    -60.0..=60.0,
                    0.5,
                    "°",
                );
                ui.label("");
                ui.weak("positive uphill, negative downhill");
                ui.end_row();
                quantity(
                    ui,
                    "Animal crossing at",
                    &mut inputs.target_speed,
                    Quantity::WindSpeed,
                    0.0..=15.0,
                    0.1,
                );
                ui.label("");
                ui.weak("walking moose ≈ 1.5, trotting ≈ 4, running ≈ 8");
                ui.end_row();
                ui.label("Spin drift");
                ui.checkbox(&mut inputs.spin_drift, "include");
                ui.end_row();
                ui.label("Coriolis");
                ui.checkbox(&mut inputs.coriolis, "include");
                ui.end_row();
                if inputs.coriolis {
                    number(
                        ui,
                        "  latitude",
                        &mut inputs.latitude_degrees,
                        -90.0..=90.0,
                        0.1,
                        "°",
                    );
                    number(
                        ui,
                        "  shooting towards",
                        &mut inputs.azimuth_degrees,
                        0.0..=360.0,
                        1.0,
                        "° (0 = north)",
                    );
                }
            });
        });

        if let Some((scope, _)) = optics {
            section(ui, "Scope", |ui| {
                ui.checkbox(
                    &mut inputs.dial_elevation,
                    "Dial the elevation (otherwise hold over)",
                );
                if scope.illuminated {
                    ui.checkbox(&mut inputs.illumination, "Illumination on");
                }
            });
        }

        section(ui, "Target and ethics", |ui| {
            if target_picker(ui, "calculator-target", data, &mut inputs.target_id) {
                // A new target brings its own zone and energy.
                inputs.vital_override_cm = None;
                inputs.min_energy_override_j = None;
                inputs.min_velocity_override_mps = None;
            }
            let Some(target) = &aim.target else {
                ui.small("No target: just the reticle and where the bullet lands, without a vital zone or energy limits.");
                if ui
                    .button("Targets…")
                    .on_hover_text("See the targets or make your own")
                    .clicked()
                {
                    actions.push(Action::Goto(Page::Targets));
                }
                return;
            };
            grid(ui, "ethics", |ui| {
                let zone_label = if target.is_game() {
                    "Vital zone diameter"
                } else {
                    "Hit zone diameter"
                };
                overridable(
                    ui,
                    zone_label,
                    &mut inputs.vital_override_cm,
                    target.vital_diameter_cm,
                    Quantity::ShortLength,
                    1.0..=200.0,
                    0.5,
                );
                if target.is_game() {
                    overridable(
                        ui,
                        "Minimum energy",
                        &mut inputs.min_energy_override_j,
                        target.min_energy_j.unwrap_or(0.0),
                        Quantity::Energy,
                        0.0..=8000.0,
                        10.0,
                    );
                    match (
                        target.min_velocity_mps,
                        bullet.and_then(|bullet| bullet.min_expansion_mps),
                    ) {
                        (Some(velocity), _) => overridable(
                            ui,
                            "Minimum velocity",
                            &mut inputs.min_velocity_override_mps,
                            velocity,
                            Quantity::Speed,
                            0.0..=900.0,
                            5.0,
                        ),
                        (None, Some(expansion)) => {
                            ui.label("Minimum velocity");
                            ui.label(format!(
                                "{} (bullet data)",
                                units::prefs(ui.ctx()).format(Quantity::Speed, expansion)
                            ));
                            ui.end_row();
                        }
                        (None, None) => quantity(
                            ui,
                            "Minimum velocity",
                            &mut inputs.min_velocity,
                            Quantity::Speed,
                            0.0..=900.0,
                            5.0,
                        ),
                    }
                }
            });
            if !target.notes.is_empty() {
                ui.small(&target.notes);
            }
            if target.is_game() {
                ui.small(
                    "Check your bullet maker's expansion data and your country's legal energy rules.",
                );
            } else {
                ui.small("A sport target: no energy or velocity limits apply.");
            }
            if ui
                .button("Targets…")
                .on_hover_text("See the targets or make your own")
                .clicked()
            {
                actions.push(Action::Goto(Page::Targets));
            }
        });

        if let Some(rifle) = rifle {
            section(ui, "Dope table", |ui| {
                let table = rifle.table;
                let u = units::prefs(ui.ctx());
                ui.label(format!(
                    "{}–{} every {}, set for this rifle.",
                    u.format(Quantity::Distance, table.start_m),
                    u.format(Quantity::Distance, table.end_m),
                    u.format(Quantity::Distance, table.step_m)
                ));
                if ui.button("Edit table").clicked() {
                    actions.push(Action::EditRifle(rifle.id.clone()));
                }
            });
        }
    }
}

/// A value the target sets that the hunter may change. Changing it keeps
/// the hunter's number; the reset button goes back to the target's.
fn overridable(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    target_value: f64,
    quantity: Quantity,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut shown = value.unwrap_or(target_value);
        if units::drag(ui, &mut shown, quantity, range, speed).changed() {
            *value = Some(shown);
        }
        if value.is_some() {
            let text = units::prefs(ui.ctx()).format(quantity, target_value);
            if ui
                .small_button("↺")
                .on_hover_text(format!("Back to the target's {text}"))
                .clicked()
            {
                *value = None;
            }
        } else {
            ui.weak("target's");
        }
    });
    ui.end_row();
}

/// Whether two rifle loads hold the same data, ignoring when they were saved.
fn same_content(a: &RifleLoad, b: &RifleLoad) -> bool {
    a.velocities == b.velocities
        && a.impact_offset_mrad == b.impact_offset_mrad
        && a.zero == b.zero
        && a.notes == b.notes
        && a.truing == b.truing
}

/// View settings changed in the results, applied by the page.
#[derive(Default)]
struct ViewRequests {
    magnification: Option<f64>,
    trajectory_3d: Option<bool>,
}

struct ResultsView<'a> {
    inputs: &'a Inputs,
    rifle: &'a Rifle,
    optics: Option<&'a (Scope, Reticle)>,
    aim: &'a Aim,
    min_velocity: f64,
    /// How the load fares under the hunter's country's rules, if one is set.
    assessment: Option<ballistics_shot::Assessment>,
}

impl ResultsView<'_> {
    /// The ethical range (none for sport targets and no target) and the
    /// point-blank range with the current zero (none without a zone).
    fn limits_of(&self, solution: &Solution) -> (Option<f64>, Option<PointBlank>) {
        let aim = self.aim;
        let ethical = aim
            .min_energy_j
            .and_then(|energy| analysis::ethical_limit(solution, energy, self.min_velocity));
        let point_blank = aim
            .vital_diameter_cm
            .and_then(|cm| analysis::point_blank_with_zero(solution, cm / 100.0));
        (ethical, point_blank)
    }

    /// "Moose at 150 m", or "Shot at 150 m" without a target.
    fn title(&self, ui: &egui::Ui) -> String {
        let range = units::prefs(ui.ctx()).format(Quantity::Distance, self.inputs.target_range_m);
        match &self.aim.target {
            Some(target) => format!("{} at {range}", target.name),
            None => format!("Shot at {range}"),
        }
    }

    /// Returns the view settings the hunter changed.
    fn show(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        optimal: Option<&OptimalZero>,
    ) -> ViewRequests {
        let inputs = self.inputs;
        let aim = self.aim;
        let vital_radius = aim.vital_diameter_cm.map(|cm| cm / 200.0);
        let (ethical, point_blank) = self.limits_of(solution);

        const PICTURE: f32 = 300.0;
        let mut requested = ViewRequests::default();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - PICTURE - 40.0).max(420.0));
                ui.heading(self.title(ui));
                match solution.at(inputs.target_range_m) {
                    Some(row) => self.shot_card(ui, &row, ethical),
                    None => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            "The bullet does not reach this range.",
                        );
                    }
                }
                ui.add_space(6.0);
                self.warnings(ui, solution);
                ui.separator();
                self.limits(ui, solution, optimal, ethical, point_blank);
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(PICTURE);
                requested.magnification = self.sight_picture(ui, solution, PICTURE);
            });
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Trajectory");
            ui.add_space(12.0);
            let three_d = self.inputs.trajectory_3d;
            if ui.selectable_label(!three_d, "2D").clicked() {
                requested.trajectory_3d = Some(false);
            }
            if ui.selectable_label(three_d, "3D").clicked() {
                requested.trajectory_3d = Some(true);
            }
        });
        if self.inputs.trajectory_3d {
            let scene = trajectory3d::Scene {
                samples: &solution.samples,
                target_range: inputs.target_range_m,
                inclination_degrees: inputs.inclination_degrees,
                figure: aim.figure.as_ref(),
                wind_speed: inputs.wind_speed,
                wind_from_degrees: inputs.wind_clock * 30.0,
            };
            trajectory3d::show(ui, &scene, 420.0);
        } else {
            self.plot(ui, solution, vital_radius, ethical);
        }

        ui.separator();
        ui.heading("Dope table");
        self.table(ui, solution, ethical);
        requested
    }

    fn limits(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        optimal: Option<&OptimalZero>,
        ethical: Option<f64>,
        point_blank: Option<PointBlank>,
    ) {
        let u = units::prefs(ui.ctx());
        ui.heading(if self.aim.is_game() {
            "Hunting limits"
        } else {
            "Limits"
        });
        egui::Grid::new("limits")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                if let Some(min_energy) = self.aim.min_energy_j {
                    ui.label("Ethical maximum range");
                    ui.label(match ethical {
                        Some(range) if self.min_velocity > 0.0 => format!(
                            "{} (≥ {} and ≥ {})",
                            u.format(Quantity::Distance, range),
                            u.format(Quantity::Energy, min_energy),
                            u.format(Quantity::Speed, self.min_velocity)
                        ),
                        Some(range) => format!(
                            "{} (≥ {})",
                            u.format(Quantity::Distance, range),
                            u.format(Quantity::Energy, min_energy)
                        ),
                        None => format!(
                            "beyond {}",
                            u.format(
                                Quantity::Distance,
                                solution.samples.last().map_or(0.0, |sample| sample.range)
                            )
                        ),
                    });
                    ui.end_row();
                }
                if self.aim.vital_diameter_cm.is_some() {
                    ui.label(format!(
                        "Dead-on hold, {} zero",
                        u.format(Quantity::Distance, self.rifle.zero_range_m)
                    ));
                    ui.label(point_blank_text(&u, point_blank, self.aim));
                    ui.end_row();
                    ui.label("Best point-blank zero");
                    ui.label(match optimal {
                        Some(best) => format!(
                            "zero at {}: {}; highest at {}",
                            u.format(Quantity::Distance, best.zero_range),
                            point_blank_text(&u, Some(best.point_blank), self.aim),
                            u.format(Quantity::Distance, best.peak_range)
                        ),
                        None => "not found".to_owned(),
                    });
                    ui.end_row();
                }
                ui.label("Muzzle velocity today");
                ui.label(u.format(Quantity::Speed, solution.muzzle_velocity));
                ui.end_row();
                ui.label("Stability (Miller Sg)");
                ui.label(
                    solution
                        .stability
                        .map_or("unknown: enter the bullet length".to_owned(), |sg| {
                            format!("{sg:.2}")
                        }),
                );
                ui.end_row();
            });
    }

    /// Returns the magnification the hunter moved to, if any.
    fn sight_picture(&self, ui: &mut egui::Ui, solution: &Solution, size: f32) -> Option<f64> {
        ui.heading("Through the scope");
        let Some((scope, reticle)) = self.optics else {
            ui.weak("Choose a scope and reticle for this rifle (Rifles page) to see the sight picture here.");
            return None;
        };
        let row = solution.at(self.inputs.target_range_m)?;
        let magnification =
            scope.clamp_magnification(self.inputs.magnification.unwrap_or(scope.magnification_max));
        let click_mrad = match self.rifle.click_unit {
            ClickUnit::Mil => self.rifle.click_value,
            ClickUnit::Moa => self.rifle.click_value * RAD_PER_MOA * 1000.0,
        };
        let (impact_mrad, dialled_clicks) =
            sight_impact(&row, click_mrad, self.inputs.dial_elevation);
        let picture = SightPicture {
            scope,
            reticle,
            magnification,
            impact_mrad,
            figure: self.aim.figure.as_ref(),
            range_m: row.range,
            illumination: self.inputs.illumination,
        };
        let drawn = picture.show(ui, size);
        let mut chosen = magnification;
        let requested = magnification_slider(ui, &mut chosen, scope, size)
            .changed()
            .then_some(chosen);
        ui.label(RichText::new(format!(
            "{} · {:.1} mrad across ({:.1} m at 100 m)",
            magnification_text(magnification),
            drawn.field_of_view_mrad,
            drawn.field_of_view_mrad / 10.0
        )));

        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} · {}", scope.display_name(), reticle.name)).small());
        if self.inputs.dial_elevation {
            ui.small(format!(
                "Dial {} click(s) {}, then hold the orange dot on the target.",
                dialled_clicks.abs(),
                if dialled_clicks >= 0.0 { "up" } else { "down" }
            ));
        } else {
            ui.small("Hold the orange dot on the target: the crosshair then sits where the picture shows it.");
        }
        let u = units::prefs(ui.ctx());
        if let (Some(target), Some(zone)) = (&self.aim.target, self.aim.vital_diameter_cm) {
            ui.small(format!(
                "{} at {}, true size; green circle: the {} {}.",
                target.name,
                u.format(Quantity::Distance, row.range),
                u.format(Quantity::ShortLength, zone),
                self.aim.zone_name()
            ));
        }
        if !drawn.impact_in_view {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The impact is outside the view: zoom out or dial.",
            );
        }
        let scale = scope.reticle_scale(magnification);
        // A plain crosshair has no marks whose size could be off.
        if reticle.id != PLAIN_CROSSHAIR && (scale - 1.0).abs() > 0.01 {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "Second focal plane: the reticle's marks are true only at {}×. At {magnification:.1}× each mark spans {scale:.2}× its stated value; the picture accounts for this.",
                    scope.reticle_reference_magnification.unwrap_or(scope.magnification_max),
                ),
            );
        }
        if !drawn.field_of_view_known {
            ui.small(format!(
                "Field of view not published; estimated as {:.0} mrad from the magnification. Enter the scope's field of view (Rifles → My scopes) for a true picture.",
                drawn.field_of_view_mrad
            ));
        }
        requested
    }

    fn unit(&self) -> ClickUnit {
        self.rifle.click_unit
    }

    fn clicks(&self, angle: f64) -> i64 {
        (angle / self.rifle.click_value).round() as i64
    }

    fn shot_card(&self, ui: &mut egui::Ui, row: &RangeRow, ethical: Option<f64>) {
        let unit = self.unit();
        let elevation = from_radians(unit, row.elevation_correction);
        let windage = from_radians(unit, row.windage_correction);
        let big = |text: String| RichText::new(text).size(22.0).strong();
        ui.horizontal(|ui| {
            ui.label(big(format!(
                "{} {:.2} {}",
                if elevation >= 0.0 {
                    "⬆ up"
                } else {
                    "⬇ down"
                },
                elevation.abs(),
                unit_label(unit)
            )));
            ui.weak(format!("{} clicks", self.clicks(elevation).abs()));
            ui.add_space(24.0);
            ui.label(big(format!(
                "{} {:.2} {}",
                if windage >= 0.0 {
                    "➡ right"
                } else {
                    "⬅ left"
                },
                windage.abs(),
                unit_label(unit)
            )));
            ui.weak(format!("{} clicks", self.clicks(windage).abs()));
        });
        let u = units::prefs(ui.ctx());
        ui.label(format!(
            "Dead-on hold hits {} {}, {} {}.  {}, {}, {:.3} s flight",
            u.format(Quantity::ShortLength, row.drop.abs() * 100.0),
            if row.drop >= 0.0 { "high" } else { "low" },
            u.format(Quantity::ShortLength, row.windage.abs() * 100.0),
            if row.windage >= 0.0 { "right" } else { "left" },
            u.format(Quantity::Speed, row.velocity),
            u.format(Quantity::Energy, row.energy_j),
            row.time
        ));
        if self.inputs.target_speed > 0.0 {
            let lead = analysis::moving_target_lead(row, self.inputs.target_speed);
            ui.label(format!(
                "Lead for a crossing animal: {} ({:.2} {}) ahead of the vital zone",
                u.format(Quantity::ShortLength, lead.distance * 100.0),
                from_radians(unit, lead.angle),
                unit_label(unit)
            ));
        }
        if ethical.is_some_and(|limit| row.range > limit) {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "⚠ Beyond your ethical range: the bullet is too slow or has too little energy here.",
            );
        }
    }

    fn warnings(&self, ui: &mut egui::Ui, solution: &Solution) {
        if let Some(assessment) = &self.assessment
            && let Some(needed) = self
                .aim
                .target
                .as_ref()
                .and_then(|target| target.ammunition_level(assessment.country))
        {
            let name = self.aim.name();
            let (allowed, text) = assessment.verdict(needed, name);
            let text = match allowed {
                ballistics_shot::Allowed::Yes => format!("✔ {text}"),
                _ => format!("⚠ {text}"),
            };
            let colour = match allowed {
                ballistics_shot::Allowed::Yes => ui.visuals().text_color(),
                ballistics_shot::Allowed::IfExpanding => ui.visuals().warn_fg_color,
                ballistics_shot::Allowed::No => ui.visuals().error_fg_color,
            };
            ui.horizontal_top(|ui| {
                crate::flags::show(ui, assessment.country, 11.0);
                ui.add(egui::Label::new(RichText::new(text).color(colour)).wrap())
                    .on_hover_text(assessment.country.rules().help());
            });
        }
        match solution.stability {
            Some(sg) if sg < 1.0 => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!(
                        "⚠ Sg {sg:.2}: this twist will not stabilise the bullet, so it may tumble."
                    ),
                );
            }
            Some(sg) if sg < 1.4 => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("Sg {sg:.2} is marginal; BC and accuracy may suffer, especially in cold air."),
                );
            }
            _ => {}
        }
        if let Some(row) = solution.at(self.inputs.target_range_m)
            && row.mach < 1.2
        {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("Mach {:.2} at the target: near the sound barrier drag changes fast, so predictions get less certain.", row.mach),
            );
        }
    }

    fn plot(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        vital_radius: Option<f64>,
        ethical: Option<f64>,
    ) {
        let u = units::prefs(ui.ctx());
        let x = |metres: f64| u.show(Quantity::Distance, metres);
        let y = |metres: f64| u.show(Quantity::ShortLength, metres * 100.0);
        let max_range = self.rifle.table.end_m.max(self.inputs.target_range_m);
        let path: Vec<[f64; 2]> = solution
            .samples
            .iter()
            .filter(|sample| sample.range <= max_range)
            .map(|sample| [x(sample.range), y(sample.drop)])
            .collect();
        // Without a zone the view is framed as for a 20 cm one.
        let zone = vital_radius.map(y);
        let zone_cm = zone.unwrap_or_else(|| y(0.1));
        let world = plot_world(&path, x(max_range), zone_cm);
        let target = x(self.inputs.target_range_m);
        let target_drop_cm = solution
            .at(self.inputs.target_range_m)
            .map(|row| y(row.drop));

        let mut command = None;
        ui.horizontal(|ui| {
            if ui
                .button("Fit")
                .on_hover_text("Show the whole trajectory (or double-click the graph)")
                .clicked()
            {
                command = Some(PlotCommand::Fit);
            }
            if ui.button("Zoom in").clicked() {
                command = Some(PlotCommand::ZoomIn);
            }
            if ui.button("Zoom out").clicked() {
                command = Some(PlotCommand::ZoomOut);
            }
            let to_target = ui.add_enabled(
                target_drop_cm.is_some(),
                egui::Button::new("Zoom to target"),
            );
            if to_target
                .on_hover_text("Close-up of the bullet path around the target range")
                .clicked()
            {
                command = Some(PlotCommand::Target);
            }
            ui.weak("Drag to pan · ⌘/Ctrl + scroll or pinch to zoom · double-click to fit");
        });

        Plot::new("hunting-trajectory")
            .height(280.0)
            .legend(Legend::default())
            .x_axis_label(format!("Range ({})", u.label(Quantity::Distance)))
            .y_axis_label(format!(
                "Height above line of sight ({})",
                u.label(Quantity::ShortLength)
            ))
            // The page scrolls with the wheel; zooming needs ⌘/Ctrl or a pinch.
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .include_x(world.min()[0])
            .include_x(world.max()[0])
            .include_y(world.min()[1])
            .include_y(world.max()[1])
            .show(ui, |plot_ui| {
                match command {
                    Some(PlotCommand::Fit) => plot_ui.set_auto_bounds(true),
                    Some(PlotCommand::ZoomIn) => {
                        let center = plot_ui.plot_bounds().center();
                        plot_ui.zoom_bounds(egui::vec2(ZOOM_STEP, ZOOM_STEP), center);
                    }
                    Some(PlotCommand::ZoomOut) => {
                        let center = plot_ui.plot_bounds().center();
                        plot_ui.zoom_bounds(egui::vec2(1.0 / ZOOM_STEP, 1.0 / ZOOM_STEP), center);
                    }
                    Some(PlotCommand::Target) => {
                        if let Some(drop_cm) = target_drop_cm {
                            plot_ui.set_plot_bounds(target_view(target, drop_cm, zone_cm));
                        }
                    }
                    None => {}
                }
                // Keep any panned or zoomed view on the trajectory.
                if !plot_ui.auto_bounds().any() {
                    let view = plot_ui.plot_bounds();
                    let clamped = clamp_view(view, world);
                    if clamped != view {
                        plot_ui.set_plot_bounds(clamped);
                    }
                }

                if let Some(zone) = zone {
                    let zone_name = capitalised(self.aim.zone_name());
                    plot_ui.hline(HLine::new(zone).color(theme::MEADOW).name(&zone_name));
                    plot_ui.hline(HLine::new(-zone).color(theme::MEADOW).name(&zone_name));
                }
                plot_ui.hline(
                    HLine::new(0.0)
                        .color(theme::BONE_MUTED)
                        .name("Line of sight"),
                );
                plot_ui.line(
                    Line::new(PlotPoints::from(path))
                        .width(2.0_f32)
                        .color(theme::BLAZE)
                        .name("Bullet"),
                );
                plot_ui.vline(VLine::new(target).color(theme::BONE).name("Target"));
                if let Some(limit) = ethical.filter(|limit| *limit <= max_range) {
                    plot_ui.vline(
                        VLine::new(x(limit))
                            .color(theme::ALARM)
                            .name("Ethical limit"),
                    );
                }
            });
    }

    fn table(&self, ui: &mut egui::Ui, solution: &Solution, ethical: Option<f64>) {
        let settings = self.rifle.table;
        let u = units::prefs(ui.ctx());
        let header = |name: &str, quantity: Quantity| format!("{name} {}", u.label(quantity));
        let (range_header, drop_header, wind_header, speed_header, energy_header, lead_header) = (
            header("Range", Quantity::Distance),
            header("Drop", Quantity::ShortLength),
            header("Wind", Quantity::ShortLength),
            u.label(Quantity::Speed).to_owned(),
            header("Energy", Quantity::Energy),
            header("Lead", Quantity::ShortLength),
        );
        let show = settings.columns;
        let target_speed = self.inputs.target_speed;
        // Each column: header and the cell text for a row (None at the muzzle
        // for corrections, which are undefined at zero range).
        type Cell<'a> = Box<dyn Fn(&RangeRow) -> String + 'a>;
        let angle = |unit: ClickUnit, radians: f64| format!("{:+.2}", from_radians(unit, radians));
        let candidates: Vec<(bool, &str, bool, Cell<'_>)> = vec![
            (
                true,
                &range_header,
                false,
                Box::new(|row| format!("{:.0}", u.show(Quantity::Distance, row.range))),
            ),
            (
                show.drop_length,
                &drop_header,
                false,
                Box::new(|row| u.number(Quantity::ShortLength, row.drop * 100.0, true)),
            ),
            (
                show.drop_mil,
                "Elev mil",
                true,
                Box::new(move |row| angle(ClickUnit::Mil, row.elevation_correction)),
            ),
            (
                show.drop_moa,
                "Elev MOA",
                true,
                Box::new(move |row| angle(ClickUnit::Moa, row.elevation_correction)),
            ),
            (
                show.drop_clicks,
                "Elev clicks",
                true,
                Box::new(|row| {
                    format!(
                        "{:+}",
                        self.clicks(from_radians(self.unit(), row.elevation_correction))
                    )
                }),
            ),
            (
                show.wind_length,
                &wind_header,
                false,
                Box::new(|row| u.number(Quantity::ShortLength, row.windage * 100.0, true)),
            ),
            (
                show.wind_mil,
                "Wind mil",
                true,
                Box::new(move |row| angle(ClickUnit::Mil, row.windage_correction)),
            ),
            (
                show.wind_moa,
                "Wind MOA",
                true,
                Box::new(move |row| angle(ClickUnit::Moa, row.windage_correction)),
            ),
            (
                show.wind_clicks,
                "Wind clicks",
                true,
                Box::new(|row| {
                    format!(
                        "{:+}",
                        self.clicks(from_radians(self.unit(), row.windage_correction))
                    )
                }),
            ),
            (
                show.velocity,
                &speed_header,
                false,
                Box::new(|row| u.number(Quantity::Speed, row.velocity, false)),
            ),
            (
                show.energy,
                &energy_header,
                false,
                Box::new(|row| u.number(Quantity::Energy, row.energy_j, false)),
            ),
            (
                show.time,
                "Time s",
                false,
                Box::new(|row| format!("{:.3}", row.time)),
            ),
            (
                target_speed > 0.0,
                &lead_header,
                false,
                Box::new(|row| {
                    u.number(
                        Quantity::ShortLength,
                        analysis::moving_target_lead(row, target_speed).distance * 100.0,
                        false,
                    )
                }),
            ),
        ];
        let columns: Vec<(&str, bool, Cell<'_>)> = candidates
            .into_iter()
            .filter(|column| column.0)
            .map(|(_, header, correction, cell)| (header, correction, cell))
            .collect();

        egui::Grid::new("dope")
            .striped(true)
            .spacing([14.0, 3.0])
            .show(ui, |ui| {
                for (header, _, _) in &columns {
                    ui.strong(*header);
                }
                ui.end_row();
                for range in settings.ranges() {
                    let Some(row) = solution.at(range) else { break };
                    let unethical = ethical.is_some_and(|limit| range > limit);
                    for (_, correction, cell) in &columns {
                        let text = if *correction && range <= 0.0 {
                            "–".to_owned()
                        } else {
                            cell(&row)
                        };
                        if unethical {
                            ui.colored_label(ui.visuals().error_fg_color, text);
                        } else {
                            ui.label(text);
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        ui.small("Elevation and wind are corrections: up/right positive. Red rows are past your ethical range. Confirm your dope on paper at real distances before hunting.");
    }
}

/// Where the bullet lands relative to the crosshair, in mrad (x right, y
/// up), and the elevation clicks dialled. Windage is always held: hunters
/// rarely dial wind.
fn sight_impact(row: &RangeRow, click_mrad: f64, dial_elevation: bool) -> ([f64; 2], f64) {
    let elevation_mrad = row.elevation_correction * 1000.0;
    let clicks = if dial_elevation {
        (elevation_mrad / click_mrad).round()
    } else {
        0.0
    };
    (
        [
            -row.windage_correction * 1000.0,
            -(elevation_mrad - clicks * click_mrad),
        ],
        clicks,
    )
}

#[derive(Clone, Copy)]
enum PlotCommand {
    Fit,
    ZoomIn,
    ZoomOut,
    Target,
}

const ZOOM_STEP: f32 = 1.5;
/// The closest the trajectory graph zooms: 10 m of range by 2 cm of height.
const MIN_VIEW: [f64; 2] = [10.0, 2.0];

/// The area the graph may show: the whole path out to `max_range`, the
/// vital zone and a little margin, and nothing beyond.
fn plot_world(path: &[[f64; 2]], max_range: f64, zone_cm: f64) -> PlotBounds {
    let heights = path.iter().map(|point| point[1]);
    let low = heights.clone().fold(-zone_cm * 3.0, f64::min);
    let high = heights.fold(zone_cm * 1.5, f64::max);
    let margin = (high - low) * 0.08;
    PlotBounds::from_min_max(
        [0.0, low - margin],
        [max_range.max(MIN_VIEW[0]), high + margin],
    )
}

/// A close-up around the target: ±25 % of the range (at least ±50 m), high
/// enough to show the bullet against the vital zone.
fn target_view(target: f64, drop_cm: f64, zone_cm: f64) -> PlotBounds {
    let half_width = (target * 0.25).max(50.0);
    let half_height = (zone_cm * 2.0).max(drop_cm.abs() * 0.4).max(15.0);
    PlotBounds::from_min_max(
        [target - half_width, drop_cm - half_height],
        [target + half_width, drop_cm + half_height],
    )
}

/// Moves and resizes `view` so it stays inside `world`, no larger than it
/// and no smaller than `MIN_VIEW`.
fn clamp_view(view: PlotBounds, world: PlotBounds) -> PlotBounds {
    let axis = |index: usize| {
        let (world_low, world_high) = (world.min()[index], world.max()[index]);
        let span = (view.max()[index] - view.min()[index])
            .max(MIN_VIEW[index])
            .min(world_high - world_low);
        let center = (view.min()[index] + view.max()[index]) / 2.0;
        let low = (center - span / 2.0).clamp(world_low, world_high - span);
        (low, low + span)
    };
    let (x, y) = (axis(0), axis(1));
    PlotBounds::from_min_max([x.0, y.0], [x.1, y.1])
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_default()
}

fn point_blank_text(u: &UnitPrefs, point_blank: Option<PointBlank>, aim: &Aim) -> String {
    let distance = |metres: f64| u.format(Quantity::Distance, metres);
    let zone = u.format(
        Quantity::ShortLength,
        aim.vital_diameter_cm.unwrap_or_default() / 2.0,
    );
    match point_blank {
        Some(PointBlank {
            near_limit,
            far_limit,
        }) if near_limit > 0.5 => {
            format!(
                "{}–{} inside ±{zone}",
                distance(near_limit),
                distance(far_limit)
            )
        }
        Some(PointBlank { far_limit, .. }) => {
            format!("out to {} inside ±{zone}", distance(far_limit))
        }
        None => format!("never inside the {}", aim.zone_name()),
    }
}

fn wind_description(clock: f64) -> &'static str {
    let hour = clock.rem_euclid(12.0);
    match hour {
        h if !(0.75..11.25).contains(&h) => "headwind",
        h if h < 2.25 => "from front right",
        h if h < 3.75 => "full value from right",
        h if h < 5.25 => "from rear right",
        h if h < 6.75 => "tailwind",
        h if h < 8.25 => "from rear left",
        h if h < 9.75 => "full value from left",
        _ => "from front left",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_core::DragModel;
    use ballistics_store::ZeroWeather;

    #[test]
    fn velocity_measured_in_the_rifle_overrides_the_box() {
        let rifle = Rifle::new("Tikka", "6-5x55-se", 8.0);
        let mut bullet = Bullet::new_manual("6-5x55-se", 6.71);
        bullet.mass_g = 9.1;
        bullet.bc_g7 = Some(0.25);
        bullet.drag_model = BcModel::G7;
        bullet.muzzle_velocity_mps = 800.0;
        let page = CalculatorPage::new(Inputs::default());

        let from_box = page.scenario(&rifle, &bullet, None);
        assert_eq!(from_box.load.muzzle_velocity, 800.0);
        assert_eq!(from_box.bullet.drag_model, DragModel::G7);
        assert_eq!(from_box.bullet.ballistic_coefficient, 0.25);
        assert_eq!(from_box.bullet.length_m, None);

        let mut measured = RifleLoad::new(&rifle.id, &bullet.id);
        measured.velocities.push(VelocityPoint {
            temperature_c: -5.0,
            velocity_mps: 781.0,
        });
        let scenario = page.scenario(&rifle, &bullet, Some(&measured));
        assert_eq!(scenario.load.muzzle_velocity, 781.0);
        assert_eq!(scenario.load.reference_temperature_c, -5.0);
        assert!((scenario.rifle.twist_m - 8.0 * 0.0254).abs() < 1e-12);
        // Without its own zero weather the load uses the rifle's.
        assert_eq!(
            scenario.zero_atmosphere.temperature_c,
            rifle.zero_temperature_c
        );
        assert_eq!(scenario.zero_powder_temperature_c, rifle.zero_temperature_c);
    }

    #[test]
    fn load_zero_weather_offset_and_bc_bands_reach_the_engine() {
        let rifle = Rifle::new("Tikka", "308-win", 10.0);
        let mut bullet = Bullet::new_manual("308-win", 7.82);
        bullet.mass_g = 11.7;
        bullet.bc_g1 = Some(0.45);
        bullet.muzzle_velocity_mps = 790.0;
        bullet.bc_bands = vec![
            ballistics_store::BcBand {
                velocity_mps: 800.0,
                bc: 0.45,
            },
            ballistics_store::BcBand {
                velocity_mps: 500.0,
                bc: 0.40,
            },
        ];
        let page = CalculatorPage::new(Inputs {
            temperature_c: -10.0,
            ..Inputs::default()
        });
        let mut load = RifleLoad::new(&rifle.id, &bullet.id);
        load.impact_offset_mrad = [0.2, -0.5];
        load.zero = LoadZero::SameAsShot;

        let scenario = page.scenario(&rifle, &bullet, Some(&load));
        assert_eq!(scenario.impact_offset, [0.0002, -0.0005]);
        assert_eq!(scenario.zero_atmosphere.temperature_c, -10.0);
        assert_eq!(scenario.zero_powder_temperature_c, -10.0);
        assert!((scenario.bullet.bc_at(650.0) - 0.425).abs() < 1e-12);

        let weather = ZeroWeather {
            temperature_c: 3.0,
            powder_temperature_c: 8.0,
            pressure_hpa: 980.0,
            humidity_percent: 40.0,
        };
        load.zero = LoadZero::Recorded(weather);
        let scenario = page.scenario(&rifle, &bullet, Some(&load));
        assert_eq!(scenario.zero_atmosphere.pressure_hpa, 980.0);
        assert_eq!(scenario.zero_powder_temperature_c, 8.0);

        bullet.drag_model = BcModel::Custom;
        bullet.drag_curve = vec![
            ballistics_store::DragPoint { mach: 0.5, cd: 0.2 },
            ballistics_store::DragPoint { mach: 2.0, cd: 0.3 },
        ];
        let scenario = page.scenario(&rifle, &bullet, Some(&load));
        let curve = scenario
            .bullet
            .drag_curve
            .expect("the curve reaches the engine");
        assert_eq!(curve.points(), [(0.5, 0.2), (2.0, 0.3)]);
    }

    #[test]
    fn panned_or_zoomed_views_stay_on_the_trajectory() {
        let world = PlotBounds::from_min_max([0.0, -120.0], [400.0, 20.0]);
        let clamp = |min: [f64; 2], max: [f64; 2]| {
            let view = clamp_view(PlotBounds::from_min_max(min, max), world);
            (view.min(), view.max())
        };
        // Panned far away: pulled back to the edge, size kept.
        assert_eq!(
            clamp([900.0, 500.0], [1000.0, 540.0]),
            ([300.0, -20.0], [400.0, 20.0])
        );
        // Zoomed out past everything: shows the whole world.
        assert_eq!(
            clamp([-5000.0, -9000.0], [9000.0, 9000.0]),
            ([0.0, -120.0], [400.0, 20.0])
        );
        // Zoomed in too far: grows to the minimum around the same centre.
        assert_eq!(
            clamp([199.9, -50.01], [200.1, -49.99]),
            ([195.0, -51.0], [205.0, -49.0])
        );
        // Already inside: untouched.
        assert_eq!(
            clamp([100.0, -60.0], [200.0, 0.0]),
            ([100.0, -60.0], [200.0, 0.0])
        );
    }

    #[test]
    fn world_covers_the_path_and_the_vital_zone() {
        let path = [[0.0, -4.5], [100.0, 0.0], [400.0, -110.0]];
        let world = plot_world(&path, 400.0, 10.0);
        assert_eq!(world.min()[0], 0.0);
        assert_eq!(world.max()[0], 400.0);
        assert!(world.min()[1] < -110.0 && world.max()[1] > 15.0);
    }

    #[test]
    fn held_shots_land_below_the_crosshair_and_dialled_ones_on_it() {
        let row = RangeRow {
            range: 300.0,
            time: 0.4,
            drop: -0.5,
            windage: -0.12,
            elevation_correction: 0.5 / 300.0,
            windage_correction: 0.12 / 300.0,
            velocity: 620.0,
            mach: 1.8,
            energy_j: 2200.0,
        };
        let (held, clicks) = sight_impact(&row, 0.1, false);
        assert_eq!(clicks, 0.0);
        assert!((held[1] + 1.6667).abs() < 1e-3, "{held:?}");
        assert!(
            (held[0] + 0.4).abs() < 1e-9,
            "wind pushes the impact left: {held:?}"
        );

        let (dialled, clicks) = sight_impact(&row, 0.1, true);
        assert_eq!(clicks, 17.0);
        assert!(
            dialled[1].abs() < 0.05,
            "only the click rounding remains: {dialled:?}"
        );
        assert_eq!(dialled[0], held[0]);
    }
}

//! The calculation page: pick a rifle and a bullet from your collection,
//! enter the weather and the shot, read holds, limits and the dope table.

use ballistics_core::analysis::{self, OptimalZero, PointBlank, ZeroFigure};
use ballistics_core::atmosphere::STANDARD_SEA_LEVEL_PRESSURE_HPA;
use ballistics_core::dispersion::{Sensitivities, Uncertainty};
use ballistics_core::{Coriolis, RangeRow, Scenario, Solution, SolveError, solver};
use ballistics_store::{
    BcModel, Bullet, Catalog, ClickUnit, LoadZero, PLAIN_CROSSHAIR, Reticle, Rifle, RifleLoad,
    Scope, TableSettings, Target,
};

use eframe::egui::{self, RichText};
use egui_plot::{HLine, Legend, Line, Plot, PlotBounds, PlotPoints, VLine};
use serde::{Deserialize, Serialize};

use ballistics_shot::{from_radians, unit_label};

use crate::app::{Action, Data, Page};
use crate::figure::Figure;
use crate::hits::{self, Hits};
use crate::mobile;
use crate::sight_picture::{SightPicture, magnification_slider, magnification_text};
use crate::theme;
use crate::trajectory3d;
use crate::units::{self, Quantity, UnitPrefs, quantity};
use crate::widgets::{caliber_name, fold, grid, number};

mod phone;

const RAD_PER_MOA: f64 = std::f64::consts::PI / (180.0 * 60.0);
/// Shortest range the engine flies, so the ethical limit can be found even
/// when it lies past the table.
const SOLVE_RANGE_M: f64 = 1200.0;
/// Muzzle velocity spread when the hunter has no chronograph string: a
/// typical factory load's, m/s.
const TYPICAL_VELOCITY_SD: f64 = 8.0;
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
    /// The jump a crosswind gives the bullet at the muzzle.
    pub aerodynamic_jump: bool,
    /// The rifle tilted, degrees: positive with the scope's top to the right.
    pub cant_degrees: f64,
    pub target_range_m: f64,
    /// Where the hunter is, from the weather lookup: for the daylight.
    pub place: Option<crate::weather::Place>,
    /// The hunter's county or region in their country's law, for seasons.
    pub region: Option<String>,
    /// The target: one of the hunter's own or the catalog's; `None` for
    /// no target (just the reticle and the shot).
    pub target_id: Option<String>,
    /// The targets picked lately, newest first, for quick choices.
    pub recent_targets: Vec<String>,
    /// Ranges picked lately (metres), newest first.
    pub recent_ranges: Vec<f64>,
    /// The hunter's own minimum energy instead of the target's.
    pub min_energy_override_j: Option<f64>,
    /// The hunter's 5-shot group at 100 m from the position they shoot
    /// from, edge to edge, cm: their own spread for the hit chance.
    pub group_cm: f64,
    /// Shot-to-shot muzzle velocity, one standard deviation; `None` takes
    /// the load's chronograph string, or a typical value.
    pub velocity_sd_mps: Option<f64>,
    /// How well the range, the wind's speed and its direction (clock hours)
    /// are known: about one standard deviation each.
    pub range_error_m: f64,
    pub wind_error_mps: f64,
    pub wind_direction_error_hours: f64,
    /// The hit chance a game shot should have, percent: sets how far the
    /// ethical range reaches.
    pub hit_needed_percent: f64,
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
            aerodynamic_jump: true,
            cant_degrees: 0.0,
            target_range_m: 150.0,
            place: None,
            region: None,
            target_id: Some(DEFAULT_TARGET.to_owned()),
            recent_targets: Vec::new(),
            recent_ranges: Vec::new(),
            min_energy_override_j: None,
            group_cm: 4.0,
            velocity_sd_mps: None,
            range_error_m: 2.0,
            wind_error_mps: 1.0,
            wind_direction_error_hours: 1.0,
            hit_needed_percent: 90.0,
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
            aerodynamic_jump: self.aerodynamic_jump,
            cant_degrees: self.cant_degrees,
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
    /// Hunting: a game target brings its ethical limits.
    pub hunting: bool,
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
                hunting: data.hunting,
            };
        };
        let target = data
            .target(id)
            .or_else(|| Catalog::builtin().target(DEFAULT_TARGET).cloned())
            .expect("the default target is in the catalog");
        let vital_diameter_cm = target.vital_diameter_cm;
        // Target shooting has no ethical limits, whatever the target.
        let min_energy_j = (data.hunting && target.is_game()).then(|| {
            inputs
                .min_energy_override_j
                .or(target.min_energy_j)
                .unwrap_or(0.0)
        });
        let min_velocity_mps = target
            .min_velocity_mps
            .filter(|_| data.hunting)
            .map(|velocity| inputs.min_velocity_override_mps.unwrap_or(velocity));
        let figure = Figure::new(&target, vital_diameter_cm);
        Aim {
            target: Some(target),
            vital_diameter_cm: Some(vital_diameter_cm),
            min_energy_j,
            min_velocity_mps,
            figure: Some(figure),
            hunting: data.hunting,
        }
    }

    /// A game target, when hunting: for target shooting every target is
    /// just a hit zone.
    pub fn is_game(&self) -> bool {
        self.hunting && self.target.as_ref().is_some_and(Target::is_game)
    }

    /// The target's name, or "No target".
    pub fn name(&self) -> &str {
        self.target
            .as_ref()
            .map_or("No target", |target| target.name.as_str())
    }

    /// "vital zone" on game, "hit zone" on sport targets.
    pub fn zone_name(&self) -> &'static str {
        if self.target.is_some() && !self.is_game() {
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
    reference: f64,
}

struct Computed {
    solution: Result<Solution, SolveError>,
    optimal: Option<OptimalZero>,
    zeros: Zeros,
    /// The shot flown again with its inputs changed a little: for the hit
    /// chance.
    sensitivities: Option<Sensitivities>,
}

/// A chance as a percentage, never quite certain: "94 %", "> 99 %".
pub fn percent(chance: f64) -> String {
    if chance > 0.995 {
        "> 99 %".to_owned()
    } else {
        format!("{:.0} %", chance * 100.0)
    }
}

/// The load's latest chronograph string's spread, and whether there was
/// one (else a typical value).
fn measured_velocity_sd(load: Option<&RifleLoad>) -> (f64, bool) {
    match load.and_then(|load| load.velocities.iter().rev().find_map(|point| point.shots)) {
        Some(shots) => (shots.sd_mps, true),
        None => (TYPICAL_VELOCITY_SD, false),
    }
}

/// What the hit chance was worked out for.
#[derive(Clone, PartialEq)]
struct HitsKey {
    generation: u64,
    uncertainty: Uncertainty,
    zone: Option<hits::Zone>,
    range: f64,
    needed: f64,
    max_range: f64,
    wind: f64,
}

/// The zeros told the way they are shot in: so high at a reference range.
#[derive(Clone, Copy, Default)]
pub struct Zeros {
    /// 100 m, or 100 yd in yards.
    reference: f64,
    /// The rifle's zero.
    figure: Option<ZeroFigure>,
    /// The best point-blank zero's height at the reference range.
    best_height: Option<f64>,
    /// How much higher this load hits than the rifle's zero load, radians.
    offset_up: f64,
}

pub struct CalculatorPage {
    pub inputs: Inputs,
    cache: Option<(CacheKey, Computed)>,
    /// On a phone: the results or the inputs page.
    phone_view: phone::PhoneView,
    /// The picker that is open: a sheet on a phone, a dialog on a desktop.
    picker: Option<phone::Picker>,
    /// The target picker's search text.
    target_search: String,
    /// The weather where the hunter is, when asked for.
    pub weather: crate::weather::Weather,
    /// The range table's settings being changed, for the rifle with this
    /// id: saved to it once the mouse or finger lets go.
    table_edit: Option<(String, TableSettings)>,
    /// The ethical limit and how far the dead-on hold reaches, from the
    /// last results: for the range picker.
    limits: (Option<f64>, Option<f64>),
    /// Counts the solves, so the hit chance knows when to work again.
    generation: u64,
    hits: Option<(HitsKey, Option<Hits>)>,
}

impl CalculatorPage {
    pub fn new(inputs: Inputs) -> Self {
        Self {
            inputs,
            cache: None,
            phone_view: phone::PhoneView::default(),
            picker: None,
            target_search: String::new(),
            weather: crate::weather::Weather::default(),
            table_edit: None,
            limits: (None, None),
            generation: 0,
            hits: None,
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
            elevation_travel_clicks: rifle
                .scope_id
                .as_deref()
                .and_then(|id| data.scope(id))
                .and_then(|scope| Some(scope.elevation_travel? / scope.click_value?)),
            zero_weather: match data
                .loads
                .iter()
                .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
                .map_or(LoadZero::Rifle, |load| load.zero)
            {
                // Zeroed in the shot's weather: the rifle's zero weather
                // is still the one the hunter zeroed in.
                LoadZero::Rifle | LoadZero::SameAsShot => Some(rifle.zero_weather()),
                LoadZero::Recorded(weather) => Some(weather),
            },
            rifle: rifle.clone(),
            bullet: bullet.clone(),
        })
    }

    /// The range table to show for the rifle: as being changed, or its own.
    fn table_for(&self, rifle: &Rifle) -> TableSettings {
        match &self.table_edit {
            Some((id, table)) if *id == rifle.id => *table,
            _ => rifle.table,
        }
    }

    /// Saves a changed range table to its rifle once nothing is held down.
    fn save_table(&mut self, context: &egui::Context, data: &Data, actions: &mut Vec<Action>) {
        let Some((id, table)) = &self.table_edit else {
            return;
        };
        if context.input(|input| input.pointer.any_down()) {
            return;
        }
        if let Some(rifle) = data.rifles.iter().find(|rifle| rifle.id == *id) {
            let mut changed = rifle.clone();
            changed.table = *table;
            if changed.validate().is_err() {
                return;
            }
            if changed.table != rifle.table {
                actions.push(Action::SaveTable(changed));
            }
        }
        self.table_edit = None;
    }

    /// Applies what the results asked for.
    fn apply_requests(
        &mut self,
        requested: ViewRequests,
        rifle: &Rifle,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let zeros = self
            .cache
            .as_ref()
            .map(|(_, computed)| computed.zeros)
            .unwrap_or_default();
        if let Some(rifle) = requested.use_zero(rifle, &zeros) {
            actions.push(Action::SaveRifle(rifle));
        }
        if let Some(magnification) = requested.magnification {
            self.inputs.magnification = Some(magnification);
        }
        if let Some(three_d) = requested.trajectory_3d {
            self.inputs.trajectory_3d = three_d;
        }
        if let Some(dial) = requested.dial_elevation {
            self.inputs.dial_elevation = dial;
        }
        if let Some(on) = requested.illumination {
            self.inputs.illumination = on;
        }
        if let Some(region) = requested.region {
            self.inputs.region = region;
        }
        if requested.locate {
            self.weather.locate_here(context);
        }
        if let Some(table) = requested.table {
            self.table_edit = Some((rifle.id.clone(), table));
        }
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
        let optics = rifle.as_ref().and_then(|rifle| data.optics(rifle));
        let aim = Aim::new(&self.inputs, data);
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
                    country: data.country,
                },
                &mut actions,
            );
            self.save_table(context, data, &mut actions);
            return actions;
        }

        egui::SidePanel::left("calculator-inputs")
            .resizable(true)
            .default_width(370.0)
            .max_width(560.0)
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
                        ui.add_space(6.0);
                        self.tiles(ui, &aim);
                        ui.add_space(4.0);
                        self.conditions(ui, data, &aim, bullet.as_ref(), &mut actions);
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
                if ui.button("Go to Bullets").clicked() {
                    actions.push(Action::ShowBulletsFor(rifle.caliber.clone()));
                }
            }
            (Some(rifle), Some(bullet)) => {
                let table = self.table_for(rifle);
                let scenario = self.scenario(rifle, bullet, load.as_ref());
                self.refresh(
                    context,
                    scenario,
                    aim.vital_diameter_cm,
                    table
                        .end_m
                        .max(SOLVE_RANGE_M)
                        .max(self.inputs.target_range_m + 10.0),
                );
                let hits = self.hits_for(&aim, load.as_ref());
                let computed = &self.cache.as_ref().expect("refresh fills the cache").1;
                match &computed.solution {
                    Ok(solution) => {
                        let view = ResultsView {
                            hits,
                            inputs: &self.inputs,
                            rifle,
                            table,
                            optics: optics.as_ref(),
                            aim: &aim,
                            min_velocity: aim.min_velocity_mps.unwrap_or_else(|| {
                                ballistics_shot::min_velocity(bullet, self.inputs.min_velocity)
                            }),
                            assessment: data
                                .country
                                .map(|country| ballistics_shot::assess(bullet, solution, country)),
                            zeros: computed.zeros,
                        };
                        let (ethical, point_blank) = view.limits_of(solution);
                        let limits = (ethical, point_blank.map(|blank| blank.far_limit));
                        let requested = egui::ScrollArea::vertical()
                            .show(ui, |ui| view.show(ui, solution, computed.optimal.as_ref()))
                            .inner;
                        self.limits = limits;
                        self.apply_requests(requested, rifle, context, &mut actions);
                    }
                    Err(error) => {
                        ui.heading("No solution");
                        ui.colored_label(ui.visuals().error_fg_color, error.to_string());
                    }
                }
            }
        });
        self.picker_sheet(context, data, rifle.as_ref());
        self.save_table(context, data, &mut actions);
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

    fn refresh(
        &mut self,
        context: &egui::Context,
        scenario: Scenario,
        vital_diameter_cm: Option<f64>,
        max_range: f64,
    ) {
        let reference = units::prefs(context).unit(Quantity::Distance).base(100.0);
        let key = CacheKey {
            scenario,
            vital_diameter: vital_diameter_cm.map(|cm| cm / 100.0),
            max_range,
            reference,
        };
        if self.cache.as_ref().is_none_or(|(cached, _)| *cached != key) {
            let optimal = key
                .vital_diameter
                .and_then(|diameter| analysis::optimal_point_blank(&key.scenario, diameter));
            let solution = solver::solve(&key.scenario, key.max_range);
            let sensitivities = solution.as_ref().ok().and_then(|solution| {
                Sensitivities::new(&key.scenario, solution.clone(), key.max_range).ok()
            });
            let computed = Computed {
                solution,
                sensitivities,
                zeros: Zeros {
                    reference,
                    figure: analysis::zero_figure(&key.scenario, reference, 1000.0).ok(),
                    best_height: optimal.and_then(|best| {
                        analysis::height_at(&key.scenario, best.elevation, reference)
                    }),
                    offset_up: key.scenario.impact_offset[1],
                },
                optimal,
            };
            self.cache = Some((key, computed));
            self.generation += 1;
        }
    }

    /// The muzzle velocity's spread: the hunter's figure, else the load's
    /// latest chronograph string, else a typical 8 m/s.
    pub fn velocity_sd(&self, load: Option<&RifleLoad>) -> (f64, bool) {
        match self.inputs.velocity_sd_mps {
            Some(sd) => (sd, false),
            None => measured_velocity_sd(load),
        }
    }

    /// The hit chance, its range and the wind bracket for this aim, worked
    /// out again only when something it depends on changes.
    fn hits_for(&mut self, aim: &Aim, load: Option<&RifleLoad>) -> Option<Hits> {
        let (key, computed) = self.cache.as_ref()?;
        let sensitivities = computed.sensitivities.as_ref()?;
        let inputs = &self.inputs;
        let uncertainty = hits::uncertainty(
            inputs.group_cm,
            self.velocity_sd(load).0,
            inputs.range_error_m,
            inputs.wind_error_mps,
            inputs.wind_direction_error_hours,
        );
        let hits_key = HitsKey {
            generation: self.generation,
            uncertainty,
            zone: hits::Zone::of(aim),
            range: inputs.target_range_m,
            needed: inputs.hit_needed_percent / 100.0,
            max_range: key.max_range,
            wind: inputs.wind_speed,
        };
        if self.hits.as_ref().is_none_or(|(cached, _)| *cached != hits_key) {
            let result = hits_key.zone.as_ref().and_then(|zone| {
                hits::compute(
                    sensitivities,
                    &uncertainty,
                    zone,
                    hits_key.range,
                    hits_key.needed,
                    hits_key.max_range - 10.0,
                    hits_key.wind,
                    inputs.wind_error_mps,
                )
            });
            self.hits = Some((hits_key, result));
        }
        self.hits.as_ref().and_then(|(_, hits)| *hits)
    }

    fn scenario(&self, rifle: &Rifle, bullet: &Bullet, load: Option<&RifleLoad>) -> Scenario {
        ballistics_shot::scenario(rifle, bullet, load, &self.inputs.conditions())
    }

    /// The rifle and bullet, and a line on the bullet in this rifle with a
    /// way to its setup on the Rifles page.
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
        let u = units::prefs(ui.ctx());
        ui.heading("Calculate");
        ui.add_space(4.0);
        grid(ui, "equipment", |ui| {
            ui.label("Rifle");
            let combo = egui::ComboBox::from_id_salt("rifle")
                .width(240.0)
                .selected_text(rifle.map_or("None".to_owned(), |rifle| shortened(&rifle.name, 40)))
                .show_ui(ui, |ui| {
                    for candidate in &data.rifles {
                        let text =
                            format!("{} ({})", candidate.name, caliber_name(&candidate.caliber));
                        let selected = rifle.is_some_and(|rifle| rifle.id == candidate.id);
                        if ui.selectable_label(selected, text).clicked() {
                            self.inputs.rifle_id = Some(candidate.id.clone());
                            self.inputs.bullet_id = None;
                        }
                    }
                });
            if let Some(rifle) = rifle {
                combo.response.on_hover_text(format!(
                    "{} · zero {} · sight {} · 1:{} {} twist · clicks {} {}",
                    caliber_name(&rifle.caliber),
                    zero_text(&u, rifle),
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
            }
            ui.end_row();
            ui.label("Bullet");
            let combo = egui::ComboBox::from_id_salt("bullet")
                .width(240.0)
                .selected_text(bullet.map_or("None".to_owned(), |bullet| {
                    shortened(&bullet.display_name(), 40)
                }))
                .show_ui(ui, |ui| {
                    for candidate in bullets {
                        let selected = bullet.is_some_and(|bullet| bullet.id == candidate.id);
                        if ui
                            .selectable_label(selected, candidate.display_name())
                            .clicked()
                        {
                            self.inputs.bullet_id = Some(candidate.id.clone());
                        }
                    }
                });
            if let Some(bullet) = bullet {
                let drag = match bullet.drag_model {
                    BcModel::Custom => {
                        format!("own drag curve ({} points)", bullet.drag_curve.len())
                    }
                    model => format!(
                        "BC {model:?} {:.3}",
                        bullet.ballistic_coefficient().unwrap_or_default()
                    ),
                };
                combo.response.on_hover_text(format!(
                    "{drag} · {}",
                    bullet.length_mm.map_or("length unknown".to_owned(), |mm| u
                        .format(Quantity::BulletLength, mm)
                        + " long"),
                ));
            }
            ui.end_row();
        });
        let (Some(rifle), Some(bullet)) = (rifle, bullet) else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.weak(crate::loads::summary(
                &u,
                bullet,
                load,
                &self.inputs.conditions(),
            ));
            if ui
                .link("Load setup…")
                .on_hover_text(
                    "Measured velocities, point of impact, zero weather and truing for this \
                     bullet in this rifle",
                )
                .clicked()
            {
                actions.push(Action::EditLoad(rifle.id.clone(), bullet.id.clone()));
            }
        });
    }

    /// Range, angle, wind and target: the inputs that change every shot,
    /// each opening its picker.
    pub(super) fn tiles(&mut self, ui: &mut egui::Ui, aim: &Aim) {
        let u = units::prefs(ui.ctx());
        let gap = 8.0;
        let width = (ui.available_width() - gap) / 2.0;
        // The glyphs: the range on a scale to 500 m, the wind arrow full at
        // 12 m/s.
        let fields = [
            (
                phone::Picker::Range,
                "Range".to_owned(),
                u.format(Quantity::Distance, self.inputs.target_range_m),
                phone::Glyph::Range((self.inputs.target_range_m / 500.0) as f32),
            ),
            (
                phone::Picker::Angle,
                if self.inputs.cant_degrees == 0.0 {
                    "Angle".to_owned()
                } else {
                    format!("Angle · cant {:+.0}°", self.inputs.cant_degrees)
                },
                format!("{:+.0}°", self.inputs.inclination_degrees),
                phone::Glyph::Angle(self.inputs.inclination_degrees as f32),
            ),
            (
                phone::Picker::Wind,
                format!("Wind · {}", phone::clock_text(self.inputs.wind_clock)),
                u.format(Quantity::WindSpeed, self.inputs.wind_speed),
                phone::Glyph::Wind {
                    clock: self.inputs.wind_clock as f32,
                    strength: (self.inputs.wind_speed / 12.0) as f32,
                },
            ),
            (
                phone::Picker::Target,
                match aim.vital_diameter_cm {
                    Some(zone) => format!("Target · {}", u.format(Quantity::ShortLength, zone)),
                    None => "Target".to_owned(),
                },
                aim.name().to_owned(),
                phone::Glyph::Figure(aim.figure.as_ref()),
            ),
        ];
        let [range, angle, wind, target] = fields;
        for pair in [[range, angle], [wind, target]] {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (picker, label, value, glyph) in pair {
                    if phone::quick_field(ui, width, &label, &value, glyph).clicked() {
                        self.picker = Some(picker);
                    }
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
    }

    /// Everything else about the shot, folded with a summary each: the
    /// weather, the fine corrections and the target's own numbers.
    fn conditions(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        aim: &Aim,
        bullet: Option<&Bullet>,
        actions: &mut Vec<Action>,
    ) {
        let u = units::prefs(ui.ctx());
        let inputs = &mut self.inputs;
        let weather = &mut self.weather;
        ui.horizontal(|ui| {
            ui.label(if data.hunting {
                "Animal crossing at"
            } else {
                "Target moving at"
            });
            units::drag(
                ui,
                &mut inputs.target_speed,
                Quantity::WindSpeed,
                0.0..=15.0,
                0.1,
            )
            .on_hover_text(if data.hunting {
                "Walking moose ≈ 1.5, trotting ≈ 4, running ≈ 8 m/s; 0 for a still animal"
            } else {
                "Sideways speed of a moving target; 0 for a still one"
            });
        });
        ui.add_space(4.0);

        let pressure = match inputs.pressure_source {
            PressureSource::Station => u.format(Quantity::Pressure, inputs.station_pressure_hpa),
            PressureSource::SeaLevelAndAltitude => format!(
                "{} · {}",
                u.format(Quantity::Pressure, inputs.sea_level_pressure_hpa),
                u.format(Quantity::Altitude, inputs.altitude_m)
            ),
        };
        let mut summary = format!(
            "{} · {pressure}",
            u.format(Quantity::Temperature, inputs.temperature_c)
        );
        if let Some(place) = &inputs.place {
            summary.push_str(" · ");
            summary.push_str(place.name.split(',').next().unwrap_or(&place.name));
        }
        fold(ui, "fold-weather", "Weather", &summary, |ui| {
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
            });
            let air = format!(
                "{} · {} % · powder {}",
                match inputs.pressure_source {
                    PressureSource::Station => "barometer",
                    PressureSource::SeaLevelAndAltitude => "forecast",
                },
                inputs.humidity_percent.round(),
                if inputs.powder_follows_air {
                    "as the air".to_owned()
                } else {
                    u.format(Quantity::Temperature, inputs.powder_temperature_c)
                }
            );
            fold(ui, "fold-air", "Air details", &air, |ui| {
                grid(ui, "air", |ui| {
                    ui.label("Air pressure");
                    ui.vertical(|ui| {
                        ui.radio_value(
                            &mut inputs.pressure_source,
                            PressureSource::SeaLevelAndAltitude,
                            "Sea-level, from the forecast",
                        );
                        ui.radio_value(
                            &mut inputs.pressure_source,
                            PressureSource::Station,
                            "Barometer at your spot",
                        );
                    });
                    ui.end_row();
                    match inputs.pressure_source {
                        PressureSource::Station => {
                            quantity(
                                ui,
                                "Barometer",
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
                });
            });
        });

        let mut on = Vec::new();
        if inputs.spin_drift {
            on.push("spin drift");
        }
        if inputs.aerodynamic_jump {
            on.push("aero jump");
        }
        if inputs.coriolis {
            on.push("Coriolis");
        }
        let mut summary = if on.is_empty() {
            "none".to_owned()
        } else {
            on.join(", ")
        };
        if inputs.cant_degrees != 0.0 {
            summary.push_str(&format!(" · cant {:+.1}°", inputs.cant_degrees));
        }
        fold(ui, "fold-fine", "Fine corrections", &summary, |ui| {
            grid(ui, "fine", |ui| {
                number(ui, "Cant", &mut inputs.cant_degrees, -30.0..=30.0, 0.5, "°");
                ui.label("");
                ui.weak("the rifle tilted: + with the scope's top to the right");
                ui.end_row();
                ui.label("Spin drift");
                ui.checkbox(&mut inputs.spin_drift, "include");
                ui.end_row();
                ui.label("Aerodynamic jump");
                ui.checkbox(&mut inputs.aerodynamic_jump, "include")
                    .on_hover_text(
                        "A crosswind throws a spinning bullet a little up or down as it leaves \
                         the muzzle (with a right-hand twist, wind from the left lifts it). \
                         Needs the bullet's length.",
                    );
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

        if aim.target.is_some() {
            let load = data.loads.iter().find(|load| {
                Some(&load.rifle_id) == inputs.rifle_id.as_ref()
                    && Some(&load.bullet_id) == inputs.bullet_id.as_ref()
            });
            let (measured_sd, from_chronograph) = measured_velocity_sd(load);
            let summary = format!(
                "{} group · ±{} · ±{}",
                u.format(Quantity::ShortLength, inputs.group_cm),
                u.format(Quantity::Distance, inputs.range_error_m),
                u.format(Quantity::WindSpeed, inputs.wind_error_mps),
            );
            fold(ui, "fold-precision", "Your precision", &summary, |ui| {
                grid(ui, "precision", |ui| {
                    quantity(
                        ui,
                        "Your group",
                        &mut inputs.group_cm,
                        Quantity::ShortLength,
                        0.5..=40.0,
                        0.1,
                    );
                    ui.label("");
                    ui.weak("5 shots at 100 m from the position you shoot from, edge to edge");
                    ui.end_row();
                    ui.label("Velocity spread");
                    ui.horizontal(|ui| match inputs.velocity_sd_mps.as_mut() {
                        Some(sd) => {
                            units::drag(ui, sd, Quantity::Speed, 0.0..=50.0, 0.1);
                            if ui
                                .small_button("↺")
                                .on_hover_text("Back to your chronograph's figure")
                                .clicked()
                            {
                                inputs.velocity_sd_mps = None;
                            }
                        }
                        None => {
                            ui.label(format!(
                                "{} ({})",
                                u.format(Quantity::Speed, measured_sd),
                                if from_chronograph {
                                    "your chronograph"
                                } else {
                                    "typical"
                                }
                            ));
                            if ui.small_button("Change").clicked() {
                                inputs.velocity_sd_mps = Some(measured_sd);
                            }
                        }
                    });
                    ui.end_row();
                    quantity(
                        ui,
                        "Range known to ±",
                        &mut inputs.range_error_m,
                        Quantity::Distance,
                        0.0..=50.0,
                        0.5,
                    );
                    quantity(
                        ui,
                        "Wind known to ±",
                        &mut inputs.wind_error_mps,
                        Quantity::WindSpeed,
                        0.0..=10.0,
                        0.1,
                    );
                    number(
                        ui,
                        "Wind direction to ±",
                        &mut inputs.wind_direction_error_hours,
                        0.0..=6.0,
                        0.25,
                        " h",
                    );
                    if aim.is_game() {
                        number(
                            ui,
                            "Hit chance needed",
                            &mut inputs.hit_needed_percent,
                            50.0..=99.0,
                            1.0,
                            " %",
                        );
                    }
                });
                ui.weak(
                    "Each ± is about one standard deviation: two times in three the truth is \
                     within it. The hit chance assumes you hold as the app tells you.",
                );
            });
        }

        let Some(target) = &aim.target else {
            return;
        };
        let mut summary = aim
            .vital_diameter_cm
            .map_or(String::new(), |cm| u.format(Quantity::ShortLength, cm));
        if let Some(energy) = aim.min_energy_j {
            summary.push_str(&format!(
                " · {} · {}",
                u.format(Quantity::Energy, energy),
                u.format(
                    Quantity::Speed,
                    aim.min_velocity_mps
                        .unwrap_or_else(|| bullet.map_or(inputs.min_velocity, |bullet| {
                            ballistics_shot::min_velocity(bullet, inputs.min_velocity)
                        }))
                )
            ));
        }
        let changed = inputs.min_energy_override_j.is_some()
            || inputs.min_velocity_override_mps.is_some();
        if changed {
            summary.push_str(" · changed");
        }
        let title = if aim.is_game() {
            "Target limits"
        } else {
            "Target"
        };
        fold(ui, "fold-target", title, &summary, |ui| {
            grid(ui, "ethics", |ui| {
                if aim.is_game() {
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
                                u.format(Quantity::Speed, expansion)
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
            if aim.is_game() {
                ui.weak(
                    "Check your bullet maker's expansion data and your country's legal energy \
                     rules.",
                );
            }
            if ui
                .button("Targets…")
                .on_hover_text("See the targets or make your own")
                .clicked()
            {
                actions.push(Action::Goto(Page::Targets));
            }
        });
    }
}

/// The text cut to `chars` characters with an ellipsis, for narrow places.
pub fn shortened(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(chars - 1).collect::<String>())
    }
}

/// The longest range to calculate a shot at, on the phone and desktop.
pub const MAX_RANGE_M: f64 = 2000.0;

/// "1 click", "3 clicks".
pub fn clicks_text(clicks: i64) -> String {
    if clicks.abs() == 1 {
        format!("{clicks} click")
    } else {
        format!("{clicks} clicks")
    }
}

/// "100 m" for a plain zero, "100 m, 4.0 cm high" for one above it.
pub fn zero_text(u: &units::UnitPrefs, rifle: &Rifle) -> String {
    let range = u.format(Quantity::Distance, rifle.zero_range_m);
    if rifle.zero_height_cm.abs() < 0.05 {
        range
    } else {
        format!(
            "{range}, {} {}",
            u.format(Quantity::ShortLength, rifle.zero_height_cm.abs()),
            if rifle.zero_height_cm > 0.0 {
                "high"
            } else {
                "low"
            }
        )
    }
}

/// "4.0 cm high at 100 m, on the line of sight again at 213 m".
pub fn zero_figure_text(u: &units::UnitPrefs, figure: &ZeroFigure) -> String {
    let at = if figure.height.abs() < 0.0005 {
        format!(
            "on the line of sight at {}",
            u.format(Quantity::Distance, figure.reference_range)
        )
    } else {
        format!(
            "{} {} at {}",
            u.format(Quantity::ShortLength, figure.height.abs() * 100.0),
            if figure.height > 0.0 { "high" } else { "low" },
            u.format(Quantity::Distance, figure.reference_range)
        )
    };
    match figure.far_zero {
        Some(far) if (far - figure.reference_range).abs() > 1.0 => format!(
            "{at}, on the line of sight again at {}",
            u.format(Quantity::Distance, far)
        ),
        _ => at,
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

/// View settings changed in the results, applied by the page.
#[derive(Default)]
struct ViewRequests {
    magnification: Option<f64>,
    trajectory_3d: Option<bool>,
    /// Make this height at the reference range the rifle's zero, metres.
    zero_height: Option<f64>,
    dial_elevation: Option<bool>,
    illumination: Option<bool>,
    /// The range table's ranges or columns changed.
    table: Option<TableSettings>,
    /// The hunter chose their county or region.
    region: Option<Option<String>>,
    /// Find the hunter's position, for the shooting light.
    locate: bool,
}

impl ViewRequests {
    /// The rifle zeroed as asked, if it was. `zeros` tell the reference
    /// range and how much higher this load hits than the rifle's zero load,
    /// so the rifle's own zero is set that much lower.
    fn use_zero(&self, rifle: &Rifle, zeros: &Zeros) -> Option<Rifle> {
        let height = self.zero_height? - zeros.offset_up * zeros.reference;
        let mut zeroed = rifle.clone();
        zeroed.zero_range_m = zeros.reference;
        // Rounded down to the millimetre, so the peak stays inside the zone.
        zeroed.zero_height_cm = (height * 1000.0).floor() / 10.0;
        Some(zeroed)
    }
}

/// The go/no-go facts of a hunt: legal class, season and light.
struct Statuses {
    country: ballistics_store::Country,
    pills: Vec<crate::hunting::Pill>,
    /// The hunter's region, found from their place if not chosen.
    region: Option<String>,
}

struct ResultsView<'a> {
    inputs: &'a Inputs,
    rifle: &'a Rifle,
    /// The range table's settings: the rifle's, or as being changed.
    table: TableSettings,
    optics: Option<&'a (Scope, Reticle)>,
    aim: &'a Aim,
    min_velocity: f64,
    /// How the load fares under the hunter's country's rules, if one is set.
    assessment: Option<ballistics_shot::Assessment>,
    zeros: Zeros,
    /// The hit chance, its range and the wind bracket; none without a zone.
    hits: Option<Hits>,
}

impl ResultsView<'_> {
    /// The ethical range (none for sport targets and no target) and the
    /// point-blank range with the current zero (none without a zone).
    fn limits_of(&self, solution: &Solution) -> (Option<f64>, Option<PointBlank>) {
        let aim = self.aim;
        // Game: as far as the energy and speed hold up and the hit chance
        // stays at what the hunter asks for.
        let ethical = aim.min_energy_j.and_then(|energy| {
            let by_energy = analysis::ethical_limit(solution, energy, self.min_velocity);
            match (by_energy, self.hits.and_then(|hits| hits.limit)) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        });
        let point_blank = aim
            .vital_diameter_cm
            .and_then(|cm| analysis::point_blank_with_zero(solution, cm / 100.0));
        (ethical, point_blank)
    }

    /// What the zone is called in the hit chance: "hit area" for a drawn
    /// one, else the vital or hit zone.
    fn zone_word(&self) -> &'static str {
        match hits::Zone::of(self.aim) {
            Some(hits::Zone::Shape(_)) => "hit area",
            _ => self.aim.zone_name(),
        }
    }

    /// The wind bracket: how far the bullet goes across for the lowest and
    /// highest likely wind, and whether the middle hold covers it.
    fn bracket_line(&self, ui: &mut egui::Ui) {
        let Some(bracket) = self.hits.and_then(|hits| hits.bracket) else {
            return;
        };
        let u = units::prefs(ui.ctx());
        let side = |drift: f64| if drift >= 0.0 { "right" } else { "left" };
        let text = format!(
            "Wind {}–{}: the bullet goes {} {} to {} {}",
            u.number(Quantity::WindSpeed, bracket.low_speed, false),
            u.format(Quantity::WindSpeed, bracket.high_speed),
            u.format(Quantity::ShortLength, bracket.low_drift.abs() * 100.0),
            side(bracket.low_drift),
            u.format(Quantity::ShortLength, bracket.high_drift.abs() * 100.0),
            side(bracket.high_drift),
        );
        if bracket.fits {
            ui.label(format!("{text}; holding for the middle keeps both in the {}.", self.zone_word()));
        } else {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{text}: more than the {} allows. Get closer or wait for steadier wind.", self.zone_word()),
            );
        }
    }

    /// "Moose at 150 m", or "Shot at 150 m" without a target.
    fn title(&self, ui: &egui::Ui) -> String {
        let range = units::prefs(ui.ctx()).format(Quantity::Distance, self.inputs.target_range_m);
        match &self.aim.target {
            Some(target) => format!("{} at {range}", target.name),
            None => format!("Shot at {range}"),
        }
    }

    /// The legal class, the season and the light, for a game target when
    /// the hunter's country is set.
    fn statuses(&self) -> Option<Statuses> {
        let target = self.aim.target.as_ref().filter(|_| self.aim.is_game())?;
        let assessment = self.assessment.as_ref()?;
        let country = assessment.country;
        let species = target.catalog_id.as_deref().unwrap_or(&target.id);
        let mut region = self.inputs.region.clone();
        crate::hunting::resolve_region(country, &mut region, self.inputs.place.as_ref());
        let mut pills = Vec::new();
        if let Some(needed) = target.ammunition_level(country) {
            let (allowed, text) = assessment.short_verdict(needed);
            let (_, full) = assessment.verdict(needed, &target.name);
            let tone = match allowed {
                ballistics_shot::Allowed::Yes => crate::hunting::Tone::Open,
                ballistics_shot::Allowed::IfExpanding => crate::hunting::Tone::Warn,
                ballistics_shot::Allowed::No => crate::hunting::Tone::Closed,
            };
            pills.push(crate::hunting::Pill {
                what: "class",
                text,
                tone,
                detail: format!("{full}\n\n{}", country.rules().help()),
                needs_position: false,
            });
        }
        for pill in crate::hunting::pills(
            country,
            species,
            &target.name,
            region.as_deref(),
            self.inputs.place.as_ref(),
        ) {
            pills.push(pill);
        }
        Some(Statuses {
            country,
            pills,
            region,
        })
    }

    /// The statuses as pills, with the region to choose and a way to find
    /// the hunter's position.
    fn go_no_go(&self, ui: &mut egui::Ui, statuses: &Statuses, requested: &mut ViewRequests) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            crate::flags::show(ui, statuses.country, 11.0);
            for pill in &statuses.pills {
                if pill.what == "season" {
                    // Without a region the drop-down asks for it instead.
                    if statuses.region.is_some() {
                        crate::hunting::pill(ui, &pill.text, pill.tone, &pill.detail);
                    }
                    let mut region = statuses.region.clone();
                    crate::hunting::region_menu(ui, statuses.country, &mut region);
                    if region != statuses.region {
                        requested.region = Some(region);
                    }
                } else if pill.needs_position && crate::sensors::can_locate() {
                    if ui
                        .button("Shooting light: use my position")
                        .on_hover_text(&pill.detail)
                        .clicked()
                    {
                        requested.locate = true;
                    }
                } else {
                    crate::hunting::pill(ui, &pill.text, pill.tone, &pill.detail);
                }
            }
        });
        if statuses.region != self.inputs.region && requested.region.is_none() {
            requested.region = Some(statuses.region.clone());
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
                ui.set_max_width((ui.available_width() - PICTURE - 64.0).max(420.0));
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
                ui.add_space(4.0);
                if let Some(statuses) = self.statuses() {
                    self.go_no_go(ui, &statuses, &mut requested);
                    ui.add_space(4.0);
                }
                self.warnings(ui, solution);
                ui.add_space(6.0);
                requested.zero_height = self.limits(ui, solution, optimal, ethical, point_blank);
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(PICTURE);
                self.sight_picture(ui, solution, PICTURE, &mut requested);
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
        self.table_heading(ui, &mut requested);
        self.table(ui, solution, ethical, None);
        requested
    }

    /// "Range table  100–400 m every 25 m  [Columns…]", the button opening
    /// the table's ranges and columns.
    fn table_heading(&self, ui: &mut egui::Ui, requested: &mut ViewRequests) {
        let u = units::prefs(ui.ctx());
        let table = self.table;
        ui.horizontal(|ui| {
            ui.heading("Range table");
            ui.weak(format!(
                "{}–{} every {}",
                u.format(Quantity::Distance, table.start_m),
                u.format(Quantity::Distance, table.end_m),
                u.format(Quantity::Distance, table.step_m)
            ));
            let button = ui
                .button("Columns…")
                .on_hover_text("The table's ranges and columns, kept for this rifle");
            let popup = ui.make_persistent_id("table-columns");
            if button.clicked() {
                ui.memory_mut(|memory| memory.toggle_popup(popup));
            }
            let mut changed = table;
            egui::popup_below_widget(
                ui,
                popup,
                &button,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(280.0);
                    table_settings(ui, &mut changed, self.rifle.click_unit);
                },
            );
            if changed != table {
                requested.table = Some(changed);
            }
        });
    }

    /// The limits that matter at a glance, and the rest folded.
    fn limits(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        optimal: Option<&OptimalZero>,
        ethical: Option<f64>,
        point_blank: Option<PointBlank>,
    ) -> Option<f64> {
        let u = units::prefs(ui.ctx());
        let zeros = self.zeros;
        let reference = zeros.reference;
        let mut use_zero = None;
        let narrow = mobile::is_narrow(ui.ctx());
        let fact = |ui: &mut egui::Ui, label: &str, value: String, note: String| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.weak(label);
                ui.label(RichText::new(value).size(17.0));
                if !note.is_empty() {
                    ui.weak(RichText::new(note).small());
                }
            });
        };
        let facts = |ui: &mut egui::Ui| {
            if let Some(min_energy) = self.aim.min_energy_j {
                let value = match ethical {
                    Some(range) => u.format(Quantity::Distance, range),
                    None => format!(
                        "beyond {}",
                        u.format(
                            Quantity::Distance,
                            solution.samples.last().map_or(0.0, |sample| sample.range)
                        )
                    ),
                };
                let mut note = if self.min_velocity > 0.0 {
                    format!(
                        "≥ {}, ≥ {}",
                        u.format(Quantity::Energy, min_energy),
                        u.format(Quantity::Speed, self.min_velocity)
                    )
                } else {
                    format!("≥ {}", u.format(Quantity::Energy, min_energy))
                };
                if self.hits.is_some() {
                    note.push_str(&format!(", ≥ {:.0} % hits", self.inputs.hit_needed_percent));
                }
                fact(ui, "Ethical max", value, note);
            }
            if let Some(hits) = self.hits {
                fact(
                    ui,
                    "Hit chance",
                    percent(hits.chance),
                    format!("in the {}, holding as told", self.zone_word()),
                );
            }
            if self.aim.vital_diameter_cm.is_some() {
                fact(
                    ui,
                    "Dead-on hold",
                    point_blank_short(&u, point_blank, self.aim),
                    "with your zero".to_owned(),
                );
            }
        };
        if narrow {
            facts(ui);
        } else {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 28.0;
                facts(ui);
            });
        }
        let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
            if narrow {
                ui.weak(label);
                add(ui);
                ui.add_space(2.0);
            } else {
                ui.label(label);
                add(ui);
                ui.end_row();
            }
        };
        let mut details = |ui: &mut egui::Ui| {
            if let Some(figure) = zeros.figure {
                row(ui, "Your zero", &mut |ui| {
                    ui.label(zero_figure_text(&u, &figure));
                });
            }
            if self.aim.vital_diameter_cm.is_some() {
                row(ui, "Best point-blank zero", &mut |ui| match optimal {
                    Some(best) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!(
                                "{}: {}; highest at {}",
                                match zeros.best_height {
                                    Some(height) => format!(
                                        "{} {} at {} (zero at {})",
                                        u.format(Quantity::ShortLength, height.abs() * 100.0),
                                        if height >= 0.0 { "high" } else { "low" },
                                        u.format(Quantity::Distance, reference),
                                        u.format(Quantity::Distance, best.zero_range),
                                    ),
                                    None => format!(
                                        "zero at {}",
                                        u.format(Quantity::Distance, best.zero_range)
                                    ),
                                },
                                point_blank_text(&u, Some(best.point_blank), self.aim),
                                u.format(Quantity::Distance, best.peak_range)
                            ));
                            if let Some(height) = zeros.best_height
                                && ui
                                    .small_button("Make it my zero")
                                    .on_hover_text(make_zero_hint(&u, &zeros, height))
                                    .clicked()
                            {
                                use_zero = Some(height);
                            }
                        });
                    }
                    None => {
                        ui.label("not found");
                    }
                });
            }
            row(ui, "Muzzle velocity today", &mut |ui| {
                ui.label(u.format(Quantity::Speed, solution.muzzle_velocity));
            });
            row(ui, "Stability (Miller Sg)", &mut |ui| {
                ui.label(
                    solution
                        .stability
                        .map_or("unknown: enter the bullet length".to_owned(), |sg| {
                            format!("{sg:.2}")
                        }),
                );
            });
        };
        egui::CollapsingHeader::new("Details")
            .id_salt("limit-details")
            .show(ui, |ui| {
                if narrow {
                    details(ui);
                } else {
                    let width = ui.available_width();
                    egui::Grid::new("limits")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .max_col_width(width * 0.7)
                        .show(ui, details);
                }
            });
        use_zero
    }

    /// The view through the scope, with its magnification and what it
    /// shows: held or dialled, lit or not.
    fn sight_picture(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        size: f32,
        requested: &mut ViewRequests,
    ) {
        ui.heading("Through the scope");
        let Some((scope, reticle)) = self.optics else {
            ui.weak("Choose a scope and reticle for this rifle (Rifles page) to see the sight picture here.");
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
        let picture = SightPicture {
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
        };
        let drawn = picture.show(ui, size);
        let mut chosen = magnification;
        if magnification_slider(ui, &mut chosen, scope, size).changed() {
            requested.magnification = Some(chosen);
        }
        view_toggles(ui, self.inputs, scope, requested);
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
                "Dial {} {}, then hold the orange dot on the target.",
                clicks_text(dialled_clicks.abs() as i64),
                if dialled_clicks >= 0.0 { "up" } else { "down" }
            ));
        } else {
            ui.small("Hold the orange dot on the target: the crosshair then sits where the picture shows it.");
        }
        let u = units::prefs(ui.ctx());
        if let (Some(target), Some(zone)) = (&self.aim.target, self.aim.vital_diameter_cm) {
            let zones = self.aim.figure.as_ref().filter(|figure| !figure.zones.is_empty());
            let key = match zones {
                Some(figure) if figure.rings.is_some() => {
                    "green: the hit area, with the moose test figure's rings".to_owned()
                }
                Some(_) => "green: the hit area".to_owned(),
                None => format!(
                    "green circle: the {} {}",
                    u.format(Quantity::ShortLength, zone),
                    self.aim.zone_name()
                ),
            };
            ui.small(format!(
                "{} at {}, true size; {key}.",
                target.name,
                u.format(Quantity::Distance, row.range),
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
                "Field of view not published; estimated as {:.0} mrad from the magnification. Enter the scope's field of view (Rifles, My scopes) for a true picture.",
                drawn.field_of_view_mrad
            ));
        }
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
            ui.weak(clicks_text(self.clicks(elevation).abs()));
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
            ui.weak(clicks_text(self.clicks(windage).abs()));
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
        if let Some(figure) = self.aim.figure.as_ref().filter(|figure| !figure.zones.is_empty()) {
            // The figure's x runs towards the head, to the right in the view.
            let hit = figure.hit([row.windage, row.drop]);
            let ring = hit.score.map_or(String::new(), |score| format!(", ring {score}"));
            match hit.zone {
                Some(zone) => ui.label(format!("A dead-on hold lands in the {zone}{ring}.")),
                None => ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("A dead-on hold misses the hit area{ring}."),
                ),
            };
        }
        self.bracket_line(ui);
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
        let cant = self.inputs.cant_degrees;
        if cant != 0.0
            && let Some((right, up)) = solution.cant_shift(self.inputs.target_range_m)
        {
            let u = units::prefs(ui.ctx());
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "Canted {:.1}° {}: a dead-on hold lands {} {} and {} {} of a level one. \
                     The holds above are for the tilted reticle; better to hold the rifle level.",
                    cant.abs(),
                    if cant > 0.0 { "right" } else { "left" },
                    u.format(Quantity::ShortLength, right.abs() * 100.0),
                    if right >= 0.0 { "right" } else { "left" },
                    u.format(Quantity::ShortLength, up.abs() * 100.0),
                    if up >= 0.0 { "higher" } else { "lower" },
                ),
            );
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
            && (0.8..1.2).contains(&row.mach)
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
        let max_range = self.table.end_m.max(self.inputs.target_range_m);
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

    /// The range table, with at most `limit` columns besides the range.
    /// Returns true when some were left out.
    fn table(
        &self,
        ui: &mut egui::Ui,
        solution: &Solution,
        ethical: Option<f64>,
        limit: Option<usize>,
    ) -> bool {
        let settings = self.table;
        let u = units::prefs(ui.ctx());
        let header = |name: &str, quantity: Quantity| format!("{name} {}", u.label(quantity));
        let (range_header, drop_header, wind_header, speed_header, energy_header, lead_header) = (
            header("Range", Quantity::Distance),
            header("Height", Quantity::ShortLength),
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
                show.wind_length,
                &wind_header,
                false,
                Box::new(|row| u.number(Quantity::ShortLength, row.windage * 100.0, true)),
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
        let mut columns: Vec<(&str, bool, Cell<'_>)> = candidates
            .into_iter()
            .filter(|column| column.0)
            .map(|(_, header, correction, cell)| (header, correction, cell))
            .collect();
        let left_out = limit.is_some_and(|limit| columns.len() > limit + 1);
        if let Some(limit) = limit
            && left_out
        {
            // Keep the columns that matter most for a shot: the clicks, the
            // angles in the scope's unit, the lead, the energy.
            let (own, other) = match self.unit() {
                ClickUnit::Mil => ("mil", "MOA"),
                ClickUnit::Moa => ("MOA", "mil"),
            };
            let rank = |header: &str| match header {
                "Elev clicks" => 0,
                _ if header == format!("Elev {own}") => 1,
                _ if header == format!("Wind {own}") => 2,
                _ if header == lead_header => 3,
                _ if header == energy_header => 4,
                "Wind clicks" => 5,
                _ if header == drop_header => 6,
                _ if header == format!("Elev {other}") => 7,
                _ => 8,
            };
            let mut order: Vec<usize> = (1..columns.len()).collect();
            order.sort_by_key(|&index| rank(columns[index].0));
            order.truncate(limit);
            columns = columns
                .into_iter()
                .enumerate()
                .filter(|(index, _)| *index == 0 || order.contains(index))
                .map(|(_, column)| column)
                .collect();
        }

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
        ui.small(if ethical.is_some() {
            "Elevation and wind are corrections: up and right positive. Red rows are past your \
             ethical range. Confirm the table on paper at real distances before hunting."
        } else {
            "Elevation and wind are corrections: up and right positive. Confirm the table on \
             paper at real distances."
        });
        left_out
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

/// "to 305 m (±15 cm)", "25–262 m (±15 cm)".
fn point_blank_short(u: &UnitPrefs, point_blank: Option<PointBlank>, aim: &Aim) -> String {
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
            format!("{}–{} (±{zone})", distance(near_limit), distance(far_limit))
        }
        Some(PointBlank { far_limit, .. }) => format!("to {} (±{zone})", distance(far_limit)),
        None => format!("never inside the {}", aim.zone_name()),
    }
}

/// What "Make it my zero" does, for its hover text.
fn make_zero_hint(u: &UnitPrefs, zeros: &Zeros, height: f64) -> String {
    let told = |height: f64| {
        format!(
            "{} {}",
            u.format(Quantity::ShortLength, height.abs() * 100.0),
            if height >= 0.0 { "high" } else { "low" }
        )
    };
    let reference = zeros.reference;
    let rifle = height - zeros.offset_up * reference;
    if zeros.offset_up == 0.0 {
        format!(
            "After you have zeroed the rifle {} at {}: saves it as the rifle's zero.",
            told(height),
            u.format(Quantity::Distance, reference),
        )
    } else {
        format!(
            "This bullet hits {} at {} with the rifle zeroed {} with its zero load. After \
             zeroing like that, saves it as the rifle's zero.",
            told(height),
            u.format(Quantity::Distance, reference),
            told(rifle),
        )
    }
}

/// Dial or hold, and the reticle lit: how the sight picture shows the shot.
fn view_toggles(ui: &mut egui::Ui, inputs: &Inputs, scope: &Scope, requested: &mut ViewRequests) {
    ui.horizontal_wrapped(|ui| {
        let mut dial = inputs.dial_elevation;
        if ui
            .checkbox(&mut dial, "Dial the elevation")
            .on_hover_text("Otherwise the picture shows the hold over")
            .changed()
        {
            requested.dial_elevation = Some(dial);
        }
        if scope.illuminated {
            let mut lit = inputs.illumination;
            if ui.checkbox(&mut lit, "Illumination").changed() {
                requested.illumination = Some(lit);
            }
        }
    });
}

/// The range table's ranges and columns.
fn table_settings(ui: &mut egui::Ui, table: &mut TableSettings, unit: ClickUnit) {
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
        ui.label("Every");
        ui.horizontal_wrapped(|ui| {
            let u = units::prefs(ui.ctx());
            let distance = u.unit(Quantity::Distance);
            for every in [10.0, 25.0, 50.0, 100.0] {
                let metres = distance.base(every);
                if ui
                    .selectable_label((table.step_m - metres).abs() < 0.01, format!("{every:.0}"))
                    .clicked()
                {
                    table.step_m = metres;
                }
            }
            units::drag(ui, &mut table.step_m, Quantity::Distance, 1.0..=500.0, 1.0);
        });
        ui.end_row();
    });
    ui.separator();
    let columns = &mut table.columns;
    let length = units::prefs(ui.ctx())
        .label(Quantity::ShortLength)
        .to_owned();
    let (angle, other) = match unit {
        ClickUnit::Mil => ("mil", "MOA"),
        ClickUnit::Moa => ("MOA", "mil"),
    };
    grid(ui, "table-columns", |ui| {
        ui.label("Elevation");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut columns.drop_clicks, "clicks");
            match unit {
                ClickUnit::Mil => {
                    ui.checkbox(&mut columns.drop_mil, angle);
                    ui.checkbox(&mut columns.drop_moa, other);
                }
                ClickUnit::Moa => {
                    ui.checkbox(&mut columns.drop_moa, angle);
                    ui.checkbox(&mut columns.drop_mil, other);
                }
            }
            ui.checkbox(&mut columns.drop_length, &length)
                .on_hover_text("Where a dead-on hold hits: + above the aim, − below");
        });
        ui.end_row();
        ui.label("Wind");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut columns.wind_clicks, "clicks");
            match unit {
                ClickUnit::Mil => {
                    ui.checkbox(&mut columns.wind_mil, angle);
                    ui.checkbox(&mut columns.wind_moa, other);
                }
                ClickUnit::Moa => {
                    ui.checkbox(&mut columns.wind_moa, angle);
                    ui.checkbox(&mut columns.wind_mil, other);
                }
            }
            ui.checkbox(&mut columns.wind_length, &length);
        });
        ui.end_row();
        ui.label("Also");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut columns.velocity, "speed");
            ui.checkbox(&mut columns.energy, "energy");
            ui.checkbox(&mut columns.time, "time of flight");
        });
        ui.end_row();
    });
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
    use ballistics_store::{VelocityPoint, ZeroWeather};

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
            shots: None,
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
    fn target_shooting_has_no_ethical_limits() {
        let inputs = Inputs::default();
        let hunting = Data {
            hunting: true,
            ..Data::default()
        };
        let aim = Aim::new(&inputs, &hunting);
        assert!(aim.is_game() && aim.min_energy_j.is_some());
        assert_eq!(aim.zone_name(), "vital zone");
        let shooting = Data::default();
        let aim = Aim::new(&inputs, &shooting);
        assert!(!aim.is_game());
        assert_eq!(aim.min_energy_j, None);
        assert_eq!(aim.min_velocity_mps, None);
        assert_eq!(aim.zone_name(), "hit zone");
        assert!(aim.vital_diameter_cm.is_some(), "the hit zone stays");
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

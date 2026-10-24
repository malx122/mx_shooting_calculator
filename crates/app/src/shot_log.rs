//! The shot log: shots logged from Calculate, newest first, each with the
//! solution the app gave, what came of it and, after a shot at game, where
//! the hunter stood and the bearing to the animal. From those the page
//! works out where the animal stood and guides the hunter there by
//! distance and direction. There is no map: nothing about the shot leaves
//! the device.

use std::sync::mpsc::{Receiver, Sender, channel};

use ballistics_core::RangeRow;
use ballistics_shot::{from_radians, unit_label};
use ballistics_store::model::{new_id, now_ms};
use ballistics_store::shots::{self, AnimalSpot, GeoPoint};
use ballistics_store::sun::{self, Date};
use ballistics_store::{
    ClickUnit, Country, Outcome, Rifle, ShooterPosition, Shot, ShotHold, ShotSolution, ShotWeather,
};
use eframe::egui::{self, RichText, vec2};

use crate::app::{Action, Data, Page};
use crate::calculator::{Aim, Inputs, clicks_text};
use crate::i18n::{tr, tr_data, trf};
use crate::mobile;
use crate::sensors::{self, Fix};
use crate::theme;
use crate::units::{self, Quantity};
use crate::weather::{Place, compass_point};
use crate::widgets::{confirm_button, fold, grid};

/// A record of the shot Calculate shows: the solution at the range, in the
/// weather and wind set there. The app adds the bullet's name as it saves.
pub fn from_solution(inputs: &Inputs, rifle: &Rifle, aim: &Aim, row: &RangeRow) -> Shot {
    let conditions = inputs.conditions();
    let unit = rifle.click_unit;
    let clicks = |radians: f64| (from_radians(unit, radians) / rifle.click_value).round() as i64;
    Shot {
        id: new_id(),
        taken_at: now_ms(),
        rifle_id: rifle.id.clone(),
        rifle_name: rifle.name.clone(),
        bullet_id: inputs.bullet_id.clone().unwrap_or_default(),
        bullet_name: String::new(),
        target_id: aim.target.as_ref().map(|target| target.id.clone()),
        target_name: aim
            .target
            .as_ref()
            .map_or_else(String::new, |target| target.name.clone()),
        range_m: inputs.target_range_m,
        inclination_degrees: inputs.inclination_degrees,
        wind_speed_mps: inputs.wind_speed,
        wind_clock: inputs.wind_clock.rem_euclid(12.0),
        weather: ShotWeather {
            temperature_c: conditions.temperature_c,
            powder_temperature_c: conditions
                .powder_temperature_c
                .unwrap_or(conditions.temperature_c),
            pressure_hpa: conditions.pressure.station_hpa(),
            humidity_percent: conditions.humidity_percent,
        },
        hold: ShotHold {
            elevation_mrad: row.elevation_correction * 1000.0,
            windage_mrad: row.windage_correction * 1000.0,
            click_unit: unit,
            click_value: rifle.click_value,
            elevation_clicks: clicks(row.elevation_correction),
            windage_clicks: clicks(row.windage_correction),
        },
        solution: ShotSolution {
            drop_cm: row.drop * 100.0,
            windage_cm: row.windage * 100.0,
            velocity_mps: row.velocity,
            energy_j: row.energy_j,
            time_of_flight_s: row.time,
        },
        outcome: Outcome::NotRecorded,
        hit_where: String::new(),
        position: None,
        bearing_degrees: None,
        animal_spot: None,
        notes: String::new(),
        updated_at: 0,
    }
}

/// The phone's sensors at work for the open shot.
struct Sensing {
    /// Waiting for the position, for the shot with this id.
    marking: Option<String>,
    /// Reading the compass for the bearing.
    taking_bearing: bool,
    /// Following the hunter's position to guide them to the animal.
    guiding: bool,
    /// Why the position couldn't be found.
    error: Option<String>,
    sender: Sender<(String, Result<Fix, String>)>,
}

impl Sensing {
    fn stop(&mut self) {
        if self.guiding {
            sensors::stop_following();
        }
        self.guiding = false;
        self.taking_bearing = false;
        self.error = None;
    }
}

pub struct ShotLogPage {
    /// The shot open, as being edited.
    open: Option<Box<Shot>>,
    sensing: Sensing,
    receiver: Receiver<(String, Result<Fix, String>)>,
}

impl Default for ShotLogPage {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            open: None,
            sensing: Sensing {
                marking: None,
                taking_bearing: false,
                guiding: false,
                error: None,
                sender,
            },
            receiver,
        }
    }
}

impl ShotLogPage {
    /// Opens a shot, as when it was just logged.
    pub fn open(&mut self, shot: &Shot) {
        self.sensing.stop();
        self.open = Some(Box::new(shot.clone()));
    }

    /// Keeps the open shot as it was saved.
    pub fn saved(&mut self, shot: &Shot) {
        if self.open.as_ref().is_some_and(|open| open.id == shot.id) {
            self.open = Some(Box::new(shot.clone()));
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if self.open.as_ref().is_some_and(|open| open.id == id) {
            self.close();
        }
    }

    /// Stops following the position when the hunter goes to another page.
    pub fn leave(&mut self) {
        if self.sensing.guiding {
            self.sensing.stop();
        }
    }

    fn close(&mut self) {
        self.sensing.stop();
        self.open = None;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Data, place: Option<&Place>) -> Vec<Action> {
        let context = &ui.ctx().clone();
        self.collect_position();
        let mut actions = Vec::new();
        if mobile::is_narrow(context) {
            // On a phone: the list, or one shot with a way back.
            egui::CentralPanel::default().show(ui, |ui| {
                if self.open.is_none() {
                    if mobile::back_button(ui, tr("Calculate")) {
                        actions.push(Action::Goto(Page::Calculate));
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data, true));
                } else {
                    if mobile::back_button(ui, tr("Shot log")) {
                        self.close();
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| self.detail(ui, data, place, &mut actions));
                }
            });
            return actions;
        }
        egui::Panel::left("shot-list")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data, false));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.detail(ui, data, place, &mut actions));
        });
        actions
    }

    /// Takes the position the device found, into the shot it was for.
    fn collect_position(&mut self) {
        while let Ok((id, found)) = self.receiver.try_recv() {
            if self.sensing.marking.as_deref() == Some(id.as_str()) {
                self.sensing.marking = None;
            }
            match (found, &mut self.open) {
                (Ok(fix), Some(shot)) if shot.id == id => {
                    shot.position = Some(ShooterPosition {
                        point: GeoPoint {
                            latitude_degrees: fix.latitude,
                            longitude_degrees: fix.longitude,
                        },
                        accuracy_m: fix.accuracy_m,
                    });
                    self.sensing.error = None;
                }
                (Err(error), Some(shot)) if shot.id == id => self.sensing.error = Some(error),
                _ => {}
            }
        }
    }

    /// The shots, newest first: when, what at, how far and how it went.
    fn list(&mut self, ui: &mut egui::Ui, data: &Data, phone: bool) {
        ui.heading(tr("Shot log"));
        if data.shots.is_empty() {
            ui.weak(tr(
                "No shots yet. On Calculate, press Log this shot when you shoot: the hold, \
                 the weather and the range are kept, and you can add what came of it.",
            ));
            return;
        }
        let u = units::prefs(ui.ctx());
        let mut chosen = None;
        for shot in &data.shots {
            let selected = self.open.as_ref().is_some_and(|open| open.id == shot.id);
            let title = format!(
                "{} · {}",
                when(shot.taken_at, data.country),
                target_of(shot)
            );
            let detail = format!(
                "{} · {} · {}",
                u.format(Quantity::Distance, shot.range_m),
                rifle_of(shot, data),
                outcome_label(shot.outcome)
            );
            let clicked = if phone {
                mobile::list_row(ui, &title, &detail, selected).clicked()
            } else {
                ui.selectable_label(selected, format!("{title}\n{detail}"))
                    .clicked()
            };
            if clicked {
                chosen = Some(shot.clone());
            }
        }
        if let Some(shot) = chosen {
            self.open(&shot);
        }
    }

    /// The open shot: what the app said, what came of it, and where.
    fn detail(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        place: Option<&Place>,
        actions: &mut Vec<Action>,
    ) {
        let Some(shot) = &mut self.open else {
            ui.weak(tr(
                "Choose a shot, or log one from the results on Calculate.",
            ));
            return;
        };
        let u = units::prefs(ui.ctx());
        ui.heading(trf!(
            "{} at {}",
            target_of(shot),
            u.format(Quantity::Distance, shot.range_m)
        ));
        ui.weak(format!(
            "{} · {} · {}",
            when(shot.taken_at, data.country),
            rifle_of(shot, data),
            bullet_of(shot, data)
        ));
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Outcome"));
            for outcome in Outcome::ALL {
                ui.selectable_value(&mut shot.outcome, outcome, outcome_label(outcome));
            }
        });
        if shot.outcome == Outcome::Hit {
            ui.horizontal(|ui| {
                ui.label(tr("Where it hit"));
                ui.add(
                    egui::TextEdit::singleline(&mut shot.hit_where)
                        .hint_text(tr("heart–lung, shoulder…"))
                        .desired_width(220.0),
                );
            });
        }
        ui.add_space(4.0);
        fold(
            ui,
            "shot-solution",
            tr("The solution"),
            &hold_summary(&shot.hold),
            |ui| solution(ui, shot),
        );
        ui.add_space(8.0);

        ui.label(RichText::new(tr("Where the animal stood")).strong());
        whereabouts(ui, shot, &mut self.sensing, data, place);
        ui.add_space(8.0);

        ui.label(tr("Notes"));
        ui.add(
            egui::TextEdit::multiline(&mut shot.notes)
                .hint_text(tr("Where it went, what you found…"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        let stored = data.shots.iter().find(|stored| stored.id == shot.id);
        let close = ui
            .horizontal(|ui| {
                if ui.button(tr("Save")).clicked() {
                    actions.push(Action::SaveShot(shot.clone()));
                }
                if stored.is_some() && confirm_button(ui, "delete-shot", tr("Delete")) {
                    actions.push(Action::DeleteShot(shot.id.clone()));
                }
                let close = ui.button(tr("Close")).clicked();
                if stored.is_none_or(|stored| stored != &**shot) {
                    ui.weak(tr("Not saved yet"));
                }
                close
            })
            .inner;
        if close {
            self.close();
        }
    }
}

/// "Moose", or "Shot" with no target; a catalog target in the current
/// language.
fn target_of(shot: &Shot) -> String {
    let catalog = shot
        .target_id
        .as_ref()
        .is_some_and(|id| ballistics_store::Catalog::builtin().target(id).is_some());
    if shot.target_name.is_empty() {
        tr("Shot").to_owned()
    } else if catalog {
        tr_data(&shot.target_name).to_owned()
    } else {
        shot.target_name.clone()
    }
}

/// "Hit", in the current language.
fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::NotRecorded => tr("Not recorded"),
        Outcome::Hit => tr("Hit"),
        Outcome::Miss => tr("Miss"),
    }
}

/// The rifle's name now, or as it was when the shot was logged.
fn rifle_of(shot: &Shot, data: &Data) -> String {
    data.rifles
        .iter()
        .find(|rifle| rifle.id == shot.rifle_id)
        .map_or_else(|| shot.rifle_name.clone(), |rifle| rifle.name.clone())
}

fn bullet_of(shot: &Shot, data: &Data) -> String {
    data.bullets
        .iter()
        .find(|bullet| bullet.id == shot.bullet_id)
        .map_or_else(|| shot.bullet_name.clone(), |bullet| bullet.display_name())
}

/// "12 Oct 2026, 07:42": on the device's clock where it says (in the
/// browser), else in the hunting rules' country, else UTC.
fn when(unix_ms: i64, country: Option<Country>) -> String {
    let (date, minutes, zone) = match (sensors::utc_offset_minutes(unix_ms), country) {
        (Some(offset), _) => {
            let local = unix_ms + (offset * 60_000.0) as i64;
            let minutes = local.rem_euclid(86_400_000) as f64 / 60_000.0;
            (Date::from_unix_ms(local), minutes, "")
        }
        (None, Some(country)) => {
            let (date, minutes) = country.hunting().local_now(unix_ms);
            (date, minutes, "")
        }
        (None, None) => {
            let minutes = unix_ms.rem_euclid(86_400_000) as f64 / 60_000.0;
            (Date::from_unix_ms(unix_ms), minutes, " UTC")
        }
    };
    format!(
        "{} {}, {}{zone}",
        crate::hunting::day_text(date),
        date.year,
        sun::clock(minutes.floor())
    )
}

/// An angle in the hold's unit: "↑ 0.60 mil".
fn angle_text(mrad: f64, unit: ClickUnit, positive: &str, negative: &str) -> String {
    let value = from_radians(unit, mrad / 1000.0);
    format!(
        "{} {:.2} {}",
        if value >= 0.0 { positive } else { negative },
        value.abs(),
        unit_label(unit)
    )
}

/// "↑ 0.60 mil · → 0.30 mil"
fn hold_summary(hold: &ShotHold) -> String {
    format!(
        "{} · {}",
        angle_text(hold.elevation_mrad, hold.click_unit, "↑", "↓"),
        angle_text(hold.windage_mrad, hold.click_unit, "→", "←")
    )
}

/// "3 o'clock", "2:30".
fn clock_text(clock: f64) -> String {
    let hour = clock.rem_euclid(12.0);
    let hour = if hour < 0.25 { 12.0 } else { hour };
    let minutes = (hour.fract() * 60.0).round();
    if minutes < 1.0 || minutes > 59.0 {
        trf!("{:.0} o'clock", hour.round())
    } else {
        format!("{}:{minutes:02.0}", hour.floor())
    }
}

/// What the app said for the shot: the hold, the path and the weather.
fn solution(ui: &mut egui::Ui, shot: &Shot) {
    let u = units::prefs(ui.ctx());
    let hold = &shot.hold;
    let solution = &shot.solution;
    let weather = &shot.weather;
    grid(ui, "shot-solution-grid", |ui| {
        ui.label(tr("Elevation"));
        ui.label(format!(
            "{} ({} {})",
            angle_text(
                hold.elevation_mrad,
                hold.click_unit,
                tr("↑ up"),
                tr("↓ down")
            ),
            clicks_text(hold.elevation_clicks.abs()),
            if hold.elevation_clicks >= 0 {
                tr("up")
            } else {
                tr("down")
            }
        ));
        ui.end_row();
        ui.label(tr("Windage"));
        ui.label(format!(
            "{} ({} {})",
            angle_text(
                hold.windage_mrad,
                hold.click_unit,
                tr("→ right"),
                tr("← left")
            ),
            clicks_text(hold.windage_clicks.abs()),
            if hold.windage_clicks >= 0 {
                tr("right")
            } else {
                tr("left")
            }
        ));
        ui.end_row();
        ui.label(tr("Dead-on hold hits"));
        ui.label(format!(
            "{} {}, {} {}",
            u.format(Quantity::ShortLength, solution.drop_cm.abs()),
            if solution.drop_cm >= 0.0 {
                tr("high")
            } else {
                tr("low")
            },
            u.format(Quantity::ShortLength, solution.windage_cm.abs()),
            if solution.windage_cm >= 0.0 {
                tr("right")
            } else {
                tr("left")
            }
        ));
        ui.end_row();
        ui.label(tr("At the target"));
        ui.label(trf!(
            "{} · {} · {:.3} s flight",
            u.format(Quantity::Speed, solution.velocity_mps),
            u.format(Quantity::Energy, solution.energy_j),
            solution.time_of_flight_s
        ));
        ui.end_row();
        ui.label(tr("Shot angle"));
        ui.label(format!("{:+.0}°", shot.inclination_degrees));
        ui.end_row();
        ui.label(tr("Wind"));
        ui.label(trf!(
            "{} from {}",
            u.format(Quantity::WindSpeed, shot.wind_speed_mps),
            clock_text(shot.wind_clock)
        ));
        ui.end_row();
        ui.label(tr("Weather"));
        let mut text = format!(
            "{} · {} · {:.0} %",
            u.format(Quantity::Temperature, weather.temperature_c),
            u.format(Quantity::Pressure, weather.pressure_hpa),
            weather.humidity_percent
        );
        if (weather.powder_temperature_c - weather.temperature_c).abs() >= 0.05 {
            text.push_str(&trf!(
                " · cartridges {}",
                u.format(Quantity::Temperature, weather.powder_temperature_c)
            ));
        }
        ui.label(text);
        ui.end_row();
    });
}

/// "64.751234° N, 20.953456° E"
fn point_text(point: GeoPoint) -> String {
    format!(
        "{:.6}° {}, {:.6}° {}",
        point.latitude_degrees.abs(),
        if point.latitude_degrees >= 0.0 {
            tr("N")
        } else {
            tr("S")
        },
        point.longitude_degrees.abs(),
        if point.longitude_degrees >= 0.0 {
            tr("E")
        } else {
            tr("W")
        }
    )
}

/// Where the hunter stood and the bearing to the animal, marked with the
/// phone or typed; then where the animal stood, and the way there.
fn whereabouts(
    ui: &mut egui::Ui,
    shot: &mut Shot,
    sensing: &mut Sensing,
    data: &Data,
    place: Option<&Place>,
) {
    let u = units::prefs(ui.ctx());
    let context = ui.ctx().clone();
    grid(ui, "shot-where", |ui| {
        ui.label(tr("Your position"));
        ui.horizontal_wrapped(|ui| {
            match &mut shot.position {
                Some(position) => {
                    let point = &mut position.point;
                    let before = *point;
                    ui.add(
                        egui::DragValue::new(&mut point.latitude_degrees)
                            .range(-90.0..=90.0)
                            .speed(0.00001)
                            .fixed_decimals(6)
                            .prefix(tr("lat "))
                            .suffix("°"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut point.longitude_degrees)
                            .range(-180.0..=180.0)
                            .speed(0.00001)
                            .fixed_decimals(6)
                            .prefix(tr("lon "))
                            .suffix("°"),
                    );
                    // Moved by hand: no longer what the device said.
                    if *point != before {
                        position.accuracy_m = None;
                    }
                    if let Some(accuracy) = position.accuracy_m {
                        ui.weak(format!("± {}", u.format(Quantity::Distance, accuracy)));
                    }
                    if ui.small_button("✖").on_hover_text(tr("Clear")).clicked() {
                        shot.position = None;
                    }
                }
                None => {
                    ui.weak(tr("not marked"));
                    if ui
                        .button(tr("Type it"))
                        .on_hover_text(tr("Latitude and longitude in degrees, from a map or a GPS"))
                        .clicked()
                    {
                        shot.position = Some(ShooterPosition {
                            point: typed_start(data, place),
                            accuracy_m: None,
                        });
                    }
                }
            }
            if sensors::can_locate() && sensing.marking.is_none() {
                let label = if shot.position.is_some() {
                    tr("📍 Mark again")
                } else {
                    tr("📍 Mark my position")
                };
                if ui
                    .button(label)
                    .on_hover_text(tr(
                        "Where you stand now, from the phone's GPS (the browser asks first). \
                         Mark it from where you shot.",
                    ))
                    .clicked()
                {
                    sensing.marking = Some(shot.id.clone());
                    sensing.error = None;
                    let sender = sensing.sender.clone();
                    let id = shot.id.clone();
                    let repaint = context.clone();
                    sensors::fix(move |found| {
                        let _ = sender.send((id.clone(), found));
                        repaint.request_repaint();
                    });
                }
            }
            if sensing.marking.is_some() {
                ui.spinner();
                ui.weak(tr("Finding your position…"));
            }
        });
        ui.end_row();

        ui.label(tr("Bearing to the animal"));
        ui.horizontal_wrapped(|ui| {
            match &mut shot.bearing_degrees {
                Some(bearing) => {
                    ui.add(
                        egui::DragValue::new(bearing)
                            .range(0.0..=359.9)
                            .speed(1.0)
                            .fixed_decimals(0)
                            .suffix("°"),
                    );
                    ui.weak(compass_point(*bearing));
                    if ui.small_button("✖").on_hover_text(tr("Clear")).clicked() {
                        shot.bearing_degrees = None;
                    }
                }
                None => {
                    ui.weak(tr("not taken"));
                    if ui
                        .button(tr("Type it"))
                        .on_hover_text(tr("Degrees from north: 90 is east"))
                        .clicked()
                    {
                        shot.bearing_degrees = Some(0.0);
                    }
                }
            }
            if sensors::has_compass() {
                if sensing.taking_bearing {
                    match sensors::heading() {
                        Some(heading) => {
                            ui.label(trf!("phone: {:.0}°", heading));
                            if ui.button(tr("Use")).clicked() {
                                shot.bearing_degrees = Some(heading.round().rem_euclid(360.0));
                                sensing.taking_bearing = false;
                            }
                        }
                        None => {
                            ui.weak(tr("waiting for the compass…"));
                        }
                    }
                    ui.ctx().request_repaint();
                } else if ui
                    .button(tr("🧭 Take bearing"))
                    .on_hover_text(tr(
                        "From where you shot: hold the phone flat, its top edge towards where \
                         the animal stood",
                    ))
                    .clicked()
                {
                    sensors::start_compass(ui.ctx().clone());
                    sensing.taking_bearing = true;
                }
            }
        });
        ui.end_row();
    });
    if let Some(error) = &sensing.error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }

    let level = shot.horizontal_range_m();
    if shot.inclination_degrees.abs() >= 0.5 {
        ui.weak(trf!(
            "{} away on the level: {} along the line of sight at {:+.0}°.",
            u.format(Quantity::Distance, level),
            u.format(Quantity::Distance, shot.range_m),
            shot.inclination_degrees
        ));
    }
    shot.update_spot();
    let Some(spot) = shot.animal_spot else {
        ui.weak(tr(
            "Mark where you shot from and take the bearing to the animal, and the app works out \
             where it stood.",
        ));
        return;
    };
    ui.label(trf!(
        "It stood at {}. Search within {} of it.",
        point_text(spot.point),
        u.format(Quantity::Distance, spot.search_radius_m)
    ))
    .on_hover_text(tr(
        "The search area allows for a compass 10° off, a range 5 % off (at least 5 m) and your \
         position's accuracy.",
    ));
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(tr("Copy coordinates"))
            .on_hover_text(tr(
                "As latitude, longitude in degrees, to paste where you like",
            ))
            .clicked()
        {
            ui.ctx().copy_text(format!(
                "{:.6}, {:.6}",
                spot.point.latitude_degrees, spot.point.longitude_degrees
            ));
        }
        if sensors::can_locate() && !sensing.guiding {
            let guide = ui.button(tr("🧭 Guide me there")).on_hover_text(tr(
                "The distance and direction from where you are (the browser asks first)",
            ));
            if guide.clicked() {
                sensors::follow(ui.ctx().clone());
                if sensors::has_compass() {
                    sensors::start_compass(ui.ctx().clone());
                }
                sensing.guiding = true;
            }
        }
    });
    if sensing.guiding {
        guide(ui, spot, sensing);
    }
}

/// Where typing a position starts: the calculator's place, else the last
/// marked position.
fn typed_start(data: &Data, place: Option<&Place>) -> GeoPoint {
    match place {
        Some(place) => GeoPoint {
            latitude_degrees: place.latitude,
            longitude_degrees: place.longitude,
        },
        None => data
            .shots
            .iter()
            .find_map(|shot| shot.position.map(|position| position.point))
            .unwrap_or(GeoPoint {
                latitude_degrees: 0.0,
                longitude_degrees: 0.0,
            }),
    }
}

/// The way from where the hunter is to the animal's spot: how far, which
/// way, and an arrow when the compass knows where the phone points.
fn guide(ui: &mut egui::Ui, spot: AnimalSpot, sensing: &mut Sensing) {
    let u = units::prefs(ui.ctx());
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));
    egui::Frame::new()
        .fill(theme::FOREST_700)
        .corner_radius(12)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match sensors::followed() {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak(tr("Finding where you are…"));
                    });
                }
                Some(Err(error)) => {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                Some(Ok(fix)) => {
                    let here = GeoPoint {
                        latitude_degrees: fix.latitude,
                        longitude_degrees: fix.longitude,
                    };
                    let (distance, bearing) = shots::distance_and_bearing(here, spot.point);
                    ui.label(
                        RichText::new(trf!(
                            "{}, bearing {:.0}° ({})",
                            u.format(Quantity::Distance, distance),
                            bearing,
                            compass_point(bearing)
                        ))
                        .size(22.0)
                        .strong(),
                    );
                    if distance <= spot.search_radius_m {
                        ui.colored_label(
                            theme::MEADOW,
                            tr("Within the search area: look around you."),
                        );
                    }
                    if let Some(heading) = sensors::heading() {
                        arrow(ui, bearing - heading);
                        ui.weak(tr("Hold the phone flat and walk where the arrow points."));
                    }
                    if let Some(accuracy) = fix.accuracy_m {
                        ui.weak(trf!(
                            "Your position ± {}",
                            u.format(Quantity::Distance, accuracy)
                        ));
                    }
                }
            }
            if ui.button(tr("Stop")).clicked() {
                sensing.stop();
            }
        });
}

/// An arrow turned `degrees` clockwise from the phone's top edge.
fn arrow(ui: &mut egui::Ui, degrees: f64) {
    let size = 120.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = size / 2.0 - 4.0;
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.5_f32, theme::FOREST_500),
    );
    let angle = degrees.to_radians() as f32;
    let way = vec2(angle.sin(), -angle.cos());
    let side = vec2(-way.y, way.x);
    let tip = center + way * (radius - 6.0);
    let tail = center - way * (radius - 18.0);
    let stroke = egui::Stroke::new(5.0_f32, theme::BLAZE);
    painter.line_segment([tail, tip - way * 14.0], stroke);
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip,
            tip - way * 22.0 + side * 12.0,
            tip - way * 22.0 - side * 12.0,
        ],
        theme::BLAZE,
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_directions_read_as_hunters_say_them() {
        assert_eq!(clock_text(3.0), "3 o'clock");
        assert_eq!(clock_text(0.0), "12 o'clock");
        assert_eq!(clock_text(2.5), "2:30");
    }

    /// A shot at the moose at 200 m, held 1.2 mrad up and 0.36 left.
    fn logged() -> Shot {
        let mut rifle = Rifle::new("Tikka", "308-win", 10.0);
        rifle.click_value = 0.1;
        let inputs = Inputs {
            bullet_id: Some("oryx".into()),
            target_range_m: 200.0,
            wind_clock: 12.0,
            ..Inputs::default()
        };
        let target = ballistics_store::Catalog::builtin()
            .target("moose")
            .cloned();
        let aim = Aim {
            vital_diameter_cm: target.as_ref().map(|target| target.vital_diameter_cm),
            target,
            min_energy_j: None,
            min_velocity_mps: None,
            figure: None,
            hunting: true,
        };
        let row = RangeRow {
            range: 200.0,
            time: 0.28,
            drop: -0.24,
            windage: -0.07,
            elevation_correction: 0.0012,
            windage_correction: -0.00036,
            velocity: 690.0,
            mach: 2.0,
            energy_j: 2780.0,
        };
        from_solution(&inputs, &rifle, &aim, &row)
    }

    #[test]
    fn a_logged_shot_keeps_the_hold_in_the_rifles_clicks() {
        let shot = logged();
        assert_eq!(shot.bullet_id, "oryx");
        assert_eq!(shot.target_id.as_deref(), Some("moose"));
        assert_eq!(shot.range_m, 200.0);
        assert!((shot.hold.elevation_mrad - 1.2).abs() < 1e-9);
        assert_eq!(
            (shot.hold.elevation_clicks, shot.hold.windage_clicks),
            (12, -4)
        );
        assert!((shot.solution.drop_cm + 24.0).abs() < 1e-9);
        assert_eq!(shot.wind_clock, 0.0, "12 o'clock kept as 0");
        assert!(shot.weather.pressure_hpa > 900.0 && shot.weather.pressure_hpa < 1013.0);
        assert!(shot.validate().is_ok(), "{:?}", shot.validate());
        assert_eq!(hold_summary(&shot.hold), "↑ 1.20 mil · ← 0.36 mil");
    }

    #[test]
    fn the_page_draws_on_a_desktop_and_a_phone() {
        let mut shot = logged();
        shot.position = Some(ShooterPosition {
            point: GeoPoint {
                latitude_degrees: 64.75,
                longitude_degrees: 20.95,
            },
            accuracy_m: Some(6.0),
        });
        shot.bearing_degrees = Some(52.0);
        shot.outcome = Outcome::Hit;
        let data = Data {
            shots: vec![shot.clone()],
            ..Data::default()
        };
        for width in [1200.0, 390.0] {
            let context = egui::Context::default();
            let mut page = ShotLogPage::default();
            page.open(&shot);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(width, 800.0),
                )),
                ..Default::default()
            };
            let mut actions = Vec::new();
            for _ in 0..2 {
                let mut output = context.run_ui(input.clone(), |ui| {
                    actions = page.show(ui, &data, None);
                });
                // Nothing draws here: the textures need no uploading.
                output.textures_delta.clear();
            }
            assert!(actions.is_empty());
            let open = page.open.as_ref().expect("still open");
            assert!(open.animal_spot.is_some(), "the spot is worked out");
        }
    }

    #[test]
    fn times_without_a_zone_are_utc() {
        // 2026-10-20 14:15:00 UTC.
        assert_eq!(when(1_792_505_700_000, None), "20 Oct 2026, 14:15 UTC");
        // Sweden keeps summer time until the last Sunday of October.
        assert_eq!(
            when(1_792_505_700_000, Some(Country::Sweden)),
            "20 Oct 2026, 16:15"
        );
    }
}

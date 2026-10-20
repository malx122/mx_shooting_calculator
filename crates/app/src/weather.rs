//! The weather where the hunter is, from Open-Meteo (free, no key; in the
//! Nordics it uses MET Norway's high-resolution model): air temperature,
//! sea-level pressure and the ground's height, humidity and the wind. The
//! place comes from the phone's position or a place search. The forecast
//! wind is turned into a clock direction from the direction of the shot,
//! taken from the phone's compass or typed in.

use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};

use crate::calculator::{Inputs, PressureSource};
use crate::http::{self, Method, Request};
use crate::units::{self, Quantity};

const FORECAST: &str = "https://api.open-meteo.com/v1/forecast";
const GEOCODING: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// Forecast wind is for 10 m above open ground; at a hunter's 1–2 m it is
/// roughly this much (a logarithmic wind profile over grass), and less in
/// forest.
const SHOOTING_HEIGHT_FACTOR: f64 = 0.72;

/// A place to get the weather for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// The current weather at a place.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub place: Place,
    /// Local time of the observation, as Open-Meteo gives it.
    pub time: String,
    pub temperature_c: f64,
    pub humidity_percent: f64,
    pub pressure_msl_hpa: f64,
    /// The ground's height there (a terrain model), m.
    pub elevation_m: f64,
    /// 10 m above ground.
    pub wind_speed_mps: f64,
    pub wind_gusts_mps: f64,
    /// Where the wind comes from, degrees from north.
    pub wind_from_degrees: f64,
}

enum Reply {
    Located(Result<Place, String>),
    Weather(Result<Report, String>),
    Places(Result<Vec<Place>, String>),
}

/// The weather panel's state.
pub struct Weather {
    pub report: Option<Report>,
    busy: Option<&'static str>,
    error: Option<String>,
    search: String,
    found: Vec<Place>,
    /// The phone's compass is being read to set the shot direction.
    compass: bool,
    sender: Sender<Reply>,
    receiver: Receiver<Reply>,
}

impl Default for Weather {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Weather {
            report: None,
            busy: None,
            error: None,
            search: String::new(),
            found: Vec::new(),
            compass: false,
            sender,
            receiver,
        }
    }
}

#[derive(Deserialize)]
struct ForecastAnswer {
    elevation: f64,
    current: Current,
}

#[derive(Deserialize)]
struct Current {
    time: String,
    temperature_2m: f64,
    relative_humidity_2m: f64,
    pressure_msl: f64,
    wind_speed_10m: f64,
    wind_gusts_10m: Option<f64>,
    wind_direction_10m: f64,
}

#[derive(Deserialize)]
struct GeocodingAnswer {
    #[serde(default)]
    results: Vec<GeocodingPlace>,
}

#[derive(Deserialize)]
struct GeocodingPlace {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    country_code: Option<String>,
    admin1: Option<String>,
}

async fn fetch_json<T: serde::de::DeserializeOwned>(url: String) -> Result<T, String> {
    let response = http::fetch(Request {
        method: Method::Get,
        url,
        headers: Vec::new(),
        body: None,
    })
    .await?;
    if response.status != 200 {
        return Err(format!("the weather service answered {}", response.status));
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| "the weather service's answer was garbled".into())
}

async fn forecast(place: Place) -> Result<Report, String> {
    let url = format!(
        "{FORECAST}?latitude={:.4}&longitude={:.4}&current=temperature_2m,relative_humidity_2m,\
         pressure_msl,wind_speed_10m,wind_gusts_10m,wind_direction_10m&wind_speed_unit=ms&timezone=auto",
        place.latitude, place.longitude
    );
    let answer: ForecastAnswer = fetch_json(url).await?;
    let current = answer.current;
    Ok(Report {
        place,
        time: current.time.replace('T', " "),
        temperature_c: current.temperature_2m,
        humidity_percent: current.relative_humidity_2m,
        pressure_msl_hpa: current.pressure_msl,
        elevation_m: answer.elevation,
        wind_speed_mps: current.wind_speed_10m,
        wind_gusts_mps: current.wind_gusts_10m.unwrap_or(current.wind_speed_10m),
        wind_from_degrees: current.wind_direction_10m,
    })
}

/// Searches places by name, those in `home` (a country code such as "SE")
/// first, then the rest of the Nordics, then the world.
async fn search(name: String, home: Option<&'static str>) -> Result<Vec<Place>, String> {
    let query: String = name
        .trim()
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' => {
                (byte as char).to_string()
            }
            b' ' => "+".to_owned(),
            other => format!("%{other:02X}"),
        })
        .collect();
    let answer: GeocodingAnswer =
        fetch_json(format!("{GEOCODING}?name={query}&count=20&format=json")).await?;
    let rank = |code: &Option<String>| match code.as_deref() {
        Some(code) if Some(code) == home => 0,
        Some("SE" | "FI" | "NO" | "DK" | "IS" | "EE") => 1,
        _ => 2,
    };
    let mut results = answer.results;
    results.sort_by_key(|place| rank(&place.country_code));
    results.truncate(6);
    Ok(results
        .into_iter()
        .map(|place| {
            let mut name = place.name;
            for extra in [place.admin1, place.country].into_iter().flatten() {
                if !name.contains(&extra) {
                    name.push_str(", ");
                    name.push_str(&extra);
                }
            }
            Place {
                name,
                latitude: place.latitude,
                longitude: place.longitude,
            }
        })
        .collect())
}

impl Weather {
    fn send(&self, context: &egui::Context) -> impl Fn(Reply) + Send + 'static {
        let sender = self.sender.clone();
        let context = context.clone();
        move |reply| {
            let _ = sender.send(reply);
            context.request_repaint();
        }
    }

    fn fetch(&mut self, context: &egui::Context, place: Place) {
        self.busy = Some("Getting the weather…");
        self.error = None;
        let reply = self.send(context);
        http::spawn(async move { reply(Reply::Weather(forecast(place).await)) });
    }

    /// Collects answers. Returns a new report, which the caller applies.
    pub fn poll(&mut self, context: &egui::Context) -> Option<Report> {
        let mut fresh = None;
        while let Ok(reply) = self.receiver.try_recv() {
            match reply {
                Reply::Located(Ok(place)) => self.fetch(context, place),
                Reply::Weather(Ok(report)) => {
                    self.busy = None;
                    self.report = Some(report.clone());
                    fresh = Some(report);
                }
                Reply::Places(Ok(places)) => {
                    self.busy = None;
                    if places.is_empty() {
                        self.error = Some("no place by that name".into());
                    }
                    self.found = places;
                }
                Reply::Located(Err(error))
                | Reply::Weather(Err(error))
                | Reply::Places(Err(error)) => {
                    self.busy = None;
                    self.error = Some(error);
                }
            }
        }
        fresh
    }

    /// The panel above the weather inputs: where to get the weather, what
    /// came, and the forecast wind for this shot.
    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        inputs: &mut Inputs,
        country: Option<ballistics_store::Country>,
    ) {
        let context = ui.ctx().clone();
        ui.horizontal_wrapped(|ui| {
            if crate::sensors::can_locate()
                && ui
                    .button("📍 Weather here")
                    .on_hover_text("From your position (the browser asks first)")
                    .clicked()
            {
                self.busy = Some("Finding where you are…");
                self.error = None;
                let reply = self.send(&context);
                crate::sensors::locate(move |place| reply(Reply::Located(place)));
            }
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("or a place: Mora, Kuopio…")
                    .desired_width(150.0),
            );
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if (ui.button("Search").clicked() || enter) && !self.search.trim().is_empty() {
                self.busy = Some("Searching…");
                self.error = None;
                self.found.clear();
                let reply = self.send(&context);
                let name = self.search.clone();
                let home = country.map(|country| match country {
                    ballistics_store::Country::Sweden => "SE",
                    ballistics_store::Country::Finland => "FI",
                });
                http::spawn(async move { reply(Reply::Places(search(name, home).await)) });
            }
            if let Some(report) = &self.report
                && ui.small_button("↻").on_hover_text("Get it again").clicked()
            {
                let place = report.place.clone();
                self.fetch(&context, place);
            }
        });
        let mut chosen = None;
        for place in &self.found {
            if ui.link(&place.name).clicked() {
                chosen = Some(place.clone());
            }
        }
        if let Some(place) = chosen {
            self.found.clear();
            self.search.clear();
            self.fetch(&context, place);
        }
        if let Some(busy) = self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(busy);
            });
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let Some(report) = self.report.clone() else {
            return;
        };
        let u = units::prefs(ui.ctx());
        ui.label(
            RichText::new(format!(
                "{} at {}: {}, {}, {:.0} %, ground {}",
                report.place.name,
                report.time.rsplit(' ').next().unwrap_or(&report.time),
                u.format(Quantity::Temperature, report.temperature_c),
                u.format(Quantity::Pressure, report.pressure_msl_hpa),
                report.humidity_percent,
                u.format(Quantity::Altitude, report.elevation_m),
            ))
            .small(),
        );
        ui.label(
            RichText::new(format!(
                "Wind {} from {} ({:.0}°), gusts {}, 10 m above open ground",
                u.format(Quantity::WindSpeed, report.wind_speed_mps),
                compass_point(report.wind_from_degrees),
                report.wind_from_degrees,
                u.format(Quantity::WindSpeed, report.wind_gusts_mps),
            ))
            .small(),
        );
        self.wind_for_shot(ui, inputs, &report);
        ui.hyperlink_to(
            RichText::new("Weather by Open-Meteo.com").small(),
            "https://open-meteo.com/",
        );
    }

    /// The forecast wind as a clock direction for the shot's direction.
    fn wind_for_shot(&mut self, ui: &mut egui::Ui, inputs: &mut Inputs, report: &Report) {
        let u = units::prefs(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            ui.label("Shooting towards");
            ui.add(
                egui::DragValue::new(&mut inputs.azimuth_degrees)
                    .range(0.0..=359.0)
                    .speed(1.0)
                    .suffix("°"),
            );
            ui.weak(compass_point(inputs.azimuth_degrees));
            if crate::sensors::has_compass() {
                if self.compass {
                    match crate::sensors::heading() {
                        Some(heading) => {
                            ui.label(format!("phone: {heading:.0}°"));
                            if ui.button("Use").clicked() {
                                inputs.azimuth_degrees = heading.round().rem_euclid(360.0);
                                self.compass = false;
                            }
                        }
                        None => {
                            ui.weak("waiting for the compass…");
                        }
                    }
                    ui.ctx().request_repaint();
                } else if ui
                    .button("🧭 Point the phone")
                    .on_hover_text("Hold the phone flat, its top edge towards the target")
                    .clicked()
                {
                    crate::sensors::start_compass(ui.ctx().clone());
                    self.compass = true;
                }
            }
        });
        let clock = wind_clock(report.wind_from_degrees, inputs.azimuth_degrees);
        let speed = shooting_height_speed(report);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(format!(
                    "Use the wind: {} from {}",
                    u.format(Quantity::WindSpeed, speed),
                    clock_text(clock)
                ))
                .on_hover_text(
                    "The forecast wind brought down to shooting height over open ground \
                     (about 70 % of the 10 m wind). In forest it is usually less: adjust it.",
                )
                .clicked()
            {
                inputs.wind_speed = (speed * 10.0).round() / 10.0;
                inputs.wind_clock = clock;
            }
        });
    }
}

/// The forecast wind brought down to shooting height over open ground.
pub fn shooting_height_speed(report: &Report) -> f64 {
    report.wind_speed_mps * SHOOTING_HEIGHT_FACTOR
}

/// Fills the calculator's weather from a report.
pub fn apply(report: &Report, inputs: &mut Inputs) {
    inputs.temperature_c = report.temperature_c;
    inputs.pressure_source = PressureSource::SeaLevelAndAltitude;
    inputs.sea_level_pressure_hpa = report.pressure_msl_hpa;
    inputs.altitude_m = report.elevation_m;
    inputs.humidity_percent = report.humidity_percent;
    inputs.latitude_degrees = report.place.latitude;
}

/// Where a wind from `from` degrees comes from on the clock, for a shot
/// towards `towards` degrees: 12 is from the target, 3 from the right.
pub fn wind_clock(from: f64, towards: f64) -> f64 {
    let hours = ((from - towards).rem_euclid(360.0) / 30.0 * 2.0).round() / 2.0;
    if hours == 0.0 { 12.0 } else { hours }
}

fn clock_text(clock: f64) -> String {
    if clock.fract() == 0.0 {
        format!("{clock:.0} o'clock")
    } else {
        format!("{:.0}:30", clock.floor())
    }
}

/// "SW" for 225°.
pub fn compass_point(degrees: f64) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    POINTS[((degrees.rem_euclid(360.0) / 22.5).round() as usize) % 16]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forecast_wind_becomes_a_clock_direction() {
        // Shooting north: a west wind comes from the left (9), an east wind
        // from the right (3), a north wind from the target (12).
        assert_eq!(wind_clock(270.0, 0.0), 9.0);
        assert_eq!(wind_clock(90.0, 0.0), 3.0);
        assert_eq!(wind_clock(0.0, 0.0), 12.0);
        assert_eq!(wind_clock(180.0, 0.0), 6.0);
        // Shooting south-west with a south-west wind: head-on.
        assert_eq!(wind_clock(225.0, 225.0), 12.0);
        // Shooting east with a north wind: from the left.
        assert_eq!(wind_clock(0.0, 90.0), 9.0);
        // Half hours.
        assert_eq!(wind_clock(45.0, 0.0), 1.5);
    }

    #[test]
    fn compass_points() {
        assert_eq!(compass_point(0.0), "N");
        assert_eq!(compass_point(225.0), "SW");
        assert_eq!(compass_point(359.0), "N");
        assert_eq!(compass_point(100.0), "E");
    }

    #[test]
    fn answers_from_open_meteo_parse() {
        let forecast: ForecastAnswer = serde_json::from_str(
            r#"{"latitude":61.0,"longitude":14.5,"elevation":165.0,"current_units":{},
               "current":{"time":"2026-10-20T14:15","interval":900,"temperature_2m":4.2,
               "relative_humidity_2m":81,"pressure_msl":1012.4,"wind_speed_10m":4.1,
               "wind_gusts_10m":7.9,"wind_direction_10m":225}}"#,
        )
        .unwrap();
        assert_eq!(forecast.elevation, 165.0);
        assert_eq!(forecast.current.wind_direction_10m, 225.0);
        let places: GeocodingAnswer = serde_json::from_str(
            r#"{"results":[{"id":1,"name":"Mora","latitude":61.0,"longitude":14.5,
               "country":"Sweden","admin1":"Dalarna"}]}"#,
        )
        .unwrap();
        assert_eq!(places.results[0].name, "Mora");
        let none: GeocodingAnswer = serde_json::from_str(r#"{"generationtime_ms":0.1}"#).unwrap();
        assert!(none.results.is_empty());
    }
}

//! The tools an AI assistant can call. Each takes JSON arguments and gives
//! back text for the assistant to read (and the same facts as JSON).
//! Everything is read-only for now.

use ballistics_core::{Coriolis, analysis, solver};
use ballistics_shot::{Conditions, Hold, Pressure, unit_label};
use ballistics_store::{Bullet, Catalog, Country, Rifle, RifleLoad, Store, Target, TargetGroup};
use serde_json::{Value, json};

/// The engine always flies this far, so the ethical limit can be found.
const SOLVE_RANGE_M: f64 = 1200.0;
/// The app's setting with the calculator's last inputs (weather and more).
const CALCULATOR_SETTINGS: &str = "calculator";
/// The app's setting with the hunter's country (whose rules to check).
const COUNTRY_SETTING: &str = "country";
/// The general minimum velocity for expansion, when the app has none saved.
const GENERAL_MIN_VELOCITY_MPS: f64 = 550.0;

/// A tool's answer: text, and the same as data when there is any.
pub struct Answer {
    pub text: String,
    pub data: Option<Value>,
}

impl Answer {
    fn text(text: impl Into<String>) -> Answer {
        Answer {
            text: text.into(),
            data: None,
        }
    }
}

/// The tools, as MCP lists them.
pub fn list() -> Value {
    let conditions = json!({
        "wind_speed_mps": {"type": "number", "description": "Wind speed in m/s. Default 0 (no wind)."},
        "wind_clock": {"type": "number", "description": "Where the wind comes FROM, as a clock face: 12 head-on, 3 from the right, 9 from the left, 6 from behind. Default 3."},
        "angle_degrees": {"type": "number", "description": "Shot angle: positive uphill, negative downhill. Default 0."},
        "temperature_c": {"type": "number", "description": "Air temperature in °C. Default: the weather last entered in the app."},
        "pressure_hpa": {"type": "number", "description": "Station (local, barometer) pressure in hPa. Default: from the app."},
        "humidity_percent": {"type": "number", "description": "Relative humidity in percent. Default: from the app."},
        "powder_temperature_c": {"type": "number", "description": "Cartridge temperature in °C if it differs from the air's."}
    });
    let with = |extra: Value, required: &[&str]| {
        let mut properties = conditions.as_object().unwrap().clone();
        properties.extend(extra.as_object().unwrap().clone());
        json!({"type": "object", "properties": properties, "required": required})
    };
    json!([
        {
            "name": "list_rifles",
            "title": "List my rifles",
            "description": "The hunter's rifles: name, caliber, scope, zero, click value, and the bullets with measured velocities in each.",
            "inputSchema": {"type": "object", "properties": {}},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "list_bullets",
            "title": "List my bullets",
            "description": "The bullets and loads in the hunter's collection, optionally for one caliber or rifle.",
            "inputSchema": {"type": "object", "properties": {
                "caliber": {"type": "string", "description": "A caliber such as \".308 Win\" or \"6.5x55\", or a rifle's name."}
            }},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "list_targets",
            "title": "List targets",
            "description": "Game animals and sport targets with their vital zone, minimum energy and the rifle ammunition level Swedish and Finnish rules ask for. Search by English, Swedish or Finnish name.",
            "inputSchema": {"type": "object", "properties": {
                "search": {"type": "string", "description": "Part of a name, e.g. \"roe\", \"rådjur\", \"grouse\"."}
            }},
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "holdover",
            "title": "Holdover at a range",
            "description": "What to dial or hold with one of the hunter's rifles at a range: elevation and windage in the scope's units (mil or MOA) and clicks, drop, drift, velocity and energy, computed by the app's ballistic engine. With a target it also says whether the shot is inside the ethical range and whether the load is legal for that animal in the hunter's country (Sweden or Finland). Use this instead of estimating.",
            "inputSchema": with(json!({
                "rifle": {"type": "string", "description": "The rifle's name (or part of it, or its caliber)."},
                "bullet": {"type": "string", "description": "The bullet or load. Default: the one last used with this rifle."},
                "range_m": {"type": "number", "description": "Distance to the target in metres (line of sight)."},
                "target": {"type": "string", "description": "Optional animal or target, e.g. \"moose\" or \"rådjur\"."},
                "country": {"type": "string", "description": "Whose hunting law to check the ammunition against: \"Sweden\", \"Finland\" or \"none\". Default: the country set in the app."}
            }), &["rifle", "range_m"]),
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "dope_table",
            "title": "Dope table",
            "description": "A table of holds for one of the hunter's rifles over a span of ranges, in the scope's units and clicks.",
            "inputSchema": with(json!({
                "rifle": {"type": "string", "description": "The rifle's name (or part of it, or its caliber)."},
                "bullet": {"type": "string", "description": "The bullet or load. Default: the one last used with this rifle."},
                "from_m": {"type": "number", "description": "First range in metres. Default: the rifle's table settings."},
                "to_m": {"type": "number", "description": "Last range in metres. Default: the rifle's table settings."},
                "step_m": {"type": "number", "description": "Step in metres. Default: the rifle's table settings."}
            }), &["rifle"]),
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "search_catalog",
            "title": "Search the bullet catalog",
            "description": "Factory ammunition and component bullets in the app's catalog (not the hunter's own collection): maker, weight, BC, velocity, construction, lead-free.",
            "inputSchema": {"type": "object", "properties": {
                "search": {"type": "string", "description": "Words to match: maker, product, caliber, e.g. \"norma 308 ecostrike\"."},
                "lead_free": {"type": "boolean", "description": "Only lead-free bullets."}
            }},
            "annotations": {"readOnlyHint": true}
        }
    ])
}

/// Runs a tool. `Err` is a message for the assistant (a bad argument or
/// nothing found), shown as a tool error.
pub fn call(store: &Store, name: &str, arguments: &Value) -> Result<Answer, String> {
    match name {
        "list_rifles" => list_rifles(store),
        "list_bullets" => list_bullets(store, arguments),
        "list_targets" => list_targets(store, arguments),
        "holdover" => holdover(store, arguments),
        "dope_table" => dope_table(store, arguments),
        "search_catalog" => search_catalog(arguments),
        other => Err(format!("there is no tool called {other}")),
    }
}

fn caliber_name(key: &str) -> String {
    Catalog::builtin()
        .caliber(key)
        .map_or_else(|| key.to_owned(), |caliber| caliber.name.clone())
}

fn scope_name(store: &Store, rifle: &Rifle) -> Option<String> {
    let id = rifle.scope_id.as_deref()?;
    let custom = store
        .scopes()
        .ok()?
        .into_iter()
        .find(|scope| scope.id == id);
    match custom {
        Some(scope) => Some(scope.display_name()),
        None => Catalog::builtin()
            .scope(id)
            .map(|scope| scope.display_name()),
    }
}

fn store_error(error: ballistics_store::StoreError) -> String {
    format!("could not read the app's data: {error}")
}

fn list_rifles(store: &Store) -> Result<Answer, String> {
    let rifles = store.rifles().map_err(store_error)?;
    if rifles.is_empty() {
        return Ok(Answer::text("The hunter has no rifles in the app yet."));
    }
    let bullets = store.bullets().map_err(store_error)?;
    let loads = store.rifle_loads().map_err(store_error)?;
    let mut lines = Vec::new();
    let mut data = Vec::new();
    for rifle in &rifles {
        let measured: Vec<String> = loads
            .iter()
            .filter(|load| load.rifle_id == rifle.id)
            .filter_map(|load| bullets.iter().find(|b| b.id == load.bullet_id))
            .map(Bullet::display_name)
            .collect();
        let scope = scope_name(store, rifle);
        lines.push(format!(
            "- {} ({}): zero {} m, sight height {} cm, {:.2}\" twist, clicks {} {}{}{}",
            rifle.name,
            caliber_name(&rifle.caliber),
            rifle.zero_range_m,
            rifle.sight_height_cm,
            rifle.twist_in,
            rifle.click_value,
            unit_label(rifle.click_unit),
            scope
                .as_ref()
                .map_or(String::new(), |scope| format!(", scope {scope}")),
            if measured.is_empty() {
                String::new()
            } else {
                format!("; measured velocities with {}", measured.join(", "))
            }
        ));
        data.push(json!({
            "name": rifle.name,
            "caliber": caliber_name(&rifle.caliber),
            "zero_range_m": rifle.zero_range_m,
            "sight_height_cm": rifle.sight_height_cm,
            "twist_in": rifle.twist_in,
            "click_unit": unit_label(rifle.click_unit),
            "click_value": rifle.click_value,
            "scope": scope,
            "measured_bullets": measured,
        }));
    }
    Ok(Answer {
        text: format!("{} rifle(s):\n{}", rifles.len(), lines.join("\n")),
        data: Some(json!({ "rifles": data })),
    })
}

fn bullet_line(bullet: &Bullet) -> String {
    let bc = match (bullet.bc_g1, bullet.bc_g7) {
        (Some(g1), Some(g7)) => format!("G1 {g1}, G7 {g7}"),
        (Some(g1), None) => format!("G1 {g1}"),
        (None, Some(g7)) => format!("G7 {g7}"),
        (None, None) => "own drag curve".into(),
    };
    format!(
        "{} ({}): {bc}, {} m/s, {}{}",
        bullet.display_name(),
        caliber_name(&bullet.caliber),
        bullet.muzzle_velocity_mps,
        bullet.construction.label().to_lowercase(),
        if bullet.lead_free { ", lead-free" } else { "" }
    )
}

fn list_bullets(store: &Store, arguments: &Value) -> Result<Answer, String> {
    let mut bullets = store.bullets().map_err(store_error)?;
    if let Some(query) = arguments["caliber"]
        .as_str()
        .filter(|q| !q.trim().is_empty())
    {
        // A rifle's name, or a caliber.
        let rifles = store.rifles().map_err(store_error)?;
        let caliber = match find(&rifles, query, rifle_names) {
            Ok(rifle) => rifle.caliber.clone(),
            Err(_) => Catalog::builtin()
                .search_calibers(query)
                .first()
                .map(|caliber| caliber.key.clone())
                .ok_or_else(|| format!("no rifle or caliber matches \"{query}\""))?,
        };
        bullets.retain(|bullet| bullet.caliber == caliber);
    }
    if bullets.is_empty() {
        return Ok(Answer::text("No bullets in the hunter's collection match."));
    }
    let lines: Vec<String> = bullets
        .iter()
        .map(|b| format!("- {}", bullet_line(b)))
        .collect();
    let data: Vec<Value> = bullets
        .iter()
        .map(|b| {
            json!({
                "name": b.display_name(),
                "caliber": caliber_name(&b.caliber),
                "mass_g": b.mass_g,
                "bc_g1": b.bc_g1,
                "bc_g7": b.bc_g7,
                "muzzle_velocity_mps": b.muzzle_velocity_mps,
                "construction": b.construction.label(),
                "lead_free": b.lead_free,
            })
        })
        .collect();
    Ok(Answer {
        text: format!("{} bullet(s):\n{}", bullets.len(), lines.join("\n")),
        data: Some(json!({ "bullets": data })),
    })
}

fn all_targets(store: &Store) -> Result<Vec<Target>, String> {
    let mut targets = store.targets().map_err(store_error)?;
    targets.extend(Catalog::builtin().targets.iter().cloned());
    Ok(targets)
}

/// Whose rules to check: the `country` argument, else the app's setting,
/// else the device's language. `None` checks no law.
fn country(store: &Store, arguments: &Value) -> Result<Option<Country>, String> {
    if let Some(name) = arguments["country"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
    {
        let wanted = name.trim().to_lowercase();
        if wanted == "none" {
            return Ok(None);
        }
        return Country::ALL
            .into_iter()
            .find(|country| {
                country.name().to_lowercase() == wanted
                    || country.adjective().to_lowercase() == wanted
            })
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "the app knows the rules of {} only",
                    Country::ALL.map(Country::name).join(" and ")
                )
            });
    }
    Ok(match store.setting::<Option<Country>>(COUNTRY_SETTING) {
        Ok(Some(chosen)) => chosen,
        _ => sys_locale::get_locale()
            .as_deref()
            .and_then(Country::from_locale),
    })
}

fn target_line(target: &Target) -> String {
    let mut text = format!(
        "{}{}: vital zone {} cm",
        target.name,
        if target.other_names.is_empty() {
            String::new()
        } else {
            format!(" ({})", target.other_names.join(", "))
        },
        target.vital_diameter_cm
    );
    if let Some(energy) = target.min_energy_j.filter(|_| target.is_game()) {
        text.push_str(&format!(", guideline ≥ {energy} J at the animal"));
    }
    for country in Country::ALL {
        if let Some(level) = target.ammunition_level(country) {
            text.push_str(&format!(
                ", {} rules: rifle {}",
                country.adjective(),
                country.rules().requirement_text(level).to_lowercase()
            ));
        }
        if let Some(notes) = target.rules_notes.get(&country) {
            text.push_str(&format!(" ({notes})"));
        }
    }
    text
}

fn list_targets(store: &Store, arguments: &Value) -> Result<Answer, String> {
    let query = arguments["search"].as_str().unwrap_or("");
    let targets: Vec<Target> = all_targets(store)?
        .into_iter()
        .filter(|target| target.matches(query))
        .collect();
    if targets.is_empty() {
        return Ok(Answer::text(format!("No target matches \"{query}\".")));
    }
    let mut lines = Vec::new();
    for group in TargetGroup::ALL {
        let in_group: Vec<&Target> = targets.iter().filter(|t| t.group() == group).collect();
        if !in_group.is_empty() {
            lines.push(format!("{}:", group.label()));
            lines.extend(in_group.iter().map(|t| format!("- {}", target_line(t))));
        }
    }
    Ok(Answer::text(lines.join("\n")))
}

// Finding records by loose names -------------------------------------------

fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How closely `text` holds every word of `query`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Closeness {
    /// Somewhere inside words.
    Inside,
    /// At the start or end of words: "308" in ".308", and "räv" in the
    /// Swedish compound "rödräv" (the fox), but not in "grävling".
    Edges,
}

fn closeness(text: &str, query: &str) -> Option<Closeness> {
    let text = normalise(text);
    let words: Vec<&str> = text.split(' ').map(|w| w.trim_start_matches('.')).collect();
    let query = normalise(query);
    let wanted: Vec<&str> = query
        .split(' ')
        .map(|w| w.trim_start_matches('.'))
        .filter(|w| !w.is_empty())
        .collect();
    let all = |test: &dyn Fn(&str, &str) -> bool| {
        wanted
            .iter()
            .all(|wanted| words.iter().any(|word| test(word, wanted)))
    };
    if all(&|word, wanted| word.starts_with(wanted) || word.ends_with(wanted)) {
        Some(Closeness::Edges)
    } else if all(&|word, wanted| word.contains(wanted)) {
        Some(Closeness::Inside)
    } else {
        None
    }
}

/// Whether `text` holds the words of `query`, preferring word edges.
fn words_match(text: &str, query: &str) -> bool {
    closeness(text, query).is_some()
}

fn rifle_names(rifle: &Rifle) -> Vec<String> {
    vec![rifle.name.clone(), caliber_name(&rifle.caliber)]
}

fn bullet_names(bullet: &Bullet) -> Vec<String> {
    vec![
        bullet.display_name(),
        format!("{} {}", bullet.manufacturer, bullet.name),
    ]
}

/// The one record whose names match `query`: the exact name, or the only one
/// holding all its words. Otherwise says what was found.
fn find<'a, T>(
    items: &'a [T],
    query: &str,
    names: impl Fn(&T) -> Vec<String>,
) -> Result<&'a T, String> {
    let query = normalise(query);
    if let Some(exact) = items
        .iter()
        .find(|item| names(item).iter().any(|name| normalise(name) == query))
    {
        return Ok(exact);
    }
    // The closest matches only: a word's start or end beats its middle.
    let scored: Vec<(&T, Closeness)> = items
        .iter()
        .filter_map(|item| Some((item, closeness(&names(item).join(" "), &query)?)))
        .collect();
    let best = scored.iter().map(|(_, closeness)| *closeness).max();
    let matching: Vec<&T> = scored
        .iter()
        .filter(|(_, closeness)| Some(*closeness) == best)
        .map(|(item, _)| *item)
        .collect();
    match matching.as_slice() {
        [one] => Ok(one),
        [] => Err(format!(
            "nothing matches \"{query}\"; there are: {}",
            items
                .iter()
                .map(|item| names(item)[0].clone())
                .collect::<Vec<_>>()
                .join("; ")
        )),
        several => Err(format!(
            "\"{query}\" matches several: {}; say which",
            several
                .iter()
                .map(|item| names(item)[0].clone())
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

/// The rifle, its bullet and what was measured with it.
struct Equipment {
    rifle: Rifle,
    bullet: Bullet,
    load: Option<RifleLoad>,
}

fn equipment(store: &Store, arguments: &Value) -> Result<Equipment, String> {
    let rifles = store.rifles().map_err(store_error)?;
    if rifles.is_empty() {
        return Err("the hunter has no rifles in the app yet".into());
    }
    let rifle_query = arguments["rifle"]
        .as_str()
        .ok_or("say which rifle (\"rifle\")")?;
    let rifle = find(&rifles, rifle_query, rifle_names)?.clone();
    let fitting: Vec<Bullet> = store
        .bullets_for_caliber(&rifle.caliber)
        .map_err(store_error)?;
    if fitting.is_empty() {
        return Err(format!(
            "there are no {} bullets in the hunter's collection",
            caliber_name(&rifle.caliber)
        ));
    }
    let loads = store.rifle_loads().map_err(store_error)?;
    let bullet = match arguments["bullet"]
        .as_str()
        .filter(|q| !q.trim().is_empty())
    {
        Some(query) => find(&fitting, query, bullet_names)?.clone(),
        None => default_bullet(store, &rifle, &fitting, &loads)?,
    };
    let load = loads
        .into_iter()
        .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id);
    Ok(Equipment {
        rifle,
        bullet,
        load,
    })
}

/// The bullet last used with the rifle: the one picked in the app's
/// calculator for it, else the latest measured, else the only one.
fn default_bullet(
    store: &Store,
    rifle: &Rifle,
    fitting: &[Bullet],
    loads: &[RifleLoad],
) -> Result<Bullet, String> {
    let saved: Value = store
        .setting(CALCULATOR_SETTINGS)
        .ok()
        .flatten()
        .unwrap_or(Value::Null);
    if saved["rifle_id"].as_str() == Some(rifle.id.as_str())
        && let Some(bullet) = fitting
            .iter()
            .find(|b| Some(b.id.as_str()) == saved["bullet_id"].as_str())
    {
        return Ok(bullet.clone());
    }
    let latest = loads
        .iter()
        .filter(|load| load.rifle_id == rifle.id)
        .max_by_key(|load| load.updated_at)
        .and_then(|load| fitting.iter().find(|b| b.id == load.bullet_id));
    if let Some(bullet) = latest {
        return Ok(bullet.clone());
    }
    match fitting {
        [one] => Ok(one.clone()),
        several => Err(format!(
            "which bullet? {} has: {}",
            rifle.name,
            several
                .iter()
                .map(Bullet::display_name)
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

/// The conditions: the arguments, else the weather last entered in the
/// app's calculator, else a standard day. Wind and angle default to none.
fn conditions(store: &Store, arguments: &Value) -> (Conditions, String) {
    let saved: Value = store
        .setting(CALCULATOR_SETTINGS)
        .ok()
        .flatten()
        .unwrap_or(Value::Null);
    let mut conditions = Conditions::default();
    let mut from_app = false;
    if let Some(temperature) = saved["temperature_c"].as_f64() {
        from_app = true;
        conditions.temperature_c = temperature;
        conditions.humidity_percent = saved["humidity_percent"].as_f64().unwrap_or(50.0);
        conditions.pressure = if saved["pressure_source"] == "Station" {
            Pressure::Station(saved["station_pressure_hpa"].as_f64().unwrap_or(1013.25))
        } else {
            Pressure::SeaLevel {
                hpa: saved["sea_level_pressure_hpa"].as_f64().unwrap_or(1013.25),
                altitude_m: saved["altitude_m"].as_f64().unwrap_or(0.0),
            }
        };
        if saved["powder_follows_air"] == false {
            conditions.powder_temperature_c = saved["powder_temperature_c"].as_f64();
        }
        conditions.spin_drift = saved["spin_drift"].as_bool().unwrap_or(true);
        if saved["coriolis"] == true {
            conditions.coriolis = Some(Coriolis {
                latitude_degrees: saved["latitude_degrees"].as_f64().unwrap_or(60.0),
                azimuth_degrees: saved["azimuth_degrees"].as_f64().unwrap_or(0.0),
            });
        }
    }
    let number = |name: &str| arguments[name].as_f64();
    if let Some(temperature) = number("temperature_c") {
        conditions.temperature_c = temperature;
    }
    if let Some(pressure) = number("pressure_hpa") {
        conditions.pressure = Pressure::Station(pressure);
    }
    if let Some(humidity) = number("humidity_percent") {
        conditions.humidity_percent = humidity;
    }
    if let Some(powder) = number("powder_temperature_c") {
        conditions.powder_temperature_c = Some(powder);
    }
    conditions.wind_speed_mps = number("wind_speed_mps").unwrap_or(0.0);
    conditions.wind_clock = number("wind_clock").unwrap_or(3.0);
    conditions.inclination_degrees = number("angle_degrees").unwrap_or(0.0);
    let wind = if conditions.wind_speed_mps > 0.0 {
        format!(
            "wind {} m/s from {} o'clock",
            conditions.wind_speed_mps, conditions.wind_clock
        )
    } else {
        "no wind".into()
    };
    let description = format!(
        "{:.0} °C, {:.0} hPa at the shooter, {:.0} % humidity{}, {wind}, {}",
        conditions.temperature_c,
        conditions.pressure.station_hpa(),
        conditions.humidity_percent,
        if from_app && number("temperature_c").is_none() {
            " (weather last entered in the app)"
        } else {
            ""
        },
        match conditions.inclination_degrees {
            angle if angle > 0.0 => format!("{angle}° uphill"),
            angle if angle < 0.0 => format!("{}° downhill", -angle),
            _ => "level".into(),
        }
    );
    (conditions, description)
}

fn general_min_velocity(store: &Store) -> f64 {
    store
        .setting::<Value>(CALCULATOR_SETTINGS)
        .ok()
        .flatten()
        .and_then(|saved| saved["min_velocity"].as_f64())
        .unwrap_or(GENERAL_MIN_VELOCITY_MPS)
}

fn direction(value: f64, positive: &'static str, negative: &'static str) -> &'static str {
    if value >= 0.0 { positive } else { negative }
}

/// "1 click", "12 clicks".
fn clicks(count: i64) -> String {
    match count.abs() {
        1 => "1 click".into(),
        count => format!("{count} clicks"),
    }
}

fn holdover(store: &Store, arguments: &Value) -> Result<Answer, String> {
    let range = arguments["range_m"]
        .as_f64()
        .filter(|range| *range > 0.0)
        .ok_or("give the range in metres (\"range_m\")")?;
    let Equipment {
        rifle,
        bullet,
        load,
    } = equipment(store, arguments)?;
    let (conditions, weather) = conditions(store, arguments);
    let scenario = ballistics_shot::scenario(&rifle, &bullet, load.as_ref(), &conditions);
    let solution = solver::solve(&scenario, range.max(SOLVE_RANGE_M))
        .map_err(|error| format!("the engine found no solution: {error}"))?;
    let row = solution
        .at(range)
        .ok_or_else(|| format!("the bullet doesn't reach {range} m"))?;
    let hold = Hold::new(&rifle, &row);
    let unit = unit_label(hold.unit);
    let mut text = format!(
        "{} ({}) with {} at {range} m; {weather}.\n\
         Dial or hold {} {:.2} {unit} ({} of {} {unit}) and {} {:.2} {unit} ({}).\n\
         Holding dead on, the bullet hits {:.0} cm {} and {:.0} cm {}.\n\
         At the target: {:.0} m/s, {:.0} J, {:.2} s flight.",
        rifle.name,
        caliber_name(&rifle.caliber),
        bullet.display_name(),
        direction(hold.elevation, "up", "down"),
        hold.elevation.abs(),
        clicks(hold.elevation_clicks),
        rifle.click_value,
        direction(hold.windage, "right", "left"),
        hold.windage.abs(),
        clicks(hold.windage_clicks),
        hold.drop_m.abs() * 100.0,
        direction(hold.drop_m, "high", "low"),
        hold.drift_m.abs() * 100.0,
        direction(hold.drift_m, "right", "left"),
        hold.velocity_mps,
        hold.energy_j,
        hold.time_s,
    );
    if load.is_some() {
        text.push_str("\nUses the velocities measured with this bullet in this rifle.");
    }
    let mut data = json!({
        "rifle": rifle.name,
        "bullet": bullet.display_name(),
        "conditions": weather,
        "hold": hold,
    });
    if let Some(query) = arguments["target"]
        .as_str()
        .filter(|q| !q.trim().is_empty())
    {
        let targets = all_targets(store)?;
        let target = find(&targets, query, |t: &Target| {
            let mut names = vec![t.name.clone()];
            names.extend(t.other_names.iter().cloned());
            names
        })?;
        let country = country(store, arguments)?;
        let (ethics, facts) = ethics(store, &bullet, &solution, target, range, country);
        text.push('\n');
        text.push_str(&ethics);
        data["target"] = facts;
    }
    Ok(Answer {
        text,
        data: Some(data),
    })
}

/// Whether a shot at `range` is ethical for the target, and what the
/// country's law says.
fn ethics(
    store: &Store,
    bullet: &Bullet,
    solution: &ballistics_core::Solution,
    target: &Target,
    range: f64,
    country: Option<Country>,
) -> (String, Value) {
    let mut lines = vec![format!("Target: {}", target_line(target))];
    let mut facts = json!({ "name": target.name, "vital_zone_cm": target.vital_diameter_cm });
    if target.is_game() {
        let energy = target.min_energy_j.unwrap_or(0.0);
        let velocity = target
            .min_velocity_mps
            .unwrap_or_else(|| ballistics_shot::min_velocity(bullet, general_min_velocity(store)));
        let limit = analysis::ethical_limit(solution, energy, velocity);
        let inside = limit.is_none_or(|limit| range <= limit);
        lines.push(match limit {
            Some(limit) => format!(
                "Ethical range: up to {limit:.0} m (≥ {energy:.0} J and ≥ {velocity:.0} m/s at the animal) — {range} m is {}.",
                if inside { "inside it" } else { "BEYOND it" }
            ),
            None => format!("Ethical range: beyond {SOLVE_RANGE_M} m (≥ {energy:.0} J, ≥ {velocity:.0} m/s)."),
        });
        facts["ethical_range_m"] = json!(limit);
        facts["inside_ethical_range"] = json!(inside);
        match country {
            Some(country) => match target.ammunition_level(country) {
                Some(needed) => {
                    let assessment = ballistics_shot::assess(bullet, solution, country);
                    let (allowed, text) = assessment.verdict(needed, &target.name);
                    lines.push(text);
                    if let Some(notes) = target.rules_notes.get(&country) {
                        lines.push(notes.clone());
                    }
                    lines.push(format!("(Law: {}.)", country.rules().law));
                    facts["country"] = json!(country);
                    facts["level_needed"] = json!(needed);
                    facts["load"] = json!(assessment);
                    facts["allowed"] = json!(allowed);
                }
                None => lines.push(format!(
                    "The app has no {} rifle rule for {}.",
                    country.adjective(),
                    target.name
                )),
            },
            None => lines.push(
                "No country set in the app, so no legal check (pass \"country\" for one).".into(),
            ),
        }
    }
    (lines.join("\n"), facts)
}

fn dope_table(store: &Store, arguments: &Value) -> Result<Answer, String> {
    let Equipment {
        rifle,
        bullet,
        load,
    } = equipment(store, arguments)?;
    let (conditions, weather) = conditions(store, arguments);
    let from = arguments["from_m"]
        .as_f64()
        .unwrap_or(rifle.table.start_m)
        .max(0.0);
    let to = arguments["to_m"].as_f64().unwrap_or(rifle.table.end_m);
    let step = arguments["step_m"].as_f64().unwrap_or(rifle.table.step_m);
    if step <= 0.0 || to < from || (to - from) / step > 200.0 {
        return Err("give a sensible span: from_m ≤ to_m and at most 200 rows".into());
    }
    let scenario = ballistics_shot::scenario(&rifle, &bullet, load.as_ref(), &conditions);
    let solution = solver::solve(&scenario, to.max(SOLVE_RANGE_M))
        .map_err(|error| format!("the engine found no solution: {error}"))?;
    let unit = unit_label(rifle.click_unit);
    let mut lines = vec![
        format!(
            "{} ({}) with {}; {weather}. Elevation and windage to dial (+ up / right), in {unit} and clicks of {} {unit}:",
            rifle.name,
            caliber_name(&rifle.caliber),
            bullet.display_name(),
            rifle.click_value
        ),
        format!(
            "range m | elevation {unit} | clicks | windage {unit} | clicks | drop cm | m/s | J"
        ),
    ];
    let mut rows = Vec::new();
    let mut range = from;
    while range <= to + 1e-9 {
        if let Some(row) = solution.at(range) {
            let hold = Hold::new(&rifle, &row);
            lines.push(format!(
                "{:.0} | {:+.2} | {:+} | {:+.2} | {:+} | {:+.0} | {:.0} | {:.0}",
                range,
                hold.elevation,
                hold.elevation_clicks,
                hold.windage,
                hold.windage_clicks,
                hold.drop_m * 100.0,
                hold.velocity_mps,
                hold.energy_j
            ));
            rows.push(hold);
        }
        range += step;
    }
    Ok(Answer {
        text: lines.join("\n"),
        data: Some(
            json!({ "rifle": rifle.name, "bullet": bullet.display_name(), "conditions": weather, "rows": rows }),
        ),
    })
}

fn search_catalog(arguments: &Value) -> Result<Answer, String> {
    let query = arguments["search"].as_str().unwrap_or("");
    let lead_free = arguments["lead_free"].as_bool() == Some(true);
    let catalog = Catalog::builtin();
    let found: Vec<String> =
        catalog
            .bullets
            .iter()
            .filter(|bullet| !lead_free || bullet.lead_free)
            .filter(|bullet| {
                let caliber = bullet
                    .caliber
                    .as_deref()
                    .map(caliber_name)
                    .unwrap_or_default();
                let text = format!(
                    "{} {} {} {}",
                    bullet.display_name(),
                    caliber,
                    bullet.caliber.as_deref().unwrap_or(""),
                    bullet.construction.label()
                );
                words_match(&text, query)
            })
            .map(|bullet| {
                let bc = match (bullet.bc_g1, bullet.bc_g7) {
                    (Some(g1), Some(g7)) => format!("G1 {g1}, G7 {g7}"),
                    (Some(g1), None) => format!("G1 {g1}"),
                    (None, Some(g7)) => format!("G7 {g7}"),
                    (None, None) => "no BC".into(),
                };
                format!(
                    "- {}{}: {bc}{}, {}{}",
                    bullet.display_name(),
                    bullet.caliber.as_deref().map_or(
                        " (component bullet)".to_owned(),
                        |c| format!(" ({})", caliber_name(c))
                    ),
                    bullet
                        .muzzle_velocity_mps
                        .map_or(String::new(), |v| format!(", {v} m/s")),
                    bullet.construction.label().to_lowercase(),
                    if bullet.lead_free { ", lead-free" } else { "" }
                )
            })
            .collect();
    if found.is_empty() {
        return Ok(Answer::text("Nothing in the catalog matches."));
    }
    let shown = found.len().min(40);
    Ok(Answer::text(format!(
        "{} match(es){}:\n{}",
        found.len(),
        if found.len() > shown {
            ", the first 40"
        } else {
            ""
        },
        found[..shown].join("\n")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        store
    }

    fn text(store: &Store, tool: &str, arguments: Value) -> String {
        call(store, tool, &arguments).unwrap().text
    }

    #[test]
    fn every_listed_tool_runs() {
        let store = demo();
        for tool in list().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let arguments = match name {
                "holdover" => json!({"rifle": "x-bolt", "range_m": 200}),
                "dope_table" => json!({"rifle": "x-bolt"}),
                _ => json!({}),
            };
            call(&store, name, &arguments).unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }

    #[test]
    fn holdover_reads_like_the_app() {
        let store = demo();
        let answer = text(
            &store,
            "holdover",
            json!({"rifle": "browning", "range_m": 300, "wind_speed_mps": 4, "wind_clock": 3, "target": "älg", "country": "sweden"}),
        );
        assert!(answer.contains("Browning X-Bolt"), "{answer}");
        assert!(answer.contains("Dial or hold up"), "{answer}");
        assert!(answer.contains("right"), "{answer}");
        assert!(answer.contains("Moose"), "{answer}");
        assert!(answer.contains("Swedish rules"), "{answer}");
        let finnish = text(
            &store,
            "holdover",
            json!({"rifle": "browning", "range_m": 300, "target": "moose", "country": "finland"}),
        );
        assert!(finnish.contains("Finnish rules"), "{finnish}");
        let none = text(
            &store,
            "holdover",
            json!({"rifle": "browning", "range_m": 300, "target": "moose", "country": "none"}),
        );
        assert!(none.contains("no legal check"), "{none}");
    }

    #[test]
    fn the_same_numbers_as_the_shared_engine() {
        let store = demo();
        let answer = call(
            &store,
            "holdover",
            &json!({"rifle": "x-bolt", "range_m": 250}),
        )
        .unwrap();
        let rifle = store
            .rifles()
            .unwrap()
            .into_iter()
            .find(|r| r.name.contains("X-Bolt"))
            .unwrap();
        let Equipment { bullet, load, .. } =
            equipment(&store, &json!({"rifle": "x-bolt"})).unwrap();
        let (conditions, _) = conditions(&store, &json!({}));
        let solution = solver::solve(
            &ballistics_shot::scenario(&rifle, &bullet, load.as_ref(), &conditions),
            SOLVE_RANGE_M,
        )
        .unwrap();
        let expected = Hold::new(&rifle, &solution.at(250.0).unwrap());
        let got = &answer.data.unwrap()["hold"];
        assert!((got["elevation"].as_f64().unwrap() - expected.elevation).abs() < 1e-9);
    }

    #[test]
    fn names_match_loosely_but_not_ambiguously() {
        let store = demo();
        let rifles = store.rifles().unwrap();
        assert!(find(&rifles, "tikka", rifle_names).is_ok());
        assert!(find(&rifles, ".223", rifle_names).is_ok());
        assert!(find(&rifles, "cz 457 17 hmr", rifle_names).is_ok());
        assert!(
            find(&rifles, "sako", rifle_names)
                .unwrap_err()
                .contains("there are")
        );
        let error = call(
            &store,
            "holdover",
            &json!({"rifle": "nope", "range_m": 100}),
        )
        .err()
        .unwrap();
        assert!(error.contains("Tikka"), "{error}");
    }

    #[test]
    fn a_dope_table_has_its_rows() {
        let store = demo();
        let answer = text(
            &store,
            "dope_table",
            json!({"rifle": "tikka", "bullet": "oryx", "from_m": 100, "to_m": 300, "step_m": 50}),
        );
        assert_eq!(answer.lines().count(), 2 + 5, "{answer}");
    }

    #[test]
    fn catalog_and_targets_search() {
        let store = demo();
        let found = text(
            &store,
            "search_catalog",
            json!({"search": "norma 308", "lead_free": true}),
        );
        assert!(
            found.contains("Norma") && found.contains("lead-free"),
            "{found}"
        );
        let roe = text(&store, "list_targets", json!({"search": "rådjur"}));
        assert!(roe.contains("Roe deer"), "{roe}");
        // A word's start, not anywhere in it: "räv" is the fox, not the
        // badger ("grävling").
        let targets = all_targets(&store).unwrap();
        let fox = find(&targets, "räv", |t: &Target| {
            let mut names = vec![t.name.clone()];
            names.extend(t.other_names.iter().cloned());
            names
        })
        .unwrap();
        assert_eq!(fox.name, "Red fox");
    }
}

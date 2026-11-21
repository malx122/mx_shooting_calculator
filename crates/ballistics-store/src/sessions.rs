//! Range sessions: a day at the range, with its weather, that groups shot
//! there belong to.

use serde::{Deserialize, Serialize};

use crate::model::{NAME_MAX, first_failure, new_id};

/// The weather at the range. Every part may be unknown, and is then left
/// out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Conditions {
    /// Air temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<f64>,
    /// The cartridges' (powder's) temperature, when it differs from the
    /// air's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub powder_temperature_c: Option<f64>,
    /// Air pressure as a barometer at the range reads it (not reduced to sea
    /// level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_hpa: Option<f64>,
    /// Relative humidity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humidity_percent: Option<f64>,
    /// The wind's speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wind_speed_mps: Option<f64>,
    /// Where the wind came from, on a clock: 12 head-on, 3 from the right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wind_clock: Option<f64>,
}

impl Conditions {
    /// These conditions, with what they leave unknown taken from `base`:
    /// a group's own conditions over its session's.
    pub fn or(self, base: Conditions) -> Conditions {
        Conditions {
            temperature_c: self.temperature_c.or(base.temperature_c),
            powder_temperature_c: self.powder_temperature_c.or(base.powder_temperature_c),
            pressure_hpa: self.pressure_hpa.or(base.pressure_hpa),
            humidity_percent: self.humidity_percent.or(base.humidity_percent),
            wind_speed_mps: self.wind_speed_mps.or(base.wind_speed_mps),
            wind_clock: self.wind_clock.or(base.wind_clock),
        }
    }

    pub(crate) fn problem(&self) -> Option<&'static str> {
        let within = |value: Option<f64>, range: std::ops::RangeInclusive<f64>| {
            value.is_none_or(|v| range.contains(&v))
        };
        if !within(self.temperature_c, -60.0..=70.0)
            || !within(self.powder_temperature_c, -60.0..=70.0)
        {
            Some("temperatures must be -60 to 70 °C")
        } else if !within(self.pressure_hpa, 300.0..=1200.0) {
            Some("air pressure must be 300 to 1200 hPa")
        } else if !within(self.humidity_percent, 0.0..=100.0) {
            Some("humidity must be 0 to 100 %")
        } else if !within(self.wind_speed_mps, 0.0..=100.0) || !within(self.wind_clock, 0.0..=12.0)
        {
            Some("the wind must be 0–100 m/s from a clock direction of 0–12")
        } else {
            None
        }
    }
}

/// A day or an outing at the range. Groups name the session they were shot
/// in and take its conditions unless they have their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Session {
    /// The session's id, unique among sessions.
    pub id: String,
    /// Where, in the hunter's words (a range's name, not a position).
    #[serde(default)]
    pub place: String,
    /// When it began, in milliseconds since 1970 (UTC).
    pub started_at: i64,
    /// The weather.
    #[serde(flatten)]
    pub conditions: Conditions,
    /// The hunter's notes.
    #[serde(default)]
    pub notes: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl Session {
    pub fn new(started_at: i64) -> Self {
        Session {
            id: new_id(),
            place: String::new(),
            started_at,
            conditions: Conditions::default(),
            notes: String::new(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a session needs an id"),
            (
                self.place.chars().count() <= NAME_MAX,
                "the place's name is too long",
            ),
            (self.started_at >= 0, "the session's time is before 1970"),
            (
                self.conditions.problem().is_none(),
                self.conditions.problem().unwrap_or_default(),
            ),
        ];
        first_failure(&checks, &self.place)
    }
}

//! Display units. Everything is stored and computed in one base unit per
//! quantity (metres, m/s, °C, hPa, …); these helpers convert at the screen,
//! following the hunter's choice per quantity.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Quantity {
    /// Ranges and distances; base metres.
    Distance,
    /// Drop, drift, vital zone and other short lengths; base centimetres.
    ShortLength,
    /// Scope height above the bore; base centimetres.
    SightHeight,
    /// Bullet speed; base m/s.
    Speed,
    /// Wind and moving-game speed; base m/s.
    WindSpeed,
    /// Base °C.
    Temperature,
    /// Velocity change per degree; base m/s per °C.
    Sensitivity,
    /// Base hPa.
    Pressure,
    /// Base metres.
    Altitude,
    /// Base joules.
    Energy,
    /// Base grams.
    BulletMass,
    /// Bullet length; base millimetres.
    BulletLength,
    /// Bullet diameter; base millimetres.
    BulletDiameter,
    /// Barrel length; base millimetres.
    BarrelLength,
    /// Barrel length per turn of rifling; base inches.
    Twist,
}

/// A display unit: `shown = base × scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub label: &'static str,
    scale: f64,
    offset: f64,
    pub decimals: usize,
}

const fn unit(label: &'static str, scale: f64, decimals: usize) -> Unit {
    Unit {
        label,
        scale,
        offset: 0.0,
        decimals,
    }
}

const METRES_PER_YARD: f64 = 0.9144;
const FEET_PER_METRE: f64 = 3.280_84;
const MM_PER_INCH: f64 = 25.4;

impl Quantity {
    pub const ALL: [Quantity; 15] = [
        Quantity::Distance,
        Quantity::ShortLength,
        Quantity::SightHeight,
        Quantity::Speed,
        Quantity::WindSpeed,
        Quantity::Temperature,
        Quantity::Sensitivity,
        Quantity::Pressure,
        Quantity::Altitude,
        Quantity::Energy,
        Quantity::BulletMass,
        Quantity::BulletLength,
        Quantity::BulletDiameter,
        Quantity::BarrelLength,
        Quantity::Twist,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Quantity::Distance => "Distance",
            Quantity::ShortLength => "Drop, drift, target size",
            Quantity::SightHeight => "Sight height",
            Quantity::Speed => "Bullet velocity",
            Quantity::WindSpeed => "Wind and game speed",
            Quantity::Temperature => "Temperature",
            Quantity::Sensitivity => "Temperature sensitivity",
            Quantity::Pressure => "Air pressure",
            Quantity::Altitude => "Altitude",
            Quantity::Energy => "Energy",
            Quantity::BulletMass => "Bullet weight",
            Quantity::BulletLength => "Bullet length",
            Quantity::BulletDiameter => "Bullet diameter",
            Quantity::BarrelLength => "Barrel length",
            Quantity::Twist => "Twist",
        }
    }

    /// The units on offer; the first is metric, the last imperial.
    pub fn units(self) -> &'static [Unit] {
        match self {
            Quantity::Distance => &DISTANCE,
            Quantity::ShortLength | Quantity::SightHeight => &SHORT_LENGTH,
            Quantity::Speed => &SPEED,
            Quantity::WindSpeed => &WIND_SPEED,
            Quantity::Temperature => &TEMPERATURE,
            Quantity::Sensitivity => &SENSITIVITY,
            Quantity::Pressure => &PRESSURE,
            Quantity::Altitude => &ALTITUDE,
            Quantity::Energy => &ENERGY,
            Quantity::BulletMass => &BULLET_MASS,
            Quantity::BulletLength | Quantity::BulletDiameter | Quantity::BarrelLength => {
                &MILLIMETRES
            }
            Quantity::Twist => &TWIST,
        }
    }
}

const DISTANCE: [Unit; 2] = [unit("m", 1.0, 0), unit("yd", 1.0 / METRES_PER_YARD, 0)];
const SHORT_LENGTH: [Unit; 2] = [unit("cm", 1.0, 1), unit("in", 1.0 / 2.54, 2)];
const SPEED: [Unit; 2] = [unit("m/s", 1.0, 0), unit("fps", FEET_PER_METRE, 0)];
const WIND_SPEED: [Unit; 3] = [
    unit("m/s", 1.0, 1),
    unit("km/h", 3.6, 1),
    unit("mph", 2.236_94, 1),
];
const TEMPERATURE: [Unit; 2] = [
    unit("°C", 1.0, 1),
    Unit {
        label: "°F",
        scale: 1.8,
        offset: 32.0,
        decimals: 1,
    },
];
const SENSITIVITY: [Unit; 2] = [
    unit("m/s per °C", 1.0, 2),
    unit("fps per °F", FEET_PER_METRE / 1.8, 2),
];
const PRESSURE: [Unit; 3] = [
    unit("hPa", 1.0, 1),
    unit("mmHg", 0.750_062, 1),
    unit("inHg", 0.029_53, 2),
];
const ALTITUDE: [Unit; 2] = [unit("m", 1.0, 0), unit("ft", FEET_PER_METRE, 0)];
const ENERGY: [Unit; 2] = [unit("J", 1.0, 0), unit("ft·lbf", 0.737_562, 0)];
const BULLET_MASS: [Unit; 2] = [unit("g", 1.0, 2), unit("gr", 15.432_358, 1)];
const MILLIMETRES: [Unit; 2] = [unit("mm", 1.0, 2), unit("in", 1.0 / MM_PER_INCH, 3)];
const TWIST: [Unit; 2] = [unit("in", 1.0, 2), unit("mm", MM_PER_INCH, 0)];

impl Unit {
    pub fn show(&self, base: f64) -> f64 {
        base * self.scale + self.offset
    }

    pub fn base(&self, shown: f64) -> f64 {
        (shown - self.offset) / self.scale
    }
}

/// The hunter's unit choice per quantity, by unit label.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitPrefs(BTreeMap<Quantity, String>);

impl UnitPrefs {
    pub fn metric() -> Self {
        UnitPrefs::default()
    }

    pub fn imperial() -> Self {
        UnitPrefs(
            Quantity::ALL
                .into_iter()
                .map(|quantity| {
                    let units = quantity.units();
                    // Hunters using imperial units still mostly give twist in inches.
                    let choice = if quantity == Quantity::Twist {
                        units[0]
                    } else {
                        units[units.len() - 1]
                    };
                    (quantity, choice.label.to_owned())
                })
                .collect(),
        )
    }

    pub fn unit(&self, quantity: Quantity) -> Unit {
        let units = quantity.units();
        self.0
            .get(&quantity)
            .and_then(|label| units.iter().find(|unit| unit.label == label))
            .copied()
            .unwrap_or(units[0])
    }

    pub fn set(&mut self, quantity: Quantity, unit: Unit) {
        self.0.insert(quantity, unit.label.to_owned());
    }

    pub fn install(&self, context: &egui::Context) {
        context.data_mut(|data| data.insert_temp(egui::Id::new(PREFS_ID), self.clone()));
    }
}

const PREFS_ID: &str = "mxsc-unit-prefs";

pub fn prefs(context: &egui::Context) -> UnitPrefs {
    context
        .data(|data| data.get_temp::<UnitPrefs>(egui::Id::new(PREFS_ID)))
        .unwrap_or_default()
}

pub fn unit_of(ui: &egui::Ui, quantity: Quantity) -> Unit {
    prefs(ui.ctx()).unit(quantity)
}

impl UnitPrefs {
    pub fn label(&self, quantity: Quantity) -> &'static str {
        self.unit(quantity).label
    }

    /// A base value in the chosen unit, as a number.
    pub fn show(&self, quantity: Quantity, base: f64) -> f64 {
        self.unit(quantity).show(base)
    }

    /// "12.3 cm": a base value in the chosen unit, with its label.
    pub fn format(&self, quantity: Quantity, base: f64) -> String {
        let unit = self.unit(quantity);
        format!("{:.*} {}", unit.decimals, unit.show(base), unit.label)
    }

    /// Like `format`, with a sign: "+12.3 cm".
    pub fn format_signed(&self, quantity: Quantity, base: f64) -> String {
        let unit = self.unit(quantity);
        format!("{:+.*} {}", unit.decimals, unit.show(base), unit.label)
    }

    /// The number alone, for table cells under a labelled header.
    pub fn number(&self, quantity: Quantity, base: f64, signed: bool) -> String {
        let unit = self.unit(quantity);
        let shown = unit.show(base);
        if signed {
            format!("{shown:+.*}", unit.decimals)
        } else {
            format!("{shown:.*}", unit.decimals)
        }
    }
}

/// An editable value stored in the base unit and shown in the hunter's.
pub fn drag(
    ui: &mut egui::Ui,
    value: &mut f64,
    quantity: Quantity,
    range: RangeInclusive<f64>,
    speed: f64,
) -> egui::Response {
    let unit = unit_of(ui, quantity);
    let (low, high) = (unit.show(*range.start()), unit.show(*range.end()));
    let mut shown = unit.show(*value);
    let response = ui.add(
        egui::DragValue::new(&mut shown)
            .range(low.min(high)..=low.max(high))
            .speed(speed * unit.scale.abs())
            .max_decimals(unit.decimals + 1)
            .suffix(format!(" {}", unit.label)),
    );
    if response.changed() {
        *value = unit.base(shown);
    }
    response
}

/// A labelled row in a two-column grid.
pub fn quantity(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    quantity: Quantity,
    range: RangeInclusive<f64>,
    speed: f64,
) {
    ui.label(label);
    drag(ui, value, quantity, range, speed);
    ui.end_row();
}

/// A grid row for a value that may be unknown; a checkbox switches it on.
pub fn optional_quantity(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    default: f64,
    quantity: Quantity,
    range: RangeInclusive<f64>,
    speed: f64,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut known = value.is_some();
        if ui.checkbox(&mut known, "").changed() {
            *value = known.then_some(default);
        }
        match value {
            Some(number) => {
                drag(ui, number, quantity, range, speed);
            }
            None => {
                ui.weak("unknown");
            }
        }
    });
    ui.end_row();
}

/// The units menu: presets and a choice per quantity. Returns true when
/// something changed.
pub fn menu(ui: &mut egui::Ui, prefs: &mut UnitPrefs) -> bool {
    let before = prefs.clone();
    ui.horizontal(|ui| {
        if ui.button("Metric").clicked() {
            *prefs = UnitPrefs::metric();
        }
        if ui.button("Imperial").clicked() {
            *prefs = UnitPrefs::imperial();
        }
    });
    ui.separator();
    egui::Grid::new("unit-prefs")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for quantity in Quantity::ALL {
                ui.label(quantity.name());
                ui.horizontal(|ui| {
                    let current = prefs.unit(quantity);
                    for unit in quantity.units() {
                        if ui
                            .selectable_label(current.label == unit.label, unit.label)
                            .clicked()
                        {
                            prefs.set(quantity, *unit);
                        }
                    }
                });
                ui.end_row();
            }
        });
    *prefs != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip_and_hit_known_values() {
        let prefs = UnitPrefs::imperial();
        // Within 0.01 %: the expected values are rounded.
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-4 * b.abs().max(1.0);
        assert!(close(prefs.unit(Quantity::Distance).show(100.0), 109.361));
        assert!(close(prefs.unit(Quantity::Speed).show(800.0), 2624.672));
        assert!(close(prefs.unit(Quantity::Temperature).show(-40.0), -40.0));
        assert!(close(prefs.unit(Quantity::Temperature).show(15.0), 59.0));
        assert!(close(prefs.unit(Quantity::Energy).show(3000.0), 2212.69));
        assert!(close(prefs.unit(Quantity::BulletMass).show(11.664), 180.0));
        assert!(close(prefs.unit(Quantity::Pressure).show(1013.25), 29.921));
        assert_eq!(prefs.unit(Quantity::Twist).label, "in");
        for quantity in Quantity::ALL {
            for unit in quantity.units() {
                assert!(
                    close(unit.base(unit.show(123.4)), 123.4),
                    "{} {}",
                    quantity.name(),
                    unit.label
                );
            }
        }
    }

    #[test]
    fn unknown_or_missing_choices_fall_back_to_metric() {
        let mut prefs: UnitPrefs = serde_json::from_str(r#"{"Distance":"furlong"}"#).unwrap();
        assert_eq!(prefs.unit(Quantity::Distance).label, "m");
        assert_eq!(prefs.unit(Quantity::Pressure).label, "hPa");
        prefs.set(Quantity::Pressure, Quantity::Pressure.units()[1]);
        let saved = serde_json::to_string(&prefs).unwrap();
        let back: UnitPrefs = serde_json::from_str(&saved).unwrap();
        assert_eq!(back.unit(Quantity::Pressure).label, "mmHg");
    }
}

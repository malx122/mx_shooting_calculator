//! The hunter's own data: rifles, the bullet collection, and loads measured
//! in a particular rifle. Every record has a UUID and an `updated_at` time so
//! JSON import (and later, sync) can merge copies made on different devices.

use web_time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickUnit {
    Mil,
    Moa,
}

impl ClickUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            ClickUnit::Mil => "mil",
            ClickUnit::Moa => "moa",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "mil" => Some(ClickUnit::Mil),
            "moa" => Some(ClickUnit::Moa),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rifle {
    pub id: String,
    pub name: String,
    /// Key into the caliber catalog, e.g. `6-5x55-se`.
    pub caliber: String,
    pub barrel_length_mm: Option<f64>,
    /// Barrel length for one turn of the rifling, in inches.
    pub twist_in: f64,
    pub right_hand_twist: bool,
    pub sight_height_cm: f64,
    pub zero_range_m: f64,
    /// Where the zero puts the bullet at the zero range: above (positive)
    /// or below the line of sight, cm. Hunters often zero "4 cm high at
    /// 100 m"; zero for a plain zero.
    #[serde(default)]
    pub zero_height_cm: f64,
    pub zero_temperature_c: f64,
    pub zero_pressure_hpa: f64,
    /// Powder temperature at the zero; `None` means the air temperature.
    #[serde(default)]
    pub zero_powder_temperature_c: Option<f64>,
    #[serde(default = "default_zero_humidity")]
    pub zero_humidity_percent: f64,
    #[serde(default)]
    pub table: TableSettings,
    pub click_unit: ClickUnit,
    pub click_value: f64,
    /// Catalog id of the mounted scope, if any.
    #[serde(default)]
    pub scope_id: Option<String>,
    /// Catalog id of the scope's reticle.
    #[serde(default)]
    pub reticle_id: Option<String>,
    pub notes: String,
    pub updated_at: i64,
}

fn default_zero_humidity() -> f64 {
    50.0
}

impl Rifle {
    pub fn zero_weather(&self) -> ZeroWeather {
        ZeroWeather {
            temperature_c: self.zero_temperature_c,
            powder_temperature_c: self
                .zero_powder_temperature_c
                .unwrap_or(self.zero_temperature_c),
            pressure_hpa: self.zero_pressure_hpa,
            humidity_percent: self.zero_humidity_percent,
        }
    }

    pub fn new(name: impl Into<String>, caliber: impl Into<String>, twist_in: f64) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            caliber: caliber.into(),
            barrel_length_mm: None,
            twist_in,
            right_hand_twist: true,
            sight_height_cm: 4.5,
            zero_range_m: 100.0,
            zero_height_cm: 0.0,
            zero_temperature_c: 15.0,
            zero_pressure_hpa: 1013.25,
            zero_powder_temperature_c: None,
            zero_humidity_percent: default_zero_humidity(),
            table: TableSettings::default(),
            click_unit: ClickUnit::Mil,
            click_value: 0.1,
            scope_id: None,
            reticle_id: None,
            notes: String::new(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.name.trim().is_empty(), "the rifle needs a name"),
            (!self.caliber.is_empty(), "choose a caliber"),
            (self.twist_in > 0.0, "twist rate must be positive"),
            (
                self.sight_height_cm >= 0.0,
                "sight height cannot be negative",
            ),
            (self.zero_range_m > 0.0, "zero range must be positive"),
            (
                self.zero_height_cm.abs() <= 50.0,
                "the zero can be at most 50 cm high or low",
            ),
            (
                self.zero_weather().problem().is_none(),
                self.zero_weather().problem().unwrap_or_default(),
            ),
            (
                self.table.problem().is_none(),
                self.table.problem().unwrap_or_default(),
            ),
            (self.click_value > 0.0, "click value must be positive"),
            (
                self.barrel_length_mm.is_none_or(|mm| mm > 0.0),
                "barrel length must be positive",
            ),
        ];
        first_failure(&checks, &self.name)
    }
}

/// The rifle's dope table: which ranges and which columns.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableSettings {
    pub start_m: f64,
    pub end_m: f64,
    pub step_m: f64,
    pub columns: TableColumns,
}

impl Default for TableSettings {
    fn default() -> Self {
        Self {
            start_m: 0.0,
            end_m: 400.0,
            step_m: 50.0,
            columns: TableColumns::default(),
        }
    }
}

impl TableSettings {
    pub fn ranges(&self) -> impl Iterator<Item = f64> + use<> {
        let TableSettings {
            start_m,
            end_m,
            step_m,
            ..
        } = *self;
        (0..)
            .map(move |index| start_m + index as f64 * step_m)
            .take_while(move |range| *range <= end_m + 1e-9)
    }

    fn problem(&self) -> Option<&'static str> {
        if self.step_m.is_nan() || self.step_m < 1.0 {
            Some("table step must be at least 1 m")
        } else if !(self.start_m >= 0.0 && self.end_m > self.start_m && self.end_m <= 3000.0) {
            Some("table range must run from 0 m or more up to 3000 m")
        } else if (self.end_m - self.start_m) / self.step_m > 500.0 {
            Some("the table would have more than 500 rows")
        } else {
            None
        }
    }
}

/// Which dope table columns to show besides the range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableColumns {
    pub drop_length: bool,
    pub drop_mil: bool,
    pub drop_moa: bool,
    pub drop_clicks: bool,
    pub wind_length: bool,
    pub wind_mil: bool,
    pub wind_moa: bool,
    pub wind_clicks: bool,
    pub velocity: bool,
    pub energy: bool,
    pub time: bool,
}

impl Default for TableColumns {
    fn default() -> Self {
        Self {
            drop_length: true,
            drop_mil: true,
            drop_moa: false,
            drop_clicks: true,
            wind_length: true,
            wind_mil: true,
            wind_moa: false,
            wind_clicks: true,
            velocity: true,
            energy: true,
            time: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulletKind {
    /// Factory-loaded cartridge: the velocity comes from the box.
    Factory,
    /// A component bullet loaded by the hunter: the velocity is their own.
    Handload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BcModel {
    G1,
    G7,
    /// The bullet's own drag curve (`Bullet::drag_curve`) instead of a BC.
    Custom,
}

/// One point of a bullet's own drag curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragPoint {
    pub mach: f64,
    pub cd: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Construction {
    CupAndCore,
    Bonded,
    Partition,
    Monolithic,
    PolymerTip,
    PolymerTipBonded,
    PolymerTipMonolithic,
    Fmj,
    Match,
    Other,
}

impl Construction {
    /// Whether the bullet is made to expand; `None` when unknown (Other).
    pub fn expanding(self) -> Option<bool> {
        match self {
            Construction::Fmj | Construction::Match => Some(false),
            Construction::Other => None,
            _ => Some(true),
        }
    }

    pub const ALL: [Construction; 10] = [
        Construction::CupAndCore,
        Construction::Bonded,
        Construction::Partition,
        Construction::Monolithic,
        Construction::PolymerTip,
        Construction::PolymerTipBonded,
        Construction::PolymerTipMonolithic,
        Construction::Fmj,
        Construction::Match,
        Construction::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Construction::CupAndCore => "Cup and core",
            Construction::Bonded => "Bonded",
            Construction::Partition => "Partition",
            Construction::Monolithic => "Monolithic (copper)",
            Construction::PolymerTip => "Polymer tip",
            Construction::PolymerTipBonded => "Polymer tip, bonded",
            Construction::PolymerTipMonolithic => "Polymer tip, monolithic",
            Construction::Fmj => "Full metal jacket",
            Construction::Match => "Match",
            Construction::Other => "Other",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Construction::CupAndCore => "cup_and_core",
            Construction::Bonded => "bonded",
            Construction::Partition => "partition",
            Construction::Monolithic => "monolithic",
            Construction::PolymerTip => "polymer_tip",
            Construction::PolymerTipBonded => "polymer_tip_bonded",
            Construction::PolymerTipMonolithic => "polymer_tip_monolithic",
            Construction::Fmj => "fmj",
            Construction::Match => "match",
            Construction::Other => "other",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Construction::ALL
            .into_iter()
            .find(|construction| construction.as_str() == text)
    }
}

/// One BC band: the BC that applies at a velocity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BcBand {
    pub velocity_mps: f64,
    pub bc: f64,
}

/// A bullet in the hunter's own collection: either factory ammunition or a
/// handload. Only these can be chosen when calculating.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bullet {
    pub id: String,
    pub kind: BulletKind,
    pub manufacturer: String,
    pub name: String,
    /// Caliber key of the cartridge this is loaded in.
    pub caliber: String,
    pub diameter_mm: f64,
    pub mass_g: f64,
    pub length_mm: Option<f64>,
    pub bc_g1: Option<f64>,
    pub bc_g7: Option<f64>,
    /// Which of the two BCs the calculation uses.
    pub drag_model: BcModel,
    /// BCs for velocity bands under `drag_model`; when present they replace
    /// the single BC in the calculation.
    #[serde(default)]
    pub bc_bands: Vec<BcBand>,
    /// Drag coefficient by Mach number, used when `drag_model` is `Custom`.
    #[serde(default)]
    pub drag_curve: Vec<DragPoint>,
    pub muzzle_velocity_mps: f64,
    /// Temperature the muzzle velocity was measured at.
    pub velocity_temperature_c: f64,
    /// Change in muzzle velocity per °C of powder temperature, in m/s.
    pub velocity_per_degree: f64,
    pub test_barrel_mm: Option<f64>,
    pub construction: Construction,
    pub lead_free: bool,
    pub min_expansion_mps: Option<f64>,
    /// The catalog entry this was copied from, if any.
    pub catalog_id: Option<String>,
    pub notes: String,
    pub updated_at: i64,
}

impl Bullet {
    /// A blank handload for manual entry.
    pub fn new_manual(caliber: impl Into<String>, diameter_mm: f64) -> Self {
        Self {
            id: new_id(),
            kind: BulletKind::Handload,
            manufacturer: String::new(),
            name: String::new(),
            caliber: caliber.into(),
            diameter_mm,
            mass_g: 0.0,
            length_mm: None,
            bc_g1: None,
            bc_g7: None,
            drag_model: BcModel::G1,
            bc_bands: Vec::new(),
            drag_curve: Vec::new(),
            muzzle_velocity_mps: 0.0,
            velocity_temperature_c: 15.0,
            velocity_per_degree: 0.5,
            test_barrel_mm: None,
            construction: Construction::Other,
            lead_free: false,
            min_expansion_mps: None,
            catalog_id: None,
            notes: String::new(),
            updated_at: 0,
        }
    }

    /// The ballistic coefficient the calculation should use.
    pub fn ballistic_coefficient(&self) -> Option<f64> {
        match self.drag_model {
            BcModel::G1 => self.bc_g1,
            BcModel::G7 => self.bc_g7,
            BcModel::Custom => None,
        }
    }

    pub fn display_name(&self) -> String {
        let mass_gr = self.mass_g / crate::GRAMS_PER_GRAIN;
        format!(
            "{} {} {:.1} g ({:.0} gr)",
            self.manufacturer, self.name, self.mass_g, mass_gr
        )
        .trim()
        .to_owned()
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = |value: Option<f64>| value.is_none_or(|v| v > 0.0);
        let checks = [
            (!self.name.trim().is_empty(), "the bullet needs a name"),
            (!self.caliber.is_empty(), "choose a caliber"),
            (self.diameter_mm > 0.0, "diameter must be positive"),
            (self.mass_g > 0.0, "mass must be positive"),
            (positive(self.length_mm), "length must be positive"),
            (
                positive(self.bc_g1) && positive(self.bc_g7),
                "BC must be positive",
            ),
            (
                self.drag_model == BcModel::Custom || self.ballistic_coefficient().is_some(),
                "enter a BC for the selected drag model (G1 or G7)",
            ),
            (
                self.drag_model != BcModel::Custom || (2..=400).contains(&self.drag_curve.len()),
                "a custom drag curve needs 2 to 400 points",
            ),
            (
                self.drag_curve
                    .iter()
                    .all(|p| (0.0..=10.0).contains(&p.mach) && p.cd > 0.0 && p.cd < 3.0),
                "drag curve points need Mach 0–10 and a drag coefficient between 0 and 3",
            ),
            (
                {
                    let mut machs: Vec<_> = self.drag_curve.iter().map(|p| p.mach).collect();
                    machs.sort_by(f64::total_cmp);
                    machs.windows(2).all(|pair| pair[0] != pair[1])
                },
                "two drag curve points have the same Mach number",
            ),
            (self.muzzle_velocity_mps > 0.0, "enter the muzzle velocity"),
            (self.bc_bands.len() <= 5, "at most five BC bands"),
            (
                self.bc_bands
                    .iter()
                    .all(|band| band.velocity_mps > 0.0 && band.bc > 0.0),
                "BC bands need a positive velocity and BC",
            ),
            (
                {
                    let mut velocities: Vec<_> =
                        self.bc_bands.iter().map(|band| band.velocity_mps).collect();
                    velocities.sort_by(f64::total_cmp);
                    velocities.windows(2).all(|pair| pair[0] != pair[1])
                },
                "two BC bands have the same velocity",
            ),
            (
                positive(self.test_barrel_mm),
                "test barrel length must be positive",
            ),
            (
                positive(self.min_expansion_mps),
                "expansion velocity must be positive",
            ),
        ];
        first_failure(&checks, &self.name)
    }
}

/// One chronograph result: the average muzzle velocity at a temperature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VelocityPoint {
    pub temperature_c: f64,
    pub velocity_mps: f64,
    /// The string it averages, when imported from a chronograph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shots: Option<ShotStats>,
}

/// How a string of shots spread.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShotStats {
    pub count: u32,
    /// Standard deviation (of a sample: n − 1), m/s.
    pub sd_mps: f64,
    /// Extreme spread, fastest less slowest, m/s.
    pub es_mps: f64,
}

/// Muzzle velocity as a straight line in powder temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityModel {
    pub velocity_mps: f64,
    pub reference_temperature_c: f64,
    /// Change in m/s per °C.
    pub per_degree: f64,
}

impl VelocityModel {
    pub fn velocity_at(&self, temperature_c: f64) -> f64 {
        self.velocity_mps + self.per_degree * (temperature_c - self.reference_temperature_c)
    }

    /// The same sensitivity as a percentage of velocity per 15 °C, the way
    /// some apps (Strelok) state it.
    pub fn percent_per_15c(&self) -> f64 {
        self.per_degree * 15.0 / self.velocity_mps * 100.0
    }
}

/// Weather when a rifle was zeroed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZeroWeather {
    pub temperature_c: f64,
    pub powder_temperature_c: f64,
    pub pressure_hpa: f64,
    pub humidity_percent: f64,
}

impl ZeroWeather {
    fn problem(&self) -> Option<&'static str> {
        if self.pressure_hpa.is_nan() || self.pressure_hpa <= 0.0 {
            Some("zero pressure must be positive")
        } else if !(0.0..=100.0).contains(&self.humidity_percent) {
            Some("zero humidity must be 0–100 %")
        } else if !(-60.0..=70.0).contains(&self.temperature_c)
            || !(-60.0..=70.0).contains(&self.powder_temperature_c)
        {
            Some("zero temperature is out of range")
        } else {
            None
        }
    }
}

/// Which weather a load's zero was made in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum LoadZero {
    /// The rifle's zero weather.
    #[default]
    Rifle,
    /// The same weather as the shot, so the weather needs no correction.
    SameAsShot,
    Recorded(ZeroWeather),
}

/// What truing adjusts to make the calculation match a real shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruingMethod {
    /// Scale the muzzle velocity: for loads without a chronograph.
    Velocity,
    /// Scale the bullet's drag (its BC the other way): for loads whose
    /// velocity was measured.
    Drag,
}

/// A long shot that showed where the calculation was off, and the factor
/// found to make it match.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Truing {
    pub method: TruingMethod,
    pub range_m: f64,
    /// What it took to hit centre: elevation in mrad, up positive.
    pub elevation_mrad: f64,
    /// Shot angle then, degrees.
    #[serde(default)]
    pub inclination_degrees: f64,
    /// The weather then.
    pub weather: ZeroWeather,
    /// The velocity or drag multiplier that makes the calculation match.
    pub factor: f64,
}

impl Truing {
    fn problem(&self) -> Option<&'static str> {
        if !(50.0..=3000.0).contains(&self.range_m) {
            Some("the truing range must be 50–3000 m")
        } else if !(0.5..=2.0).contains(&self.factor) {
            Some("the truing factor is out of range")
        } else {
            self.weather.problem()
        }
    }
}

/// A bullet as shot from one particular rifle: chronographed velocities,
/// where it hits relative to the rifle's zero, and the weather it was zeroed
/// in. Everything here overrides the bullet's general data for this rifle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RifleLoad {
    pub id: String,
    pub rifle_id: String,
    pub bullet_id: String,
    /// Measured velocities; empty means the bullet's own velocity is used.
    #[serde(default)]
    pub velocities: Vec<VelocityPoint>,
    /// Where this load hits relative to the rifle's zero, as an angle in
    /// mrad: [right, up].
    #[serde(default)]
    pub impact_offset_mrad: [f64; 2],
    #[serde(default)]
    pub zero: LoadZero,
    /// A long shot the calculation was trued to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truing: Option<Truing>,
    pub notes: String,
    pub updated_at: i64,
    // A single measured velocity, as exports before format 2 stored it.
    #[serde(default, skip_serializing)]
    muzzle_velocity_mps: Option<f64>,
    #[serde(default, skip_serializing)]
    velocity_temperature_c: Option<f64>,
}

impl RifleLoad {
    pub fn new(rifle_id: &str, bullet_id: &str) -> Self {
        Self {
            id: new_id(),
            rifle_id: rifle_id.to_owned(),
            bullet_id: bullet_id.to_owned(),
            velocities: Vec::new(),
            impact_offset_mrad: [0.0, 0.0],
            zero: LoadZero::Rifle,
            truing: None,
            notes: String::new(),
            updated_at: 0,
            muzzle_velocity_mps: None,
            velocity_temperature_c: None,
        }
    }

    /// Moves a velocity from an old export into the measurement list.
    pub(crate) fn upgrade_legacy(&mut self) {
        if let Some(velocity_mps) = self.muzzle_velocity_mps.take() {
            let temperature_c = self.velocity_temperature_c.take().unwrap_or(15.0);
            if self.velocities.is_empty() {
                self.velocities.push(VelocityPoint {
                    temperature_c,
                    velocity_mps,
                    shots: None,
                });
            }
        }
        self.velocity_temperature_c = None;
    }

    /// Whether this holds anything beyond the bullet's own data.
    pub fn is_empty(&self) -> bool {
        self.velocities.is_empty()
            && self.impact_offset_mrad == [0.0, 0.0]
            && self.zero == LoadZero::Rifle
            && self.truing.is_none()
    }

    /// Velocity against powder temperature. Two or more measurements at
    /// different temperatures give a fitted line (least squares); one
    /// measurement keeps the bullet's sensitivity; none uses the bullet.
    pub fn velocity_model(&self, bullet: &Bullet) -> VelocityModel {
        let points = &self.velocities;
        let from_bullet = VelocityModel {
            velocity_mps: bullet.muzzle_velocity_mps,
            reference_temperature_c: bullet.velocity_temperature_c,
            per_degree: bullet.velocity_per_degree,
        };
        if points.is_empty() {
            return from_bullet;
        }
        let count = points.len() as f64;
        let mean_temperature = points.iter().map(|p| p.temperature_c).sum::<f64>() / count;
        let mean_velocity = points.iter().map(|p| p.velocity_mps).sum::<f64>() / count;
        let spread: f64 = points
            .iter()
            .map(|p| (p.temperature_c - mean_temperature).powi(2))
            .sum();
        let per_degree = if spread > 1e-9 {
            points
                .iter()
                .map(|p| (p.temperature_c - mean_temperature) * (p.velocity_mps - mean_velocity))
                .sum::<f64>()
                / spread
        } else {
            bullet.velocity_per_degree
        };
        VelocityModel {
            velocity_mps: mean_velocity,
            reference_temperature_c: mean_temperature,
            per_degree,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let zero_problem = match &self.zero {
            LoadZero::Recorded(weather) => weather.problem(),
            _ => None,
        };
        let truing_problem = self.truing.as_ref().and_then(Truing::problem);
        let checks = [
            (
                self.velocities.len() <= 10,
                "at most ten velocity measurements",
            ),
            (
                self.velocities
                    .iter()
                    .all(|p| p.velocity_mps > 0.0 && (-60.0..=70.0).contains(&p.temperature_c)),
                "measured velocities must be positive, at -60…70 °C",
            ),
            (
                self.impact_offset_mrad
                    .iter()
                    .all(|offset| offset.abs() < 50.0),
                "impact offset must be under 50 mrad",
            ),
            (zero_problem.is_none(), zero_problem.unwrap_or_default()),
            (truing_problem.is_none(), truing_problem.unwrap_or_default()),
        ];
        first_failure(&checks, "rifle load")
    }
}

fn first_failure(checks: &[(bool, &str)], record: &str) -> Result<(), String> {
    match checks.iter().find(|(ok, _)| !ok) {
        Some((_, problem)) if record.trim().is_empty() => Err((*problem).to_owned()),
        Some((_, problem)) => Err(format!("{}: {problem}", record.trim())),
        None => Ok(()),
    }
}

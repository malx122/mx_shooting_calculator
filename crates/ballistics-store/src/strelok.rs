//! Import of Strelok Pro backups (`.srl`).
//!
//! A backup is read into a `StrelokPlan`: the rifles, bullets, rifle loads
//! and scopes it becomes, how sure the importer is about each rifle's
//! caliber, and warnings. The plan's records go through the ordinary merge
//! (`Store::import_data`), so importing it can be previewed and repeated.
//!
//! Every record gets an id derived from what it is, so the same backup
//! always produces the same ids:
//!
//! - a rifle by its Strelok name;
//! - a bullet by caliber, maker, product and weight (the same cartridge in
//!   two Strelok rifles becomes one bullet);
//! - a load by rifle and bullet;
//! - each rifle's scope by the rifle.
//!
//! Re-importing therefore finds the records it made before instead of
//! adding copies, and leaves alone those the hunter has edited since.
//!
//! See `docs/strelok-import.md` for the file format.

use std::collections::{HashMap, HashSet};

use roxmltree::{Document, Node};
use uuid::Uuid;

use crate::catalog::{Caliber, Catalog};
use crate::model::{
    BcBand, BcModel, Bullet, BulletKind, ClickUnit, Construction, LoadZero, Rifle, RifleLoad,
    TableColumns, TableSettings, VelocityPoint, ZeroWeather,
};
use crate::optics::{AngleUnit, FocalPlane, PLAIN_CROSSHAIR, UserScope};
use crate::transfer::{FORMAT, PersonalData, VERSION};
use crate::{GRAMS_PER_GRAIN, Store, StoreError};

const MRAD_PER_MOA: f64 = 0.290_888_2;
const HPA_PER_MMHG: f64 = 1.333_224;
const MM_PER_INCH: f64 = 25.4;
/// Bullet diameters in a backup are typed by hand; this much off still fits.
const DIAMETER_TOLERANCE_MM: f64 = 0.15;
const IMPORT_NOTE: &str = "Imported from Strelok Pro.";

/// Fixed namespace for ids derived from Strelok data.
const NAMESPACE: Uuid = Uuid::from_u128(0x5e7c_1a2b_0000_4d2e_9c51_57e1_0c00_0001);

fn derived_id(kind: &str, key: &str) -> String {
    Uuid::new_v5(&NAMESPACE, format!("strelok/{kind}/{key}").as_bytes()).to_string()
}

#[derive(Clone, Debug, Default)]
pub struct PlanOptions {
    /// Caliber keys chosen by the hunter, by Strelok rifle name.
    pub caliber_overrides: HashMap<String, String>,
    /// Timestamp for every imported record: the backup file's modification
    /// time, so a newer backup wins over records not edited since.
    pub updated_at: i64,
}

/// How the importer arrived at a rifle's caliber.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaliberSource {
    /// Chosen by the hunter for this import.
    Chosen,
    /// The rifle is already in the database; its caliber is kept.
    Existing,
    /// Found in the rifle or cartridge names, and fits the bullet diameter.
    Names,
    /// Only one caliber fits the bullet diameter.
    Diameter,
    /// A guess among several calibers: check it.
    Guess,
}

impl CaliberSource {
    pub fn is_certain(self) -> bool {
        !matches!(self, CaliberSource::Guess)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedRifle {
    pub strelok_name: String,
    pub rifle_id: String,
    pub caliber: String,
    pub caliber_source: CaliberSource,
    pub bullets: usize,
}

#[derive(Clone, Debug)]
pub struct StrelokPlan {
    pub data: PersonalData,
    pub rifles: Vec<PlannedRifle>,
    pub warnings: Vec<String>,
    /// The rifle and bullet selected in Strelok when the backup was made.
    pub selected: Option<(String, Option<String>)>,
}

impl Store {
    /// Reads a Strelok backup into a plan, keeping the calibers of rifles
    /// imported before.
    pub fn plan_strelok(
        &self,
        xml: &str,
        options: &PlanOptions,
    ) -> Result<StrelokPlan, StoreError> {
        plan(xml, options, |id| {
            self.rifle(id).ok().flatten().map(|rifle| rifle.caliber)
        })
    }
}

pub fn plan(
    xml: &str,
    options: &PlanOptions,
    existing_caliber: impl Fn(&str) -> Option<String>,
) -> Result<StrelokPlan, StoreError> {
    let backup = parse(xml)?;
    let catalog = Catalog::builtin();
    let t = options.updated_at;
    let mut data = PersonalData {
        format: FORMAT.to_owned(),
        version: VERSION,
        exported_at: t,
        ..Default::default()
    };
    let mut planned = Vec::new();
    let mut warnings = Vec::new();
    let mut bullet_ids: HashSet<String> = HashSet::new();
    let mut load_ids: HashSet<String> = HashSet::new();
    let mut selected = None;

    for (index, source) in backup.rifles.iter().enumerate() {
        let name = source.name.trim();
        if name.is_empty() {
            warnings.push(format!("Rifle {} has no name and was skipped.", index + 1));
            continue;
        }
        let rifle_id = derived_id("rifle", &name.to_lowercase());
        // Bullet ids use the caliber inferred from the file alone, so they
        // stay the same when the hunter later corrects the rifle's caliber.
        let inferred = infer_caliber(source, catalog);
        let (caliber, caliber_source) = match options
            .caliber_overrides
            .get(name)
            .and_then(|key| catalog.caliber(key))
        {
            Some(caliber) => (caliber, CaliberSource::Chosen),
            None => match existing_caliber(&rifle_id)
                .as_deref()
                .and_then(|key| catalog.caliber(key))
            {
                Some(caliber) => (caliber, CaliberSource::Existing),
                None => inferred,
            },
        };
        if caliber_source == CaliberSource::Guess {
            warnings.push(format!(
                "{name}: the caliber is a guess ({}); please check it.",
                caliber.name
            ));
        }

        let scope = scope_for(source, name, t);
        let mut rifle = rifle_for(source, &rifle_id, name, caliber, t, &mut warnings);
        rifle.scope_id = Some(scope.id.clone());
        rifle.reticle_id = Some(PLAIN_CROSSHAIR.to_owned());
        if let Err(problem) = rifle.validate() {
            warnings.push(format!("{name} was skipped: {problem}"));
            continue;
        }
        if let Err(problem) = scope.validate() {
            warnings.push(format!("{name}: the scope was left out: {problem}"));
            rifle.scope_id = None;
            rifle.reticle_id = None;
        } else {
            data.scopes.push(scope);
        }

        let mut imported_here = 0;
        let mut current_bullet = None;
        for (cartridge_index, cartridge) in source.cartridges.iter().enumerate() {
            let Some((bullet, load)) = bullet_and_load(
                cartridge,
                name,
                &rifle,
                caliber,
                inferred.0,
                t,
                &mut warnings,
            ) else {
                continue;
            };
            if !load_ids.insert(load.id.clone()) {
                warnings.push(format!(
                    "{name}: \"{}\" appears twice in Strelok; the second copy was skipped.",
                    cartridge.name.trim()
                ));
                continue;
            }
            if cartridge_index == source.current_cartridge {
                current_bullet = Some(bullet.id.clone());
            }
            if bullet_ids.insert(bullet.id.clone()) {
                data.bullets.push(bullet);
            }
            data.rifle_loads.push(load);
            imported_here += 1;
        }
        if index == backup.current_rifle {
            selected = Some((rifle_id.clone(), current_bullet));
        }
        planned.push(PlannedRifle {
            strelok_name: name.to_owned(),
            rifle_id,
            caliber: caliber.key.clone(),
            caliber_source,
            bullets: imported_here,
        });
        data.rifles.push(rifle);
    }
    Ok(StrelokPlan {
        data,
        rifles: planned,
        warnings,
        selected,
    })
}

// The backup, as read from the XML ----------------------------------------

#[derive(Debug, Default)]
struct Backup {
    current_rifle: usize,
    rifles: Vec<StrelokRifle>,
}

#[derive(Debug, Default)]
struct StrelokRifle {
    name: String,
    zero_distance_m: f64,
    scope_height_cm: f64,
    click_moa: f64,
    /// 1 shows clicks as mrad / cm per 100 m; 0 as MOA.
    click_units: i64,
    reticle: String,
    min_magnification: f64,
    max_magnification: f64,
    true_magnification: f64,
    first_focal: bool,
    twist_in: f64,
    twist_left: bool,
    table_start_m: f64,
    table_end_m: f64,
    table_step_m: f64,
    columns: TableColumns,
    cartridges: Vec<StrelokCartridge>,
    current_cartridge: usize,
    notes: String,
}

#[derive(Debug, Default)]
struct StrelokCartridge {
    name: String,
    weight_gr: f64,
    velocity: f64,
    velocity_temperature_c: f64,
    /// (BC, velocity it applies at; 0 for a single BC).
    bcs: Vec<(f64, f64)>,
    /// Velocity change in percent per 15 °C.
    temperature_modifier: f64,
    drag_function: String,
    drag_category: i64,
    length_in: f64,
    diameter_in: f64,
    shift_up_moa: f64,
    shift_right_moa: f64,
    /// (temperature, velocity) measurements.
    temperature_table: Vec<(f64, f64)>,
    zero_temperature_c: f64,
    zero_powder_temperature_c: f64,
    zero_pressure_mmhg: f64,
    zero_humidity: f64,
    zero_same_as_shot: bool,
    notes: String,
}

/// Strelok writes `&` unescaped in names ("Sellier & Bellot"), which makes
/// the file invalid XML. Escape every `&` that does not start an entity.
fn repair_ampersands(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    for (index, c) in xml.char_indices() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let rest = &xml[index + 1..];
        let entity = rest
            .find(';')
            .filter(|&end| end <= 10)
            .map(|end| &rest[..end]);
        let is_entity = entity.is_some_and(|name| {
            matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
                || name.strip_prefix("#x").is_some_and(|hex| {
                    !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
                })
                || name.strip_prefix('#').is_some_and(|digits| {
                    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
                })
        });
        out.push_str(if is_entity { "&" } else { "&amp;" });
    }
    out
}

fn child<'a>(node: Node<'a, 'a>, tag: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|child| child.has_tag_name(tag))
}

fn text(node: Node<'_, '_>, tag: &str) -> String {
    let value = child(node, tag)
        .and_then(|child| child.text())
        .unwrap_or("")
        .trim();
    if value == "null" {
        String::new()
    } else {
        value.to_owned()
    }
}

fn number(node: Node<'_, '_>, tag: &str) -> f64 {
    text(node, tag).replace(',', ".").parse().unwrap_or(0.0)
}

fn flag(node: Node<'_, '_>, tag: &str) -> bool {
    text(node, tag).eq_ignore_ascii_case("true")
}

fn parse(xml: &str) -> Result<Backup, StoreError> {
    let repaired = repair_ampersands(xml);
    let document = Document::parse(&repaired).map_err(|error| {
        StoreError::Invalid(format!("this is not a readable Strelok backup: {error}"))
    })?;
    let root = document.root_element();
    if !root.has_tag_name("StrelokPro") {
        return Err(StoreError::Invalid(
            "this is not a Strelok Pro backup".into(),
        ));
    }
    if !flag(root, "Metric_units_on") {
        return Err(StoreError::Invalid(
            "only Strelok backups made with metric units can be imported: switch Strelok to metric and back up again"
                .into(),
        ));
    }
    let rifles = root
        .children()
        .filter(|node| node.has_tag_name("Rifle"))
        .map(|rifle| StrelokRifle {
            name: text(rifle, "RifleName"),
            zero_distance_m: number(rifle, "ZeroDistance"),
            scope_height_cm: number(rifle, "ScopeHeight"),
            click_moa: number(rifle, "ScopeClickVert"),
            click_units: number(rifle, "click_units") as i64,
            reticle: text(rifle, "Reticle"),
            min_magnification: number(rifle, "min_magnification"),
            max_magnification: number(rifle, "max_magnification"),
            true_magnification: number(rifle, "true_magnification"),
            first_focal: flag(rifle, "first_focal"),
            twist_in: number(rifle, "TwistRate"),
            twist_left: flag(rifle, "TwistLeft"),
            table_start_m: number(rifle, "m_StartDistance"),
            table_end_m: number(rifle, "m_EndDistance"),
            table_step_m: number(rifle, "m_StepDistance"),
            columns: TableColumns {
                drop_length: flag(rifle, "m_show_path_cm"),
                drop_mil: flag(rifle, "m_show_path_td"),
                drop_moa: flag(rifle, "m_show_path_moa"),
                drop_clicks: flag(rifle, "m_show_path_click"),
                wind_length: flag(rifle, "m_show_wind_cm"),
                wind_mil: flag(rifle, "m_show_wind_td"),
                wind_moa: flag(rifle, "m_show_wind_moa"),
                wind_clicks: flag(rifle, "m_show_wind_click"),
                velocity: flag(rifle, "m_show_speed"),
                energy: flag(rifle, "m_show_energy"),
                time: flag(rifle, "m_show_time"),
            },
            cartridges: rifle
                .children()
                .filter(|node| node.has_tag_name("Cartridge"))
                .map(parse_cartridge)
                .collect(),
            current_cartridge: number(rifle, "CurrentCartridge") as usize,
            notes: text(rifle, "RifleNotes"),
        })
        .collect();
    Ok(Backup {
        current_rifle: number(root, "CurrentRifle") as usize,
        rifles,
    })
}

fn parse_cartridge(cartridge: Node<'_, '_>) -> StrelokCartridge {
    let bc_tags = [
        ("BulletBC", "BulletBCSpeed"),
        ("BulletBC2", "BulletBC2Speed"),
        ("BulletBC3", "BulletBC3Speed"),
        ("BulletBC4", "BulletBC4Speed"),
        ("BulletBC5", "BulletBC5Speed"),
    ];
    let temperature_table = child(cartridge, "TermoSensitivity")
        .map(|table| {
            table
                .children()
                .filter(|row| row.has_tag_name("TermoRow"))
                .map(|row| (number(row, "TS_Temperature"), number(row, "TS_Speed")))
                .collect()
        })
        .unwrap_or_default();
    StrelokCartridge {
        name: text(cartridge, "CartridgeName"),
        weight_gr: number(cartridge, "BulletWeight_gr"),
        velocity: number(cartridge, "BulletSpeed"),
        velocity_temperature_c: number(cartridge, "BulletTemperature"),
        bcs: bc_tags
            .iter()
            .map(|(bc, speed)| (number(cartridge, bc), number(cartridge, speed)))
            .collect(),
        temperature_modifier: number(cartridge, "TempModifyer"),
        drag_function: text(cartridge, "DragFunctionName"),
        drag_category: number(cartridge, "DragFunctionCategory") as i64,
        length_in: number(cartridge, "bullet_length_inch"),
        diameter_in: number(cartridge, "bullet_diam_inch"),
        shift_up_moa: number(cartridge, "ShiftVerticalMOA"),
        shift_right_moa: number(cartridge, "ShiftHorizontalMOA"),
        temperature_table,
        zero_temperature_c: number(cartridge, "ZeroTemperature"),
        zero_powder_temperature_c: number(cartridge, "ZeroPowderTemperature"),
        zero_pressure_mmhg: number(cartridge, "ZeroPressure"),
        zero_humidity: number(cartridge, "ZeroHumidity"),
        zero_same_as_shot: flag(cartridge, "same_atm"),
        notes: text(cartridge, "CartridgeNotes"),
    }
}

// Calibers ----------------------------------------------------------------

/// Lower case without spaces, dots, dashes or commas; × becomes x.
fn squash(text: &str) -> String {
    text.to_lowercase()
        .replace('×', "x")
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '.' | ',' | '-' | '_' | '/'))
        .collect()
}

/// Text that names a caliber: its name, aliases, and the leading figures of
/// its name ("308" for .308 Win) when they are long enough to mean something.
fn caliber_tokens(caliber: &Caliber) -> Vec<String> {
    let mut tokens: Vec<String> = std::iter::once(&caliber.name)
        .chain(&caliber.aliases)
        .map(|name| squash(name))
        .collect();
    let squashed = squash(&caliber.name);
    let figures: String = squashed
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == 'x')
        .collect();
    if figures.len() >= 3 && figures.chars().any(|c| c.is_ascii_digit()) {
        tokens.push(figures);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Names without parenthesised item numbers, whose digits could look like a
/// caliber.
fn without_parentheses(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn is_sample(cartridge: &StrelokCartridge) -> bool {
    cartridge
        .name
        .trim_start()
        .to_lowercase()
        .starts_with("sample.")
}

/// Picks the caliber from the bullet diameter and the names: the rifle's
/// name counts most, cartridge names less, longer matches more.
fn infer_caliber<'a>(rifle: &StrelokRifle, catalog: &'a Catalog) -> (&'a Caliber, CaliberSource) {
    let cartridges: Vec<&StrelokCartridge> =
        rifle.cartridges.iter().filter(|c| !is_sample(c)).collect();
    let mut diameters: Vec<f64> = cartridges
        .iter()
        .map(|c| c.diameter_in * MM_PER_INCH)
        .filter(|d| *d > 1.0)
        .collect();
    diameters.sort_by(f64::total_cmp);
    let diameter = diameters.get(diameters.len() / 2).copied();
    let candidates: Vec<&Caliber> = catalog
        .calibers
        .iter()
        .filter(|caliber| {
            diameter.is_none_or(|d| (caliber.bullet_diameter_mm - d).abs() <= DIAMETER_TOLERANCE_MM)
        })
        .collect();
    let rifle_text = squash(&rifle.name);
    let cartridge_texts: Vec<String> = cartridges
        .iter()
        .map(|c| squash(&without_parentheses(&c.name)))
        .collect();
    let score = |caliber: &Caliber| -> usize {
        caliber_tokens(caliber)
            .iter()
            .map(|token| {
                let weight = token.len() * token.len();
                let in_rifle = if rifle_text.contains(token.as_str()) {
                    3 * weight
                } else {
                    0
                };
                let in_cartridges = cartridge_texts
                    .iter()
                    .filter(|text| text.contains(token.as_str()))
                    .count()
                    * weight;
                in_rifle + in_cartridges
            })
            .sum()
    };
    let mut scored: Vec<(usize, &Caliber)> = candidates
        .iter()
        .map(|caliber| (score(caliber), *caliber))
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    match scored.as_slice() {
        [(best, caliber), rest @ ..]
            if *best > 0 && rest.first().is_none_or(|(next, _)| next < best) =>
        {
            (caliber, CaliberSource::Names)
        }
        [(_, caliber)] => (caliber, CaliberSource::Diameter),
        [(_, caliber), ..] => (caliber, CaliberSource::Guess),
        [] => {
            // Nothing fits the diameter: take the closest one.
            let closest = catalog
                .calibers
                .iter()
                .min_by(|a, b| {
                    let d = diameter.unwrap_or(7.82);
                    (a.bullet_diameter_mm - d)
                        .abs()
                        .total_cmp(&(b.bullet_diameter_mm - d).abs())
                })
                .expect("the catalog has calibers");
            (closest, CaliberSource::Guess)
        }
    }
}

// Records -----------------------------------------------------------------

fn scope_for(source: &StrelokRifle, rifle_name: &str, updated_at: i64) -> UserScope {
    let (low, high) = {
        let (a, b) = (
            source.min_magnification.max(1.0),
            source.max_magnification.max(1.0),
        );
        if a <= b { (a, b) } else { (b, a) }
    };
    let focal_plane = if source.first_focal {
        FocalPlane::Ffp
    } else {
        FocalPlane::Sfp
    };
    let reference = if (low..=high).contains(&source.true_magnification) {
        source.true_magnification
    } else {
        high
    };
    let (click_unit, click_value) = match source.click_units {
        1 => (AngleUnit::Mrad, round(source.click_moa * MRAD_PER_MOA, 4)),
        _ => (AngleUnit::Moa, round(source.click_moa, 4)),
    };
    let reticle_note = if source.reticle.is_empty() || source.reticle == "0" {
        String::new()
    } else {
        format!(" Reticle #{} in Strelok's library.", source.reticle)
    };
    UserScope {
        id: derived_id("scope", &rifle_name.to_lowercase()),
        manufacturer: "Strelok".to_owned(),
        model: format!("{low}–{high}× ({rifle_name})"),
        magnification_min: low,
        magnification_max: high,
        objective_mm: None,
        focal_plane,
        reticle_reference_magnification: (focal_plane == FocalPlane::Sfp).then_some(reference),
        click_unit,
        click_value: if click_value > 0.0 { click_value } else { 0.1 },
        elevation_travel: None,
        fov_m_at_100m: None,
        illuminated: false,
        reticle_id: None,
        notes: format!("{IMPORT_NOTE}{reticle_note}"),
        updated_at,
    }
}

fn rifle_for(
    source: &StrelokRifle,
    id: &str,
    name: &str,
    caliber: &Caliber,
    updated_at: i64,
    warnings: &mut Vec<String>,
) -> Rifle {
    let mut rifle = Rifle::new(name, &caliber.key, caliber.typical_twist_in);
    rifle.id = id.to_owned();
    if source.twist_in > 0.0 {
        rifle.twist_in = source.twist_in;
    } else {
        warnings.push(format!(
            "{name}: no twist rate in the backup; {} in assumed.",
            caliber.typical_twist_in
        ));
    }
    rifle.right_hand_twist = !source.twist_left;
    rifle.sight_height_cm = source.scope_height_cm.max(0.0);
    if source.zero_distance_m > 0.0 {
        rifle.zero_range_m = source.zero_distance_m;
    }
    if source.click_moa > 0.0 {
        (rifle.click_unit, rifle.click_value) = match source.click_units {
            1 => (ClickUnit::Mil, round(source.click_moa * MRAD_PER_MOA, 4)),
            _ => (ClickUnit::Moa, round(source.click_moa, 4)),
        };
    }
    // Strelok keeps zero weather per cartridge; the rifle takes the one of
    // the cartridge that was selected.
    if let Some(zero) = source
        .cartridges
        .get(source.current_cartridge)
        .or(source.cartridges.first())
        .and_then(zero_weather)
    {
        rifle.zero_temperature_c = zero.temperature_c;
        rifle.zero_powder_temperature_c =
            Some(zero.powder_temperature_c).filter(|p| *p != zero.temperature_c);
        rifle.zero_pressure_hpa = zero.pressure_hpa;
        rifle.zero_humidity_percent = zero.humidity_percent;
    }
    let table = TableSettings {
        start_m: source.table_start_m.max(0.0),
        end_m: source.table_end_m,
        step_m: source.table_step_m,
        columns: source.columns,
    };
    let mut probe = rifle.clone();
    probe.table = table;
    if probe.validate().is_ok() {
        rifle.table = table;
    } else {
        warnings.push(format!(
            "{name}: Strelok's table {}–{} m every {} m can't be used; the default table is set.",
            source.table_start_m, source.table_end_m, source.table_step_m
        ));
    }
    rifle.notes = join_notes(&[&source.notes, IMPORT_NOTE]);
    rifle.updated_at = updated_at;
    rifle
}

fn zero_weather(cartridge: &StrelokCartridge) -> Option<ZeroWeather> {
    (cartridge.zero_pressure_mmhg > 0.0).then(|| ZeroWeather {
        temperature_c: cartridge.zero_temperature_c,
        powder_temperature_c: cartridge.zero_powder_temperature_c,
        pressure_hpa: round(cartridge.zero_pressure_mmhg * HPA_PER_MMHG, 2),
        humidity_percent: cartridge.zero_humidity.clamp(0.0, 100.0),
    })
}

/// The bullet for the collection and its load in this rifle, or `None`
/// (with a warning) for entries that can't or shouldn't be imported.
fn bullet_and_load(
    cartridge: &StrelokCartridge,
    rifle_name: &str,
    rifle: &Rifle,
    caliber: &Caliber,
    key_caliber: &Caliber,
    updated_at: i64,
    warnings: &mut Vec<String>,
) -> Option<(Bullet, RifleLoad)> {
    let label = cartridge.name.trim();
    if is_sample(cartridge) {
        return None;
    }
    let skip = |warnings: &mut Vec<String>, why: &str| {
        warnings.push(format!("{rifle_name}: \"{label}\" was skipped: {why}."));
    };
    let drag_model = match (
        cartridge.drag_function.to_uppercase().as_str(),
        cartridge.drag_category,
    ) {
        ("G1", 0) => BcModel::G1,
        ("G7", 0) => BcModel::G7,
        (function, 0) => {
            skip(
                warnings,
                &format!("drag function {function} is not supported"),
            );
            return None;
        }
        _ => {
            skip(
                warnings,
                "it uses a custom drag curve, which this importer can't read yet",
            );
            return None;
        }
    };
    if cartridge.velocity <= 0.0 || cartridge.weight_gr <= 0.0 {
        skip(warnings, "no velocity or weight");
        return None;
    }
    let first_bc = cartridge.bcs.first().map_or(0.0, |(bc, _)| *bc);
    if first_bc <= 0.0 {
        skip(warnings, "no ballistic coefficient");
        return None;
    }

    let name = CartridgeName::parse(label);
    let weight_key = format!("{:.1}", cartridge.weight_gr);
    // The item number tells apart products that share a name ("JHP").
    let item_key = name.item.as_deref().unwrap_or("").to_lowercase();
    let bullet_key = [
        key_caliber.key.as_str(),
        &name.manufacturer.to_lowercase(),
        &name.product.to_lowercase(),
        &item_key,
        &weight_key,
    ]
    .join("|");
    let mut bullet = Bullet::new_manual(&caliber.key, caliber.bullet_diameter_mm);
    bullet.id = derived_id("bullet", &bullet_key);
    bullet.kind = BulletKind::Factory;
    bullet.manufacturer = name.manufacturer.clone();
    // Keep the item number visible, as Strelok shows it: it tells apart
    // products that share a name.
    bullet.name = match &name.item {
        Some(item) => format!("{} ({item})", name.product),
        None => name.product.clone(),
    };
    if cartridge.diameter_in > 0.0 {
        bullet.diameter_mm = round(cartridge.diameter_in * MM_PER_INCH, 3);
    }
    bullet.mass_g = round(cartridge.weight_gr * GRAMS_PER_GRAIN, 3);
    bullet.length_mm =
        (cartridge.length_in > 0.0).then(|| round(cartridge.length_in * MM_PER_INCH, 2));
    bullet.drag_model = drag_model;
    match drag_model {
        BcModel::G7 => bullet.bc_g7 = Some(first_bc),
        _ => bullet.bc_g1 = Some(first_bc),
    }
    let mut bands: Vec<BcBand> = cartridge
        .bcs
        .iter()
        .filter(|(bc, velocity)| *bc > 0.0 && *velocity > 0.0)
        .map(|&(bc, velocity_mps)| BcBand { velocity_mps, bc })
        .collect();
    bands.sort_by(|a, b| a.velocity_mps.total_cmp(&b.velocity_mps));
    bands.dedup_by(|a, b| a.velocity_mps == b.velocity_mps);
    bullet.bc_bands = bands;
    bullet.muzzle_velocity_mps = cartridge.velocity;
    bullet.velocity_temperature_c = cartridge.velocity_temperature_c;
    bullet.velocity_per_degree = round(
        cartridge.velocity * cartridge.temperature_modifier / 100.0 / 15.0,
        4,
    );
    bullet.construction = Construction::Other;
    bullet.notes = join_notes(&[&cartridge.notes, IMPORT_NOTE]);
    bullet.updated_at = updated_at;
    if let Err(problem) = bullet.validate() {
        skip(warnings, &problem);
        return None;
    }

    let mut load = RifleLoad::new(&rifle.id, &bullet.id);
    load.id = derived_id(
        "load",
        &format!("{}|{bullet_key}", rifle.name.to_lowercase()),
    );
    let mut seen = HashSet::new();
    load.velocities = cartridge
        .temperature_table
        .iter()
        .filter(|(_, velocity)| *velocity > 0.0)
        .filter(|(temperature, _)| seen.insert(temperature.to_bits()))
        .map(|&(temperature_c, velocity_mps)| VelocityPoint {
            temperature_c,
            velocity_mps,
        })
        .collect();
    if load.velocities.is_empty() {
        load.velocities.push(VelocityPoint {
            temperature_c: cartridge.velocity_temperature_c,
            velocity_mps: cartridge.velocity,
        });
    }
    // Strelok stores the shift in MOA; positive is taken as up and right.
    load.impact_offset_mrad = [
        round(cartridge.shift_right_moa * MRAD_PER_MOA, 4),
        round(cartridge.shift_up_moa * MRAD_PER_MOA, 4),
    ];
    load.zero = match (cartridge.zero_same_as_shot, zero_weather(cartridge)) {
        (true, _) => LoadZero::SameAsShot,
        (false, Some(weather)) => LoadZero::Recorded(weather),
        (false, None) => LoadZero::Rifle,
    };
    load.notes = IMPORT_NOTE.to_owned();
    load.updated_at = updated_at;
    if let Err(problem) = load.validate() {
        skip(warnings, &problem);
        return None;
    }
    Some((bullet, load))
}

/// A Strelok cartridge name split into its parts. Strelok writes names like
/// "150gr. VOR-TX (21540), 308 Win, (Barnes)" or "168gr. VOR-TX (21541), ,
/// Barnes"; hand-entered ones are free text ("Lapua Naturalis 50 gr").
#[derive(Debug, PartialEq)]
struct CartridgeName {
    manufacturer: String,
    product: String,
    item: Option<String>,
}

const KNOWN_MAKERS: &[&str] = &[
    "Sellier & Bellot",
    "Prvi Partizan",
    "Barnes",
    "CCI",
    "Federal",
    "Fiocchi",
    "Geco",
    "GGG",
    "Hornady",
    "Lapua",
    "Norma",
    "Nosler",
    "PMP",
    "Remington",
    "RWS",
    "Sako",
    "Sierra",
    "Winchester",
];

impl CartridgeName {
    fn parse(raw: &str) -> CartridgeName {
        let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
        let (product, mut manufacturer) = match parts.as_slice() {
            [product] => (product.to_string(), String::new()),
            [product, maker] => (product.to_string(), maker.to_string()),
            [product, .., maker] => (product.to_string(), maker.to_string()),
            [] => (String::new(), String::new()),
        };
        manufacturer = manufacturer
            .trim_matches(|c| c == '(' || c == ')')
            .trim()
            .to_owned();
        let mut product = strip_weight(&product);
        // A trailing "(code)" is the maker's item number.
        let mut item = None;
        if let (Some(open), true) = (product.rfind('('), product.ends_with(')')) {
            let code = product[open + 1..product.len() - 1].trim().to_owned();
            if !code.is_empty() {
                item = Some(code);
                product = product[..open].trim().to_owned();
            }
        }
        if manufacturer.is_empty() {
            for maker in KNOWN_MAKERS {
                if let Some(rest) = strip_prefix_ignore_case(&product, maker) {
                    manufacturer = (*maker).to_owned();
                    product = rest.trim().to_owned();
                    break;
                }
            }
        }
        let product = strip_trailing_weight(&product)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        CartridgeName {
            manufacturer,
            product: if product.is_empty() {
                raw.trim().to_owned()
            } else {
                product
            },
            item,
        }
    }
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    let rest = &text[prefix.len()..];
    (head.eq_ignore_ascii_case(prefix) && (rest.is_empty() || rest.starts_with(' ')))
        .then_some(rest)
}

/// "150gr. VOR-TX" → "VOR-TX"; also "55-gr.", "55-grain", "56gr".
fn strip_weight(text: &str) -> String {
    let digits = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .count();
    if digits == 0 {
        return text.trim().to_owned();
    }
    let rest = text[digits..].trim_start_matches(['-', ' ']);
    let lower = rest.to_lowercase();
    for unit in ["grain", "gr"] {
        if lower.starts_with(unit) {
            return rest[unit.len()..]
                .trim_start_matches(['.', ' '])
                .trim()
                .to_owned();
        }
    }
    text.trim().to_owned()
}

/// "Naturalis 50 gr" → "Naturalis".
fn strip_trailing_weight(text: &str) -> String {
    let lower = text.to_lowercase();
    for unit in [" gr", "gr"] {
        if let Some(head) = lower.strip_suffix(unit) {
            let head = head.trim_end();
            let figures = head
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .count();
            if figures > 0 && head.len() > figures {
                return text[..head.len() - figures].trim().to_owned();
            }
        }
    }
    text.trim().to_owned()
}

fn join_notes(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn round(value: f64, decimals: i32) -> f64 {
    let scale = 10_f64.powi(decimals);
    (value * scale).round() / scale
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../testdata/strelok-sample.srl");

    fn options() -> PlanOptions {
        PlanOptions {
            caliber_overrides: HashMap::new(),
            updated_at: 1_700_000_000_000,
        }
    }

    #[test]
    fn ampersands_are_repaired_but_entities_kept() {
        assert_eq!(
            repair_ampersands("Sellier & Bellot &amp; &#38; &lt;"),
            "Sellier &amp; Bellot &amp; &#38; &lt;"
        );
    }

    #[test]
    fn cartridge_names_split_into_maker_product_and_item() {
        let parse = CartridgeName::parse;
        assert_eq!(
            parse("150gr. VOR-TX (21540), 308 Win, (Barnes)"),
            CartridgeName {
                manufacturer: "Barnes".into(),
                product: "VOR-TX".into(),
                item: Some("21540".into())
            }
        );
        assert_eq!(
            parse("124gr. FMJ (V340762), 308 Win, (Sellier & Bellot)").manufacturer,
            "Sellier & Bellot"
        );
        assert_eq!(
            parse("168gr. VOR-TX (21541), , Barnes").manufacturer,
            "Barnes"
        );
        assert_eq!(
            parse("55-grain Soft Point (4315050), , Lapua").product,
            "Soft Point"
        );
        assert_eq!(
            parse("Lapua Naturalis 50 gr"),
            CartridgeName {
                manufacturer: "Lapua".into(),
                product: "Naturalis".into(),
                item: None
            }
        );
        assert_eq!(parse("77gr. HPBT, GGG").manufacturer, "GGG");
        assert_eq!(parse("fox bullet").product, "fox bullet");
        assert_eq!(parse("56gr Copperbear").product, "Copperbear");
        assert_eq!(
            parse("20gr. Gamepoint  JSP (0052), , CCI").product,
            "Gamepoint JSP"
        );
    }

    #[test]
    fn calibers_are_found_from_names_and_diameter() {
        let plan = plan(SAMPLE, &options(), |_| None).unwrap();
        let caliber = |name: &str| {
            let rifle = plan
                .rifles
                .iter()
                .find(|rifle| rifle.strelok_name == name)
                .unwrap();
            (rifle.caliber.as_str(), rifle.caliber_source)
        };
        assert_eq!(
            caliber("Brownig X-bolt, .308Win"),
            ("308-win", CaliberSource::Names)
        );
        assert_eq!(
            caliber("saur 202 30-06"),
            ("30-06-sprg", CaliberSource::Names)
        );
        assert_eq!(caliber("Marcus 6.5"), ("6-5x55-se", CaliberSource::Names));
        assert_eq!(caliber("CZ 455 17 HMR"), ("17-hmr", CaliberSource::Names));
        assert_eq!(caliber("CZ 455 22lr"), ("22-lr", CaliberSource::Names));
        assert_eq!(caliber("Mystery rifle").1, CaliberSource::Guess);
    }

    #[test]
    fn the_sample_backup_becomes_valid_records() {
        let plan = plan(SAMPLE, &options(), |_| None).unwrap();
        let data = &plan.data;
        assert_eq!(data.rifles.len(), 6);
        assert_eq!(data.scopes.len(), 6);
        // Samples are skipped and the custom-drag cartridge is reported.
        assert!(data.bullets.iter().all(|b| !b.name.starts_with("Sample")));
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.contains("custom drag curve")),
            "{:?}",
            plan.warnings
        );
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.contains("Mystery rifle") && w.contains("guess"))
        );

        let xbolt = data
            .rifles
            .iter()
            .find(|r| r.name.starts_with("Brownig"))
            .unwrap();
        assert_eq!((xbolt.click_unit, xbolt.click_value), (ClickUnit::Mil, 0.1));
        assert_eq!(xbolt.twist_in, 12.0);
        assert_eq!(
            (xbolt.table.start_m, xbolt.table.end_m, xbolt.table.step_m),
            (100.0, 200.0, 10.0)
        );
        assert!(xbolt.table.columns.drop_clicks && !xbolt.table.columns.drop_moa);
        let scope = data
            .scopes
            .iter()
            .find(|s| Some(&s.id) == xbolt.scope_id.as_ref())
            .unwrap();
        assert_eq!(
            (
                scope.magnification_min,
                scope.magnification_max,
                scope.focal_plane
            ),
            (2.5, 10.0, FocalPlane::Sfp)
        );

        let multi = data.bullets.iter().find(|b| b.name == "HPBT").unwrap();
        assert_eq!(multi.manufacturer, "GGG");
        assert_eq!(multi.bc_bands.len(), 4);
        let copper = data
            .rifle_loads
            .iter()
            .find(|l| l.impact_offset_mrad != [0.0, 0.0])
            .unwrap();
        assert!((copper.impact_offset_mrad[1] - 0.69 * MRAD_PER_MOA).abs() < 1e-3);
        let recorded = data
            .rifle_loads
            .iter()
            .find(|l| matches!(l.zero, LoadZero::Recorded(_)))
            .unwrap();
        let LoadZero::Recorded(weather) = recorded.zero else {
            unreachable!()
        };
        assert!((weather.pressure_hpa - 999.92).abs() < 0.01, "750 mmHg");
        let sample_temp = data
            .bullets
            .iter()
            .find(|b| b.name.starts_with("Super-X"))
            .unwrap();
        assert!((sample_temp.velocity_per_degree - 351.0 * 2.4 / 100.0 / 15.0).abs() < 1e-3);
        assert_eq!(
            plan.selected.as_ref().map(|(id, _)| id.as_str()),
            Some(xbolt.id.as_str())
        );
        for rifle in &data.rifles {
            rifle.validate().unwrap();
        }
    }

    #[test]
    fn importing_twice_adds_nothing_and_keeps_local_edits() {
        let mut store = Store::open_in_memory().unwrap();
        let first_plan = store.plan_strelok(SAMPLE, &options()).unwrap();
        let first = store.import_data(&first_plan.data, false).unwrap();
        assert_eq!(
            first.added,
            first_plan.data.rifles.len()
                + first_plan.data.bullets.len()
                + first_plan.data.rifle_loads.len()
                + first_plan.data.scopes.len()
        );
        let counts = (
            store.rifles().unwrap().len(),
            store.bullets().unwrap().len(),
            store.rifle_loads().unwrap().len(),
        );

        // The hunter fixes a caliber and renames a bullet after importing.
        let mut mystery = store
            .rifles()
            .unwrap()
            .into_iter()
            .find(|r| r.name == "Mystery rifle")
            .unwrap();
        mystery.caliber = "6-5x55-se".into();
        store.save_rifle(&mut mystery).unwrap();

        let again_plan = store.plan_strelok(SAMPLE, &options()).unwrap();
        let again = store.import_data(&again_plan.data, false).unwrap();
        assert_eq!(again.added, 0, "{again:?}");
        assert_eq!(again.updated, 0, "{again:?}");
        assert!(again.kept_local >= 1, "the edited rifle is kept: {again:?}");
        assert_eq!(
            (
                store.rifles().unwrap().len(),
                store.bullets().unwrap().len(),
                store.rifle_loads().unwrap().len()
            ),
            counts
        );
        assert_eq!(
            store.rifle(&mystery.id).unwrap().unwrap().caliber,
            "6-5x55-se"
        );
        let rifle = again_plan
            .rifles
            .iter()
            .find(|r| r.strelok_name == "Mystery rifle")
            .unwrap();
        assert_eq!(
            rifle.caliber_source,
            CaliberSource::Existing,
            "a fixed caliber is reused when planning"
        );

        // A newer backup updates what the hunter hasn't touched.
        let newer = PlanOptions {
            updated_at: i64::MAX / 2,
            ..options()
        };
        let changed = SAMPLE.replacen(
            "<ZeroDistance>100</ZeroDistance>",
            "<ZeroDistance>150</ZeroDistance>",
            1,
        );
        let later = store
            .import_data(&store.plan_strelok(&changed, &newer).unwrap().data, false)
            .unwrap();
        assert_eq!(later.added, 0);
        assert!(later.updated > 0);
    }

    #[test]
    fn imperial_backups_are_refused() {
        let imperial = SAMPLE.replace(
            "<Metric_units_on>true</Metric_units_on>",
            "<Metric_units_on>false</Metric_units_on>",
        );
        assert!(matches!(
            plan(&imperial, &options(), |_| None),
            Err(StoreError::Invalid(_))
        ));
        assert!(plan("<notxml", &options(), |_| None).is_err());
    }
}

//! A saved group's velocities on its page: readings typed, pasted or read
//! from a chronograph file, paired with the shots in marking order; their
//! spread and warnings; how the shots' height follows velocity, beside
//! the solver's expected slope; and the string as its load's velocity.
//! Also the page's tools around the numbers (`GroupTools`): what they
//! asked for goes back to the page as actions.

use ballistics_core::solver;
use ballistics_store::chrono;
use ballistics_store::{
    Bullet, Conditions, Group, Marks, Rifle, RifleLoad, ShotStats, VelocityPoint,
};
use eframe::egui::{self, Color32, RichText, Stroke, pos2, vec2};
use groups_core::{
    Point, StatsError, VelocityStats, VelocityVertical, VelocityWarning, velocity::MIN_PAIRED_SHOTS,
};

use crate::app::{Action, Data};
use crate::groups::details::export;
use crate::groups::measure::Measured;
use crate::i18n::{tr, trf};
use crate::number_field::NumberField;
use crate::theme;
use crate::units::{self, Quantity, Unit, UnitPrefs};

/// The slowest and fastest readings a group may keep, m/s.
const SLOWEST_MPS: f64 = 100.0;
const FASTEST_MPS: f64 = 1500.0;
/// The most velocities a load keeps.
const LOAD_VELOCITIES: usize = 10;
/// A load's velocity within this of a temperature is that temperature's.
const SAME_TEMPERATURE_C: f64 = 0.5;

/// Why a string of readings can't be read.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadingError {
    /// Not a number: the text as written.
    NotANumber(String),
    /// A comma that could be a decimal comma or a thousands separator,
    /// such as "2,850".
    Ambiguous(String),
    /// Outside what a group keeps (100–1500 m/s): the text and m/s.
    OutOfRange(String, f64),
}

impl ReadingError {
    pub fn text(&self, u: &UnitPrefs) -> String {
        match self {
            ReadingError::NotANumber(token) => trf!("“{}” isn't a velocity.", token),
            ReadingError::Ambiguous(token) => trf!(
                "“{}” could be read two ways: write it without the comma, or with a decimal \
                 point.",
                token
            ),
            ReadingError::OutOfRange(token, mps) => trf!(
                "“{}” is {}: velocities must be {} to {}. Is the unit right?",
                token,
                u.format(Quantity::Speed, *mps),
                u.format(Quantity::Speed, SLOWEST_MPS),
                u.format(Quantity::Speed, FASTEST_MPS)
            ),
        }
    }
}

fn plain_number(text: &str) -> Option<f64> {
    let digits = text.strip_prefix('+').unwrap_or(text);
    let shaped = !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
        && digits.chars().filter(|&c| c == '.').count() <= 1
        && digits.chars().any(|c| c.is_ascii_digit());
    shaped
        .then(|| digits.parse::<f64>().ok())
        .flatten()
        .filter(|value| value.is_finite())
}

/// Readings typed or pasted in `unit`, as m/s. They may be one per line or
/// separated by spaces, commas or semicolons; a comma is a decimal comma
/// where it can't be anything else ("823,4"). A comma that could be a
/// thousands separator ("2,850") is refused, never guessed at, as is a
/// reading outside 100–1500 m/s.
pub fn parse_readings(text: &str, unit: Unit) -> Result<Vec<f64>, ReadingError> {
    let in_range = |shown: f64| (SLOWEST_MPS..=FASTEST_MPS).contains(&unit.base(shown));
    let mut readings = Vec::new();
    for chunk in text
        .split(|c: char| c.is_whitespace() || c == ';')
        .filter(|chunk| !chunk.is_empty())
    {
        let parts: Vec<&str> = chunk.split(',').filter(|part| !part.is_empty()).collect();
        let numbers: Vec<Option<f64>> = parts.iter().map(|part| plain_number(part)).collect();
        if let Some(index) = numbers.iter().position(Option::is_none) {
            return Err(ReadingError::NotANumber(parts[index].to_owned()));
        }
        let numbers: Vec<f64> = numbers.into_iter().flatten().collect();
        let shown: Vec<f64> = if parts.len() == 1 {
            numbers
        } else if numbers.iter().all(|&value| in_range(value)) {
            // Commas between readings: "823,825,822".
            numbers
        } else if parts.len() == 2
            && !parts[0].contains('.')
            && parts[1].chars().all(|c| c.is_ascii_digit())
            && parts[1].len() != 3
        {
            // A decimal comma: "823,4".
            vec![
                format!("{}.{}", parts[0], parts[1])
                    .parse()
                    .unwrap_or(f64::NAN),
            ]
        } else {
            return Err(ReadingError::Ambiguous(chunk.to_owned()));
        };
        for value in shown {
            let mps = unit.base(value);
            if !in_range(value) {
                return Err(ReadingError::OutOfRange(chunk.to_owned(), mps));
            }
            readings.push(mps);
        }
    }
    Ok(readings)
}

/// Readings paired with `shots` marked shots in marking order: the first
/// reading to the first shot, and so on. Readings beyond the last shot are
/// left over, as the group's unpaired velocities.
pub fn pair(shots: usize, readings: &[f64]) -> (Vec<Option<f64>>, Vec<f64>) {
    let paired = (0..shots)
        .map(|index| readings.get(index).copied())
        .collect();
    let extra = readings.iter().skip(shots).copied().collect();
    (paired, extra)
}

/// Each marked shot's velocity, in marking order.
pub fn shot_velocities(marks: &Marks) -> Vec<Option<f64>> {
    match marks {
        Marks::Photo(marks) => marks.shots.iter().map(|shot| shot.velocity_mps).collect(),
        Marks::Grid(marks) => marks.shots.iter().map(|shot| shot.velocity_mps).collect(),
    }
}

/// Sets each marked shot's velocity; shots beyond `velocities` have none.
pub fn set_shot_velocities(marks: &mut Marks, velocities: &[Option<f64>]) {
    let velocity = |index: usize| velocities.get(index).copied().flatten();
    match marks {
        Marks::Photo(marks) => {
            for (index, shot) in marks.shots.iter_mut().enumerate() {
                shot.velocity_mps = velocity(index);
            }
        }
        Marks::Grid(marks) => {
            for (index, shot) in marks.shots.iter_mut().enumerate() {
                shot.velocity_mps = velocity(index);
            }
        }
    }
}

/// The weather a group was shot in for the solver: its own conditions,
/// else its session's, else a standard day. No wind: only the vertical is
/// asked of it.
pub fn solver_conditions(conditions: &Conditions) -> ballistics_shot::Conditions {
    let standard = ballistics_shot::Conditions::default();
    let temperature_c = conditions.temperature_c.unwrap_or(standard.temperature_c);
    ballistics_shot::Conditions {
        temperature_c,
        pressure: conditions
            .pressure_hpa
            .map_or(standard.pressure, ballistics_shot::Pressure::Station),
        humidity_percent: conditions
            .humidity_percent
            .unwrap_or(standard.humidity_percent),
        powder_temperature_c: conditions.powder_temperature_c,
        wind_speed_mps: 0.0,
        aerodynamic_jump: false,
        ..standard
    }
}

/// How much higher a shot lands at `distance_m` per m/s more muzzle
/// velocity, mm per m/s, for this rifle and bullet with the sights left
/// as zeroed: the solver's flight at the load's velocity, and 2 m/s
/// either side of it.
pub fn expected_slope(
    rifle: &Rifle,
    bullet: &Bullet,
    load: Option<&RifleLoad>,
    conditions: &ballistics_shot::Conditions,
    distance_m: f64,
) -> Option<f64> {
    const STEP_MPS: f64 = 2.0;
    let scenario = ballistics_shot::scenario(rifle, bullet, load, conditions);
    let reach = distance_m + 5.0;
    let zeroed = solver::solve(&scenario, reach).ok()?;
    let zero = (zeroed.zero_elevation, zeroed.zero_windage);
    let height = |change: f64| {
        let mut faster = scenario.clone();
        faster.load.muzzle_velocity += change;
        let flight = solver::solve_with_zero(&faster, reach, zero).ok()?;
        flight.at(distance_m).map(|row| row.drop)
    };
    let slope = (height(STEP_MPS)? - height(-STEP_MPS)?) / (2.0 * STEP_MPS) * 1000.0;
    slope.is_finite().then_some(slope)
}

/// `load` (or a new one for the rifle and bullet) with `readings` as its
/// velocity at `temperature_c`: the measurement at that temperature is
/// replaced, else one is added (in place of the nearest when the load has
/// ten). With `replace_all` the load's other measurements go, as when they
/// were made through an earlier barrel.
pub fn with_velocity(
    load: Option<&RifleLoad>,
    rifle_id: &str,
    bullet_id: &str,
    readings: &[f64],
    temperature_c: f64,
    replace_all: bool,
) -> Option<RifleLoad> {
    let stats = VelocityStats::new(readings).ok()?;
    let mut load = load
        .cloned()
        .unwrap_or_else(|| RifleLoad::new(rifle_id, bullet_id));
    if replace_all {
        load.velocities.clear();
    }
    let point = VelocityPoint {
        temperature_c,
        velocity_mps: stats.average,
        shots: Some(ShotStats {
            count: stats.shots as u32,
            sd_mps: stats.sd.map_or(0.0, |sd| sd.value),
            es_mps: stats.extreme_spread,
        }),
    };
    let nearest = load
        .velocities
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1.temperature_c - temperature_c)
                .abs()
                .total_cmp(&(b.1.temperature_c - temperature_c).abs())
        })
        .map(|(index, existing)| (index, (existing.temperature_c - temperature_c).abs()));
    match nearest {
        Some((index, gap))
            if gap <= SAME_TEMPERATURE_C || load.velocities.len() >= LOAD_VELOCITIES =>
        {
            load.velocities[index] = point;
        }
        _ => load.velocities.push(point),
    }
    Some(load)
}

/// What the group's page knows besides the numbers, made fresh each
/// frame by `GroupsPage::show_with_tools`.
#[derive(Clone)]
pub struct Context {
    pub group: Group,
    /// Its own conditions over its session's.
    pub conditions: Conditions,
    pub rifle: Option<Rifle>,
    pub bullet: Option<Bullet>,
    pub load: Option<RifleLoad>,
    /// The barrel fitted to the rifle now.
    pub fitted: Option<ballistics_store::Barrel>,
    /// The barrel the group was shot through, when it says.
    pub barrel: Option<ballistics_store::Barrel>,
    /// The air temperature on Calculate, for a group without one.
    pub today_c: f64,
}

impl Context {
    pub fn of(group_id: &str, data: &Data) -> Option<Context> {
        let group = data
            .groups
            .groups
            .iter()
            .find(|g| g.id == group_id)?
            .clone();
        let session = group
            .session_id
            .as_ref()
            .and_then(|id| data.groups.sessions.iter().find(|s| &s.id == id));
        let conditions = session.map_or(group.conditions, |s| group.conditions.or(s.conditions));
        let rifle = group
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|r| &r.id == id))
            .cloned();
        let bullet = group
            .bullet_id
            .as_ref()
            .and_then(|id| data.bullets.iter().find(|b| &b.id == id))
            .cloned();
        let load = match (&rifle, &bullet) {
            (Some(rifle), Some(bullet)) => data
                .loads
                .iter()
                .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
                .cloned(),
            _ => None,
        };
        let barrel_named = |id: Option<&str>| {
            data.groups
                .barrels
                .iter()
                .find(|barrel| Some(barrel.id.as_str()) == id)
                .cloned()
        };
        let fitted = barrel_named(rifle.as_ref().and_then(|r| r.fitted_barrel_id.as_deref()));
        let barrel = barrel_named(group.barrel_id.as_deref());
        Some(Context {
            group,
            conditions,
            rifle,
            bullet,
            load,
            fitted,
            barrel,
            today_c: data.today.temperature_c,
        })
    }

    /// The powder's temperature when the group was shot, if known.
    fn temperature_c(&self) -> Option<f64> {
        self.conditions
            .powder_temperature_c
            .or(self.conditions.temperature_c)
    }
}

/// What the tools asked the page for.
pub enum Request {
    /// Save the group with these velocities.
    SaveVelocities {
        shots: Vec<Option<f64>>,
        extra: Vec<f64>,
    },
    /// Save this as the group's load.
    UseVelocity(Box<RifleLoad>),
    /// Save the group as a picture.
    Picture,
    /// Save the group's shots as CSV.
    ShotsCsv,
}

/// The tools' state on a group's page.
#[derive(Default)]
pub struct GroupTools {
    pub(super) context: Option<Box<Context>>,
    pub(super) requests: Vec<Request>,
    draft: Draft,
    /// The expected slope, and what it was worked out from.
    slope: Option<(SlopeKey, Option<f64>)>,
    pub(super) picture: Option<export::PictureJob>,
    chrono_file: crate::files::Picked,
}

#[derive(Clone, PartialEq)]
struct SlopeKey {
    group: i64,
    rifle: Option<i64>,
    bullet: Option<i64>,
    load: Option<i64>,
    distance_m: Option<u64>,
}

/// How a group's velocities are to go into its load.
struct Using {
    temperature_c: f64,
    /// Replace the load's velocities measured before the barrel was fitted.
    replace_earlier: bool,
}

/// How many of the load's velocities predate the fitted barrel: all of them
/// when the load was last changed before the barrel was fitted.
pub fn earlier_than_barrel(load: &RifleLoad, fitted: &ballistics_store::Barrel) -> usize {
    match fitted.fitted_at {
        Some(at) if load.updated_at < at => load.velocities.len(),
        _ => 0,
    }
}

/// The velocities as edited.
#[derive(Default)]
struct Draft {
    group_id: String,
    /// The saved velocities the draft started from.
    saved: (Vec<Option<f64>>, Vec<f64>),
    shots: Vec<Option<f64>>,
    extra: Vec<f64>,
    text: String,
    problem: Option<String>,
    /// A chronograph file read: its name, strings and the one chosen.
    chrono: Option<(String, Vec<chrono::Session>, usize)>,
    /// "Use as this load's velocity" asked.
    using: Option<Using>,
}

impl Draft {
    fn changed(&self) -> bool {
        (&self.shots, &self.extra) != (&self.saved.0, &self.saved.1)
    }

    /// Follows the saved group: a new group starts afresh, and a draft
    /// without changes takes what was saved since.
    fn follow(&mut self, group: &Group) {
        let saved = (
            shot_velocities(&group.marks),
            group.extra_velocities_mps.clone(),
        );
        if self.group_id != group.id {
            *self = Draft {
                group_id: group.id.clone(),
                shots: saved.0.clone(),
                extra: saved.1.clone(),
                saved,
                ..Draft::default()
            };
        } else if saved != self.saved {
            if !self.changed() {
                self.shots = saved.0.clone();
                self.extra = saved.1.clone();
            }
            self.saved = saved;
        }
    }
}

impl GroupTools {
    fn expected_slope(&mut self, context: &Context) -> Option<f64> {
        let (rifle, bullet, distance_m) = (
            context.rifle.as_ref()?,
            context.bullet.as_ref()?,
            context.group.distance_m?,
        );
        let key = SlopeKey {
            group: context.group.updated_at,
            rifle: Some(rifle.updated_at),
            bullet: Some(bullet.updated_at),
            load: context.load.as_ref().map(|load| load.updated_at),
            distance_m: Some(distance_m.to_bits()),
        };
        if self.slope.as_ref().is_none_or(|(kept, _)| *kept != key) {
            let conditions = solver_conditions(&context.conditions);
            let slope = expected_slope(
                rifle,
                bullet,
                context.load.as_ref(),
                &conditions,
                distance_m,
            );
            self.slope = Some((key, slope));
        }
        self.slope.as_ref().and_then(|(_, slope)| *slope)
    }
}

fn warning_text(warning: &VelocityWarning, u: &UnitPrefs) -> String {
    match warning {
        VelocityWarning::Unusual { index, mps } => trf!(
            "Reading {} ({}) is unusual for a rifle: check it.",
            index + 1,
            u.format(Quantity::Speed, *mps)
        ),
        VelocityWarning::FarFromMedian {
            index,
            mps,
            median_mps,
        } => trf!(
            "Reading {} ({}) is far from the median ({}): a slip of the decimal point, or \
             fps typed as m/s?",
            index + 1,
            u.format(Quantity::Speed, *mps),
            u.format(Quantity::Speed, *median_mps)
        ),
    }
}

/// The spread of the readings: avg, SD with its range, ES, and warnings.
pub fn summary(ui: &mut egui::Ui, readings: &[f64], u: &UnitPrefs) {
    let Ok(stats) = VelocityStats::new(readings) else {
        return;
    };
    let mut parts = vec![trf!("avg {}", u.format(Quantity::Speed, stats.average))];
    if let Some(sd) = stats.sd {
        parts.push(trf!(
            "SD {} (95%: {} – {})",
            u.number(Quantity::Speed, sd.value, false),
            u.number(Quantity::Speed, sd.range.low, false),
            u.format(Quantity::Speed, sd.range.high)
        ));
    }
    parts.push(trf!(
        "ES {}",
        u.format(Quantity::Speed, stats.extreme_spread)
    ));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(trf!("Velocity, {} readings", stats.shots)).strong());
        ui.label(parts.join(" · "));
    });
    let palette = theme::palette(ui.ctx());
    for warning in &stats.warnings {
        ui.add(
            egui::Label::new(RichText::new(warning_text(warning, u)).color(palette.warning)).wrap(),
        );
    }
}

/// The counted shots that have a velocity: (velocity m/s, shot mm).
fn paired_points(measured: &Measured, shots: &[Option<f64>]) -> (Vec<Point>, Vec<Option<f64>>) {
    measured
        .counted
        .iter()
        .map(|&index| {
            (
                measured.shots_mm[index],
                shots.get(index).copied().flatten(),
            )
        })
        .unzip()
}

/// The velocities part of a saved group's page; without the page's
/// context (while marking), just the readings' spread.
pub fn section(ui: &mut egui::Ui, measured: &Measured, readings: &[f64], tools: &mut GroupTools) {
    let u = units::prefs(ui.ctx());
    let Some(context) = tools.context.clone() else {
        ui.add_space(4.0);
        summary(ui, readings, &u);
        return;
    };
    let context = *context;
    tools.draft.follow(&context.group);
    collect_chrono(tools);
    ui.add_space(6.0);
    ui.separator();
    let saved: Vec<f64> = {
        let (shots, extra) = &tools.draft.saved;
        shots.iter().flatten().chain(extra).copied().collect()
    };
    if saved.is_empty() {
        ui.label(RichText::new(tr("Velocity")).strong());
        ui.weak(tr(
            "No readings yet. Paste the chronograph's string below, read its file, or type \
             the readings.",
        ));
    } else {
        summary(ui, &saved, &u);
    }
    let (points, velocities) = paired_points(measured, &tools.draft.saved.0);
    let paired = velocities.iter().flatten().count();
    if paired >= MIN_PAIRED_SHOTS {
        let slope = tools.expected_slope(&context);
        velocity_vertical(
            ui,
            &points,
            &velocities,
            slope,
            context.group.distance_m,
            &u,
        );
    } else if paired > 0 {
        ui.weak(trf!(
            "Velocity against height needs {} shots with a velocity; {} have one.",
            MIN_PAIRED_SHOTS,
            paired
        ));
    }
    use_for_load(ui, &context, &saved, tools, &u);
    let open = saved.is_empty();
    crate::widgets::fold_with(
        ui,
        "group-velocity-entry",
        tr("Enter or edit readings"),
        &trf!("in {}", u.label(Quantity::Speed)),
        open,
        |ui| entry(ui, &context, tools, &u),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(tr("Share")).strong());
        let busy = tools.picture.is_some();
        if ui
            .add_enabled(!busy, egui::Button::new(tr("Picture (PNG)")))
            .on_hover_text(tr(
                "The drawing with its marks and numbers, to send or print",
            ))
            .clicked()
        {
            tools.requests.push(Request::Picture);
        }
        if busy {
            ui.spinner();
        }
        if ui
            .button(tr("Shots (CSV)"))
            .on_hover_text(tr(
                "One row per shot, in mm and mrad, from the aim and from the centre",
            ))
            .clicked()
        {
            tools.requests.push(Request::ShotsCsv);
        }
    });
}

fn collect_chrono(tools: &mut GroupTools) {
    let Some((file, bytes)) = tools.chrono_file.take() else {
        return;
    };
    match chrono::parse(&bytes) {
        Ok(strings) => {
            tools.draft.problem = None;
            if strings.len() == 1 {
                fill_from(&mut tools.draft, &strings[0]);
            }
            tools.draft.chrono = Some((file, strings, 0));
        }
        Err(error) => tools.draft.problem = Some(format!("{file}: {error}")),
    }
}

/// Puts a chronograph string's readings in the box, in the hunter's unit.
fn fill_from(draft: &mut Draft, string: &chrono::Session) {
    draft.text = string
        .velocities_mps
        .iter()
        .map(|mps| format!("{mps:.1}"))
        .collect::<Vec<_>>()
        .join("\n");
    draft.problem = None;
}

fn entry(ui: &mut egui::Ui, context: &Context, tools: &mut GroupTools, u: &UnitPrefs) {
    let unit = units::unit_of(ui, Quantity::Speed);
    let shots = tools.draft.shots.len();
    ui.add(
        egui::TextEdit::multiline(&mut tools.draft.text)
            .hint_text(tr("One reading a line, or 823 825 821 …"))
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    ui.horizontal_wrapped(|ui| {
        let typed = !tools.draft.text.trim().is_empty();
        let read = |text: &str| parse_readings(text, unit);
        if ui
            .add_enabled(
                typed,
                egui::Button::new(tr("Pair to shots in marking order")),
            )
            .on_hover_text(tr(
                "The first reading to shot 1, and so on; readings past the last shot are kept \
                 unpaired",
            ))
            .clicked()
        {
            match read(&tools.draft.text) {
                Ok(readings) => {
                    let (paired, extra) = pair(shots, &readings);
                    tools.draft.shots = paired;
                    tools.draft.extra = extra;
                    tools.draft.text.clear();
                    tools.draft.problem = None;
                }
                Err(error) => tools.draft.problem = Some(error.text(u)),
            }
        }
        if ui
            .add_enabled(typed, egui::Button::new(tr("Add unpaired")))
            .on_hover_text(tr("Readings that match no marked shot, such as foulers"))
            .clicked()
        {
            match read(&tools.draft.text) {
                Ok(readings) => {
                    tools.draft.extra.extend(readings);
                    tools.draft.text.clear();
                    tools.draft.problem = None;
                }
                Err(error) => tools.draft.problem = Some(error.text(u)),
            }
        }
        if ui
            .button(tr("From a chronograph file…"))
            .on_hover_text(tr(
                "Garmin Xero (ShotView CSV), LabRadar (SR…Report.csv) or MagnetoSpeed (LOG.CSV)",
            ))
            .clicked()
        {
            tools
                .chrono_file
                .pick(ui.ctx(), tr("Chronograph files"), &["csv", "CSV", "txt"]);
        }
    });
    if unit.label != "m/s" {
        ui.weak(trf!("Readings in {}; kept in m/s.", unit.label));
    }
    if let Some(problem) = &tools.draft.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    let mut chosen = None;
    if let Some((file, strings, index)) = &mut tools.draft.chrono
        && strings.len() > 1
    {
        ui.label(trf!("{} has {} strings: choose one", file, strings.len()));
        for (number, string) in strings.iter().enumerate() {
            let name = string
                .name
                .clone()
                .unwrap_or_else(|| trf!("String {}", number + 1));
            let text = trf!(
                "{} · {} · {} shots, avg {}",
                name,
                string.device.name(),
                string.velocities_mps.len(),
                u.format(Quantity::Speed, string.average_mps())
            );
            if ui.radio(*index == number, text).clicked() {
                *index = number;
                chosen = Some(string.clone());
            }
        }
    }
    if let Some(string) = chosen {
        fill_from(&mut tools.draft, &string);
    }

    ui.add_space(4.0);
    egui::Grid::new("shot-velocities")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for (index, velocity) in tools.draft.shots.iter_mut().enumerate() {
                ui.label(trf!("Shot {}", index + 1));
                ui.horizontal(|ui| {
                    let mut known = velocity.is_some();
                    if ui.checkbox(&mut known, "").changed() {
                        *velocity = known.then_some(context.load.as_ref().map_or(800.0, |load| {
                            load.velocities.first().map_or(800.0, |p| p.velocity_mps)
                        }));
                    }
                    match velocity {
                        Some(mps) => {
                            speed_field(ui, mps, unit);
                        }
                        None => {
                            ui.weak("—");
                        }
                    }
                });
                ui.end_row();
            }
        });
    if !tools.draft.extra.is_empty() {
        ui.label(tr("Unpaired"));
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (index, mps) in tools.draft.extra.iter_mut().enumerate() {
                speed_field(ui, mps, unit);
                if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                    remove = Some(index);
                }
            }
        });
        if let Some(index) = remove {
            tools.draft.extra.remove(index);
        }
    }
    if tools.draft.changed() {
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(
                    RichText::new(tr("Save velocities")).strong(),
                ))
                .clicked()
            {
                tools.requests.push(Request::SaveVelocities {
                    shots: tools.draft.shots.clone(),
                    extra: tools.draft.extra.clone(),
                });
            }
            if ui.button(tr("Undo changes")).clicked() {
                tools.draft.shots = tools.draft.saved.0.clone();
                tools.draft.extra = tools.draft.saved.1.clone();
            }
            ui.weak(tr("Not saved yet"));
        });
    }
}

fn speed_field(ui: &mut egui::Ui, mps: &mut f64, unit: Unit) {
    let mut shown = unit.show(*mps);
    let (low, high) = (unit.show(SLOWEST_MPS), unit.show(FASTEST_MPS));
    let response = ui.add(
        NumberField::new(&mut shown)
            .range(low..=high)
            .speed(unit.show(1.0) - unit.show(0.0))
            .max_decimals(unit.decimals + 1)
            .suffix(format!(" {}", unit.label)),
    );
    if response.changed() {
        *mps = unit.base(shown);
    }
}

fn use_for_load(
    ui: &mut egui::Ui,
    context: &Context,
    saved: &[f64],
    tools: &mut GroupTools,
    u: &UnitPrefs,
) {
    let (Some(rifle), Some(bullet)) = (&context.rifle, &context.bullet) else {
        return;
    };
    if saved.is_empty() {
        return;
    }
    let Some(using) = &mut tools.draft.using else {
        if ui
            .button(tr("Use as this load's velocity"))
            .on_hover_text(tr(
                "The average and spread become this rifle's measured velocity for the bullet, \
                 at the group's temperature, for the calculator",
            ))
            .clicked()
        {
            tools.draft.using = Some(Using {
                temperature_c: context.temperature_c().unwrap_or(context.today_c).round(),
                replace_earlier: true,
            });
        }
        return;
    };
    let Ok(stats) = VelocityStats::new(saved) else {
        return;
    };
    let palette = theme::palette(ui.ctx());
    let mut done = false;
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, palette.accent))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(trf!(
                    "Save {} ({} shots) as the velocity of {} in {}?",
                    u.format(Quantity::Speed, stats.average),
                    stats.shots,
                    bullet.display_name(),
                    rifle.name
                ))
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Powder temperature"));
                units::drag(
                    ui,
                    &mut using.temperature_c,
                    Quantity::Temperature,
                    -60.0..=70.0,
                    0.5,
                );
            });
            let temperature = &using.temperature_c;
            let warn = |ui: &mut egui::Ui, text: String| {
                ui.add(egui::Label::new(RichText::new(text).color(palette.warning)).wrap());
            };
            if let (Some(shot), Some(fitted)) = (&context.barrel, &context.fitted)
                && shot.id != fitted.id
            {
                warn(
                    ui,
                    trf!(
                        "The group was shot through {}, not {}, the barrel on the rifle now.",
                        shot.name,
                        fitted.name
                    ),
                );
            }
            let earlier = match (&context.load, &context.fitted) {
                (Some(load), Some(fitted)) => earlier_than_barrel(load, fitted),
                _ => 0,
            };
            if earlier > 0
                && let Some(fitted) = &context.fitted
            {
                ui.checkbox(
                    &mut using.replace_earlier,
                    trf!(
                        "Replace the load's {} velocities measured before {} was fitted",
                        earlier,
                        fitted.name
                    ),
                );
            }
            if let Some(truing) = context.load.as_ref().and_then(|load| load.truing) {
                warn(
                    ui,
                    match truing.method {
                        ballistics_store::TruingMethod::Velocity => tr(
                            "The load is trued by its velocity: the calculator scales this \
                             velocity too. True it again after saving.",
                        ),
                        ballistics_store::TruingMethod::Drag => {
                            tr("The load is trued by its drag, worked out with its old \
                             velocity. True it again after saving.")
                        }
                    }
                    .to_owned(),
                );
            }
            let temperature = *temperature;
            let replace_all = earlier > 0 && using.replace_earlier;
            if context.temperature_c().is_none() {
                ui.weak(tr(
                    "The group has no temperature: the one on Calculate is filled in.",
                ));
            }
            let replaces = context.load.as_ref().is_some_and(|load| {
                load.velocities
                    .iter()
                    .any(|p| (p.temperature_c - temperature).abs() <= SAME_TEMPERATURE_C)
            });
            if replaces && !replace_all {
                ui.weak(tr("It replaces the load's velocity at that temperature."));
            }
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new(
                        RichText::new(tr("Save to the load")).strong(),
                    ))
                    .clicked()
                    && let Some(load) = with_velocity(
                        context.load.as_ref(),
                        &rifle.id,
                        &bullet.id,
                        saved,
                        temperature,
                        replace_all,
                    )
                {
                    tools.requests.push(Request::UseVelocity(Box::new(load)));
                    done = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    done = true;
                }
            });
        });
    if done {
        tools.draft.using = None;
    }
}

/// The plain line comparing the measured slope with the expected one.
pub fn comparison(fit: &VelocityVertical, expected: f64) -> &'static str {
    let range = fit.slope.range;
    if range.high < expected {
        tr(
            "The shots rise less with velocity than the solver expects: the barrel may be \
            compensating, the slower shots leaving it later and higher.",
        )
    } else if range.low > expected {
        tr(
            "The shots rise more with velocity than the solver expects: something besides \
            velocity moves them up and down too.",
        )
    } else {
        tr(
            "The shots rise with velocity about as the solver expects: no sign of \
            compensation, though few shots can't rule it out.",
        )
    }
}

fn velocity_vertical(
    ui: &mut egui::Ui,
    points: &[Point],
    velocities: &[Option<f64>],
    expected: Option<f64>,
    distance_m: Option<f64>,
    u: &UnitPrefs,
) {
    let fit = match VelocityVertical::new(points, velocities) {
        Ok(fit) => fit,
        Err(StatsError::NoVelocitySpread) => {
            ui.weak(tr(
                "Every reading is the same: velocity can't explain the height.",
            ));
            return;
        }
        Err(_) => return,
    };
    ui.add_space(4.0);
    ui.label(RichText::new(tr("Velocity × vertical")).strong());
    let centre_y = {
        let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
        ys.iter().sum::<f64>() / ys.len().max(1) as f64
    };
    let pairs: Vec<(f64, f64)> = points
        .iter()
        .zip(velocities)
        .filter_map(|(point, v)| v.map(|v| (v, point.y - centre_y)))
        .collect();
    chart(ui, &pairs, &fit, expected, u);
    let speed = units::unit_of(ui, Quantity::Speed);
    // The slope in the hunter's units: height per one of their speed unit.
    let number = |mm_per_mps: f64| {
        u.number(
            Quantity::GroupSize,
            mm_per_mps / speed.show(1.0).max(1e-9),
            false,
        )
    };
    let per = |mm_per_mps: f64| {
        trf!(
            "{} {} per {}",
            number(mm_per_mps),
            u.label(Quantity::GroupSize),
            speed.label
        )
    };
    let wrapped = |ui: &mut egui::Ui, text: String| {
        ui.add(egui::Label::new(text).wrap());
    };
    match (expected, distance_m) {
        (Some(expected), Some(_)) => wrapped(
            ui,
            trf!(
                "Vertical follows velocity: {} (solver expects {}).",
                per(fit.slope.value),
                number(expected)
            ),
        ),
        _ => wrapped(
            ui,
            trf!("Vertical follows velocity: {}.", per(fit.slope.value)),
        ),
    }
    ui.add(
        egui::Label::new(
            RichText::new(trf!(
                "Velocity explains {}% of the vertical ({} shots); the slope's 95% range is {} \
                 to {}.",
                (fit.r_squared * 100.0).round(),
                fit.shots,
                number(fit.slope.range.low),
                number(fit.slope.range.high)
            ))
            .weak(),
        )
        .wrap(),
    );
    match (expected, distance_m) {
        (Some(expected), Some(_)) => {
            ui.add(egui::Label::new(RichText::new(comparison(&fit, expected)).weak()).wrap());
        }
        _ => {
            ui.weak(tr(
                "Give the group its rifle, bullet and distance to compare with what the solver \
                 expects.",
            ));
        }
    }
}

/// Height (mm from the centre) against velocity, with the fitted line and
/// the solver's slope (dashed) through the shots' mean.
fn chart(
    ui: &mut egui::Ui,
    pairs: &[(f64, f64)],
    fit: &VelocityVertical,
    expected: Option<f64>,
    u: &UnitPrefs,
) {
    let palette = theme::palette(ui.ctx());
    let width = ui.available_width().min(420.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 170.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        8.0,
        palette.sunken,
        palette.edge,
        egui::StrokeKind::Inside,
    );
    let plot = egui::Rect::from_min_max(rect.min + vec2(44.0, 10.0), rect.max - vec2(10.0, 24.0));
    let (mut v_low, mut v_high) = pairs.iter().fold((f64::MAX, f64::MIN), |(l, h), (v, _)| {
        (l.min(*v), h.max(*v))
    });
    let pad = ((v_high - v_low) * 0.1).max(1.0);
    v_low -= pad;
    v_high += pad;
    let h_reach = pairs.iter().map(|(_, h)| h.abs()).fold(1.0_f64, f64::max) * 1.25;
    let at = |v: f64, h: f64| {
        pos2(
            plot.left() + ((v - v_low) / (v_high - v_low)) as f32 * plot.width(),
            plot.center().y - (h / h_reach) as f32 * plot.height() / 2.0,
        )
    };
    let axis = Stroke::new(1.0_f32, palette.stroke);
    painter.line_segment([at(v_low, 0.0), at(v_high, 0.0)], axis);
    let text = |at: egui::Pos2, align: egui::Align2, words: String| {
        painter.text(
            at,
            align,
            words,
            egui::FontId::proportional(11.0),
            palette.text_muted,
        );
    };
    text(
        pos2(plot.left(), rect.bottom() - 4.0),
        egui::Align2::LEFT_BOTTOM,
        u.format(Quantity::Speed, v_low),
    );
    text(
        pos2(plot.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        tr("muzzle velocity").to_owned(),
    );
    let title = painter.layout_no_wrap(
        tr("height").to_owned(),
        egui::FontId::proportional(11.0),
        palette.text_muted,
    );
    painter.add(
        egui::epaint::TextShape::new(
            pos2(rect.left() + 6.0, plot.center().y + title.size().x / 2.0),
            title,
            palette.text_muted,
        )
        .with_angle(-std::f32::consts::FRAC_PI_2),
    );
    text(
        pos2(plot.right(), rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        u.format(Quantity::Speed, v_high),
    );
    text(
        pos2(rect.left() + 4.0, plot.top()),
        egui::Align2::LEFT_TOP,
        format!("+{}", u.format(Quantity::GroupSize, h_reach)),
    );
    text(
        pos2(rect.left() + 4.0, plot.bottom()),
        egui::Align2::LEFT_BOTTOM,
        format!("−{}", u.format(Quantity::GroupSize, h_reach)),
    );
    let through = |slope: f64| {
        let height = |v: f64| fit.mean_height + slope * (v - fit.mean_velocity);
        [at(v_low, height(v_low)), at(v_high, height(v_high))]
    };
    let clip = |painter: &egui::Painter| painter.with_clip_rect(plot);
    clip(&painter).line_segment(
        through(fit.slope.value),
        Stroke::new(2.0_f32, palette.accent),
    );
    if let Some(expected) = expected {
        let [from, to] = through(expected);
        clip(&painter).extend(egui::Shape::dashed_line(
            &[from, to],
            Stroke::new(1.5_f32, palette.measure),
            6.0,
            4.0,
        ));
    }
    for (v, h) in pairs {
        painter.circle_filled(at(*v, *h), 4.0, palette.text);
    }
    let key = |x: f32, colour: Color32, words: &str| {
        painter.line_segment(
            [
                pos2(x, rect.top() + 12.0),
                pos2(x + 14.0, rect.top() + 12.0),
            ],
            Stroke::new(2.0_f32, colour),
        );
        painter.text(
            pos2(x + 18.0, rect.top() + 12.0),
            egui::Align2::LEFT_CENTER,
            words,
            egui::FontId::proportional(11.0),
            palette.text_muted,
        );
    };
    key(plot.right() - 150.0, palette.accent, tr("measured"));
    if expected.is_some() {
        key(plot.right() - 74.0, palette.measure, tr("solver"));
    }
}

impl crate::groups::GroupsPage {
    /// The page, with the tools around a saved group's numbers: what they
    /// ask for comes back as actions with the page's own.
    pub fn show_with_tools(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        photos: &dyn ballistics_store::PhotoStore,
    ) -> Vec<Action> {
        let tools = &mut self.numbers.tools;
        tools.context = match &self.view {
            crate::groups::View::Group(open) => Context::of(&open.id, data).map(Box::new),
            _ => None,
        };
        let mut actions = Vec::new();
        if let Some(job) = &mut tools.picture
            && let Some(done) = job.poll(data)
        {
            tools.picture = None;
            actions.push(done);
        }
        actions.extend(self.show(ui, data, photos));
        let tools = &mut self.numbers.tools;
        let Some(context) = tools.context.take() else {
            tools.requests.clear();
            return actions;
        };
        for request in std::mem::take(&mut tools.requests) {
            match request {
                Request::SaveVelocities { shots, extra } => {
                    let mut group = context.group.clone();
                    set_shot_velocities(&mut group.marks, &shots);
                    group.extra_velocities_mps = extra;
                    actions.push(Action::SaveGroup(Box::new(crate::groups::GroupSave::of(
                        group, data,
                    ))));
                }
                Request::UseVelocity(load) => {
                    actions.push(Action::Record(Box::new(
                        crate::records::Change::UseVelocity(*load),
                    )));
                }
                Request::Picture => {
                    tools.picture = Some(export::PictureJob::start(
                        &context.group,
                        data,
                        photos,
                        units::prefs(ui.ctx()),
                    ));
                }
                Request::ShotsCsv => {
                    let csv = export::shots_csv(&[&context.group], data);
                    actions.push(Action::SaveFile(Box::new(crate::files::FileOut {
                        name: export::file_name(&context.group, data, "csv"),
                        bytes: csv.into_bytes(),
                        mime: "text/csv",
                    })));
                }
            }
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speed(label: &str) -> Unit {
        *Quantity::Speed
            .units()
            .iter()
            .find(|unit| unit.label == label)
            .unwrap()
    }

    #[test]
    fn readings_are_read_one_a_line_or_separated() {
        let mps = speed("m/s");
        let expected = vec![823.0, 825.5, 821.0];
        for text in [
            "823\n825.5\n821",
            "823 825.5 821",
            "823, 825.5, 821",
            "823;825.5;821",
            "823,825.5,821",
            " 823\t825,5  821 ",
            "823\r\n825,5\r\n821\r\n",
        ] {
            assert_eq!(parse_readings(text, mps), Ok(expected.clone()), "{text:?}");
        }
        assert_eq!(parse_readings("", mps), Ok(vec![]));
    }

    #[test]
    fn a_comma_that_could_be_a_thousands_separator_is_refused() {
        let fps = speed("fps");
        assert_eq!(
            parse_readings("2,850", fps),
            Err(ReadingError::Ambiguous("2,850".into()))
        );
        assert!(matches!(
            parse_readings("2,850 2,861", fps),
            Err(ReadingError::Ambiguous(_))
        ));
        assert!(matches!(
            parse_readings("823,4,825,1", speed("m/s")),
            Err(ReadingError::Ambiguous(_))
        ));
        assert!(matches!(
            parse_readings("82x", speed("m/s")),
            Err(ReadingError::NotANumber(_))
        ));
    }

    #[test]
    fn readings_in_fps_are_kept_in_mps_and_out_of_range_ones_refused() {
        let fps = speed("fps");
        let readings = parse_readings("2850 2861,5", fps).unwrap();
        assert!((readings[0] - 2850.0 * 0.3048).abs() < 1e-3);
        assert!((readings[1] - 2861.5 * 0.3048).abs() < 1e-3);
        let mps = speed("m/s");
        assert!(matches!(
            parse_readings("2850", mps),
            Err(ReadingError::OutOfRange(_, value)) if value == 2850.0
        ));
        assert!(matches!(
            parse_readings("82.3", mps),
            Err(ReadingError::OutOfRange(..))
        ));
    }

    #[test]
    fn readings_pair_with_the_shots_in_marking_order() {
        assert_eq!(
            pair(3, &[801.0, 802.0, 803.0, 804.0]),
            (vec![Some(801.0), Some(802.0), Some(803.0)], vec![804.0])
        );
        assert_eq!(pair(3, &[801.0]), (vec![Some(801.0), None, None], vec![]));
        let mut group = Group::on_grid(100.0, 100.0, 0);
        if let Marks::Grid(marks) = &mut group.marks {
            for x in [0.0, 1.0] {
                marks.shots.push(ballistics_store::GridShot {
                    x_mm: x,
                    y_mm: 0.0,
                    cold_bore: false,
                    called_flier: false,
                    velocity_mps: None,
                });
            }
        }
        set_shot_velocities(&mut group.marks, &[Some(800.0)]);
        assert_eq!(shot_velocities(&group.marks), vec![Some(800.0), None]);
    }

    #[test]
    fn a_string_becomes_the_loads_velocity_at_its_temperature() {
        let load = with_velocity(None, "r", "b", &[800.0, 810.0, 820.0], 5.0, false).unwrap();
        assert_eq!(load.velocities.len(), 1);
        let point = load.velocities[0];
        assert_eq!(point.velocity_mps, 810.0);
        assert_eq!(point.shots.unwrap().count, 3);
        assert_eq!(point.shots.unwrap().es_mps, 20.0);
        let again = with_velocity(Some(&load), "r", "b", &[790.0], 5.3, false).unwrap();
        assert_eq!(
            again.velocities.len(),
            1,
            "the same temperature is replaced"
        );
        assert_eq!(again.velocities[0].velocity_mps, 790.0);
        let colder = with_velocity(Some(&again), "r", "b", &[780.0], -10.0, false).unwrap();
        assert_eq!(colder.velocities.len(), 2);
        let new_barrel = with_velocity(Some(&colder), "r", "b", &[770.0], 0.0, true).unwrap();
        assert_eq!(new_barrel.velocities.len(), 1, "the earlier barrel's go");

        let mut fitted = ballistics_store::Barrel::new("r", "Proof");
        fitted.fitted_at = Some(5_000);
        let mut old = colder.clone();
        old.updated_at = 4_000;
        assert_eq!(earlier_than_barrel(&old, &fitted), 2);
        old.updated_at = 6_000;
        assert_eq!(earlier_than_barrel(&old, &fitted), 0);
    }

    fn demo() -> (Rifle, Bullet) {
        let store = ballistics_store::Store::open_in_memory().unwrap();
        let mut store = store;
        store.load_demo().unwrap();
        let rifle = store.rifles().unwrap().remove(0);
        let bullet = store
            .bullets()
            .unwrap()
            .into_iter()
            .find(|bullet| bullet.caliber == rifle.caliber)
            .unwrap();
        (rifle, bullet)
    }

    #[test]
    fn the_expected_slope_matches_the_solver_and_points_up() {
        let (rifle, bullet) = demo();
        let conditions = solver_conditions(&Conditions::default());
        let at =
            |distance: f64| expected_slope(&rifle, &bullet, None, &conditions, distance).unwrap();
        // A finite difference of the solver of its own, 10 m/s each side.
        let scenario = ballistics_shot::scenario(&rifle, &bullet, None, &conditions);
        let zeroed = solver::solve(&scenario, 310.0).unwrap();
        let zero = (zeroed.zero_elevation, zeroed.zero_windage);
        let drop = |change: f64| {
            let mut faster = scenario.clone();
            faster.load.muzzle_velocity += change;
            solver::solve_with_zero(&faster, 310.0, zero)
                .unwrap()
                .at(300.0)
                .unwrap()
                .drop
        };
        let wide = (drop(10.0) - drop(-10.0)) / 20.0 * 1000.0;
        let slope = at(300.0);
        assert!(slope > 0.0, "a faster shot lands higher");
        assert!((slope - wide).abs() < 0.02 * wide, "{slope} against {wide}");
        // In a vacuum the change is g t² / v; drag makes it a little more.
        let row = zeroed.at(300.0).unwrap();
        let vacuum = 9.81 * row.time * row.time / zeroed.muzzle_velocity * 1000.0;
        assert!(
            slope > 0.8 * vacuum && slope < 2.0 * vacuum,
            "{slope} against {vacuum}"
        );
        assert!(at(100.0) < slope, "less at a shorter distance");
    }
}

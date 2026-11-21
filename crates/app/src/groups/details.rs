//! A group's numbers, results first: its size, where it landed and what to
//! dial, then the table of spreads, the flier suggestion, the cold-bore
//! shot and the velocities.

use ballistics_store::Rifle;
use eframe::egui::{self, RichText};
use groups_core::{GroupStats, Options, Point, RadiusMethod, VelocityStats, flier_check};

use super::measure::Measured;
use crate::calculator::clicks_text;
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

/// Fewer shots than this make every number rough.
const ROUGH_BELOW: usize = 5;

/// Choices on the numbers that stay while the page is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NumbersState {
    /// Show the statistics with the called fliers counted.
    pub with_called_fliers: bool,
    /// The hit-chance circle's diameter, mrad; `None` for the unit's first
    /// preset.
    pub circle_mrad: Option<f64>,
    /// The circle's diameter on the target, mm, for a group without a
    /// distance.
    pub circle_mm: Option<f64>,
    /// The circle is typed in rather than a preset.
    pub custom_circle: bool,
}

/// What a tap on the numbers asked for.
pub enum NumbersEvent {
    /// Call this shot (by index among all the marks) a flier.
    CallFlier(usize),
}

/// What the numbers are of.
pub struct Subject<'a> {
    pub measured: &'a Measured,
    pub rifle: Option<&'a Rifle>,
    pub calibre_mm: Option<f64>,
    pub distance_m: Option<f64>,
    /// Every chronograph reading for the group.
    pub velocities: Vec<f64>,
}

impl Subject<'_> {
    /// The statistics shown: with or without the called fliers.
    pub fn stats(&self, state: &NumbersState) -> Option<&GroupStats> {
        if state.with_called_fliers {
            self.measured
                .with_called_fliers
                .as_ref()
                .or(self.measured.stats.as_ref())
        } else {
            self.measured.stats.as_ref()
        }
    }
}

fn mrad(mm: f64, distance: f64) -> f64 {
    groups_core::angles::mrad(mm, distance)
}

/// The number alone: as an angle at the distance, else on the target.
fn main_number(u: &UnitPrefs, mm: f64, distance: Option<f64>) -> String {
    match distance {
        Some(distance) => u.number(Quantity::GroupAngle, mrad(mm, distance), false),
        None => u.number(Quantity::GroupSize, mm, false),
    }
}

/// The unit [`main_number`] is in.
fn main_unit(u: &UnitPrefs, distance: Option<f64>) -> &'static str {
    match distance {
        Some(_) => u.label(Quantity::GroupAngle),
        None => u.label(Quantity::GroupSize),
    }
}

/// A size as an angle at the distance, or on the target without one.
fn main_size(u: &UnitPrefs, mm: f64, distance: Option<f64>) -> String {
    format!(
        "{} {}",
        main_number(u, mm, distance),
        main_unit(u, distance)
    )
}

fn size(u: &UnitPrefs, mm: f64) -> String {
    u.format(Quantity::GroupSize, mm)
}

/// "→ 0.17 · ↓ 0.06": how far the centre is from the aim, leaving out a
/// way that shows as nothing, or "On the aim".
pub fn impact_text(u: &UnitPrefs, impact: Point, distance: Option<f64>) -> String {
    let shown = |mm: f64| main_number(u, mm.abs(), distance);
    let zero = shown(0.0);
    let mut parts = Vec::new();
    let across = shown(impact.x);
    if across != zero {
        parts.push(format!(
            "{} {across}",
            if impact.x > 0.0 { "→" } else { "←" }
        ));
    }
    let up = shown(impact.y);
    if up != zero {
        parts.push(format!("{} {up}", if impact.y > 0.0 { "↑" } else { "↓" }));
    }
    if parts.is_empty() {
        tr("On the aim").to_owned()
    } else {
        parts.join(" · ")
    }
}

/// What to dial in the rifle's clicks to put the group on the aim: "2
/// clicks left, 1 click up". `Err` says what's missing.
pub fn dial(
    stats: &GroupStats,
    distance_m: Option<f64>,
    rifle: Option<&Rifle>,
) -> Result<String, &'static str> {
    let correction = stats.correction().ok_or(tr("No aim point marked"))?;
    let rifle = rifle.ok_or(tr("Choose the rifle"))?;
    let distance = distance_m.ok_or(tr("Give the distance"))?;
    Ok(dial_text(correction, distance, rifle))
}

/// The correction (aim − centre, mm) in the rifle's clicks.
pub fn dial_text(correction_mm: Point, distance_m: f64, rifle: &Rifle) -> String {
    let clicks = |mm: f64| -> i64 {
        let angle = ballistics_shot::from_radians(rifle.click_unit, mrad(mm, distance_m) / 1000.0);
        if rifle.click_value > 0.0 {
            (angle / rifle.click_value).round() as i64
        } else {
            0
        }
    };
    let (across, up) = (clicks(correction_mm.x), clicks(correction_mm.y));
    let mut parts = Vec::new();
    if across != 0 {
        parts.push(format!(
            "{} {}",
            clicks_text(across.abs()),
            if across > 0 { tr("right") } else { tr("left") }
        ));
    }
    if up != 0 {
        parts.push(format!(
            "{} {}",
            clicks_text(up.abs()),
            if up > 0 { tr("up") } else { tr("down") }
        ));
    }
    if parts.is_empty() {
        tr("Nothing: it's on").to_owned()
    } else {
        parts.join(", ")
    }
}

/// "0.1 mil clicks".
fn click_label(rifle: &Rifle) -> String {
    trf!(
        "{} {} clicks",
        rifle.click_value,
        ballistics_shot::unit_label(rifle.click_unit)
    )
}

fn tile(ui: &mut egui::Ui, width: f32, title: &str, big: &str, small: &[String]) {
    let palette = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(palette.raised)
        .stroke(palette.edge)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).small().color(palette.text_muted));
                ui.add(
                    egui::Label::new(RichText::new(big).size(20.0).strong().color(palette.text))
                        .wrap(),
                );
                for line in small {
                    ui.add(egui::Label::new(RichText::new(line).color(palette.text_muted)).wrap());
                }
            });
        });
}

/// "5 shots · 100 m", and a word when that is too few to go by.
pub fn heading(ui: &mut egui::Ui, stats: &GroupStats, distance: Option<f64>) {
    let u = units::prefs(ui.ctx());
    let mut text = if stats.shots == 1 {
        tr("1 shot").to_owned()
    } else {
        trf!("{} shots", stats.shots)
    };
    if let Some(distance) = distance {
        text = format!("{text} · {}", u.format(Quantity::Distance, distance));
    }
    ui.label(RichText::new(text).strong());
}

/// Says the numbers are rough when the group is small.
pub fn rough(ui: &mut egui::Ui, stats: &GroupStats) {
    if stats.shots < ROUGH_BELOW {
        ui.add(
            egui::Label::new(
                RichText::new(tr(
                    "With fewer than 5 shots these numbers are rough: take them as a first look.",
                ))
                .color(theme::palette(ui.ctx()).warning),
            )
            .wrap(),
        );
    }
}

/// The group's size: extreme spread and mean radius, side by side or one
/// above the other.
pub fn size_tiles(
    ui: &mut egui::Ui,
    subject: &Subject,
    stats: &GroupStats,
    width: f32,
    stacked: bool,
) {
    let u = units::prefs(ui.ctx());
    let distance = subject.distance_m;
    let gap = ui.spacing().item_spacing.x;
    let each = if stacked {
        width
    } else {
        ((width - gap) / 2.0).max(110.0)
    };
    let extreme = |ui: &mut egui::Ui| match stats.spread {
        Some(spread) => {
            let es = spread.extreme_spread;
            let mut small = Vec::new();
            if distance.is_some() {
                small.push(size(&u, es.centre_to_centre));
            }
            if let Some(edge) = es.edge_to_edge {
                small.push(trf!("edge to edge {}", main_size(&u, edge, distance)));
            }
            tile(
                ui,
                each,
                tr("Extreme spread"),
                &main_size(&u, es.centre_to_centre, distance),
                &small,
            );
        }
        None => tile(
            ui,
            each,
            tr("Extreme spread"),
            "—",
            &[tr("Mark a second shot").to_owned()],
        ),
    };
    let mean = |ui: &mut egui::Ui| {
        let mut small = Vec::new();
        if let Some(spread) = stats.spread {
            let range = spread.mean_radius_range;
            small.push(trf!(
                "95%: {} – {}",
                main_number(&u, range.low, distance),
                main_size(&u, range.high, distance)
            ));
        }
        tile(
            ui,
            each,
            tr("Mean radius"),
            &main_size(&u, stats.mean_radius, distance),
            &small,
        );
    };
    if stacked {
        extreme(ui);
        mean(ui);
    } else {
        ui.horizontal_top(|ui| {
            extreme(ui);
            mean(ui);
        });
    }
}

/// Where the group landed and what to dial.
pub fn aim_tiles(ui: &mut egui::Ui, subject: &Subject, stats: &GroupStats, width: f32) {
    let u = units::prefs(ui.ctx());
    let distance = subject.distance_m;
    let Some(impact) = stats.impact else {
        ui.weak(tr(
            "No aim point marked, so there is no impact or correction to dial.",
        ));
        return;
    };
    let each = ((width - ui.spacing().item_spacing.x) / 2.0).max(110.0);
    ui.horizontal_top(|ui| {
        tile(
            ui,
            each,
            tr("Impact from the aim"),
            &impact_text(&u, impact, distance),
            &[main_unit(&u, distance).to_owned()],
        );
        match dial(stats, distance, subject.rifle) {
            Ok(text) => {
                let clicks = subject.rifle.map(click_label).unwrap_or_default();
                tile(ui, each, tr("Dial"), &text, &[clicks]);
            }
            Err(missing) => tile(ui, each, tr("Dial"), "—", &[missing.to_owned()]),
        }
    });
}

/// Two choices in a frame, as the app's tabs look.
fn segmented(ui: &mut egui::Ui, value: &mut bool, labels: [&str; 2]) {
    let palette = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(palette.chrome)
        .stroke(egui::Stroke::new(1.0_f32, palette.stroke))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                for (choice, label) in [false, true].into_iter().zip(labels) {
                    let selected = *value == choice;
                    let text = if selected {
                        RichText::new(label).color(palette.on_accent)
                    } else {
                        RichText::new(label).color(palette.text)
                    };
                    let button = egui::Button::new(text)
                        .fill(if selected {
                            palette.accent
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(6);
                    if ui.add(button).clicked() {
                        *value = choice;
                    }
                }
            });
        });
}

/// Choose between the numbers without and with the called fliers, when
/// there are any.
pub fn fliers_switch(ui: &mut egui::Ui, measured: &Measured, state: &mut NumbersState) {
    if !measured.called_fliers.is_empty() {
        segmented(
            ui,
            &mut state.with_called_fliers,
            [tr("Without called fliers"), tr("With called fliers")],
        );
    }
}

/// Everything, results first, for a pane. Returns what was tapped.
pub fn show(
    ui: &mut egui::Ui,
    subject: &Subject,
    state: &mut NumbersState,
) -> Option<NumbersEvent> {
    fliers_switch(ui, subject.measured, state);
    let Some(stats) = subject.stats(state) else {
        ui.weak(if subject.measured.shots_mm.is_empty() {
            tr("The numbers appear as you mark the shots.")
        } else {
            tr("No shot counts yet: cold-bore shots and called fliers are left out.")
        });
        return None;
    };
    heading(ui, stats, subject.distance_m);
    rough(ui, stats);
    let width = ui.available_width();
    size_tiles(ui, subject, stats, width, false);
    aim_tiles(ui, subject, stats, width);
    more(ui, subject, stats, state)
}

/// The table of spreads, the hit chance, the flier suggestion, the
/// cold-bore shot and the velocities. Returns what was tapped.
pub fn more(
    ui: &mut egui::Ui,
    subject: &Subject,
    stats: &GroupStats,
    state: &mut NumbersState,
) -> Option<NumbersEvent> {
    let u = units::prefs(ui.ctx());
    let distance = subject.distance_m;
    if let Some(spread) = stats.spread {
        ui.add_space(4.0);
        egui::Grid::new("group-numbers")
            .num_columns(3)
            .striped(true)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                ui.weak("");
                ui.weak(u.label(Quantity::GroupAngle));
                ui.weak(u.label(Quantity::GroupSize));
                ui.end_row();
                let row = |ui: &mut egui::Ui, label: &str, mm: &[f64], separator: &str| {
                    ui.label(label);
                    ui.label(
                        mm.iter()
                            .map(|value| {
                                distance.map_or_else(
                                    || "—".to_owned(),
                                    |distance| {
                                        u.number(
                                            Quantity::GroupAngle,
                                            mrad(*value, distance),
                                            false,
                                        )
                                    },
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(separator),
                    );
                    ui.label(
                        mm.iter()
                            .map(|value| u.number(Quantity::GroupSize, *value, false))
                            .collect::<Vec<_>>()
                            .join(separator),
                    );
                    ui.end_row();
                };
                row(ui, tr("σ (radial)"), &[spread.sigma.value], "");
                row(
                    ui,
                    tr("CEP · R90"),
                    &[spread.cep.value, spread.r90.value],
                    " · ",
                );
                row(
                    ui,
                    tr("SD across · up"),
                    &[spread.sd_horizontal, spread.sd_vertical],
                    " · ",
                );
                row(
                    ui,
                    tr("Width × height"),
                    &[stats.width, stats.height],
                    " × ",
                );
            });
        ui.add(
            egui::Label::new(
                RichText::new(match spread.radius_method {
                    RadiusMethod::Circular => tr(
                        "CEP and R90 from σ: the group is round enough. Half the shots land within \
                         CEP of the centre, nine in ten within R90.",
                    ),
                    RadiusMethod::Grubbs => tr(
                        "CEP and R90 by Grubbs's method: the group is clearly longer one way. Half \
                         the shots land within CEP of the centre, nine in ten within R90.",
                    ),
                })
                .weak(),
            )
            .wrap(),
        );
        hit_chance(ui, stats, distance, state, &u);
    }
    let event = flier_card(ui, subject, stats, state, &u);
    cold_bore(ui, subject, stats, &u);
    velocity(ui, &subject.velocities, &u);
    event
}

/// The hit-chance circles offered in the angle unit, as diameters in mrad.
fn circle_presets(u: &UnitPrefs) -> [(f64, &'static str); 2] {
    match u.label(Quantity::GroupAngle) {
        "MOA" => [
            (groups_core::angles::mrad_from_moa(0.5), "½ MOA"),
            (groups_core::angles::mrad_from_moa(1.0), "1 MOA"),
        ],
        "cm/100 m" => [(0.2, "2 cm/100 m"), (0.3, "3 cm/100 m")],
        _ => [(0.2, "0.2 mil"), (0.3, "0.3 mil")],
    }
}

/// The hit-chance circles offered on the target, in the size unit, as
/// diameters in mm.
fn circle_presets_on_target(u: &UnitPrefs) -> [(f64, &'static str); 2] {
    match u.label(Quantity::GroupSize) {
        "in" => [(12.7, "½ in"), (25.4, "1 in")],
        _ => [(25.0, "25 mm"), (50.0, "50 mm")],
    }
}

fn hit_chance(
    ui: &mut egui::Ui,
    stats: &GroupStats,
    distance: Option<f64>,
    state: &mut NumbersState,
    u: &UnitPrefs,
) {
    let (presets, current) = match distance {
        Some(_) => {
            let presets = circle_presets(u);
            let current = state.circle_mrad.unwrap_or(presets[1].0);
            (presets, current)
        }
        None => {
            let presets = circle_presets_on_target(u);
            let current = state.circle_mm.unwrap_or(presets[0].0);
            (presets, current)
        }
    };
    let diameter_mm = match distance {
        Some(distance) => groups_core::angles::mm_at(current, distance),
        None => current,
    };
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Hit chance in a circle of"));
        for (value, label) in presets {
            let chosen = !state.custom_circle && (current - value).abs() < 1e-9;
            if ui.selectable_label(chosen, label).clicked() {
                state.custom_circle = false;
                match distance {
                    Some(_) => state.circle_mrad = Some(value),
                    None => state.circle_mm = Some(value),
                }
            }
        }
        if ui
            .selectable_label(state.custom_circle, tr("Custom"))
            .clicked()
        {
            state.custom_circle = true;
        }
        if state.custom_circle {
            let mut value = current;
            let changed =
                match distance {
                    Some(_) => units::drag(ui, &mut value, Quantity::GroupAngle, 0.01..=50.0, 0.01)
                        .changed(),
                    None => units::drag(ui, &mut value, Quantity::GroupSize, 1.0..=2000.0, 0.5)
                        .changed(),
                };
            if changed {
                match distance {
                    Some(_) => state.circle_mrad = Some(value),
                    None => state.circle_mm = Some(value),
                }
            }
        }
        if let Some(chance) = stats.chance_in_circle(stats.centre, diameter_mm / 2.0) {
            ui.label(RichText::new(format!("{:.0} %", chance * 100.0)).strong());
        }
    });
    ui.add(
        egui::Label::new(
            RichText::new(tr(
                "Centred on the group's centre: how often a shot lands inside once the zero is \
                 right.",
            ))
            .weak(),
        )
        .wrap(),
    );
}

fn flier_card(
    ui: &mut egui::Ui,
    subject: &Subject,
    stats: &GroupStats,
    state: &NumbersState,
    u: &UnitPrefs,
) -> Option<NumbersEvent> {
    if state.with_called_fliers {
        return None;
    }
    let measured = subject.measured;
    let points: Vec<Point> = measured
        .counted
        .iter()
        .map(|&i| measured.shots_mm[i])
        .collect();
    let options = Options {
        aim: measured.aim_mm,
        calibre_mm: subject.calibre_mm,
        distance_m: subject.distance_m,
    };
    let flier = flier_check(&points, options).ok().flatten()?;
    let shot = measured.counted[flier.shot];
    let palette = theme::palette(ui.ctx());
    let mut called = false;
    ui.add_space(6.0);
    egui::Frame::new()
        .fill(palette.warning_tint)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(trf!("⚑ Shot {} pulls the group", shot + 1))
                    .strong()
                    .color(palette.text),
            );
            let without = &flier.without;
            let distance = subject.distance_m;
            let es = without
                .spread
                .map(|spread| main_size(u, spread.extreme_spread.centre_to_centre, distance));
            ui.add(
                egui::Label::new(match es {
                    Some(es) => trf!(
                        "Without it: ES {} · MR {} (now MR {})",
                        es,
                        main_size(u, without.mean_radius, distance),
                        main_size(u, stats.mean_radius, distance)
                    ),
                    None => trf!(
                        "Without it: MR {}",
                        main_size(u, without.mean_radius, distance)
                    ),
                })
                .wrap(),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(tr(
                        "It lies further out than the other shots' spread explains. Call it a \
                         flier only if you know the shot went wrong.",
                    ))
                    .weak(),
                )
                .wrap(),
            );
            called = ui.button(tr("Call it a flier")).clicked();
        });
    called.then_some(NumbersEvent::CallFlier(shot))
}

fn cold_bore(ui: &mut egui::Ui, subject: &Subject, stats: &GroupStats, u: &UnitPrefs) {
    let measured = subject.measured;
    for &index in &measured.cold_bore {
        let shot = measured.shots_mm[index];
        let from_centre = main_size(u, shot.distance(stats.centre), subject.distance_m);
        let text = match measured.aim_mm {
            Some(aim) => trf!(
                "Cold bore (shot {}): {} from the aim, {} from the group's centre",
                index + 1,
                main_size(u, shot.distance(aim), subject.distance_m),
                from_centre
            ),
            None => trf!(
                "Cold bore (shot {}): {} from the group's centre",
                index + 1,
                from_centre
            ),
        };
        ui.add_space(4.0);
        ui.add(egui::Label::new(text).wrap());
    }
}

fn velocity(ui: &mut egui::Ui, readings: &[f64], u: &UnitPrefs) {
    let Ok(stats) = VelocityStats::new(readings) else {
        return;
    };
    let mut parts = vec![trf!("avg {}", u.format(Quantity::Speed, stats.average))];
    if let Some(sd) = stats.sd {
        parts.push(trf!("SD {}", u.number(Quantity::Speed, sd.value, false)));
    }
    parts.push(trf!(
        "ES {}",
        u.number(Quantity::Speed, stats.extreme_spread, false)
    ));
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(trf!("Velocity, {} readings", stats.shots)).strong());
        ui.label(parts.join(" · "));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::{GridShot, Marks};

    fn demo_rifle() -> Rifle {
        let mut store = ballistics_store::Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        let rifle = store.rifles().unwrap().remove(0);
        assert_eq!(
            (rifle.click_unit, rifle.click_value),
            (ballistics_store::ClickUnit::Mil, 0.1)
        );
        rifle
    }

    #[test]
    fn the_correction_reads_in_the_rifles_clicks() {
        let rifle = demo_rifle();
        // 10 mm at 100 m is 0.1 mil: one click of 0.1 mil.
        assert_eq!(
            dial_text(Point::new(-20.0, 10.0), 100.0, &rifle),
            "2 clicks left, 1 click up"
        );
        assert_eq!(
            dial_text(Point::new(0.0, -30.0), 100.0, &rifle),
            "3 clicks down"
        );
        assert_eq!(
            dial_text(Point::new(2.0, 1.0), 100.0, &rifle),
            "Nothing: it's on"
        );
    }

    #[test]
    fn a_group_right_and_low_of_the_aim_is_dialled_left_and_up() {
        let rifle = demo_rifle();
        let mut group = ballistics_store::Group::on_grid(200.0, 200.0, 1);
        if let Marks::Grid(grid) = &mut group.marks {
            grid.aim_mm = Some([0.0, 0.0]);
            grid.shots = [(19.0, -11.0), (21.0, -9.0), (20.0, -10.0)]
                .map(|(x_mm, y_mm)| GridShot {
                    x_mm,
                    y_mm,
                    cold_bore: false,
                    called_flier: false,
                    velocity_mps: None,
                })
                .to_vec();
        }
        let measured = super::super::measure::measure(&group.marks, None, None, Some(100.0))
            .expect("a grid group measures");
        let stats = measured.stats.as_ref().unwrap();
        assert_eq!(
            dial(stats, Some(100.0), Some(&rifle)),
            Ok("2 clicks left, 1 click up".to_owned())
        );
        let u = UnitPrefs::metric();
        assert_eq!(
            impact_text(&u, stats.impact.unwrap(), Some(100.0)),
            "→ 0.20 · ↓ 0.10"
        );
        assert_eq!(dial(stats, Some(100.0), None), Err("Choose the rifle"));
    }

    #[test]
    fn an_impact_too_small_to_show_is_left_out() {
        let u = UnitPrefs::metric();
        assert_eq!(impact_text(&u, Point::new(0.2, 2.0), Some(100.0)), "↑ 0.02");
        assert_eq!(
            impact_text(&u, Point::new(-0.2, 0.3), Some(100.0)),
            "On the aim"
        );
        assert_eq!(impact_text(&u, Point::new(-3.0, 0.0), None), "← 3.0");
    }
}

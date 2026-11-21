//! Trend: a group size or velocity against time, charge, seating,
//! temperature, pressure or barrel rounds, one line per load.

use ballistics_store::Bullet;
use eframe::egui::{self, RichText};
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Line, LineStyle, Plot, PlotPoints, Points};
use groups_core::{AnalysisError, TemperatureSensitivity};

use super::pool::{self, Against, Metric, TrendData, TrendFilters};
use super::{Memo, key_of};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

pub struct State {
    pub metric: Metric,
    pub against: Against,
    pub filters: TrendFilters,
    /// Velocity readings as box plots.
    pub boxes: bool,
    /// The filters start on the last used rifle, once.
    started: bool,
    memo: Memo<TrendData>,
}

impl Default for State {
    fn default() -> Self {
        State {
            metric: Metric::MeanRadius,
            against: Against::Date,
            filters: TrendFilters::default(),
            boxes: false,
            started: false,
            memo: Memo::default(),
        }
    }
}

pub fn metric_name(metric: Metric) -> &'static str {
    match metric {
        Metric::MeanRadius => tr("Mean radius"),
        Metric::ExtremeSpread => tr("Extreme spread"),
        Metric::Sigma => tr("σ (radial)"),
        Metric::VelocityAverage => tr("Velocity, average"),
        Metric::VelocitySd => tr("Velocity SD"),
        Metric::ColdBoreOffset => tr("Cold-bore shot from aim"),
    }
}

pub fn against_name(against: Against) -> &'static str {
    match against {
        Against::Order => tr("Order shot"),
        Against::Date => tr("Date"),
        Against::Charge => tr("Charge"),
        Against::Seating => tr("Seating (CBTO)"),
        Against::Temperature => tr("Temperature"),
        Against::Pressure => tr("Pressure"),
        Against::BarrelRounds => tr("Rounds on the barrel"),
        Against::SinceClean => tr("Rounds since clean"),
    }
}

/// A value along `against` in the hunter's unit: grains or grams, mm or
/// inches, °C or °F; counts and days as they are.
pub fn shown_x(u: &UnitPrefs, against: Against, base: f64) -> f64 {
    against
        .quantity()
        .map_or(base, |quantity| u.show(quantity, base))
}

/// A value of `metric` in the hunter's unit.
pub fn shown_y(u: &UnitPrefs, metric: Metric, base: f64) -> f64 {
    u.show(metric.quantity(), base)
}

fn axis_label(u: &UnitPrefs, against: Against) -> String {
    match against.quantity() {
        Some(quantity) => format!("{} ({})", against_name(against), u.label(quantity)),
        None => against_name(against).to_owned(),
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    data: &Data,
    stamp: u64,
    phone: bool,
    actions: &mut Vec<Action>,
) {
    let u = units::prefs(ui.ctx());
    if !std::mem::replace(&mut state.started, true) {
        state.filters.rifle_id = data
            .groups
            .groups
            .iter()
            .filter(|group| {
                group
                    .rifle_id
                    .as_ref()
                    .is_some_and(|id| data.rifles.iter().any(|rifle| &rifle.id == id))
            })
            .max_by_key(|group| group.taken_at)
            .and_then(|group| group.rifle_id.clone());
    }
    controls(ui, state, data, phone);
    let key = key_of(stamp, (&state.filters, state.metric, state.against));
    let (metric, against, filters) = (state.metric, state.against, state.filters.clone());
    let trend = state
        .memo
        .get(key, || pool::trend(data, metric, against, &filters));
    ui.add_space(6.0);
    if trend.points() < 2 {
        too_little(ui, &trend, metric, against);
        return;
    }
    chart(ui, &trend, state, data, &u, phone);
    colour_key(ui, &trend, data, phone);
    if trend.missing_metric + trend.missing_axis > 0 {
        super::weak(
            ui,
            &trf!(
                "{} of {} groups are on the chart: {} have no {}, {} no {}.",
                trend.points(),
                trend.groups,
                trend.missing_metric,
                metric_name(metric).to_lowercase(),
                trend.missing_axis,
                against_name(against).to_lowercase()
            ),
        );
    }
    if matches!(metric, Metric::MeanRadius | Metric::Sigma) {
        super::weak(ui, tr("Bars: the 95% range of each group's size."));
    }
    if metric == Metric::VelocityAverage && against == Against::Temperature {
        sensitivities(ui, &trend, &state.filters, data, &u, actions);
    }
}

fn controls(ui: &mut egui::Ui, state: &mut State, data: &Data, phone: bool) {
    let width = if phone {
        (ui.available_width() - 40.0) / 2.0
    } else {
        200.0
    };
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("trend-metric")
            .selected_text(metric_name(state.metric))
            .show_ui(ui, |ui| {
                for metric in Metric::ALL {
                    ui.selectable_value(&mut state.metric, metric, metric_name(metric));
                }
            });
        ui.label(tr("against"));
        egui::ComboBox::from_id_salt("trend-against")
            .selected_text(against_name(state.against))
            .show_ui(ui, |ui| {
                for against in Against::ALL {
                    ui.selectable_value(&mut state.against, against, against_name(against));
                }
            });
    });
    let filters = &mut state.filters;
    let groups = &data.groups.groups;
    ui.horizontal_wrapped(|ui| {
        let rifles: Vec<(String, String)> = data
            .rifles
            .iter()
            .filter(|rifle| {
                groups
                    .iter()
                    .any(|g| g.rifle_id.as_ref() == Some(&rifle.id))
            })
            .map(|rifle| (rifle.id.clone(), rifle.name.clone()))
            .collect();
        super::pick(
            ui,
            "trend-rifle",
            &mut filters.rifle_id,
            tr("All rifles"),
            &rifles,
            width,
        );
        let barrels: Vec<(String, String)> = data
            .groups
            .barrels
            .iter()
            .filter(|barrel| {
                filters
                    .rifle_id
                    .as_ref()
                    .is_none_or(|rifle| &barrel.rifle_id == rifle)
            })
            .map(|barrel| (barrel.id.clone(), barrel.name.clone()))
            .collect();
        if !barrels.is_empty() {
            super::pick(
                ui,
                "trend-barrel",
                &mut filters.barrel_id,
                tr("All barrels"),
                &barrels,
                width,
            );
        }
        let loads: Vec<(String, String)> = data
            .bullets
            .iter()
            .filter(|bullet| {
                groups.iter().any(|g| {
                    g.bullet_id.as_ref() == Some(&bullet.id)
                        && filters
                            .rifle_id
                            .as_ref()
                            .is_none_or(|rifle| g.rifle_id.as_ref() == Some(rifle))
                })
            })
            .map(|bullet| (bullet.id.clone(), bullet.display_name()))
            .collect();
        super::pick(
            ui,
            "trend-load",
            &mut filters.bullet_id,
            tr("All loads"),
            &loads,
            width,
        );
        let mut tags: Vec<String> = groups.iter().flat_map(|g| g.tags.iter().cloned()).collect();
        tags.sort_by_key(|tag| tag.to_lowercase());
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        if !tags.is_empty() {
            let options: Vec<(String, String)> =
                tags.into_iter().map(|tag| (tag.clone(), tag)).collect();
            super::pick(
                ui,
                "trend-tag",
                &mut filters.tag,
                tr("All tags"),
                &options,
                width,
            );
        }
        ui.checkbox(&mut filters.ladders, tr("Ladder rungs too"))
            .on_hover_text(tr("Trends leave load development out unless you ask"));
        if state.metric == Metric::VelocityAverage {
            ui.checkbox(&mut state.boxes, tr("Box plots"))
                .on_hover_text(tr(
                    "Each group's readings: lowest, quartiles, median, highest",
                ));
        }
    });
}

/// Says what is missing for a chart.
fn too_little(ui: &mut egui::Ui, trend: &TrendData, metric: Metric, against: Against) {
    if trend.groups == 0 {
        super::weak(ui, tr("No group matches the filters."));
        return;
    }
    let mut lines = vec![trf!(
        "A trend needs 2 groups or more with both values; {} of {} have them.",
        trend.points(),
        trend.groups
    )];
    if trend.missing_metric > 0 {
        lines.push(
            match metric {
                Metric::MeanRadius | Metric::ExtremeSpread | Metric::Sigma => {
                    tr("Sizes need groups of 2 shots or more with a distance.")
                }
                Metric::VelocityAverage => {
                    tr("Velocities come from the chronograph readings saved with the groups.")
                }
                Metric::VelocitySd => tr("A velocity SD needs 2 readings or more in a group."),
                Metric::ColdBoreOffset => {
                    tr("Mark the cold-bore shot in groups with an aim point and a distance.")
                }
            }
            .to_owned(),
        );
    }
    if trend.missing_axis > 0 {
        lines.push(
            match against {
                Against::Order | Against::Date => "",
                Against::Charge => tr(
                    "Give the load's recipe its charge on the Ammunition page, or include ladder \
                     rungs.",
                ),
                Against::Seating => tr(
                    "Give the load's recipe its CBTO on the Ammunition page, or include ladder \
                     rungs.",
                ),
                Against::Temperature => tr("Give the session, or the group, its temperature."),
                Against::Pressure => tr("Give the session, or the group, its air pressure."),
                Against::BarrelRounds => {
                    tr("Groups need a barrel; rounds come from its round log.")
                }
                Against::SinceClean => tr("Log a clean of the barrel: the rounds count from it."),
            }
            .to_owned(),
        );
    }
    for line in lines.iter().filter(|line| !line.is_empty()) {
        super::weak(ui, line);
    }
}

/// "Norma Oryx 11.7 g (181 gr) · Browning X-Bolt · Proof 24\"".
fn series_name(series: &pool::Series, data: &Data) -> String {
    let mut parts = vec![super::bullet_name(data, series.bullet_id.as_deref())];
    if let Some(rifle) = series
        .rifle_id
        .as_ref()
        .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
    {
        parts.push(rifle.name.clone());
    }
    if let Some(barrel) = series
        .barrel_id
        .as_ref()
        .and_then(|id| data.groups.barrels.iter().find(|barrel| &barrel.id == id))
    {
        parts.push(barrel.name.clone());
    }
    parts.join(" · ")
}

fn chart(
    ui: &mut egui::Ui,
    trend: &TrendData,
    state: &State,
    data: &Data,
    u: &UnitPrefs,
    phone: bool,
) {
    let (metric, against) = (state.metric, state.against);
    let palette = theme::palette(ui.ctx());
    let x = |base: f64| shown_x(u, against, base);
    let y = |base: f64| shown_y(u, metric, base);
    let xs: Vec<f64> = trend
        .series
        .iter()
        .flat_map(|series| series.points.iter().map(|point| x(point.x)))
        .collect();
    let (low, high) = xs
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let box_width = ((high - low) * 0.03).max(0.2);
    let country = data.country;
    let mut plot = Plot::new("trend")
        .height(if phone { 240.0 } else { 300.0 })
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_double_click_reset(false)
        .x_axis_label(axis_label(u, against))
        .y_axis_label(format!(
            "{} ({})",
            metric_name(metric),
            u.label(metric.quantity())
        ));
    if metric != Metric::VelocityAverage {
        plot = plot.include_y(0.0);
    }
    // Room beyond the first and last points, so their labels aren't cut.
    let pad = ((high - low) * 0.06).max(0.5);
    plot = plot.include_x(low - pad).include_x(high + pad);
    if against == Against::Date {
        plot = plot.x_axis_formatter(move |mark, _| {
            let ms = (mark.value * 86_400_000.0) as i64;
            let today = super::super::local_ms(ballistics_store::model::now_ms(), country)
                .div_euclid(86_400_000);
            let day = super::super::local_ms(ms, country).div_euclid(86_400_000);
            super::super::library::day_name(day, today)
        });
    }
    plot.show(ui, |plot| {
        for series in &trend.series {
            let colour = super::colour(series.colour);
            let name = series_name(series, data);
            let mut points: Vec<[f64; 2]> = series
                .points
                .iter()
                .map(|point| [x(point.x), y(point.y)])
                .collect();
            if !matches!(against, Against::Order | Against::Date) {
                points.sort_by(|a, b| a[0].total_cmp(&b[0]));
            }
            if points.len() >= 2 {
                plot.line(
                    Line::new(name.clone(), PlotPoints::from(points.clone()))
                        .allow_band(false)
                        .color(colour)
                        .width(1.5_f32),
                );
            }
            plot.points(
                Points::new(name.clone(), PlotPoints::from(points))
                    .color(colour)
                    .radius(4.0),
            );
            for point in &series.points {
                if let Some(range) = point.range {
                    plot.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![
                                [x(point.x), y(range.low)],
                                [x(point.x), y(range.high)],
                            ]),
                        )
                        .allow_band(false)
                        .color(colour.gamma_multiply(0.7))
                        .width(1.0_f32),
                    );
                }
            }
            if state.boxes && metric == Metric::VelocityAverage {
                let boxes: Vec<BoxElem> = series
                    .points
                    .iter()
                    .filter_map(|point| {
                        let spread = point.spread?;
                        Some(
                            BoxElem::new(
                                x(point.x),
                                BoxSpread::new(
                                    y(spread.min),
                                    y(spread.q1),
                                    y(spread.median),
                                    y(spread.q3),
                                    y(spread.max),
                                ),
                            )
                            .box_width(box_width)
                            .whisker_width(box_width * 0.6),
                        )
                    })
                    .collect();
                plot.box_plot(BoxPlot::new("", boxes).color(colour));
            }
            if let Some(Ok(fit)) = &series.sensitivity {
                for (slope, style, alpha) in [
                    (fit.per_degree.value, LineStyle::Solid, 1.0),
                    (fit.per_degree.range.low, LineStyle::dashed_dense(), 0.5),
                    (fit.per_degree.range.high, LineStyle::dashed_dense(), 0.5),
                ] {
                    let at = |c: f64| fit.mean_velocity_mps + slope * (c - fit.mean_temperature_c);
                    let (cold, warm) = series
                        .points
                        .iter()
                        .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                            (lo.min(p.x), hi.max(p.x))
                        });
                    plot.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![[x(cold), y(at(cold))], [x(warm), y(at(warm))]]),
                        )
                        .allow_band(false)
                        .color(colour.gamma_multiply(alpha))
                        .style(style)
                        .width(1.0_f32),
                    );
                }
            }
        }
        let _ = palette;
    });
}

/// Which colour is which load.
fn colour_key(ui: &mut egui::Ui, trend: &TrendData, data: &Data, phone: bool) {
    let entry = |ui: &mut egui::Ui, series: &pool::Series| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 5.0, super::colour(series.colour));
            ui.add(
                egui::Label::new(series_name(series, data)).wrap_mode(egui::TextWrapMode::Truncate),
            );
        });
    };
    if phone {
        for series in &trend.series {
            entry(ui, series);
        }
    } else {
        ui.horizontal_wrapped(|ui| {
            for series in &trend.series {
                entry(ui, series);
                ui.add_space(8.0);
            }
        });
    }
}

fn fit_error(error: &AnalysisError, u: &UnitPrefs) -> String {
    let span = |c: f64| {
        let unit = u.unit(Quantity::Temperature);
        format!("{:.0} {}", unit.show(c) - unit.show(0.0), unit.label)
    };
    match error {
        AnalysisError::TooFew { need, have, .. } => trf!(
            "needs {} sessions at different temperatures; has {}",
            need,
            have
        ),
        AnalysisError::TooNarrow { need_c, have_c } => trf!(
            "the sessions span {}; needs {}",
            span(*have_c),
            span(*need_c)
        ),
        other => other.to_string(),
    }
}

/// Each load's powder temperature sensitivity, and a way to give it to
/// the bullet the calculator uses.
fn sensitivities(
    ui: &mut egui::Ui,
    trend: &TrendData,
    filters: &TrendFilters,
    data: &Data,
    u: &UnitPrefs,
    actions: &mut Vec<Action>,
) {
    // The bullet's sensitivity is shared by every rifle that shoots it: it
    // is written only from one rifle's (and barrel's) sessions.
    let one_barrel = filters.rifle_id.as_ref().is_some_and(|rifle| {
        filters.barrel_id.is_some()
            || !data
                .groups
                .barrels
                .iter()
                .any(|barrel| &barrel.rifle_id == rifle)
    });
    ui.add_space(6.0);
    ui.label(RichText::new(tr("Temperature sensitivity")).strong());
    for series in &trend.series {
        let name = series_name(series, data);
        match &series.sensitivity {
            Some(Ok(fit)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&name).color(super::colour(series.colour)));
                    ui.label(sensitivity_text(fit, u));
                });
                let bullet: Option<&Bullet> = series
                    .bullet_id
                    .as_ref()
                    .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id));
                let rifle_id = series.rifle_id.as_deref();
                match (bullet, rifle_id) {
                    (Some(bullet), Some(rifle_id))
                        if pool::load_sets_own_sensitivity(data, rifle_id, bullet) =>
                    {
                        super::weak(
                            ui,
                            tr(
                                "In this rifle the calculator takes the load's sensitivity from \
                                its own velocities at several temperatures, not from the bullet: \
                                add a velocity at another temperature to the load instead.",
                            ),
                        );
                    }
                    (Some(bullet), Some(_)) if one_barrel => {
                        let id = format!("trend-use-{}", bullet.id);
                        let question = trf!(
                            "Write {} as {}'s velocity change per degree (now {})? Every rifle \
                             without velocities of its own at several temperatures uses it.",
                            u.format(Quantity::Sensitivity, fit.per_degree.value),
                            bullet.display_name(),
                            u.format(Quantity::Sensitivity, bullet.velocity_per_degree)
                        );
                        if super::confirm(ui, &id, tr("Use in calculator"), &question, false) {
                            let bullet = pool::bullet_with_sensitivity(bullet, fit);
                            actions.push(Action::SaveBullet(Box::new(bullet)));
                        }
                    }
                    (Some(_), _) => super::weak(
                        ui,
                        tr("Choose one rifle and one barrel to use this in the calculator."),
                    ),
                    (None, _) => {}
                }
            }
            Some(Err(error)) => {
                super::weak(ui, &format!("{name}: {}", fit_error(error, u)));
            }
            None => {}
        }
    }
    super::weak(
        ui,
        tr(
            "Fitted from each session's average, weighted by its shots: needs 3 sessions \
            spanning 10 °C or more. The dashed lines are its 95% range.",
        ),
    );
}

pub(super) fn sensitivity_text(fit: &TemperatureSensitivity, u: &UnitPrefs) -> String {
    let unit = u.unit(Quantity::Temperature);
    let span = format!(
        "{:.0} {}",
        unit.show(fit.span_c) - unit.show(0.0),
        unit.label
    );
    trf!(
        "{} (95%: {} to {}), {:.2}% per {}, from {} sessions over {}",
        u.format(Quantity::Sensitivity, fit.per_degree.value),
        u.number(Quantity::Sensitivity, fit.per_degree.range.low, false),
        u.number(Quantity::Sensitivity, fit.per_degree.range.high, false),
        fit.percent_per_degree.value / (unit.show(1.0) - unit.show(0.0)),
        unit.label,
        fit.sessions,
        span
    )
}

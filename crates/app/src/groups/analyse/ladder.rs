//! Ladder: load development. Groups at a series of charges or seating
//! depths, each rung's impact with its noise, the runs where the impact
//! holds still, and a way to make the middle of a run a real load.

use ballistics_store::{Bullet, BulletKind, Ladder, LadderVariable};
use eframe::egui::{self, RichText};
use egui_plot::{GridMark, Line, LineStyle, Plot, PlotPoints, Points, Span};
use groups_core::{AnalysisError, LadderKey, LadderRun};

use super::pool::{self, LadderData};
use super::{Memo, angle, key_of};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

pub struct State {
    /// The ladder shown; `None` is the newest.
    pub open: Option<String>,
    pub key: LadderKey,
    /// A new ladder being made.
    creating: Option<NewLadder>,
    /// The run chosen in the list, by its first rung and length.
    run: Option<(usize, usize)>,
    /// Where "make it my load" writes: `None` is a new bullet.
    target: Option<String>,
    memo: Memo<LadderData>,
}

impl Default for State {
    fn default() -> Self {
        State {
            open: None,
            key: LadderKey::Default,
            creating: None,
            run: None,
            target: None,
            memo: Memo::default(),
        }
    }
}

struct NewLadder {
    rifle_id: Option<String>,
    bullet_id: Option<String>,
    variable: LadderVariable,
    name: String,
}

/// The ladder's name, or its rifle and bullet.
pub fn name(ladder: &Ladder, data: &Data) -> String {
    if !ladder.name.trim().is_empty() {
        return ladder.name.clone();
    }
    let what = match ladder.variable {
        LadderVariable::Charge => tr("charge ladder"),
        LadderVariable::Seating => tr("seating ladder"),
    };
    format!(
        "{} · {}",
        super::bullet_name(data, Some(&ladder.bullet_id)),
        what
    )
}

/// The quantity a ladder's rung values are shown in.
pub fn quantity(variable: LadderVariable) -> Quantity {
    match variable {
        LadderVariable::Charge => Quantity::PowderCharge,
        LadderVariable::Seating => Quantity::BulletLength,
    }
}

fn key_number(key: LadderKey) -> u8 {
    match key {
        LadderKey::Default => 0,
        LadderKey::Vertical => 1,
        LadderKey::Horizontal => 2,
        LadderKey::Sigma => 3,
    }
}

/// Returns the groups of a run to pool, when asked.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    data: &Data,
    stamp: u64,
    phone: bool,
    actions: &mut Vec<Action>,
) -> Option<Vec<String>> {
    let u = units::prefs(ui.ctx());
    let ladders = &data.groups.ladders;
    let open = state
        .open
        .as_ref()
        .and_then(|id| ladders.iter().find(|ladder| &ladder.id == id))
        .or_else(|| ladders.iter().max_by_key(|ladder| ladder.updated_at));
    ui.horizontal_wrapped(|ui| {
        if let Some(open) = open {
            let mut chosen = open.id.clone();
            egui::ComboBox::from_id_salt("ladder-open")
                .selected_text(name(open, data))
                .width(ui.available_width().clamp(140.0, 280.0))
                .truncate()
                .show_ui(ui, |ui| {
                    for ladder in ladders {
                        ui.selectable_value(&mut chosen, ladder.id.clone(), name(ladder, data));
                    }
                });
            if chosen != open.id {
                state.open = Some(chosen);
                state.run = None;
            }
        }
        if state.creating.is_none() && ui.button(tr("+ New ladder")).clicked() {
            let rifle_id = data.rifles.first().map(|rifle| rifle.id.clone());
            state.creating = Some(NewLadder {
                rifle_id,
                bullet_id: None,
                variable: LadderVariable::Charge,
                name: String::new(),
            });
        }
    });
    if state.creating.is_some() {
        create(ui, state, data, actions);
    }
    let Some(ladder) = open else {
        if state.creating.is_none() {
            super::weak(
                ui,
                tr(
                    "A ladder is a load development: groups of one bullet at a series of charges \
                    or seating depths. Start one, then save each rung's group with “Part of a \
                    ladder” in its details.",
                ),
            );
        }
        return None;
    };
    let key = state.key;
    let result = state
        .memo
        .get(key_of(stamp, (&ladder.id, key_number(key))), || {
            pool::ladder(data, ladder, key)
        });
    ui.add_space(6.0);
    header(ui, ladder, &result, data, &u);
    let worked = match &result.ladder {
        None => {
            super::weak(
                ui,
                tr(
                    "No groups on this ladder yet: save each rung's group with “Part of a \
                    ladder” in its details, with its charge or seating.",
                ),
            );
            return None;
        }
        Some(Err(error)) => {
            let text = match error {
                AnalysisError::NoSpreadEstimate => tr(
                    "The rungs are single shots: their noise can't be measured. Shoot rungs \
                     of 2 shots or more.",
                )
                .to_owned(),
                other => trf!("Can't be worked out: {}", other),
            };
            super::weak(ui, &text);
            return None;
        }
        Some(Ok(worked)) => worked,
    };
    let mut chosen_key = state.key;
    super::chips(
        ui,
        &mut chosen_key,
        &[
            (LadderKey::Default, tr("Vertical")),
            (LadderKey::Horizontal, tr("Horizontal")),
            (LadderKey::Sigma, tr("Group size")),
        ],
    );
    if chosen_key != state.key {
        state.key = chosen_key;
        state.run = None;
    }
    let run = chosen_run(state, worked);
    chart(ui, worked, run.as_ref(), ladder.variable, &u, phone);
    if worked.rungs.iter().any(|rung| rung.velocity.is_some()) {
        velocity_chart(ui, worked, ladder.variable, &u, phone);
    }
    ui.add_space(6.0);
    ui.label(RichText::new(tr("Flattest runs")).strong());
    let mut picked = None;
    let mut listed: Vec<&LadderRun> = worked.best.iter().collect();
    listed.sort_by_key(|run| (!run.flat, std::cmp::Reverse(run.rungs)));
    for best in listed {
        let selected = run.is_some_and(|run| run.first == best.first && run.rungs == best.rungs);
        if run_row(ui, worked, best, ladder.variable, selected, &u) {
            picked = Some((best.first, best.rungs));
        }
    }
    if picked.is_some() {
        state.run = picked;
    }
    if worked.best.is_empty() {
        super::weak(ui, tr("A run needs 3 rungs or more."));
    }
    super::note(
        ui,
        tr(
            "Most flat spots are noise: with 3-shot rungs a ladder that climbs steadily shows a \
            flat run almost every time. Read the error bars, and shoot the promising run again \
            with more shots before believing it.",
        ),
    );
    let mut pool_run = None;
    if let Some(run) = run {
        ui.add_space(8.0);
        promote(ui, state, ladder, worked, &run, data, &u, actions);
        if ui.button(tr("Composite of this run")).clicked() {
            let ids: Vec<String> = worked.rungs[run.first..run.first + run.rungs]
                .iter()
                .flat_map(|rung| rung.groups.iter().map(|&g| result.ids[g].clone()))
                .collect();
            pool_run = Some(ids);
        }
    }
    if !result.left_out.is_empty() {
        super::weak(
            ui,
            &trf!(
                "{} groups left out: without an aim point, a distance or numbers.",
                result.left_out.len()
            ),
        );
    }
    pool_run
}

/// The run the list has chosen, or the longest flat one, or the first.
fn chosen_run(state: &State, worked: &groups_core::Ladder) -> Option<LadderRun> {
    if let Some((first, rungs)) = state.run
        && let Some(run) = worked
            .best
            .iter()
            .find(|run| run.first == first && run.rungs == rungs)
    {
        return Some(*run);
    }
    worked
        .best
        .iter()
        .rev()
        .find(|run| run.flat)
        .or_else(|| worked.best.first())
        .copied()
}

fn create(ui: &mut egui::Ui, state: &mut State, data: &Data, actions: &mut Vec<Action>) {
    let Some(new) = &mut state.creating else {
        return;
    };
    let palette = theme::palette(ui.ctx());
    let mut done = None;
    egui::Frame::new()
        .fill(palette.raised)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("New ladder")).strong());
            crate::widgets::grid(ui, "new-ladder", |ui| {
                ui.label(tr("Rifle"));
                let before = new.rifle_id.clone();
                egui::ComboBox::from_id_salt("new-ladder-rifle")
                    .selected_text(super::rifle_name(data, new.rifle_id.as_deref()))
                    .show_ui(ui, |ui| {
                        for rifle in &data.rifles {
                            ui.selectable_value(
                                &mut new.rifle_id,
                                Some(rifle.id.clone()),
                                &rifle.name,
                            );
                        }
                    });
                if new.rifle_id != before {
                    new.bullet_id = None;
                }
                ui.end_row();
                ui.label(tr("Handload"));
                let rifle = new
                    .rifle_id
                    .as_ref()
                    .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id));
                let handloads: Vec<&Bullet> = rifle
                    .map(|rifle| crate::groups::form::bullets_for(rifle, data))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|bullet| bullet.kind == BulletKind::Handload)
                    .collect();
                egui::ComboBox::from_id_salt("new-ladder-bullet")
                    .selected_text(match &new.bullet_id {
                        Some(id) => super::bullet_name(data, Some(id)),
                        None => tr("Choose").to_owned(),
                    })
                    .width(ui.available_width().clamp(120.0, 260.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for bullet in &handloads {
                            ui.selectable_value(
                                &mut new.bullet_id,
                                Some(bullet.id.clone()),
                                bullet.display_name(),
                            );
                        }
                    });
                ui.end_row();
                if handloads.is_empty() {
                    ui.label("");
                    ui.weak(tr("Add the handload on the Ammunition page first."));
                    ui.end_row();
                }
                ui.label(tr("Varies"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut new.variable, LadderVariable::Charge, tr("Charge"));
                    ui.selectable_value(
                        &mut new.variable,
                        LadderVariable::Seating,
                        tr("Seating (CBTO)"),
                    );
                });
                ui.end_row();
                ui.label(tr("Name"));
                ui.add(
                    egui::TextEdit::singleline(&mut new.name)
                        .hint_text(tr("H4350 ladder"))
                        .desired_width(200.0),
                );
                ui.end_row();
            });
            ui.horizontal(|ui| {
                let ready = new.rifle_id.is_some() && new.bullet_id.is_some();
                let palette = theme::palette(ui.ctx());
                if ui
                    .add_enabled(
                        ready,
                        egui::Button::new(RichText::new(tr("Create")).color(palette.on_accent))
                            .fill(palette.accent),
                    )
                    .clicked()
                    && let (Some(rifle), Some(bullet)) = (&new.rifle_id, &new.bullet_id)
                {
                    let mut ladder = Ladder::new(rifle, bullet, new.variable);
                    ladder.name = new.name.trim().chars().take(100).collect();
                    done = Some(Some(ladder));
                }
                if ui.button(tr("Cancel")).clicked() {
                    done = Some(None);
                }
            });
        });
    match done {
        Some(Some(ladder)) => {
            state.open = Some(ladder.id.clone());
            state.creating = None;
            state.run = None;
            actions.push(Action::SaveLadder(ladder));
        }
        Some(None) => state.creating = None,
        None => {}
    }
}

/// "Oryx 308 · Browning X-Bolt · 6 rungs × 3 shots · 100 m".
fn header(ui: &mut egui::Ui, ladder: &Ladder, result: &LadderData, data: &Data, u: &UnitPrefs) {
    let mut parts = vec![
        super::rifle_name(data, Some(&ladder.rifle_id)),
        super::bullet_name(data, Some(&ladder.bullet_id)),
    ];
    if let Some(Ok(worked)) = &result.ladder {
        let shots: Vec<usize> = worked.rungs.iter().map(|rung| rung.shots).collect();
        if shots.windows(2).all(|pair| pair[0] == pair[1]) && !shots.is_empty() {
            parts.push(trf!("{} rungs × {} shots", shots.len(), shots[0]));
        } else {
            parts.push(trf!(
                "{} rungs, {} shots",
                shots.len(),
                shots.iter().sum::<usize>()
            ));
        }
    }
    if let Some(distance) = result.distance_m {
        parts.push(u.format(Quantity::Distance, distance));
    }
    ui.heading(name(ladder, data));
    super::weak(ui, &parts.join(" · "));
    if let Some(promoted) = &ladder.promoted_bullet_id {
        super::weak(
            ui,
            &trf!(
                "Made into a load: {}",
                super::bullet_name(data, Some(promoted))
            ),
        );
    }
}

/// Vertical impact per rung with its noise as bars, and across faintly;
/// the chosen run shaded.
fn chart(
    ui: &mut egui::Ui,
    worked: &groups_core::Ladder,
    run: Option<&LadderRun>,
    variable: LadderVariable,
    u: &UnitPrefs,
    phone: bool,
) {
    let palette = theme::palette(ui.ctx());
    let value_quantity = quantity(variable);
    let x = |value: f64| u.show(value_quantity, value);
    let y = |mrad: f64| u.show(Quantity::GroupAngle, mrad);
    let vertical: Vec<[f64; 2]> = worked
        .rungs
        .iter()
        .map(|rung| [x(rung.value), y(rung.vertical)])
        .collect();
    let across: Vec<[f64; 2]> = worked
        .rungs
        .iter()
        .map(|rung| [x(rung.value), y(rung.horizontal)])
        .collect();
    let step = worked
        .rungs
        .windows(2)
        .map(|pair| x(pair[1].value) - x(pair[0].value))
        .fold(f64::MAX, f64::min);
    let half = if step.is_finite() && step < f64::MAX {
        step / 2.0
    } else {
        0.1
    };
    let xs: Vec<f64> = worked.rungs.iter().map(|rung| x(rung.value)).collect();
    rung_axis(
        Plot::new("ladder-vertical"),
        xs,
        u.unit(value_quantity).decimals,
    )
    .height(if phone { 220.0 } else { 260.0 })
    .allow_drag(false)
    .allow_zoom(false)
    .allow_scroll(false)
    .allow_double_click_reset(false)
    .x_axis_label(format!(
        "{} ({})",
        match variable {
            LadderVariable::Charge => tr("Charge"),
            LadderVariable::Seating => tr("Seating (CBTO)"),
        },
        u.label(value_quantity)
    ))
    .y_axis_label(trf!("Impact ({})", u.label(Quantity::GroupAngle)))
    .show(ui, |plot| {
        if let Some(run) = run {
            let first = x(worked.rungs[run.first].value) - half;
            let last = x(worked.rungs[run.first + run.rungs - 1].value) + half;
            plot.span(
                Span::new(tr("Chosen run"), first..=last)
                    .fill(palette.good.gamma_multiply(0.15))
                    .border_color(palette.good.gamma_multiply(0.5)),
            );
        }
        for rung in &worked.rungs {
            let at = x(rung.value);
            plot.line(
                Line::new(
                    "",
                    PlotPoints::from(vec![
                        [at, y(rung.vertical - rung.vertical_standard_error)],
                        [at, y(rung.vertical + rung.vertical_standard_error)],
                    ]),
                )
                .allow_band(false)
                .color(palette.accent.gamma_multiply(0.7))
                .width(2.0_f32),
            );
        }
        plot.line(
            Line::new(tr("Vertical"), PlotPoints::from(vertical.clone()))
                .allow_band(false)
                .color(palette.accent)
                .width(1.5_f32),
        );
        plot.points(
            Points::new(tr("Vertical"), PlotPoints::from(vertical))
                .color(palette.accent)
                .radius(4.0),
        );
        plot.line(
            Line::new(tr("Across"), PlotPoints::from(across))
                .allow_band(false)
                .color(palette.text_muted)
                .style(LineStyle::dashed_dense())
                .width(1.0_f32),
        );
    });
    super::weak(
        ui,
        tr(
            "Vertical impact per rung, its bars the noise (± one standard error); dashed: \
            across; shaded: the chosen run.",
        ),
    );
}

fn velocity_chart(
    ui: &mut egui::Ui,
    worked: &groups_core::Ladder,
    variable: LadderVariable,
    u: &UnitPrefs,
    phone: bool,
) {
    let palette = theme::palette(ui.ctx());
    let value_quantity = quantity(variable);
    let x = |value: f64| u.show(value_quantity, value);
    let v = |mps: f64| u.show(Quantity::Speed, mps);
    let points: Vec<[f64; 2]> = worked
        .rungs
        .iter()
        .filter_map(|rung| Some([x(rung.value), v(rung.velocity?.mean_mps)]))
        .collect();
    let xs: Vec<f64> = worked.rungs.iter().map(|rung| x(rung.value)).collect();
    rung_axis(
        Plot::new("ladder-velocity"),
        xs,
        u.unit(value_quantity).decimals,
    )
    .height(if phone { 150.0 } else { 170.0 })
    .allow_drag(false)
    .allow_zoom(false)
    .allow_scroll(false)
    .allow_double_click_reset(false)
    .y_axis_label(trf!("Velocity ({})", u.label(Quantity::Speed)))
    .show(ui, |plot| {
        for rung in &worked.rungs {
            let Some(velocity) = rung.velocity else {
                continue;
            };
            if let Some(sd) = velocity.sd_mps {
                plot.line(
                    Line::new(
                        "",
                        PlotPoints::from(vec![
                            [x(rung.value), v(velocity.mean_mps - sd)],
                            [x(rung.value), v(velocity.mean_mps + sd)],
                        ]),
                    )
                    .allow_band(false)
                    .color(palette.link.gamma_multiply(0.7))
                    .width(2.0_f32),
                );
            }
        }
        plot.line(
            Line::new(tr("Velocity"), PlotPoints::from(points.clone()))
                .allow_band(false)
                .color(palette.link)
                .width(1.5_f32),
        );
        plot.points(
            Points::new(tr("Velocity"), PlotPoints::from(points))
                .color(palette.link)
                .radius(4.0),
        );
    });
    super::weak(ui, tr("Velocity per rung; bars: ± one SD of its readings."));
}

/// A chart whose x axis is marked and labelled at each rung's value.
fn rung_axis(plot: Plot<'_>, xs: Vec<f64>, decimals: usize) -> Plot<'_> {
    let step = xs
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|step| *step > 0.0)
        .fold(f64::INFINITY, f64::min);
    let step = if step.is_finite() { step } else { 1.0 };
    plot.x_grid_spacer(move |_| {
        xs.iter()
            .map(|&value| GridMark {
                value,
                step_size: step,
            })
            .collect()
    })
    .x_axis_formatter(move |mark, _| format!("{:.*}", decimals, mark.value))
}

/// A run's values, "45.0–46.0 gr".
fn run_values(
    worked: &groups_core::Ladder,
    run: &LadderRun,
    quantity: Quantity,
    u: &UnitPrefs,
) -> String {
    let first = worked.rungs[run.first].value;
    let last = worked.rungs[run.first + run.rungs - 1].value;
    let unit = u.unit(quantity);
    format!(
        "{:.*}–{:.*} {}",
        unit.decimals,
        unit.show(first),
        unit.decimals,
        unit.show(last),
        unit.label
    )
}

/// A run as a selectable row: its values, how far the impact moves
/// against its noise, the velocities and the verdict. True when tapped.
fn run_row(
    ui: &mut egui::Ui,
    worked: &groups_core::Ladder,
    run: &LadderRun,
    variable: LadderVariable,
    selected: bool,
    u: &UnitPrefs,
) -> bool {
    let palette = theme::palette(ui.ctx());
    let value_quantity = quantity(variable);
    let rungs = &worked.rungs[run.first..run.first + run.rungs];
    let speeds: Vec<f64> = rungs
        .iter()
        .filter_map(|rung| rung.velocity.map(|v| v.mean_mps))
        .collect();
    let mut detail = trf!(
        "vertical moves {} · noise ±{}",
        angle(u, run.vertical_range),
        angle(u, run.vertical_noise)
    );
    if let (Some(low), Some(high)) = (
        speeds.iter().copied().reduce(f64::min),
        speeds.iter().copied().reduce(f64::max),
    ) {
        detail.push_str(&format!(
            " · {:.0}–{:.0} {}",
            u.show(Quantity::Speed, low),
            u.show(Quantity::Speed, high),
            u.label(Quantity::Speed)
        ));
    }
    let verdict = if run.flat && run.flat_across {
        trf!(
            "Flat up and across: score {:.1}, within the limit of {:.2}.",
            run.vertical_score,
            run.flat_limit
        )
    } else if run.flat {
        trf!(
            "Flat vertically (score {:.1}, limit {:.2}); it moves across.",
            run.vertical_score,
            run.flat_limit
        )
    } else {
        trf!(
            "Vertical moves more than its noise: score {:.1} against a limit of {:.2}.",
            run.vertical_score,
            run.flat_limit
        )
    };
    let velocity = run.velocity_per_unit.map(|trend| {
        let per_shown = |per_unit: f64| per_unit / u.show(value_quantity, 1.0);
        let mut text = trf!(
            "{:+.1} {} per {}",
            u.show(Quantity::Speed, per_shown(trend.per_unit)),
            u.label(Quantity::Speed),
            u.label(value_quantity)
        );
        if let Some(range) = trend.range {
            text.push_str(&trf!(
                " (95%: {:.1} to {:.1})",
                u.show(Quantity::Speed, per_shown(range.low)),
                u.show(Quantity::Speed, per_shown(range.high))
            ));
        }
        text
    });
    let mut clicked = false;
    let fill = if selected {
        palette.hover
    } else {
        palette.raised
    };
    let stroke = if run.flat {
        egui::Stroke::new(1.5_f32, palette.good)
    } else {
        palette.edge
    };
    let response = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(trf!(
                        "{} · {} rungs",
                        run_values(worked, run, value_quantity, u),
                        run.rungs
                    ))
                    .strong(),
                );
                if run.flat {
                    ui.label(RichText::new("✓").color(palette.good).strong());
                }
            });
            ui.add(egui::Label::new(RichText::new(detail).color(palette.text_muted)).wrap());
            ui.add(egui::Label::new(RichText::new(verdict).color(palette.text)).wrap());
            if let Some(velocity) = velocity {
                ui.add(egui::Label::new(RichText::new(velocity).color(palette.text_muted)).wrap());
            }
        })
        .response;
    if ui
        .interact(
            response.rect,
            response.id.with("pick"),
            egui::Sense::click(),
        )
        .clicked()
    {
        clicked = true;
    }
    clicked
}

/// "Make 45.5 gr my load": the run's middle value into a handload's
/// recipe, a new one or one chosen, and the ladder told which.
#[allow(clippy::too_many_arguments)]
fn promote(
    ui: &mut egui::Ui,
    state: &mut State,
    ladder: &Ladder,
    worked: &groups_core::Ladder,
    run: &LadderRun,
    data: &Data,
    u: &UnitPrefs,
    actions: &mut Vec<Action>,
) {
    let Some(source) = data.bullets.iter().find(|b| b.id == ladder.bullet_id) else {
        return;
    };
    let middle = pool::run_middle(worked, run);
    let value_quantity = quantity(ladder.variable);
    let value_text = u.format(value_quantity, middle);
    let candidates: Vec<&Bullet> = data
        .bullets
        .iter()
        .filter(|bullet| bullet.kind == BulletKind::Handload && bullet.caliber == source.caliber)
        .collect();
    if state
        .target
        .as_ref()
        .is_some_and(|id| !candidates.iter().any(|bullet| &bullet.id == id))
    {
        state.target = None;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Into"));
        let shown = match &state.target {
            None => trf!("a new load, copied from {}", source.display_name()),
            Some(id) => super::bullet_name(data, Some(id)),
        };
        egui::ComboBox::from_id_salt("ladder-target")
            .selected_text(shown)
            .width(ui.available_width().clamp(140.0, 300.0))
            .truncate()
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.target,
                    None,
                    trf!("a new load, copied from {}", source.display_name()),
                );
                for bullet in &candidates {
                    ui.selectable_value(
                        &mut state.target,
                        Some(bullet.id.clone()),
                        bullet.display_name(),
                    );
                }
            });
    });
    let label = trf!("Make {} my load", value_text);
    let target = state
        .target
        .as_ref()
        .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id));
    let question = match (target, ladder.variable) {
        (Some(target), LadderVariable::Charge) => {
            trf!("Set {}'s charge to {}?", target.display_name(), value_text)
        }
        (Some(target), LadderVariable::Seating) => {
            trf!("Set {}'s CBTO to {}?", target.display_name(), value_text)
        }
        (None, _) => trf!(
            "Make a new load of {} at {}, with its data in {}?",
            source.display_name(),
            value_text,
            super::rifle_name(data, Some(&ladder.rifle_id))
        ),
    };
    if super::confirm(ui, "ladder-promote", &label, &question, false) {
        let name = match ladder.variable {
            LadderVariable::Charge => trf!("{} charge", value_text),
            LadderVariable::Seating => trf!("CBTO {}", value_text),
        };
        let promotion = pool::promotion(source, target, ladder, middle, &name, data);
        actions.push(Action::PromoteLadder(Box::new(promotion)));
    }
    super::weak(
        ui,
        &match ladder.variable {
            LadderVariable::Charge => trf!(
                "Sets the recipe's charge to {}, the middle of the chosen run.",
                value_text
            ),
            LadderVariable::Seating => trf!(
                "Sets the recipe's CBTO to {}, the middle of the chosen run.",
                value_text
            ),
        },
    );
}

//! Zero: is the zero holding? Every group of a rifle, barrel and load shot
//! since its last zero reset, net of what was dialled, pooled into one
//! impact with its 95% range.

use ballistics_store::ZeroReset;
use ballistics_store::model::now_ms;
use eframe::egui::{self, RichText};
use egui_plot::{HLine, LineStyle, MarkerShape, Plot, PlotPoints, Points, Polygon, VLine};
use groups_core::{AnalysisError, Point, ZeroCheck, ZeroVerdict};

use super::pool::{self, Ineligible, Pick, ZeroData, ZeroScope};
use super::{Memo, Tone, angle, arrows, key_of};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

#[derive(Default)]
pub struct State {
    /// The hunter's choice; `None` follows the last used rifle and load.
    pub scope: Option<ZeroScope>,
    memo: Memo<ZeroData>,
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
    let Some(mut scope) = state
        .scope
        .clone()
        .filter(|scope| data.rifles.iter().any(|rifle| rifle.id == scope.rifle_id))
        .or_else(|| ZeroScope::default_for(data))
    else {
        super::weak(
            ui,
            tr(
                "A zero check is of a rifle: add one on the Rifles page, then save groups shot \
                with it.",
            ),
        );
        return;
    };
    if filters(ui, &mut scope, data) {
        state.scope = Some(scope.clone());
    }
    let zero = state
        .memo
        .get(key_of(stamp, &scope), || pool::zero_data(data, &scope));
    let rifle = data.rifles.iter().find(|rifle| rifle.id == scope.rifle_id);
    if scope.pools_everything(data) {
        super::note(
            ui,
            tr(
                "This pools every barrel or load of the rifle. Each can hit somewhere else: \
                choose one to check its own zero.",
            ),
        );
    }
    if let Some(zero_range_m) = zero.zero_range_m {
        super::weak(
            ui,
            &trf!(
                "Counts groups shot at the rifle's zero range, {} (±{}).",
                u.format(Quantity::Distance, zero_range_m),
                u.format(
                    Quantity::Distance,
                    pool::zero_range_tolerance_m(zero_range_m)
                )
            ),
        );
    }
    if zero.no_barrel > 0 {
        super::note(
            ui,
            &trf!(
                "{} groups of this rifle and load name no barrel: choose “All barrels” to count \
                 them.",
                zero.no_barrel
            ),
        );
    }
    ui.add_space(6.0);
    let check = zero.check.as_ref().and_then(|check| check.as_ref().ok());
    verdict(ui, &zero, &u, data);
    if let (Some(check), Some(rifle)) = (check, rifle) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Dial")).strong());
            ui.label(super::dial_text(rifle, check.correction_mrad));
            ui.weak(trf!(
                "({} {} clicks)",
                rifle.click_value,
                ballistics_shot::unit_label(rifle.click_unit)
            ));
        });
    }
    if zero.dialled_groups > 0 {
        let text = if zero.dialled_groups == 1 {
            tr("1 group had a dialled offset, subtracted before pooling.").to_owned()
        } else {
            trf!(
                "{} groups had a dialled offset, subtracted before pooling.",
                zero.dialled_groups
            )
        };
        super::weak(ui, &text);
    }
    if zero.zero_height_mrad != 0.0
        && let Some(rifle) = rifle
    {
        super::weak(
            ui,
            &trf!(
                "Your zero is {} at {}: groups shot at that range are measured from there.",
                u.format_signed(Quantity::ShortLength, rifle.zero_height_cm),
                u.format(Quantity::Distance, rifle.zero_range_m)
            ),
        );
    }
    if let Some(check) = check {
        ui.add_space(6.0);
        scatter(ui, check, &u, phone);
        super::weak(
            ui,
            tr(
                "Dots: each group's impact, less what was dialled · diamond: the pooled impact, \
                with its 95% box · dashed: the tolerance.",
            ),
        );
    }
    if !zero.entries.is_empty() {
        ui.add_space(6.0);
        table(ui, &zero, data, &u);
    }
    left_out(ui, &zero);
    ui.add_space(10.0);
    buttons(ui, &scope, &zero, data, &u, actions);
    resets(ui, &scope, data);
}

/// The rifle, barrel and load pickers. True when one changed.
fn filters(ui: &mut egui::Ui, scope: &mut ZeroScope, data: &Data) -> bool {
    let before = scope.clone();
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("zero-rifle")
            .selected_text(super::rifle_name(data, Some(&scope.rifle_id)))
            .show_ui(ui, |ui| {
                for rifle in &data.rifles {
                    ui.selectable_value(&mut scope.rifle_id, rifle.id.clone(), &rifle.name);
                }
            });
        if scope.rifle_id != before.rifle_id {
            let rifle = data.rifles.iter().find(|rifle| rifle.id == scope.rifle_id);
            scope.barrel = rifle
                .and_then(|rifle| rifle.fitted_barrel_id.clone())
                .map_or(Pick::All, Pick::One);
            scope.load = data
                .groups
                .groups
                .iter()
                .filter(|group| group.rifle_id.as_ref() == Some(&scope.rifle_id))
                .max_by_key(|group| group.taken_at)
                .and_then(|group| group.bullet_id.clone())
                .map_or(Pick::All, Pick::One);
        }
        let barrels: Vec<_> = data
            .groups
            .barrels
            .iter()
            .filter(|barrel| barrel.rifle_id == scope.rifle_id)
            .collect();
        if !barrels.is_empty() {
            let shown = match &scope.barrel {
                Pick::All => tr("All barrels").to_owned(),
                Pick::One(id) => barrels.iter().find(|barrel| &barrel.id == id).map_or_else(
                    || tr("A barrel not on this device").to_owned(),
                    |b| b.name.clone(),
                ),
            };
            egui::ComboBox::from_id_salt("zero-barrel")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut scope.barrel, Pick::All, tr("All barrels"));
                    for barrel in &barrels {
                        ui.selectable_value(
                            &mut scope.barrel,
                            Pick::One(barrel.id.clone()),
                            &barrel.name,
                        );
                    }
                });
        }
        let mut loads: Vec<&str> = data
            .groups
            .groups
            .iter()
            .filter(|group| group.rifle_id.as_ref() == Some(&scope.rifle_id))
            .filter_map(|group| group.bullet_id.as_deref())
            .collect();
        loads.sort_unstable();
        loads.dedup();
        let shown = match &scope.load {
            Pick::All => tr("All loads").to_owned(),
            Pick::One(id) => super::bullet_name(data, Some(id)),
        };
        egui::ComboBox::from_id_salt("zero-load")
            .selected_text(shown)
            .width(ui.available_width().clamp(120.0, 240.0))
            .truncate()
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut scope.load, Pick::All, tr("All loads"));
                for id in loads {
                    ui.selectable_value(
                        &mut scope.load,
                        Pick::One(id.to_owned()),
                        super::bullet_name(data, Some(id)),
                    );
                }
            });
    });
    *scope != before
}

fn error_text(error: &AnalysisError) -> String {
    match error {
        AnalysisError::NoSpread => {
            tr("The shots have no spread at all, so there is no range.").to_owned()
        }
        AnalysisError::NoSpreadEstimate => {
            tr("Needs a group of 2 shots or more, or 3 groups, to measure the spread.").to_owned()
        }
        other => trf!("Can't be worked out: {}", other),
    }
}

/// "0.02 mil left, 0.04 mil high".
fn impact_words(u: &UnitPrefs, impact: Point) -> String {
    let zero = angle(u, 0.0);
    let mut parts = Vec::new();
    let across = angle(u, impact.x.abs());
    if across != zero {
        parts.push(if impact.x > 0.0 {
            trf!("{} right", across)
        } else {
            trf!("{} left", across)
        });
    }
    let up = angle(u, impact.y.abs());
    if up != zero {
        parts.push(if impact.y > 0.0 {
            trf!("{} high", up)
        } else {
            trf!("{} low", up)
        });
    }
    if parts.is_empty() {
        tr("On the aim").to_owned()
    } else {
        parts.join(", ")
    }
}

/// The verdict card: holding, drifting or can't tell yet, in plain words.
fn verdict(ui: &mut egui::Ui, zero: &ZeroData, u: &UnitPrefs, data: &Data) {
    let since = match &zero.reset {
        Some(reset) => trf!("since the reset on {}", super::date(reset.at, data)),
        None => tr("no zero reset marked: every group counts").to_owned(),
    };
    let check =
        match &zero.check {
            None => {
                super::verdict(
                ui,
                Tone::Unsure,
                tr("Can't tell yet"),
                &[
                    tr("No group counts yet. A zero check counts groups with an aim point and a \
                        distance, shot after the last zero reset, with “Include in zero check” \
                        on.")
                    .to_owned(),
                    since,
                ],
            );
                return;
            }
            Some(Err(error)) => {
                super::verdict(ui, Tone::Unsure, tr("Can't tell yet"), &[error_text(error)]);
                return;
            }
            Some(Ok(check)) => check,
        };
    let impact = Point::new(check.across.impact.value, check.up.impact.value);
    let half =
        |axis: &groups_core::ZeroAxis| (axis.impact.range.high - axis.impact.range.low) / 2.0;
    let within = half(&check.across).max(half(&check.up));
    let tolerance = angle(u, check.tolerance_mrad);
    let mut lines = vec![trf!(
        "{}, 95% within ±{}.",
        impact_words(u, impact),
        angle(u, within)
    )];
    let (tone, word) =
        match check.verdict {
            ZeroVerdict::Holding => {
                lines.push(trf!(
                    "The mean impact's range lies within ±{} on both axes: the zero holds.",
                    tolerance
                ));
                (Tone::Good, tr("Holding"))
            }
            ZeroVerdict::Drifting => {
                lines.push(trf!(
                    "The impact has moved beyond ±{}, by more than the shots explain: dial the \
                 correction, or use it in the calculator.",
                    tolerance
                ));
                (Tone::Bad, tr("Drifting"))
            }
            ZeroVerdict::CantTell => {
                if check.groups < groups_core::zero::MIN_GROUPS_FOR_SHIFTS {
                    lines.push(
                    tr("One or two groups give no verdict: how far the impact moves from group \
                        to group can't be measured yet.")
                    .to_owned(),
                );
                } else {
                    lines.push(trf!(
                        "Neither holding within ±{} nor clearly moved yet.",
                        tolerance
                    ));
                }
                if let Some(more) = check.more_groups {
                    lines.push(if more == 1 {
                        tr("About 1 more group like these would settle it.").to_owned()
                    } else {
                        trf!("About {} more groups like these would settle it.", more)
                    });
                }
                (Tone::Unsure, tr("Can't tell yet"))
            }
        };
    if !check.counts_group_to_group {
        lines.push(
            tr(
                "The range rests on the shots alone: it is too narrow if the impact moves \
                between groups.",
            )
            .to_owned(),
        );
    } else if check.effective_groups < 0.6 * check.groups as f64 {
        lines.push(
            tr("One group holds most of the shots, so the range leans on the few others.")
                .to_owned(),
        );
    }
    lines.push(trf!(
        "{} groups · {} shots, {}",
        check.groups,
        check.shots,
        since
    ));
    super::verdict(ui, tone, word, &lines);
}

/// Each group's net impact around the aim, and the pooled impact with its
/// 95% box and the tolerance.
fn scatter(ui: &mut egui::Ui, check: &ZeroCheck, u: &UnitPrefs, phone: bool) {
    let palette = theme::palette(ui.ctx());
    let show = |mrad: f64| u.show(Quantity::GroupAngle, mrad);
    let unit = u.label(Quantity::GroupAngle);
    let points: Vec<[f64; 2]> = check
        .net_impacts_mrad
        .iter()
        .map(|point| [show(point.x), show(point.y)])
        .collect();
    let mean = [show(check.across.impact.value), show(check.up.impact.value)];
    let (across, up) = (check.across.impact.range, check.up.impact.range);
    let rect = |x0: f64, x1: f64, y0: f64, y1: f64| -> Vec<[f64; 2]> {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    };
    let tolerance = show(check.tolerance_mrad);
    let mut reach = tolerance * 1.4;
    for point in points.iter().chain([&mean]) {
        reach = reach.max(point[0].abs() * 1.15).max(point[1].abs() * 1.15);
    }
    for value in [across.low, across.high, up.low, up.high] {
        reach = reach.max(show(value).abs() * 1.1);
    }
    let side = ui.available_width().min(if phone { 360.0 } else { 440.0 });
    Plot::new("zero-scatter")
        .width(side)
        .height(side)
        .data_aspect(1.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_double_click_reset(false)
        .include_x(-reach)
        .include_x(reach)
        .include_y(-reach)
        .include_y(reach)
        .x_axis_label(trf!("Right ({})", unit))
        .y_axis_label(trf!("Up ({})", unit))
        .show(ui, |plot| {
            plot.hline(HLine::new(tr("Aim"), 0.0).color(palette.text_muted));
            plot.vline(VLine::new(tr("Aim"), 0.0).color(palette.text_muted));
            plot.polygon(
                Polygon::new(
                    tr("Tolerance"),
                    PlotPoints::from(rect(-tolerance, tolerance, -tolerance, tolerance)),
                )
                .stroke(egui::Stroke::new(1.0_f32, palette.good))
                .fill_color(egui::Color32::TRANSPARENT)
                .style(LineStyle::dashed_loose()),
            );
            plot.polygon(
                Polygon::new(
                    tr("95% range"),
                    PlotPoints::from(rect(
                        show(across.low),
                        show(across.high),
                        show(up.low),
                        show(up.high),
                    )),
                )
                .stroke(egui::Stroke::new(1.5_f32, palette.accent))
                .fill_color(palette.accent.gamma_multiply(0.15)),
            );
            plot.points(
                Points::new(tr("Groups"), PlotPoints::from(points))
                    .radius(4.0)
                    .color(palette.text),
            );
            plot.points(
                Points::new(tr("Pooled impact"), PlotPoints::from(vec![mean]))
                    .shape(MarkerShape::Diamond)
                    .radius(7.0)
                    .color(palette.accent),
            );
        });
}

fn table(ui: &mut egui::Ui, zero: &ZeroData, data: &Data, u: &UnitPrefs) {
    let palette = theme::palette(ui.ctx());
    let impacts = zero
        .check
        .as_ref()
        .and_then(|check| check.as_ref().ok())
        .map(|check| check.net_impacts_mrad.clone());
    egui::Grid::new("zero-groups")
        .striped(true)
        .num_columns(4)
        .spacing(egui::vec2(14.0, 4.0))
        .show(ui, |ui| {
            for heading in [tr("Date"), tr("Impact"), tr("Shots"), tr("Distance")] {
                ui.label(RichText::new(heading).small().color(palette.text_muted));
            }
            ui.end_row();
            for (index, entry) in zero.entries.iter().enumerate().rev() {
                ui.label(super::date(entry.taken_at, data));
                let net = impacts
                    .as_ref()
                    .and_then(|impacts| impacts.get(index).copied())
                    .unwrap_or(entry.group.impact_mrad);
                let mut text = arrows(u, net);
                if entry.dialled_mrad != [0.0, 0.0] {
                    text.push_str(" ⊕");
                }
                ui.label(text).on_hover_text(tr(
                    "Its impact less what was dialled for it (⊕: something was)",
                ));
                ui.label(entry.group.shots.to_string());
                ui.label(u.format(Quantity::Distance, entry.distance_m));
                ui.end_row();
            }
        });
}

/// What kept groups of this rifle, barrel and load out, counted by why.
fn left_out(ui: &mut egui::Ui, zero: &ZeroData) {
    if zero.left_out.is_empty() {
        return;
    }
    let count = |reason: Ineligible| {
        zero.left_out
            .iter()
            .filter(|(_, why)| *why == reason)
            .count()
    };
    let mut parts = Vec::new();
    for (reason, text) in [
        (Ineligible::BeforeReset, tr("shot before the reset")),
        (Ineligible::NotIncluded, tr("“Include in zero check” off")),
        (Ineligible::Ladder, tr("ladder rungs")),
        (Ineligible::NoAim, tr("without an aim point")),
        (Ineligible::NoDistance, tr("without a distance")),
        (Ineligible::NotMeasured, tr("can't be measured")),
        (Ineligible::NotAtZeroRange, tr("not at the zero range")),
        (Ineligible::BarrelRemoved, tr("barrel removed")),
    ] {
        let n = count(reason);
        if n > 0 {
            parts.push(format!("{text} ({n})"));
        }
    }
    ui.add_space(4.0);
    super::weak(ui, &trf!("Groups left out: {}.", parts.join(" · ")));
}

fn buttons(
    ui: &mut egui::Ui,
    scope: &ZeroScope,
    zero: &ZeroData,
    data: &Data,
    u: &UnitPrefs,
    actions: &mut Vec<Action>,
) {
    let check = zero.check.as_ref().and_then(|check| check.as_ref().ok());
    let rifle = data.rifles.iter().find(|rifle| rifle.id == scope.rifle_id);
    ui.horizontal_wrapped(|ui| {
        if crate::widgets::confirm_button(ui, "zero-reset", tr("Mark zero reset now")) {
            let reset = ZeroReset::new(&scope.rifle_id, scope.barrel.one(), now_ms());
            actions.push(Action::SaveZeroReset(reset));
        }
    });
    let writable = check.filter(|check| pool::zero_writable(check));
    match (writable, check, scope.load.one()) {
        (Some(check), _, Some(bullet_id)) => {
            let correction = check.correction_mrad;
            let range =
                |axis: &groups_core::ZeroAxis| (-axis.impact.range.high, -axis.impact.range.low);
            let (across, up) = (range(&check.across), range(&check.up));
            let dial = rifle.map_or_else(String::new, |rifle| super::dial_text(rifle, correction));
            let question = trf!(
                "Correction: {} ({}). 95% range: right {} to {}, up {} to {}. Write where this \
                 load hits into the calculator?",
                dial,
                arrows(u, correction),
                u.number(Quantity::GroupAngle, across.0, true),
                u.number(Quantity::GroupAngle, across.1, true),
                u.number(Quantity::GroupAngle, up.0, true),
                u.number(Quantity::GroupAngle, up.1, true)
            );
            if super::confirm(ui, "zero-use", tr("Use in calculator"), &question, false) {
                let offset = pool::zero_offset(check);
                let load = pool::load_with_offset(data, &scope.rifle_id, bullet_id, offset);
                actions.push(Action::SaveLoad(load));
            }
        }
        (None, Some(_), Some(_)) => super::weak(
            ui,
            tr(
                "Use in calculator comes with a verdict, holding or drifting: it needs three \
                groups or more.",
            ),
        ),
        (_, Some(_), None) => {
            super::weak(ui, tr("Choose one load to use its zero in the calculator."))
        }
        _ => {}
    }
    if let (Some(check), Some(_)) = (writable, scope.load.one()) {
        let offset = pool::zero_offset(check);
        super::weak(
            ui,
            &trf!(
                "Use in calculator makes {} this load's point of impact in the rifle, so the \
                 calculator aims for where it really hits. Mark a zero reset when you dial the \
                 correction instead.",
                arrows(u, Point::new(offset[0], offset[1]))
            ),
        );
    }
}

/// The rifle's zero resets, newest first.
fn resets(ui: &mut egui::Ui, scope: &ZeroScope, data: &Data) {
    let resets: Vec<&ZeroReset> = data
        .groups
        .zero_resets
        .iter()
        .filter(|reset| reset.rifle_id == scope.rifle_id)
        .collect();
    if resets.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tr("Zero resets")).strong());
    for reset in resets {
        let mut parts = vec![super::date(reset.at, data)];
        match reset
            .barrel_id
            .as_ref()
            .and_then(|id| data.groups.barrels.iter().find(|barrel| &barrel.id == id))
        {
            Some(barrel) => parts.push(barrel.name.clone()),
            None => parts.push(tr("every barrel").to_owned()),
        }
        if !reset.note.trim().is_empty() {
            parts.push(reset.note.trim().to_owned());
        }
        ui.weak(parts.join(" · "));
    }
}

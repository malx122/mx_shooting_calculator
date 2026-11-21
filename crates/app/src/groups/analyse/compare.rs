//! Compare: groups side by side, best and worst by the library's measure,
//! and for two groups a test of whether their spreads really differ.

use ballistics_store::{Analysis, AnalysisKind};
use eframe::egui::{self, Align2, FontId, RichText, Sense, Stroke, pos2, vec2};
use groups_core::{AnalysisError, SigmaVerdict};

use super::pool::{self, Compared, NotRanked};
use super::{Memo, Tone, angle, key_of, letter};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

#[derive(Default)]
pub struct State {
    pub ids: Vec<String>,
    /// The saved analysis this is, if it is one.
    pub saved: Option<String>,
    /// Its groups deleted since it was saved.
    pub missing: usize,
    memo: Memo<Compared>,
}

impl State {
    pub fn of(ids: Vec<String>, saved: Option<String>) -> State {
        State {
            ids,
            saved,
            ..State::default()
        }
    }
}

/// Where Compare sends the hunter next.
pub enum Next {
    /// A composite of these groups.
    Composite(Vec<String>),
    /// A saved comparison.
    Open(String),
}

fn rank_by(by: crate::groups::RankBy) -> groups_core::RankBy {
    match by {
        crate::groups::RankBy::MeanRadius => groups_core::RankBy::MeanRadius,
        crate::groups::RankBy::ExtremeSpread => groups_core::RankBy::ExtremeSpread,
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    data: &Data,
    stamp: u64,
    by: crate::groups::RankBy,
    phone: bool,
    actions: &mut Vec<Action>,
) -> Option<Next> {
    let u = units::prefs(ui.ctx());
    let saved = state
        .saved
        .as_ref()
        .and_then(|id| data.groups.analyses.iter().find(|a| &a.id == id));
    if state.ids.is_empty() && saved.is_none() {
        return empty(ui, data);
    }
    let ids = state.ids.clone();
    let compared = state.memo.get(
        key_of(stamp, (&ids, by == crate::groups::RankBy::MeanRadius)),
        || pool::compare(data, &ids, rank_by(by)),
    );
    let heading = saved.map_or_else(
        || trf!("{} groups", compared.ids.len()),
        super::analysis_name,
    );
    ui.heading(heading);
    if state.missing > 0 {
        super::note(
            ui,
            &trf!(
                "{} of its groups were deleted since it was saved.",
                state.missing
            ),
        );
    }
    if compared.ids.len() < 2 {
        super::note(
            ui,
            tr("Fewer than 2 of its groups are left: nothing to compare."),
        );
    }
    ui.add_space(4.0);
    cards(ui, &compared, data, by, &u, phone);
    if compared.unequal_shots {
        super::weak(
            ui,
            tr(
                "The groups have different numbers of shots: a small group's mean radius reads \
                a little small and its extreme spread much smaller, so the ranking flatters \
                small groups.",
            ),
        );
    }
    for (index, why) in &compared.not_ranked {
        let reason = match why {
            NotRanked::NoNumbers(text) if !text.is_empty() => text.clone(),
            NotRanked::NoNumbers(_) => tr("no shot counts").to_owned(),
            NotRanked::NoDistance => tr("no distance, so no size as an angle").to_owned(),
            NotRanked::Core(groups_core::Unrankable::NoSpread) => {
                tr("its shots are all in one hole").to_owned()
            }
            NotRanked::Core(groups_core::Unrankable::Stats(error)) => error.to_string(),
        };
        super::weak(ui, &trf!("{} can't be ranked: {}.", letter(*index), reason));
    }
    if let Some(pair) = &compared.pair {
        ui.add_space(8.0);
        pair_verdict(ui, pair, &compared);
    }
    if compared.ids.len() >= 2 {
        ui.add_space(8.0);
        table(ui, &compared, &u);
    }
    ui.add_space(10.0);
    let mut next = None;
    ui.horizontal_wrapped(|ui| {
        match saved {
            Some(analysis) => {
                if crate::widgets::confirm_button(ui, "compare-delete", tr("Delete comparison")) {
                    actions.push(Action::DeleteAnalysis(analysis.id.clone()));
                    state.saved = None;
                }
            }
            None => {
                if compared.ids.len() >= 2 && super::primary(ui, tr("Save")) {
                    let name = compared
                        .ids
                        .iter()
                        .filter_map(|id| data.groups.groups.iter().find(|g| &g.id == id))
                        .map(|group| crate::groups::title_in(group, data, &u))
                        .collect::<Vec<_>>()
                        .join(" v ");
                    let name: String = name.chars().take(100).collect();
                    let analysis = Analysis::new(AnalysisKind::Compare, name, compared.ids.clone());
                    state.saved = Some(analysis.id.clone());
                    actions.push(Action::SaveAnalysis(analysis));
                }
            }
        }
        if !compared.ids.is_empty() && ui.button(tr("Composite")).clicked() {
            next = Some(Next::Composite(compared.ids.clone()));
        }
        if ui.button(tr("Clear")).clicked() {
            *state = State::default();
        }
    });
    next
}

/// With nothing chosen: how to choose, and the saved comparisons.
fn empty(ui: &mut egui::Ui, data: &Data) -> Option<Next> {
    super::weak(
        ui,
        tr(
            "Choose the groups in the library: Select (or Ctrl/⌘-click on a computer), then \
            Compare.",
        ),
    );
    let saved: Vec<&Analysis> = data
        .groups
        .analyses
        .iter()
        .filter(|analysis| analysis.kind == AnalysisKind::Compare)
        .collect();
    let mut next = None;
    if !saved.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Saved comparisons")).strong());
        for analysis in saved {
            let detail = super::analysis_detail(analysis, data);
            if crate::mobile::list_row(ui, &super::analysis_name(analysis), &detail, false)
                .clicked()
            {
                next = Some(Next::Open(analysis.id.clone()));
            }
        }
    }
    next
}

/// One card per group: its picture, size and shots, and its place.
fn cards(
    ui: &mut egui::Ui,
    compared: &Compared,
    data: &Data,
    by: crate::groups::RankBy,
    u: &UnitPrefs,
    phone: bool,
) {
    let palette = theme::palette(ui.ctx());
    let spacing = ui.spacing().item_spacing.x;
    let columns = if phone {
        2.0
    } else {
        ((ui.available_width() + spacing) / 230.0).floor().max(1.0)
    };
    let width = ((ui.available_width() + spacing) / columns - spacing).max(140.0);
    ui.horizontal_wrapped(|ui| {
        for (index, id) in compared.ids.iter().enumerate() {
            let Some(group) = data.groups.groups.iter().find(|group| &group.id == id) else {
                continue;
            };
            let measured = compared.measured[index].as_deref();
            let stats = measured.and_then(|measured| measured.stats.as_ref());
            let best = compared.best == Some(index);
            let worst = compared.worst == Some(index);
            let edge = if best {
                Stroke::new(1.5_f32, palette.good)
            } else if worst {
                Stroke::new(1.5_f32, palette.warning)
            } else {
                palette.edge
            };
            egui::Frame::new()
                .fill(palette.raised)
                .stroke(edge)
                .corner_radius(10)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    ui.set_min_height(if phone { 150.0 } else { 128.0 });
                    ui.horizontal_top(|ui| {
                        let side = if phone { 52.0 } else { 60.0 };
                        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                        crate::groups::drawing::thumbnail(ui.painter(), rect, measured, palette);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            let mut badge = letter(index);
                            if best {
                                badge = trf!("BEST · {}", badge);
                            } else if worst {
                                badge = trf!("WORST · {}", badge);
                            }
                            if let Some(rank) = compared.ranks[index].filter(|_| !best) {
                                badge = match by {
                                    crate::groups::RankBy::MeanRadius => trf!(
                                        "{} · mean radius +{:.0}%",
                                        badge,
                                        rank.above_best_percent
                                    ),
                                    crate::groups::RankBy::ExtremeSpread => trf!(
                                        "{} · extreme spread +{:.0}%",
                                        badge,
                                        rank.above_best_percent
                                    ),
                                };
                            }
                            let colour = if best {
                                palette.good
                            } else if worst {
                                palette.warning
                            } else {
                                palette.text_muted
                            };
                            ui.label(RichText::new(badge).small().strong().color(colour));
                            let title = crate::calculator::shortened(
                                &crate::groups::title_in(group, data, u),
                                if phone { 18 } else { 26 },
                            );
                            ui.label(RichText::new(title).color(palette.text));
                            // Tells apart groups whose names are cut alike.
                            let mut when = vec![super::date(group.taken_at, data)];
                            if let Some(distance) = group.distance_m {
                                when.push(u.format(Quantity::Distance, distance));
                            }
                            ui.label(
                                RichText::new(when.join(" · "))
                                    .small()
                                    .color(palette.text_muted),
                            );
                        });
                    });
                    let distance = group.distance_m;
                    let size = |mm: f64| match distance {
                        Some(d) => angle(u, groups_core::angles::mrad(mm, d)),
                        None => u.format(Quantity::GroupSize, mm),
                    };
                    match stats {
                        Some(stats) => {
                            let es = stats
                                .spread
                                .map(|spread| spread.extreme_spread.centre_to_centre);
                            let (big, small) = match (by, es) {
                                (crate::groups::RankBy::ExtremeSpread, Some(es)) => (
                                    trf!("ES {}", size(es)),
                                    trf!("MR {}", size(stats.mean_radius)),
                                ),
                                (_, es) => (
                                    trf!("MR {}", size(stats.mean_radius)),
                                    es.map_or_else(String::new, |es| trf!("ES {}", size(es))),
                                ),
                            };
                            ui.label(RichText::new(big).size(18.0).strong());
                            let shots = if stats.shots == 1 {
                                tr("1 shot").to_owned()
                            } else {
                                trf!("{} shots", stats.shots)
                            };
                            ui.weak(format!("{shots} · {small}"));
                        }
                        None => {
                            ui.weak(tr("No numbers"));
                        }
                    }
                });
        }
    });
}

/// The ratio of two σ with its range, and what it says.
fn pair_verdict(
    ui: &mut egui::Ui,
    pair: &Result<groups_core::SigmaComparison, AnalysisError>,
    compared: &Compared,
) {
    let comparison = match pair {
        Ok(comparison) => comparison,
        Err(error) => {
            let text = match error {
                AnalysisError::MixedUnits => tr(
                    "One group has a distance and the other has none, so their sizes can't be \
                     compared.",
                )
                .to_owned(),
                AnalysisError::Empty => tr("Both groups need numbers to be compared.").to_owned(),
                AnalysisError::NoSpread => {
                    tr("A group's shots are all in one hole: it has no spread to compare.")
                        .to_owned()
                }
                other => other.to_string(),
            };
            super::verdict(ui, Tone::Unsure, tr("Can't tell yet"), &[text]);
            return;
        }
    };
    let shots = |index: usize| {
        compared.measured[index]
            .as_ref()
            .and_then(|m| m.stats.as_ref())
            .map_or(0, |stats| stats.shots)
    };
    // Shown as larger over smaller, so the ratio reads 1× or more.
    let ratio = comparison.ratio;
    let (big, small, value, low, high) = if ratio.value >= 1.0 {
        (0, 1, ratio.value, ratio.range.low, ratio.range.high)
    } else {
        (
            1,
            0,
            1.0 / ratio.value,
            1.0 / ratio.range.high,
            1.0 / ratio.range.low,
        )
    };
    let (a, b) = (letter(big), letter(small));
    let times = |x: f64| format!("{x:.1}×");
    let (tone, word, line) = match comparison.verdict {
        SigmaVerdict::Same => (
            Tone::Good,
            tr("Same"),
            trf!(
                "{} and {} spread alike: the true σ ratio lies between {} and {}, under 10% \
                 apart.",
                a,
                b,
                times(low),
                times(high)
            ),
        ),
        SigmaVerdict::Different => (
            Tone::Bad,
            tr("Different"),
            trf!(
                "{} really spreads more: its σ is {} that of {}, and the true σ ratio is {} \
                 to {}.",
                a,
                times(value),
                b,
                times(low),
                times(high)
            ),
        ),
        SigmaVerdict::CantTell => (
            Tone::Unsure,
            tr("Can't tell yet"),
            trf!(
                "{}'s σ looks {} that of {}, but with {} + {} shots the true σ ratio could be \
                 anywhere from {} to {}.",
                a,
                times(value),
                b,
                shots(big),
                shots(small),
                times(low),
                times(high)
            ),
        ),
    };
    let mut lines = vec![line];
    match comparison.shots_needed {
        Some(n) if comparison.verdict != SigmaVerdict::Different => lines.push(trf!(
            "About {} shots each would settle a difference this size.",
            n
        )),
        None if value < groups_core::compare::SAME_BAND => {
            lines.push(tr("A difference under 10% isn't worth chasing with more shots.").to_owned())
        }
        _ => {}
    }
    lines.push(tr("The test compares σ, which uses every shot of both groups.").to_owned());
    super::verdict(ui, tone, word, &lines);
    super::weak(
        ui,
        &trf!(
            "σ ratio, {} to {}: {}, 95% {} to {}",
            a,
            b,
            times(value),
            times(low),
            times(high)
        ),
    );
    ratio_axis(ui, value, low, high);
}

/// The ratio on a scale around 1×, with its 95% range as a bar and the
/// band within which two groups count as the same.
fn ratio_axis(ui: &mut egui::Ui, value: f64, low: f64, high: f64) {
    let palette = theme::palette(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 52.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let left = (low.min(0.5)).ln().min(-(high.max(2.0)).ln()) * 1.12;
    let right = -left;
    let span = rect.shrink2(vec2(16.0, 0.0));
    let x = |ratio: f64| {
        let t = ((ratio.max(1e-6).ln() - left) / (right - left)) as f32;
        span.left() + t.clamp(0.0, 1.0) * span.width()
    };
    let axis_y = rect.top() + 20.0;
    painter.line_segment(
        [pos2(span.left(), axis_y), pos2(span.right(), axis_y)],
        Stroke::new(1.0_f32, palette.text_muted),
    );
    let band = egui::Rect::from_x_y_ranges(
        x(1.0 / groups_core::compare::SAME_BAND)..=x(groups_core::compare::SAME_BAND),
        (axis_y - 8.0)..=(axis_y + 8.0),
    );
    painter.rect_filled(band, 2.0, palette.good.gamma_multiply(0.25));
    let font = FontId::proportional(11.0);
    for tick in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0] {
        if tick < left.exp() * 0.99 || tick > right.exp() * 1.01 {
            continue;
        }
        let at = x(tick);
        painter.line_segment(
            [pos2(at, axis_y - 4.0), pos2(at, axis_y + 4.0)],
            Stroke::new(1.0_f32, palette.text_muted),
        );
        painter.text(
            pos2(at, axis_y + 8.0),
            Align2::CENTER_TOP,
            format!("{tick}×"),
            font.clone(),
            if tick == 1.0 {
                palette.text
            } else {
                palette.text_muted
            },
        );
    }
    painter.line_segment(
        [pos2(x(low), axis_y), pos2(x(high), axis_y)],
        Stroke::new(5.0_f32, palette.accent.gamma_multiply(0.7)),
    );
    painter.circle_filled(pos2(x(value), axis_y), 6.0, palette.accent);
    painter.text(
        pos2(x(value), axis_y - 9.0),
        Align2::CENTER_BOTTOM,
        format!("{value:.2}×"),
        font,
        palette.text,
    );
}

/// σ, vertical SD and velocity SD side by side.
fn table(ui: &mut egui::Ui, compared: &Compared, u: &UnitPrefs) {
    let palette = theme::palette(ui.ctx());
    let n = compared.ids.len();
    let stats = |index: usize| {
        compared.measured[index]
            .as_ref()
            .and_then(|measured| measured.stats)
    };
    let size = |index: usize, pick: &dyn Fn(&groups_core::GroupStats) -> Option<f64>| -> String {
        stats(index)
            .and_then(|stats| {
                let mm = pick(&stats)?;
                Some(match stats.mrad(mm) {
                    Some(mrad) => u.number(Quantity::GroupAngle, mrad, false),
                    None => u.format(Quantity::GroupSize, mm),
                })
            })
            .unwrap_or_else(|| "—".to_owned())
    };
    egui::Grid::new("compare-table")
        .striped(true)
        .num_columns(n + 1)
        .spacing(vec2(16.0, 4.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(u.label(Quantity::GroupAngle))
                    .small()
                    .color(palette.text_muted),
            );
            for index in 0..n {
                ui.label(RichText::new(letter(index)).strong());
            }
            ui.end_row();
            ui.label(tr("σ radial"));
            for index in 0..n {
                ui.label(size(index, &|stats| stats.sigma().map(|sigma| sigma.value)));
            }
            ui.end_row();
            ui.label(tr("SD vert."));
            for index in 0..n {
                ui.label(size(index, &|stats| stats.spread.map(|s| s.sd_vertical)));
            }
            ui.end_row();
            ui.label(trf!("Vel. SD ({})", u.label(Quantity::Speed)));
            for index in 0..n {
                ui.label(compared.velocity_sd[index].map_or_else(
                    || "—".to_owned(),
                    |sd| format!("{:.1}", u.show(Quantity::Speed, sd)),
                ));
            }
            ui.end_row();
        });
}

//! A composite: several groups' shots pooled into one group, shown like a
//! group's page, each group's shots in its own colour and shape.

use eframe::egui::{self, Color32, Painter, Pos2, RichText, Stroke, pos2, vec2};
use groups_core::{AnalysisError, Composite, CompositeFigures, CompositeMode, Point};

use super::pool::{self, Composed, LeftOut};
use super::{Memo, angle, key_of, letter};
use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

#[derive(Default)]
pub struct State {
    pub ids: Vec<String>,
    /// The saved analysis this is, if it is one.
    pub saved: Option<String>,
    /// The hunter's choice; `None` is the one the groups allow first.
    pub mode: Option<CompositeMode>,
    /// The distance mm are given at; `None` is the first group's.
    pub distance_m: Option<f64>,
    /// Its groups deleted since it was saved.
    pub missing: usize,
    memo: Memo<Composed>,
}

impl State {
    pub fn of(
        ids: Vec<String>,
        saved: Option<String>,
        mode: Option<CompositeMode>,
        distance_m: Option<f64>,
    ) -> State {
        State {
            ids,
            saved,
            mode,
            distance_m,
            ..State::default()
        }
    }
}

pub enum Outcome {
    Stay,
    Close,
}

pub fn mode_name(mode: CompositeMode) -> &'static str {
    match mode {
        CompositeMode::AboutAims => tr("About aims"),
        CompositeMode::AboutOwnCentres => tr("About own centres"),
        CompositeMode::ColdBore => tr("Cold-bore series"),
    }
}

/// What a mode answers, in one line.
pub fn mode_line(mode: CompositeMode) -> &'static str {
    match mode {
        CompositeMode::AboutAims => tr(
            "Each group on its aim point: where your shots go all told, the shifts between \
             groups included.",
        ),
        CompositeMode::AboutOwnCentres => {
            tr("Each group on its own centre: how precise the rifle and load are, impact left out.")
        }
        CompositeMode::ColdBore => tr(
            "Each group's cold-bore shot on its aim point: how the first shot of the day \
             spreads.",
        ),
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    data: &Data,
    stamp: u64,
    phone: bool,
    actions: &mut Vec<Action>,
) -> Outcome {
    let u = units::prefs(ui.ctx());
    let back = if phone {
        mobile::back_button(ui, tr("Analyse"))
    } else {
        ui.button(tr("‹ Back")).clicked()
    };
    if back {
        return Outcome::Close;
    }
    let ids = state.ids.clone();
    let wanted = state.mode;
    let composed = state
        .memo
        .get(key_of(stamp, (&ids, wanted.map(pool::mode_key))), || {
            pool::composite(data, &ids, wanted)
        });
    let saved = state
        .saved
        .as_ref()
        .and_then(|id| data.groups.analyses.iter().find(|a| &a.id == id));
    let heading = saved.map_or_else(
        || trf!("Composite of {} groups", composed.used.len()),
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
    if composed.used.len() < 2 {
        super::note(
            ui,
            tr("Fewer than 2 groups to pool: a composite needs at least two."),
        );
    }
    let mut mode = composed.mode;
    let choices: Vec<(CompositeMode, &str)> = composed
        .modes
        .iter()
        .map(|mode| (*mode, mode_name(*mode)))
        .collect();
    super::chips(ui, &mut mode, &choices);
    if mode != composed.mode {
        state.mode = Some(mode);
    }
    super::weak(ui, mode_line(composed.mode));
    let distance_m = state.distance_m.unwrap_or(composed.distance_m);
    match &composed.composite {
        Ok(composite) => {
            ui.add_space(4.0);
            let side = if phone {
                ui.available_width().min(460.0)
            } else {
                ui.available_width().min(520.0)
            };
            drawing(ui, composite, side, &u);
            legend(ui, &composed, data, &u);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(tr("Millimetres at"));
                let mut distance = distance_m;
                if units::drag(ui, &mut distance, Quantity::Distance, 1.0..=5000.0, 1.0).changed() {
                    state.distance_m = Some(distance.clamp(1.0, 5000.0));
                }
            });
            table(ui, composite, distance_m, &u);
            if !composite.left_out.is_empty() {
                let names: Vec<String> = composite
                    .left_out
                    .iter()
                    .filter_map(|&index| composed.used.get(index))
                    .filter_map(|id| data.groups.groups.iter().find(|g| &g.id == id))
                    .map(|group| crate::groups::title_in(group, data, &u))
                    .collect();
                super::weak(
                    ui,
                    &trf!(
                        "Left out, a single shot shows no spread about its own centre: {}.",
                        names.join(", ")
                    ),
                );
            }
        }
        Err(error) => {
            let text = match error {
                AnalysisError::Empty => tr("No group here can be pooled.").to_owned(),
                AnalysisError::NoSpread => tr("The shots have no spread at all.").to_owned(),
                other => trf!("Can't be worked out: {}", other),
            };
            super::weak(ui, &text);
        }
    }
    for (id, why) in &composed.left_out {
        let Some(group) = data.groups.groups.iter().find(|g| &g.id == id) else {
            continue;
        };
        let reason = match why {
            LeftOut::NotMeasured => tr("its shots can't be measured"),
            LeftOut::NoDistance => tr("it has no distance"),
            LeftOut::NoColdBore => tr("it has no cold-bore shot"),
        };
        super::weak(
            ui,
            &trf!(
                "Left out: {}, {}.",
                crate::groups::title_in(group, data, &u),
                reason
            ),
        );
    }
    super::weak(
        ui,
        tr(
            "A composite is a view of its groups: zero checks and trends count the groups \
            themselves, never the composite.",
        ),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| match saved {
        Some(analysis) => {
            let changed = pool::rebuild(analysis, data).mode != Some(composed.mode)
                || state
                    .distance_m
                    .is_some_and(|d| pool::rebuild(analysis, data).distance_m != Some(d));
            if changed && super::primary(ui, tr("Save changes")) {
                let mut updated = pool::composite_analysis(
                    analysis.name.clone(),
                    analysis.group_ids.clone(),
                    composed.mode,
                    distance_m,
                );
                updated.id = analysis.id.clone();
                actions.push(Action::SaveAnalysis(updated));
            }
            if crate::widgets::confirm_button(ui, "composite-delete", tr("Delete composite")) {
                actions.push(Action::DeleteAnalysis(analysis.id.clone()));
                state.saved = None;
            }
        }
        None => {
            if composed.used.len() >= 2 && super::primary(ui, tr("Save composite")) {
                let name = composite_name(&composed, data);
                let analysis = pool::composite_analysis(
                    name,
                    composed.used.clone(),
                    composed.mode,
                    distance_m,
                );
                state.saved = Some(analysis.id.clone());
                state.ids = composed.used.clone();
                actions.push(Action::SaveAnalysis(analysis));
            }
        }
    });
    Outcome::Stay
}

/// "Oryx 308 · composite of 3", from the groups' common bullet.
fn composite_name(composed: &Composed, data: &Data) -> String {
    let groups = pool::groups_of(data, &composed.used);
    let bullet = groups.first().and_then(|group| group.bullet_id.clone());
    let same = groups.iter().all(|group| group.bullet_id == bullet);
    let what = if same && bullet.is_some() {
        super::bullet_name(data, bullet.as_deref())
    } else {
        tr("Mixed loads").to_owned()
    };
    let name = trf!("{} · composite of {}", what, groups.len());
    name.chars().take(100).collect()
}

/// The shape of a group's marks: circle, square, triangle, diamond, cross.
fn mark(painter: &Painter, at: Pos2, radius: f32, index: usize, colour: Color32) {
    let stroke = Stroke::new(1.0_f32, Color32::from_black_alpha(90));
    match index % 5 {
        0 => {
            painter.circle(at, radius, colour, stroke);
        }
        1 => {
            let rect = egui::Rect::from_center_size(at, vec2(radius * 1.8, radius * 1.8));
            painter.rect(rect, 1.0, colour, stroke, egui::StrokeKind::Middle);
        }
        2 => {
            let r = radius * 1.25;
            let points = vec![
                at + vec2(0.0, -r),
                at + vec2(r * 0.87, r * 0.5),
                at + vec2(-r * 0.87, r * 0.5),
            ];
            painter.add(egui::Shape::convex_polygon(points, colour, stroke));
        }
        3 => {
            let r = radius * 1.3;
            let points = vec![
                at + vec2(0.0, -r),
                at + vec2(r, 0.0),
                at + vec2(0.0, r),
                at + vec2(-r, 0.0),
            ];
            painter.add(egui::Shape::convex_polygon(points, colour, stroke));
        }
        _ => {
            let r = radius;
            let line = Stroke::new(radius * 0.7, colour);
            painter.line_segment([at - vec2(r, r), at + vec2(r, r)], line);
            painter.line_segment([at + vec2(-r, r), at + vec2(r, -r)], line);
        }
    }
}

/// The pooled points on paper, in mrad: each group in its colour and
/// shape, a ring at each group's centre, the mean radius about the
/// composite's centre and a cross on that centre.
fn drawing(ui: &mut egui::Ui, composite: &Composite, side: f32, u: &UnitPrefs) {
    let palette = theme::palette(ui.ctx());
    let paper = if palette.dark {
        theme::BONE
    } else {
        theme::SUN.sunken
    };
    let ink = theme::SUN;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect(rect, 6.0, paper, palette.edge, egui::StrokeKind::Inside);
    let centre = composite.mrad.impact.unwrap_or(Point::ORIGIN);
    let mut points: Vec<Point> = composite.points_mrad.clone();
    points.push(Point::ORIGIN);
    let (mut low, mut high) = (
        Point::new(f64::MAX, f64::MAX),
        Point::new(f64::MIN, f64::MIN),
    );
    for point in &points {
        low = Point::new(low.x.min(point.x), low.y.min(point.y));
        high = Point::new(high.x.max(point.x), high.y.max(point.y));
    }
    let middle = Point::new((low.x + high.x) / 2.0, (low.y + high.y) / 2.0);
    let half = ((high.x - low.x).max(high.y - low.y) / 2.0)
        .max(composite.mrad.mean_radius * 1.2)
        .max(0.05)
        * 1.15;
    let scale = (rect.width() as f64 - 16.0) / (2.0 * half);
    let at = |point: Point| {
        rect.center()
            + vec2(
                ((point.x - middle.x) * scale) as f32,
                (-(point.y - middle.y) * scale) as f32,
            )
    };
    // Grid lines at a round step of the hunter's angle unit.
    let unit = u.unit(Quantity::GroupAngle);
    let per_unit = 1.0 / unit.show(1.0);
    let step = [0.01, 0.02, 0.05, 0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|shown| shown * per_unit)
        .find(|mrad| mrad * scale >= 24.0)
        .unwrap_or(10.0 * per_unit);
    let grid = Stroke::new(1.0_f32, ink.stroke.gamma_multiply(0.35));
    let reach = half * 1.2;
    let mut line = (middle.x - reach).div_euclid(step) * step;
    while line <= middle.x + reach {
        let x = at(Point::new(line, 0.0)).x;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], grid);
        line += step;
    }
    let mut line = (middle.y - reach).div_euclid(step) * step;
    while line <= middle.y + reach {
        let y = at(Point::new(0.0, line)).y;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], grid);
        line += step;
    }
    if composite.mode != CompositeMode::AboutOwnCentres {
        let aim = at(Point::ORIGIN);
        let stroke = Stroke::new(1.6_f32, ink.alarm);
        painter.line_segment([aim - vec2(9.0, 0.0), aim + vec2(9.0, 0.0)], stroke);
        painter.line_segment([aim - vec2(0.0, 9.0), aim + vec2(0.0, 9.0)], stroke);
    }
    let groups = composite
        .point_groups
        .iter()
        .copied()
        .max()
        .map_or(0, |g| g + 1);
    let radius = (side / 70.0).clamp(3.0, 6.0);
    for (point, group) in composite.points_mrad.iter().zip(&composite.point_groups) {
        mark(&painter, at(*point), radius, *group, super::colour(*group));
    }
    for group in 0..groups {
        let members: Vec<Point> = composite
            .points_mrad
            .iter()
            .zip(&composite.point_groups)
            .filter(|(_, g)| **g == group)
            .map(|(p, _)| *p)
            .collect();
        if members.is_empty() {
            continue;
        }
        let n = members.len() as f64;
        let mean = Point::new(
            members.iter().map(|p| p.x).sum::<f64>() / n,
            members.iter().map(|p| p.y).sum::<f64>() / n,
        );
        painter.circle_stroke(
            at(mean),
            radius * 2.2,
            Stroke::new(1.5_f32, super::colour(group)),
        );
    }
    let c = at(centre);
    painter.circle_stroke(
        c,
        (composite.mrad.mean_radius * scale) as f32,
        Stroke::new(1.2_f32, ink.accent.gamma_multiply(0.8)),
    );
    let stroke = Stroke::new(2.0_f32, ink.accent);
    let arm = 8.0;
    painter.line_segment([c - vec2(arm, arm), c + vec2(arm, arm)], stroke);
    painter.line_segment([c + vec2(-arm, arm), c + vec2(arm, -arm)], stroke);
    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        trf!("grid {}", angle(u, step)),
        egui::FontId::proportional(11.0),
        ink.text_muted,
    );
}

/// Which colour and shape is which group.
fn legend(ui: &mut egui::Ui, composed: &Composed, data: &Data, u: &UnitPrefs) {
    let palette = theme::palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        for (index, id) in composed.used.iter().enumerate() {
            let Some(group) = data.groups.groups.iter().find(|g| &g.id == id) else {
                continue;
            };
            let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
            mark(
                ui.painter(),
                rect.center(),
                5.0,
                index,
                super::colour(index),
            );
            let title = crate::calculator::shortened(&crate::groups::title_in(group, data, u), 22);
            ui.label(
                RichText::new(format!("{} {}", letter(index), title)).color(palette.text_muted),
            );
            ui.add_space(6.0);
        }
    });
}

fn table(ui: &mut egui::Ui, composite: &Composite, distance_m: f64, u: &UnitPrefs) {
    let palette = theme::palette(ui.ctx());
    let Ok(mm) = composite.mm_at(distance_m) else {
        return;
    };
    let f: &CompositeFigures = &composite.mrad;
    let a = |mrad: f64| u.number(Quantity::GroupAngle, mrad, false);
    let m = |mm: f64| u.number(Quantity::GroupSize, mm, false);
    let range = |low: f64, high: f64| format!("{}–{}", a(low), a(high));
    let shots = trf!("{} shots in {} groups", composite.shots, composite.groups);
    ui.label(RichText::new(shots).strong());
    egui::Grid::new("composite-table")
        .striped(true)
        .num_columns(4)
        .spacing(vec2(14.0, 4.0))
        .show(ui, |ui| {
            ui.label("");
            for heading in [
                u.label(Quantity::GroupAngle).to_owned(),
                format!(
                    "{} @ {}",
                    u.label(Quantity::GroupSize),
                    u.format(Quantity::Distance, distance_m)
                ),
                tr("95%").to_owned(),
            ] {
                ui.label(RichText::new(heading).small().color(palette.text_muted));
            }
            ui.end_row();
            let mut row = |label: &str, angle: String, millimetres: String, extra: String| {
                ui.label(label);
                ui.label(angle);
                ui.label(millimetres);
                ui.label(RichText::new(extra).color(palette.text_muted));
                ui.end_row();
            };
            row(
                tr("Extreme spread"),
                a(f.extreme_spread),
                m(mm.extreme_spread),
                f.extreme_spread_range
                    .map_or_else(String::new, |r| range(r.low, r.high)),
            );
            row(
                tr("Mean radius"),
                a(f.mean_radius),
                m(mm.mean_radius),
                range(f.mean_radius_range.low, f.mean_radius_range.high),
            );
            row(
                tr("Median radius"),
                a(f.median_radius),
                m(mm.median_radius),
                String::new(),
            );
            row(
                tr("σ (radial)"),
                a(f.sigma.value),
                m(mm.sigma.value),
                range(f.sigma.range.low, f.sigma.range.high),
            );
            row(
                tr("CEP · R90 · R95"),
                format!(
                    "{} · {} · {}",
                    a(f.cep.value),
                    a(f.r90.value),
                    a(f.r95.value)
                ),
                format!(
                    "{} · {} · {}",
                    m(mm.cep.value),
                    m(mm.r90.value),
                    m(mm.r95.value)
                ),
                String::new(),
            );
            row(
                tr("SD horiz. · vert."),
                format!("{} · {}", a(f.sd_horizontal), a(f.sd_vertical)),
                format!("{} · {}", m(mm.sd_horizontal), m(mm.sd_vertical)),
                String::new(),
            );
            row(
                tr("Width × height"),
                format!("{} × {}", a(f.width), a(f.height)),
                format!("{} × {}", m(mm.width), m(mm.height)),
                String::new(),
            );
            if let (Some(impact), Some(impact_mm)) = (f.impact, mm.impact) {
                row(
                    tr("Impact"),
                    super::arrows(u, impact),
                    format!("{} · {}", m(impact_mm.x), m(impact_mm.y)),
                    String::new(),
                );
            }
        });
    if let Some(within) = composite.within_group_sigma {
        super::weak(
            ui,
            &trf!(
                "σ within the groups: {}. Well below the composite's σ, the impact moved \
                 between groups; the zero check tells by how much.",
                angle(u, within.value)
            ),
        );
    }
}

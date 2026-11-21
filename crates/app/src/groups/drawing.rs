//! A group drawn as vectors on a square of paper: the library's small
//! picture of it, and the group page's drawing in true proportion with its
//! aim, centre, mean radius and CEP.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use groups_core::{GroupStats, Point};

use super::measure::Measured;
use crate::i18n::{tr, trf};
use crate::theme::{self, Palette};
use crate::units::{self, Quantity};

/// The colours marks have on the paper, which is light in every theme.
struct Ink {
    paper: Color32,
    grid: Color32,
    hole: Color32,
    cold_bore: Color32,
    flier: Color32,
    aim: Color32,
    centre: Color32,
    cep: Color32,
}

fn ink(palette: &Palette) -> Ink {
    let marks = &theme::SUN;
    Ink {
        paper: if palette.dark {
            theme::BONE
        } else {
            marks.sunken
        },
        grid: marks.stroke.gamma_multiply(0.35),
        hole: marks.text,
        cold_bore: marks.measure,
        flier: marks.warning,
        aim: marks.alarm,
        centre: marks.accent,
        cep: marks.measure,
    }
}

/// What kind of mark a shot is.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Counted,
    ColdBore,
    Flier,
}

fn kind(measured: &Measured, index: usize) -> Kind {
    if measured.cold_bore.contains(&index) {
        Kind::ColdBore
    } else if measured.called_fliers.contains(&index) {
        Kind::Flier
    } else {
        Kind::Counted
    }
}

/// Millimetres on the target to points on screen: the target's `centre`
/// at the rect's middle, `scale` points per mm, y up.
#[derive(Clone, Copy)]
struct Fit {
    centre: Point,
    middle: Pos2,
    scale: f32,
}

impl Fit {
    /// Fits `points` and `extra_mm` around them in `rect`, showing at least
    /// `min_half_mm` each way from their middle.
    fn around(points: &[Point], extra_mm: f64, min_half_mm: f64, rect: Rect) -> Fit {
        let (mut low, mut high) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for point in points {
            low = Point::new(low.x.min(point.x), low.y.min(point.y));
            high = Point::new(high.x.max(point.x), high.y.max(point.y));
        }
        if points.is_empty() {
            (low, high) = (Point::ORIGIN, Point::ORIGIN);
        }
        let centre = Point::new((low.x + high.x) / 2.0, (low.y + high.y) / 2.0);
        let half = ((high.x - low.x).max(high.y - low.y) / 2.0 + extra_mm).max(min_half_mm) * 1.12;
        Fit {
            centre,
            middle: rect.center(),
            scale: (rect.width().min(rect.height()) as f64 / (2.0 * half)) as f32,
        }
    }

    fn at(&self, point: Point) -> Pos2 {
        self.middle
            + vec2(
                ((point.x - self.centre.x) * f64::from(self.scale)) as f32,
                (-(point.y - self.centre.y) * f64::from(self.scale)) as f32,
            )
    }

    fn length(&self, mm: f64) -> f32 {
        (mm * f64::from(self.scale)) as f32
    }
}

/// The points the paper is framed on: the shots that count and the aim,
/// so a flier or a cold-bore shot far out doesn't shrink the group.
fn framed_points(measured: &Measured) -> Vec<Point> {
    let counted: Vec<Point> = measured
        .counted
        .iter()
        .map(|&i| measured.shots_mm[i])
        .collect();
    let shots = if counted.is_empty() {
        measured.shots_mm.clone()
    } else {
        counted
    };
    shots.into_iter().chain(measured.aim_mm).collect()
}

/// Where the line from the rect's middle towards `at` leaves `rect`.
fn edge_towards(rect: Rect, at: Pos2) -> (Pos2, Vec2) {
    let from = rect.center();
    let direction = (at - from).normalized();
    let half = rect.size() / 2.0;
    let reach = |half: f32, d: f32| {
        if d.abs() < 1e-6 {
            f32::INFINITY
        } else {
            half / d.abs()
        }
    };
    let t = reach(half.x, direction.x).min(reach(half.y, direction.y));
    (from + direction * t, direction)
}

fn grid_lines(painter: &Painter, rect: Rect, fit: &Fit, colour: Color32) -> f64 {
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
        .into_iter()
        .find(|step| fit.length(*step) >= 26.0)
        .unwrap_or(1000.0);
    let stroke = Stroke::new(1.0_f32, colour);
    let half_x = f64::from(rect.width() / 2.0 / fit.scale);
    let half_y = f64::from(rect.height() / 2.0 / fit.scale);
    let first = |from: f64| (from / step).ceil() as i64;
    for i in first(fit.centre.x - half_x)..=((fit.centre.x + half_x) / step).floor() as i64 {
        let x = fit.at(Point::new(i as f64 * step, 0.0)).x;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
    }
    for i in first(fit.centre.y - half_y)..=((fit.centre.y + half_y) / step).floor() as i64 {
        let y = fit.at(Point::new(0.0, i as f64 * step)).y;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }
    step
}

fn cross(painter: &Painter, at: Pos2, half: f32, stroke: Stroke) {
    painter.line_segment([at - vec2(half, 0.0), at + vec2(half, 0.0)], stroke);
    painter.line_segment([at - vec2(0.0, half), at + vec2(0.0, half)], stroke);
}

fn saltire(painter: &Painter, at: Pos2, half: f32, stroke: Stroke) {
    painter.line_segment([at - vec2(half, half), at + vec2(half, half)], stroke);
    painter.line_segment([at + vec2(-half, half), at + vec2(half, -half)], stroke);
}

fn dashed_circle(painter: &Painter, centre: Pos2, radius: f32, stroke: Stroke) {
    let steps = 72;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
            centre + radius * vec2(angle.cos(), angle.sin())
        })
        .collect();
    let dash = (radius * 0.35).clamp(2.0, 5.0);
    painter.extend(egui::Shape::dashed_line(&points, stroke, dash, dash * 0.8));
}

/// A shot as a hole of `radius`, in its kind's colours.
fn hole(painter: &Painter, at: Pos2, radius: f32, kind: Kind, ink: &Ink) {
    match kind {
        Kind::Counted => {
            painter.circle_filled(at, radius, ink.hole);
        }
        Kind::ColdBore => {
            painter.circle_filled(at, radius, ink.cold_bore);
        }
        Kind::Flier => {
            painter.circle_filled(at, radius, ink.flier.gamma_multiply(0.35));
            painter.circle_stroke(at, radius, Stroke::new(1.5_f32, ink.flier));
        }
    }
}

/// The library's picture of a group: its shots as dots on paper, scaled to
/// fit those that count; shots further out sit at the edge.
pub fn thumbnail(painter: &Painter, rect: Rect, measured: Option<&Measured>, palette: &Palette) {
    let ink = ink(palette);
    painter.rect(rect, 4.0, ink.paper, palette.edge, egui::StrokeKind::Inside);
    let Some(measured) = measured else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(rect.height() * 0.45),
            ink.grid.gamma_multiply(2.5),
        );
        return;
    };
    let inner = rect.shrink(4.0);
    let fit = Fit::around(&framed_points(measured), 2.0, 8.0, inner);
    if let Some(aim) = measured.aim_mm {
        cross(painter, fit.at(aim), 3.0, Stroke::new(1.0_f32, ink.aim));
    }
    let dot = 1.8_f32.max(rect.width() / 26.0);
    for (index, shot) in measured.shots_mm.iter().enumerate() {
        let mut at = fit.at(*shot);
        if !inner.contains(at) {
            at = edge_towards(inner, at).0;
        }
        hole(painter, at, dot, kind(measured, index), &ink);
    }
}

/// The group in true proportion on a square of paper `side` points wide,
/// framed on the shots that count and the aim: bullet-sized holes, the aim
/// and the centre as crosses, the mean radius and (dashed) CEP about the
/// centre, and cold-bore shots and called fliers in their own colours.
/// Shots beyond the paper show as an arrow and their number at the edge.
/// The response is clickable.
pub fn group(
    ui: &mut egui::Ui,
    measured: &Measured,
    stats: Option<&GroupStats>,
    calibre_mm: Option<f64>,
    side: f32,
) -> egui::Response {
    let palette = theme::palette(ui.ctx());
    let ink = ink(palette);
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect(rect, 6.0, ink.paper, palette.edge, egui::StrokeKind::Inside);
    let radius_mm = calibre_mm.unwrap_or(6.0) / 2.0;
    let mut points = framed_points(measured);
    let mut extra = radius_mm;
    if let Some(stats) = stats {
        points.push(stats.centre);
        let cep = stats.spread.map_or(0.0, |spread| spread.cep.value);
        extra = extra.max(stats.mean_radius.max(cep) - stats.width.max(stats.height) / 2.0);
    }
    let inner = rect.shrink(8.0);
    let fit = Fit::around(&points, extra, 3.5 * 2.0 * radius_mm, inner);
    let step = grid_lines(&painter, rect, &fit, ink.grid);

    let radius = fit.length(radius_mm).max(2.5);
    let small = side < 240.0;
    for (index, shot) in measured.shots_mm.iter().enumerate() {
        let at = fit.at(*shot);
        let kind = kind(measured, index);
        if !inner.expand(radius).contains(at) {
            off_paper(&painter, inner.shrink(4.0), at, index + 1, kind, &ink);
            continue;
        }
        hole(&painter, at, radius, kind, &ink);
        painter.circle_stroke(
            at,
            radius,
            Stroke::new(1.0_f32, ink.paper.gamma_multiply(0.6)),
        );
        if radius >= 7.0 && !small {
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                (index + 1).to_string(),
                egui::FontId::proportional((radius * 0.9).min(13.0)),
                ink.paper,
            );
        }
    }
    if let Some(stats) = stats {
        let centre = fit.at(stats.centre);
        if stats.shots >= 2 {
            painter.circle_stroke(
                centre,
                fit.length(stats.mean_radius),
                Stroke::new(1.5_f32, ink.centre),
            );
        }
        if let Some(spread) = stats.spread {
            dashed_circle(
                &painter,
                centre,
                fit.length(spread.cep.value),
                Stroke::new(1.3_f32, ink.cep),
            );
        }
    }
    let marker = if small { 6.0 } else { 9.0 };
    if let Some(aim) = measured.aim_mm {
        cross(&painter, fit.at(aim), marker, Stroke::new(1.6_f32, ink.aim));
    }
    if let Some(stats) = stats {
        saltire(
            &painter,
            fit.at(stats.centre),
            marker * 0.7,
            Stroke::new(1.6_f32, ink.centre),
        );
    }
    if !small {
        let u = units::prefs(ui.ctx());
        painter.text(
            rect.left_bottom() + vec2(8.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            trf!("grid {}", u.format(Quantity::GroupSize, step)),
            egui::FontId::proportional(11.0),
            theme::SUN.text_muted,
        );
    }
    response
}

/// A shot beyond the paper: an arrow at the edge pointing its way, with
/// its number.
fn off_paper(painter: &Painter, rect: Rect, at: Pos2, number: usize, kind: Kind, ink: &Ink) {
    let (edge, direction) = edge_towards(rect, at);
    let colour = match kind {
        Kind::Counted => ink.hole,
        Kind::ColdBore => ink.cold_bore,
        Kind::Flier => ink.flier,
    };
    let side = vec2(-direction.y, direction.x);
    let tip = edge;
    let base = edge - direction * 9.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, base + side * 5.0, base - side * 5.0],
        colour,
        Stroke::NONE,
    ));
    painter.text(
        base - direction * 8.0,
        egui::Align2::CENTER_CENTER,
        number.to_string(),
        egui::FontId::proportional(11.0),
        colour,
    );
}

/// What the drawing's marks mean, with the marks themselves as the key.
pub fn legend(ui: &mut egui::Ui, measured: &Measured, stats: bool) {
    let palette = theme::palette(ui.ctx());
    let ink = ink(palette);
    #[derive(Clone, Copy)]
    enum Swatch {
        Aim,
        Centre,
        Ring,
        DashedRing,
        Hole(Kind),
    }
    let mut items: Vec<(&str, Swatch)> = Vec::new();
    if measured.aim_mm.is_some() {
        items.push((tr("aim"), Swatch::Aim));
    }
    if stats {
        items.push((tr("centre"), Swatch::Centre));
        items.push((tr("mean radius"), Swatch::Ring));
        items.push((tr("CEP"), Swatch::DashedRing));
    }
    if !measured.cold_bore.is_empty() {
        items.push((tr("cold bore"), Swatch::Hole(Kind::ColdBore)));
    }
    if !measured.called_fliers.is_empty() {
        items.push((tr("called flier"), Swatch::Hole(Kind::Flier)));
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (label, swatch) in items {
            let text =
                ui.painter()
                    .layout_no_wrap(label.to_owned(), font.clone(), palette.text_muted);
            let size = vec2(24.0 + text.size().x, text.size().y.max(18.0));
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let key = egui::Rect::from_min_size(rect.left_top(), vec2(18.0, size.y));
            let painter = ui.painter();
            painter.rect_filled(
                key.shrink2(vec2(0.0, (size.y - 18.0) / 2.0)),
                3.0,
                ink.paper,
            );
            let at = key.center();
            let thin = Stroke::new(1.4_f32, ink.centre);
            match swatch {
                Swatch::Aim => cross(painter, at, 6.0, Stroke::new(1.6_f32, ink.aim)),
                Swatch::Centre => saltire(painter, at, 4.5, Stroke::new(1.6_f32, ink.centre)),
                Swatch::Ring => {
                    painter.circle_stroke(at, 6.0, thin);
                }
                Swatch::DashedRing => {
                    dashed_circle(painter, at, 6.0, Stroke::new(1.3_f32, ink.cep));
                }
                Swatch::Hole(kind) => hole(painter, at, 5.0, kind, &ink),
            }
            painter.galley(
                rect.left_top() + vec2(24.0, (size.y - text.size().y) / 2.0),
                text,
                palette.text_muted,
            );
        }
    });
}

//! The stand card: a map of what can be shot from a hunting stand, with
//! range rings and their holds, the ethical limit and the dead-on zone,
//! the shooting sector and the directions never to shoot, and the
//! landmarks' ranges. Printed and kept at the stand.

use ballistics_shot::Hold;
use serde::{Deserialize, Serialize};

use super::dope::Measure;
use super::sheet::{Align, Ink, PT, Sheet, arc, fit, text_width};
use super::{CardSize, Subject};
use crate::units::{Quantity, UnitPrefs};
use crate::weather::compass_point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum View {
    /// 90° ahead.
    Quarter,
    /// 180°: ahead and to both sides.
    Half,
    /// All round.
    Full,
}

impl View {
    /// Degrees each side of the facing direction.
    pub fn half_angle(self) -> f64 {
        match self {
            View::Quarter => 45.0,
            View::Half => 90.0,
            View::Full => 180.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            View::Quarter => "90°",
            View::Half => "180°",
            View::Full => "360°",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Landmark {
    pub name: String,
    pub range_m: f64,
    /// Compass direction from the stand, degrees.
    pub bearing_degrees: f64,
}

impl Default for Landmark {
    fn default() -> Self {
        Landmark {
            name: String::new(),
            range_m: 100.0,
            bearing_degrees: 0.0,
        }
    }
}

/// A direction never to shoot in (a road, a house, the next stand),
/// clockwise from `from` to `to`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoShoot {
    pub name: String,
    pub from_degrees: f64,
    pub to_degrees: f64,
}

impl Default for NoShoot {
    fn default() -> Self {
        NoShoot {
            name: String::new(),
            from_degrees: 0.0,
            to_degrees: 20.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stand {
    pub name: String,
    /// Where the stand looks, degrees from north.
    pub facing_degrees: f64,
    pub view: View,
    /// The shooting sector, left and right edge (degrees from north), if
    /// the hunt leader gave one.
    pub sector: Option<[f64; 2]>,
    pub landmarks: Vec<Landmark>,
    pub no_shoot: Vec<NoShoot>,
    pub note: String,
}

impl Default for Stand {
    fn default() -> Self {
        Stand {
            name: "Stand 1".into(),
            facing_degrees: 0.0,
            view: View::Half,
            sector: None,
            landmarks: Vec::new(),
            no_shoot: Vec::new(),
            note: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StandOptions {
    pub size: CardSize,
    pub landscape: bool,
    /// Metres between the rings; `None` picks.
    pub ring_step_m: Option<f64>,
    /// The outermost ring; `None` reaches past the landmarks and the limit.
    pub max_range_m: Option<f64>,
    pub hold_as: Measure,
}

impl Default for StandOptions {
    fn default() -> Self {
        StandOptions {
            size: CardSize::A5,
            landscape: false,
            ring_step_m: None,
            max_range_m: None,
            hold_as: Measure::Clicks,
        }
    }
}

/// The rings: the step between them and the outermost.
pub fn rings(stand: &Stand, options: &StandOptions, ethical_m: Option<f64>) -> (f64, f64) {
    let farthest = stand
        .landmarks
        .iter()
        .map(|landmark| landmark.range_m)
        .fold(0.0, f64::max);
    let wanted = options
        .max_range_m
        .unwrap_or_else(|| (farthest.max(ethical_m.unwrap_or(200.0)) * 1.08).max(100.0));
    let step = options.ring_step_m.unwrap_or_else(|| {
        [25.0, 50.0, 100.0, 200.0, 250.0, 500.0]
            .into_iter()
            .find(|step| wanted / step <= 6.0)
            .unwrap_or(500.0)
    });
    let outer = if options.max_range_m.is_some() {
        wanted
    } else {
        (wanted / step).ceil() * step
    };
    (step, outer.max(step))
}

/// The angle on the card for a compass direction: degrees clockwise from
/// straight up, between -180 and 180.
fn on_card(stand: &Stand, bearing: f64) -> f64 {
    (bearing - stand.facing_degrees + 180.0).rem_euclid(360.0) - 180.0
}

/// Whether a compass direction lies clockwise from `from` to `to`.
fn between(bearing: f64, from: f64, to: f64) -> bool {
    (bearing - from).rem_euclid(360.0) <= (to - from).rem_euclid(360.0) + 1e-9
}

/// The parts of the view (card angles) where `inside` holds, split at the
/// given compass directions.
fn spans(stand: &Stand, edges: &[f64], inside: impl Fn(f64) -> bool) -> Vec<(f64, f64)> {
    let half = stand.view.half_angle();
    let mut cuts: Vec<f64> = edges
        .iter()
        .map(|&bearing| on_card(stand, bearing))
        .filter(|angle| angle.abs() < half)
        .collect();
    cuts.extend([-half, half]);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for pair in cuts.windows(2) {
        let middle = (pair[0] + pair[1]) / 2.0;
        if !inside(stand.facing_degrees + middle) {
            continue;
        }
        match spans.last_mut() {
            Some(last) if (last.1 - pair[0]).abs() < 1e-9 => last.1 = pair[1],
            _ => spans.push((pair[0], pair[1])),
        }
    }
    spans
}

/// A band between two radii over card angles `from` to `to`, in pieces
/// small enough to fill as simple shapes (overlapping a little, so no
/// hairline shows between them).
fn band(center: [f32; 2], inner: f32, outer: f32, from: f32, to: f32) -> Vec<Vec<[f32; 2]>> {
    let pieces = ((to - from) / 90.0).ceil().max(1.0) as usize;
    let step = (to - from) / pieces as f32;
    (0..pieces)
        .map(|piece| {
            let start = from + step * piece as f32;
            let end = (start + step + 0.4).min(to);
            let mut points = arc(center, outer, start, end);
            if inner > 0.01 {
                points.extend(arc(center, inner, end, start));
            } else {
                points.push(center);
            }
            points
        })
        .collect()
}

pub fn layout(
    subject: &Subject,
    stand: &Stand,
    options: &StandOptions,
    u: &UnitPrefs,
) -> Result<Sheet, String> {
    let [width, height] = options.size.oriented(options.landscape);
    let today = subject.conditions.temperature_c;
    let probe = subject
        .solve(today, 0.0, 3000.0_f64.min(5000.0))
        .map_err(|error| error.to_string())?;
    let ethical_m = subject.ethical_limit(&probe);
    let (step, outer_m) = rings(stand, options, ethical_m);
    let solution = subject
        .solve(today, 0.0, outer_m + 10.0)
        .map_err(|error| error.to_string())?;
    let dead_on = subject.dead_on(&solution);
    let rifle = &subject.rifle;
    let hold_text = |range: f64| -> String {
        let Some(row) = solution.at(range) else {
            return "–".into();
        };
        let hold = Hold::new(rifle, &row);
        match options.hold_as {
            Measure::Clicks => format!("{:+} cl", hold.elevation_clicks).replace("+0 ", "0 "),
            Measure::Angle => format!(
                "{:+.1} {}",
                hold.elevation,
                ballistics_shot::unit_label(rifle.click_unit)
            ),
            Measure::Length => u.format_signed(Quantity::ShortLength, hold.drop_m * 100.0),
        }
    };

    let mut s = (width.min(height) / 17.0).clamp(6.5, 10.0);
    let margin = (width.min(height) * 0.055).clamp(5.0, 10.0);
    let inner = width - 2.0 * margin;
    let mut landmarks: Vec<&Landmark> = stand.landmarks.iter().collect();
    landmarks.sort_by(|a, b| a.range_m.total_cmp(&b.range_m));
    let mut legend = vec![format!(
        "Rings every {}: range, and the hold{}.",
        u.format(Quantity::Distance, step),
        match options.hold_as {
            Measure::Length => " (where a dead-on hold hits)",
            _ => " up",
        }
    )];
    if let Some(dead_on) = dead_on {
        legend.push(format!(
            "Green: hold dead on, out to {}.",
            u.format(Quantity::Distance, dead_on.far_limit)
        ));
    }
    if let (Some(limit), Some(target)) = (ethical_m, &subject.target) {
        legend.push(format!(
            "Red: beyond the ethical limit for {} ({}).",
            target.to_lowercase(),
            u.format(Quantity::Distance, limit)
        ));
    }
    if stand.sector.is_some() {
        legend.push("Grey: outside the shooting sector.".into());
    }
    if !stand.no_shoot.is_empty() {
        legend.push("Dark, hatched: never shoot this way.".into());
    }
    // Shrink the text until the map keeps at least 40 % of the card.
    let (header, table, footer) = loop {
        let line = s * PT * 1.3;
        let header = s * 1.6 * PT * 1.3
            + 2.0 * s * 0.9 * PT * 1.3
            + if stand.note.is_empty() { 0.0 } else { line };
        let table = if landmarks.is_empty() {
            0.0
        } else {
            (landmarks.len() + 1) as f32 * s * PT * 1.5 + 2.0
        };
        let footer = (legend.len() + 1) as f32 * s * 0.85 * PT * 1.3;
        let map = height - 2.0 * margin - header - table - footer;
        if map >= 0.4 * height || s <= 4.5 {
            break (header, table, footer);
        }
        s *= 0.92;
    };

    let mut sheet = Sheet::new(width, height);
    let mut y = margin;
    let line = |sheet: &mut Sheet, y: &mut f32, text: &str, size: f32, bold: bool, ink: Ink| {
        sheet.text(
            [margin, *y + size * PT * 0.95],
            fit(text, size, bold, inner),
            size,
            bold,
            Align::Left,
            ink,
        );
        *y += size * PT * 1.3;
    };
    line(&mut sheet, &mut y, &stand.name, s * 1.6, true, Ink::Black);
    line(
        &mut sheet,
        &mut y,
        &format!(
            "{} · {}{}",
            rifle.name,
            subject.bullet.display_name(),
            subject
                .target
                .as_ref()
                .map_or(String::new(), |target| format!(" · {target}"))
        ),
        s * 0.9,
        false,
        Ink::Grey,
    );
    line(
        &mut sheet,
        &mut y,
        &format!(
            "Faces {} ({:.0}°) · {} · zero {}",
            compass_point(stand.facing_degrees),
            stand.facing_degrees.rem_euclid(360.0),
            u.format(Quantity::Temperature, today),
            crate::calculator::zero_text(u, rifle)
        ),
        s * 0.9,
        false,
        Ink::Grey,
    );
    if !stand.note.is_empty() {
        line(&mut sheet, &mut y, &stand.note, s, false, Ink::Black);
    }
    debug_assert!((y - margin - header).abs() < 0.5);

    // The map: fit the fan into its box, with room for the labels.
    let label = s * PT * 2.6;
    let top = y + label;
    let bottom = height - margin - table - footer - label * 0.6;
    let half = stand.view.half_angle() as f32;
    // The map's radius, and how far below the stand its labels reach.
    let (radius, below) = match stand.view {
        View::Full => (
            ((bottom - top - label) / 2.0).min(inner / 2.0 - label),
            label,
        ),
        View::Half => ((bottom - top - label).min(inner / 2.0 - label * 0.4), label),
        View::Quarter => (
            (bottom - top - label).min((inner / 2.0 - label * 0.4) / 45f32.to_radians().sin()),
            label * 0.6,
        ),
    };
    let radius = radius.max(5.0);
    let center = [width / 2.0, top + radius];
    let map_bottom = match stand.view {
        View::Full => center[1] + radius + below,
        _ => center[1] + below,
    };
    let scale = radius / outer_m as f32;
    let at = |angle: f32, range: f32| {
        let angle = angle.to_radians();
        [
            center[0] + range * scale * angle.sin(),
            center[1] - range * scale * angle.cos(),
        ]
    };
    for piece in band(center, 0.0, radius, -half, half) {
        sheet.fill(piece, Ink::White);
    }
    if let Some(dead_on) = dead_on {
        let far = (dead_on.far_limit as f32 * scale).min(radius);
        for piece in band(center, 0.0, far, -half, half) {
            sheet.fill(piece, Ink::GreenShade);
        }
    }
    if let Some(limit) = ethical_m.filter(|limit| *limit < outer_m) {
        for piece in band(center, limit as f32 * scale, radius, -half, half) {
            sheet.fill(piece, Ink::RedShade);
        }
    }
    let mut edges: Vec<f64> = Vec::new();
    if let Some([left, right]) = stand.sector {
        edges.extend([left, right]);
        for (from, to) in spans(stand, &edges, |bearing| !between(bearing, left, right)) {
            for piece in band(center, 0.0, radius, from as f32, to as f32) {
                sheet.fill(piece, Ink::Shade);
            }
        }
    }
    for no in &stand.no_shoot {
        for (from, to) in spans(stand, &[no.from_degrees, no.to_degrees], |bearing| {
            between(bearing, no.from_degrees, no.to_degrees)
        }) {
            let (from, to) = (from as f32, to as f32);
            for piece in band(center, 0.0, radius, from, to) {
                sheet.fill(piece, Ink::Block);
            }
            // Hatching: short strokes across the wedge.
            let mut range = step as f32 / 2.0;
            while range < outer_m as f32 {
                let span = (to - from).min(80.0);
                let mid = (from + to) / 2.0;
                sheet.polyline(
                    arc(center, range * scale, mid - span / 2.0, mid + span / 2.0),
                    0.2,
                    Ink::Grey,
                    true,
                    false,
                );
                range += step as f32 / 2.0;
            }
            for edge in [from, to] {
                if edge.abs() < half - 0.01 {
                    sheet.line(center, at(edge, outer_m as f32), 0.5, Ink::Red);
                }
            }
            let mid = (from + to) / 2.0;
            if !no.name.is_empty() {
                let [x, y] = at(mid, outer_m as f32 * 0.82);
                sheet.text(
                    [x, y],
                    format!("No shot: {}", no.name),
                    s * 0.8,
                    true,
                    Align::Center,
                    Ink::Red,
                );
            }
        }
    }

    // Rings and their labels.
    let mut range = step;
    while range <= outer_m + 1e-6 {
        let r = range as f32 * scale;
        sheet.polyline(
            arc(center, r, -half, half),
            0.2,
            Ink::Rule,
            false,
            half >= 180.0,
        );
        let text = u.format(Quantity::Distance, range);
        let hold = hold_text(range);
        match stand.view {
            View::Half => {
                let below = center[1] + s * PT * 1.1;
                sheet.text(
                    [center[0] - r, below],
                    &text,
                    s * 0.8,
                    true,
                    Align::Center,
                    Ink::Black,
                );
                sheet.text(
                    [center[0] + r, below],
                    &hold,
                    s * 0.8,
                    false,
                    Align::Center,
                    Ink::Black,
                );
            }
            View::Quarter => {
                let [x, y] = at(-half, range as f32);
                let [x2, y2] = at(half, range as f32);
                sheet.text(
                    [x - 0.8, y + s * PT * 0.3],
                    &text,
                    s * 0.8,
                    true,
                    Align::Right,
                    Ink::Black,
                );
                sheet.text(
                    [x2 + 0.8, y2 + s * PT * 0.3],
                    &hold,
                    s * 0.8,
                    false,
                    Align::Left,
                    Ink::Black,
                );
            }
            View::Full => {
                // Behind the stand, where landmarks seldom are.
                let [x, y] = at(180.0, range as f32);
                let label = format!("{text}  {hold}");
                let w = text_width(&label, s * 0.75, false) + 1.0;
                sheet.rect(
                    x - w / 2.0,
                    y - s * 0.75 * PT * 0.5,
                    w,
                    s * 0.75 * PT,
                    Ink::White,
                );
                sheet.text(
                    [x, y + s * 0.75 * PT * 0.35],
                    label,
                    s * 0.75,
                    false,
                    Align::Center,
                    Ink::Black,
                );
            }
        }
        range += step;
    }
    if half < 180.0 {
        for edge in [-half, half] {
            sheet.line(center, at(edge, outer_m as f32), 0.25, Ink::Rule);
        }
        if stand.view == View::Half {
            sheet.line(
                [center[0] - radius, center[1]],
                [center[0] + radius, center[1]],
                0.25,
                Ink::Rule,
            );
        }
    }
    if let Some(limit) = ethical_m.filter(|limit| *limit < outer_m) {
        let r = limit as f32 * scale;
        sheet.polyline(
            arc(center, r, -half, half),
            0.6,
            Ink::Red,
            false,
            half >= 180.0,
        );
        let [x, y] = at(0.0, limit as f32);
        let text = format!("limit {}", u.format(Quantity::Distance, limit));
        let w = text_width(&text, s * 0.75, true) + 1.2;
        sheet.rect(
            x - w / 2.0,
            y - s * 0.75 * PT * 0.6,
            w,
            s * 0.75 * PT * 1.2,
            Ink::White,
        );
        sheet.text(
            [x, y + s * 0.75 * PT * 0.35],
            format!("limit {}", u.format(Quantity::Distance, limit)),
            s * 0.75,
            true,
            Align::Center,
            Ink::Red,
        );
    }
    if let Some([left, right]) = stand.sector {
        for (bearing, name) in [(left, "left edge"), (right, "right edge")] {
            let angle = on_card(stand, bearing) as f32;
            if angle.abs() <= half {
                let end = at(angle, outer_m as f32);
                sheet.line(center, end, 0.6, Ink::Black);
                // Beside the line, on the side outside the sector.
                let [x, y] = at(angle, outer_m as f32 * 0.62);
                let (align, nudge) = if bearing == left {
                    (Align::Right, -1.2)
                } else {
                    (Align::Left, 1.2)
                };
                sheet.text(
                    [x + nudge, y + s * PT],
                    name,
                    s * 0.7,
                    false,
                    align,
                    Ink::Black,
                );
            }
        }
    }
    // Compass points around the edge.
    for index in 0..8 {
        let bearing = index as f64 * 45.0;
        let angle = on_card(stand, bearing) as f32;
        if angle.abs() <= half + 1e-3 && !(half >= 180.0 && angle.abs() > 179.0 && index != 0) {
            let [x, mut y] = at(angle, outer_m as f32 + (s * PT * 1.6) / scale);
            // Beside the half map's baseline the ring labels sit: go above.
            if stand.view == View::Half && angle.abs() > 89.0 {
                y -= s * PT * 1.4;
            }
            sheet.text(
                [x, y + s * PT * 0.35],
                compass_point(bearing),
                s * 0.8,
                bearing == 0.0,
                Align::Center,
                if bearing == 0.0 {
                    Ink::Black
                } else {
                    Ink::Grey
                },
            );
        }
    }
    // The landmarks.
    for landmark in &landmarks {
        let angle = on_card(stand, landmark.bearing_degrees) as f32;
        if angle.abs() > half || landmark.range_m > outer_m {
            continue;
        }
        let point = at(angle, landmark.range_m as f32);
        let dot = arc(point, 0.9, 0.0, 360.0);
        sheet.fill(dot, Ink::Black);
        let left = point[0] > center[0] + radius * 0.5;
        let (x, align) = if left {
            (point[0] - 1.6, Align::Right)
        } else {
            (point[0] + 1.6, Align::Left)
        };
        sheet.text(
            [x, point[1] + s * PT * 0.3],
            &landmark.name,
            s * 0.85,
            true,
            align,
            Ink::Black,
        );
        sheet.text(
            [x, point[1] + s * PT * 1.25],
            u.format(Quantity::Distance, landmark.range_m),
            s * 0.75,
            false,
            align,
            Ink::Black,
        );
    }
    // The stand: a small triangle pointing the way it faces.
    let tip = 2.2;
    sheet.fill(
        vec![
            [center[0], center[1] - tip],
            [center[0] + tip * 0.8, center[1] + tip * 0.6],
            [center[0] - tip * 0.8, center[1] + tip * 0.6],
        ],
        Ink::Black,
    );

    // The landmarks' table, under the map.
    let mut y = map_bottom + 1.0;
    if !landmarks.is_empty() {
        y += 2.0;
        let row = s * PT * 1.5;
        let heads = ["Landmark", "Range", "Direction", "Hold", "Note"];
        let rows: Vec<[String; 5]> =
            landmarks
                .iter()
                .map(|landmark| {
                    let range = landmark.range_m;
                    let note =
                        if stand.no_shoot.iter().any(|no| {
                            between(landmark.bearing_degrees, no.from_degrees, no.to_degrees)
                        }) {
                            "never shoot this way"
                        } else if stand.sector.is_some_and(|[left, right]| {
                            !between(landmark.bearing_degrees, left, right)
                        }) {
                            "outside the sector"
                        } else if ethical_m.is_some_and(|limit| range > limit) {
                            "beyond the limit"
                        } else if dead_on.is_some_and(|dead_on| {
                            range <= dead_on.far_limit && range >= dead_on.near_limit
                        }) {
                            "hold dead on"
                        } else {
                            ""
                        };
                    [
                        landmark.name.clone(),
                        u.format(Quantity::Distance, range),
                        format!(
                            "{} {:.0}°",
                            compass_point(landmark.bearing_degrees),
                            landmark.bearing_degrees.rem_euclid(360.0)
                        ),
                        hold_text(range),
                        note.to_owned(),
                    ]
                })
                .collect();
        let natural: Vec<f32> = (0..5)
            .map(|column| {
                rows.iter()
                    .map(|cells| text_width(&cells[column], s, column == 0))
                    .fold(text_width(heads[column], s * 0.85, false), f32::max)
                    + s * PT * 1.2
            })
            .collect();
        let total: f32 = natural.iter().sum();
        let widths: Vec<f32> = natural
            .iter()
            .map(|width| width + (inner - total).max(0.0) / 5.0)
            .collect();
        let lefts: Vec<f32> = widths
            .iter()
            .scan(margin, |left, width| {
                let this = *left;
                *left += width;
                Some(this)
            })
            .collect();
        for (column, head) in heads.iter().enumerate() {
            sheet.text(
                [lefts[column], y + row * 0.7],
                *head,
                s * 0.85,
                false,
                Align::Left,
                Ink::Grey,
            );
        }
        y += row;
        sheet.line([margin, y], [margin + inner, y], 0.3, Ink::Black);
        for (index, cells) in rows.iter().enumerate() {
            if index % 2 == 1 {
                sheet.rect(margin, y, inner, row, Ink::Shade);
            }
            for (column, cell) in cells.iter().enumerate() {
                let ink = match (column, cell.as_str()) {
                    (4, "hold dead on") => Ink::Green,
                    (4, "") => Ink::Black,
                    (4, _) => Ink::Red,
                    _ => Ink::Black,
                };
                sheet.text(
                    [lefts[column], y + row * 0.7],
                    fit(cell, s, column == 0, widths[column] - 0.5),
                    s,
                    column == 0,
                    Align::Left,
                    ink,
                );
            }
            y += row;
        }
        sheet.line([margin, y], [margin + inner, y], 0.3, Ink::Black);
    }
    // Room to spare: lines for notes at the stand.
    let footer_top = height - margin - footer;
    let spacing = (s * PT * 2.6).max(6.0);
    if footer_top - y > 3.0 * spacing {
        y += spacing;
        sheet.text(
            [margin, y],
            "Notes",
            s * 0.85,
            false,
            Align::Left,
            Ink::Grey,
        );
        y += spacing;
        while y < footer_top - spacing * 0.4 {
            sheet.line([margin, y], [margin + inner, y], 0.15, Ink::Rule);
            y += spacing;
        }
    }
    let mut y = footer_top;
    legend.push(format!("{} · {}", crate::APP_NAME, super::today()));
    for text in &legend {
        line(&mut sheet, &mut y, text, s * 0.85, false, Ink::Grey);
    }
    sheet.frame(0.3, 0.3, width - 0.6, height - 0.6, 0.2, Ink::Rule);
    Ok(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand(facing: f64, view: View) -> Stand {
        Stand {
            facing_degrees: facing,
            view,
            ..Stand::default()
        }
    }

    #[test]
    fn directions_turn_with_the_stand() {
        let north = stand(350.0, View::Half);
        assert!((on_card(&north, 10.0) - 20.0).abs() < 1e-9);
        assert!((on_card(&north, 300.0) + 50.0).abs() < 1e-9);
        assert!(between(5.0, 350.0, 20.0));
        assert!(!between(30.0, 350.0, 20.0));
    }

    #[test]
    fn sectors_split_the_view() {
        let east = stand(90.0, View::Half);
        // Shooting allowed from 60° to 120°: the rest of the view is out.
        let outside = spans(&east, &[60.0, 120.0], |bearing| {
            !between(bearing, 60.0, 120.0)
        });
        assert_eq!(outside, vec![(-90.0, -30.0), (30.0, 90.0)]);
        // A road from 170° to 200° is half outside the view.
        let road = spans(&east, &[170.0, 200.0], |bearing| {
            between(bearing, 170.0, 200.0)
        });
        assert_eq!(road, vec![(80.0, 90.0)]);
    }

    #[test]
    fn rings_reach_past_the_landmarks_and_the_limit() {
        let mut far = stand(0.0, View::Half);
        far.landmarks.push(Landmark {
            name: "Rock".into(),
            range_m: 230.0,
            bearing_degrees: 10.0,
        });
        let (step, outer) = rings(&far, &StandOptions::default(), Some(180.0));
        assert_eq!((step, outer), (50.0, 250.0));
        let (step, outer) = rings(&far, &StandOptions::default(), Some(320.0));
        assert_eq!((step, outer), (100.0, 400.0));
    }
}

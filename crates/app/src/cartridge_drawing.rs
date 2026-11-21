//! A cartridge's measurements as a drawing: its outline from the side with
//! the C.I.P. drawing's letters and values, diameters above it and the
//! lengths from the head below it, as docs/cartridges.md describes them.
//! Each value is the largest the standard allows; where the drawing prints
//! a tolerance, the value shows as the range from the smallest to the
//! largest. It fits the height it is given, so it can stand beside the 3D
//! view without making the page taller.

use std::sync::Arc;

use ballistics_store::{Cartridge, Material};
use eframe::egui::{
    self, Color32, FontId, Galley, Id, Pos2, Sense, Shape, Stroke, epaint::Mesh, pos2, vec2,
};

use crate::i18n::tr;

const CASE: Color32 = Color32::from_rgb(0xea, 0xd9, 0xab);
const BULLET: Color32 = Color32::from_rgb(0xe3, 0xb4, 0x8e);
const PRIMER: Color32 = Color32::from_rgb(0xb8, 0xb8, 0xb4);
pub const HULL: Color32 = Color32::from_rgb(0xb3, 0x2d, 0x2a);
const OUTLINE: Color32 = Color32::from_rgb(0x1d, 0x2a, 0x22);
const TEXT: f32 = 11.0;
/// A row of labels: letter and value on one line.
const ROW: f32 = 14.0;
/// The cartridge's height at most, so a short one doesn't fill the page.
const MAX_BAND: f32 = 110.0;
const ARROW: f32 = 4.0;
/// Side by side from this width; below it the measurements fold away.
const SIDE_BY_SIDE: f32 = 700.0;

/// A measurement on the drawing: its letter, where along the axis it is
/// drawn (mm from the head) and its largest value.
struct Measure {
    letter: &'static str,
    at: f64,
    value: f64,
}

/// The value as the drawing gives it: the largest, or the range down to the
/// smallest where it prints a tolerance.
fn value_text(cartridge: &Cartridge, letter: &str, value: f64) -> String {
    match cartridge.tolerances.get(letter) {
        Some(&tolerance) if tolerance < 0.0 => format!("{:.2}–{:.2}", value + tolerance, value),
        _ => format!("{value:.2}"),
    }
}

/// A shotgun shell's diameters by C.I.P.'s table VII letters: the rim (g),
/// the body ahead of it (d) and the mouth (h).
fn shell_diameters(cartridge: &Cartridge) -> Vec<Measure> {
    let case = &cartridge.case;
    vec![
        Measure {
            letter: "g",
            at: case.rim_thickness / 2.0,
            value: case.rim_diameter,
        },
        Measure {
            letter: "d",
            at: case.rim_thickness + 0.6,
            value: case.body_diameter,
        },
        Measure {
            letter: "h",
            at: case.length - 3.0,
            value: case.mouth_diameter,
        },
    ]
}

fn diameters(cartridge: &Cartridge) -> Vec<Measure> {
    if cartridge.hull.is_some() {
        return shell_diameters(cartridge);
    }
    let case = &cartridge.case;
    let mut out = vec![Measure {
        letter: "R1",
        at: case.rim_thickness / 2.0,
        value: case.rim_diameter,
    }];
    if let Some(groove) = &case.groove {
        let [floor_end, bevel_end] = case.groove_ends(groove);
        out.push(Measure {
            letter: "E1",
            at: (case.rim_thickness + floor_end) / 2.0,
            value: groove.diameter,
        });
        if let Some(belt) = case.belt_diameter {
            out.push(Measure {
                letter: "R3",
                at: (bevel_end + case.body_start) / 2.0,
                value: belt,
            });
        }
    }
    // Just past E, so it isn't drawn on the rim's or the belt's front.
    out.push(Measure {
        letter: "P1",
        at: case.body_start + 0.4,
        value: case.body_diameter,
    });
    if let Some(shoulder) = &case.shoulder {
        out.push(Measure {
            letter: "P2",
            at: shoulder.start,
            value: shoulder.diameter,
        });
        out.push(Measure {
            letter: "H1",
            at: shoulder.neck_start,
            value: shoulder.neck_diameter,
        });
    }
    out.push(Measure {
        letter: "H2",
        at: case.length - 0.3,
        value: case.mouth_diameter,
    });
    // The bullet where it is widest out of the case: between the mouth and
    // the nose, or just past the mouth when the nose starts inside it.
    if let Some(bullet) = &cartridge.bullet {
        let nose = cartridge.overall_length - bullet.nose_length;
        out.push(Measure {
            letter: "G1",
            at: if nose > case.length + 1.0 {
                (case.length + nose) / 2.0
            } else {
                case.length + 0.3
            },
            value: bullet.diameter,
        });
    }
    out
}

fn lengths(cartridge: &Cartridge) -> Vec<Measure> {
    let case = &cartridge.case;
    if cartridge.hull.is_some() {
        // The rim (t), and the shell crimped as it is loaded; its open
        // length (l) is in the line under the drawing.
        return vec![
            Measure {
                letter: "t",
                at: case.rim_thickness,
                value: case.rim_thickness,
            },
            Measure {
                letter: "L",
                at: case.length,
                value: case.length,
            },
        ];
    }
    let mut out = vec![Measure {
        letter: "R",
        at: case.rim_thickness,
        value: case.rim_thickness,
    }];
    if case.body_start > case.rim_thickness + 1e-6 {
        out.push(Measure {
            letter: "E",
            at: case.body_start,
            value: case.body_start,
        });
    }
    if let Some(shoulder) = &case.shoulder {
        out.push(Measure {
            letter: "L1",
            at: shoulder.start,
            value: shoulder.start,
        });
        out.push(Measure {
            letter: "L2",
            at: shoulder.neck_start,
            value: shoulder.neck_start,
        });
    }
    out.push(Measure {
        letter: "L3",
        at: case.length,
        value: case.length,
    });
    if cartridge.bullet.is_some() {
        out.push(Measure {
            letter: "L6",
            at: cartridge.overall_length,
            value: cartridge.overall_length,
        });
    }
    out
}

/// The measurements that aren't drawn: the head's bevel, the groove's floor
/// and bevel, the shoulder's roundings.
fn others(cartridge: &Cartridge) -> String {
    let case = &cartridge.case;
    let mut parts = Vec::new();
    if let Some(hull) = &cartridge.hull {
        parts.push(format!(
            "l {}",
            value_text(cartridge, "l", hull.open_length)
        ));
        parts.push(crate::i18n::trf!(
            "brass head about {:.0} (typical)",
            hull.head_height
        ));
        return parts.join(" · ");
    }
    if let Some(bevel) = &case.head_bevel {
        parts.push(format!("f {:.2}", bevel.length));
        parts.push(format!("β {}°", bevel.degrees));
    }
    if let Some(groove) = &case.groove {
        parts.push(format!("e min {:.2}", groove.floor_min));
        parts.push(format!("δ {}°", groove.bevel_degrees));
    }
    if let Some(shoulder) = &case.shoulder {
        parts.push(format!("r1 {:.2}", shoulder.radius));
        parts.push(format!("r2 {:.2}", shoulder.neck_radius));
    }
    parts.join(" · ")
}

/// What each letter stands for, for the hover text.
fn meaning(letter: &str) -> &'static str {
    match letter {
        "R" => tr("rim thickness"),
        "R1" => tr("rim diameter"),
        "R3" => tr("belt diameter"),
        "E" => tr("from the head to where the body starts"),
        "E1" => tr("extractor groove diameter"),
        "P1" => tr("body diameter at E"),
        "P2" => tr("body diameter where the shoulder starts (L1)"),
        "H1" => tr("neck diameter where the neck starts (L2)"),
        "H2" => tr("diameter at the case mouth"),
        "G1" => tr("bullet diameter"),
        "L1" => tr("from the head to where the shoulder starts"),
        "L2" => tr("from the head to where the neck starts"),
        "L3" => tr("case length"),
        "L6" => tr("overall length, head to bullet tip"),
        "g" => tr("rim diameter"),
        "t" => tr("rim thickness"),
        "d" => tr("body diameter ahead of the rim"),
        "h" => tr("diameter at the mouth"),
        "l" => tr("shell length opened (fired), as its caliber's name says"),
        "L" => tr("shell length crimped, as loaded (SAAMI's maximum, or estimated)"),
        _ => "",
    }
}

/// The cartridge in 3D with `under` below it (its buttons), and its
/// measurements beside them, as tall as the two together (`height` at
/// least), when `width` leaves room for both. Else the 3D view `narrow`
/// points tall, `under`, and the measurements folded under a heading.
pub fn with_3d(
    ui: &mut egui::Ui,
    cartridge: &Cartridge,
    width: f32,
    height: f32,
    narrow: f32,
    id: &str,
    under: impl FnOnce(&mut egui::Ui),
) {
    if width < SIDE_BY_SIDE {
        crate::cartridge3d::show(ui, cartridge, vec2(width, narrow));
        under(ui);
        egui::CollapsingHeader::new(tr("Measurements"))
            .id_salt(("cartridge-measurements", id))
            .show(ui, |ui| {
                show(ui, cartridge, ui.available_width().min(560.0), 230.0)
            });
        return;
    }
    // What `under` took last time, so the 3D view leaves room for it.
    let key = Id::unique(("cartridge-under", id));
    let below = ui.data(|data| data.get_temp::<f32>(key)).unwrap_or(0.0);
    let gap = 16.0;
    let left = (width - gap) * 0.4;
    ui.horizontal_top(|ui| {
        let column = ui.vertical(|ui| {
            ui.set_width(left);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            crate::cartridge3d::show(ui, cartridge, vec2(left, (height - below).max(100.0)));
            let top = ui.cursor().top();
            under(ui);
            let took = ui.cursor().top() - top;
            if (took - below).abs() > 0.5 {
                // Lay it out again with the 3D view leaving the right room.
                ui.data_mut(|data| data.insert_temp(key, took));
                ui.ctx()
                    .request_discard("the cartridge's buttons changed height");
            }
        });
        ui.add_space(gap);
        let tall = column.response.rect.height().max(height);
        let rest = width - gap - left - 2.0 * ui.spacing().item_spacing.x;
        show(ui, cartridge, rest, tall);
    });
}

struct Label {
    letter: Arc<Galley>,
    value: Arc<Galley>,
    /// Where it is measured, and where its label's middle went.
    at: f32,
    x: f32,
    row: usize,
}

impl Label {
    fn width(&self) -> f32 {
        self.letter.size().x + 3.0 + self.value.size().x
    }
}

/// Puts each label in the lowest row it fits without touching the one
/// before it, moved right by up to its own width if that keeps it low.
/// None starts left of `floor`. Returns the number of rows.
fn pack(labels: &mut [Label], width: f32, floor: f32) -> usize {
    let mut ends: Vec<f32> = Vec::new();
    for label in labels.iter_mut() {
        let half = label.width() / 2.0;
        let wanted = label
            .at
            .max(floor + half)
            .clamp(half, (width - half).max(half));
        let fits = |end: f32| {
            let x = wanted.max(end + 8.0 + half);
            (x - wanted <= 2.0 * half && x <= width - half).then_some(x)
        };
        let (row, x) = ends
            .iter()
            .enumerate()
            .find_map(|(row, &end)| fits(end).map(|x| (row, x)))
            .unwrap_or((ends.len(), wanted));
        if row == ends.len() {
            ends.push(0.0);
        }
        ends[row] = x + half;
        label.x = x;
        label.row = row;
    }
    ends.len()
}

/// Draws the cartridge's measurements in `width` × `height` points.
pub fn show(ui: &mut egui::Ui, cartridge: &Cartridge, width: f32, height: f32) {
    let painter = ui.painter().clone();
    let dim = if ui.visuals().dark_mode {
        Color32::from_rgb(0x8c, 0xc0, 0xe0)
    } else {
        Color32::from_rgb(0x2f, 0x6b, 0x8f)
    };
    let ink = ui.visuals().text_color();
    let font = FontId::proportional(TEXT);
    let labels = |measures: &[Measure]| -> Vec<Label> {
        measures
            .iter()
            .map(|measure| Label {
                letter: painter.layout_no_wrap(measure.letter.to_owned(), font.clone(), dim),
                value: painter.layout_no_wrap(
                    value_text(cartridge, measure.letter, measure.value),
                    font.clone(),
                    ink,
                ),
                at: 0.0,
                x: 0.0,
                row: 0,
            })
            .collect()
    };

    let outline = cartridge.outline();
    let mut points = vec![(Material::Case, outline.start)];
    for (material, stretch) in &outline.stretches {
        points.extend(stretch.iter().map(|&point| (*material, point)));
    }
    let widest = points.iter().map(|(_, [_, r])| *r).fold(0.0, f64::max) as f32;
    let long = cartridge.overall_length as f32;
    let margin = 4.0;
    let others = painter.layout(others(cartridge), font.clone(), ink, width - 2.0 * margin);

    // The scale sets where the labels go and the labels' rows set the room
    // left for the cartridge: settle it in a few rounds.
    let diameters = diameters(cartridge);
    let lengths = lengths(cartridge);
    let mut above = labels(&diameters);
    let mut below = labels(&lengths);
    let (mut scale, mut left, mut rows_above, mut rows_below) = (1.0, margin, 1, 1);
    for _ in 0..3 {
        let band = height - (rows_above + rows_below) as f32 * ROW - 14.0 - others.size().y;
        scale = ((width - 2.0 * margin - 20.0) / long)
            .min(band.min(MAX_BAND) / (2.0 * widest))
            .max(0.3);
        left = margin + (width - 2.0 * margin - long * scale).max(0.0) / 2.0;
        for (label, measure) in above.iter_mut().zip(&diameters) {
            label.at = left + measure.at as f32 * scale;
        }
        for (label, measure) in below.iter_mut().zip(&lengths) {
            label.at = left + measure.at as f32 * scale;
        }
        rows_above = pack(&mut above, width, 0.0);
        // Clear of the line down from the head.
        rows_below = pack(&mut below, width, left + 3.0);
    }
    // Taller than asked if the labels' rows leave too little room.
    let needed =
        (rows_above + rows_below) as f32 * ROW + 14.0 + others.size().y + 2.0 * widest * scale;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height.max(needed)), Sense::hover());
    if ui.is_rect_visible(rect) {
        let origin = rect.left_top();
        let top = origin.y + rows_above as f32 * ROW + 5.0;
        let axis = top + widest * scale;
        let bottom = axis + widest * scale;
        let at = |along: f64, radius: f64| {
            pos2(
                origin.x + left + along as f32 * scale,
                axis - radius as f32 * scale,
            )
        };
        let under = |along: f64, radius: f64| {
            pos2(
                origin.x + left + along as f32 * scale,
                axis + radius as f32 * scale,
            )
        };

        // The outline, filled stretch by stretch as brass, copper and primer.
        let mut mesh = Mesh::default();
        for pair in points.windows(2) {
            let ((_, [a0, r0]), (material, [a1, r1])) = (pair[0], pair[1]);
            let colour = match material {
                Material::Case => CASE,
                Material::Bullet => BULLET,
                Material::Primer => PRIMER,
                Material::Hull => HULL,
            };
            let first = mesh.vertices.len() as u32;
            mesh.colored_vertex(at(a0, r0), colour);
            mesh.colored_vertex(at(a1, r1), colour);
            mesh.colored_vertex(under(a1, r1), colour);
            mesh.colored_vertex(under(a0, r0), colour);
            mesh.add_triangle(first, first + 1, first + 2);
            mesh.add_triangle(first, first + 2, first + 3);
        }
        painter.add(Shape::mesh(mesh));
        let line = Stroke::new(1.0, OUTLINE);
        painter.add(Shape::line(
            points.iter().map(|(_, [a, r])| at(*a, *r)).collect(),
            line,
        ));
        painter.add(Shape::line(
            points.iter().map(|(_, [a, r])| under(*a, *r)).collect(),
            line,
        ));
        let end = origin.x + left + long * scale;
        painter.extend(Shape::dashed_line(
            &[pos2(origin.x + left - 5.0, axis), pos2(end + 5.0, axis)],
            Stroke::new(0.8, dim),
            9.0,
            3.0,
        ));

        let thin = Stroke::new(0.8, dim);
        let leader = Stroke::new(0.6, dim.gamma_multiply(0.6));
        let arrow = |tip: Pos2, towards: egui::Vec2| {
            let back = tip - towards * ARROW;
            let side = vec2(-towards.y, towards.x) * (ARROW * 0.45);
            painter.add(Shape::convex_polygon(
                vec![tip, back + side, back - side],
                dim,
                Stroke::NONE,
            ));
        };
        // Labels go on last, on the page's colour, so the lines that pass
        // them don't run through their text: (label, its text's top).
        let mut texts: Vec<(&Label, f32)> = Vec::new();

        // Diameters: a line across, and up to the label.
        for (measure, label) in diameters.iter().zip(&above) {
            let up = at(measure.at, measure.value / 2.0);
            let down = under(measure.at, measure.value / 2.0);
            painter.line_segment([up, down], thin);
            arrow(up, vec2(0.0, -1.0));
            arrow(down, vec2(0.0, 1.0));
            let row_bottom = top - 3.0 - label.row as f32 * ROW;
            let x = origin.x + label.x;
            painter.line_segment([up, pos2(up.x, row_bottom)], leader);
            painter.line_segment([pos2(up.x, row_bottom), pos2(x, row_bottom)], leader);
            texts.push((label, row_bottom - ROW + 1.0));
        }

        // Lengths from the head: a line down from each place to its label,
        // and one from the head itself they are measured from.
        let rows_end = bottom + 4.0 + rows_below as f32 * ROW;
        painter.line_segment(
            [
                under(0.0, edge(&points, 0.0)),
                pos2(origin.x + left, rows_end),
            ],
            thin,
        );
        for (measure, label) in lengths.iter().zip(&below) {
            let from = under(measure.at, edge(&points, measure.at));
            let row_top = bottom + 3.0 + label.row as f32 * ROW;
            let x = origin.x + label.x;
            painter.line_segment([from, pos2(from.x, row_top)], leader);
            painter.line_segment([pos2(from.x, row_top), pos2(x, row_top)], leader);
            texts.push((label, row_top + 1.0));
        }

        let paper = ui.visuals().panel_fill;
        for (label, y) in texts {
            let x = origin.x + label.x - label.width() / 2.0;
            let size = vec2(
                label.width(),
                label.letter.size().y.max(label.value.size().y),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(pos2(x, y), size).expand(1.0),
                2.0,
                paper,
            );
            painter.galley(pos2(x, y), label.letter.clone(), dim);
            painter.galley(
                pos2(x + label.letter.size().x + 3.0, y),
                label.value.clone(),
                ink,
            );
        }
        painter.galley(pos2(origin.x + margin, rows_end + 4.0), others, ink);
    }
    response.on_hover_ui(|ui| {
        ui.label(tr("Millimetres, the largest the standard allows; a range where its drawing gives a tolerance"));
        ui.label(tr("Lengths are from the case head."));
        for measure in diameters.iter().chain(&lengths) {
            ui.label(format!("{}: {}", measure.letter, meaning(measure.letter)));
        }
    });
}

/// The outline's radius at `along` (the largest there, at a step), mm.
fn edge(points: &[(Material, [f64; 2])], along: f64) -> f64 {
    let mut radius: f64 = 0.0;
    for pair in points.windows(2) {
        let ([a0, r0], [a1, r1]) = (pair[0].1, pair[1].1);
        if (a0 - along).abs() < 1e-6 {
            radius = radius.max(r0);
        }
        if (a1 - along).abs() < 1e-6 {
            radius = radius.max(r1);
        }
        if a0 < along && along < a1 {
            radius = radius.max(r0 + (r1 - r0) * (along - a0) / (a1 - a0));
        }
    }
    radius
}

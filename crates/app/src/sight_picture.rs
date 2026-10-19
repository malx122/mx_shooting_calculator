//! The view through the scope: the reticle at the chosen magnification, the
//! point where the bullet lands with the crosshair on the target, and the
//! target with its vital zone at true size. Everything is drawn as vectors.

use ballistics_store::{FocalPlane, Reticle, ReticleElement, Scope};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke, pos2, vec2};

use crate::figure::Figure;
use crate::theme;

const SCENE: Color32 = Color32::from_rgb(0xC9, 0xD2, 0xBE);
const FIELD: Color32 = Color32::from_rgb(0xA9, 0xB8, 0x98);
const INK: Color32 = Color32::from_rgb(0x10, 0x14, 0x10);
const ILLUMINATION: Color32 = Color32::from_rgb(0xD6, 0x2A, 0x22);
/// Field of view times magnification of a typical hunting scope, in mrad
/// (about 38 m at 100 m at 1×), for scopes whose field isn't published.
const TYPICAL_FIELD_TIMES_MAGNIFICATION: f64 = 380.0;

pub struct SightPicture<'a> {
    pub scope: &'a Scope,
    pub reticle: &'a Reticle,
    pub magnification: f64,
    /// Where the bullet lands relative to the aiming point, in mrad: x to
    /// the right, y up.
    pub impact_mrad: [f64; 2],
    /// The target, drawn with its vital zone where the bullet lands.
    pub figure: &'a Figure,
    pub range_m: f64,
    /// Draw the illuminated parts of the reticle lit (red).
    pub illumination: bool,
}

/// What the drawing showed, for the caption.
pub struct Drawn {
    pub field_of_view_mrad: f64,
    pub field_of_view_known: bool,
    pub impact_in_view: bool,
}

impl SightPicture<'_> {
    pub fn show(&self, ui: &mut egui::Ui, size: f32) -> Drawn {
        let published = self.scope.field_of_view_mrad(self.magnification);
        let field = published.unwrap_or_else(|| estimated_field_of_view_mrad(self.magnification));
        let eyepiece = Eyepiece::open(ui, size, field);

        // The target with its vital zone centred where the bullet lands:
        // the sight picture for a hit in the middle of it.
        let impact = eyepiece.to_screen(self.impact_mrad);
        let mrad_per_metre = 1000.0 / self.range_m.max(1.0);
        let at = |[x, y]: [f64; 2]| {
            eyepiece.to_screen([
                self.impact_mrad[0] + x * mrad_per_metre,
                self.impact_mrad[1] + y * mrad_per_metre,
            ])
        };
        let feet = at([0.0, -self.figure.aim_height]).y;
        if feet < eyepiece.rect.bottom() {
            eyepiece.painter.rect_filled(
                egui::Rect::from_min_max(
                    pos2(eyepiece.rect.left(), feet.max(eyepiece.rect.top())),
                    eyepiece.rect.max,
                ),
                0.0,
                FIELD,
            );
        }
        for part in &self.figure.parts {
            let points: Vec<Pos2> = part.points.iter().map(|&point| at(point)).collect();
            let outline = part
                .stroke
                .map_or(Stroke::NONE, |colour| Stroke::new(1.0_f32, colour));
            eyepiece
                .painter
                .add(Shape::convex_polygon(points, part.fill, outline));
        }
        // Second-focal-plane marks change angular size with magnification.
        eyepiece.draw_reticle(
            self.reticle,
            self.scope.reticle_scale(self.magnification),
            self.illumination,
        );
        eyepiece.close(ui);

        let offset = impact - eyepiece.center;
        let impact_in_view = offset.length() < eyepiece.radius - 6.0;
        if impact_in_view {
            eyepiece
                .painter
                .circle(impact, 3.5, theme::BLAZE, Stroke::new(1.0_f32, INK));
        } else {
            draw_edge_arrow(
                &eyepiece.painter,
                eyepiece.center,
                eyepiece.radius,
                offset.normalized(),
            );
        }
        Drawn {
            field_of_view_mrad: field,
            field_of_view_known: published.is_some(),
            impact_in_view,
        }
    }
}

/// A typical scope's field of view at this magnification: the view still
/// narrows as the hunter zooms when the maker publishes no field.
pub fn estimated_field_of_view_mrad(magnification: f64) -> f64 {
    TYPICAL_FIELD_TIMES_MAGNIFICATION / magnification.max(1.0)
}

/// The scope's magnification range as a slider. On a second-focal-plane
/// scope the magnification its reticle is true at is marked, and a
/// double-click jumps there. The response reports a change.
pub fn magnification_slider(
    ui: &mut egui::Ui,
    value: &mut f64,
    scope: &Scope,
    width: f32,
) -> egui::Response {
    let (low, high) = (scope.magnification_min, scope.magnification_max);
    let true_at = match (scope.focal_plane, scope.reticle_reference_magnification) {
        (FocalPlane::Sfp, Some(reference)) => Some(reference.clamp(low, high)),
        _ => None,
    };
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(width, 40.0), egui::Sense::click_and_drag());
    let rail_y = rect.top() + 12.0;
    let (left, right) = (rect.left() + 10.0, rect.right() - 10.0);
    let span = high - low;
    let x_of = |magnification: f64| {
        if span > 0.0 {
            left + ((magnification - low) / span) as f32 * (right - left)
        } else {
            (left + right) / 2.0
        }
    };

    if span > 0.0 {
        if response.double_clicked() {
            if let Some(reference) = true_at {
                *value = reference;
                response.mark_changed();
            }
        } else if let Some(pointer) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            let fraction = ((pointer.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
            let magnification = ((low + fraction * span) * 10.0).round() / 10.0;
            if magnification != *value {
                *value = magnification.clamp(low, high);
                response.mark_changed();
            }
        }
    }

    let painter = ui.painter_at(rect.expand(4.0));
    let handle = x_of(*value);
    painter.line_segment(
        [pos2(left, rail_y), pos2(right, rail_y)],
        Stroke::new(4.0_f32, theme::FOREST_600),
    );
    painter.line_segment(
        [pos2(left, rail_y), pos2(handle, rail_y)],
        Stroke::new(4.0_f32, theme::BLAZE_DEEP),
    );
    if let Some(reference) = true_at {
        let x = x_of(reference);
        painter.line_segment(
            [pos2(x, rail_y - 8.0), pos2(x, rail_y + 8.0)],
            Stroke::new(2.0_f32, theme::AMBER),
        );
    }
    let hovered = response.hovered() || response.dragged();
    painter.circle(
        pos2(handle, rail_y),
        if hovered { 8.0 } else { 7.0 },
        theme::BONE,
        Stroke::new(2.0_f32, theme::BLAZE),
    );

    // Labels under the rail: the ends, and where the marks are true. A mark
    // at an end shares that end's label.
    let font = egui::FontId::proportional(11.5);
    let label_y = rail_y + 12.0;
    let muted = theme::BONE_MUTED;
    let at_low = true_at.is_some_and(|reference| x_of(reference) - left < 36.0);
    let at_high = true_at.is_some_and(|reference| right - x_of(reference) < 36.0);
    let end_label = |magnification: f64, marks_true: bool| {
        format!(
            "{}{}",
            magnification_text(magnification),
            if marks_true { " · true" } else { "" }
        )
    };
    let low_colour = if at_low { theme::AMBER } else { muted };
    let high_colour = if at_high { theme::AMBER } else { muted };
    if span > 0.0 {
        painter.text(
            pos2(left - 6.0, label_y),
            egui::Align2::LEFT_TOP,
            end_label(low, at_low),
            font.clone(),
            low_colour,
        );
    }
    painter.text(
        pos2(right + 6.0, label_y),
        egui::Align2::RIGHT_TOP,
        end_label(high, at_high),
        font.clone(),
        high_colour,
    );
    if let Some(reference) = true_at
        && !at_low
        && !at_high
    {
        let text = format!("true {}", magnification_text(reference));
        painter.text(
            pos2(x_of(reference), label_y),
            egui::Align2::CENTER_TOP,
            text,
            font,
            theme::AMBER,
        );
    }

    match true_at {
        Some(reference) => response.on_hover_text(format!(
            "Second focal plane: the reticle's marks are true at {}. Double-click to go there.",
            magnification_text(reference)
        )),
        None if span > 0.0 => {
            response.on_hover_text("First focal plane: the marks are true at every magnification.")
        }
        None => response,
    }
}

/// "12×" or "2.4×".
pub fn magnification_text(magnification: f64) -> String {
    if (magnification - magnification.round()).abs() < 0.05 {
        format!("{:.0}×", magnification.round())
    } else {
        format!("{magnification:.1}×")
    }
}

/// A reticle on its own at its published size, framed to show its marks:
/// for choosing one. `illumination` lights the parts that can glow.
pub fn reticle_preview(ui: &mut egui::Ui, reticle: &Reticle, size: f32, illumination: bool) {
    let field = (reticle.inner_extent_mrad() * 2.6).max(6.0);
    let eyepiece = Eyepiece::open(ui, size, field);
    eyepiece.draw_reticle(reticle, 1.0, illumination);
    eyepiece.close(ui);
}

/// A round view `field` mrad across, drawn into a square of `size` points.
struct Eyepiece {
    painter: egui::Painter,
    center: Pos2,
    radius: f32,
    pixels_per_mrad: f32,
    rect: egui::Rect,
}

impl Eyepiece {
    fn open(ui: &mut egui::Ui, size: f32, field_mrad: f64) -> Eyepiece {
        let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = size / 2.0 - 6.0;
        painter.circle_filled(center, radius, SCENE);
        Eyepiece {
            painter,
            center,
            radius,
            pixels_per_mrad: (radius as f64 / (field_mrad / 2.0)) as f32,
            rect,
        }
    }

    fn to_screen(&self, [x, y]: [f64; 2]) -> Pos2 {
        pos2(
            self.center.x + x as f32 * self.pixels_per_mrad,
            self.center.y - y as f32 * self.pixels_per_mrad,
        )
    }

    /// Draws the reticle with its marks `scale` times their published size.
    fn draw_reticle(&self, reticle: &Reticle, scale: f64, illumination: bool) {
        let point = |[x, y]: [f64; 2]| self.to_screen([x * scale, y * scale]);
        let thickness = |width: Option<f64>| {
            width
                .map_or(1.0, |width| (width * scale) as f32 * self.pixels_per_mrad)
                .max(1.0)
        };
        for element in reticle.in_mrad() {
            draw_element(
                &self.painter,
                &element,
                point,
                thickness,
                scale as f32 * self.pixels_per_mrad,
                illumination,
            );
        }
    }

    /// Hides everything outside the eyepiece and draws its rim.
    fn close(&self, ui: &egui::Ui) {
        // A solid ring from the rim out past the corners, as a mesh: a
        // stroke this thick leaves gaps in the corners.
        let outer = (self.rect.width().hypot(self.rect.height()) / 2.0) + 2.0;
        let colour = ui.visuals().panel_fill;
        let mut mask = egui::Mesh::default();
        const SEGMENTS: u32 = 128;
        for index in 0..SEGMENTS {
            let angle = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let direction = vec2(angle.cos(), angle.sin());
            mask.colored_vertex(self.center + direction * (self.radius - 0.5), colour);
            mask.colored_vertex(self.center + direction * outer, colour);
        }
        for index in 0..SEGMENTS {
            let (inner, out) = (2 * index, 2 * index + 1);
            let (next_inner, next_out) = ((inner + 2) % (2 * SEGMENTS), (out + 2) % (2 * SEGMENTS));
            mask.add_triangle(inner, out, next_out);
            mask.add_triangle(inner, next_out, next_inner);
        }
        self.painter.add(Shape::mesh(mask));
        self.painter.circle_stroke(
            self.center,
            self.radius,
            Stroke::new(5.0_f32, theme::FOREST_950),
        );
    }
}

fn draw_element(
    painter: &egui::Painter,
    element: &ReticleElement,
    point: impl Fn([f64; 2]) -> Pos2,
    thickness: impl Fn(Option<f64>) -> f32,
    pixels_per_unit: f32,
    illumination: bool,
) {
    let color = |illuminated: bool| {
        if illuminated && illumination {
            ILLUMINATION
        } else {
            INK
        }
    };
    match element {
        ReticleElement::Line {
            from,
            to,
            width,
            illuminated,
        } => {
            painter.line_segment(
                [point(*from), point(*to)],
                Stroke::new(thickness(*width), color(*illuminated)),
            );
        }
        ReticleElement::Polygon {
            points,
            illuminated,
        } => {
            let points = points.iter().map(|p| point(*p)).collect();
            painter.add(Shape::convex_polygon(
                points,
                color(*illuminated),
                Stroke::NONE,
            ));
        }
        ReticleElement::Circle {
            center,
            radius,
            fill,
            width,
            illuminated,
        } => {
            let screen_radius = (*radius as f32 * pixels_per_unit).max(1.0);
            if *fill {
                painter.circle_filled(point(*center), screen_radius.max(1.5), color(*illuminated));
            } else {
                painter.circle_stroke(
                    point(*center),
                    screen_radius,
                    Stroke::new(thickness(*width), color(*illuminated)),
                );
            }
        }
    }
}

/// An orange arrow on the rim pointing towards an impact outside the view.
fn draw_edge_arrow(painter: &egui::Painter, center: Pos2, radius: f32, direction: egui::Vec2) {
    let tip = center + direction * (radius - 8.0);
    let base = center + direction * (radius - 26.0);
    let side = vec2(-direction.y, direction.x) * 9.0;
    painter.add(Shape::convex_polygon(
        vec![tip, base + side, base - side],
        theme::BLAZE,
        Stroke::new(1.5_f32, INK),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::Catalog;

    #[test]
    fn estimated_field_narrows_with_magnification_like_real_scopes() {
        assert!(estimated_field_of_view_mrad(22.0) < estimated_field_of_view_mrad(4.5));
        // Within a quarter of what the makers publish, at both ends.
        for scope in &Catalog::builtin().scopes {
            for magnification in [scope.magnification_min, scope.magnification_max] {
                let Some(published) = scope.field_of_view_mrad(magnification) else {
                    continue;
                };
                let estimate = estimated_field_of_view_mrad(magnification);
                assert!(
                    (estimate / published - 1.0).abs() < 0.25,
                    "{} at {magnification}×: {estimate:.0} vs {published:.0} mrad",
                    scope.model
                );
            }
        }
    }
}

//! Phone-sized screens: when the window is narrow the app switches to a
//! one-column layout with a bottom tab bar, bigger touch targets and
//! sheets that slide up from the bottom. Wide windows keep the desktop
//! layout.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Id, Order, Pos2, RichText, Sense, Stroke, Ui, pos2, vec2,
};

use crate::theme;

/// Below this width (in points) the phone layout is used.
const NARROW_WIDTH: f32 = 640.0;

/// The smallest comfortable touch target, in points.
pub const TOUCH: f32 = 44.0;

pub fn is_narrow(context: &egui::Context) -> bool {
    context.screen_rect().width() < NARROW_WIDTH
}

/// Bigger text and controls on phones; the desktop sizes otherwise.
/// Only touches the style when the layout changes.
pub fn apply_style(context: &egui::Context) {
    let narrow = is_narrow(context);
    let id = Id::new("mobile-style");
    if context.data(|data| data.get_temp::<bool>(id)) == Some(narrow) {
        return;
    }
    context.data_mut(|data| data.insert_temp(id, narrow));
    context.style_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};
        let scale = if narrow { 1.3 } else { 1.0 };
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(9.0 * scale, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(12.5 * scale, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(12.5 * scale, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(18.0 * scale, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.0 * scale, FontFamily::Monospace),
            ),
        ]
        .into();
        if narrow {
            style.spacing.item_spacing = vec2(10.0, 8.0);
            style.spacing.button_padding = vec2(12.0, 8.0);
            style.spacing.interact_size = vec2(48.0, 36.0);
            style.spacing.combo_height = 400.0;
        } else {
            style.spacing.item_spacing = vec2(8.0, 6.0);
            style.spacing.button_padding = vec2(8.0, 4.0);
            style.spacing.interact_size = vec2(40.0, 18.0);
        }
    });
}

/// "‹ Label": back to the previous view. Returns true when tapped.
pub fn back_button(ui: &mut Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(format!("‹ {label}")).color(theme::BLAZE))
            .frame(false)
            .min_size(vec2(TOUCH, TOUCH)),
    )
    .clicked()
}

/// A full-width row button for lists: a title, an optional second line and
/// a chevron. Returns the response for clicks.
pub fn list_row(ui: &mut Ui, title: &str, detail: &str, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let height = if detail.is_empty() { 48.0 } else { 60.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter();
    let fill = if selected || response.is_pointer_button_down_on() {
        theme::FOREST_600
    } else {
        theme::FOREST_700
    };
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    let text_x = rect.left() + 14.0;
    // Text stops before the chevron, cut with "…" when too long.
    let text_width = rect.width() - 14.0 - 34.0;
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::FontId::proportional(body.size * 0.85);
    let line = |text: &str, font: egui::FontId, colour: Color32| {
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, colour);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    if detail.is_empty() {
        let galley = line(title, body, theme::BONE);
        let at = pos2(text_x, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, theme::BONE);
    } else {
        let galley = line(title, body, theme::BONE);
        painter.galley(pos2(text_x, rect.top() + 9.0), galley, theme::BONE);
        let galley = line(detail, small, theme::BONE_MUTED);
        let at = pos2(text_x, rect.bottom() - 9.0 - galley.size().y);
        painter.galley(at, galley, theme::BONE_MUTED);
    }
    painter.text(
        pos2(rect.right() - 14.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(22.0),
        theme::BONE_MUTED,
    );
    response
}

/// What a sheet reports back.
pub struct Sheet<R> {
    /// The hunter tapped Done or outside the sheet.
    pub closed: bool,
    pub inner: R,
}

/// A sheet sliding up from the bottom over a dimmed screen, with a title
/// and a Done button. Tapping outside it also closes it.
pub fn sheet<R>(
    context: &egui::Context,
    id: &str,
    title: &str,
    contents: impl FnOnce(&mut Ui) -> R,
) -> Sheet<R> {
    let screen = context.screen_rect();
    let mut closed = false;
    egui::Area::new(Id::new(id).with("dim"))
        .order(Order::Middle)
        .fixed_pos(Pos2::ZERO)
        .show(context, |ui| {
            let response = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(150));
            closed |= response.clicked();
        });
    let width = screen.width();
    // Sit on the bottom edge, using the height measured last frame (an
    // anchored area would get too little room on its first frame).
    let height_id = Id::new(id).with("height");
    let height: f32 = context
        .data(|data| data.get_temp(height_id))
        .unwrap_or(screen.height() * 0.6);
    let top = (screen.bottom() - height).max(screen.top() + screen.height() * 0.08);
    let shown = egui::Area::new(Id::new(id))
        .order(Order::Foreground)
        .fixed_pos(pos2(screen.left(), top))
        .constrain(false)
        .show(context, |ui| {
            // An area only gets the room between panels; the sheet covers
            // the tab bar too.
            ui.set_max_height(screen.height());
            egui::Frame::new()
                .fill(theme::FOREST_800)
                .stroke(Stroke::new(1.0_f32, theme::FOREST_600))
                .corner_radius(CornerRadius {
                    nw: 18,
                    ne: 18,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 8,
                    bottom: 24,
                })
                .show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    // The grab handle.
                    let (handle, _) =
                        ui.allocate_exact_size(vec2(width - 32.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(
                        egui::Rect::from_center_size(handle.center(), vec2(40.0, 5.0)),
                        CornerRadius::same(3),
                        theme::FOREST_600,
                    );
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(title).heading());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let done = ui.add(
                                egui::Button::new(
                                    RichText::new("Done").color(theme::FOREST_900).strong(),
                                )
                                .fill(theme::BLAZE)
                                .min_size(vec2(72.0, TOUCH)),
                            );
                            closed |= done.clicked();
                        });
                    });
                    ui.add_space(6.0);
                    let scroll = egui::ScrollArea::vertical()
                        .max_height(screen.height() * 0.8)
                        .show(ui, contents);
                    // How much taller the sheet wants to be than shown.
                    let hidden = scroll.content_size.y - scroll.inner_rect.height();
                    (scroll.inner, hidden.max(0.0))
                })
                .inner
        });
    let (inner, hidden) = shown.inner;
    let measured = (shown.response.rect.height() + hidden).min(screen.height() * 0.92);
    if (measured - height).abs() > 0.5 {
        context.data_mut(|data| data.insert_temp(height_id, measured));
        context.request_repaint();
    }
    Sheet { closed, inner }
}

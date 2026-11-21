//! Everything round the canvas: the step bar, the instruction, the live
//! strip and the toolbars, laid out for a phone or a desktop.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    TextStyle, Ui, UiBuilder, pos2, vec2,
};

use super::paint::{Icon, paint_icon};
use super::scale::{self, ScaleState};
use super::{Command, Handle, LiveView, Marking, Step, ZoomTo};
use crate::i18n::tr;
use crate::{mobile, theme};

/// The smallest a tall button's label is drawn before it is cut short.
const MIN_LABEL_SIZE: f32 = 8.0;

/// A label that fits `width`: smaller text, down to [`MIN_LABEL_SIZE`],
/// then cut short with "…".
fn fitted_label(
    ui: &Ui,
    label: &str,
    mut font: egui::FontId,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), color);
    if galley.size().x <= width {
        return galley;
    }
    font.size = (font.size * width / galley.size().x).max(MIN_LABEL_SIZE);
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}

/// A toolbar button: an icon and a label, beside it or (`tall`, for
/// phones, exactly this wide) under it. Returns true when clicked.
pub fn tool_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    selected: bool,
    tall: Option<f32>,
) -> bool {
    let palette = theme::palette(ui.ctx());
    let style = if tall.is_some() {
        TextStyle::Small
    } else {
        TextStyle::Button
    };
    let font = style.resolve(ui.style());
    let text_color = if !enabled {
        palette.text_muted.gamma_multiply(0.6)
    } else if selected {
        palette.on_selection
    } else {
        palette.text
    };
    let galley = match tall {
        Some(width) => fitted_label(ui, label, font, text_color, width - 6.0),
        None => ui
            .painter()
            .layout_no_wrap(label.to_owned(), font, text_color),
    };
    let size = match tall {
        Some(width) => vec2(width, 52.0),
        None => vec2(
            galley.size().x + 34.0,
            ui.spacing().interact_size.y.max(28.0),
        ),
    };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let fill = if selected {
        palette.selection
    } else if enabled && response.is_pointer_button_down_on() {
        palette.active
    } else if enabled && response.hovered() {
        palette.hover
    } else if tall.is_some() {
        Color32::TRANSPARENT
    } else {
        palette.raised
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    if tall.is_none() && !selected {
        painter.rect_stroke(
            rect,
            CornerRadius::same(8),
            palette.edge,
            StrokeKind::Inside,
        );
    }
    match tall {
        Some(_) => {
            paint_icon(
                painter,
                pos2(rect.center().x, rect.top() + 17.0),
                18.0,
                text_color,
                icon,
            );
            let at = pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.bottom() - galley.size().y - 5.0,
            );
            painter.galley(at, galley, text_color);
        }
        None => {
            paint_icon(
                painter,
                pos2(rect.left() + 15.0, rect.center().y),
                14.0,
                text_color,
                icon,
            );
            let at = pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0);
            painter.galley(at, galley, text_color);
        }
    }
    enabled && response.clicked()
}

/// The leading button of a step: Next or Done.
fn primary(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let palette = theme::palette(ui.ctx());
    let height = if mobile::is_narrow(ui.ctx()) {
        mobile::TOUCH
    } else {
        28.0
    };
    let text = RichText::new(label).color(if enabled {
        palette.on_accent
    } else {
        palette.text_muted
    });
    let button = egui::Button::new(text)
        .fill(if enabled {
            palette.accent
        } else {
            palette.raised
        })
        .min_size(vec2(64.0, height));
    ui.add_enabled(enabled, button).clicked()
}

fn secondary(ui: &mut Ui, label: &str) -> bool {
    let height = if mobile::is_narrow(ui.ctx()) {
        mobile::TOUCH
    } else {
        28.0
    };
    ui.add(egui::Button::new(label).min_size(vec2(48.0, height)))
        .clicked()
}

/// The zoom buttons' places on the canvas: top right, one under another.
pub fn zoom_buttons(canvas: Rect, narrow: bool) -> Vec<(Rect, ZoomTo)> {
    let size = if narrow { mobile::TOUCH } else { 32.0 };
    [ZoomTo::In, ZoomTo::Out, ZoomTo::Fit]
        .into_iter()
        .enumerate()
        .map(|(i, to)| {
            let top = canvas.top() + 10.0 + i as f32 * (size + 8.0);
            let rect =
                Rect::from_min_size(pos2(canvas.right() - 10.0 - size, top), vec2(size, size));
            (rect, to)
        })
        .filter(|(rect, _)| rect.bottom() < canvas.bottom() - 8.0)
        .collect()
}

/// A round zoom button floating on the canvas. Returns true when clicked.
pub fn zoom_button(ui: &mut Ui, rect: Rect, to: ZoomTo) -> bool {
    let palette = theme::palette(ui.ctx());
    let (icon, label) = match to {
        ZoomTo::In => (Icon::ZoomIn, tr("Zoom in")),
        ZoomTo::Out => (Icon::ZoomOut, tr("Zoom out")),
        ZoomTo::Fit => (Icon::Fit, tr("Show it all")),
    };
    let response = ui
        .interact(
            rect,
            ui.scope_id().with(("marking-zoom", label)),
            Sense::click(),
        )
        .on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let fill = if response.is_pointer_button_down_on() {
        palette.active
    } else if response.hovered() {
        palette.hover
    } else {
        palette.background
    };
    let painter = ui.painter();
    painter.circle_filled(rect.center(), rect.width() / 2.0, fill.gamma_multiply(0.94));
    painter.circle_stroke(
        rect.center(),
        rect.width() / 2.0,
        Stroke::new(1.0, palette.stroke),
    );
    paint_icon(
        painter,
        rect.center(),
        rect.width() * 0.42,
        palette.text,
        icon,
    );
    response.clicked()
}

impl Marking {
    /// The steps as chips: the current one chosen, the earlier ones a tap
    /// back, the later ones dimmed.
    fn step_chips(&self, ui: &mut Ui, commands: &mut Vec<Command>) {
        let palette = theme::palette(ui.ctx());
        let current = self.depth() - 1;
        for (index, step) in self.steps().iter().enumerate() {
            if index > 0 {
                ui.label(RichText::new("›").color(palette.text_muted));
            }
            let text = RichText::new(step.name());
            let response = ui.add_enabled(
                index <= current,
                egui::Button::selectable(index == current, text),
            );
            if response.clicked() && index < current {
                commands.push(Command::GoTo(*step));
            }
        }
    }

    /// A phone's header: back, the steps as numbered dots with the current
    /// one named, and Cancel.
    fn phone_header(&self, ui: &mut Ui, commands: &mut Vec<Command>) {
        let palette = theme::palette(ui.ctx());
        let current = self.depth() - 1;
        let steps = self.steps();
        if current > 0 {
            let back = egui::Button::new(
                RichText::new("‹")
                    .color(palette.accent)
                    .text_style(TextStyle::Heading),
            )
            .frame(false)
            .min_size(vec2(mobile::TOUCH, mobile::TOUCH));
            if ui
                .add(back)
                .on_hover_text(steps[current - 1].name())
                .clicked()
            {
                commands.push(Command::Back);
            }
        }
        ui.spacing_mut().item_spacing.x = 0.0;
        let font = TextStyle::Small.resolve(ui.style());
        for (index, step) in steps.iter().enumerate() {
            let sense = if index < current {
                Sense::click()
            } else {
                Sense::hover()
            };
            let (rect, response) =
                ui.allocate_exact_size(vec2(mobile::TOUCH, mobile::TOUCH), sense);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, index < current, step.name())
            });
            if response.clicked() {
                commands.push(Command::GoTo(*step));
            }
            let painter = ui.painter();
            let (centre, radius) = (rect.center(), 12.0);
            let line = Stroke::new(1.5, palette.stroke);
            if index > 0 {
                painter.line_segment(
                    [
                        pos2(rect.left(), centre.y),
                        pos2(centre.x - radius - 2.0, centre.y),
                    ],
                    line,
                );
            }
            if index + 1 < steps.len() {
                painter.line_segment(
                    [
                        pos2(centre.x + radius + 2.0, centre.y),
                        pos2(rect.right(), centre.y),
                    ],
                    line,
                );
            }
            let (fill, number) = if index == current {
                (palette.accent, palette.on_accent)
            } else if index < current {
                (palette.raised, palette.text)
            } else {
                (Color32::TRANSPARENT, palette.text_muted)
            };
            painter.circle_filled(centre, radius, fill);
            if index != current {
                painter.circle_stroke(centre, radius, Stroke::new(1.2, palette.stroke));
            }
            painter.text(
                centre,
                egui::Align2::CENTER_CENTER,
                (index + 1).to_string(),
                font.clone(),
                number,
            );
        }
        ui.spacing_mut().item_spacing.x = 6.0;
        let cancel = RichText::new(tr("Cancel")).color(palette.accent);
        let cancel_width = ui
            .painter()
            .layout_no_wrap(
                tr("Cancel").to_owned(),
                TextStyle::Button.resolve(ui.style()),
                Color32::WHITE,
            )
            .size()
            .x
            + 2.0 * ui.spacing().button_padding.x;
        let name_width = (ui.available_width() - cancel_width.max(mobile::TOUCH) - 12.0).max(0.0);
        ui.allocate_ui_with_layout(
            vec2(name_width, mobile::TOUCH),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(RichText::new(self.step.name()).strong().color(palette.text))
                        .truncate(),
                );
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let button = egui::Button::new(cancel)
                .frame(false)
                .min_size(vec2(mobile::TOUCH, mobile::TOUCH));
            if ui.add(button).clicked() {
                commands.push(Command::Cancel);
            }
        });
    }

    /// The step's tools besides undo and redo.
    fn step_tools(&self, ui: &mut Ui, tall: Option<f32>, commands: &mut Vec<Command>) {
        match self.step {
            Step::Shots => {
                let shot = match self.selected {
                    Some(Handle::Shot(index)) => self.shot(index),
                    _ => None,
                };
                let (cold, flier) = shot.map_or((false, false), |(_, cold, flier)| (cold, flier));
                if tool_button(
                    ui,
                    Icon::ColdBore,
                    tr("Cold bore"),
                    shot.is_some(),
                    cold,
                    tall,
                ) {
                    commands.push(Command::ColdBore);
                }
                if tool_button(ui, Icon::Flier, tr("Flier"), shot.is_some(), flier, tall) {
                    commands.push(Command::Flier);
                }
                if tool_button(ui, Icon::Delete, tr("Delete"), shot.is_some(), false, tall) {
                    commands.push(Command::Delete);
                }
            }
            Step::Scale => {
                let point = matches!(self.selected, Some(Handle::Scale(_)));
                if tall.is_none()
                    && tool_button(ui, Icon::Delete, tr("Remove point"), point, false, tall)
                {
                    commands.push(Command::Delete);
                }
            }
            Step::Aim => {}
        }
    }

    fn undo_redo(&self, ui: &mut Ui, tall: Option<f32>, commands: &mut Vec<Command>) {
        if tool_button(ui, Icon::Undo, tr("Undo"), self.can_undo(), false, tall) {
            commands.push(Command::Undo);
        }
        if tool_button(
            ui,
            Icon::Redo,
            tr("Redo"),
            self.history.can_redo(),
            false,
            tall,
        ) {
            commands.push(Command::Redo);
        }
    }

    fn can_undo(&self) -> bool {
        self.history.can_undo()
            || self.scale != self.committed.scale
            || self.marks != self.committed.marks
    }

    /// The buttons that end a step, laid out right to left.
    fn step_ends(&self, ui: &mut Ui, commands: &mut Vec<Command>) {
        let narrow = mobile::is_narrow(ui.ctx());
        match self.step {
            Step::Scale => {
                let ready = matches!(self.scale.state(), ScaleState::Ready(_))
                    && !self.finder.awaits_size(&self.scale);
                let next = if narrow {
                    tr("Next")
                } else {
                    tr("Next: aim point")
                };
                if primary(ui, next, ready) {
                    commands.push(Command::Next);
                }
                if !ready && secondary(ui, tr("Skip for now")) {
                    commands.push(Command::Skip);
                }
            }
            Step::Aim => {
                let next = if narrow {
                    tr("Next")
                } else {
                    tr("Next: shots")
                };
                if primary(ui, next, true) {
                    commands.push(Command::Next);
                }
                if secondary(ui, tr("No aim point")) {
                    commands.push(Command::NoAim);
                }
            }
            Step::Shots => {
                if primary(ui, tr("Done"), true) {
                    commands.push(Command::Done);
                }
            }
        }
    }

    /// The live strip: shots, extreme spread and mean radius.
    fn strip(&self, ui: &mut Ui, live: &LiveView) {
        let palette = theme::palette(ui.ctx());
        egui::Frame::new()
            .fill(palette.raised)
            .stroke(palette.edge)
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.label(RichText::new(&live.text).color(palette.text).strong());
            });
    }

    /// The top of the view; the y where the canvas starts.
    pub(super) fn chrome_top(
        &mut self,
        ui: &mut Ui,
        full: Rect,
        narrow: bool,
        live: &LiveView,
        commands: &mut Vec<Command>,
    ) -> f32 {
        let palette = theme::palette(ui.ctx());
        let background = ui.painter().add(Shape::Noop);
        let margin = if narrow { 4.0 } else { 8.0 };
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(full.shrink2(vec2(margin, 0.0)))
                .layout(Layout::top_down(Align::Min)),
        );
        child.add_space(if narrow { 2.0 } else { 6.0 });
        if narrow {
            let row = child.horizontal(|ui| self.phone_header(ui, commands));
            remember_rect(ui, "marking-header", row.response.rect);
            if self.step == Step::Shots {
                self.strip(&mut child, live);
            }
        } else {
            child.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    commands.push(Command::Cancel);
                }
                ui.separator();
                self.step_chips(ui, commands);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.step_ends(ui, commands);
                });
            });
            child.horizontal_wrapped(|ui| {
                self.undo_redo(ui, None, commands);
                self.step_tools(ui, None, commands);
                if self.step == Step::Shots {
                    ui.separator();
                    self.strip(ui, live);
                }
            });
            child.label(RichText::new(self.instruction(false)).color(palette.text_muted));
            if self.step == Step::Scale {
                scale::controls(&mut child, &mut self.scale, &mut self.finder, self.sight);
            }
        }
        child.add_space(if narrow { 4.0 } else { 6.0 });
        let bottom = child.min_rect().bottom().min(full.bottom() - 60.0);
        let rect = Rect::from_min_max(full.min, pos2(full.right(), bottom));
        ui.painter()
            .set(background, Shape::rect_filled(rect, 0.0, palette.chrome));
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, palette.stroke),
        );
        bottom
    }

    /// The bottom of a phone's view; the y where the canvas ends.
    pub(super) fn chrome_bottom(
        &mut self,
        ui: &mut Ui,
        full: Rect,
        commands: &mut Vec<Command>,
    ) -> f32 {
        let palette = theme::palette(ui.ctx());
        let id = ui
            .scope_id()
            .with(("marking-bottom-height", self.step == Step::Scale));
        let guess: f32 = ui
            .data(|data| data.get_temp(id))
            .unwrap_or(if self.step == Step::Scale {
                230.0
            } else {
                90.0
            });
        let top = (full.bottom() - guess).max(full.top() + 80.0);
        let background = ui.painter().add(Shape::Noop);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(full.left() + 6.0, top),
                    pos2(full.right() - 6.0, full.bottom()),
                ))
                .layout(Layout::top_down(Align::Min)),
        );
        child.add_space(4.0);
        child.add(
            egui::Label::new(RichText::new(self.instruction(true)).color(palette.text_muted))
                .wrap(),
        );
        if self.step == Step::Scale {
            scale::controls(&mut child, &mut self.scale, &mut self.finder, self.sight);
        }
        let row = child.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if self.step == Step::Shots {
                let width = ((ui.available_width() - 10.0) / 6.0).floor();
                self.undo_redo(ui, Some(width), commands);
                self.step_tools(ui, Some(width), commands);
                if tool_button(ui, Icon::Done, tr("Done"), true, true, Some(width)) {
                    commands.push(Command::Done);
                }
            } else {
                self.undo_redo(ui, Some(mobile::TOUCH + 8.0), commands);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    self.step_ends(ui, commands);
                });
            }
        });
        remember_rect(ui, "marking-toolbar", row.response.rect);
        child.add_space(4.0);
        let used = child.min_rect().height();
        if (used - guess).abs() > 0.5 {
            ui.data_mut(|data| data.insert_temp(id, used));
            ui.ctx()
                .request_discard("marking: the toolbar's height changed");
        }
        let rect = Rect::from_min_max(pos2(full.left(), top), full.max);
        ui.painter()
            .set(background, Shape::rect_filled(rect, 0.0, palette.chrome));
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0, palette.stroke),
        );
        top
    }
}

/// Keeps where a row of controls ended up, for the layout tests.
fn remember_rect(ui: &Ui, name: &'static str, rect: Rect) {
    ui.data_mut(|data| data.insert_temp(egui::Id::unique(name), rect));
}

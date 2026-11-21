//! The Groups home: every group, newest first, under the session it was
//! shot in or the day, with filters by rifle, bullet, tags and notes.

use ballistics_store::sun::Date;
use ballistics_store::{Analysis, Group, Session};
use eframe::egui::{self, CornerRadius, RichText, Sense, pos2, vec2};

use super::{GroupSettings, RankBy, Summary};
use crate::app::Data;
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};

const DAY_MS: i64 = 86_400_000;

/// What the library shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
    pub rifle_id: Option<String>,
    pub bullet_id: Option<String>,
    /// Words to find in the tags or notes.
    pub search: String,
    pub show_archived: bool,
}

impl Filters {
    pub fn matches(&self, group: &Group) -> bool {
        if group.archived && !self.show_archived {
            return false;
        }
        if self
            .rifle_id
            .as_ref()
            .is_some_and(|id| group.rifle_id.as_ref() != Some(id))
        {
            return false;
        }
        if self
            .bullet_id
            .as_ref()
            .is_some_and(|id| group.bullet_id.as_ref() != Some(id))
        {
            return false;
        }
        let words = self.search.to_lowercase();
        words.split_whitespace().all(|word| {
            group.notes.to_lowercase().contains(word)
                || group
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(word))
        })
    }
}

/// The groups picked in the library, to compare, pool, archive or delete
/// together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub ids: Vec<String>,
    /// On a phone: taps pick groups instead of opening them.
    pub selecting: bool,
    /// The last group picked, where a Shift-click range starts.
    anchor: Option<String>,
}

/// How a group in the library was clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Plain,
    /// Ctrl/⌘-click, a long press, or a tap while selecting.
    Toggle,
    /// Shift-click.
    Range,
}

impl Selection {
    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|picked| picked == id)
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.selecting = false;
        self.anchor = None;
    }

    /// Picks or opens after a click on `id`; `order` is the groups as the
    /// library lists them. True when the group should open.
    pub fn click(&mut self, id: &str, how: Click, order: &[&str]) -> bool {
        let how = match how {
            Click::Plain if self.selecting => Click::Toggle,
            other => other,
        };
        match how {
            Click::Plain => {
                self.ids.clear();
                self.anchor = None;
                true
            }
            Click::Toggle => {
                if let Some(at) = self.ids.iter().position(|picked| picked == id) {
                    self.ids.remove(at);
                } else {
                    self.ids.push(id.to_owned());
                }
                self.anchor = Some(id.to_owned());
                false
            }
            Click::Range => {
                let place = |id: &str| order.iter().position(|listed| *listed == id);
                match (self.anchor.as_deref().and_then(place), place(id)) {
                    (Some(from), Some(to)) => {
                        let (low, high) = (from.min(to), from.max(to));
                        self.ids = order[low..=high]
                            .iter()
                            .map(|id| (*id).to_owned())
                            .collect();
                    }
                    _ => {
                        if !self.contains(id) {
                            self.ids.push(id.to_owned());
                        }
                        self.anchor = Some(id.to_owned());
                    }
                }
                false
            }
        }
    }

    /// Forgets groups that are gone or no longer listed.
    pub fn keep_only(&mut self, listed: &[&str]) {
        self.ids.retain(|id| listed.contains(&id.as_str()));
    }
}

/// What a run of the library's groups are listed under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heading {
    /// The session they were shot in.
    Session(String),
    /// The day, in days since 1970 on the device's clock, of groups shot in
    /// no session (or one this device doesn't have).
    Day(i64),
}

/// The groups under one heading, newest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub heading: Heading,
    /// Indices into the groups given.
    pub groups: Vec<usize>,
    /// Indices of the saved analyses listed here, under their newest
    /// group's heading.
    pub analyses: Vec<usize>,
}

/// The groups `filters` lets through, newest first, under their session or
/// day; sections in the order of their newest group. `local` turns a time
/// into the device's clock.
pub fn sections(
    groups: &[Group],
    sessions: &[Session],
    filters: &Filters,
    local: impl Fn(i64) -> i64,
) -> Vec<Section> {
    let mut order: Vec<usize> = (0..groups.len())
        .filter(|&i| filters.matches(&groups[i]))
        .collect();
    order.sort_by(|&a, &b| {
        groups[b]
            .taken_at
            .cmp(&groups[a].taken_at)
            .then_with(|| groups[a].id.cmp(&groups[b].id))
    });
    let mut sections: Vec<Section> = Vec::new();
    for index in order {
        let group = &groups[index];
        let heading = match &group.session_id {
            Some(id) if sessions.iter().any(|session| &session.id == id) => {
                Heading::Session(id.clone())
            }
            _ => Heading::Day(local(group.taken_at).div_euclid(DAY_MS)),
        };
        match sections
            .iter_mut()
            .find(|section| section.heading == heading)
        {
            Some(section) => section.groups.push(index),
            None => sections.push(Section {
                heading,
                groups: vec![index],
                analyses: Vec::new(),
            }),
        }
    }
    sections
}

/// Puts each saved analysis under the heading of its newest group the
/// library lists; one with no group listed is left out.
pub fn place_analyses(sections: &mut [Section], groups: &[Group], analyses: &[Analysis]) {
    for (index, analysis) in analyses.iter().enumerate() {
        let newest = sections
            .iter()
            .enumerate()
            .flat_map(|(at, section)| section.groups.iter().map(move |&g| (at, g)))
            .filter(|(_, g)| analysis.group_ids.contains(&groups[*g].id))
            .max_by_key(|(_, g)| groups[*g].taken_at);
        if let Some((at, _)) = newest {
            sections[at].analyses.push(index);
        }
    }
}

/// "Today", "Yesterday", "12 Nov", or "12 Nov 2025" in another year.
pub fn day_name(day: i64, today: i64) -> String {
    match today - day {
        0 => tr("Today").to_owned(),
        1 => tr("Yesterday").to_owned(),
        _ => {
            let date = Date::from_days(day);
            let text = crate::hunting::day_text(date);
            if date.year == Date::from_days(today).year {
                text
            } else {
                format!("{text} {}", date.year)
            }
        }
    }
}

/// A session as one line: its day, place and weather.
pub fn session_line(
    session: &Session,
    today: i64,
    local: impl Fn(i64) -> i64,
    u: &UnitPrefs,
) -> String {
    let mut parts = vec![day_name(
        local(session.started_at).div_euclid(DAY_MS),
        today,
    )];
    if !session.place.trim().is_empty() {
        parts.push(session.place.trim().to_owned());
    }
    let whole = |quantity: Quantity, value: f64| {
        let unit = u.unit(quantity);
        format!("{:.0} {}", unit.show(value), unit.label)
    };
    if let Some(temperature) = session.conditions.temperature_c {
        parts.push(whole(Quantity::Temperature, temperature));
    }
    if let Some(pressure) = session.conditions.pressure_hpa {
        parts.push(whole(Quantity::Pressure, pressure));
    }
    parts.join(" · ")
}

/// What a tap in the library asked for.
pub enum Event {
    Open(String),
    NewGroup,
    Grid,
    /// The phone's Analyse button.
    Analyse,
    OpenAnalysis(String),
    Compare(Vec<String>),
    Composite(Vec<String>),
    /// Archive (true) or bring back (false) these groups.
    Archive(Vec<String>, bool),
    Delete(Vec<String>),
}

/// The library: the new-group buttons, filters, the groups and settings.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    data: &Data,
    filters: &mut Filters,
    settings: &mut GroupSettings,
    summaries: &mut super::Summaries,
    photos: (
        &mut super::textures::Textures,
        &dyn ballistics_store::PhotoStore,
    ),
    selection: &mut Selection,
    selected: Option<&str>,
    enabled: bool,
    phone: bool,
) -> Option<Event> {
    let (textures, store) = photos;
    let palette = theme::palette(ui.ctx());
    let mut event = None;
    ui.add_enabled_ui(enabled, |ui| {
        if phone {
            ui.horizontal(|ui| {
                ui.heading(tr("Groups"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(tr("📈 Analyse"))
                                .min_size(vec2(mobile::TOUCH, mobile::TOUCH - 8.0)),
                        )
                        .on_hover_text(tr("Zero, trend, compare and load ladders"))
                        .clicked()
                    {
                        event = Some(Event::Analyse);
                    }
                    let label = if selection.selecting {
                        tr("Done")
                    } else {
                        tr("Select")
                    };
                    if ui
                        .add(
                            egui::Button::new(label)
                                .min_size(vec2(mobile::TOUCH, mobile::TOUCH - 8.0)),
                        )
                        .clicked()
                    {
                        if selection.selecting {
                            selection.clear();
                        } else {
                            selection.selecting = true;
                        }
                    }
                });
            });
        }
        ui.horizontal(|ui| {
            let height = if phone { mobile::TOUCH + 8.0 } else { 34.0 };
            let width = (ui.available_width() * 0.62).max(120.0);
            let new = ui.add(
                egui::Button::new(
                    RichText::new(tr("+ New group"))
                        .color(palette.on_accent)
                        .strong()
                        .size(if phone { 18.0 } else { 15.0 }),
                )
                .fill(palette.accent)
                .min_size(vec2(width, height)),
            );
            if new
                .on_hover_text(tr("Mark the shots on a photo of the target"))
                .clicked()
            {
                event = Some(Event::NewGroup);
            }
            let grid = ui.add(
                egui::Button::new(RichText::new(tr("⌗ Grid")).size(if phone {
                    16.0
                } else {
                    14.0
                }))
                .min_size(vec2(ui.available_width(), height)),
            );
            if grid
                .on_hover_text(tr(
                    "A blank grid at true scale, for a target you didn't photograph",
                ))
                .clicked()
            {
                event = Some(Event::Grid);
            }
        });
        if !phone {
            ui.weak(tr("Or drop a photo of the target onto the window."));
            ui.weak(tr("Ctrl/⌘-click or Shift-click groups to pick several."));
        }
        ui.add_space(4.0);
        filter_bar(ui, data, filters);
        ui.add_space(4.0);

        let country = data.country;
        let u = units::prefs(ui.ctx());
        let local = |ms: i64| super::local_ms(ms, country);
        let today = local(ballistics_store::model::now_ms()).div_euclid(DAY_MS);
        let groups = &data.groups.groups;
        let mut shown = sections(groups, &data.groups.sessions, filters, local);
        place_analyses(&mut shown, groups, &data.groups.analyses);
        let order: Vec<&str> = shown
            .iter()
            .flat_map(|section| section.groups.iter().map(|&i| groups[i].id.as_str()))
            .collect();
        selection.keep_only(&order);
        if groups.is_empty() {
            ui.add_space(8.0);
            ui.weak(tr(
                "No groups yet. Photograph the target after a string and mark the holes: the \
                 app works out the group's size, where it landed and what to dial. No photo? \
                 Use a blank grid.",
            ));
        } else if shown.is_empty() {
            ui.weak(tr("No group matches the filters."));
        }
        let modifiers = ui.input(|input| input.modifiers);
        for section in &shown {
            let title = match &section.heading {
                Heading::Session(id) => data
                    .groups
                    .sessions
                    .iter()
                    .find(|session| &session.id == id)
                    .map(|session| session_line(session, today, local, &u))
                    .unwrap_or_default(),
                Heading::Day(day) => day_name(*day, today),
            };
            ui.add_space(6.0);
            ui.label(RichText::new(title).strong().color(palette.text_muted));
            for &index in &section.analyses {
                let analysis = &data.groups.analyses[index];
                if analysis_row(ui, analysis, data, phone).clicked() {
                    event = Some(Event::OpenAnalysis(analysis.id.clone()));
                }
            }
            for &index in &section.groups {
                let group = &groups[index];
                let summary = summaries.of(group, data, &u);
                let photo = match (&summary.measured, super::photo_of(group, data)) {
                    (None, Some(photo)) => match textures.thumbnail(ui.ctx(), store, photo) {
                        super::textures::State::Ready(image) => {
                            Some((image.texture.id(), image.texture.size_vec2()))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let picked = selection.contains(&group.id);
                let check = (selection.selecting || !selection.ids.is_empty()).then_some(picked);
                let row = group_row(
                    ui,
                    group,
                    &summary,
                    photo,
                    settings.rank_by,
                    picked || selected == Some(group.id.as_str()),
                    check,
                    phone,
                );
                let how = if row.long_touched() {
                    selection.selecting = true;
                    Some(Click::Toggle)
                } else if row.clicked() {
                    Some(if modifiers.shift {
                        Click::Range
                    } else if modifiers.command {
                        Click::Toggle
                    } else {
                        Click::Plain
                    })
                } else {
                    None
                };
                if let Some(how) = how
                    && selection.click(&group.id, how, &order)
                {
                    event = Some(Event::Open(group.id.clone()));
                }
            }
        }
        ui.add_space(12.0);
        crate::widgets::fold(ui, "groups-settings", tr("Settings"), "", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Rank groups by"));
                ui.selectable_value(&mut settings.rank_by, RankBy::MeanRadius, tr("mean radius"));
                ui.selectable_value(
                    &mut settings.rank_by,
                    RankBy::ExtremeSpread,
                    tr("extreme spread"),
                );
            });
            ui.checkbox(
                &mut settings.keep_original_photos,
                tr("Keep original photos"),
            );
            ui.weak(tr(
                "Off: photos are reduced to 4096 pixels on the long side as they come in, \
                 which is plenty for marking and keeps this device's storage small. Either way \
                 their location is taken out.",
            ));
        });
    });
    event
}

/// What to do with the picked groups: compare or pool them, archive or
/// delete them.
/// What to do with the picked groups, kept above the list so it stays in
/// view while picking.
pub fn selection_bar(
    ui: &mut egui::Ui,
    selection: &mut Selection,
    data: &Data,
    phone: bool,
) -> Option<Event> {
    if selection.ids.is_empty() && !selection.selecting {
        return None;
    }
    let palette = theme::palette(ui.ctx());
    let mut event = None;
    egui::Frame::new()
        .fill(palette.raised)
        .stroke(egui::Stroke::new(1.0_f32, palette.accent))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let count = selection.ids.len();
            ui.label(RichText::new(match count {
                0 => tr("Tap groups to pick them").to_owned(),
                1 => tr("1 group picked").to_owned(),
                n => trf!("{} groups picked", n),
            }));
            ui.horizontal_wrapped(|ui| {
                let size = vec2(0.0, if phone { mobile::TOUCH - 8.0 } else { 0.0 });
                let ids = selection.ids.clone();
                if ui
                    .add_enabled(count >= 2, egui::Button::new(tr("Compare")).min_size(size))
                    .clicked()
                {
                    event = Some(Event::Compare(ids.clone()));
                }
                if ui
                    .add_enabled(
                        count >= 2,
                        egui::Button::new(tr("Composite")).min_size(size),
                    )
                    .clicked()
                {
                    event = Some(Event::Composite(ids.clone()));
                }
                let archived = !ids.is_empty()
                    && ids.iter().all(|id| {
                        data.groups
                            .groups
                            .iter()
                            .any(|group| &group.id == id && group.archived)
                    });
                let archive = if archived {
                    tr("Bring back")
                } else {
                    tr("Archive")
                };
                if ui
                    .add_enabled(count >= 1, egui::Button::new(archive).min_size(size))
                    .clicked()
                {
                    event = Some(Event::Archive(ids.clone(), !archived));
                }
                if ui
                    .add(egui::Button::new(tr("Cancel")).min_size(size))
                    .clicked()
                {
                    selection.clear();
                }
            });
            if count >= 1 {
                ui.add_space(4.0);
                let question = if count == 1 {
                    tr("Delete this group? Undo brings it back for a moment.").to_owned()
                } else {
                    trf!(
                        "Delete these {} groups? Undo brings them back for a moment.",
                        count
                    )
                };
                if super::analyse::confirm(
                    ui,
                    "groups-delete-picked",
                    tr("Delete"),
                    &question,
                    true,
                ) {
                    event = Some(Event::Delete(selection.ids.clone()));
                }
            }
        });
    if event.is_some() {
        selection.clear();
    }
    event
}

/// A saved analysis's line in the library, marked as what it is.
fn analysis_row(
    ui: &mut egui::Ui,
    analysis: &Analysis,
    data: &Data,
    phone: bool,
) -> egui::Response {
    let palette = theme::palette(ui.ctx());
    let height = if phone { 56.0 } else { 46.0 };
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if response.is_pointer_button_down_on() || response.hovered() {
        palette.hover
    } else {
        palette.raised
    };
    painter.rect(
        rect.shrink2(vec2(0.0, 2.0)),
        CornerRadius::same(10),
        fill,
        egui::Stroke::new(1.0_f32, palette.accent.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let icon = egui::Rect::from_min_size(
        rect.left_top() + vec2(8.0, 8.0),
        vec2(height - 16.0, height - 16.0),
    );
    painter.rect_filled(icon, 6.0, palette.accent.gamma_multiply(0.2));
    painter.text(
        icon.center(),
        egui::Align2::CENTER_CENTER,
        match analysis.kind {
            ballistics_store::AnalysisKind::Composite => "⧉",
            ballistics_store::AnalysisKind::Compare => "⇆",
            ballistics_store::AnalysisKind::Ladder => "≡",
        },
        egui::FontId::proportional(icon.height() * 0.6),
        palette.accent,
    );
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::FontId::proportional(body.size * 0.85);
    let kind = match analysis.kind {
        ballistics_store::AnalysisKind::Composite => tr("COMPOSITE"),
        ballistics_store::AnalysisKind::Compare => tr("COMPARISON"),
        ballistics_store::AnalysisKind::Ladder => tr("LADDER"),
    };
    let badge = painter.layout_no_wrap(kind.to_owned(), small.clone(), palette.accent);
    let right = rect.right() - 12.0;
    let badge_width = badge.size().x;
    painter.galley(
        pos2(right - badge_width, rect.center().y - badge.size().y / 2.0),
        badge,
        palette.accent,
    );
    let text_x = icon.right() + 10.0;
    let text_width = (right - badge_width - 10.0 - text_x).max(20.0);
    let line = |text: String, font: egui::FontId, colour| {
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, colour);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
        painter.layout_job(job)
    };
    let title = line(super::analyse::analysis_name(analysis), body, palette.text);
    painter.galley(pos2(text_x, rect.top() + 7.0), title, palette.text);
    let detail = line(
        super::analyse::analysis_detail(analysis, data),
        small,
        palette.text_muted,
    );
    let at = pos2(text_x, rect.bottom() - 7.0 - detail.size().y);
    painter.galley(at, detail, palette.text_muted);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn filter_bar(ui: &mut egui::Ui, data: &Data, filters: &mut Filters) {
    let groups = &data.groups.groups;
    ui.horizontal_wrapped(|ui| {
        let rifles: Vec<_> = data
            .rifles
            .iter()
            .filter(|rifle| {
                groups
                    .iter()
                    .any(|group| group.rifle_id.as_ref() == Some(&rifle.id))
            })
            .collect();
        let rifle_name = filters
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
            .map_or_else(|| tr("All rifles").to_owned(), |rifle| rifle.name.clone());
        egui::ComboBox::from_id_salt("groups-rifle-filter")
            .selected_text(rifle_name)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filters.rifle_id, None, tr("All rifles"));
                for rifle in rifles {
                    ui.selectable_value(&mut filters.rifle_id, Some(rifle.id.clone()), &rifle.name);
                }
            });
        let bullets: Vec<_> = data
            .bullets
            .iter()
            .filter(|bullet| {
                groups.iter().any(|group| {
                    group.bullet_id.as_ref() == Some(&bullet.id)
                        && filters
                            .rifle_id
                            .as_ref()
                            .is_none_or(|rifle| group.rifle_id.as_ref() == Some(rifle))
                })
            })
            .collect();
        let bullet_name = filters
            .bullet_id
            .as_ref()
            .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id))
            .map_or_else(
                || tr("All loads").to_owned(),
                |bullet| bullet.display_name(),
            );
        egui::ComboBox::from_id_salt("groups-bullet-filter")
            .selected_text(bullet_name)
            .width(160.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filters.bullet_id, None, tr("All loads"));
                for bullet in bullets {
                    ui.selectable_value(
                        &mut filters.bullet_id,
                        Some(bullet.id.clone()),
                        bullet.display_name(),
                    );
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut filters.search)
                .hint_text(tr("⌕ Tags or notes"))
                .desired_width(140.0),
        );
        ui.checkbox(&mut filters.show_archived, tr("Show archived"));
    });
}

/// A group's line in the library: its picture, what was shot, and its size
/// by the measure the library ranks by.
#[allow(clippy::too_many_arguments)]
fn group_row(
    ui: &mut egui::Ui,
    group: &Group,
    summary: &Summary,
    photo: Option<(egui::TextureId, egui::Vec2)>,
    rank_by: RankBy,
    selected: bool,
    check: Option<bool>,
    phone: bool,
) -> egui::Response {
    let palette = theme::palette(ui.ctx());
    let height = if phone { 64.0 } else { 54.0 };
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if selected || response.is_pointer_button_down_on() {
        palette.hover
    } else if response.hovered() {
        palette.raised.lerp_to_gamma(palette.hover, 0.5)
    } else {
        palette.raised
    };
    let edge = if selected && !palette.dark {
        egui::Stroke::new(1.5_f32, palette.accent)
    } else {
        palette.edge
    };
    painter.rect(
        rect.shrink2(vec2(0.0, 2.0)),
        CornerRadius::same(10),
        fill,
        edge,
        egui::StrokeKind::Inside,
    );
    let mut left = rect.left() + 8.0;
    if let Some(checked) = check {
        let centre = pos2(left + 10.0, rect.center().y);
        if checked {
            painter.circle_filled(centre, 9.0, palette.accent);
            painter.text(
                centre,
                egui::Align2::CENTER_CENTER,
                "✓",
                egui::FontId::proportional(13.0),
                palette.on_accent,
            );
        } else {
            painter.circle_stroke(centre, 9.0, egui::Stroke::new(1.5_f32, palette.text_muted));
        }
        left += 26.0;
    }
    let picture = egui::Rect::from_min_size(
        pos2(left, rect.top() + 8.0),
        vec2(height - 16.0, height - 16.0),
    );
    match photo {
        Some((texture, size)) => {
            let shown = egui::Rect::from_center_size(
                picture.center(),
                size * (picture.width() / size.x.max(size.y)),
            );
            painter.image(
                texture,
                shown,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => super::drawing::thumbnail(&painter, picture, summary.measured.as_deref(), palette),
    }

    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::FontId::proportional(body.size * 0.85);
    let (big, little) = size_texts(summary, rank_by, group.distance_m, &units::prefs(ui.ctx()));
    let size_galley = painter.layout_no_wrap(big, body.clone(), palette.text);
    let little_galley = painter.layout_no_wrap(little, small.clone(), palette.text_muted);
    let size_width = size_galley.size().x.max(little_galley.size().x);
    let right = rect.right() - 12.0;
    painter.galley(
        pos2(right - size_galley.size().x, rect.top() + 10.0),
        size_galley,
        palette.text,
    );
    painter.galley(
        pos2(
            right - little_galley.size().x,
            rect.bottom() - 10.0 - little_galley.size().y,
        ),
        little_galley,
        palette.text_muted,
    );
    let text_x = picture.right() + 10.0;
    let text_width = (right - size_width - 10.0 - text_x).max(20.0);
    let line = |text: String, font: egui::FontId, colour| {
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, colour);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
        painter.layout_job(job)
    };
    let mut title = summary.title.clone();
    if group.archived {
        title = trf!("{} (archived)", title);
    }
    let title = line(title, body, palette.text);
    painter.galley(pos2(text_x, rect.top() + 10.0), title, palette.text);
    let subtitle = line(summary.subtitle.clone(), small, palette.text_muted);
    let at = pos2(text_x, rect.bottom() - 10.0 - subtitle.size().y);
    painter.galley(at, subtitle, palette.text_muted);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The size the library ranks by, large, and the other one small: as
/// angles at the group's distance, or on the target without one.
fn size_texts(
    summary: &Summary,
    rank_by: RankBy,
    distance_m: Option<f64>,
    u: &UnitPrefs,
) -> (String, String) {
    let Some(stats) = summary.measured.as_deref().and_then(|m| m.stats.as_ref()) else {
        return ("—".to_owned(), summary.problem.clone().unwrap_or_default());
    };
    let size = |mm: f64| match distance_m {
        Some(distance) => {
            let unit = u.unit(Quantity::GroupAngle);
            format!(
                "{:.*} {}",
                unit.decimals,
                unit.show(groups_core::angles::mrad(mm, distance)),
                unit.label
            )
        }
        None => u.format(Quantity::GroupSize, mm),
    };
    let short = |mm: f64| {
        let text = size(mm);
        text.split(' ').next().unwrap_or_default().to_owned()
    };
    let extreme = stats
        .spread
        .map(|spread| spread.extreme_spread.centre_to_centre);
    match (rank_by, extreme) {
        (_, None) => (tr("1 shot").to_owned(), String::new()),
        (RankBy::MeanRadius, Some(es)) => (
            trf!("MR {}", size(stats.mean_radius)),
            trf!("ES {}", short(es)),
        ),
        (RankBy::ExtremeSpread, Some(es)) => (
            trf!("ES {}", size(es)),
            trf!("MR {}", short(stats.mean_radius)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, taken_at: i64, session: Option<&str>) -> Group {
        let mut group = Group::on_grid(200.0, 200.0, taken_at);
        group.id = id.into();
        group.session_id = session.map(str::to_owned);
        group
    }

    #[test]
    fn groups_are_listed_newest_first_under_their_session_or_day() {
        let mut session = Session::new(5 * DAY_MS);
        session.id = "range".into();
        let groups = vec![
            group("a", 2 * DAY_MS + 1_000, None),
            group("b", 5 * DAY_MS + 3_000, Some("range")),
            group("c", 5 * DAY_MS + 9_000, None),
            group("d", 5 * DAY_MS + 5_000, Some("range")),
            group("e", 2 * DAY_MS + 8_000, Some("gone")),
            group("f", 9 * DAY_MS, None),
        ];
        let filters = Filters::default();
        let listed = sections(&groups, &[session], &filters, |ms| ms);
        let ids = |section: &Section| -> Vec<&str> {
            section
                .groups
                .iter()
                .map(|&i| groups[i].id.as_str())
                .collect()
        };
        assert_eq!(listed.len(), 4);
        assert_eq!(listed[0].heading, Heading::Day(9));
        assert_eq!(listed[1].heading, Heading::Day(5));
        assert_eq!(ids(&listed[1]), ["c"]);
        assert_eq!(listed[2].heading, Heading::Session("range".into()));
        assert_eq!(ids(&listed[2]), ["d", "b"], "newest first");
        assert_eq!(
            listed[3].heading,
            Heading::Day(2),
            "a session this device doesn't have: the day"
        );
        assert_eq!(ids(&listed[3]), ["e", "a"]);

        // The device's clock decides the day.
        let shifted = sections(&groups[..1], &[], &filters, |ms| ms + DAY_MS);
        assert_eq!(shifted[0].heading, Heading::Day(3));
    }

    #[test]
    fn filters_pick_by_rifle_bullet_words_and_archive() {
        let mut groups = [
            group("a", 1, None),
            group("b", 2, None),
            group("c", 3, None),
        ];
        groups[0].rifle_id = Some("tikka".into());
        groups[0].bullet_id = Some("eldm".into());
        groups[0].tags = vec!["Practice".into(), "prone".into()];
        groups[1].rifle_id = Some("tikka".into());
        groups[1].notes = "Windy at the range, practice".into();
        groups[2].archived = true;
        groups[2].tags = vec!["practice".into()];
        let pick = |filters: &Filters| -> Vec<&str> {
            groups
                .iter()
                .filter(|group| filters.matches(group))
                .map(|group| group.id.as_str())
                .collect()
        };
        let mut filters = Filters::default();
        assert_eq!(pick(&filters), ["a", "b"], "archived groups are left out");
        filters.show_archived = true;
        assert_eq!(pick(&filters), ["a", "b", "c"]);
        filters.search = "PRACTICE".into();
        assert_eq!(pick(&filters), ["a", "b", "c"], "tags and notes, any case");
        filters.search = "practice windy".into();
        assert_eq!(pick(&filters), ["b"], "every word must be found");
        filters.search.clear();
        filters.rifle_id = Some("tikka".into());
        assert_eq!(pick(&filters), ["a", "b"]);
        filters.bullet_id = Some("eldm".into());
        assert_eq!(pick(&filters), ["a"]);
    }

    #[test]
    fn days_read_as_today_yesterday_or_a_date() {
        let today = Date {
            year: 2026,
            month: 11,
            day: 21,
        }
        .days();
        assert_eq!(day_name(today, today), "Today");
        assert_eq!(day_name(today - 1, today), "Yesterday");
        assert_eq!(day_name(today - 9, today), "12 Nov");
        assert_eq!(day_name(today - 365, today), "21 Nov 2025");
    }

    #[test]
    fn groups_are_picked_by_toggle_range_or_a_tap_while_selecting() {
        let order = ["a", "b", "c", "d", "e"];
        let mut selection = Selection::default();
        assert!(
            selection.click("b", Click::Plain, &order),
            "a plain click opens"
        );
        assert!(selection.ids.is_empty());

        assert!(!selection.click("b", Click::Toggle, &order));
        assert!(!selection.click("d", Click::Toggle, &order));
        assert_eq!(selection.ids, ["b", "d"]);
        selection.click("b", Click::Toggle, &order);
        assert_eq!(selection.ids, ["d"], "a second toggle unpicks");

        // Shift-click picks from the last one clicked, either way.
        selection.click("a", Click::Range, &order);
        assert_eq!(selection.ids, ["a", "b"]);
        selection.click("e", Click::Toggle, &order);
        selection.click("c", Click::Range, &order);
        assert_eq!(selection.ids, ["c", "d", "e"]);

        // A plain click clears the picks and opens.
        assert!(selection.click("a", Click::Plain, &order));
        assert!(selection.ids.is_empty());

        // On a phone, Select makes taps pick.
        selection.selecting = true;
        assert!(!selection.click("a", Click::Plain, &order));
        assert!(!selection.click("c", Click::Plain, &order));
        assert_eq!(selection.ids, ["a", "c"]);
        selection.keep_only(&["c", "d"]);
        assert_eq!(selection.ids, ["c"], "groups no longer listed drop out");
        selection.clear();
        assert!(!selection.selecting && selection.ids.is_empty());
    }

    #[test]
    fn saved_analyses_sit_under_their_newest_listed_group() {
        let groups = vec![
            group("a", 2 * DAY_MS, None),
            group("b", 5 * DAY_MS, None),
            group("c", 9 * DAY_MS, None),
        ];
        let mut sections = sections(&groups, &[], &Filters::default(), |ms| ms);
        let analyses = vec![
            Analysis::new(
                ballistics_store::AnalysisKind::Composite,
                "",
                vec!["a".into(), "b".into()],
            ),
            Analysis::new(
                ballistics_store::AnalysisKind::Compare,
                "",
                vec!["gone".into()],
            ),
        ];
        place_analyses(&mut sections, &groups, &analyses);
        let day = |day: i64| {
            sections
                .iter()
                .find(|s| s.heading == Heading::Day(day))
                .unwrap()
        };
        assert_eq!(day(5).analyses, [0], "under b, its newest group");
        assert!(day(2).analyses.is_empty() && day(9).analyses.is_empty());
    }
}

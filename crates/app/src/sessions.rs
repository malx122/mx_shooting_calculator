//! The Sessions page: days at the range, newest first, each with its
//! weather and the groups shot in it. The weather can be filled in from
//! Open-Meteo, asking for the device's position only when the button is
//! pressed.

use ballistics_core::Atmosphere;
use ballistics_store::model::now_ms;
use ballistics_store::{Conditions, Country, Session};
use eframe::egui::{self, RichText};

use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::number_field::NumberField;
use crate::records::Change;
use crate::units::{self, Quantity};
use crate::weather::{Report, Weather};
use crate::widgets::{confirm_button, grid, text};

const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;

#[derive(Default)]
pub struct SessionsPage {
    /// The session open in the editor, with unsaved changes.
    editing: Option<Session>,
    /// A new session as it was opened, so an untouched one isn't taken for
    /// unsaved changes.
    blank: Option<Session>,
    /// The editor's date and time as typed.
    when: When,
    weather: Weather,
    /// The weather asked for: for which session, and how.
    asked: Option<Asked>,
    /// The weather last filled in: for which session, and the place a name
    /// search found (`None` for the device's position).
    filled: Option<(String, Option<String>)>,
}

/// A request for the weather now.
struct Asked {
    session_id: String,
    /// By the place's name, rather than the device's position.
    by_name: bool,
}

/// A session's start as typed: "2026-11-21" and "14:30", local time.
#[derive(Clone, Debug, Default, PartialEq)]
struct When {
    session_id: String,
    date: String,
    time: String,
}

/// Unix milliseconds for a local `date` ("2026-11-21") and `time`
/// ("14:30"), with `offset` the local clock's lead on UTC at a moment.
pub fn parse_when(date: &str, time: &str, offset: impl Fn(i64) -> i64) -> Option<i64> {
    let mut parts = date.trim().split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let date_value = ballistics_store::sun::Date { year, month, day };
    let days = date_value.days();
    if ballistics_store::sun::Date::from_days(days) != date_value {
        return None;
    }
    let (hours, minutes) = time.trim().split_once([':', '.'])?;
    let (hours, minutes): (i64, i64) = (hours.parse().ok()?, minutes.parse().ok()?);
    if !(0..24).contains(&hours) || !(0..60).contains(&minutes) {
        return None;
    }
    let local = days * DAY_MS + (hours * 60 + minutes) * MINUTE_MS;
    let unix = local - offset(local);
    (unix >= 0).then_some(local - offset(unix))
}

fn when_of(session: &Session, country: Option<Country>) -> When {
    let local = crate::groups::local_ms(session.started_at, country);
    let date = ballistics_store::sun::Date::from_days(local.div_euclid(DAY_MS));
    let minutes = local.rem_euclid(DAY_MS) / MINUTE_MS;
    When {
        session_id: session.id.clone(),
        date: format!("{:04}-{:02}-{:02}", date.year, date.month, date.day),
        time: format!("{:02}:{:02}", minutes / 60, minutes % 60),
    }
}

/// Fills a session's weather from a report: the barometer at the range,
/// and the wind brought down to shooting height. The wind's clock needs
/// the direction of fire, so it is left as it was.
pub fn apply_report(conditions: &mut Conditions, report: &Report) {
    conditions.temperature_c = Some(report.temperature_c);
    conditions.pressure_hpa = Some(Atmosphere::station_pressure(
        report.pressure_msl_hpa,
        report.elevation_m,
        report.temperature_c,
    ));
    conditions.humidity_percent = Some(report.humidity_percent);
    conditions.wind_speed_mps = Some(crate::weather::shooting_height_speed(report));
}

fn group_count(session: &Session, data: &Data) -> usize {
    data.groups
        .groups
        .iter()
        .filter(|group| group.session_id.as_deref() == Some(session.id.as_str()))
        .count()
}

fn groups_text(count: usize) -> String {
    match count {
        0 => tr("no groups").to_owned(),
        1 => tr("1 group").to_owned(),
        count => trf!("{} groups", count),
    }
}

impl SessionsPage {
    /// Views open over the list on a phone: the editor.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn depth(&self) -> usize {
        usize::from(self.editing.is_some())
    }

    /// Back to the list, dropping unsaved changes.
    pub fn back(&mut self) {
        self.editing = None;
    }

    /// The editor has changes that aren't saved.
    pub fn unsaved(&self, data: &Data) -> bool {
        let Some(session) = &self.editing else {
            return false;
        };
        let typed =
            self.when.session_id == session.id && when_of(session, data.country) != self.when;
        typed
            || match data.groups.sessions.iter().find(|s| s.id == session.id) {
                Some(stored) => stored != session,
                None => self.blank.as_ref() != Some(session),
            }
    }

    /// Keeps the editor on the session just saved.
    pub fn saved(&mut self, session: &Session) {
        if self
            .editing
            .as_ref()
            .is_some_and(|open| open.id == session.id)
        {
            self.editing = Some(session.clone());
        }
    }

    pub fn deleted(&mut self, id: &str) {
        if self.editing.as_ref().is_some_and(|open| open.id == id) {
            self.editing = None;
        }
    }

    /// Opens a session in the editor.
    pub fn open(&mut self, session: &Session, data: &Data) {
        self.when = when_of(session, data.country);
        self.editing = Some(session.clone());
    }

    fn new_session(&mut self, data: &Data) {
        let session = Session::new(now_ms());
        self.blank = Some(session.clone());
        self.open(&session, data);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &Data) -> Vec<Action> {
        let context = &ui.ctx().clone();
        let mut actions = Vec::new();
        if let Some(report) = self.weather.poll(context)
            && let Some(asked) = self.asked.take()
        {
            let from = asked.by_name.then(|| report.place.name.clone());
            match &mut self.editing {
                Some(session) if session.id == asked.session_id => {
                    apply_report(&mut session.conditions, &report);
                }
                _ => {
                    if let Some(stored) = data
                        .groups
                        .sessions
                        .iter()
                        .find(|s| s.id == asked.session_id)
                    {
                        let mut session = stored.clone();
                        apply_report(&mut session.conditions, &report);
                        actions.push(Action::Record(Box::new(Change::SaveSession(session))));
                    }
                }
            }
            self.filled = Some((asked.session_id, from));
        }
        if let Some(session) = &self.editing
            && self.when.session_id != session.id
        {
            self.when = when_of(session, data.country);
        }
        if mobile::is_narrow(context) {
            egui::CentralPanel::default().show(ui, |ui| {
                if self.editing.is_none() {
                    if mobile::back_button(ui, tr("Calculate")) {
                        actions.push(Action::Back);
                    }
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.list(ui, data, true));
                } else {
                    if mobile::back_button(ui, tr("Sessions")) {
                        actions.push(Action::Back);
                    }
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.editor(ui, data, &mut actions));
                }
            });
            return actions;
        }
        egui::Panel::left("session-list")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.list(ui, data, false));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.editing.is_some() {
                        self.editor(ui, data, &mut actions);
                    } else {
                        ui.weak(tr("Choose a session, or add a new one."));
                    }
                });
        });
        actions
    }

    fn list(&mut self, ui: &mut egui::Ui, data: &Data, phone: bool) {
        ui.heading(tr("Sessions"));
        ui.weak(tr(
            "Days at the range with their weather. Groups shot in a session take its weather \
             unless they have their own.",
        ));
        if ui.button(tr("+ New session")).clicked() {
            self.new_session(data);
        }
        ui.add_space(4.0);
        let u = units::prefs(ui.ctx());
        let local = |ms: i64| crate::groups::local_ms(ms, data.country);
        let today = local(now_ms()).div_euclid(DAY_MS);
        for session in &data.groups.sessions {
            let line = crate::groups::library::session_line(session, today, local, &u);
            let count = groups_text(group_count(session, data));
            let selected = self
                .editing
                .as_ref()
                .is_some_and(|open| open.id == session.id);
            let clicked = if phone {
                mobile::list_row(ui, &line, &count, selected).clicked()
            } else {
                ui.selectable_label(selected, format!("{line}\n{count}"))
                    .clicked()
            };
            if clicked {
                self.open(session, data);
            }
        }
        if data.groups.sessions.is_empty() {
            ui.weak(tr(
                "No sessions yet. Make one here, or when you save a group.",
            ));
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, data: &Data, actions: &mut Vec<Action>) {
        let context = ui.ctx().clone();
        let Some(session) = &mut self.editing else {
            return;
        };
        let stored = data.groups.sessions.iter().find(|s| s.id == session.id);
        ui.heading(if stored.is_some() {
            tr("Edit session")
        } else {
            tr("New session")
        });
        let offset = |ms: i64| crate::groups::local_ms(ms, data.country) - ms;
        let started = parse_when(&self.when.date, &self.when.time, offset);
        grid(ui, "session-editor", |ui| {
            text(ui, tr("Place"), &mut session.place);
            ui.label(tr("Date and time"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.when.date)
                        .hint_text("2026-11-21")
                        .desired_width(118.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.when.time)
                        .hint_text("14:30")
                        .desired_width(64.0),
                );
            });
            ui.end_row();
        });
        let retyped = when_of(session, data.country) != self.when;
        match started {
            Some(at) if retyped => session.started_at = at,
            Some(_) => {}
            None => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tr("Write the date as 2026-11-21 and the time as 14:30."),
                );
            }
        }
        ui.add_space(6.0);
        let local = |ms: i64| crate::groups::local_ms(ms, data.country).div_euclid(DAY_MS);
        let today = local(session.started_at) == local(now_ms());
        let session_id = session.id.clone();
        let mut asked = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Weather")).strong());
            if !today {
                return;
            }
            if crate::sensors::can_locate()
                && ui
                    .button(tr("📍 Fill in the weather"))
                    .on_hover_text(tr(
                        "The weather now where you are, from Open-Meteo (the browser asks \
                         for your position first)",
                    ))
                    .clicked()
            {
                self.weather.locate_here(&context);
                asked = Some(false);
            }
            let place = session.place.trim();
            if !place.is_empty()
                && ui
                    .button(trf!("Weather at {}", place))
                    .on_hover_text(tr("The weather there now, from Open-Meteo"))
                    .clicked()
            {
                self.weather.fetch_named(&context, place, data.country);
                asked = Some(true);
            }
            if let Some(busy) = self.weather.busy() {
                ui.spinner();
                ui.weak(busy);
            }
        });
        if let Some(by_name) = asked {
            self.asked = Some(Asked {
                session_id: session_id.clone(),
                by_name,
            });
        }
        if !today {
            ui.weak(tr(
                "The weather can be filled in only for a session today: it is the weather now.",
            ));
        } else if !crate::sensors::can_locate() && session.place.trim().is_empty() {
            ui.weak(tr("Type the place to fill in its weather now."));
        }
        if let Some((id, from)) = &self.filled
            && *id == session_id
        {
            ui.weak(match from {
                Some(place) => trf!("Filled in with the weather now at {}.", place),
                None => tr("Filled in with the weather now where you are.").to_owned(),
            });
        }
        if let Some(error) = self.weather.error() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        conditions_fields(ui, &mut session.conditions);
        ui.label(tr("Notes"));
        ui.text_edit_multiline(&mut session.notes);
        ui.add_space(6.0);

        let problem = session.validate().err().or_else(|| {
            started
                .is_none()
                .then(|| tr("the date or time can't be read").to_owned())
        });
        let changed = match stored {
            Some(stored) => stored != session,
            None => true,
        };
        let mut close = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    problem.is_none() && changed,
                    egui::Button::new(RichText::new(tr("Save")).strong()),
                )
                .clicked()
            {
                actions.push(Action::Record(Box::new(Change::SaveSession(
                    session.clone(),
                ))));
            }
            match stored {
                Some(stored) => {
                    if changed && ui.button(tr("Undo changes")).clicked() {
                        *session = stored.clone();
                        self.when = when_of(stored, data.country);
                    }
                    if confirm_button(ui, "delete-session", tr("Delete")) {
                        actions.push(Action::Record(Box::new(Change::DeleteSession(
                            stored.id.clone(),
                        ))));
                    }
                }
                None => {
                    if ui.button(tr("Cancel")).clicked() {
                        close = true;
                    }
                }
            }
        });
        if let Some(problem) = problem {
            ui.colored_label(ui.visuals().warn_fg_color, problem);
        } else if changed && stored.is_some() {
            ui.weak(tr("Unsaved changes"));
        }
        if stored.is_some() {
            ui.weak(tr(
                "Deleting the session keeps its groups: each keeps the session's weather as \
                 its own.",
            ));
        }
        if close {
            self.editing = None;
            return;
        }
        ui.add_space(10.0);
        groups(ui, session, data, actions);
    }
}

/// A checkbox whose touch target is at least 44 points on a phone.
fn known_box(ui: &mut egui::Ui, known: &mut bool, label: &str) -> bool {
    let side = if mobile::is_narrow(ui.ctx()) {
        44.0
    } else {
        ui.spacing().interact_size.y
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    if response.clicked() {
        *known = !*known;
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *known, label));
    let visuals = ui.style().interact_selectable(&response, *known);
    let icon = egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0));
    let painter = ui.painter();
    painter.rect(
        icon,
        visuals.corner_radius,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    if *known {
        let at = |x: f32, y: f32| icon.lerp_inside(egui::vec2(x, y));
        painter.add(egui::Shape::line(
            vec![at(0.22, 0.52), at(0.42, 0.72), at(0.78, 0.28)],
            egui::Stroke::new(2.0_f32, visuals.fg_stroke.color),
        ));
    }
    response.clicked()
}

/// A grid row for a value that may be unknown, switched on by a
/// [`known_box`].
fn optional_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    default: f64,
    field: impl FnOnce(&mut egui::Ui, &mut f64),
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut known = value.is_some();
        if known_box(ui, &mut known, label) {
            *value = known.then_some(default);
        }
        match value {
            Some(number) => field(ui, number),
            None => {
                ui.weak(tr("unknown"));
            }
        }
    });
    ui.end_row();
}

fn conditions_fields(ui: &mut egui::Ui, conditions: &mut Conditions) {
    let quantity = |quantity: Quantity, range: std::ops::RangeInclusive<f64>, speed: f64| {
        move |ui: &mut egui::Ui, value: &mut f64| {
            units::drag(ui, value, quantity, range, speed);
        }
    };
    grid(ui, "session-weather", |ui| {
        optional_row(
            ui,
            tr("Temperature"),
            &mut conditions.temperature_c,
            10.0,
            quantity(Quantity::Temperature, -60.0..=70.0, 0.5),
        );
        let air = conditions.temperature_c.unwrap_or(10.0);
        optional_row(
            ui,
            tr("Powder temperature"),
            &mut conditions.powder_temperature_c,
            air,
            quantity(Quantity::Temperature, -60.0..=70.0, 0.5),
        );
        optional_row(
            ui,
            tr("Barometer at the range"),
            &mut conditions.pressure_hpa,
            1000.0,
            quantity(Quantity::Pressure, 300.0..=1200.0, 1.0),
        );
        optional_row(
            ui,
            tr("Humidity"),
            &mut conditions.humidity_percent,
            50.0,
            |ui, humidity| {
                ui.add(
                    NumberField::new(humidity)
                        .range(0.0..=100.0)
                        .speed(1.0)
                        .suffix(" %"),
                );
            },
        );
        optional_row(
            ui,
            tr("Wind"),
            &mut conditions.wind_speed_mps,
            2.0,
            quantity(Quantity::WindSpeed, 0.0..=100.0, 0.1),
        );
        optional_row(
            ui,
            tr("Wind from"),
            &mut conditions.wind_clock,
            3.0,
            |ui, clock| {
                ui.add(NumberField::new(clock).range(0.0..=12.0).speed(0.5));
                ui.weak(tr("o'clock: 12 head-on, 3 from the right"));
            },
        );
    });
}

/// The session's groups; a tap opens one in Groups.
fn groups(ui: &mut egui::Ui, session: &Session, data: &Data, actions: &mut Vec<Action>) {
    let groups: Vec<_> = data
        .groups
        .groups
        .iter()
        .filter(|group| group.session_id.as_deref() == Some(session.id.as_str()))
        .collect();
    ui.label(RichText::new(trf!("Groups ({})", groups.len())).strong());
    if groups.is_empty() {
        ui.weak(tr(
            "No groups in this session yet: choose it when you save a group.",
        ));
    }
    let u = units::prefs(ui.ctx());
    let mut summaries = crate::groups::Summaries::default();
    for group in groups {
        let summary = summaries.of(group, data, &u);
        if mobile::list_row(ui, &summary.title, &summary.subtitle, false).clicked() {
            actions.push(Action::OpenGroup(group.id.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_date_and_time_is_read_on_the_local_clock() {
        let utc = |_| 0;
        let at = parse_when("2026-11-21", "14:30", utc).unwrap();
        let date = ballistics_store::sun::Date::from_days(at / DAY_MS);
        assert_eq!((date.year, date.month, date.day), (2026, 11, 21));
        assert_eq!(at % DAY_MS, (14 * 60 + 30) * MINUTE_MS);
        let stockholm = |_| 60 * MINUTE_MS;
        assert_eq!(
            parse_when("2026-11-21", "14.30", stockholm),
            Some(at - 60 * MINUTE_MS)
        );
        assert_eq!(parse_when("2026-02-30", "10:00", utc), None);
        assert_eq!(parse_when("21/11/2026", "10:00", utc), None);
        assert_eq!(parse_when("2026-11-21", "25:00", utc), None);
    }
}

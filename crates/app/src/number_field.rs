//! Number fields: egui's `DragValue` made safe for what hunters type and for
//! phones.
//!
//! - A decimal comma counts: "3,8" is 3.8, not 3.
//! - A typed unit is converted: "140 gr" in a gram field is 9.07 g.
//! - Text it can't read, or a number outside the field's range, keeps the
//!   value and says why, instead of clamping quietly. So does Escape,
//!   without a word.
//! - A value outside the range is shown as it is, not quietly changed.
//! - On a touch screen the field takes taps only, so a swipe that starts on
//!   it scrolls the page instead of changing the number.

use std::cell::RefCell;
use std::ops::RangeInclusive;

use eframe::egui::{self, RichText, TextWrapMode};

use crate::i18n::trf;

/// How long a note about the typed text stays up.
const NOTE_S: f64 = 4.0;

/// A unit the hunter may type besides the field's own:
/// `field = typed × scale + offset`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedUnit {
    pub labels: Vec<&'static str>,
    pub scale: f64,
    pub offset: f64,
}

/// Use like `egui::DragValue`.
pub struct NumberField<'a> {
    get_set: Box<dyn 'a + FnMut(Option<f64>) -> f64>,
    range: RangeInclusive<f64>,
    speed: f64,
    prefix: String,
    suffix: String,
    min_decimals: usize,
    max_decimals: Option<usize>,
    units: Vec<TypedUnit>,
}

impl<'a> NumberField<'a> {
    pub fn new<Num: egui::emath::Numeric>(value: &'a mut Num) -> Self {
        let field = NumberField {
            get_set: Box::new(move |new: Option<f64>| {
                if let Some(new) = new {
                    *value = Num::from_f64(new);
                }
                value.to_f64()
            }),
            range: f64::NEG_INFINITY..=f64::INFINITY,
            speed: 1.0,
            prefix: String::new(),
            suffix: String::new(),
            min_decimals: 0,
            max_decimals: None,
            units: Vec::new(),
        };
        if Num::INTEGRAL {
            field.max_decimals(0).range(Num::MIN..=Num::MAX).speed(0.25)
        } else {
            field
        }
    }

    pub fn range<Num: egui::emath::Numeric>(mut self, range: RangeInclusive<Num>) -> Self {
        self.range = range.start().to_f64()..=range.end().to_f64();
        self
    }

    pub fn speed(mut self, speed: impl Into<f64>) -> Self {
        self.speed = speed.into();
        self
    }

    pub fn prefix(mut self, prefix: impl ToString) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    pub fn suffix(mut self, suffix: impl ToString) -> Self {
        self.suffix = suffix.to_string();
        self
    }

    pub fn max_decimals(mut self, decimals: usize) -> Self {
        self.max_decimals = Some(decimals);
        self
    }

    pub fn fixed_decimals(mut self, decimals: usize) -> Self {
        self.min_decimals = decimals;
        self.max_decimals = Some(decimals);
        self
    }

    /// Other units the hunter may type, converted to the field's.
    pub fn units(mut self, units: Vec<TypedUnit>) -> Self {
        self.units = units;
        self
    }
}

/// What the typed text says.
#[derive(Clone, Debug, PartialEq)]
enum Reading {
    /// A number in the field's unit, and the unit it was typed in when that
    /// was another.
    Number(f64, Option<&'static str>),
    Unreadable,
}

impl egui::Widget for NumberField<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let NumberField {
            get_set,
            range,
            speed,
            prefix,
            suffix,
            min_decimals,
            max_decimals,
            units,
        } = self;
        let get_set = RefCell::new(get_set);
        let value = (get_set.borrow_mut())(None);
        // The `DragValue`, and the button standing in for it, take this id.
        let id = ui.next_auto_id();
        let note_id = id.with("number-field-note");
        let editing_id = id.with("number-field-editing");
        let escaped_id = id.with("number-field-escaped");
        // egui reads the text once more the frame after an edit ends, even
        // when it ended with Escape: refuse it then.
        let escaped = ui
            .data_mut(|data| data.remove_temp::<bool>(escaped_id))
            .is_some();
        let focused = ui.memory(|memory| memory.has_focus(id));
        // For two frames after an edit the `DragValue` itself is drawn, so it
        // clears what it kept of the edit: egui does that in the frame after
        // focus is lost, and on a phone Escape can take the focus away before
        // the field is drawn, which puts that one frame later.
        let frames_since = ui.data(|data| data.get_temp::<u8>(editing_id));
        let just_edited = !focused && frames_since.is_some();
        if focused {
            ui.data_mut(|data| data.insert_temp(editing_id, 2_u8));
        } else if let Some(left) = frames_since {
            ui.data_mut(|data| {
                if left > 1 {
                    data.insert_temp(editing_id, left - 1);
                } else {
                    data.remove::<u8>(editing_id);
                }
            });
        }
        let (low, high) = (
            range.start().min(*range.end()),
            range.start().max(*range.end()),
        );
        let decimals = decimals(ui, speed, min_decimals, max_decimals);
        let number_formatter = ui.style().number_formatter.clone();
        let format = |number: f64| number_formatter.format(number, decimals.clone());
        let shown = |number: f64| format!("{prefix}{}{suffix}", format(number));

        let touch = ui.input(egui::InputState::has_touch_screen);
        let mut response = if touch && !focused && !just_edited {
            // Taps only: a swipe starting here goes to the page.
            let response = ui.add(
                egui::Button::new(
                    RichText::new(shown(value))
                        .text_style(ui.style().drag_value_text_style.clone()),
                )
                .wrap_mode(TextWrapMode::Extend)
                .min_size(ui.spacing().interact_size),
            );
            if response.clicked() {
                // As the `DragValue` does on a click: edit, all selected.
                ui.memory_mut(|memory| memory.request_focus(id));
                let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::select_all_str(&format(
                        value,
                    ))));
                state.store(ui.ctx(), id);
            }
            response
        } else {
            // The text is read when the edit ends (Enter, a tap elsewhere),
            // not while typing; Escape and text it can't take keep the value.
            let typed = RefCell::new(None);
            let mut drag = egui::DragValue::from_get_set(|new| (get_set.borrow_mut())(new))
                .range(range)
                .clamp_existing_to_range(false)
                .update_while_editing(false)
                .speed(speed)
                .min_decimals(min_decimals)
                .prefix(prefix.clone())
                .suffix(suffix.clone())
                .custom_parser(|text| {
                    if escaped {
                        return None;
                    }
                    let reading = read(text, &prefix, &suffix, &units);
                    let accepted = match reading {
                        Reading::Number(number, _) => {
                            (low..=high).contains(&number).then_some(number)
                        }
                        Reading::Unreadable => None,
                    };
                    *typed.borrow_mut() = Some((text.trim().to_owned(), reading));
                    accepted
                });
            if let Some(max) = max_decimals {
                drag = drag.max_decimals(max);
            }
            let response = ui.add(drag);
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                ui.data_mut(|data| data.insert_temp(escaped_id, true));
            }
            let note = match typed.into_inner() {
                // An emptied field keeps its value without a word.
                Some((text, _)) if text.is_empty() => None,
                Some((text, Reading::Unreadable)) => Some(trf!(
                    "Couldn't read “{text}”; kept {old}",
                    text = text,
                    old = shown(value)
                )),
                Some((_, Reading::Number(number, _))) if !(low..=high).contains(&number) => {
                    Some(trf!(
                        "Must be {low}–{high}; kept {old}",
                        low = shown(low),
                        high = shown(high),
                        old = shown(value)
                    ))
                }
                Some((text, Reading::Number(number, Some(_)))) => {
                    Some(format!("{text} = {}", shown(number)))
                }
                _ => None,
            };
            if let Some(note) = note {
                let until = ui.input(|input| input.time) + NOTE_S;
                ui.data_mut(|data| data.insert_temp(note_id, (note, until)));
            }
            response
        };

        if let Some((note, until)) = ui.data(|data| data.get_temp::<(String, f64)>(note_id)) {
            let now = ui.input(|input| input.time);
            if now < until {
                response.show_tooltip_text(note);
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(until - now));
            } else {
                ui.data_mut(|data| data.remove::<(String, f64)>(note_id));
            }
        }
        if (get_set.borrow_mut())(None) != value {
            response.mark_changed();
        }
        response
    }
}

/// The decimals egui's `DragValue` shows for these settings, so the button
/// standing in for it on touch screens looks the same.
fn decimals(
    ui: &egui::Ui,
    speed: f64,
    min_decimals: usize,
    max_decimals: Option<usize>,
) -> RangeInclusive<usize> {
    let aim_radius = f64::from(ui.input(egui::InputState::aim_radius));
    let auto = (aim_radius / speed.abs()).log10().ceil().clamp(0.0, 15.0) as usize;
    let max = max_decimals.unwrap_or(auto + 2).max(min_decimals);
    auto.clamp(min_decimals, max)..=max
}

/// Reads a typed number: spaces anywhere, a decimal comma or point, the
/// field's own prefix and suffix, and a unit of `units` after the number.
/// A twist may be typed "1:11".
fn read(text: &str, prefix: &str, suffix: &str, units: &[TypedUnit]) -> Reading {
    let squeeze = |text: &str| -> String {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| if c == '−' { '-' } else { c })
            .collect::<String>()
            .to_lowercase()
    };
    let text = squeeze(text);
    let text = text.strip_prefix(&squeeze(prefix)).unwrap_or(&text);
    let text = text.strip_prefix("1:").unwrap_or(text);
    let end = number_end(text);
    let (number, unit) = text.split_at(end);
    let Some(number) = decimal(number) else {
        return Reading::Unreadable;
    };
    let Some(number) = number
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
    else {
        return Reading::Unreadable;
    };
    if unit.is_empty() || unit == squeeze(suffix) {
        return Reading::Number(number, None);
    }
    units
        .iter()
        .find_map(|typed| {
            typed
                .labels
                .iter()
                .find(|label| squeeze(label) == unit)
                .map(|label| {
                    // The field's own unit, spelled another way, isn't a conversion.
                    let same = typed.scale == 1.0 && typed.offset == 0.0;
                    Reading::Number(
                        number * typed.scale + typed.offset,
                        (!same).then_some(*label),
                    )
                })
        })
        .unwrap_or(Reading::Unreadable)
}

/// A number with a decimal comma or point as Rust reads it. With both, the
/// first must group thousands: "1,234.5" or "1.234,5".
fn decimal(number: &str) -> Option<String> {
    let has = |c: char| number.contains(c);
    if !(has(',') && has('.')) {
        return Some(number.replace(',', "."));
    }
    let (group, point) = if number.find(',') < number.find('.') {
        (',', '.')
    } else {
        ('.', ',')
    };
    let (whole, fraction) = number.split_once(point)?;
    let unsigned = whole.trim_start_matches(['-', '+']);
    let mut groups = unsigned.split(group);
    let first = groups.next()?;
    let grouped = (1..=3).contains(&first.len())
        && groups.all(|thousands| thousands.len() == 3)
        && !fraction.contains([',', '.']);
    grouped.then(|| format!("{}.{fraction}", whole.replace(group, "")))
}

/// Where the number at the start of `text` ends: a sign, digits, a decimal
/// comma or point, and an exponent ("1e5").
fn number_end(text: &str) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let digit_at = |index: usize| chars.get(index).is_some_and(|(_, c)| c.is_ascii_digit());
    let mut index = 0;
    if matches!(chars.first(), Some((_, '-' | '+'))) {
        index = 1;
    }
    while let Some(&(_, c)) = chars.get(index) {
        let exponent = matches!(c, 'e' | 'E')
            && index > 0
            && digit_at(index - 1)
            && (digit_at(index + 1)
                || matches!(chars.get(index + 1), Some((_, '-' | '+'))) && digit_at(index + 2));
        if c.is_ascii_digit() || c == '.' || c == ',' {
            index += 1;
        } else if exponent {
            index += 2;
        } else {
            break;
        }
    }
    chars.get(index).map_or(text.len(), |(byte, _)| *byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame with a field over `value`; returns where the field is.
    fn frame(context: &egui::Context, value: &mut f64, events: Vec<egui::Event>) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        let mut output = context.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                rect = ui
                    .add(NumberField::new(value).range(1.0..=100.0).suffix(" g"))
                    .rect;
            },
        );
        output.textures_delta.clear();
        rect
    }

    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![egui::Event::PointerMoved(at), button(true), button(false)]
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Clicks the field, types `text` over its value and ends with `end`.
    fn type_in(value: &mut f64, text: &str, end: egui::Key) {
        let context = egui::Context::default();
        let rect = frame(&context, value, Vec::new());
        frame(&context, value, click(rect.center()));
        frame(&context, value, vec![egui::Event::Text(text.to_owned())]);
        frame(&context, value, vec![key(end)]);
        for _ in 0..3 {
            frame(&context, value, Vec::new());
        }
    }

    #[test]
    fn typing_commits_on_enter_and_keeps_the_value_otherwise() {
        let mut value = 11.7;
        type_in(&mut value, "3,8", egui::Key::Enter);
        assert_eq!(value, 3.8);
        type_in(&mut value, "abc", egui::Key::Enter);
        assert_eq!(value, 3.8, "unreadable text keeps the value");
        type_in(&mut value, "500", egui::Key::Enter);
        assert_eq!(value, 3.8, "out of range keeps the value");
        type_in(&mut value, "50", egui::Key::Escape);
        assert_eq!(value, 3.8, "Escape keeps the value");
    }

    #[test]
    fn an_edit_left_without_ending_doesnt_reach_the_next_record() {
        let context = egui::Context::default();
        let mut first = 20.0;
        let rect = frame(&context, &mut first, Vec::new());
        frame(&context, &mut first, click(rect.center()));
        frame(
            &context,
            &mut first,
            vec![egui::Event::Text("abc".to_owned())],
        );
        // The editor switches to another record while the field has focus.
        let mut second = 30.0;
        frame(&context, &mut second, click(egui::pos2(500.0, 500.0)));
        for _ in 0..3 {
            frame(&context, &mut second, Vec::new());
        }
        assert_eq!(first, 20.0);
        assert_eq!(second, 30.0);
    }

    fn grains() -> Vec<TypedUnit> {
        vec![TypedUnit {
            labels: vec!["gr", "grain", "grains"],
            scale: 1.0 / 15.432_358,
            offset: 0.0,
        }]
    }

    fn number(text: &str) -> Option<f64> {
        match read(text, "", " g", &grains()) {
            Reading::Number(number, _) => Some(number),
            Reading::Unreadable => None,
        }
    }

    #[test]
    fn reads_decimal_commas_and_points() {
        assert_eq!(number("3,8"), Some(3.8));
        assert_eq!(number("3.8"), Some(3.8));
        assert_eq!(number(" -3,5 "), Some(-3.5));
        assert_eq!(number("−2"), Some(-2.0));
        assert_eq!(number("1 000"), Some(1000.0));
        assert_eq!(number("1,234.5"), Some(1234.5));
        assert_eq!(number("1.234,5"), Some(1234.5));
        assert_eq!(number("-12,345,678.25"), Some(-12_345_678.25));
        assert_eq!(number(",5"), Some(0.5));
        assert_eq!(number("1e3"), Some(1000.0));
        assert_eq!(number("2.5e-1"), Some(0.25));
    }

    #[test]
    fn reads_the_fields_own_unit_and_converts_others() {
        assert_eq!(number("11,7 g"), Some(11.7));
        assert_eq!(number("11.7G"), Some(11.7));
        let grains = read("180 gr", "", " g", &grains());
        let Reading::Number(grams, Some("gr")) = grains else {
            panic!("{grains:?}");
        };
        assert!((grams - 11.664).abs() < 0.001, "{grams}");
        assert_eq!(read("BC 0,45", "BC ", "", &[]), Reading::Number(0.45, None));
        assert_eq!(read("1:11", "", " in", &[]), Reading::Number(11.0, None));
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        for text in [
            "", "abc", "1,2,3", ".", ",", "--5", "5 kg", "NaN", "12x", "4.53,8", "1,23.4", "1e999",
        ] {
            assert_eq!(number(text), None, "{text:?}");
        }
    }

    #[test]
    fn converts_between_the_apps_units() {
        use crate::units::Quantity;
        let field = |quantity: Quantity, label: &str| {
            let unit = *quantity
                .units()
                .iter()
                .find(|unit| unit.label == label)
                .unwrap();
            let units = quantity.typed_units(unit);
            let suffix = format!(" {label}");
            move |text: &str| read(text, "", &suffix, &units)
        };
        let close = |reading: Reading, expected: f64, unit: Option<&str>| match reading {
            Reading::Number(number, typed) => {
                assert!((number - expected).abs() < 0.01, "{number} vs {expected}");
                assert_eq!(typed, unit);
            }
            Reading::Unreadable => panic!("unreadable, expected {expected}"),
        };
        let celsius = field(Quantity::Temperature, "°C");
        close(celsius("50 °F"), 10.0, Some("°F"));
        close(celsius("-3,5"), -3.5, None);
        close(celsius("-3,5 c"), -3.5, None);
        let fahrenheit = field(Quantity::Temperature, "°F");
        close(fahrenheit("10C"), 50.0, Some("C"));
        let grams = field(Quantity::BulletMass, "g");
        close(grams("140 gr"), 9.072, Some("gr"));
        let velocity = field(Quantity::Speed, "m/s");
        close(velocity("2490 fps"), 758.95, Some("fps"));
        let sight = field(Quantity::SightHeight, "cm");
        close(sight("1,5\""), 3.81, Some("\""));
        close(sight("38 mm"), 3.8, Some("mm"));
        let inches = field(Quantity::SightHeight, "in");
        close(inches("1.5\""), 1.5, None);
        let twist = field(Quantity::Twist, "in");
        close(twist("1:11"), 11.0, None);
        close(twist("1:280 mm"), 11.02, Some("mm"));
        assert_eq!(grams("5 kg"), Reading::Unreadable);
    }
}

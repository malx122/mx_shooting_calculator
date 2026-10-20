//! The turret label: a strip wrapped round the scope's elevation turret,
//! with the ranges marked at the clicks they need, a band for each turn of
//! the turret, and a ruler of the clicks underneath. The marks come from
//! the calculation; only the turret's size is measured by hand.

use std::f32::consts::PI;

use ballistics_shot::Hold;
use serde::{Deserialize, Serialize};

use super::Subject;
use super::sheet::{Align, Ink, PT, Sheet, arc, fit, text_width};
use crate::units::{Quantity, UnitPrefs};

/// A rifle's elevation turret, as measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Turret {
    pub diameter_mm: f64,
    /// The height of the band the label covers.
    pub height_mm: f64,
    pub clicks_per_turn: u32,
    /// Dialling up turns the turret clockwise, seen from above. Most turn
    /// anticlockwise.
    pub clockwise: bool,
}

impl Default for Turret {
    fn default() -> Self {
        Turret {
            diameter_mm: 30.0,
            height_mm: 12.0,
            clicks_per_turn: 100,
            clockwise: false,
        }
    }
}

impl Turret {
    /// A starting point for a rifle's clicks: 10 mil a turn for 0.1 mil
    /// clicks, 15 MOA for ¼ MOA.
    pub fn for_clicks(click_value: f64, mil: bool) -> Turret {
        let per_turn = if mil { 10.0 } else { 15.0 };
        Turret {
            clicks_per_turn: ((per_turn / click_value).round() as u32).clamp(10, 400),
            ..Turret::default()
        }
    }

    pub fn circumference_mm(&self) -> f32 {
        PI * self.diameter_mm as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numbers {
    /// "1 2 3" for 100, 200, 300.
    Hundreds,
    /// "100 200 300".
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TurretOptions {
    /// The last range marked, metres.
    pub end_m: f64,
    /// A number every so many (in the hunter's distance unit).
    pub numbers_every: f64,
    /// A mark every so many between the numbers; 0 for none.
    pub marks_every: f64,
    pub numbers: Numbers,
    /// The air (and cartridge) temperature; `None` for today's.
    pub temperature_c: Option<f64>,
    /// Leave out the ranges beyond the ethical limit instead of marking
    /// them red.
    pub stop_at_limit: bool,
    pub ruler: bool,
    pub description: bool,
    /// White on black instead of black on white.
    pub dark: bool,
    pub copies: u32,
}

impl Default for TurretOptions {
    fn default() -> Self {
        TurretOptions {
            end_m: 500.0,
            numbers_every: 100.0,
            marks_every: 25.0,
            numbers: Numbers::Hundreds,
            temperature_c: None,
            stop_at_limit: false,
            ruler: true,
            description: true,
            dark: false,
            copies: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Number,
    Line,
    Dot,
}

/// One range on the label.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeMark {
    pub range_m: f64,
    pub clicks: i64,
    pub kind: Kind,
    pub text: String,
    pub beyond: bool,
    /// Left off: too close to a more important mark.
    pub crowded: bool,
}

/// The label's marks and what it found on the way.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub marks: Vec<RangeMark>,
    pub turns: u32,
    pub ethical_m: Option<f64>,
    pub temperature_c: f64,
    pub muzzle_velocity: f64,
    /// Ranges that need more clicks than the scope has.
    pub out_of_travel: Vec<f64>,
}

impl Plan {
    pub fn compute(
        subject: &Subject,
        turret: &Turret,
        options: &TurretOptions,
        u: &UnitPrefs,
    ) -> Result<Plan, String> {
        let distance = u.unit(Quantity::Distance);
        let temperature_c = options
            .temperature_c
            .unwrap_or(subject.conditions.temperature_c);
        let solution = subject
            .solve(temperature_c, 0.0, options.end_m.max(1200.0) + 10.0)
            .map_err(|error| error.to_string())?;
        let ethical_m = subject.ethical_limit(&solution);
        let rifle = &subject.rifle;
        let numbers_every = options.numbers_every.max(1.0);
        let step = if options.marks_every > 0.0 {
            options.marks_every.min(numbers_every)
        } else {
            numbers_every
        };
        // From the zero (nothing to dial below it) to the last range, in
        // the hunter's unit.
        let first = (distance.show(rifle.zero_range_m) / step).ceil() * step;
        let last = distance.show(options.end_m);
        let mut marks = Vec::new();
        let mut out_of_travel = Vec::new();
        let mut shown = first;
        while shown <= last + 1e-6 {
            let range_m = distance.base(shown);
            let beyond = ethical_m.is_some_and(|limit| range_m > limit + 1e-6);
            if beyond && options.stop_at_limit {
                break;
            }
            let Some(row) = solution.at(range_m) else {
                break;
            };
            let clicks = Hold::new(rifle, &row).elevation_clicks.max(0);
            if subject
                .elevation_travel_clicks
                .is_some_and(|travel| clicks as f64 > travel)
            {
                out_of_travel.push(range_m);
            } else {
                let on = |every: f64| ((shown / every).round() * every - shown).abs() < 1e-6;
                let kind = if on(numbers_every) {
                    Kind::Number
                } else if on(numbers_every / 2.0) {
                    Kind::Line
                } else {
                    Kind::Dot
                };
                let text = match (kind, options.numbers) {
                    (Kind::Number, Numbers::Full) => format!("{shown:.0}"),
                    (Kind::Number, Numbers::Hundreds) => {
                        let hundreds = shown / 100.0;
                        if (hundreds - hundreds.round()).abs() < 1e-6 {
                            format!("{hundreds:.0}")
                        } else {
                            format!("{hundreds:.1}")
                        }
                    }
                    _ => String::new(),
                };
                marks.push(RangeMark {
                    range_m,
                    clicks,
                    kind,
                    text,
                    beyond,
                    crowded: false,
                });
            }
            shown += step;
        }
        let per_turn = i64::from(turret.clicks_per_turn.max(1));
        let turns = marks
            .iter()
            .map(|mark| (mark.clicks / per_turn) as u32 + 1)
            .max()
            .unwrap_or(1);
        Ok(Plan {
            marks,
            turns,
            ethical_m,
            temperature_c,
            muzzle_velocity: solution.muzzle_velocity,
            out_of_travel,
        })
    }
}

/// The strip's colours: background, marks, and marks beyond the limit.
fn colours(options: &TurretOptions) -> (Ink, Ink, Ink) {
    if options.dark {
        (Ink::Black, Ink::White, Ink::Red)
    } else {
        (Ink::White, Ink::Black, Ink::Red)
    }
}

/// The strip itself, `circumference × height`, as it is cut out. Marks too
/// close to a more important one are left off (and flagged in `plan`).
pub fn strip(
    subject: &Subject,
    turret: &Turret,
    options: &TurretOptions,
    plan: &mut Plan,
    u: &UnitPrefs,
) -> Sheet {
    let width = turret.circumference_mm();
    let height = turret.height_mm as f32;
    let (paper, ink, red) = colours(options);
    let mut sheet = Sheet::new(width, height);
    sheet.rect(0.0, 0.0, width, height, paper);

    let per_turn = turret.clicks_per_turn.max(1) as f32;
    // Click 0 sits a little in from the end the numbers grow away from;
    // the seam falls just past the last click of the turn.
    let zero_inset = 3.0_f32.min(width * 0.05);
    let x_of = |clicks: f32| {
        let along = (clicks.rem_euclid(per_turn)) / per_turn * width;
        if turret.clockwise {
            let x = zero_inset + along;
            if x > width { x - width } else { x }
        } else {
            let x = width - zero_inset - along;
            if x < 0.0 { x + width } else { x }
        }
    };

    let description_height = if options.description {
        (height * 0.17).clamp(1.8, 3.5)
    } else {
        0.0
    };
    let ruler_height = if options.ruler {
        (height * 0.3).clamp(2.5, 6.0)
    } else {
        0.0
    };
    let bands_top = description_height;
    let bands_bottom = height - ruler_height;
    let band_height = (bands_bottom - bands_top) / plan.turns as f32;

    if options.description {
        let size = description_height * 0.72 / PT;
        let rifle = &subject.rifle;
        let text = format!(
            "{} · {} · {} · zero {}",
            subject.bullet.display_name(),
            u.format(Quantity::Speed, plan.muzzle_velocity),
            u.format(Quantity::Temperature, plan.temperature_c),
            u.format(Quantity::Distance, rifle.zero_range_m),
        );
        sheet.text(
            [width / 2.0, description_height * 0.78],
            fit(&text, size, false, width - 2.0),
            size,
            false,
            Align::Center,
            ink,
        );
        sheet.line([0.0, bands_top], [width, bands_top], 0.12, ink);
    }

    // The range marks, a band for each turn, the first turn lowest.
    let number_size = band_height * 0.66 / PT;
    let gap = band_height * 0.12;
    let mut taken: Vec<Vec<(f32, f32)>> = vec![Vec::new(); plan.turns as usize];
    let mut order: Vec<usize> = (0..plan.marks.len()).collect();
    order.sort_by_key(|&index| match plan.marks[index].kind {
        Kind::Number => 0,
        Kind::Line => 1,
        Kind::Dot => 2,
    });
    for index in order {
        let mark = &mut plan.marks[index];
        let turn = (mark.clicks / per_turn as i64) as usize;
        let x = x_of(mark.clicks as f32);
        let half = match mark.kind {
            // Numbers may sit close; the small marks keep their distance.
            Kind::Number => text_width(&mark.text, number_size, true) / 2.0 + 0.15,
            Kind::Line => 0.35 + gap,
            Kind::Dot => band_height * 0.08 + gap,
        };
        let span = (x - half, x + half);
        if taken[turn]
            .iter()
            .any(|other| span.0 < other.1 && other.0 < span.1)
        {
            mark.crowded = true;
            continue;
        }
        taken[turn].push(span);
        let bottom = bands_bottom - turn as f32 * band_height;
        let middle = bottom - band_height / 2.0;
        let colour = if mark.beyond { red } else { ink };
        match mark.kind {
            Kind::Number => sheet.text(
                [x, middle + number_size * PT * 0.36],
                &mark.text,
                number_size,
                true,
                Align::Center,
                colour,
            ),
            Kind::Line => sheet.line(
                [x, middle - band_height * 0.25],
                [x, middle + band_height * 0.25],
                0.35,
                colour,
            ),
            Kind::Dot => sheet.fill(arc([x, middle], band_height * 0.08, 0.0, 360.0), colour),
        }
    }
    for turn in 1..plan.turns {
        let y = bands_bottom - turn as f32 * band_height;
        sheet.line([0.0, y], [width, y], 0.12, ink);
    }

    // The ruler: a tick for each click rising from the bottom edge, longer
    // at each mil (or MOA) and half, numbered at each whole.
    if options.ruler {
        let rifle = &subject.rifle;
        let per_unit = (1.0 / rifle.click_value).round().max(1.0) as u32;
        let spacing = width / per_turn;
        let every = if spacing < 0.45 { 2 } else { 1 };
        sheet.line([0.0, bands_bottom], [width, bands_bottom], 0.2, ink);
        let size = ruler_height * 0.36 / PT;
        for click in 0..turret.clicks_per_turn {
            let whole = click % per_unit == 0;
            let half = per_unit.is_multiple_of(2) && click % per_unit == per_unit / 2;
            if !whole && !half && click % every != 0 {
                continue;
            }
            let x = x_of(click as f32);
            let length = ruler_height
                * if whole {
                    0.45
                } else if half {
                    0.32
                } else {
                    0.2
                };
            sheet.line(
                [x, height],
                [x, height - length],
                if whole { 0.22 } else { 0.12 },
                ink,
            );
            if whole {
                sheet.text(
                    [x, height - ruler_height * 0.55],
                    format!("{}", click / per_unit),
                    size,
                    false,
                    Align::Center,
                    ink,
                );
            }
        }
    }
    sheet
}

/// The A4 page: copies of the strip with cut marks, a bar to check the
/// print scale, and the clicks for each range.
pub fn page(
    subject: &Subject,
    turret: &Turret,
    options: &TurretOptions,
    plan: &Plan,
    strip: &Sheet,
    u: &UnitPrefs,
) -> Sheet {
    let mut page = Sheet::new(210.0, 297.0);
    let margin = 15.0;
    let mut y = margin;
    page.text(
        [margin, y + 5.0],
        format!("Turret label · {}", subject.rifle.name),
        14.0,
        true,
        Align::Left,
        Ink::Black,
    );
    y += 9.0;
    page.text(
        [margin, y + 3.0],
        format!(
            "{} · {} · zero {} · turret {:.1} mm across, {} clicks a turn, dialled {}",
            subject.bullet.display_name(),
            u.format(Quantity::Temperature, plan.temperature_c),
            u.format(Quantity::Distance, subject.rifle.zero_range_m),
            turret.diameter_mm,
            turret.clicks_per_turn,
            if turret.clockwise {
                "clockwise"
            } else {
                "anticlockwise"
            }
        ),
        8.0,
        false,
        Align::Left,
        Ink::Grey,
    );
    y += 10.0;
    // Copies of the strip, each with cut marks at its corners.
    let left = (210.0 - strip.width) / 2.0;
    for _ in 0..options.copies.clamp(1, 8) {
        page.place(strip, [left, y]);
        let (right, bottom) = (left + strip.width, y + strip.height);
        for (x, y, dx, dy) in [
            (left, y, -1.0, -1.0),
            (right, y, 1.0, -1.0),
            (left, bottom, -1.0, 1.0),
            (right, bottom, 1.0, 1.0),
        ] {
            page.line([x + dx * 1.0, y], [x + dx * 5.0, y], 0.15, Ink::Black);
            page.line([x, y + dy * 1.0], [x, y + dy * 5.0], 0.15, Ink::Black);
        }
        y += strip.height + 10.0;
    }

    // A bar to check the print is true to size.
    page.line([margin, y], [margin + 50.0, y], 0.3, Ink::Black);
    for x in [margin, margin + 50.0] {
        page.line([x, y - 2.0], [x, y + 2.0], 0.3, Ink::Black);
    }
    page.text(
        [margin + 55.0, y + 1.2],
        "This bar is 50 mm. If not, print at actual size (100 %, no fitting to the page).",
        8.0,
        false,
        Align::Left,
        Ink::Black,
    );
    y += 9.0;
    let steps = [
        "Cut on the corner marks. The strip is as long as the turret is round.",
        "Set the turret to zero. Wrap the strip with the ruler's 0 at the index line and the numbers",
        "running the way the turret turns when dialling up. Clear tape over it keeps it dry.",
    ];
    for text in steps {
        page.text([margin, y], text, 8.0, false, Align::Left, Ink::Black);
        y += 4.2;
    }
    y += 5.0;

    // The clicks for each range, as on the label.
    page.text([margin, y], "Range", 8.0, true, Align::Left, Ink::Black);
    page.text(
        [margin + 30.0, y],
        "Clicks",
        8.0,
        true,
        Align::Right,
        Ink::Black,
    );
    page.text(
        [margin + 48.0, y],
        "Turn",
        8.0,
        true,
        Align::Right,
        Ink::Black,
    );
    y += 1.5;
    page.line([margin, y], [margin + 90.0, y], 0.2, Ink::Black);
    y += 4.0;
    let per_turn = i64::from(turret.clicks_per_turn.max(1));
    let column_top = y;
    let mut column = 0.0;
    for mark in &plan.marks {
        if y > 297.0 - margin {
            column += 95.0;
            y = column_top;
        }
        let ink = if mark.beyond { Ink::Red } else { Ink::Black };
        page.text(
            [margin + column, y],
            u.format(Quantity::Distance, mark.range_m),
            8.0,
            false,
            Align::Left,
            ink,
        );
        page.text(
            [margin + column + 30.0, y],
            mark.clicks.to_string(),
            8.0,
            false,
            Align::Right,
            ink,
        );
        page.text(
            [margin + column + 48.0, y],
            (mark.clicks / per_turn + 1).to_string(),
            8.0,
            false,
            Align::Right,
            ink,
        );
        if mark.crowded {
            page.text(
                [margin + column + 52.0, y],
                "not on the label (crowded)",
                7.0,
                false,
                Align::Left,
                Ink::Grey,
            );
        }
        y += 4.0;
    }
    if let (Some(limit), Some(target)) = (plan.ethical_m, &subject.target) {
        y += 3.0;
        page.text(
            [margin + column, y],
            format!(
                "Red: beyond the ethical limit for {} ({}).",
                target.to_lowercase(),
                u.format(Quantity::Distance, limit)
            ),
            8.0,
            false,
            Align::Left,
            Ink::Red,
        );
    }
    page.text(
        [105.0, 297.0 - 6.0],
        format!("{} · {}", crate::APP_NAME, super::today()),
        7.0,
        false,
        Align::Center,
        Ink::Grey,
    );
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_starts_from_the_rifles_clicks() {
        assert_eq!(Turret::for_clicks(0.1, true).clicks_per_turn, 100);
        assert_eq!(Turret::for_clicks(0.25, false).clicks_per_turn, 60);
        assert!((Turret::default().circumference_mm() - 94.25).abs() < 0.01);
    }
}

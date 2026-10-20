//! The turret label: a strip wrapped round the scope's elevation turret,
//! with the ranges marked at the clicks they need, a band for each turn of
//! the turret, and a ruler of the clicks underneath. The marks come from
//! the calculation; only the turret's size is measured by hand.

use std::collections::BTreeMap;
use std::f32::consts::PI;

use ballistics_shot::Hold;
use serde::{Deserialize, Serialize};

use super::Subject;
use super::fonts::Font;
use super::sheet::{Align, Ink, PT, Sheet, arc, fit_font};
use crate::units::{Quantity, UnitPrefs};

/// A rifle's elevation turret, as measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Turret {
    pub diameter_mm: f64,
    /// The height of the band the label covers.
    pub height_mm: f64,
    pub clicks_per_turn: u32,
    /// The label's thickness: it bends round its middle, so a thicker
    /// label must be longer to close.
    pub thickness_mm: f64,
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
            thickness_mm: 0.1,
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

    /// The strip's length: round the turret at the label's middle.
    pub fn circumference_mm(&self) -> f32 {
        PI * (self.diameter_mm + self.thickness_mm) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numbers {
    /// "1 2 3" for 100, 200, 300 m (1.1, 1.25 between).
    Hundreds,
    /// "10 11 12" for 100, 110, 120 m.
    Tens,
    /// "100 200 300".
    Full,
}

/// The temperature the label is for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Temperature {
    Today,
    /// The weather the rifle (or this load) was zeroed in.
    Zero,
    Chosen(f64),
}

/// A small mark under each number, at its exact click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Marker {
    None,
    Tick,
    Triangle,
}

pub type Rgb = [u8; 3];

/// A part of the strip's own colours; `None` keeps the label's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Colours {
    pub background: Option<Rgb>,
    pub ink: Option<Rgb>,
}

/// Colour schemes to start from: name, background, marks, beyond the limit.
pub const SCHEMES: [(&str, Rgb, Rgb, Rgb); 5] = [
    (
        "Black on white",
        [255, 255, 255],
        [20, 20, 20],
        [192, 36, 28],
    ),
    (
        "White on black",
        [20, 20, 20],
        [255, 255, 255],
        [255, 90, 74],
    ),
    ("Black on yellow", [255, 212, 0], [20, 20, 20], [176, 0, 0]),
    (
        "White on olive",
        [59, 74, 42],
        [255, 255, 255],
        [255, 176, 0],
    ),
    (
        "Orange on black",
        [20, 20, 20],
        [255, 138, 30],
        [255, 59, 48],
    ),
];

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
    pub temperature: Temperature,
    /// Leave out the ranges beyond the ethical limit instead of colouring
    /// them.
    pub stop_at_limit: bool,
    pub ruler: bool,
    pub description: bool,
    pub copies: u32,
    pub font: Font,
    /// The numbers' height as a share of their band.
    pub number_size: f32,
    pub marker: Marker,
    pub background: Rgb,
    pub ink: Rgb,
    /// Marks beyond the ethical limit.
    pub beyond: Rgb,
    pub description_colours: Colours,
    pub ruler_colours: Colours,
    /// Each turn's band, the first turn first.
    pub bands: Vec<Colours>,
    /// Marks with a colour of their own, by range in whole metres.
    pub range_colours: BTreeMap<u32, Rgb>,
}

impl Default for TurretOptions {
    fn default() -> Self {
        let (_, background, ink, beyond) = SCHEMES[0];
        TurretOptions {
            end_m: 500.0,
            numbers_every: 100.0,
            marks_every: 25.0,
            numbers: Numbers::Hundreds,
            temperature: Temperature::Today,
            stop_at_limit: false,
            ruler: true,
            description: true,
            copies: 3,
            font: Font::HelveticaBold,
            number_size: 0.62,
            marker: Marker::None,
            background,
            ink,
            beyond,
            description_colours: Colours::default(),
            ruler_colours: Colours::default(),
            bands: Vec::new(),
            range_colours: BTreeMap::new(),
        }
    }
}

impl TurretOptions {
    /// A turn's band colours, background and marks.
    pub fn band(&self, turn: usize) -> (Rgb, Rgb) {
        let colours = self.bands.get(turn).copied().unwrap_or_default();
        (
            colours.background.unwrap_or(self.background),
            colours.ink.unwrap_or(self.ink),
        )
    }

    /// A mark's colour: its own, else beyond the limit, else its band's.
    pub fn mark_colour(&self, mark: &RangeMark, turn: usize) -> Rgb {
        if let Some(own) = self.range_colours.get(&range_key(mark.range_m)) {
            *own
        } else if mark.beyond {
            self.beyond
        } else {
            self.band(turn).1
        }
    }
}

/// The key a range's own colour is kept under.
pub fn range_key(range_m: f64) -> u32 {
    range_m.round().max(0.0) as u32
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
    /// Which marks give way when they crowd: rounder ranges first.
    pub rank: u8,
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

/// A number with as few decimals as it needs (at most two).
fn short(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl Plan {
    pub fn compute(
        subject: &Subject,
        turret: &Turret,
        options: &TurretOptions,
        u: &UnitPrefs,
    ) -> Result<Plan, String> {
        let distance = u.unit(Quantity::Distance);
        let (temperature_c, powder_c) = match options.temperature {
            Temperature::Today => (
                subject.conditions.temperature_c,
                subject.conditions.powder_temperature_c,
            ),
            Temperature::Zero => match subject.zero_weather {
                Some(zero) => (zero.temperature_c, Some(zero.powder_temperature_c)),
                None => (
                    subject.conditions.temperature_c,
                    subject.conditions.powder_temperature_c,
                ),
            },
            Temperature::Chosen(temperature) => (temperature, None),
        };
        let solution = subject
            .solve_at(
                temperature_c,
                powder_c,
                0.0,
                options.end_m.max(1200.0) + 10.0,
            )
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
        let first = (distance.show(rifle.zero_range_m) / step - 1e-9).ceil() * step;
        let last = distance.show(options.end_m);
        let mut marks = Vec::new();
        let mut out_of_travel = Vec::new();
        let mut index = 0;
        loop {
            let shown = first + index as f64 * step;
            index += 1;
            if shown > last + 1e-6 {
                break;
            }
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
                continue;
            }
            let on = |every: f64| ((shown / every).round() * every - shown).abs() < 1e-6;
            let kind = if on(numbers_every) {
                Kind::Number
            } else if on(numbers_every / 2.0) {
                Kind::Line
            } else {
                Kind::Dot
            };
            let text = match (kind, options.numbers) {
                (Kind::Number, Numbers::Hundreds) => short(shown / 100.0),
                (Kind::Number, Numbers::Tens) => short(shown / 10.0),
                (Kind::Number, Numbers::Full) => format!("{shown:.0}"),
                _ => String::new(),
            };
            let rank = [100.0, 50.0, 25.0, 10.0]
                .iter()
                .position(|every| on(*every))
                .unwrap_or(4) as u8;
            marks.push(RangeMark {
                range_m,
                clicks,
                kind,
                text,
                rank,
                beyond,
                crowded: false,
            });
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
    let font = options.font;
    let mut sheet = Sheet::new(width, height);
    sheet.rect(0.0, 0.0, width, height, Ink::Rgb(options.background));
    let ink = Ink::Rgb(options.ink);

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
        let colours = options.description_colours;
        if let Some(background) = colours.background {
            sheet.rect(0.0, 0.0, width, description_height, Ink::Rgb(background));
        }
        let size = description_height * 0.72 / PT;
        let rifle = &subject.rifle;
        let text = format!(
            "{} · {} · {} · zero {}",
            subject.bullet.display_name(),
            u.format(Quantity::Speed, plan.muzzle_velocity),
            u.format(Quantity::Temperature, plan.temperature_c),
            u.format(Quantity::Distance, rifle.zero_range_m),
        );
        sheet.text_font(
            [width / 2.0, description_height * 0.78],
            fit_font(&text, size, font, width - 2.0),
            size,
            font,
            Align::Center,
            Ink::Rgb(colours.ink.unwrap_or(options.ink)),
        );
    }
    for turn in 0..plan.turns as usize {
        let (background, _) = options.band(turn);
        if background != options.background {
            let top = bands_bottom - (turn + 1) as f32 * band_height;
            sheet.rect(0.0, top, width, band_height, Ink::Rgb(background));
        }
    }
    if options.ruler
        && let Some(background) = options.ruler_colours.background
    {
        sheet.rect(0.0, bands_bottom, width, ruler_height, Ink::Rgb(background));
    }

    // The range marks, a band for each turn, the first turn lowest.
    let marker_height = match options.marker {
        Marker::None => 0.0,
        _ => band_height * 0.16,
    };
    let number_size = band_height * options.number_size.clamp(0.3, 0.95) / PT;
    let gap = band_height * 0.12;
    let mut taken: Vec<Vec<(f32, f32)>> = vec![Vec::new(); plan.turns as usize];
    let mut order: Vec<usize> = (0..plan.marks.len()).collect();
    order.sort_by_key(|&index| {
        let mark = &plan.marks[index];
        let kind = match mark.kind {
            Kind::Number => 0,
            Kind::Line => 1,
            Kind::Dot => 2,
        };
        (kind, mark.rank)
    });
    for index in order {
        let mark = &mut plan.marks[index];
        let turn = (mark.clicks / per_turn as i64) as usize;
        let x = x_of(mark.clicks as f32);
        let half = match mark.kind {
            // Numbers may sit close; the small marks keep their distance.
            Kind::Number => font.width(&mark.text, number_size) / 2.0 + 0.15,
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
        let colour = Ink::Rgb(options.mark_colour(mark, turn));
        match mark.kind {
            Kind::Number => {
                // Centred in the band above the marker; the font's capitals
                // and figures are about 0.7 of its size high.
                let centre = middle - marker_height / 2.0;
                let cap = font.metrics().cap_height as f32 / 1000.0;
                sheet.text_font(
                    [x, centre + number_size * PT * cap / 2.0],
                    &mark.text,
                    number_size,
                    font,
                    Align::Center,
                    colour,
                );
                let tip = bottom - 0.25;
                match options.marker {
                    Marker::None => {}
                    Marker::Tick => {
                        sheet.line([x, tip], [x, tip - marker_height], 0.3, colour);
                    }
                    Marker::Triangle => sheet.fill(
                        vec![
                            [x, tip],
                            [x - marker_height * 0.6, tip - marker_height],
                            [x + marker_height * 0.6, tip - marker_height],
                        ],
                        colour,
                    ),
                }
            }
            Kind::Line => sheet.line(
                [x, middle - band_height * 0.25],
                [x, middle + band_height * 0.25],
                0.35,
                colour,
            ),
            Kind::Dot => sheet.fill(arc([x, middle], band_height * 0.08, 0.0, 360.0), colour),
        }
    }
    if options.description {
        sheet.line([0.0, bands_top], [width, bands_top], 0.12, ink);
    }
    for turn in 1..plan.turns {
        let y = bands_bottom - turn as f32 * band_height;
        sheet.line([0.0, y], [width, y], 0.12, ink);
    }

    // The ruler: a tick for each click rising from the bottom edge, longer
    // at each mil (or MOA) and half, numbered at each whole.
    if options.ruler {
        let ruler_ink = Ink::Rgb(options.ruler_colours.ink.unwrap_or(options.ink));
        let rifle = &subject.rifle;
        let per_unit = (1.0 / rifle.click_value).round().max(1.0) as u32;
        let spacing = width / per_turn;
        let every = if spacing < 0.45 { 2 } else { 1 };
        sheet.line([0.0, bands_bottom], [width, bands_bottom], 0.2, ruler_ink);
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
                ruler_ink,
            );
            if whole {
                sheet.text_font(
                    [x, height - ruler_height * 0.55],
                    format!("{}", click / per_unit),
                    size,
                    font,
                    Align::Center,
                    ruler_ink,
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
    for line in [
        format!(
            "{} · {} · zero {}",
            subject.bullet.display_name(),
            u.format(Quantity::Temperature, plan.temperature_c),
            u.format(Quantity::Distance, subject.rifle.zero_range_m),
        ),
        format!(
            "Turret {:.1} mm across, label {:.2} mm thick, {} clicks a turn, dialled {}",
            turret.diameter_mm,
            turret.thickness_mm,
            turret.clicks_per_turn,
            if turret.clockwise {
                "clockwise"
            } else {
                "anticlockwise"
            }
        ),
    ] {
        page.text([margin, y + 3.0], line, 8.0, false, Align::Left, Ink::Grey);
        y += 4.2;
    }
    y += 5.8;
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
        "Cut on the corner marks. The strip is as long as the turret is round, at the label's middle.",
        "Ends overlapping? Make the label thinner (or the diameter smaller) and print again; a gap: thicker.",
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
        // 30 mm turret, 0.1 mm paper: π × 30.1.
        assert!((Turret::default().circumference_mm() - 94.56).abs() < 0.01);
    }
}

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
use crate::i18n::{tr, trf};
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

/// A mark under each number, at its exact click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Marker {
    None,
    Tick,
    Triangle,
    Line,
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

/// A colour scheme's name in the current language.
pub fn scheme_name(name: &str) -> &str {
    match name {
        "Black on white" => tr("Black on white"),
        "White on black" => tr("White on black"),
        "Black on yellow" => tr("Black on yellow"),
        "White on olive" => tr("White on olive"),
        "Orange on black" => tr("Orange on black"),
        other => other,
    }
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
    /// Line start within the space below each mark, from 0 to 1.
    pub marker_line_start: f32,
    /// Line end within the space below each mark; 1 reaches the label's edge.
    #[serde(alias = "marker_line_length")]
    pub marker_line_end: f32,
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
            marker_line_start: 0.0,
            marker_line_end: 1.0,
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
pub(super) fn short(value: f64) -> String {
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
            let clicks = Hold::new(rifle, &row).elevation_clicks;
            // Inside a zero set high, the bullet is above the line of sight:
            // nothing to dial (the turret's 0 is the far zero).
            if clicks < 0 {
                continue;
            }
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

/// The heights of the description at the top and the ruler at the bottom
/// of a strip `height` high; 0 for a part that is off.
pub(super) fn edges(options: &TurretOptions, height: f32) -> (f32, f32) {
    let description = if options.description {
        (height * 0.17).clamp(1.8, 3.5)
    } else {
        0.0
    };
    let ruler = if options.ruler {
        (height * 0.3).clamp(2.5, 6.0)
    } else {
        0.0
    };
    (description, ruler)
}

/// The description line across the top of a strip, on its own background
/// if it has one.
pub(super) fn description(
    sheet: &mut Sheet,
    options: &TurretOptions,
    description_height: f32,
    text: &str,
) {
    let width = sheet.width;
    let colours = options.description_colours;
    if let Some(background) = colours.background {
        sheet.rect(0.0, 0.0, width, description_height, Ink::Rgb(background));
    }
    let size = description_height * 0.72 / PT;
    sheet.text_font(
        [width / 2.0, description_height * 0.78],
        fit_font(text, size, options.font, width - 2.0),
        size,
        options.font,
        Align::Center,
        Ink::Rgb(colours.ink.unwrap_or(options.ink)),
    );
}

/// The sizes of the marks in a band of the strip.
pub(super) struct Band {
    pub height: f32,
    /// The numbers' size, points.
    pub number_size: f32,
    marker_height: f32,
    gap: f32,
}

impl Band {
    pub fn new(options: &TurretOptions, height: f32) -> Band {
        Band {
            height,
            number_size: height * options.number_size.clamp(0.3, 0.95) / PT,
            marker_height: match options.marker {
                Marker::None => 0.0,
                _ => height * 0.16,
            },
            gap: height * 0.12,
        }
    }

    /// Half the room a mark takes along the strip.
    pub fn half(&self, options: &TurretOptions, kind: Kind, text: &str) -> f32 {
        match kind {
            // Numbers may sit close; the small marks keep their distance.
            Kind::Number => options.font.width(text, self.number_size) / 2.0 + 0.15,
            Kind::Line => 0.35 + self.gap,
            Kind::Dot => self.height * 0.08 + self.gap,
        }
    }

    /// Where a number's baseline goes, at the middle of the band above the
    /// marker (the font's capitals and figures are about 0.7 of its size
    /// high).
    pub fn baseline(&self, options: &TurretOptions, bottom: f32) -> f32 {
        let centre = bottom - self.height / 2.0 - self.marker_height / 2.0;
        let cap = options.font.metrics().cap_height as f32 / 1000.0;
        centre + self.number_size * PT * cap / 2.0
    }

    /// A mark at `x` in the band whose lower edge is at `bottom`, with the
    /// marker under it.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        sheet: &mut Sheet,
        options: &TurretOptions,
        kind: Kind,
        text: &str,
        x: f32,
        bottom: f32,
        colour: Ink,
    ) {
        let band_height = self.height;
        let marker_height = self.marker_height;
        let height = sheet.height;
        let middle = bottom - band_height / 2.0;
        let line_start = match kind {
            Kind::Number => {
                let baseline = self.baseline(options, bottom);
                sheet.text_font(
                    [x, baseline],
                    text,
                    self.number_size,
                    options.font,
                    Align::Center,
                    colour,
                );
                baseline + band_height * 0.04
            }
            Kind::Line => {
                let bottom = middle + band_height * 0.25;
                sheet.line([x, middle - band_height * 0.25], [x, bottom], 0.35, colour);
                bottom
            }
            Kind::Dot => {
                let radius = band_height * 0.08;
                sheet.fill(arc([x, middle], radius, 0.0, 360.0), colour);
                middle + radius
            }
        };
        let tip = bottom - 0.25;
        match options.marker {
            Marker::None => {}
            Marker::Tick => sheet.line([x, tip], [x, tip - marker_height], 0.3, colour),
            Marker::Triangle => sheet.fill(
                vec![
                    [x, tip],
                    [x - marker_height * 0.6, tip - marker_height],
                    [x + marker_height * 0.6, tip - marker_height],
                ],
                colour,
            ),
            Marker::Line => {
                // The endpoints share a scale from below the mark to the
                // strip's lower edge, including the ruler.
                let space = height - 0.15 - line_start;
                let end = if options.marker_line_end.is_finite() {
                    options.marker_line_end.clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let start = if options.marker_line_start.is_finite() {
                    options.marker_line_start.clamp(0.0, end)
                } else {
                    0.0
                };
                if space > 0.0 && end > start {
                    sheet.line(
                        [x, line_start + space * start],
                        [x, line_start + space * end],
                        0.3,
                        colour,
                    );
                }
            }
        }
    }
}

/// What the ruler drew its numbers with.
pub(super) struct Ruler {
    /// The numbers' size, points.
    pub size: f32,
    /// Their baseline.
    pub baseline: f32,
    /// The distance between two numbers along the strip, mm.
    pub spacing: f32,
}

/// The ruler along the strip's lower edge: a tick for each click from
/// `clicks`, longer at each mil (or MOA) and half, numbered at each whole,
/// the numbers counting away from 0 both ways. `largest` is the largest
/// number it may need.
#[allow(clippy::too_many_arguments)]
pub(super) fn ruler(
    sheet: &mut Sheet,
    options: &TurretOptions,
    click_value: f64,
    clicks_per_turn: u32,
    clicks: std::ops::RangeInclusive<i64>,
    largest: u32,
    ruler_height: f32,
    x_of: impl Fn(f32) -> f32,
) -> Ruler {
    let (width, height) = (sheet.width, sheet.height);
    let font = options.font;
    let ruler_ink = Ink::Rgb(options.ruler_colours.ink.unwrap_or(options.ink));
    let per_unit = (1.0 / click_value).round().max(1.0) as i64;
    let spacing = width / clicks_per_turn.max(1) as f32;
    // Ticks at least 0.4 mm apart, and numbers far enough apart to read
    // (every 1, 2, 5 or 10 units on a small turret).
    let every = [1, 2, 5, 10, 20, 50]
        .into_iter()
        .find(|every| *every as f32 * spacing >= 0.4)
        .unwrap_or(100);
    let bands_bottom = height - ruler_height;
    sheet.line([0.0, bands_bottom], [width, bands_bottom], 0.2, ruler_ink);
    let size = ruler_height * 0.36 / PT;
    let widest = font.width(&format!("{largest}"), size) + 0.8;
    let unit_spacing = spacing * per_unit as f32;
    let numbered = [1, 2, 5, 10, 20, 50]
        .into_iter()
        .find(|every| *every as f32 * unit_spacing >= widest)
        .unwrap_or(100);
    let baseline = height - ruler_height * 0.55;
    for click in clicks.filter(|click| click.rem_euclid(every) == 0) {
        let whole = click.rem_euclid(per_unit) == 0;
        let half = per_unit % 2 == 0 && click.rem_euclid(per_unit) == per_unit / 2;
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
        let number = click.abs() / per_unit;
        let text = format!("{number}");
        // A number across the strip's end would be cut in two.
        let half_text = font.width(&text, size) / 2.0;
        if whole && number % numbered == 0 && x - half_text >= 0.0 && x + half_text <= width {
            sheet.text_font([x, baseline], text, size, font, Align::Center, ruler_ink);
        }
    }
    Ruler {
        size,
        baseline,
        spacing: unit_spacing * numbered as f32,
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

    let (description_height, ruler_height) = edges(options, height);
    let bands_top = description_height;
    let bands_bottom = height - ruler_height;
    let band_height = (bands_bottom - bands_top) / plan.turns as f32;

    if options.description {
        let text = trf!(
            "{} · {} · {} · zero {}",
            subject.bullet.display_name(),
            u.format(Quantity::Speed, plan.muzzle_velocity),
            u.format(Quantity::Temperature, plan.temperature_c),
            crate::calculator::zero_text(u, &subject.rifle),
        );
        description(&mut sheet, options, description_height, &text);
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
    let band = Band::new(options, band_height);
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
        let half = band.half(options, mark.kind, &mark.text);
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
        let colour = Ink::Rgb(options.mark_colour(mark, turn));
        band.draw(
            &mut sheet, options, mark.kind, &mark.text, x, bottom, colour,
        );
    }
    if options.description {
        sheet.line([0.0, bands_top], [width, bands_top], 0.12, ink);
    }
    for turn in 1..plan.turns {
        let y = bands_bottom - turn as f32 * band_height;
        sheet.line([0.0, y], [width, y], 0.12, ink);
    }

    if options.ruler {
        let clicks = i64::from(turret.clicks_per_turn);
        let per_unit = (1.0 / subject.rifle.click_value).round().max(1.0) as u32;
        ruler(
            &mut sheet,
            options,
            subject.rifle.click_value,
            turret.clicks_per_turn,
            0..=clicks - 1,
            turret.clicks_per_turn / per_unit,
            ruler_height,
            x_of,
        );
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
        trf!("Turret label · {}", subject.rifle.name),
        14.0,
        true,
        Align::Left,
        Ink::Black,
    );
    y += 9.0;
    for line in [
        trf!(
            "{} · {} · zero {}",
            subject.bullet.display_name(),
            u.format(Quantity::Temperature, plan.temperature_c),
            crate::calculator::zero_text(u, &subject.rifle),
        ),
        trf!(
            "Turret {:.1} mm across, label {:.2} mm thick, {} clicks a turn, dialled {}",
            turret.diameter_mm,
            turret.thickness_mm,
            turret.clicks_per_turn,
            if turret.clockwise {
                tr("clockwise")
            } else {
                tr("anticlockwise")
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
        tr("This bar is 50 mm. If not, print at actual size (100 %, no fitting to the page)."),
        8.0,
        false,
        Align::Left,
        Ink::Black,
    );
    y += 9.0;
    let steps = [
        tr(
            "Cut on the corner marks. The strip is as long as the turret is round, at the label's middle.",
        ),
        tr(
            "Ends overlapping? Make the label thinner (or the diameter smaller) and print again; a gap: thicker.",
        ),
        tr(
            "Set the turret to zero. Wrap the strip with the ruler's 0 at the index line and the numbers",
        ),
        tr("running the way the turret turns when dialling up. Clear tape over it keeps it dry."),
    ];
    for text in steps {
        page.text([margin, y], text, 8.0, false, Align::Left, Ink::Black);
        y += 4.2;
    }
    y += 5.0;

    // The clicks for each range, as on the label.
    page.text([margin, y], tr("Range"), 8.0, true, Align::Left, Ink::Black);
    page.text(
        [margin + 30.0, y],
        tr("Clicks"),
        8.0,
        true,
        Align::Right,
        Ink::Black,
    );
    page.text(
        [margin + 48.0, y],
        tr("Turn"),
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
                tr("not on the label (crowded)"),
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
            trf!(
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
    use super::super::sheet::Mark;
    use super::*;

    #[test]
    fn number_lines_start_below_the_text_and_can_reach_the_labels_bottom_edge() {
        let subject = super::super::tests::demo_subject();
        let turret = Turret::default();
        let plan = Plan {
            marks: [0, 125]
                .into_iter()
                .enumerate()
                .map(|(index, clicks)| RangeMark {
                    range_m: 0.0,
                    clicks,
                    kind: Kind::Number,
                    text: format!("{}", index + 1),
                    rank: 0,
                    beyond: false,
                    crowded: false,
                })
                .collect(),
            turns: 2,
            ethical_m: None,
            temperature_c: 0.0,
            muzzle_velocity: 0.0,
            out_of_travel: Vec::new(),
        };
        let mut preview = Vec::new();
        for font in Font::ALL {
            for length in [0.1, 0.5, 1.0] {
                for ruler in [false, true] {
                    let options = TurretOptions {
                        font,
                        marker: Marker::Line,
                        marker_line_end: length,
                        description: false,
                        ruler,
                        ..TurretOptions::default()
                    };
                    let sheet = strip(
                        &subject,
                        &turret,
                        &options,
                        &mut plan.clone(),
                        &UnitPrefs::metric(),
                    );
                    let lines: Vec<_> = sheet
                        .marks
                        .iter()
                        .filter_map(|mark| {
                            if let Mark::Line { points, width, .. } = mark
                                && *width == 0.3
                            {
                                return Some(points);
                            }
                            None
                        })
                        .collect();
                    assert_eq!(lines.len(), 2);
                    for points in lines {
                        assert_eq!(points[0][0], points[1][0]);
                        let baseline = sheet
                            .marks
                            .iter()
                            .find_map(|mark| {
                                if let Mark::Text { at, text, .. } = mark
                                    && (text == "1" || text == "2")
                                    && at[0] == points[0][0]
                                {
                                    return Some(at[1]);
                                }
                                None
                            })
                            .unwrap();
                        assert!(points[0][1] > baseline, "line must start below the digits");
                        let end = sheet.height - 0.15;
                        assert!(
                            (points[1][1] - (points[0][1] + (end - points[0][1]) * length)).abs()
                                < 1e-5
                        );
                        if length == 1.0 {
                            assert!((points[1][1] - end).abs() < 1e-5);
                        }
                    }
                    if font == Font::HelveticaBold && ruler {
                        let mut preview_plan = plan.clone();
                        for (clicks, kind) in [
                            (35, Kind::Line),
                            (60, Kind::Dot),
                            (135, Kind::Line),
                            (160, Kind::Dot),
                        ] {
                            preview_plan.marks.push(RangeMark {
                                range_m: 0.0,
                                clicks,
                                kind,
                                text: String::new(),
                                rank: 0,
                                beyond: false,
                                crowded: false,
                            });
                        }
                        preview.push(strip(
                            &subject,
                            &turret,
                            &options,
                            &mut preview_plan,
                            &UnitPrefs::metric(),
                        ));
                        if length == 1.0 {
                            for marker in [Marker::Tick, Marker::Triangle] {
                                let options = TurretOptions {
                                    marker,
                                    ..options.clone()
                                };
                                preview.push(strip(
                                    &subject,
                                    &turret,
                                    &options,
                                    &mut preview_plan.clone(),
                                    &UnitPrefs::metric(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        if let Ok(path) = std::env::var("MXSC_LINE_PREVIEW_OUT") {
            std::fs::write(
                path,
                super::super::pdf::write(&preview, "Line length preview"),
            )
            .unwrap();
        }
    }

    #[test]
    fn tick_and_triangle_styles_mark_every_visible_number_tick_and_dot() {
        let subject = super::super::tests::demo_subject();
        let turret = Turret::default();
        let plan = Plan {
            marks: [
                (0, Kind::Number),
                (25, Kind::Line),
                (50, Kind::Dot),
                (100, Kind::Number),
                (125, Kind::Line),
                (150, Kind::Dot),
                (0, Kind::Line), // This overlaps a number and must stay hidden.
            ]
            .into_iter()
            .map(|(clicks, kind)| RangeMark {
                range_m: 0.0,
                clicks,
                kind,
                text: "1".into(),
                rank: 0,
                beyond: false,
                crowded: false,
            })
            .collect(),
            turns: 2,
            ethical_m: None,
            temperature_c: 0.0,
            muzzle_velocity: 0.0,
            out_of_travel: Vec::new(),
        };
        for marker in [Marker::None, Marker::Tick, Marker::Triangle] {
            let options = TurretOptions {
                marker,
                description: false,
                ..TurretOptions::default()
            };
            let mut plan = plan.clone();
            let sheet = strip(&subject, &turret, &options, &mut plan, &UnitPrefs::metric());
            assert_eq!(plan.marks.iter().filter(|mark| !mark.crowded).count(), 6);
            assert!(plan.marks.last().unwrap().crowded);
            let ticks = sheet
                .marks
                .iter()
                .filter(|mark| matches!(mark, Mark::Line { width, .. } if *width == 0.3))
                .count();
            let triangles = sheet
                .marks
                .iter()
                .filter(|mark| matches!(mark, Mark::Fill { points, .. } if points.len() == 3))
                .count();
            assert_eq!(ticks, if marker == Marker::Tick { 6 } else { 0 });
            assert_eq!(triangles, if marker == Marker::Triangle { 6 } else { 0 });
        }
    }

    #[test]
    fn lines_extend_from_intermediate_ticks_and_dots_when_the_line_style_is_selected() {
        let subject = super::super::tests::demo_subject();
        let turret = Turret::default();
        let plan = Plan {
            marks: [
                (0, Kind::Line),
                (25, Kind::Dot),
                (100, Kind::Line),
                (125, Kind::Dot),
            ]
            .into_iter()
            .map(|(clicks, kind)| RangeMark {
                range_m: 0.0,
                clicks,
                kind,
                text: String::new(),
                rank: 0,
                beyond: false,
                crowded: false,
            })
            .collect(),
            turns: 2,
            ethical_m: None,
            temperature_c: 0.0,
            muzzle_velocity: 0.0,
            out_of_travel: Vec::new(),
        };
        for marker in [Marker::None, Marker::Line] {
            for length in [0.25, 1.0] {
                let options = TurretOptions {
                    marker,
                    marker_line_end: length,
                    description: false,
                    ..TurretOptions::default()
                };
                let sheet = strip(
                    &subject,
                    &turret,
                    &options,
                    &mut plan.clone(),
                    &UnitPrefs::metric(),
                );
                let stems: Vec<_> = sheet
                    .marks
                    .iter()
                    .filter_map(|mark| {
                        if let Mark::Line { points, width, .. } = mark
                            && *width == 0.3
                        {
                            return Some(points);
                        }
                        None
                    })
                    .collect();
                if marker == Marker::None {
                    assert!(stems.is_empty());
                    continue;
                }
                assert_eq!(stems.len(), 4, "every visible submark needs a stem");
                for points in stems {
                    let [x, start] = points[0];
                    let joined = sheet.marks.iter().any(|mark| {
                        let bottom = match mark {
                            Mark::Line { points, width, .. }
                                if *width == 0.35 && points[0][0] == x =>
                            {
                                points[1][1]
                            }
                            Mark::Fill { points, ink, .. } if *ink == Ink::Rgb(options.ink) => {
                                let left = points
                                    .iter()
                                    .map(|point| point[0])
                                    .fold(f32::INFINITY, f32::min);
                                let right = points
                                    .iter()
                                    .map(|point| point[0])
                                    .fold(f32::NEG_INFINITY, f32::max);
                                if ((left + right) * 0.5 - x).abs() > 0.001 {
                                    return false;
                                }
                                points
                                    .iter()
                                    .map(|point| point[1])
                                    .fold(f32::NEG_INFINITY, f32::max)
                            }
                            _ => return false,
                        };
                        (bottom - start).abs() < 0.001
                    });
                    assert!(joined, "stem must join the bottom of its tick or dot");
                    assert_eq!(points[1][0], x);
                    if length == 1.0 {
                        assert!((points[1][1] - (sheet.height - 0.15)).abs() < 1e-5);
                    } else {
                        assert!(points[1][1] > start && points[1][1] < sheet.height - 0.15);
                    }
                }
            }
        }
    }

    #[test]
    fn old_label_settings_keep_their_marker_and_default_line_endpoints() {
        let options = TurretOptions {
            marker: Marker::Tick,
            ..TurretOptions::default()
        };
        let mut value = serde_json::to_value(&options).unwrap();
        value.as_object_mut().unwrap().remove("marker_line_end");
        value.as_object_mut().unwrap().remove("marker_line_start");
        let restored: TurretOptions = serde_json::from_value(value).unwrap();
        assert_eq!(restored, options);
    }

    #[test]
    fn saved_line_lengths_become_end_positions_without_changing_the_start() {
        let expected = TurretOptions {
            marker: Marker::Line,
            marker_line_end: 0.37,
            ..TurretOptions::default()
        };
        let mut value = serde_json::to_value(&expected).unwrap();
        let fields = value.as_object_mut().unwrap();
        let end = fields.remove("marker_line_end").unwrap();
        fields.remove("marker_line_start");
        fields.insert("marker_line_length".into(), end);
        let restored: TurretOptions = serde_json::from_value(value).unwrap();
        assert_eq!(restored, expected);
        let saved = serde_json::to_value(restored).unwrap();
        assert!(saved.get("marker_line_length").is_none());
    }

    #[test]
    fn both_line_endpoints_move_independently_for_numbers_ticks_and_dots() {
        let subject = super::super::tests::demo_subject();
        let turret = Turret::default();
        let plan = Plan {
            marks: [(0, Kind::Number), (25, Kind::Line), (50, Kind::Dot)]
                .into_iter()
                .map(|(clicks, kind)| RangeMark {
                    range_m: 0.0,
                    clicks,
                    kind,
                    text: "1".into(),
                    rank: 0,
                    beyond: false,
                    crowded: false,
                })
                .collect(),
            turns: 1,
            ethical_m: None,
            temperature_c: 0.0,
            muzzle_velocity: 0.0,
            out_of_travel: Vec::new(),
        };
        let render = |start, end| {
            let options = TurretOptions {
                marker: Marker::Line,
                marker_line_start: start,
                marker_line_end: end,
                description: false,
                ..TurretOptions::default()
            };
            strip(
                &subject,
                &turret,
                &options,
                &mut plan.clone(),
                &UnitPrefs::metric(),
            )
        };
        let lines = |sheet: &Sheet| -> Vec<Vec<[f32; 2]>> {
            sheet
                .marks
                .iter()
                .filter_map(|mark| match mark {
                    Mark::Line { points, width, .. } if *width == 0.3 => Some(points.clone()),
                    _ => None,
                })
                .collect()
        };
        let full = render(0.0, 1.0);
        let cropped = render(0.25, 0.75);
        let complete_lines = lines(&full);
        let cropped_lines = lines(&cropped);
        assert_eq!(complete_lines.len(), 3);
        assert_eq!(cropped_lines.len(), 3);
        for (full, cropped) in complete_lines.iter().zip(&cropped_lines) {
            let space = full[1][1] - full[0][1];
            assert_eq!(cropped[0][0], full[0][0]);
            assert!((cropped[0][1] - (full[0][1] + space * 0.25)).abs() < 1e-5);
            assert!((cropped[1][1] - (full[0][1] + space * 0.75)).abs() < 1e-5);
        }
        assert!(
            lines(&render(0.5, 0.5)).is_empty(),
            "collapsed endpoints must not leave dots behind"
        );
        assert!(lines(&render(0.9, 0.2)).is_empty());
        assert_eq!(render(-1.0, 2.0), full);
        assert_eq!(render(f32::NAN, f32::INFINITY), full);
        if let Ok(path) = std::env::var("MXSC_LINE_POSITION_PREVIEW_OUT") {
            std::fs::write(
                path,
                super::super::pdf::write(&[full, cropped], "Line position preview"),
            )
            .unwrap();
        }
    }

    #[test]
    fn a_turn_starts_from_the_rifles_clicks() {
        assert_eq!(Turret::for_clicks(0.1, true).clicks_per_turn, 100);
        assert_eq!(Turret::for_clicks(0.25, false).clicks_per_turn, 60);
        // 30 mm turret, 0.1 mm paper: π × 30.1.
        assert!((Turret::default().circumference_mm() - 94.56).abs() < 0.01);
    }
}

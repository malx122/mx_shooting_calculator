//! The windage label: a strip wrapped round the scope's windage turret, with
//! a band for each of a few ranges and in it the wind speeds at the clicks
//! they need, right of the turret's 0 for a wind from the right and left of
//! it for a wind from the left. The marks are for a full-value wind (from 3
//! or 9 o'clock) and for the wind alone: spin drift and the like are left
//! out, so the two sides are alike.

use ballistics_shot::Hold;
use serde::{Deserialize, Serialize};

use super::Subject;
use super::sheet::{Align, Ink, PT, Sheet};
use super::turret::{self, Band, Kind, Numbers, Temperature, Turret, TurretOptions, short};
use crate::i18n::{tr, trf};
use crate::units::{Quantity, UnitPrefs};

/// The most bands a label takes.
pub const MAX_RANGES: usize = 6;

/// The most wind speeds a band takes.
const MAX_WINDS: usize = 40;

/// For a wind unit: the steps to number the winds by, and the numbers and
/// marks to start from.
pub fn wind_steps(unit: &str) -> (&'static [f64], f64, f64) {
    match unit {
        "km/h" => (&[5.0, 10.0, 20.0], 10.0, 5.0),
        "mph" => (&[2.0, 5.0, 10.0], 5.0, 1.0),
        _ => (&[1.0, 2.0, 5.0, 10.0], 2.0, 1.0),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindageOptions {
    /// The ranges with a band each, metres.
    pub ranges_m: Vec<f64>,
    /// A number every so many, in the hunter's wind unit.
    pub numbers_every: f64,
    /// A dot every so many between the numbers; 0 for none.
    pub marks_every: f64,
    /// The strongest wind marked, m/s.
    pub wind_to_mps: f64,
    pub temperature: Temperature,
    /// Leave out the ranges beyond the ethical limit instead of colouring
    /// their numbers.
    pub stop_at_limit: bool,
    /// How the label looks. Only its appearance is used (font, colours,
    /// marker, ruler, description, copies and how ranges are written); the
    /// band colours go with the ranges, nearest first.
    pub look: TurretOptions,
}

impl Default for WindageOptions {
    fn default() -> Self {
        WindageOptions {
            ranges_m: vec![100.0, 200.0, 300.0],
            numbers_every: 2.0,
            marks_every: 1.0,
            wind_to_mps: 10.0,
            temperature: Temperature::Today,
            stop_at_limit: false,
            look: TurretOptions::default(),
        }
    }
}

/// A wind speed on the label, at the same clicks either side of the 0.
#[derive(Clone, Debug, PartialEq)]
pub struct WindMark {
    pub wind_mps: f64,
    /// The clicks to dial towards the wind.
    pub clicks: i64,
    pub kind: Kind,
    pub text: String,
    /// Left off: too close to the range or a more important mark.
    pub crowded: bool,
    /// Left off: more than half a turn, where the two sides meet.
    pub past_half: bool,
}

/// One range's band.
#[derive(Clone, Debug, PartialEq)]
pub struct WindBand {
    pub range_m: f64,
    /// The range as the band shows it.
    pub text: String,
    pub beyond: bool,
    pub marks: Vec<WindMark>,
}

/// The label's bands and what it found on the way.
#[derive(Clone, Debug, PartialEq)]
pub struct WindPlan {
    /// Nearest first; the nearest is the lowest band.
    pub bands: Vec<WindBand>,
    pub ethical_m: Option<f64>,
    pub temperature_c: f64,
    pub muzzle_velocity: f64,
    /// Ranges left out: past the ethical limit (when asked) or past the
    /// calculation.
    pub left_out: Vec<f64>,
}

impl WindPlan {
    pub fn compute(
        subject: &Subject,
        turret: &Turret,
        options: &WindageOptions,
        u: &UnitPrefs,
    ) -> Result<WindPlan, String> {
        let distance = u.unit(Quantity::Distance);
        let wind = u.unit(Quantity::WindSpeed);
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
        let mut ranges: Vec<f64> = options
            .ranges_m
            .iter()
            .copied()
            .filter(|range| range.is_finite() && *range > 0.0)
            .collect();
        ranges.sort_by(f64::total_cmp);
        ranges.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        ranges.truncate(MAX_RANGES);
        if ranges.is_empty() {
            return Err(tr("Add a range for the label.").to_owned());
        }
        let farthest = ranges.last().copied().unwrap_or(100.0);
        let solve = |wind_mps: f64| {
            subject
                .solve_at(
                    temperature_c,
                    powder_c,
                    wind_mps,
                    farthest.max(100.0) + 10.0,
                )
                .map_err(|error| error.to_string())
        };
        let calm = solve(0.0)?;
        let ethical_m = subject.ethical_limit(&calm);

        // The wind speeds, in the hunter's unit, from the first step up.
        let numbers_every = if options.numbers_every.is_finite() {
            options.numbers_every.max(0.5)
        } else {
            2.0
        };
        let step = if options.marks_every > 0.0 && options.marks_every.is_finite() {
            options.marks_every.min(numbers_every)
        } else {
            numbers_every
        };
        let last = wind.show(options.wind_to_mps.clamp(0.5, 40.0));
        let speeds: Vec<f64> = (1..=MAX_WINDS)
            .map(|index| index as f64 * step)
            .take_while(|shown| *shown <= last + 1e-6)
            .collect();
        let windy = speeds
            .iter()
            .map(|shown| solve(wind.base(*shown)))
            .collect::<Result<Vec<_>, _>>()?;

        let rifle = &subject.rifle;
        let half_turn = i64::from(turret.clicks_per_turn.max(1)) / 2;
        let mut bands = Vec::new();
        let mut left_out = Vec::new();
        for range_m in ranges {
            let beyond = ethical_m.is_some_and(|limit| range_m > limit + 1e-6);
            let Some(calm_row) = calm.at(range_m) else {
                left_out.push(range_m);
                continue;
            };
            if beyond && options.stop_at_limit {
                left_out.push(range_m);
                continue;
            }
            let calm_windage = Hold::new(rifle, &calm_row).windage;
            let mut marks = Vec::new();
            for (shown, solution) in speeds.iter().zip(&windy) {
                let Some(row) = solution.at(range_m) else {
                    continue;
                };
                // The wind's own part, without the spin drift.
                let angle = Hold::new(rifle, &row).windage - calm_windage;
                let clicks = (angle / rifle.click_value).round().abs() as i64;
                let on = |every: f64| ((shown / every).round() * every - shown).abs() < 1e-6;
                let kind = if on(numbers_every) {
                    Kind::Number
                } else {
                    Kind::Dot
                };
                marks.push(WindMark {
                    wind_mps: wind.base(*shown),
                    clicks,
                    kind,
                    text: if kind == Kind::Number {
                        short(*shown)
                    } else {
                        String::new()
                    },
                    crowded: false,
                    past_half: clicks > half_turn,
                });
            }
            let shown = distance.show(range_m);
            let text = match options.look.numbers {
                Numbers::Hundreds => short(shown / 100.0),
                Numbers::Tens => short(shown / 10.0),
                Numbers::Full => format!("{shown:.0}"),
            };
            bands.push(WindBand {
                range_m,
                text,
                beyond,
                marks,
            });
        }
        if bands.is_empty() {
            return Err(tr("None of the ranges can go on the label.").to_owned());
        }
        Ok(WindPlan {
            bands,
            ethical_m,
            temperature_c,
            muzzle_velocity: calm.muzzle_velocity,
            left_out,
        })
    }

    /// The numbered winds left off where they would overlap.
    pub fn crowded(&self) -> usize {
        self.marks()
            .filter(|mark| mark.crowded && mark.kind == Kind::Number)
            .count()
    }

    pub fn past_half(&self) -> usize {
        self.marks().filter(|mark| mark.past_half).count()
    }

    fn marks(&self) -> impl Iterator<Item = &WindMark> {
        self.bands.iter().flat_map(|band| band.marks.iter())
    }
}

/// Where a click sits along a strip `width` long, `clicks` right (negative:
/// left) of the 0 in the middle.
fn x_at(turret: &Turret, width: f32, clicks: f32) -> f32 {
    let along = clicks / turret.clicks_per_turn.max(1) as f32 * width;
    if turret.clockwise {
        width / 2.0 + along
    } else {
        width / 2.0 - along
    }
}

/// The strip itself, `circumference × height`, as it is cut out, with the 0
/// in the middle. Marks too close to the range or a more important mark are
/// left off (and flagged in `plan`).
pub fn strip(
    subject: &Subject,
    turret: &Turret,
    options: &WindageOptions,
    plan: &mut WindPlan,
    u: &UnitPrefs,
) -> Sheet {
    let look = &options.look;
    let width = turret.circumference_mm();
    let height = turret.height_mm as f32;
    let mut sheet = Sheet::new(width, height);
    sheet.rect(0.0, 0.0, width, height, Ink::Rgb(look.background));
    let ink = Ink::Rgb(look.ink);
    let x_of = |clicks: f32| x_at(turret, width, clicks);

    let (description_height, ruler_height) = turret::edges(look, height);
    let bands_top = description_height;
    let bands_bottom = height - ruler_height;
    let count = plan.bands.len().max(1);
    let band_height = (bands_bottom - bands_top) / count as f32;

    if look.description {
        let text = trf!(
            "{} · {} · {} · full-value wind in {}",
            subject.bullet.display_name(),
            u.format(Quantity::Speed, plan.muzzle_velocity),
            u.format(Quantity::Temperature, plan.temperature_c),
            u.label(Quantity::WindSpeed),
        );
        turret::description(&mut sheet, look, description_height, &text);
    }
    for index in 0..count {
        let (background, _) = look.band(index);
        if background != look.background {
            let top = bands_bottom - (index + 1) as f32 * band_height;
            sheet.rect(0.0, top, width, band_height, Ink::Rgb(background));
        }
    }
    if look.ruler
        && let Some(background) = look.ruler_colours.background
    {
        sheet.rect(0.0, bands_bottom, width, ruler_height, Ink::Rgb(background));
    }

    let style = Band::new(look, band_height);
    for (index, band) in plan.bands.iter_mut().enumerate() {
        let bottom = bands_bottom - index as f32 * band_height;
        let (background, band_ink) = look.band(index);
        // The range at the 0, white on the band's ink, so it doesn't read
        // as a wind.
        let zero = x_of(0.0);
        let half = look.font.width(&band.text, style.number_size) / 2.0 + band_height * 0.18;
        let mut taken = vec![(zero - half - 0.15, zero + half + 0.15)];
        let range_ink = if band.beyond { look.beyond } else { band_ink };
        // Level with the winds' numbers, which sit above their marker.
        let baseline = style.baseline(look, bottom);
        let cap = look.font.metrics().cap_height as f32 / 1000.0;
        let centre = baseline - style.number_size * PT * cap / 2.0;
        let box_half = (style.number_size * PT * cap / 2.0 + band_height * 0.12)
            .min(centre - (bottom - band_height) - band_height * 0.06);
        sheet.rect(
            zero - half,
            centre - box_half,
            half * 2.0,
            box_half * 2.0,
            Ink::Rgb(range_ink),
        );
        sheet.text_font(
            [zero, baseline],
            &band.text,
            style.number_size,
            look.font,
            Align::Center,
            Ink::Rgb(background),
        );

        let mut order: Vec<usize> = (0..band.marks.len()).collect();
        order.sort_by_key(|&index| {
            let mark = &band.marks[index];
            (mark.kind != Kind::Number, mark.clicks)
        });
        for index in order {
            let mark = &mut band.marks[index];
            if mark.past_half || mark.clicks == 0 {
                continue;
            }
            let half = style.half(look, mark.kind, &mark.text);
            let spans = [-1.0, 1.0].map(|side| {
                let x = x_of(side * mark.clicks as f32);
                (x, (x - half, x + half))
            });
            // A mark across the strip's ends would be cut and meet its
            // twin from the other side.
            if spans.iter().any(|(_, span)| {
                span.0 < 0.0
                    || span.1 > width
                    || taken
                        .iter()
                        .any(|other| span.0 < other.1 && other.0 < span.1)
            }) {
                mark.crowded = true;
                continue;
            }
            for (x, span) in spans {
                taken.push(span);
                style.draw(
                    &mut sheet,
                    look,
                    mark.kind,
                    &mark.text,
                    x,
                    bottom,
                    Ink::Rgb(band_ink),
                );
            }
        }
    }
    if look.description {
        sheet.line([0.0, bands_top], [width, bands_top], 0.12, ink);
    }
    for index in 1..count {
        let y = bands_bottom - index as f32 * band_height;
        sheet.line([0.0, y], [width, y], 0.12, ink);
    }

    // The ruler counts away from the 0 both ways, with L and R beside it.
    if look.ruler {
        let per_turn = i64::from(turret.clicks_per_turn.max(1));
        let per_unit = (1.0 / subject.rifle.click_value).round().max(1.0) as u32;
        let ruler = turret::ruler(
            &mut sheet,
            look,
            subject.rifle.click_value,
            turret.clicks_per_turn,
            -((per_turn - 1) / 2)..=per_turn / 2,
            turret.clicks_per_turn / 2 / per_unit,
            ruler_height,
            x_of,
        );
        let ruler_ink = Ink::Rgb(look.ruler_colours.ink.unwrap_or(look.ink));
        let offset = ruler.spacing / 2.0;
        let room = look.font.width("0", ruler.size) / 2.0 + look.font.width("R", ruler.size);
        if offset > room + 0.3 {
            for (side, letter) in [(1.0, tr("R")), (-1.0, tr("L"))] {
                let x = x_of(side * offset / width * per_turn as f32);
                sheet.text_font(
                    [x, ruler.baseline],
                    letter,
                    ruler.size,
                    look.font,
                    Align::Center,
                    ruler_ink,
                );
            }
        }
    }
    sheet
}

/// The A4 page: copies of the strip with cut marks, a bar to check the
/// print scale, and the clicks for each range and wind.
pub fn page(
    subject: &Subject,
    turret: &Turret,
    options: &WindageOptions,
    plan: &WindPlan,
    strip: &Sheet,
    u: &UnitPrefs,
) -> Sheet {
    let mut page = Sheet::new(210.0, 297.0);
    let margin = 15.0;
    let mut y = margin;
    page.text(
        [margin, y + 5.0],
        trf!("Windage label · {}", subject.rifle.name),
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
            "Turret {:.1} mm across, label {:.2} mm thick, {} clicks a turn, dialled right {}",
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
    let left = (210.0 - strip.width) / 2.0;
    for _ in 0..options.look.copies.clamp(1, 8) {
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
            "Set the windage turret to zero. Wrap the strip with the ruler's 0 at the index line, the",
        ),
        tr(
            "ruler towards the scope, so that the R side comes to the index when dialling right. Tape it.",
        ),
        tr(
            "The numbers are a full-value wind, from 3 or 9 o'clock: dial towards the wind, R for a wind",
        ),
        tr(
            "from the right. From 1, 5, 7 or 11 o'clock a wind counts half; from 2, 4, 8 or 10, 0.87.",
        ),
        tr("The marks are for the wind alone; Calculate adds the spin drift."),
    ];
    for text in steps {
        page.text([margin, y], text, 8.0, false, Align::Left, Ink::Black);
        y += 4.2;
    }
    y += 5.0;

    // The clicks for each range and numbered wind, as on the label.
    let winds: Vec<&WindMark> = plan
        .bands
        .first()
        .map(|band| {
            band.marks
                .iter()
                .filter(|mark| mark.kind == Kind::Number)
                .collect()
        })
        .unwrap_or_default();
    let column = 12.0;
    let first = margin + 34.0;
    let fits = (((210.0 - margin - first) / column) as usize + 1).min(winds.len());
    page.text(
        [margin, y],
        trf!("Range · wind in {}", u.label(Quantity::WindSpeed)),
        8.0,
        true,
        Align::Left,
        Ink::Black,
    );
    for (index, mark) in winds.iter().take(fits).enumerate() {
        page.text(
            [first + index as f32 * column, y],
            &mark.text,
            8.0,
            true,
            Align::Right,
            Ink::Black,
        );
    }
    y += 1.5;
    page.line(
        [margin, y],
        [first + fits.saturating_sub(1) as f32 * column + 2.0, y],
        0.2,
        Ink::Black,
    );
    y += 4.0;
    let mut crowded = false;
    let mut past_half = false;
    for band in &plan.bands {
        let ink = if band.beyond { Ink::Red } else { Ink::Black };
        page.text(
            [margin, y],
            u.format(Quantity::Distance, band.range_m),
            8.0,
            false,
            Align::Left,
            ink,
        );
        let numbers = band.marks.iter().filter(|mark| mark.kind == Kind::Number);
        for (index, mark) in numbers.take(fits).enumerate() {
            let text = if mark.past_half {
                past_half = true;
                "–".to_owned()
            } else if mark.crowded {
                crowded = true;
                format!("{}*", mark.clicks)
            } else {
                mark.clicks.to_string()
            };
            page.text(
                [first + index as f32 * column, y],
                text,
                8.0,
                false,
                Align::Right,
                ink,
            );
        }
        y += 4.0;
    }
    y += 1.0;
    let mut notes = vec![tr("Clicks to dial towards the wind, the same either way.").to_owned()];
    if let Some(dropped) = winds.get(fits) {
        notes.push(trf!(
            "From {} the winds don't fit in the table.",
            u.format(Quantity::WindSpeed, dropped.wind_mps)
        ));
    }
    if crowded {
        notes.push(tr("*: not on the label (crowded).").to_owned());
    }
    if past_half {
        notes.push(tr("–: more than half a turn, not on the label.").to_owned());
    }
    for note in notes {
        page.text([margin, y], note, 7.0, false, Align::Left, Ink::Grey);
        y += 3.6;
    }
    if let (Some(limit), Some(target)) = (plan.ethical_m, &subject.target)
        && plan.bands.iter().any(|band| band.beyond)
    {
        y += 2.0;
        page.text(
            [margin, y],
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

    fn plan_for(options: &WindageOptions, turret: &Turret) -> WindPlan {
        let subject = super::super::tests::demo_subject();
        WindPlan::compute(&subject, turret, options, &UnitPrefs::metric()).unwrap()
    }

    #[test]
    fn wind_clicks_grow_with_wind_and_range() {
        let turret = Turret::default();
        let plan = plan_for(&WindageOptions::default(), &turret);
        assert_eq!(plan.bands.len(), 3);
        assert_eq!(plan.bands[0].text, "1");
        assert_eq!(plan.bands[2].text, "3");
        for band in &plan.bands {
            assert_eq!(band.marks.len(), 10);
            assert!(
                band.marks
                    .windows(2)
                    .all(|pair| pair[0].clicks <= pair[1].clicks)
            );
            let numbers: Vec<_> = band
                .marks
                .iter()
                .filter(|mark| mark.kind == Kind::Number)
                .map(|mark| mark.text.as_str())
                .collect();
            assert_eq!(numbers, ["2", "4", "6", "8", "10"]);
        }
        for pair in plan.bands.windows(2) {
            assert!(pair[1].marks[9].clicks > pair[0].marks[9].clicks);
        }
        // A 10 m/s wind at 300 m moves a hunting bullet some 30–60 cm:
        // 1–2 mil, 10–20 clicks of 0.1 mil.
        let clicks = plan.bands[2].marks[9].clicks;
        assert!((8..=25).contains(&clicks), "{clicks} clicks");
    }

    #[test]
    fn marks_sit_either_side_of_the_zero_in_the_middle() {
        let subject = super::super::tests::demo_subject();
        let turret = Turret::default();
        let options = WindageOptions {
            look: TurretOptions {
                description: false,
                ruler: false,
                ..TurretOptions::default()
            },
            ..WindageOptions::default()
        };
        let u = UnitPrefs::metric();
        let mut plan = WindPlan::compute(&subject, &turret, &options, &u).unwrap();
        let sheet = strip(&subject, &turret, &options, &mut plan, &u);
        let middle = sheet.width / 2.0;
        let mut texts: Vec<(f32, String)> = sheet
            .marks
            .iter()
            .filter_map(|mark| match mark {
                Mark::Text { at, text, .. } => Some((at[0], text.clone())),
                _ => None,
            })
            .collect();
        // The ranges at the 0.
        for range in ["1", "2", "3"] {
            assert!(
                texts
                    .iter()
                    .any(|(x, text)| text == range && (x - middle).abs() < 1e-4)
            );
        }
        texts.retain(|(x, _)| (x - middle).abs() > 1e-4);
        assert!(!texts.is_empty());
        // Each wind on one side has its twin on the other.
        for (x, text) in &texts {
            assert!(
                texts
                    .iter()
                    .any(|(other, twin)| twin == text && (other + x - 2.0 * middle).abs() < 1e-3),
                "{text} at {x} has no twin"
            );
        }
        let drawn = texts.len() / 2;
        let numbers: usize = plan
            .bands
            .iter()
            .flat_map(|band| &band.marks)
            .filter(|mark| {
                mark.kind == Kind::Number && !mark.crowded && !mark.past_half && mark.clicks > 0
            })
            .count();
        assert_eq!(drawn, numbers);
    }

    #[test]
    fn nothing_is_cut_by_the_strips_ends() {
        let subject = super::super::tests::demo_subject();
        let u = UnitPrefs::metric();
        for clicks_per_turn in [20, 40, 60, 100] {
            let turret = Turret {
                clicks_per_turn,
                diameter_mm: 20.0,
                ..Turret::default()
            };
            let options = WindageOptions {
                ranges_m: vec![500.0],
                numbers_every: 1.0,
                marks_every: 0.0,
                wind_to_mps: 20.0,
                ..WindageOptions::default()
            };
            let mut plan = WindPlan::compute(&subject, &turret, &options, &u).unwrap();
            let sheet = strip(&subject, &turret, &options, &mut plan, &u);
            for mark in &sheet.marks {
                if let Mark::Text {
                    at,
                    text,
                    size,
                    font,
                    ..
                } = mark
                    && text.len() < 4
                {
                    let half = font.width(text, *size) / 2.0;
                    assert!(
                        at[0] - half >= -1e-4 && at[0] + half <= sheet.width + 1e-4,
                        "{text} at {} is cut ({clicks_per_turn} clicks a turn)",
                        at[0]
                    );
                }
            }
        }
    }

    #[test]
    fn winds_past_half_a_turn_are_left_off() {
        let turret = Turret {
            clicks_per_turn: 20,
            ..Turret::default()
        };
        let options = WindageOptions {
            ranges_m: vec![600.0],
            wind_to_mps: 20.0,
            ..WindageOptions::default()
        };
        let plan = plan_for(&options, &turret);
        assert!(plan.past_half() > 0);
        assert!(
            plan.bands[0]
                .marks
                .iter()
                .all(|mark| mark.past_half == (mark.clicks > 10))
        );
    }

    #[test]
    fn ranges_beyond_the_limit_are_coloured_or_left_out() {
        let options = WindageOptions {
            ranges_m: vec![300.0, 100.0, 2000.0, 100.0],
            ..WindageOptions::default()
        };
        let turret = Turret::default();
        let plan = plan_for(&options, &turret);
        let ranges: Vec<_> = plan.bands.iter().map(|band| band.range_m).collect();
        assert_eq!(ranges, [100.0, 300.0, 2000.0]);
        assert!(plan.bands[2].beyond);
        let stop = WindageOptions {
            stop_at_limit: true,
            ..options
        };
        let plan = plan_for(&stop, &turret);
        assert_eq!(plan.bands.len(), 2);
        assert_eq!(plan.left_out, [2000.0]);
    }
}

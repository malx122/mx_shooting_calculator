//! The dope card: what to dial or hold at each range, sized to a card the
//! hunter carries (on the stock, in a pocket, in the binocular harness).

use ballistics_core::analysis::PointBlank;
use ballistics_shot::Hold;
use serde::{Deserialize, Serialize};

use super::sheet::{Align, Ink, PT, Sheet, fit, text_width};
use super::{CardSize, Subject};
use crate::units::{Quantity, UnitPrefs};

/// How a correction is written: in clicks, as an angle in the scope's unit
/// (mil or MOA), or as a length at the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    Clicks,
    Angle,
    Length,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DopeOptions {
    pub size: CardSize,
    pub landscape: bool,
    /// Several cards on an A4 page with cut marks, for an ordinary printer.
    pub on_a4: bool,
    pub start_m: f64,
    pub end_m: f64,
    pub step_m: f64,
    /// End the table at the ethical limit for the target.
    pub stop_at_limit: bool,
    /// Air temperatures, °C, an elevation column set for each; empty for
    /// today's.
    pub temperatures: Vec<f64>,
    pub clicks: bool,
    pub angle: bool,
    pub drop: bool,
    /// Full-value wind speeds, m/s, a column each.
    pub winds: Vec<f64>,
    pub wind_as: Measure,
    pub velocity: bool,
    pub energy: bool,
    pub time: bool,
    /// The marks explained at the foot of the card.
    pub legend: bool,
    pub note: String,
}

impl Default for DopeOptions {
    fn default() -> Self {
        DopeOptions {
            size: CardSize::Card,
            landscape: true,
            on_a4: true,
            start_m: 50.0,
            end_m: 300.0,
            step_m: 50.0,
            stop_at_limit: false,
            temperatures: Vec::new(),
            clicks: true,
            angle: true,
            drop: false,
            winds: vec![4.0],
            wind_as: Measure::Angle,
            velocity: false,
            energy: false,
            time: false,
            legend: true,
            note: String::new(),
        }
    }
}

impl DopeOptions {
    /// The card's width and height, mm.
    pub fn card_mm(&self) -> [f32; 2] {
        self.size.oriented(self.landscape)
    }

    pub fn temperatures(&self, today: f64) -> Vec<f64> {
        if self.temperatures.is_empty() {
            vec![today]
        } else {
            self.temperatures.clone()
        }
    }

    pub fn ranges(&self) -> Vec<f64> {
        let step = self.step_m.max(1.0);
        (0..200)
            .map(|index| self.start_m + index as f64 * step)
            .take_while(|range| *range <= self.end_m + 1e-6)
            .collect()
    }
}

/// One column of the card.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// Spans the neighbouring columns with the same group.
    pub group: String,
    pub head: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DopeTable {
    pub ranges: Vec<f64>,
    /// The range column first.
    pub columns: Vec<Column>,
    /// The lowest ethical limit over the temperatures, if the target sets one.
    pub ethical_m: Option<f64>,
    /// Where a dead-on hold stays in the zone, at the first temperature.
    pub dead_on: Option<PointBlank>,
    pub muzzle_velocity: f64,
    pub temperatures: Vec<f64>,
}

impl DopeTable {
    pub fn compute(
        subject: &Subject,
        options: &DopeOptions,
        u: &UnitPrefs,
    ) -> Result<DopeTable, String> {
        let temperatures = options.temperatures(subject.conditions.temperature_c);
        let mut ranges = options.ranges();
        if ranges.is_empty() {
            return Err("no ranges: check From, To and Every".into());
        }
        // Far enough to find the ethical limit beyond the table, too.
        let max_range = (ranges.last().copied().unwrap_or(100.0) + 10.0).max(1200.0);
        let solutions = temperatures
            .iter()
            .map(|&temperature| subject.solve(temperature, 0.0, max_range))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let ethical_m = solutions
            .iter()
            .filter_map(|solution| subject.ethical_limit(solution))
            .reduce(f64::min);
        let dead_on = subject.dead_on(&solutions[0]);
        if options.stop_at_limit
            && let Some(limit) = ethical_m
        {
            ranges.retain(|range| *range <= limit);
            if ranges.is_empty() {
                return Err("every range is beyond the ethical limit".into());
            }
        }

        let rifle = &subject.rifle;
        let unit = rifle.click_unit;
        let angle_label = ballistics_shot::unit_label(unit);
        let decimals = angle_decimals(rifle.click_value);
        let number = |value: f64, decimals: usize| {
            let text = format!("{value:.decimals$}");
            // No "-0" for a hold of nothing.
            if text.trim_start_matches(['-', '0', '.']).is_empty() {
                text.trim_start_matches('-').to_owned()
            } else {
                text
            }
        };
        let mut columns = vec![Column {
            group: String::new(),
            head: u.label(Quantity::Distance).to_owned(),
            cells: ranges
                .iter()
                .map(|range| u.number(Quantity::Distance, *range, false))
                .collect(),
        }];
        let several = temperatures.len() > 1;
        for (temperature, solution) in temperatures.iter().zip(&solutions) {
            let group = if several {
                format!("Up at {}", whole(u, Quantity::Temperature, *temperature))
            } else {
                "Up".to_owned()
            };
            let holds: Vec<Option<Hold>> = ranges
                .iter()
                .map(|range| solution.at(*range).map(|row| Hold::new(rifle, &row)))
                .collect();
            let mut add = |head: &str, cell: &dyn Fn(&Hold) -> String| {
                columns.push(Column {
                    group: group.clone(),
                    head: head.to_owned(),
                    cells: holds
                        .iter()
                        .map(|hold| hold.as_ref().map_or("–".to_owned(), cell))
                        .collect(),
                });
            };
            if options.clicks {
                add("clicks", &|hold| hold.elevation_clicks.to_string());
            }
            if options.angle {
                add(angle_label, &|hold| number(hold.elevation, decimals));
            }
            if options.drop {
                add(u.label(Quantity::ShortLength), &|hold| {
                    number(u.show(Quantity::ShortLength, hold.drop_m * 100.0), 0)
                });
            }
        }
        if !options.winds.is_empty() {
            let still = &solutions[0];
            let group = match options.wind_as {
                Measure::Clicks => "Wind, clicks".to_owned(),
                Measure::Angle => format!("Wind, {angle_label}"),
                Measure::Length => format!("Wind, {}", u.label(Quantity::ShortLength)),
            };
            for &speed in &options.winds {
                let windy = subject
                    .solve(temperatures[0], speed, max_range)
                    .map_err(|error| error.to_string())?;
                let cells = ranges
                    .iter()
                    .map(|&range| {
                        let (Some(calm), Some(blown)) = (still.at(range), windy.at(range)) else {
                            return "–".to_owned();
                        };
                        // The wind alone, without spin drift; always a size.
                        let angle = (blown.windage_correction - calm.windage_correction).abs();
                        let shown = ballistics_shot::from_radians(unit, angle);
                        match options.wind_as {
                            Measure::Clicks => {
                                format!("{:.0}", (shown / rifle.click_value).round())
                            }
                            Measure::Angle => number(shown, decimals),
                            Measure::Length => number(
                                u.show(
                                    Quantity::ShortLength,
                                    (blown.windage - calm.windage).abs() * 100.0,
                                ),
                                0,
                            ),
                        }
                    })
                    .collect();
                columns.push(Column {
                    group: group.clone(),
                    head: whole(u, Quantity::WindSpeed, speed),
                    cells,
                });
            }
        }
        let still = &solutions[0];
        let rows: Vec<_> = ranges.iter().map(|range| still.at(*range)).collect();
        let mut extra =
            |wanted: bool, group: &str, head: &str, cell: &dyn Fn(f64, f64, f64) -> String| {
                if wanted {
                    columns.push(Column {
                        group: group.to_owned(),
                        head: head.to_owned(),
                        cells: rows
                            .iter()
                            .map(|row| {
                                row.map_or("–".to_owned(), |row| {
                                    cell(row.velocity, row.energy_j, row.time)
                                })
                            })
                            .collect(),
                    });
                }
            };
        extra(
            options.velocity,
            "Speed",
            u.label(Quantity::Speed),
            &|velocity, _, _| u.number(Quantity::Speed, velocity, false),
        );
        extra(
            options.energy,
            "Energy",
            u.label(Quantity::Energy),
            &|_, energy, _| u.number(Quantity::Energy, energy, false),
        );
        extra(options.time, "Time", "s", &|_, _, time| {
            format!("{time:.2}")
        });

        Ok(DopeTable {
            ranges,
            columns,
            ethical_m,
            dead_on,
            muzzle_velocity: still.muzzle_velocity,
            temperatures,
        })
    }
}

/// The value with its unit, without decimals when it is a whole number.
fn whole(u: &UnitPrefs, quantity: Quantity, base: f64) -> String {
    let shown = u.show(quantity, base);
    if (shown - shown.round()).abs() < 0.05 {
        format!("{:.0} {}", shown, u.label(quantity))
    } else {
        u.format(quantity, base)
    }
}

/// Decimals for angles: as many as the click value has (0.1 mil: one,
/// ¼ MOA: two).
fn angle_decimals(click_value: f64) -> usize {
    (1..=2)
        .find(|&decimals| {
            let scaled = click_value * 10f64.powi(decimals as i32);
            (scaled - scaled.round()).abs() < 1e-6
        })
        .unwrap_or(2)
}

/// The card laid out, and how big its text came out.
pub struct Layout {
    pub sheet: Sheet,
    /// The table's text size, points.
    pub text_size: f32,
}

/// Lays the card out with the largest text that fits.
pub fn layout(
    subject: &Subject,
    table: &DopeTable,
    options: &DopeOptions,
    u: &UnitPrefs,
) -> Layout {
    let [width, height] = options.card_mm();
    let margin = (width.min(height) * 0.05).clamp(2.5, 8.0);
    let inner = width - 2.0 * margin;
    let rifle = &subject.rifle;

    // The lines above and below the table.
    let title = format!(
        "{} · {}",
        rifle.name,
        crate::widgets::caliber_name(&rifle.caliber)
    );
    let mut about = vec![format!(
        "{} · {} · zero {}",
        subject.bullet.display_name(),
        u.format(Quantity::Speed, table.muzzle_velocity),
        u.format(Quantity::Distance, rifle.zero_range_m),
    )];
    let conditions = &subject.conditions;
    let mut weather = match conditions.pressure {
        ballistics_shot::Pressure::SeaLevel { hpa, altitude_m } => format!(
            "{} at {} above sea level",
            u.format(Quantity::Pressure, hpa),
            u.format(Quantity::Altitude, altitude_m)
        ),
        ballistics_shot::Pressure::Station(hpa) => {
            format!("{} at the stand", u.format(Quantity::Pressure, hpa))
        }
    };
    if let [temperature] = table.temperatures[..] {
        weather = format!(
            "{} · {weather}",
            u.format(Quantity::Temperature, temperature)
        );
    }
    if subject
        .load
        .as_ref()
        .is_some_and(|load| load.truing.is_some())
    {
        weather.push_str(" · trued");
    }
    about.push(weather);
    if !options.note.trim().is_empty() {
        about.push(options.note.trim().to_owned());
    }
    let mut legend = Vec::new();
    if options.legend {
        if let (Some(limit), Some(target)) = (table.ethical_m, &subject.target) {
            legend.push(format!(
                "Red line: ethical limit for {} at {}",
                target.to_lowercase(),
                u.format(Quantity::Distance, limit)
            ));
        }
        if let (Some(dead_on), Some(zone)) = (table.dead_on, subject.vital_cm) {
            legend.push(format!(
                "Green bar: hold dead on ({} zone) to {}",
                u.format(Quantity::ShortLength, zone),
                u.format(Quantity::Distance, dead_on.far_limit)
            ));
        }
        if !options.winds.is_empty() {
            legend.push(
                "Wind full value from 3 or 9 o'clock; half from 1-2, 4-5, 7-8 or 10-11".into(),
            );
        }
    }
    legend.push(format!("{} · {}", crate::APP_NAME, super::today()));

    // Every size scales with the table's text size `s`: find the largest
    // that fits the card both ways.
    const TITLE: f32 = 1.2;
    const SMALL: f32 = 0.85;
    const LINE: f32 = 1.3;
    const ROW: f32 = 1.45;
    const PAD: f32 = 0.9;
    let grouped = table.columns.iter().any(|column| !column.group.is_empty());
    let natural: Vec<f32> = table
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let cells = column
                .cells
                .iter()
                .map(|cell| text_width(cell, 1.0, index == 0))
                .fold(text_width(&column.head, SMALL, false), f32::max);
            cells + PAD * PT
        })
        .collect();
    let mut widths_at_1 = natural.clone();
    // A group's name must fit over its columns.
    for (start, end, group) in groups(&table.columns) {
        let need = text_width(&group, SMALL, true) + PAD * PT;
        let have: f32 = widths_at_1[start..end].iter().sum();
        if need > have {
            let extra = (need - have) / (end - start) as f32;
            for width in &mut widths_at_1[start..end] {
                *width += extra;
            }
        }
    }
    let per_point_wide: f32 = widths_at_1.iter().sum();
    let rows = table.ranges.len() as f32;
    let per_point_high = PT
        * (TITLE * LINE
            + about.len() as f32 * SMALL * LINE
            + 0.4
            + if grouped { SMALL * ROW } else { 0.0 }
            + SMALL * ROW
            + rows * ROW
            + 0.4
            + legend.len() as f32 * SMALL * 0.9 * LINE);
    let s = ((height - 2.0 * margin) / per_point_high)
        .min(inner / per_point_wide)
        .min(if width * height > 20_000.0 {
            14.0
        } else {
            11.0
        });

    let mut sheet = Sheet::new(width, height);
    let mut y = margin;
    let line = |sheet: &mut Sheet, y: &mut f32, text: &str, size: f32, bold: bool, ink: Ink| {
        sheet.text(
            [margin, *y + size * PT * 0.95],
            fit(text, size, bold, inner),
            size,
            bold,
            Align::Left,
            ink,
        );
        *y += size * PT * LINE;
    };
    line(&mut sheet, &mut y, &title, s * TITLE, true, Ink::Black);
    for text in &about {
        line(&mut sheet, &mut y, text, s * SMALL, false, Ink::Grey);
    }
    y += s * PT * 0.4;

    // Spread the columns over the card's width.
    let spare = (inner - per_point_wide * s).max(0.0);
    let widths: Vec<f32> = widths_at_1
        .iter()
        .map(|width| width * s + spare / widths_at_1.len() as f32)
        .collect();
    let lefts: Vec<f32> = widths
        .iter()
        .scan(margin, |left, width| {
            let this = *left;
            *left += width;
            Some(this)
        })
        .collect();
    // Taller rows where the card has height to spare: easier to follow.
    let spare_height = (height - 2.0 * margin - per_point_high * s).max(0.0);
    let row_height = s * PT * ROW + (spare_height / rows).min(s * PT * (2.2 - ROW));
    let right = |index: usize| lefts[index] + widths[index] - PAD * PT * s / 2.0;
    let table_top = y;
    if grouped {
        let head_height = s * SMALL * PT * ROW;
        for (start, end, group) in groups(&table.columns) {
            let left = lefts[start];
            let span = lefts[end - 1] + widths[end - 1] - left;
            sheet.text(
                [left + span / 2.0, y + head_height * 0.72],
                fit(&group, s * SMALL, true, span),
                s * SMALL,
                true,
                Align::Center,
                Ink::Black,
            );
            sheet.line(
                [left + 0.6, y + head_height],
                [left + span - 0.6, y + head_height],
                0.15,
                Ink::Rule,
            );
        }
        y += head_height;
    }
    let head_height = s * SMALL * PT * ROW;
    for (index, column) in table.columns.iter().enumerate() {
        sheet.text(
            [right(index), y + head_height * 0.72],
            &column.head,
            s * SMALL,
            false,
            Align::Right,
            Ink::Grey,
        );
    }
    y += head_height;
    sheet.line([margin, y], [margin + inner, y], 0.3, Ink::Black);
    let body_top = y;
    for (row, range) in table.ranges.iter().enumerate() {
        let beyond = table.ethical_m.is_some_and(|limit| *range > limit + 1e-6);
        if row % 2 == 1 {
            sheet.rect(margin, y, inner, row_height, Ink::Shade);
        }
        if table.dead_on.is_some_and(|dead_on| {
            *range >= dead_on.near_limit - 1e-6 && *range <= dead_on.far_limit + 1e-6
        }) {
            sheet.rect(margin, y, 0.9, row_height, Ink::Green);
        }
        for (index, column) in table.columns.iter().enumerate() {
            sheet.text(
                [right(index), y + row_height * 0.72],
                &column.cells[row],
                s,
                index == 0,
                Align::Right,
                if beyond { Ink::Grey } else { Ink::Black },
            );
        }
        y += row_height;
        let next_beyond = table
            .ranges
            .get(row + 1)
            .is_some_and(|next| table.ethical_m.is_some_and(|limit| *next > limit + 1e-6));
        if !beyond && next_beyond {
            sheet.line([margin, y], [margin + inner, y], 0.5, Ink::Red);
        }
    }
    // Rules between the column groups.
    for (start, _, _) in groups(&table.columns).into_iter().skip(1) {
        let x = lefts[start] - 0.2;
        sheet.line([x, table_top + 0.5], [x, y], 0.15, Ink::Rule);
    }
    if grouped && lefts.len() > 1 {
        sheet.line(
            [lefts[1] - 0.2, body_top],
            [lefts[1] - 0.2, y],
            0.15,
            Ink::Rule,
        );
    }
    sheet.line([margin, y], [margin + inner, y], 0.3, Ink::Black);
    y += s * PT * 0.4;
    for text in &legend {
        line(&mut sheet, &mut y, text, s * SMALL * 0.9, false, Ink::Grey);
    }
    sheet.frame(0.3, 0.3, width - 0.6, height - 0.6, 0.2, Ink::Rule);
    Layout {
        sheet,
        text_size: s,
    }
}

/// The column groups as (first column, past the last, name).
fn groups(columns: &[Column]) -> Vec<(usize, usize, String)> {
    let mut groups: Vec<(usize, usize, String)> = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        match groups.last_mut() {
            Some((_, end, group)) if *group == column.group && *end == index => *end += 1,
            _ => groups.push((index, index + 1, column.group.clone())),
        }
    }
    groups.retain(|(_, _, group)| !group.is_empty());
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_keep_the_click_values_decimals() {
        assert_eq!(angle_decimals(0.1), 1);
        assert_eq!(angle_decimals(0.25), 2);
        assert_eq!(angle_decimals(0.05), 2);
        assert_eq!(angle_decimals(1.0), 1);
    }

    #[test]
    fn groups_span_neighbouring_columns() {
        let column = |group: &str| Column {
            group: group.into(),
            head: String::new(),
            cells: Vec::new(),
        };
        let columns = [column(""), column("Up"), column("Up"), column("Wind")];
        assert_eq!(
            groups(&columns),
            vec![(1, 3, "Up".to_owned()), (3, 4, "Wind".to_owned())]
        );
    }
}

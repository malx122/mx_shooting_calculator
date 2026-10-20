//! When the chosen game may be hunted, where the hunter is: the season in
//! their county or region and today's shooting light, from the country's
//! law (ballistics_store::rules::hunting). Shown short in the calculator
//! and in full on the Targets page.

use ballistics_store::rules::hunting::{Coverage, HuntingLaw, Light, Season, SeasonRow};
use ballistics_store::sun::{self, Date};
use ballistics_store::{Country, model::now_ms};
use eframe::egui::{self, RichText};

use crate::weather::Place;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "8 Oct".
pub fn day_text(date: Date) -> String {
    format!("{} {}", date.day, MONTHS[(date.month - 1) as usize])
}

/// The law's region for a place, from the region the place search gave
/// ("Dalarna", "Stockholm County", "Lapland").
pub fn region_of(country: Country, place: &Place) -> Option<String> {
    let given = place.region.as_deref()?;
    let simple = |text: &str| {
        text.to_lowercase()
            .replace(" county", "")
            .replace(" region", "")
            .replace(" län", "")
            .replace(" maakunta", "")
            .trim()
            .to_owned()
    };
    let given = simple(given);
    // English names the place search uses for Finland's regions.
    let alias = match given.as_str() {
        "scania" => "skåne",
        "lapland" => "lappi",
        "southwest finland" | "finland proper" => "varsinais-suomi",
        "tavastia proper" => "kanta-häme",
        "päijänne tavastia" | "päijät-häme region" => "päijät-häme",
        "south karelia" => "etelä-karjala",
        "south savo" | "southern savonia" => "etelä-savo",
        "north savo" | "northern savonia" => "pohjois-savo",
        "north karelia" => "pohjois-karjala",
        "central finland" => "keski-suomi",
        "south ostrobothnia" | "southern ostrobothnia" => "etelä-pohjanmaa",
        "ostrobothnia" => "pohjanmaa",
        "central ostrobothnia" => "keski-pohjanmaa",
        "north ostrobothnia" | "northern ostrobothnia" => "pohjois-pohjanmaa",
        "kainuu" => "kainuu",
        other => other,
    }
    .to_owned();
    country.hunting().regions.iter().find_map(|region| {
        let name = simple(region);
        // Swedish county names are genitive: "Dalarnas" for "Dalarna".
        (name == alias || name.strip_suffix('s') == Some(alias.as_str())).then(|| region.clone())
    })
}

/// The law's time-zone date and clock now.
fn now(law: &HuntingLaw) -> (Date, f64) {
    law.local_now(now_ms())
}

/// What the season says on `date` in `region`, in a line or two.
pub fn season_lines(season: &Season, region: Option<&str>, date: Date) -> Vec<(String, Tone)> {
    let mut lines = Vec::new();
    if season.decree_expired_on(date) {
        lines.push((
            format!(
                "The decree this season hangs on ran out ({}): check the current rules.",
                season
                    .decree
                    .as_ref()
                    .map_or("", |decree| decree.name.as_str())
            ),
            Tone::Warn,
        ));
    }
    if season.licence_only {
        lines.push((
            "No general season: licence or protective hunting by decision only, or protected."
                .into(),
            Tone::Closed,
        ));
    }
    let Some(region) = region else {
        if !season.licence_only {
            lines.push((
                "Choose your county or region to see the season there.".into(),
                Tone::Plain,
            ));
        }
        return lines;
    };
    let rows: Vec<&SeasonRow> = season
        .rows_on(date)
        .iter()
        .filter(|row| row.in_region(region) != Coverage::None)
        .collect();
    if rows.is_empty() {
        if !season.licence_only {
            lines.push((format!("No season in {region}."), Tone::Closed));
        }
        return lines;
    }
    // Each row as (what it says, its tone, the part of the region it is
    // for); rows saying the same are told once.
    let mut told: Vec<(String, Tone, Option<String>, usize)> = Vec::new();
    for row in rows {
        let part = (row.in_region(region) == Coverage::Part).then(|| row.area.clone());
        let who = if row.applies_to == "all" {
            String::new()
        } else {
            format!(" ({})", row.applies_to)
        };
        let (text, tone) = if row.closed() {
            (format!("No hunting{who}."), Tone::Closed)
        } else {
            let Some((start, end)) = row.span_around(date) else {
                continue;
            };
            if row.open_on(date) {
                let method = row
                    .method_on(date)
                    .map_or(String::new(), |method| format!(", {method}"));
                let opening = match &row.from_time {
                    Some(time) if start == date => format!(" from {time}"),
                    _ => String::new(),
                };
                let closing = match &row.to_time {
                    Some(time) => format!(" {time}"),
                    None => String::new(),
                };
                (
                    format!(
                        "Open{opening}, until {}{closing}{who}{method}.",
                        day_text(end)
                    ),
                    Tone::Open,
                )
            } else {
                (
                    format!("Opens {} – {}{who}.", day_text(start), day_text(end)),
                    Tone::Closed,
                )
            }
        };
        match told.iter_mut().find(|(said, ..)| *said == text) {
            Some(same) => {
                same.3 += 1;
                if part.is_none() {
                    same.2 = None;
                }
            }
            None => told.push((text, tone, part, 1)),
        }
    }
    for (text, tone, part, count) in told {
        let text = match part {
            // One row for part of the region: say which part.
            Some(area) if count == 1 => {
                let area: String = if area.chars().count() > 90 {
                    format!("{}…", area.chars().take(90).collect::<String>())
                } else {
                    area
                };
                format!("{text} In part of {region}: {area}")
            }
            _ => text,
        };
        lines.push((text, tone));
    }
    if season.permit_needed {
        lines.push((
            "A permit (pyyntilupa) is needed as well.".into(),
            Tone::Plain,
        ));
    }
    lines
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Open,
    Closed,
    Warn,
    Plain,
}

fn coloured(ui: &egui::Ui, text: &str, tone: Tone) -> RichText {
    let text = RichText::new(text);
    match tone {
        Tone::Open => text.color(crate::theme::MEADOW),
        Tone::Closed => text.color(ui.visuals().weak_text_color()),
        Tone::Warn => text.color(ui.visuals().warn_fg_color),
        Tone::Plain => text,
    }
}

/// Today's shooting light in words, and how it stands right now.
pub fn light_line(
    law: &HuntingLaw,
    species: &str,
    name: &str,
    place: Option<&Place>,
    country: Country,
) -> Option<(String, Tone)> {
    let (today, clock) = now(law);
    if law.daily_rule(species, today).is_none() {
        return Some((
            format!(
                "{} law sets no time of day for {}.",
                country.adjective(),
                name.to_lowercase()
            ),
            Tone::Plain,
        ));
    }
    let Some(place) = place else {
        return Some((
            "Shooting light: find where you are under Weather (📍 Weather here, or a place)."
                .into(),
            Tone::Plain,
        ));
    };
    let light = law.light(species, today, place.latitude, place.longitude)?;
    let short_place = place
        .name
        .split(',')
        .next()
        .unwrap_or(&place.name)
        .to_owned();
    Some(match light {
        Light::MidnightSun => (
            format!(
                "Midnight sun at {short_place}: the law counts from a sunset that doesn't happen today."
            ),
            Tone::Warn,
        ),
        Light::PolarNight => (
            format!(
                "Polar night at {short_place}: the law counts from a sunrise that doesn't happen today."
            ),
            Tone::Warn,
        ),
        Light::Window {
            start,
            end,
            restricted,
        } => {
            let mut text = format!(
                "Shooting light at {short_place} today: {}–{}",
                sun::clock(start),
                sun::clock(end)
            );
            if let Some((from, what)) = &restricted {
                text.push_str(&format!("; from {} {what}", sun::clock(*from)));
            }
            let (now_text, tone) = if clock < start {
                (format!("starts in {}", span(start - clock)), Tone::Closed)
            } else if clock > end {
                ("over for today".to_owned(), Tone::Closed)
            } else if restricted.as_ref().is_some_and(|(from, _)| clock >= *from) {
                ("now in the restricted part".to_owned(), Tone::Warn)
            } else {
                (
                    format!("allowed now, {} left", span(end - clock)),
                    Tone::Open,
                )
            };
            (format!("{text}. {} {now_text}.", sun::clock(clock)), tone)
        }
    })
}

/// "2 h 10 min".
fn span(minutes: f64) -> String {
    let minutes = minutes.round() as i64;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The short block for the calculator: the season where the hunter is and
/// today's light. `region` is the hunter's choice, kept with the inputs.
pub fn today_block(
    ui: &mut egui::Ui,
    country: Country,
    species: &str,
    name: &str,
    region: &mut Option<String>,
    place: Option<&Place>,
) {
    let law = country.hunting();
    let (today, _) = now(law);
    if region
        .as_ref()
        .is_some_and(|chosen| !law.regions.contains(chosen))
    {
        *region = None;
    }
    if region.is_none() {
        *region = place.and_then(|place| region_of(country, place));
    }
    ui.horizontal_wrapped(|ui| {
        crate::flags::show(ui, country, 11.0);
        ui.strong("Hunting today");
        region_picker(ui, law, region);
    });
    if let Some(season) = law.season(species) {
        for (text, tone) in season_lines(season, region.as_deref(), today) {
            ui.label(coloured(ui, &text, tone));
        }
    }
    if let Some((text, tone)) = light_line(law, species, name, place, country) {
        ui.label(coloured(ui, &text, tone));
    }
    ui.weak(format!(
        "From the law as of {}; seasons can change, check before you hunt. More under Targets.",
        law.as_of
    ));
}

fn region_picker(ui: &mut egui::Ui, law: &HuntingLaw, region: &mut Option<String>) {
    egui::ComboBox::from_id_salt("hunting-region")
        .selected_text(region.as_deref().unwrap_or("choose county or region"))
        .show_ui(ui, |ui| {
            for name in &law.regions {
                ui.selectable_value(region, Some(name.clone()), name);
            }
        });
}

/// The full picture for the Targets page: the season table, the time of
/// day with the law's words, other rules, and the sources.
pub fn species_section(ui: &mut egui::Ui, country: Country, species: &str, region: Option<&str>) {
    let law = country.hunting();
    let (today, _) = now(law);
    ui.horizontal(|ui| {
        crate::flags::show(ui, country, 11.0);
        ui.strong(format!("{} seasons and hours", country.adjective()));
    });
    match law.season(species) {
        None => {
            ui.weak("Not in the law's lists.");
        }
        Some(season) => {
            // What holds now; the table below has the rest.
            for (text, tone) in season_lines(season, region, today) {
                if tone != Tone::Closed || !text.starts_with("Opens") {
                    ui.label(coloured(ui, &text, tone));
                }
            }
            let rows = season.rows_on(today);
            if !rows.is_empty() {
                egui::Grid::new(format!("season-{species}"))
                    .num_columns(3)
                    .min_col_width(90.0)
                    .striped(true)
                    .spacing([10.0, 3.0])
                    .show(ui, |ui| {
                        for row in rows {
                            let here = region
                                .is_some_and(|region| row.in_region(region) != Coverage::None);
                            let dates = match row.span_around(today) {
                                Some((start, end)) if !row.closed() => {
                                    format!("{} – {}", day_text(start), day_text(end))
                                }
                                _ => "no hunting".to_owned(),
                            };
                            let mut area = RichText::new(&row.area);
                            if here {
                                area = area.strong();
                            }
                            ui.add(egui::Label::new(area).wrap());
                            ui.label(dates);
                            let mut detail = if row.applies_to == "all" {
                                String::new()
                            } else {
                                row.applies_to.clone()
                            };
                            if let Some(method) = &row.method {
                                detail = if detail.is_empty() {
                                    method.clone()
                                } else {
                                    format!("{detail}, {method}")
                                };
                            }
                            ui.add(egui::Label::new(RichText::new(detail).weak()).wrap());
                            ui.end_row();
                        }
                    });
            }
            if let Some(change) = &season.change {
                ui.weak(format!("The seasons change from {}.", change.from));
            }
            if let Some(note) = &season.note {
                ui.collapsing("What the law says", |ui| {
                    ui.add(egui::Label::new(RichText::new(note).small()).wrap());
                });
            }
        }
    }
    if let Some(rule) = law.daily_rule(species, today) {
        ui.label(&rule.rule);
        ui.collapsing(format!("{} (the text)", rule.law), |ui| {
            ui.add(egui::Label::new(RichText::new(&rule.quote).italics().small()).wrap());
            ui.hyperlink_to("Source", &rule.url);
        });
    }
    for note in law.notes_for(species) {
        ui.add(egui::Label::new(RichText::new(&note.text).small()).wrap())
            .on_hover_text(&note.law);
    }
    ui.collapsing("Sources", |ui| {
        ui.weak(format!("As of {}. {}", law.as_of, law.version));
        for source in &law.sources {
            ui.hyperlink_to(&source.name, &source.url);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(region: &str) -> Place {
        Place {
            name: "Somewhere".into(),
            latitude: 61.0,
            longitude: 15.0,
            region: Some(region.into()),
            country_code: None,
        }
    }

    #[test]
    fn regions_are_found_from_the_place_search() {
        let region = |country, name| region_of(country, &place(name));
        assert_eq!(
            region(Country::Sweden, "Dalarna").as_deref(),
            Some("Dalarnas län")
        );
        assert_eq!(
            region(Country::Sweden, "Stockholm County").as_deref(),
            Some("Stockholms län")
        );
        assert_eq!(
            region(Country::Sweden, "Västra Götaland").as_deref(),
            Some("Västra Götalands län")
        );
        assert_eq!(
            region(Country::Sweden, "Gotland").as_deref(),
            Some("Gotlands län")
        );
        assert_eq!(
            region(Country::Finland, "Lapland").as_deref(),
            Some("Lappi")
        );
        assert_eq!(
            region(Country::Finland, "Pirkanmaa").as_deref(),
            Some("Pirkanmaa")
        );
        assert_eq!(
            region(Country::Finland, "North Ostrobothnia").as_deref(),
            Some("Pohjois-Pohjanmaa")
        );
        assert_eq!(region(Country::Sweden, "Lapland"), None);
    }

    #[test]
    fn the_season_is_told_for_the_region() {
        let law = Country::Sweden.hunting();
        let moose = law.season("moose").unwrap();
        let day = Date {
            year: 2026,
            month: 9,
            day: 15,
        };
        let lines = season_lines(moose, Some("Skåne län"), day);
        assert!(
            lines[0].0.starts_with("Opens 8 Oct"),
            "{lines:?}",
            lines = lines.iter().map(|l| &l.0).collect::<Vec<_>>()
        );
        let lines = season_lines(moose, Some("Jämtlands län"), day);
        assert!(lines[0].0.starts_with("Open, until 31 Jan"));
        let bear = law.season("brown-bear").unwrap();
        assert!(
            season_lines(bear, Some("Jämtlands län"), day)[0]
                .0
                .starts_with("No general season")
        );
    }
}

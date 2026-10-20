//! When game may be hunted: the time of day (counted from sunrise and
//! sunset) and the seasons, per country, from the law's own text. The data
//! is in `catalog/hunting/<country>.json`, with the paragraphs quoted and
//! their sources; it changes with the law, so each file says when it was
//! taken and which rows hang on decrees that run out.

use serde::Deserialize;

use crate::sun::{self, Date, Daylight};

/// What a time of day is counted from.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum From {
    Sunrise,
    Sunset,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct SunTime {
    pub relative_to: From,
    /// Minutes after the event (negative: before).
    pub minutes: i32,
}

/// Part of the day's window when only some kinds of hunting are allowed.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Restricted {
    pub from: SunTime,
    pub to: SunTime,
    /// What is allowed then, as the law puts it.
    pub text: String,
}

/// A rule for the time of day, for some species.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DailyRule {
    pub species: Vec<String>,
    /// In plain words.
    pub rule: String,
    pub start: SunTime,
    pub end: SunTime,
    #[serde(default)]
    pub restricted: Option<Restricted>,
    /// "YYYY-MM-DD" bounds of a rule that is only in force for a while.
    #[serde(default)]
    pub valid_from: Option<String>,
    #[serde(default)]
    pub valid_until: Option<String>,
    pub law: String,
    pub quote: String,
    pub url: String,
}

/// Something else the law says about when or how; species "ALL" for all.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Note {
    pub species: Vec<String>,
    pub text: String,
    pub law: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub url: String,
}

/// A day of the year as the law gives it: a date, or one counted from the
/// weekdays ("the second Saturday of October") or the month's end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DayRule {
    Date {
        month: u32,
        day: u32,
    },
    /// The `nth` Saturday of a month; -1 for the last.
    Saturday {
        nth: i32,
        month: u32,
    },
    /// The last day of February: 28 or 29.
    EndOfFebruary,
    /// The day before another.
    DayBefore(Box<DayRule>),
}

impl<'de> Deserialize<'de> for DayRule {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        DayRule::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("not a day of the year: {text}")))
    }
}

impl DayRule {
    /// "10-08", "first-saturday-of-10", "last-saturday-of-09",
    /// "day-before-second-saturday-of-10", "end-of-02".
    pub fn parse(text: &str) -> Option<DayRule> {
        if let Some(rest) = text.strip_prefix("day-before-") {
            return Some(DayRule::DayBefore(Box::new(DayRule::parse(rest)?)));
        }
        if text == "end-of-02" {
            return Some(DayRule::EndOfFebruary);
        }
        if let Some((nth, month)) = text.split_once("-saturday-of-") {
            let nth = match nth {
                "first" => 1,
                "second" => 2,
                "third" => 3,
                "fourth" => 4,
                "last" => -1,
                _ => return None,
            };
            let month = month
                .parse()
                .ok()
                .filter(|month| (1..=12).contains(month))?;
            return Some(DayRule::Saturday { nth, month });
        }
        let (month, day) = text.split_once('-')?;
        let (month, day): (u32, u32) = (month.parse().ok()?, day.parse().ok()?);
        ((1..=12).contains(&month) && (1..=31).contains(&day))
            .then_some(DayRule::Date { month, day })
    }

    /// The day in `year`.
    pub fn in_year(&self, year: i32) -> Date {
        match self {
            DayRule::Date { month, day } => Date {
                year,
                month: *month,
                day: *day,
            },
            DayRule::EndOfFebruary => Date {
                year,
                month: 3,
                day: 1,
            }
            .add_days(-1),
            DayRule::Saturday { nth, month } => {
                let saturday = |date: Date| date.add_days(i64::from((5 + 7 - date.weekday()) % 7));
                if *nth > 0 {
                    saturday(Date {
                        year,
                        month: *month,
                        day: 1,
                    })
                    .add_days(7 * i64::from(nth - 1))
                } else {
                    let (next_year, next_month) = if *month == 12 {
                        (year + 1, 1)
                    } else {
                        (year, month + 1)
                    };
                    let last = Date {
                        year: next_year,
                        month: next_month,
                        day: 1,
                    }
                    .add_days(-1);
                    last.add_days(-i64::from((last.weekday() + 7 - 5) % 7))
                }
            }
            DayRule::DayBefore(rule) => rule.in_year(year).add_days(-1),
        }
    }

    /// Month and day, to tell whether a span runs over the new year.
    fn month_day(&self, year: i32) -> (u32, u32) {
        let date = self.in_year(year);
        (date.month, date.day)
    }
}

/// Part of a season with a limit on how one may hunt.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Period {
    pub from: DayRule,
    pub to: DayRule,
    pub method: String,
}

/// One line of a season table.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SeasonRow {
    /// As the law writes it.
    pub area: String,
    /// The whole regions (counties) the row covers.
    #[serde(default)]
    pub regions: Vec<String>,
    /// Regions the row covers only part of.
    #[serde(default)]
    pub part_of: Vec<String>,
    /// `None` for both: no hunting there.
    #[serde(default)]
    pub from: Option<DayRule>,
    /// Inclusive; before `from` in the year when the season runs over the
    /// new year.
    #[serde(default)]
    pub to: Option<DayRule>,
    /// Which animals: "all", "calves", …
    pub applies_to: String,
    #[serde(default)]
    pub note: Option<String>,
    /// How one may hunt during the whole row, if limited.
    #[serde(default)]
    pub method: Option<String>,
    /// Parts of the row with their own limits.
    #[serde(default)]
    pub periods: Vec<Period>,
    /// A clock time the season opens at on its first day ("12:00").
    #[serde(default)]
    pub from_time: Option<String>,
    /// A clock time it closes at on its last day.
    #[serde(default)]
    pub to_time: Option<String>,
}

/// Whether `date` falls in from…to (inclusive), the span starting in the
/// date's year or the year before when it runs over the new year.
fn within(date: Date, from: &DayRule, to: &DayRule) -> bool {
    [date.year - 1, date.year].into_iter().any(|year| {
        let start = from.in_year(year);
        let end_year = if to.month_day(year) < from.month_day(year) {
            year + 1
        } else {
            year
        };
        let end = to.in_year(end_year);
        start <= date && date <= end
    })
}

impl SeasonRow {
    /// Whether the row is open on `date` (whatever the region).
    pub fn open_on(&self, date: Date) -> bool {
        match (&self.from, &self.to) {
            (Some(from), Some(to)) => within(date, from, to),
            _ => false,
        }
    }

    /// A row that says there is no hunting.
    pub fn closed(&self) -> bool {
        self.from.is_none() || self.to.is_none()
    }

    /// The row's span in the hunting year that `date` falls in or is
    /// nearest after: its first and last day.
    pub fn span_around(&self, date: Date) -> Option<(Date, Date)> {
        let (from, to) = (self.from.as_ref()?, self.to.as_ref()?);
        [date.year - 1, date.year, date.year + 1]
            .into_iter()
            .map(|year| {
                let end_year = if to.month_day(year) < from.month_day(year) {
                    year + 1
                } else {
                    year
                };
                (from.in_year(year), to.in_year(end_year))
            })
            .find(|(_, end)| *end >= date)
    }

    /// How the row covers a region.
    pub fn in_region(&self, region: &str) -> Coverage {
        if self.regions.iter().any(|name| name == region) {
            Coverage::Whole
        } else if self.part_of.iter().any(|name| name == region) {
            Coverage::Part
        } else {
            Coverage::None
        }
    }

    /// The method limit on `date`, if any.
    pub fn method_on(&self, date: Date) -> Option<&str> {
        self.periods
            .iter()
            .find(|period| within(date, &period.from, &period.to))
            .map(|period| period.method.as_str())
            .or(self.method.as_deref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Whole,
    Part,
    None,
}

/// Rows that replace a species' rows from a date (a change in the law
/// already decided).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Change {
    pub from: String,
    pub rows: Vec<SeasonRow>,
    #[serde(default)]
    pub note: Option<String>,
}

/// A decree a season hangs on, which runs out.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Decree {
    pub name: String,
    /// "YYYY-MM-DD".
    pub valid_until: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Season {
    pub species: String,
    #[serde(default)]
    pub rows: Vec<SeasonRow>,
    /// No general season: licence or protective hunting by decision only,
    /// or protected.
    #[serde(default)]
    pub licence_only: bool,
    /// A permit is needed even in season (Finland's pyyntilupa).
    #[serde(default)]
    pub permit_needed: bool,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub change: Option<Change>,
    #[serde(default)]
    pub decree: Option<Decree>,
    #[serde(default)]
    pub law: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl Season {
    /// The rows in force on `date`.
    pub fn rows_on(&self, date: Date) -> &[SeasonRow] {
        match &self.change {
            Some(change) if date.to_string() >= change.from => &change.rows,
            _ => &self.rows,
        }
    }

    /// The decree the season hangs on has run out by `date`.
    pub fn decree_expired_on(&self, date: Date) -> bool {
        self.decree
            .as_ref()
            .is_some_and(|decree| date.to_string() > decree.valid_until)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Source {
    pub name: String,
    pub url: String,
}

/// A country's hunting times.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HuntingLaw {
    /// When the data was taken from the sources, "YYYY-MM-DD".
    pub as_of: String,
    /// Which version of the law it follows.
    pub version: String,
    /// The country's regions (counties) in the law's naming.
    pub regions: Vec<String>,
    /// Hours east of UTC in winter; the countries keep EU summer time.
    pub utc_offset_hours: f64,
    pub sources: Vec<Source>,
    pub daily: Vec<DailyRule>,
    #[serde(default)]
    pub notes: Vec<Note>,
    pub seasons: Vec<Season>,
}

/// The day's shooting light for a species at a place, local clock times
/// (minutes after midnight).
#[derive(Clone, Debug, PartialEq)]
pub enum Light {
    /// From `start` to `end`; from the restriction's start to `end` only
    /// what it says.
    Window {
        start: f64,
        end: f64,
        restricted: Option<(f64, String)>,
    },
    /// Midnight sun: the law counts from a sunrise and sunset that don't
    /// happen.
    MidnightSun,
    /// Polar night: likewise.
    PolarNight,
}

impl HuntingLaw {
    pub fn season(&self, species: &str) -> Option<&Season> {
        self.seasons.iter().find(|season| season.species == species)
    }

    /// The time-of-day rule for a species in force on `date`.
    pub fn daily_rule(&self, species: &str, date: Date) -> Option<&DailyRule> {
        let today = date.to_string();
        self.daily.iter().find(|rule| {
            rule.species.iter().any(|id| id == species)
                && rule.valid_from.as_ref().is_none_or(|from| today >= *from)
                && rule
                    .valid_until
                    .as_ref()
                    .is_none_or(|until| today <= *until)
        })
    }

    pub fn notes_for<'a>(&'a self, species: &'a str) -> impl Iterator<Item = &'a Note> + 'a {
        self.notes
            .iter()
            .filter(move |note| note.species.iter().any(|id| id == species || id == "ALL"))
    }

    /// The shooting light for a species on `date` at a place, if the law
    /// limits the time of day for it.
    pub fn light(&self, species: &str, date: Date, latitude: f64, longitude: f64) -> Option<Light> {
        let rule = self.daily_rule(species, date)?;
        let (sunrise, sunset) = match sun::daylight(date, latitude, longitude) {
            Daylight::RisesAndSets { sunrise, sunset } => (sunrise, sunset),
            Daylight::AlwaysUp => return Some(Light::MidnightSun),
            Daylight::AlwaysDown => return Some(Light::PolarNight),
        };
        let at = |time: SunTime| {
            let utc = match time.relative_to {
                From::Sunrise => sunrise,
                From::Sunset => sunset,
            } + f64::from(time.minutes);
            sun::local(date, utc, self.utc_offset_hours).0
        };
        Some(Light::Window {
            start: at(rule.start),
            end: at(rule.end),
            restricted: rule
                .restricted
                .as_ref()
                .map(|restricted| (at(restricted.from), restricted.text.clone())),
        })
    }

    /// The local date and clock (minutes after midnight) at a moment.
    pub fn local_now(&self, unix_ms: i64) -> (Date, f64) {
        let utc_date = Date::from_unix_ms(unix_ms);
        let minutes = unix_ms.rem_euclid(86_400_000) as f64 / 60_000.0;
        let (local, days) = sun::local(utc_date, minutes, self.utc_offset_hours);
        (utc_date.add_days(days), local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    fn rule(text: &str) -> DayRule {
        DayRule::parse(text).unwrap()
    }

    #[test]
    fn the_data_parses_and_names_known_species_and_regions() {
        let catalog = crate::Catalog::builtin();
        for country in crate::Country::ALL {
            let law = country.hunting();
            let known = |id: &str| id == "ALL" || catalog.target(id).is_some();
            for season in &law.seasons {
                assert!(known(&season.species), "{country:?}: {}", season.species);
                let rows = season
                    .rows
                    .iter()
                    .chain(season.change.iter().flat_map(|c| &c.rows));
                for row in rows {
                    for region in row.regions.iter().chain(&row.part_of) {
                        assert!(law.regions.contains(region), "{country:?}: {region}");
                    }
                    assert_eq!(row.from.is_none(), row.to.is_none(), "{}", season.species);
                }
            }
            for rule in &law.daily {
                assert!(rule.species.iter().all(|id| known(id)), "{country:?}");
            }
            for note in &law.notes {
                assert!(
                    note.species.iter().all(|id| known(id)),
                    "{country:?}: {:?}",
                    note.species
                );
            }
        }
    }

    #[test]
    fn moose_in_dalarna_and_pirkanmaa() {
        let sweden = crate::Country::Sweden.hunting();
        let moose = sweden.season("moose").unwrap();
        let open_in = |region: &str, day: Date| {
            moose
                .rows_on(day)
                .iter()
                .filter(|row| row.in_region(region) != Coverage::None && row.open_on(day))
                .count()
        };
        // Dalarna: open from 1 Sept in the north-west, 8 Oct in the rest.
        assert_eq!(open_in("Dalarnas län", date(2026, 9, 15)), 1);
        assert_eq!(open_in("Dalarnas län", date(2026, 10, 20)), 2);
        assert_eq!(open_in("Skåne län", date(2026, 9, 15)), 0);
        // Moose light in Mora: from an hour before sunrise, the last hour
        // stalking or watching only.
        let Some(Light::Window {
            start,
            end,
            restricted,
        }) = sweden.light("moose", date(2026, 10, 20), 61.0, 14.54)
        else {
            panic!("moose has a time of day in Sweden");
        };
        assert!(start > 5.0 * 60.0 && start < 7.0 * 60.0, "{start}");
        assert!(end > 18.0 * 60.0 && end < 19.5 * 60.0, "{end}");
        assert!((end - restricted.unwrap().0 - 60.0).abs() < 1.0);
        assert!(
            sweden
                .light("wild-boar", date(2026, 10, 20), 61.0, 14.54)
                .is_none()
        );

        // Finland: in Pirkanmaa only stand hunting from a field until the
        // day before the second Saturday of October (9 Oct 2026).
        let finland = crate::Country::Finland.hunting();
        let moose = finland.season("moose").unwrap();
        let rows = |day| {
            moose
                .rows_on(day)
                .iter()
                .filter(|row| row.in_region("Pirkanmaa") == Coverage::Whole && row.open_on(day))
                .map(|row| row.applies_to.clone())
                .collect::<Vec<_>>()
        };
        assert!(rows(date(2026, 10, 9))[0].contains("field"));
        assert_eq!(rows(date(2026, 10, 10)), vec!["all".to_owned()]);
        assert!(moose.permit_needed);
        assert!(
            finland
                .light("moose", date(2026, 10, 20), 61.5, 23.8)
                .is_none()
        );
        assert!(
            finland
                .light("mallard", date(2026, 10, 20), 61.5, 23.8)
                .is_some()
        );
    }

    #[test]
    fn days_are_counted_as_the_law_counts_them() {
        // October 2026 starts on a Thursday.
        assert_eq!(
            rule("first-saturday-of-10").in_year(2026),
            date(2026, 10, 3)
        );
        assert_eq!(
            rule("second-saturday-of-10").in_year(2026),
            date(2026, 10, 10)
        );
        assert_eq!(
            rule("day-before-second-saturday-of-10").in_year(2026),
            date(2026, 10, 9)
        );
        assert_eq!(rule("last-saturday-of-09").in_year(2026), date(2026, 9, 26));
        assert_eq!(rule("end-of-02").in_year(2028), date(2028, 2, 29));
        assert_eq!(rule("end-of-02").in_year(2027), date(2027, 2, 28));
        assert!(DayRule::parse("13-01").is_none());
        assert!(DayRule::parse("fifth-sunday-of-10").is_none());
    }

    #[test]
    fn seasons_run_over_the_new_year() {
        let (from, to) = (rule("10-08"), rule("01-31"));
        assert!(within(date(2026, 12, 24), &from, &to));
        assert!(within(date(2027, 1, 31), &from, &to));
        assert!(!within(date(2027, 2, 1), &from, &to));
        assert!(!within(date(2026, 10, 7), &from, &to));
        let (from, to) = (rule("11-16"), rule("end-of-02"));
        assert!(within(date(2028, 2, 29), &from, &to));
        assert!(!within(date(2028, 3, 1), &from, &to));
        // A moose season from the second Saturday of October.
        let (from, to) = (rule("second-saturday-of-10"), rule("01-15"));
        assert!(!within(date(2026, 10, 9), &from, &to));
        assert!(within(date(2026, 10, 10), &from, &to));
    }
}

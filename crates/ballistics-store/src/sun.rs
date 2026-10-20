//! Sunrise and sunset anywhere, offline: NOAA's solar position formulas
//! (from Meeus), good to a minute or so. Hunting laws count from them, so
//! midnight sun and polar night are answered as such. Times are minutes
//! after midnight UTC on the day asked for; `local` turns them into the
//! clock time of a country that keeps EU summer time.

use std::f64::consts::PI;

/// A calendar day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The day `days` after 1970-01-01 (Howard Hinnant's algorithm).
    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
        let month = if month_index < 10 {
            month_index + 3
        } else {
            month_index - 9
        } as u32;
        let year = (year_of_era + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }

    /// Days since 1970-01-01.
    pub fn days(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year.rem_euclid(400);
        let month = i64::from(self.month);
        let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5
            + i64::from(self.day)
            - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    /// The date of a moment, in UTC.
    pub fn from_unix_ms(ms: i64) -> Date {
        Date::from_days(ms.div_euclid(86_400_000))
    }

    /// 0 Monday … 6 Sunday.
    pub fn weekday(self) -> u32 {
        // 1970-01-01 was a Thursday.
        ((self.days() + 3).rem_euclid(7)) as u32
    }

    pub fn add_days(self, days: i64) -> Date {
        Date::from_days(self.days() + days)
    }

    /// The last Sunday of a month.
    fn last_sunday(year: i32, month: u32) -> Date {
        let first_of_next = if month == 12 {
            Date {
                year: year + 1,
                month: 1,
                day: 1,
            }
        } else {
            Date {
                year,
                month: month + 1,
                day: 1,
            }
        };
        let last = first_of_next.add_days(-1);
        last.add_days(-i64::from((last.weekday() + 1) % 7))
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// The sun on a day at a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Daylight {
    /// Rises and sets: minutes after midnight UTC.
    RisesAndSets { sunrise: f64, sunset: f64 },
    /// Midnight sun: up all day.
    AlwaysUp,
    /// Polar night: never rises.
    AlwaysDown,
}

/// Sunrise and sunset (the sun's upper edge on the horizon, with the
/// usual refraction: 90.833° from the zenith) on `date` at a place,
/// latitude north and longitude east positive.
pub fn daylight(date: Date, latitude: f64, longitude: f64) -> Daylight {
    // First at local noon, then again at each event's own time.
    let noon = 720.0 - 4.0 * longitude;
    let first = event(date, noon, latitude, longitude);
    let (Some(rise), Some(set)) = first else {
        return match first {
            (None, None) if sun_at(date, noon).0 * latitude.signum() > 0.0 => Daylight::AlwaysUp,
            _ => Daylight::AlwaysDown,
        };
    };
    let refine = |minutes: f64, rising: bool| {
        let (rise, set) = event(date, minutes, latitude, longitude);
        if rising { rise } else { set }.unwrap_or(minutes)
    };
    Daylight::RisesAndSets {
        sunrise: refine(rise, true),
        sunset: refine(set, false),
    }
}

/// Sunrise and sunset worked out with the sun's position at `minutes`
/// UTC; `None` when the sun doesn't cross the horizon.
fn event(date: Date, minutes: f64, latitude: f64, longitude: f64) -> (Option<f64>, Option<f64>) {
    let (declination, equation_of_time) = sun_at(date, minutes);
    let lat = latitude.to_radians();
    let cos_hour_angle = (90.833_f64.to_radians().cos() - lat.sin() * declination.sin())
        / (lat.cos() * declination.cos());
    if !(-1.0..=1.0).contains(&cos_hour_angle) {
        return (None, None);
    }
    let hour_angle = cos_hour_angle.acos().to_degrees();
    let noon = 720.0 - 4.0 * longitude - equation_of_time;
    (Some(noon - 4.0 * hour_angle), Some(noon + 4.0 * hour_angle))
}

/// The sun's declination (radians) and the equation of time (minutes) at
/// `minutes` after midnight UTC on `date`.
fn sun_at(date: Date, minutes: f64) -> (f64, f64) {
    let julian_day = date.days() as f64 + 2_440_587.5 + minutes / 1440.0;
    let t = (julian_day - 2_451_545.0) / 36_525.0;
    let radians = |degrees: f64| degrees * PI / 180.0;
    let mean_longitude = (280.466_46 + t * (36_000.769_83 + t * 0.000_303_2)).rem_euclid(360.0);
    let mean_anomaly = 357.529_11 + t * (35_999.050_29 - 0.000_153_7 * t);
    let eccentricity = 0.016_708_634 - t * (0.000_042_037 + 0.000_000_126_7 * t);
    let m = radians(mean_anomaly);
    let centre = m.sin() * (1.914_602 - t * (0.004_817 + 0.000_014 * t))
        + (2.0 * m).sin() * (0.019_993 - 0.000_101 * t)
        + (3.0 * m).sin() * 0.000_289;
    let omega = radians(125.04 - 1934.136 * t);
    let apparent_longitude = radians(mean_longitude + centre - 0.005_69 - 0.004_78 * omega.sin());
    let mean_obliquity =
        23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.000_59 - t * 0.001_813))) / 60.0) / 60.0;
    let obliquity = radians(mean_obliquity + 0.002_56 * omega.cos());
    let declination = (obliquity.sin() * apparent_longitude.sin()).asin();
    let y = (obliquity / 2.0).tan().powi(2);
    let l0 = radians(mean_longitude);
    let equation_of_time = 4.0
        * (y * (2.0 * l0).sin() - 2.0 * eccentricity * m.sin()
            + 4.0 * eccentricity * y * m.sin() * (2.0 * l0).cos()
            - 0.5 * y * y * (4.0 * l0).sin()
            - 1.25 * eccentricity * eccentricity * (2.0 * m).sin())
        .to_degrees();
    (declination, equation_of_time)
}

/// Is EU summer time on at `minutes` UTC on `date`? It runs from 01:00
/// UTC on the last Sunday of March to 01:00 UTC on the last Sunday of
/// October.
pub fn eu_summer_time(date: Date, minutes: f64) -> bool {
    let start = Date::last_sunday(date.year, 3);
    let end = Date::last_sunday(date.year, 10);
    let after_start = date > start || (date == start && minutes >= 60.0);
    let before_end = date < end || (date == end && minutes < 60.0);
    after_start && before_end
}

/// Minutes UTC as the local clock in a zone `standard_offset_hours` east of
/// UTC that keeps EU summer time: (minutes after local midnight, and the
/// days it moved, -1, 0 or 1).
pub fn local(date: Date, minutes_utc: f64, standard_offset_hours: f64) -> (f64, i64) {
    let summer = if eu_summer_time(date, minutes_utc) {
        60.0
    } else {
        0.0
    };
    let minutes = minutes_utc + standard_offset_hours * 60.0 + summer;
    let days = minutes.div_euclid(1440.0) as i64;
    (minutes.rem_euclid(1440.0), days)
}

/// "06:42", minutes after midnight.
pub fn clock(minutes: f64) -> String {
    let rounded = minutes.round().rem_euclid(1440.0) as i64;
    format!("{:02}:{:02}", rounded / 60, rounded % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn dates_count_days() {
        let days = date(2026, 10, 20).days();
        assert_eq!(Date::from_days(days), date(2026, 10, 20));
        assert_eq!(date(1970, 1, 1).days(), 0);
        assert_eq!(date(2026, 10, 20).weekday(), 1); // a Tuesday
        assert_eq!(Date::last_sunday(2026, 3), date(2026, 3, 29));
        assert_eq!(Date::last_sunday(2026, 10), date(2026, 10, 25));
        assert_eq!(date(2024, 2, 28).add_days(1), date(2024, 2, 29));
    }

    /// Minutes UTC to the local clock in Sweden (UTC+1, summer time).
    fn swedish(day: Date, minutes: f64) -> String {
        clock(local(day, minutes, 1.0).0)
    }

    #[test]
    fn stockholm_sunrise_and_sunset() {
        // Stockholm, published times (to the minute): midsummer 03:30 and
        // 22:08; the winter solstice 08:43 and 14:48.
        let (latitude, longitude) = (59.3293, 18.0686);
        let check = |day: Date, rise: &str, set: &str| {
            let Daylight::RisesAndSets { sunrise, sunset } = daylight(day, latitude, longitude)
            else {
                panic!("the sun rises in Stockholm");
            };
            let near = |got: String, want: &str| {
                let minutes = |text: &str| {
                    let (h, m) = text.split_once(':').unwrap();
                    h.parse::<i32>().unwrap() * 60 + m.parse::<i32>().unwrap()
                };
                (minutes(&got) - minutes(want)).abs() <= 2
            };
            assert!(
                near(swedish(day, sunrise), rise),
                "{} {}",
                swedish(day, sunrise),
                rise
            );
            assert!(
                near(swedish(day, sunset), set),
                "{} {}",
                swedish(day, sunset),
                set
            );
        };
        check(date(2026, 6, 21), "03:30", "22:08");
        check(date(2026, 12, 21), "08:43", "14:48");
    }

    #[test]
    fn the_far_north_has_midnight_sun_and_polar_night() {
        // Kiruna.
        assert_eq!(
            daylight(date(2026, 6, 21), 67.855, 20.225),
            Daylight::AlwaysUp
        );
        assert_eq!(
            daylight(date(2026, 12, 21), 67.855, 20.225),
            Daylight::AlwaysDown
        );
        // And the south of the world the other way round.
        assert_eq!(daylight(date(2026, 12, 21), -70.0, 0.0), Daylight::AlwaysUp);
    }

    #[test]
    fn summer_time_follows_the_eu_rule() {
        assert!(!eu_summer_time(date(2026, 3, 29), 59.0));
        assert!(eu_summer_time(date(2026, 3, 29), 60.0));
        assert!(eu_summer_time(date(2026, 10, 25), 59.0));
        assert!(!eu_summer_time(date(2026, 10, 25), 60.0));
        // 23:30 UTC in summer is 01:30 the next day in Sweden.
        assert_eq!(local(date(2026, 7, 1), 23.0 * 60.0 + 30.0, 1.0), (90.0, 1));
    }
}

//! The shot log: each shot the hunter took, with the solution the app gave
//! for it and what came of it. After a shot at game the record can also
//! hold where the hunter stood and the bearing to the animal, from which
//! the spot the animal stood on is worked out, with an area to search.
//!
//! Positions are on a sphere the size of the Earth: over the few hundred
//! metres of a shot that is far more exact than a compass.

use serde::{Deserialize, Serialize};

use crate::model::ClickUnit;

/// The Earth's mean radius, m (a sphere as large as the WGS84 ellipsoid).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;
/// How far off a phone's compass, or a bearing taken by eye, may be: a
/// phone's compass near steel, a rifle or a car is often 5–10° out.
pub const COMPASS_ERROR_DEGREES: f64 = 10.0;
/// How far off the range may be: 5 % of it (a range taken by eye or from a
/// map), but at least `RANGE_ERROR_MIN_M` (a rangefinder's few metres).
pub const RANGE_ERROR_FRACTION: f64 = 0.05;
pub const RANGE_ERROR_MIN_M: f64 = 5.0;
/// How far off a position is taken to be when the device didn't say (one
/// typed in, say).
pub const UNKNOWN_POSITION_ERROR_M: f64 = 10.0;
/// The smallest area to search: an animal stands on more than a point, and
/// the hunter needs room to look for hair and blood.
pub const MIN_SEARCH_RADIUS_M: f64 = 10.0;

/// What came of a shot.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Not written down (yet).
    #[default]
    NotRecorded,
    /// The shot hit.
    Hit,
    /// The shot missed.
    Miss,
}

impl Outcome {
    pub const ALL: [Outcome; 3] = [Outcome::NotRecorded, Outcome::Hit, Outcome::Miss];

    pub fn label(self) -> &'static str {
        match self {
            Outcome::NotRecorded => "Not recorded",
            Outcome::Hit => "Hit",
            Outcome::Miss => "Miss",
        }
    }
}

/// The weather the solution was worked out for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShotWeather {
    /// Air temperature.
    pub temperature_c: f64,
    /// The cartridges' (powder's) temperature.
    pub powder_temperature_c: f64,
    /// Air pressure where the hunter stood, as a barometer there reads it
    /// (not reduced to sea level).
    pub pressure_hpa: f64,
    /// Relative humidity.
    pub humidity_percent: f64,
}

/// The hold the app gave: the angle to come up and right by, and the same
/// in the rifle's clicks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShotHold {
    /// Up (positive) or down.
    pub elevation_mrad: f64,
    /// Right (positive) or left.
    pub windage_mrad: f64,
    /// The unit of the rifle's clicks then.
    pub click_unit: ClickUnit,
    /// One click, in `click_unit`.
    pub click_value: f64,
    /// Clicks up (positive) or down, rounded to whole clicks.
    pub elevation_clicks: i64,
    /// Clicks right (positive) or left.
    pub windage_clicks: i64,
}

/// Where the bullet was worked out to be at the range, aimed dead on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShotSolution {
    /// Above (positive) or below the line of sight.
    pub drop_cm: f64,
    /// Right (positive) or left of the line of sight: wind and spin drift.
    pub windage_cm: f64,
    /// Speed at the range.
    pub velocity_mps: f64,
    /// Energy at the range.
    pub energy_j: f64,
    /// Seconds from the muzzle to the range.
    pub time_of_flight_s: f64,
}

/// A place on the Earth, in degrees (WGS84, as GPS gives them).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeoPoint {
    /// North positive, south negative.
    pub latitude_degrees: f64,
    /// East positive, west negative.
    pub longitude_degrees: f64,
}

/// Where the hunter stood when shooting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShooterPosition {
    #[serde(flatten)]
    pub point: GeoPoint,
    /// How far off the position may be, as the device said; `None` when
    /// it was typed in.
    #[serde(default)]
    pub accuracy_m: Option<f64>,
}

/// Where the animal stood, worked out from the hunter's position, the
/// bearing and the range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnimalSpot {
    #[serde(flatten)]
    pub point: GeoPoint,
    /// How far around the spot to search: the bearing's, the range's and
    /// the position's possible errors together.
    pub search_radius_m: f64,
}

/// A shot taken, with the solution the app gave and what came of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Shot {
    /// The shot's id, unique among shots.
    pub id: String,
    /// When it was logged, in milliseconds since 1970 (UTC).
    pub taken_at: i64,
    /// The rifle shot with; it may have been deleted since.
    pub rifle_id: String,
    /// The rifle's name then, so the log reads after it is renamed or
    /// deleted.
    #[serde(default)]
    pub rifle_name: String,
    /// The bullet shot.
    pub bullet_id: String,
    /// The bullet's name then.
    #[serde(default)]
    pub bullet_name: String,
    /// What was shot at: one of the hunter's targets or the catalog's, or
    /// `None` for no target.
    #[serde(default)]
    pub target_id: Option<String>,
    /// The target's name then.
    #[serde(default)]
    pub target_name: String,
    /// The range along the line of sight.
    pub range_m: f64,
    /// The shot angle: up (positive) or down the line of sight.
    #[serde(default)]
    pub inclination_degrees: f64,
    /// The wind's speed.
    pub wind_speed_mps: f64,
    /// Where the wind came from, on a clock: 12 head-on, 3 from the right.
    pub wind_clock: f64,
    /// The weather the solution was worked out for.
    pub weather: ShotWeather,
    /// The hold the app gave.
    pub hold: ShotHold,
    /// The bullet's path at the range.
    pub solution: ShotSolution,
    /// Hit, miss or not written down.
    #[serde(default)]
    pub outcome: Outcome,
    /// Where a hit landed, in the hunter's words ("heart–lung").
    #[serde(default)]
    pub hit_where: String,
    /// Where the hunter stood, if they marked it.
    #[serde(default)]
    pub position: Option<ShooterPosition>,
    /// The direction from the hunter to the animal, degrees from true
    /// north, if taken.
    #[serde(default)]
    pub bearing_degrees: Option<f64>,
    /// Where the animal stood, from `position`, `bearing_degrees`, the
    /// range and the shot angle; `None` without a position and a bearing.
    #[serde(default)]
    pub animal_spot: Option<AnimalSpot>,
    /// Notes about it.
    #[serde(default)]
    pub notes: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl Shot {
    /// The level distance to the animal: the range along the line of
    /// sight, shortened by the shot angle.
    pub fn horizontal_range_m(&self) -> f64 {
        self.range_m * self.inclination_degrees.to_radians().cos()
    }

    /// Works out the animal's spot again from the position, the bearing
    /// and the range (or clears it when one is missing).
    pub fn update_spot(&mut self) {
        self.animal_spot = match (self.position, self.bearing_degrees) {
            (Some(position), Some(bearing)) => {
                let distance = self.horizontal_range_m();
                Some(AnimalSpot {
                    point: destination(position.point, bearing, distance),
                    search_radius_m: search_radius_m(distance, position.accuracy_m),
                })
            }
            _ => None,
        };
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
        let solution = &self.solution;
        let hold = &self.hold;
        let weather = &self.weather;
        let problem = if self.id.trim().is_empty() {
            Some("a shot needs an id")
        } else if self.taken_at < 0 {
            Some("the shot's time is before 1970")
        } else if !(self.range_m > 0.0 && self.range_m <= 5000.0) {
            Some("the shot's range must be between 0 and 5000 m")
        } else if !(-90.0..=90.0).contains(&self.inclination_degrees) {
            Some("the shot angle must be between -90° and 90°")
        } else if !((0.0..=100.0).contains(&self.wind_speed_mps)
            && (0.0..=12.0).contains(&self.wind_clock))
        {
            Some("the wind must be 0–100 m/s from a clock direction of 0–12")
        } else if !finite(&[
            weather.temperature_c,
            weather.powder_temperature_c,
            weather.pressure_hpa,
            weather.humidity_percent,
        ]) || weather.pressure_hpa <= 0.0
            || !(0.0..=100.0).contains(&weather.humidity_percent)
        {
            Some("the shot's weather isn't possible")
        } else if !finite(&[hold.elevation_mrad, hold.windage_mrad, hold.click_value])
            || hold.click_value <= 0.0
        {
            Some("the shot's hold isn't a number")
        } else if !finite(&[
            solution.drop_cm,
            solution.windage_cm,
            solution.velocity_mps,
            solution.energy_j,
            solution.time_of_flight_s,
        ]) || solution.velocity_mps < 0.0
            || solution.energy_j < 0.0
            || solution.time_of_flight_s < 0.0
        {
            Some("the shot's solution isn't possible")
        } else if self.position.is_some_and(|position| {
            !valid_point(position.point)
                || position
                    .accuracy_m
                    .is_some_and(|accuracy| !(0.0..=100_000.0).contains(&accuracy))
        }) {
            Some("the shooter's position must be a latitude of ±90° and a longitude of ±180°")
        } else if self
            .bearing_degrees
            .is_some_and(|bearing| !(0.0..=360.0).contains(&bearing))
        {
            Some("the bearing must be 0–360°")
        } else if self.animal_spot.is_some_and(|spot| {
            !valid_point(spot.point) || !(0.0..=100_000.0).contains(&spot.search_radius_m)
        }) {
            Some("the animal's spot must be a latitude of ±90° and a longitude of ±180°")
        } else {
            None
        };
        match problem {
            Some(problem) => Err(problem.to_owned()),
            None => Ok(()),
        }
    }
}

fn valid_point(point: GeoPoint) -> bool {
    (-90.0..=90.0).contains(&point.latitude_degrees)
        && (-180.0..=180.0).contains(&point.longitude_degrees)
}

/// The point `distance_m` from `start` in the direction `bearing_degrees`
/// (from true north, clockwise), along a great circle.
pub fn destination(start: GeoPoint, bearing_degrees: f64, distance_m: f64) -> GeoPoint {
    let latitude = start.latitude_degrees.to_radians();
    let longitude = start.longitude_degrees.to_radians();
    let bearing = bearing_degrees.to_radians();
    let angle = distance_m / EARTH_RADIUS_M;
    let end_latitude = (latitude.sin() * angle.cos()
        + latitude.cos() * angle.sin() * bearing.cos())
    .clamp(-1.0, 1.0)
    .asin();
    let end_longitude = longitude
        + (bearing.sin() * angle.sin() * latitude.cos())
            .atan2(angle.cos() - latitude.sin() * end_latitude.sin());
    GeoPoint {
        latitude_degrees: end_latitude.to_degrees(),
        // Back into -180..180 across the date line.
        longitude_degrees: (end_longitude.to_degrees() + 540.0).rem_euclid(360.0) - 180.0,
    }
}

/// The distance (m) and the bearing (degrees from true north, 0–360) from
/// one point to another, along a great circle.
pub fn distance_and_bearing(from: GeoPoint, to: GeoPoint) -> (f64, f64) {
    let (latitude_1, latitude_2) = (
        from.latitude_degrees.to_radians(),
        to.latitude_degrees.to_radians(),
    );
    let latitude_change = latitude_2 - latitude_1;
    let longitude_change = (to.longitude_degrees - from.longitude_degrees).to_radians();
    // The haversine formula: exact at short distances too.
    let half_chord = (latitude_change / 2.0).sin().powi(2)
        + latitude_1.cos() * latitude_2.cos() * (longitude_change / 2.0).sin().powi(2);
    let distance = 2.0 * EARTH_RADIUS_M * half_chord.sqrt().min(1.0).asin();
    let bearing = (longitude_change.sin() * latitude_2.cos())
        .atan2(
            latitude_1.cos() * latitude_2.sin()
                - latitude_1.sin() * latitude_2.cos() * longitude_change.cos(),
        )
        .to_degrees()
        .rem_euclid(360.0);
    (distance, bearing)
}

/// How far around the worked-out spot to search, for an animal
/// `distance_m` away (level distance) from a position good to
/// `position_accuracy_m` (`None`: `UNKNOWN_POSITION_ERROR_M`).
///
/// It adds up, as independent errors do (the root of the sum of squares):
/// - the bearing's: `COMPASS_ERROR_DEGREES` (±10°) to either side, which at
///   distance d is d × sin 10°, about 17 % of the distance;
/// - the range's: `RANGE_ERROR_FRACTION` (5 %) of it, but at least
///   `RANGE_ERROR_MIN_M` (5 m);
/// - the position's accuracy.
///
/// It is never less than `MIN_SEARCH_RADIUS_M` (10 m).
pub fn search_radius_m(distance_m: f64, position_accuracy_m: Option<f64>) -> f64 {
    let sideways = distance_m * COMPASS_ERROR_DEGREES.to_radians().sin();
    let along = (distance_m * RANGE_ERROR_FRACTION).max(RANGE_ERROR_MIN_M);
    let position = position_accuracy_m.unwrap_or(UNKNOWN_POSITION_ERROR_M);
    (sideways.powi(2) + along.powi(2) + position.powi(2))
        .sqrt()
        .max(MIN_SEARCH_RADIUS_M)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(latitude_degrees: f64, longitude_degrees: f64) -> GeoPoint {
        GeoPoint {
            latitude_degrees,
            longitude_degrees,
        }
    }

    #[test]
    fn a_degree_of_latitude_is_about_111_km() {
        let north = destination(point(0.0, 0.0), 0.0, 111_195.0);
        assert!((north.latitude_degrees - 1.0).abs() < 1e-4, "{north:?}");
        assert!(north.longitude_degrees.abs() < 1e-9);
        let (distance, bearing) = distance_and_bearing(point(0.0, 0.0), point(1.0, 0.0));
        assert!((distance - 111_195.0).abs() < 1.0, "{distance}");
        assert!(bearing.abs() < 1e-9);
    }

    #[test]
    fn east_at_sixty_north_a_degree_of_longitude_is_half_as_long() {
        // At 60° the parallels are half as long as the equator.
        let (distance, bearing) = distance_and_bearing(point(60.0, 15.0), point(60.0, 16.0));
        assert!((distance - 55_597.0).abs() < 50.0, "{distance}");
        // A great circle east starts a little north of east.
        assert!((bearing - 89.57).abs() < 0.05, "{bearing}");
    }

    #[test]
    fn going_there_and_measuring_back_agree_at_shooting_distances() {
        let start = point(64.75, 20.95);
        for bearing in [0.0, 52.0, 90.0, 135.0, 180.0, 270.0, 359.0] {
            for distance in [25.0, 128.0, 300.0, 1000.0] {
                let end = destination(start, bearing, distance);
                let (measured, measured_bearing) = distance_and_bearing(start, end);
                assert!((measured - distance).abs() < 1e-3, "{distance} {measured}");
                let off = (measured_bearing - bearing + 540.0).rem_euclid(360.0) - 180.0;
                assert!(off.abs() < 1e-6, "{bearing} {measured_bearing}");
            }
        }
        // And the way back points the other way.
        let end = destination(start, 52.0, 128.0);
        let (_, back) = distance_and_bearing(end, start);
        assert!((back - 232.0).abs() < 0.01, "{back}");
    }

    #[test]
    fn the_date_line_and_the_pole_are_handled() {
        let across = destination(point(0.0, 179.9995), 90.0, 111.0);
        assert!(across.longitude_degrees < -179.0, "{across:?}");
        let (distance, _) = distance_and_bearing(point(0.0, 179.9995), across);
        assert!((distance - 111.0).abs() < 0.01);
        let over = destination(point(89.9995, 0.0), 0.0, 111.0);
        assert!(over.latitude_degrees <= 90.0 && over.latitude_degrees > 89.99);
        let here = point(61.0, 14.5);
        assert_eq!(distance_and_bearing(here, here).0, 0.0);
    }

    #[test]
    fn the_search_area_grows_with_distance() {
        // Close: the least area.
        assert_eq!(search_radius_m(20.0, Some(3.0)), MIN_SEARCH_RADIUS_M);
        // 150 m with a 5 m fix: sin 10° × 150 = 26.0, 7.5 along, 5.
        let radius = search_radius_m(150.0, Some(5.0));
        assert!((radius - 27.56).abs() < 0.05, "{radius}");
        // An unknown position counts as 10 m.
        assert!(search_radius_m(150.0, None) > radius);
        assert!(search_radius_m(300.0, Some(5.0)) > 2.0 * radius * 0.95);
    }
}

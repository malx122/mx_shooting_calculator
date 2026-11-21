//! The statistics shown while marking: worked out again only when the
//! marks change.

use ballistics_store::{Calibration, Marks};

use crate::groups::measure::{self, MeasureError, Measured};
use crate::i18n::{tr, trf};
use crate::units::{Quantity, UnitPrefs};

/// What the statistics were worked out from.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    marks: Marks,
    calibration: Option<Calibration>,
    calibre_mm: Option<f64>,
    distance_m: Option<f64>,
}

/// The last statistics and what they came from.
#[derive(Default)]
pub struct LiveStats {
    key: Option<Key>,
    result: Option<Result<Measured, MeasureError>>,
}

impl LiveStats {
    /// The statistics of these marks, worked out when they changed.
    pub fn get(
        &mut self,
        marks: &Marks,
        calibration: Option<&Calibration>,
        calibre_mm: Option<f64>,
        distance_m: Option<f64>,
    ) -> &Result<Measured, MeasureError> {
        let fresh = self.key.as_ref().is_some_and(|key| {
            key.marks == *marks
                && key.calibration.as_ref() == calibration
                && key.calibre_mm == calibre_mm
                && key.distance_m == distance_m
        });
        if !fresh || self.result.is_none() {
            self.result = Some(measure::measure(marks, calibration, calibre_mm, distance_m));
            self.key = Some(Key {
                marks: marks.clone(),
                calibration: calibration.copied(),
                calibre_mm,
                distance_m,
            });
        }
        self.result.as_ref().expect("just worked out")
    }

    /// How many times the statistics were worked out, for tests.
    #[cfg(test)]
    pub fn is_cached_for(&self, marks: &Marks) -> bool {
        self.key.as_ref().is_some_and(|key| key.marks == *marks)
    }
}

/// A size on the target as the strip shows it: as an angle at a known
/// distance, on the target otherwise, in the shooter's units.
pub fn size_text(units: &UnitPrefs, mm: f64, distance_m: Option<f64>) -> String {
    match distance_m {
        Some(distance) if distance > 0.0 => units.format(
            Quantity::GroupAngle,
            groups_core::angles::mrad(mm, distance),
        ),
        _ => units.format(Quantity::GroupSize, mm),
    }
}

/// The strip's line: how many shots, and from the second counted shot the
/// extreme spread and mean radius.
pub fn strip_text(
    units: &UnitPrefs,
    result: &Result<Measured, MeasureError>,
    shots: usize,
    distance_m: Option<f64>,
) -> String {
    let count = |n: usize| {
        if n == 1 {
            tr("1 shot").to_owned()
        } else {
            trf!("{} shots", n)
        }
    };
    match result {
        Err(MeasureError::NoScale) => {
            trf!("{} · no scale, so no sizes", count(shots))
        }
        Err(MeasureError::Scale(_)) => {
            trf!("{} · the scale can't be used", count(shots))
        }
        Err(MeasureError::OffTarget) => {
            trf!(
                "{} · a shot is beyond the straightened target",
                count(shots)
            )
        }
        Err(MeasureError::Stats(_)) => count(shots),
        Ok(measured) => {
            let counted = measured.counted.len();
            let mut text = count(counted);
            let left_out = shots - counted;
            if left_out > 0 {
                text = trf!("{} (+{} left out)", text, left_out);
            }
            if let Some(stats) = &measured.stats
                && let Some(spread) = &stats.spread
            {
                text = trf!(
                    "{} · ES {} · MR {}",
                    text,
                    size_text(units, spread.extreme_spread.centre_to_centre, distance_m),
                    size_text(units, stats.mean_radius, distance_m)
                );
            }
            text
        }
    }
}

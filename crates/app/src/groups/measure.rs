//! A group's marks turned into millimetres on the target, and its statistics.

use ballistics_store::{Calibration, Marks};
use groups_core::{CalibrationError, GroupStats, Options, Point, StatsError};

/// A group's shots on the target in millimetres, x to the right and y up.
pub struct Measured {
    /// Every marked shot, in marking order.
    pub shots_mm: Vec<Point>,
    /// The aim point, when one is marked.
    pub aim_mm: Option<Point>,
    /// Indices of the shots the group's statistics count: neither cold bore
    /// nor called fliers.
    pub counted: Vec<usize>,
    /// Indices of cold-bore shots, which never count in the group.
    pub cold_bore: Vec<usize>,
    /// Indices of the shots the shooter called as fliers.
    pub called_fliers: Vec<usize>,
    /// Statistics of the counted shots; `None` when no shot counts.
    pub stats: Option<GroupStats>,
    /// Statistics with the called fliers put back; `None` when there are
    /// none.
    pub with_called_fliers: Option<GroupStats>,
}

/// Why a group can't be measured.
#[derive(Debug)]
pub enum MeasureError {
    /// A photo group whose photo has no scale yet.
    NoScale,
    /// The photo's scale can't be used.
    Scale(CalibrationError),
    /// A point maps beyond the straightened target (behind the vanishing
    /// line of a four-corner scale).
    OffTarget,
    /// The statistics refused the shots.
    Stats(StatsError),
}

/// The photo-to-target mapping for a stored calibration.
pub fn scale(calibration: &Calibration) -> Result<groups_core::Calibration, CalibrationError> {
    let point = |[x, y]: [f64; 2]| Point::new(x, y);
    match calibration {
        Calibration::TwoPoint {
            a_px,
            b_px,
            distance_mm,
        } => groups_core::Calibration::two_points(point(*a_px), point(*b_px), *distance_mm),
        Calibration::FourCorner {
            corners_px,
            width_mm,
            height_mm,
        } => groups_core::Calibration::four_corners(corners_px.map(point), *width_mm, *height_mm),
    }
}

/// Measures a group: its shots in millimetres and the statistics of the
/// shots that count. `calibration` is the photo's, ignored for grid groups.
pub fn measure(
    marks: &Marks,
    calibration: Option<&Calibration>,
    calibre_mm: Option<f64>,
    distance_m: Option<f64>,
) -> Result<Measured, MeasureError> {
    struct Mark {
        at: Point,
        cold_bore: bool,
        called_flier: bool,
    }
    let (aim_mm, marks) = match marks {
        Marks::Photo(photo) => {
            let mapping =
                scale(calibration.ok_or(MeasureError::NoScale)?).map_err(MeasureError::Scale)?;
            let to_mm = |[x, y]: [f64; 2]| {
                mapping
                    .to_mm(Point::new(x, y))
                    .ok_or(MeasureError::OffTarget)
            };
            let aim = photo.aim_px.map(to_mm).transpose()?;
            let shots = photo
                .shots
                .iter()
                .map(|shot| {
                    Ok(Mark {
                        at: to_mm([shot.x_px, shot.y_px])?,
                        cold_bore: shot.cold_bore,
                        called_flier: shot.called_flier,
                    })
                })
                .collect::<Result<Vec<_>, MeasureError>>()?;
            (aim, shots)
        }
        Marks::Grid(grid) => (
            grid.aim_mm.map(|[x, y]| Point::new(x, y)),
            grid.shots
                .iter()
                .map(|shot| Mark {
                    at: Point::new(shot.x_mm, shot.y_mm),
                    cold_bore: shot.cold_bore,
                    called_flier: shot.called_flier,
                })
                .collect(),
        ),
    };
    let indices = |keep: &dyn Fn(&Mark) -> bool| -> Vec<usize> {
        (0..marks.len()).filter(|&i| keep(&marks[i])).collect()
    };
    let counted = indices(&|mark| !mark.cold_bore && !mark.called_flier);
    let cold_bore = indices(&|mark| mark.cold_bore);
    let called_fliers = indices(&|mark| mark.called_flier && !mark.cold_bore);
    let options = Options {
        aim: aim_mm,
        calibre_mm,
        distance_m,
    };
    let stats_of = |chosen: &[usize]| -> Result<Option<GroupStats>, MeasureError> {
        if chosen.is_empty() {
            return Ok(None);
        }
        let points: Vec<Point> = chosen.iter().map(|&i| marks[i].at).collect();
        GroupStats::new(&points, options)
            .map(Some)
            .map_err(MeasureError::Stats)
    };
    let stats = stats_of(&counted)?;
    let with_called_fliers = if called_fliers.is_empty() {
        None
    } else {
        let mut all: Vec<usize> = counted.iter().chain(&called_fliers).copied().collect();
        all.sort_unstable();
        stats_of(&all)?
    };
    Ok(Measured {
        shots_mm: marks.iter().map(|mark| mark.at).collect(),
        aim_mm,
        counted,
        cold_bore,
        called_fliers,
        stats,
        with_called_fliers,
    })
}

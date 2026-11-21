//! Shot groups: shots marked on a photo of a target or on a blank grid,
//! with what was shot and where; ladders that tie groups of a load
//! development together; and saved analyses of several groups.
//!
//! Marks stay where they were made: in the photo's pixels, or in
//! millimetres on a grid. The photo's calibration (on its `Photo` record,
//! shared by every group on it) turns pixels into millimetres on the
//! target when the statistics are worked out, so a better scale never
//! moves or loses a shot.

use serde::{Deserialize, Serialize};

use crate::model::{NAME_MAX, first_failure, new_id};
use crate::photos::is_photo_id;
use crate::sessions::Conditions;

/// The most shots one group may hold.
pub const MAX_SHOTS: usize = 1000;
/// The most tags one group may have.
pub const MAX_TAGS: usize = 50;
/// The longest tag.
pub const TAG_MAX: usize = 60;
/// The farthest a pixel coordinate may lie from the photo's corner.
const PIXELS_MAX: f64 = 1_000_000.0;
/// The farthest a grid coordinate or a calibration length may reach, mm.
const MILLIMETRES_MAX: f64 = 10_000.0;

/// How a photo's pixels map to millimetres on the target. Points are in the
/// photo's pixels: x to the right and y down from the photo's top-left
/// corner, so the middle of the top-left pixel is `[0.5, 0.5]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Calibration {
    /// Two points a known distance apart on the target, such as lines of
    /// its grid: gives the scale, the photo taken to face the target
    /// squarely.
    TwoPoint {
        /// The first point, `[x, y]`.
        a_px: [f64; 2],
        /// The second point, `[x, y]`.
        b_px: [f64; 2],
        /// How far apart the two points are on the target.
        distance_mm: f64,
    },
    /// The corners of a rectangle of known size on the target, such as the
    /// sheet's edges: gives the scale and straightens a photo taken at an
    /// angle.
    FourCorner {
        /// The rectangle's corners as `[x, y]`: its top left, top right,
        /// bottom right and bottom left.
        corners_px: [[f64; 2]; 4],
        /// The rectangle's width: from its top-left to its top-right corner.
        width_mm: f64,
        /// The rectangle's height: from its top-right to its bottom-right
        /// corner.
        height_mm: f64,
    },
}

impl Calibration {
    pub(crate) fn problem(&self) -> Option<&'static str> {
        let length = |mm: f64| mm > 0.0 && mm <= MILLIMETRES_MAX;
        match self {
            Calibration::TwoPoint {
                a_px,
                b_px,
                distance_mm,
            } => {
                if !on_photo(*a_px) || !on_photo(*b_px) {
                    Some("the scale's points must be on the photo")
                } else if a_px == b_px {
                    Some("the scale's two points must differ")
                } else if !length(*distance_mm) {
                    Some("the scale's distance must be more than 0 and at most 10 m")
                } else {
                    None
                }
            }
            Calibration::FourCorner {
                corners_px,
                width_mm,
                height_mm,
            } => {
                let distinct = (0..4).all(|i| (i + 1..4).all(|j| corners_px[i] != corners_px[j]));
                if !corners_px.iter().all(|corner| on_photo(*corner)) {
                    Some("the rectangle's corners must be on the photo")
                } else if !distinct {
                    Some("the rectangle's four corners must differ")
                } else if !length(*width_mm) || !length(*height_mm) {
                    Some("the rectangle's sides must be more than 0 and at most 10 m")
                } else {
                    None
                }
            }
        }
    }
}

fn on_photo(point: [f64; 2]) -> bool {
    point
        .iter()
        .all(|value| value.is_finite() && value.abs() <= PIXELS_MAX)
}

fn on_grid(point: [f64; 2]) -> bool {
    point
        .iter()
        .all(|value| value.is_finite() && value.abs() <= MILLIMETRES_MAX)
}

/// A shot marked on a photo.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PhotoShot {
    /// The hole's centre: pixels from the photo's left edge.
    pub x_px: f64,
    /// The hole's centre: pixels down from the photo's top edge.
    pub y_px: f64,
    /// The first shot from a cold, clean barrel.
    #[serde(default)]
    pub cold_bore: bool,
    /// The shooter knew the shot was bad as it broke; statistics can leave
    /// it out.
    #[serde(default)]
    pub called_flier: bool,
    /// Its muzzle velocity, if measured.
    #[serde(default)]
    pub velocity_mps: Option<f64>,
}

/// A shot placed on a blank grid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GridShot {
    /// Millimetres right (positive) or left of the grid's centre.
    pub x_mm: f64,
    /// Millimetres up (positive) or down from the grid's centre.
    pub y_mm: f64,
    /// The first shot from a cold, clean barrel.
    #[serde(default)]
    pub cold_bore: bool,
    /// The shooter knew the shot was bad as it broke; statistics can leave
    /// it out.
    #[serde(default)]
    pub called_flier: bool,
    /// Its muzzle velocity, if measured.
    #[serde(default)]
    pub velocity_mps: Option<f64>,
}

fn velocity_ok(velocity: Option<f64>) -> bool {
    velocity.is_none_or(|mps| (100.0..=1500.0).contains(&mps))
}

/// How a group was last shown while marking, on this device. It isn't
/// part of the group's record: each device keeps its own (see
/// `Store::group_view`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupView {
    /// How many times the photo or grid is enlarged: 1 shows it whole.
    pub zoom: f64,
    /// The point at the middle of the view, `[x, y]` in the marks' own
    /// units: photo pixels, or grid millimetres.
    pub centre: [f64; 2],
}

impl GroupView {
    pub fn validate(&self) -> Result<(), String> {
        if (0.01..=1000.0).contains(&self.zoom) && on_photo(self.centre) {
            Ok(())
        } else {
            Err("the view isn't possible".into())
        }
    }
}

/// Shots marked on a photo of the target, in its pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PhotoMarks {
    /// The photo (one of the file's `photos`), whose `calibration` gives the
    /// scale.
    pub photo_id: String,
    /// The point aimed at, `[x, y]` in pixels; `null` without one, which
    /// leaves out the group's correction and zero checks.
    #[serde(default)]
    pub aim_px: Option<[f64; 2]>,
    /// The shots, in the order they were marked.
    #[serde(default)]
    pub shots: Vec<PhotoShot>,
}

/// Shots placed on a blank grid at true scale, in millimetres from its
/// centre: x to the right, y up. For targets that weren't photographed:
/// scored paper, or positions from an electronic target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GridMarks {
    /// The grid's width.
    pub width_mm: f64,
    /// The grid's height.
    pub height_mm: f64,
    /// The point aimed at, `[x, y]` in mm; `null` without one. Usually the
    /// centre, `[0, 0]`.
    #[serde(default)]
    pub aim_mm: Option<[f64; 2]>,
    /// The shots, in the order they were placed.
    #[serde(default)]
    pub shots: Vec<GridShot>,
}

/// Where a group's shots are marked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum Marks {
    /// On a photo of the target.
    Photo(PhotoMarks),
    /// On a blank grid.
    Grid(GridMarks),
}

impl Marks {
    fn problem(&self) -> Option<&'static str> {
        let too_many = |count: usize| count > MAX_SHOTS;
        match self {
            Marks::Photo(marks) => {
                if !is_photo_id(&marks.photo_id) {
                    Some("a group's photo must be named by its id")
                } else if !marks.aim_px.is_none_or(on_photo) {
                    Some("the aim point must be on the photo")
                } else if too_many(marks.shots.len()) {
                    Some("a group can hold at most 1000 shots")
                } else if !marks
                    .shots
                    .iter()
                    .all(|shot| on_photo([shot.x_px, shot.y_px]))
                {
                    Some("every shot must be on the photo")
                } else if !marks
                    .shots
                    .iter()
                    .all(|shot| velocity_ok(shot.velocity_mps))
                {
                    Some("shot velocities must be 100 to 1500 m/s")
                } else {
                    None
                }
            }
            Marks::Grid(marks) => {
                let side = |mm: f64| (10.0..=MILLIMETRES_MAX).contains(&mm);
                if !side(marks.width_mm) || !side(marks.height_mm) {
                    Some("a grid must be 10 mm to 10 m each way")
                } else if !marks.aim_mm.is_none_or(on_grid) {
                    Some("the aim point must be within 10 m of the grid's centre")
                } else if too_many(marks.shots.len()) {
                    Some("a group can hold at most 1000 shots")
                } else if !marks
                    .shots
                    .iter()
                    .all(|shot| on_grid([shot.x_mm, shot.y_mm]))
                {
                    Some("every shot must be within 10 m of the grid's centre")
                } else if !marks
                    .shots
                    .iter()
                    .all(|shot| velocity_ok(shot.velocity_mps))
                {
                    Some("shot velocities must be 100 to 1500 m/s")
                } else {
                    None
                }
            }
        }
    }
}

/// A group of shots: where they went, and what, where and how they were
/// shot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Group {
    /// The group's id, unique among groups.
    pub id: String,
    /// When it was shot, in milliseconds since 1970 (UTC).
    pub taken_at: i64,
    /// Where its shots are marked.
    pub marks: Marks,
    /// The rifle shot (one of the file's `rifles`), if given.
    #[serde(default)]
    pub rifle_id: Option<String>,
    /// The rifle's barrel (one of the file's `barrels`), if given.
    #[serde(default)]
    pub barrel_id: Option<String>,
    /// The bullet or load shot (one of the file's `bullets`), if given.
    #[serde(default)]
    pub bullet_id: Option<String>,
    /// The ladder (one of the file's `ladders`) this group is a rung of.
    #[serde(default)]
    pub ladder_id: Option<String>,
    /// The rung's value, in the unit the ladder's `variable` names: the
    /// charge in grams, or the seating's CBTO in mm.
    #[serde(default)]
    pub rung_value: Option<f64>,
    /// The handload's powder charge when the group was saved, in grams, as
    /// its bullet's `recipe` gave it; `null` when it gave none (a factory
    /// load, or a group saved before this was kept).
    #[serde(default)]
    pub charge_g: Option<f64>,
    /// The handload's cartridge base to ogive (CBTO) when the group was
    /// saved, mm, as its bullet's `recipe` gave it; `null` when it gave none.
    #[serde(default)]
    pub cbto_mm: Option<f64>,
    /// The distance to the target; `null` when not given, which leaves out
    /// sizes as angles.
    #[serde(default)]
    pub distance_m: Option<f64>,
    /// The session (one of the file's `sessions`) it was shot in.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Conditions that differ from the session's; what is `null` here is
    /// the session's.
    #[serde(default)]
    pub conditions: Conditions,
    /// Muzzle velocities measured for rounds that match no marked shot
    /// (foulers, shots off the paper), in the order they came.
    #[serde(default)]
    pub extra_velocities_mps: Vec<f64>,
    /// What was dialled on the scope, right and up positive, compared with
    /// the zero. Zero checks take it off the group's impact.
    #[serde(default)]
    pub dialled_offset_mrad: [f64; 2],
    /// Whether zero checks count this group.
    #[serde(default = "yes")]
    pub include_in_zero: bool,
    /// Whether saving the group logs its shots as rounds fired (an entry of
    /// `round_log` naming the group).
    #[serde(default = "yes")]
    pub count_rounds: bool,
    /// The hunter's tags, e.g. `practice`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The hunter's notes.
    #[serde(default)]
    pub notes: String,
    /// Kept, but out of the library's lists.
    #[serde(default)]
    pub archived: bool,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

fn yes() -> bool {
    true
}

impl Group {
    /// A group with no shots yet, marked on a photo.
    pub fn on_photo(photo_id: &str, taken_at: i64) -> Self {
        Group::new(
            taken_at,
            Marks::Photo(PhotoMarks {
                photo_id: photo_id.to_owned(),
                aim_px: None,
                shots: Vec::new(),
            }),
        )
    }

    /// A group with no shots yet, on a blank grid of this size aimed at its
    /// centre.
    pub fn on_grid(width_mm: f64, height_mm: f64, taken_at: i64) -> Self {
        Group::new(
            taken_at,
            Marks::Grid(GridMarks {
                width_mm,
                height_mm,
                aim_mm: Some([0.0, 0.0]),
                shots: Vec::new(),
            }),
        )
    }

    fn new(taken_at: i64, marks: Marks) -> Self {
        Group {
            id: new_id(),
            taken_at,
            marks,
            rifle_id: None,
            barrel_id: None,
            bullet_id: None,
            ladder_id: None,
            rung_value: None,
            charge_g: None,
            cbto_mm: None,
            distance_m: None,
            session_id: None,
            conditions: Conditions::default(),
            extra_velocities_mps: Vec::new(),
            dialled_offset_mrad: [0.0, 0.0],
            include_in_zero: true,
            count_rounds: true,
            tags: Vec::new(),
            notes: String::new(),
            archived: false,
            updated_at: 0,
        }
    }

    /// The photo the shots are marked on; `None` on a grid.
    pub fn photo_id(&self) -> Option<&str> {
        match &self.marks {
            Marks::Photo(marks) => Some(&marks.photo_id),
            Marks::Grid(_) => None,
        }
    }

    pub fn shot_count(&self) -> usize {
        match &self.marks {
            Marks::Photo(marks) => marks.shots.len(),
            Marks::Grid(marks) => marks.shots.len(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let tags_ok = self.tags.len() <= MAX_TAGS
            && self.tags.iter().all(|tag| {
                !tag.trim().is_empty() && tag.trim() == tag && tag.chars().count() <= TAG_MAX
            });
        let checks = [
            (!self.id.trim().is_empty(), "a group needs an id"),
            (self.taken_at >= 0, "the group's time is before 1970"),
            (
                self.marks.problem().is_none(),
                self.marks.problem().unwrap_or_default(),
            ),
            (
                self.distance_m.is_none_or(|m| (1.0..=5000.0).contains(&m)),
                "the distance must be 1 to 5000 m",
            ),
            (
                self.rung_value.is_none_or(f64::is_finite),
                "the rung's value isn't a number",
            ),
            (
                self.charge_g.is_none_or(|g| (0.01..=30.0).contains(&g)),
                "the powder charge must be 0.01 to 30 g",
            ),
            (
                self.cbto_mm.is_none_or(|mm| (5.0..=200.0).contains(&mm)),
                "the CBTO must be 5 to 200 mm",
            ),
            (
                self.conditions.problem().is_none(),
                self.conditions.problem().unwrap_or_default(),
            ),
            (
                self.extra_velocities_mps.len() <= MAX_SHOTS
                    && self
                        .extra_velocities_mps
                        .iter()
                        .all(|mps| velocity_ok(Some(*mps))),
                "velocities must be 100 to 1500 m/s, at most 1000 of them",
            ),
            (
                self.dialled_offset_mrad
                    .iter()
                    .all(|mrad| mrad.is_finite() && mrad.abs() <= 100.0),
                "the dialled offset must be at most 100 mrad each way",
            ),
            (
                tags_ok,
                "tags must be 1 to 60 characters without spaces at the ends, at most 50",
            ),
        ];
        first_failure(&checks, "")
    }
}

/// What a ladder varies from rung to rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LadderVariable {
    /// The powder charge; rung values are grams.
    Charge,
    /// The seating depth; rung values are the cartridge's base-to-ogive
    /// length (CBTO), mm.
    Seating,
}

/// A load development: groups of one bullet in one rifle, each a rung at a
/// different charge or seating depth (the groups' `ladder_id` and
/// `rung_value`). Zero checks and trends leave its groups out unless the
/// hunter adds them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Ladder {
    /// The ladder's id, unique among ladders.
    pub id: String,
    /// The hunter's name for it; may be empty.
    #[serde(default)]
    pub name: String,
    /// The rifle (one of the file's `rifles`).
    pub rifle_id: String,
    /// The bullet loaded (one of the file's `bullets`), whose recipe the
    /// rungs vary.
    pub bullet_id: String,
    /// What changes from rung to rung.
    pub variable: LadderVariable,
    /// The hunter's notes.
    #[serde(default)]
    pub notes: String,
    /// The load (one of the file's `bullets`) made from the rung the hunter
    /// kept, once there is one.
    #[serde(default)]
    pub promoted_bullet_id: Option<String>,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl Ladder {
    pub fn new(rifle_id: &str, bullet_id: &str, variable: LadderVariable) -> Self {
        Ladder {
            id: new_id(),
            name: String::new(),
            rifle_id: rifle_id.to_owned(),
            bullet_id: bullet_id.to_owned(),
            variable,
            notes: String::new(),
            promoted_bullet_id: None,
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a ladder needs an id"),
            (
                !self.rifle_id.is_empty() && !self.bullet_id.is_empty(),
                "a ladder is of a bullet in a rifle",
            ),
            (
                self.name.chars().count() <= NAME_MAX,
                "the name is too long",
            ),
        ];
        first_failure(&checks, &self.name)
    }
}

/// What a saved analysis does with its groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisKind {
    /// Groups side by side, tested against each other.
    Compare,
    /// The groups' shots pooled into one group.
    Composite,
    /// A ladder's rungs, ranked.
    Ladder,
}

/// An analysis of several groups the hunter saved, worked out again from
/// the groups whenever it is opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Analysis {
    /// The analysis's id, unique among analyses.
    pub id: String,
    /// What it does.
    pub kind: AnalysisKind,
    /// The hunter's name for it.
    #[serde(default)]
    pub name: String,
    /// Its groups (of the file's `groups`), in the hunter's order; ones
    /// deleted since are skipped.
    #[serde(default)]
    pub group_ids: Vec<String>,
    /// How the app shows it, as the app's own JSON object; a reader may
    /// ignore it.
    #[serde(default = "no_settings")]
    pub settings: serde_json::Value,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

fn no_settings() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// The largest `settings` an analysis may have, as JSON.
const SETTINGS_MAX_BYTES: usize = 64 * 1024;

impl Analysis {
    pub fn new(kind: AnalysisKind, name: impl Into<String>, group_ids: Vec<String>) -> Self {
        Analysis {
            id: new_id(),
            kind,
            name: name.into(),
            group_ids,
            settings: no_settings(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "an analysis needs an id"),
            (
                self.name.chars().count() <= NAME_MAX,
                "the name is too long",
            ),
            (
                self.group_ids.len() <= 10_000 && self.group_ids.iter().all(|id| !id.is_empty()),
                "an analysis names at most 10 000 groups",
            ),
            (
                self.settings.is_object() && self.settings.to_string().len() <= SETTINGS_MAX_BYTES,
                "an analysis's settings must be a JSON object of at most 64 KiB",
            ),
        ];
        first_failure(&checks, &self.name)
    }
}

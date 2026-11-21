//! Which groups each analysis counts, and their numbers in the units the
//! maths takes: angles in mrad, charges in g, seating in mm. Nothing here
//! draws, so every rule is tested on its own.

use std::rc::Rc;

use ballistics_store::{
    Analysis, AnalysisKind, Barrel, Bullet, Group, Ladder, LadderVariable, Marks, Rifle, RifleLoad,
    RoundCounts, ZeroReset,
};
use groups_core::{
    AnalysisError, BoxPlot, Composite, CompositeMode, GroupStats, Interval, LadderGroup, LadderKey,
    LadderOptions, Point, RankedGroup, Ranking, ShotGroup, SigmaComparison, TemperatureSensitivity,
    TemperatureSession, Unrankable, VelocityStats, ZeroCheck, ZeroGroup,
};
use serde::{Deserialize, Serialize};

use crate::app::Data;
use crate::groups::measure::Measured;
use crate::groups::{measure_group, velocities};
use crate::units::Quantity;

const DAY_MS: f64 = 86_400_000.0;

/// What a filter lets through: everything, or the groups of one record.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pick {
    All,
    One(String),
}

impl Pick {
    pub fn lets(&self, id: Option<&str>) -> bool {
        match self {
            Pick::All => true,
            Pick::One(wanted) => id == Some(wanted.as_str()),
        }
    }

    pub fn one(&self) -> Option<&str> {
        match self {
            Pick::All => None,
            Pick::One(id) => Some(id),
        }
    }
}

fn group_has_aim(group: &Group) -> bool {
    match &group.marks {
        Marks::Photo(marks) => marks.aim_px.is_some(),
        Marks::Grid(marks) => marks.aim_mm.is_some(),
    }
}

/// A group's numbers, as the library measures it.
fn measured(group: &Group, data: &Data) -> Option<Rc<Measured>> {
    measure_group(group, data).ok().map(Rc::new)
}

/// The counted shots of a group, mm.
fn counted_shots(measured: &Measured) -> Vec<Point> {
    measured
        .counted
        .iter()
        .map(|&i| measured.shots_mm[i])
        .collect()
}

// --- Zero check ---------------------------------------------------------

/// Why a zero check leaves a group out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ineligible {
    OtherRifle,
    OtherBarrel,
    OtherLoad,
    /// "Include in zero check" is off.
    NotIncluded,
    /// A rung of a ladder.
    Ladder,
    /// Shot before the rifle's latest zero reset.
    BeforeReset,
    NoAim,
    NoDistance,
    /// Shot further than 5% (and 5 m) from the rifle's zero range.
    NotAtZeroRange,
    /// Its barrel's record was deleted, and the zero resets with it.
    BarrelRemoved,
    /// Its shots can't be measured (no scale, or no shot counts).
    NotMeasured,
}

/// How far from the zero range a group may be shot and still count in a
/// zero check: 5% of it, and at least 5 m.
pub fn zero_range_tolerance_m(zero_range_m: f64) -> f64 {
    (zero_range_m * 0.05).max(5.0)
}

/// Whether `distance_m` is the zero range, as a zero check takes it.
pub fn at_zero_range(distance_m: f64, zero_range_m: f64) -> bool {
    (distance_m - zero_range_m).abs() <= zero_range_tolerance_m(zero_range_m)
}

/// The rifle, barrel and load a zero check is of.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ZeroScope {
    pub rifle_id: String,
    pub barrel: Pick,
    pub load: Pick,
}

impl ZeroScope {
    /// The last used rifle, with its fitted barrel and the load it last
    /// shot; `None` without a rifle.
    pub fn default_for(data: &Data) -> Option<ZeroScope> {
        let newest = data
            .groups
            .groups
            .iter()
            .filter(|group| {
                group
                    .rifle_id
                    .as_ref()
                    .is_some_and(|id| data.rifles.iter().any(|rifle| &rifle.id == id))
            })
            .max_by_key(|group| group.taken_at);
        let rifle = match newest {
            Some(group) => data
                .rifles
                .iter()
                .find(|rifle| group.rifle_id.as_ref() == Some(&rifle.id))?,
            None => data.rifles.first()?,
        };
        let barrel = rifle.fitted_barrel_id.clone().map_or(Pick::All, Pick::One);
        let load = newest
            .and_then(|group| group.bullet_id.clone())
            .map_or(Pick::All, Pick::One);
        Some(ZeroScope {
            rifle_id: rifle.id.clone(),
            barrel,
            load,
        })
    }

    /// Whether the check pools what a hunter would keep apart: every
    /// barrel of a rifle that has some, or every load.
    pub fn pools_everything(&self, data: &Data) -> bool {
        let has_barrels = data
            .groups
            .barrels
            .iter()
            .any(|barrel| barrel.rifle_id == self.rifle_id);
        (self.barrel == Pick::All && has_barrels) || self.load == Pick::All
    }
}

/// The rifle's latest zero reset that applies to a group shot through
/// `barrel_id`: one naming that barrel, or naming none.
pub fn latest_reset<'a>(
    resets: &'a [ZeroReset],
    rifle_id: &str,
    barrel_id: Option<&str>,
) -> Option<&'a ZeroReset> {
    resets
        .iter()
        .filter(|reset| {
            reset.rifle_id == rifle_id
                && (reset.barrel_id.is_none() || reset.barrel_id.as_deref() == barrel_id)
        })
        .max_by_key(|reset| reset.at)
}

/// Whether a zero check of `scope` counts `group`, before its shots are
/// measured: the rifle, barrel and load match, its barrel still exists,
/// it has an aim and a distance at the rifle's zero range (`zero_range_m`,
/// when the rifle is known), "Include in zero check" is on, it is no
/// ladder's rung, and it was shot after the rifle's latest zero reset (for
/// its barrel, when the reset names one).
pub fn zero_eligibility(
    group: &Group,
    scope: &ZeroScope,
    resets: &[ZeroReset],
    zero_range_m: Option<f64>,
    barrels: &[Barrel],
) -> Result<(), Ineligible> {
    if group.rifle_id.as_deref() != Some(scope.rifle_id.as_str()) {
        return Err(Ineligible::OtherRifle);
    }
    if !scope.barrel.lets(group.barrel_id.as_deref()) {
        return Err(Ineligible::OtherBarrel);
    }
    if !scope.load.lets(group.bullet_id.as_deref()) {
        return Err(Ineligible::OtherLoad);
    }
    if group
        .barrel_id
        .as_ref()
        .is_some_and(|id| !barrels.iter().any(|barrel| &barrel.id == id))
    {
        return Err(Ineligible::BarrelRemoved);
    }
    if group.ladder_id.is_some() {
        return Err(Ineligible::Ladder);
    }
    if !group.include_in_zero {
        return Err(Ineligible::NotIncluded);
    }
    if !group_has_aim(group) {
        return Err(Ineligible::NoAim);
    }
    let Some(distance_m) = group.distance_m else {
        return Err(Ineligible::NoDistance);
    };
    if zero_range_m.is_some_and(|zero_range_m| !at_zero_range(distance_m, zero_range_m)) {
        return Err(Ineligible::NotAtZeroRange);
    }
    if latest_reset(resets, &scope.rifle_id, group.barrel_id.as_deref())
        .is_some_and(|reset| group.taken_at <= reset.at)
    {
        return Err(Ineligible::BeforeReset);
    }
    Ok(())
}

/// Where a group at `distance_m` lands from its aim with the sights as
/// zeroed, mrad, [right, up]: the solver's flight of its bullet, without
/// the load's own point of impact, in the group's weather (without wind).
/// Without the bullet, only the zero's own height.
pub fn expected_impact_mrad(group: &Group, rifle: &Rifle, distance_m: f64, data: &Data) -> Point {
    let height_only = Point::new(
        0.0,
        groups_core::angles::mrad(rifle.zero_height_cm * 10.0, rifle.zero_range_m.max(1.0)),
    );
    let Some(bullet) = group
        .bullet_id
        .as_ref()
        .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id))
    else {
        return height_only;
    };
    let mut load = data
        .loads
        .iter()
        .find(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
        .cloned();
    if let Some(load) = &mut load {
        load.impact_offset_mrad = [0.0, 0.0];
    }
    let session = group
        .session_id
        .as_ref()
        .and_then(|id| data.groups.sessions.iter().find(|s| &s.id == id))
        .map(|session| session.conditions)
        .unwrap_or_default();
    let conditions =
        crate::groups::details::velocity::solver_conditions(&group.conditions.or(session));
    let scenario = ballistics_shot::scenario(rifle, bullet, load.as_ref(), &conditions);
    let row = ballistics_core::solver::solve(&scenario, distance_m + 5.0)
        .ok()
        .and_then(|flight| flight.at(distance_m));
    match row {
        Some(row) if row.drop.is_finite() && row.windage.is_finite() => Point::new(
            row.windage / distance_m * 1000.0,
            row.drop / distance_m * 1000.0,
        ),
        _ => height_only,
    }
}

/// A group in a zero check.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroEntry {
    pub group_id: String,
    pub taken_at: i64,
    pub distance_m: f64,
    pub group: ZeroGroup,
    /// What the hunter noted as dialled for it, mrad, [right, up].
    pub dialled_mrad: [f64; 2],
}

/// A zero check of the groups `scope` lets through.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroData {
    pub entries: Vec<ZeroEntry>,
    /// `None` without groups.
    pub check: Option<Result<ZeroCheck, AnalysisError>>,
    /// Groups of the rifle, barrel and load left out by the rules, with why.
    pub left_out: Vec<(String, Ineligible)>,
    /// The latest reset for the chosen barrel (or the rifle).
    pub reset: Option<ZeroReset>,
    pub dialled_groups: usize,
    /// Groups of the rifle and load that name no barrel, left out by a
    /// barrel filter.
    pub no_barrel: usize,
    /// The rifle's zero height at its zero range, mrad up: where groups at
    /// that range are expected to land.
    pub zero_height_mrad: f64,
    /// The rifle's zero range, the distance groups must be shot at.
    pub zero_range_m: Option<f64>,
}

/// The zero check of `scope`: each group shot at the rifle's zero range,
/// its impact less what was dialled for it and less where the bullet lands
/// at that distance with the sights as zeroed (the zero's own height at
/// the zero range itself).
pub fn zero_data(data: &Data, scope: &ZeroScope) -> ZeroData {
    let rifle = data.rifles.iter().find(|rifle| rifle.id == scope.rifle_id);
    let resets = &data.groups.zero_resets;
    let barrels = &data.groups.barrels;
    let zero_range_m = rifle.map(|rifle| rifle.zero_range_m);
    let zero_height_mrad = rifle.map_or(0.0, |rifle| {
        groups_core::angles::mrad(rifle.zero_height_cm * 10.0, rifle.zero_range_m.max(1.0))
    });
    let mut entries = Vec::new();
    let mut left_out = Vec::new();
    let mut no_barrel = 0;
    let mut groups: Vec<&Group> = data.groups.groups.iter().collect();
    groups.sort_by_key(|group| group.taken_at);
    for group in groups {
        match zero_eligibility(group, scope, resets, zero_range_m, barrels) {
            Err(Ineligible::OtherBarrel) if group.barrel_id.is_none() => {
                let any_barrel = ZeroScope {
                    barrel: Pick::All,
                    ..scope.clone()
                };
                if zero_eligibility(group, &any_barrel, resets, zero_range_m, barrels).is_ok() {
                    no_barrel += 1;
                }
            }
            Err(Ineligible::OtherRifle | Ineligible::OtherBarrel | Ineligible::OtherLoad) => {}
            Err(reason) => left_out.push((group.id.clone(), reason)),
            Ok(()) => {
                let distance_m = group.distance_m.unwrap_or(100.0);
                let stats = measured(group, data).and_then(|measured| measured.stats);
                let expected = rifle.map_or(Point::ORIGIN, |rifle| {
                    expected_impact_mrad(group, rifle, distance_m, data)
                });
                let dialled = Point::new(
                    group.dialled_offset_mrad[0] + expected.x,
                    group.dialled_offset_mrad[1] + expected.y,
                );
                match stats
                    .as_ref()
                    .map(|stats| ZeroGroup::from_stats(stats, dialled))
                {
                    Some(Ok(zero_group)) => entries.push(ZeroEntry {
                        group_id: group.id.clone(),
                        taken_at: group.taken_at,
                        distance_m,
                        group: zero_group,
                        dialled_mrad: group.dialled_offset_mrad,
                    }),
                    _ => left_out.push((group.id.clone(), Ineligible::NotMeasured)),
                }
            }
        }
    }
    let zero_groups: Vec<ZeroGroup> = entries.iter().map(|entry| entry.group).collect();
    let check = (!zero_groups.is_empty()).then(|| ZeroCheck::new(&zero_groups));
    let reset = latest_reset(resets, &scope.rifle_id, scope.barrel.one()).cloned();
    ZeroData {
        dialled_groups: entries
            .iter()
            .filter(|entry| entry.dialled_mrad != [0.0, 0.0])
            .count(),
        entries,
        check,
        left_out,
        reset,
        no_barrel,
        zero_height_mrad,
        zero_range_m,
    }
}

/// Whether the zero check may be written to the calculator: only with a
/// verdict, holding or drifting, which takes three groups or more.
pub fn zero_writable(check: &ZeroCheck) -> bool {
    matches!(
        check.verdict,
        groups_core::ZeroVerdict::Holding | groups_core::ZeroVerdict::Drifting
    )
}

/// Where the load hits compared with the rifle's zero, as the calculator
/// takes it (`RifleLoad::impact_offset_mrad`, [right, up]): the checked
/// mean impact, which is minus the correction to dial.
pub fn zero_offset(check: &ZeroCheck) -> [f64; 2] {
    [check.across.impact.value, check.up.impact.value]
}

/// The rifle's setup for the load with the zero check's offset: the one
/// there is, or a new one.
pub fn load_with_offset(
    data: &Data,
    rifle_id: &str,
    bullet_id: &str,
    offset: [f64; 2],
) -> RifleLoad {
    let mut load = data
        .loads
        .iter()
        .find(|load| load.rifle_id == rifle_id && load.bullet_id == bullet_id)
        .cloned()
        .unwrap_or_else(|| RifleLoad::new(rifle_id, bullet_id));
    load.impact_offset_mrad = offset.map(|mrad| mrad.clamp(-49.0, 49.0));
    load
}

// --- Your precision -----------------------------------------------------

/// The fewest groups, or shots, "Use my groups" needs.
pub const PRECISION_MIN_GROUPS: usize = 2;
pub const PRECISION_MIN_SHOTS: usize = 10;

/// The spread of a rifle and bullet's groups, pooled about their own
/// centres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Precision {
    pub sigma_mrad: f64,
    pub groups: usize,
    pub shots: usize,
    /// What the calculator's "Your group" means: the expected 5-shot group
    /// at 100 m, edge to edge, cm.
    pub group_cm: f64,
}

impl Precision {
    /// Enough to go by.
    pub fn enough(&self) -> bool {
        self.groups >= PRECISION_MIN_GROUPS || self.shots >= PRECISION_MIN_SHOTS
    }
}

/// The expected 5-shot group at 100 m, edge to edge, cm, of a spread of σ
/// (per axis, mrad): its mean centre-to-centre extreme spread (3.07 σ,
/// 1 mrad being 10 cm at 100 m) plus one calibre.
pub fn five_shot_group_cm(sigma_mrad: f64, calibre_mm: Option<f64>) -> f64 {
    ballistics_core::dispersion::FIVE_SHOT_SPREAD_IN_SIGMAS * sigma_mrad * 10.0
        + calibre_mm.unwrap_or(0.0) / 10.0
}

/// The pooled spread of the rifle and bullet's groups with a distance,
/// about their own centres, leaving out ladders; `None` when there is
/// none to pool.
pub fn precision(data: &Data, rifle_id: &str, bullet_id: &str) -> Option<Precision> {
    let groups: Vec<ShotGroup> = data
        .groups
        .groups
        .iter()
        .filter(|group| {
            group.rifle_id.as_deref() == Some(rifle_id)
                && group.bullet_id.as_deref() == Some(bullet_id)
                && group.ladder_id.is_none()
        })
        .filter_map(|group| {
            let distance_m = group.distance_m?;
            let measured = measured(group, data)?;
            let shots = counted_shots(&measured);
            (!shots.is_empty()).then(|| ShotGroup {
                shots,
                aim: measured.aim_mm,
                distance_m,
            })
        })
        .collect();
    if groups.is_empty() {
        return None;
    }
    let composite = Composite::with_mode(&groups, CompositeMode::AboutOwnCentres).ok()?;
    let calibre = crate::groups::form::calibre_of(Some(bullet_id), data);
    Some(Precision {
        sigma_mrad: composite.sigma.value,
        groups: composite.groups - composite.left_out.len(),
        shots: composite.shots,
        group_cm: five_shot_group_cm(composite.sigma.value, calibre),
    })
}

// --- Compare and composite ------------------------------------------------

/// Why a compared group isn't ranked.
#[derive(Clone, Debug, PartialEq)]
pub enum NotRanked {
    /// Its shots can't be measured; the library's words for why.
    NoNumbers(String),
    NoDistance,
    Core(Unrankable),
}

/// Several groups side by side.
#[derive(Clone)]
pub struct Compared {
    pub ids: Vec<String>,
    pub measured: Vec<Option<Rc<Measured>>>,
    /// Each group's place, by input index.
    pub ranks: Vec<Option<RankedGroup>>,
    pub not_ranked: Vec<(usize, NotRanked)>,
    pub best: Option<usize>,
    pub worst: Option<usize>,
    pub unequal_shots: bool,
    /// With exactly two groups: their σ compared.
    pub pair: Option<Result<SigmaComparison, AnalysisError>>,
    /// Each group's chronograph SD, m/s.
    pub velocity_sd: Vec<Option<f64>>,
}

/// The groups of `ids` this device has, in order.
pub fn groups_of<'a>(data: &'a Data, ids: &[String]) -> Vec<&'a Group> {
    ids.iter()
        .filter_map(|id| data.groups.groups.iter().find(|group| &group.id == id))
        .collect()
}

pub fn compare(data: &Data, ids: &[String], by: groups_core::RankBy) -> Compared {
    let groups = groups_of(data, ids);
    let measured: Vec<Option<Rc<Measured>>> =
        groups.iter().map(|group| measured(group, data)).collect();
    let mut not_ranked = Vec::new();
    let mut rankable = Vec::new();
    let mut shot_groups = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        match (&measured[index], group.distance_m) {
            (None, _) => {
                let why = match measure_group(group, data) {
                    Err(error) => crate::groups::problem_text(&error),
                    Ok(_) => String::new(),
                };
                not_ranked.push((index, NotRanked::NoNumbers(why)));
            }
            (Some(_), None) => not_ranked.push((index, NotRanked::NoDistance)),
            (Some(measured), Some(distance_m)) => {
                rankable.push(index);
                shot_groups.push(ShotGroup {
                    shots: counted_shots(measured),
                    aim: measured.aim_mm,
                    distance_m,
                });
            }
        }
    }
    let ranking: Ranking = groups_core::rank(&shot_groups, by);
    let mut ranks = vec![None; groups.len()];
    for ranked in &ranking.ranked {
        let index = rankable[ranked.index];
        ranks[index] = Some(RankedGroup { index, ..*ranked });
    }
    for unranked in &ranking.unranked {
        not_ranked.push((rankable[unranked.index], NotRanked::Core(unranked.reason)));
    }
    not_ranked.sort_by_key(|(index, _)| *index);
    let pair = (groups.len() == 2).then(|| {
        match (
            measured[0].as_ref().and_then(|m| m.stats.as_ref()),
            measured[1].as_ref().and_then(|m| m.stats.as_ref()),
        ) {
            (Some(a), Some(b)) => groups_core::compare_groups(a, b),
            _ => Err(AnalysisError::Empty),
        }
    });
    let velocity_sd = groups
        .iter()
        .map(|group| {
            VelocityStats::new(&velocities(&group.marks, &group.extra_velocities_mps))
                .ok()
                .and_then(|stats| stats.sd.map(|sd| sd.value))
        })
        .collect();
    Compared {
        ids: groups.iter().map(|group| group.id.clone()).collect(),
        measured,
        ranks,
        not_ranked,
        best: ranking.best().map(|best| rankable[best.index]),
        worst: (ranking.ranked.len() >= 2)
            .then(|| ranking.worst().map(|worst| rankable[worst.index]))
            .flatten(),
        unequal_shots: ranking.unequal_shots,
        pair,
        velocity_sd,
    }
}

/// Why a group is left out of a composite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOut {
    NotMeasured,
    NoDistance,
    /// A cold-bore series takes each group's cold-bore shot.
    NoColdBore,
}

/// Groups pooled into one.
#[derive(Clone, Debug)]
pub struct Composed {
    /// The pooled groups' ids, in the order the composite counts them.
    pub used: Vec<String>,
    pub left_out: Vec<(String, LeftOut)>,
    /// The ways these groups can be pooled.
    pub modes: Vec<CompositeMode>,
    pub mode: CompositeMode,
    pub composite: Result<Composite, AnalysisError>,
    /// The distance mm are given at by default: the first group's.
    pub distance_m: f64,
}

/// Pools the groups of `ids`, in `wanted` mode when they allow it.
pub fn composite(data: &Data, ids: &[String], wanted: Option<CompositeMode>) -> Composed {
    let mut left_out = Vec::new();
    let mut usable: Vec<(&Group, Rc<Measured>, f64)> = Vec::new();
    for group in groups_of(data, ids) {
        match (measured(group, data), group.distance_m) {
            (None, _) => left_out.push((group.id.clone(), LeftOut::NotMeasured)),
            (Some(_), None) => left_out.push((group.id.clone(), LeftOut::NoDistance)),
            (Some(measured), Some(distance)) => usable.push((group, measured, distance)),
        }
    }
    let all_aimed = !usable.is_empty() && usable.iter().all(|(_, m, _)| m.aim_mm.is_some());
    let all_cold = all_aimed && usable.iter().all(|(_, m, _)| !m.cold_bore.is_empty());
    let mut modes = Vec::new();
    if all_aimed {
        modes.push(CompositeMode::AboutAims);
    }
    modes.push(CompositeMode::AboutOwnCentres);
    if all_cold {
        modes.push(CompositeMode::ColdBore);
    }
    let mode = wanted
        .filter(|mode| modes.contains(mode))
        .unwrap_or(modes[0]);
    let mut used = Vec::new();
    let mut shot_groups = Vec::new();
    for (group, measured, distance_m) in &usable {
        let shots = if mode == CompositeMode::ColdBore {
            measured
                .cold_bore
                .first()
                .map(|&i| vec![measured.shots_mm[i]])
        } else {
            Some(counted_shots(measured)).filter(|shots| !shots.is_empty())
        };
        match shots {
            Some(shots) => {
                used.push(group.id.clone());
                shot_groups.push(ShotGroup {
                    shots,
                    aim: measured.aim_mm,
                    distance_m: *distance_m,
                });
            }
            None if mode == CompositeMode::ColdBore => {
                left_out.push((group.id.clone(), LeftOut::NoColdBore));
            }
            None => left_out.push((group.id.clone(), LeftOut::NotMeasured)),
        }
    }
    let composite = if shot_groups.is_empty() {
        Err(AnalysisError::Empty)
    } else {
        Composite::with_mode(&shot_groups, mode)
    };
    Composed {
        distance_m: shot_groups.first().map_or(100.0, |group| group.distance_m),
        used,
        left_out,
        modes,
        mode,
        composite,
    }
}

/// A composite mode as a saved analysis keeps it.
pub fn mode_key(mode: CompositeMode) -> &'static str {
    match mode {
        CompositeMode::AboutAims => "about_aims",
        CompositeMode::AboutOwnCentres => "about_own_centres",
        CompositeMode::ColdBore => "cold_bore",
    }
}

pub fn mode_from_key(key: &str) -> Option<CompositeMode> {
    [
        CompositeMode::AboutAims,
        CompositeMode::AboutOwnCentres,
        CompositeMode::ColdBore,
    ]
    .into_iter()
    .find(|mode| mode_key(*mode) == key)
}

/// A saved analysis opened again: the groups of it this device still has.
#[derive(Clone, Debug, PartialEq)]
pub struct Rebuilt {
    pub ids: Vec<String>,
    /// Its groups deleted since.
    pub missing: usize,
    /// A composite's mode and distance, as saved.
    pub mode: Option<CompositeMode>,
    pub distance_m: Option<f64>,
}

pub fn rebuild(analysis: &Analysis, data: &Data) -> Rebuilt {
    let ids: Vec<String> = groups_of(data, &analysis.group_ids)
        .into_iter()
        .map(|group| group.id.clone())
        .collect();
    let settings = &analysis.settings;
    Rebuilt {
        missing: analysis.group_ids.len() - ids.len(),
        ids,
        mode: settings
            .get("mode")
            .and_then(|mode| mode.as_str())
            .and_then(mode_from_key),
        distance_m: settings
            .get("distance_m")
            .and_then(serde_json::Value::as_f64)
            .filter(|m| (1.0..=5000.0).contains(m)),
    }
}

/// A composite to save, with its mode and distance.
pub fn composite_analysis(
    name: String,
    ids: Vec<String>,
    mode: CompositeMode,
    distance_m: f64,
) -> Analysis {
    let mut analysis = Analysis::new(AnalysisKind::Composite, name, ids);
    analysis.settings = serde_json::json!({ "mode": mode_key(mode), "distance_m": distance_m });
    analysis
}

// --- Trend ---------------------------------------------------------------

/// What a trend shows up the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metric {
    MeanRadius,
    ExtremeSpread,
    Sigma,
    VelocityAverage,
    VelocitySd,
    /// How far the cold-bore shot landed from the aim.
    ColdBoreOffset,
}

impl Metric {
    pub const ALL: [Metric; 6] = [
        Metric::MeanRadius,
        Metric::ExtremeSpread,
        Metric::Sigma,
        Metric::VelocityAverage,
        Metric::VelocitySd,
        Metric::ColdBoreOffset,
    ];

    /// The unit the values are shown in; their base is mrad or m/s.
    pub fn quantity(self) -> Quantity {
        match self {
            Metric::VelocityAverage | Metric::VelocitySd => Quantity::Speed,
            _ => Quantity::GroupAngle,
        }
    }
}

/// What a trend shows along the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Against {
    /// 1, 2, 3… in the order they were shot.
    Order,
    /// Days since 1970.
    Date,
    /// The charge, g.
    Charge,
    /// The cartridge's base to ogive, mm.
    Seating,
    /// The powder's temperature where given, else the air's, °C.
    Temperature,
    Pressure,
    /// Rounds through the group's barrel once the group was saved; groups
    /// naming no barrel are left out.
    BarrelRounds,
    /// Rounds since the barrel was last cleaned, once the group was saved.
    SinceClean,
}

impl Against {
    pub const ALL: [Against; 8] = [
        Against::Order,
        Against::Date,
        Against::Charge,
        Against::Seating,
        Against::Temperature,
        Against::Pressure,
        Against::BarrelRounds,
        Against::SinceClean,
    ];

    /// The unit the values are shown in, for those that have one: charge
    /// in gr (or g), seating in mm (or in), °C, hPa.
    pub fn quantity(self) -> Option<Quantity> {
        match self {
            Against::Charge => Some(Quantity::PowderCharge),
            Against::Seating => Some(Quantity::BulletLength),
            Against::Temperature => Some(Quantity::Temperature),
            Against::Pressure => Some(Quantity::Pressure),
            Against::Order | Against::Date | Against::BarrelRounds | Against::SinceClean => None,
        }
    }
}

/// The groups a trend looks at.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TrendFilters {
    pub rifle_id: Option<String>,
    pub barrel_id: Option<String>,
    pub bullet_id: Option<String>,
    pub tag: Option<String>,
    /// Ladders' rungs too, which trends leave out unless asked.
    pub ladders: bool,
}

impl TrendFilters {
    pub fn lets(&self, group: &Group) -> bool {
        let wanted = |filter: &Option<String>, value: &Option<String>| {
            filter.as_ref().is_none_or(|id| value.as_ref() == Some(id))
        };
        wanted(&self.rifle_id, &group.rifle_id)
            && wanted(&self.barrel_id, &group.barrel_id)
            && wanted(&self.bullet_id, &group.bullet_id)
            && self
                .tag
                .as_ref()
                .is_none_or(|tag| group.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            && (self.ladders || group.ladder_id.is_none())
    }
}

/// A point of a trend, in base units.
#[derive(Clone, Debug, PartialEq)]
pub struct TrendPoint {
    pub group_id: String,
    pub x: f64,
    pub y: f64,
    /// The 95% range of σ or the mean radius.
    pub range: Option<Interval>,
    /// The velocity readings' box plot.
    pub spread: Option<BoxPlot>,
}

/// One load's line.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub rifle_id: Option<String>,
    pub barrel_id: Option<String>,
    pub bullet_id: Option<String>,
    /// Stable for the rifle, barrel and bullet, whatever the filters.
    pub colour: usize,
    pub points: Vec<TrendPoint>,
    /// Velocity against temperature: the load's sensitivity.
    pub sensitivity: Option<Result<TemperatureSensitivity, AnalysisError>>,
}

/// A trend's lines, and what kept groups off them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrendData {
    pub series: Vec<Series>,
    /// Groups the filters let through.
    pub groups: usize,
    /// Of those, the ones without the metric, and without the axis value.
    pub missing_metric: usize,
    pub missing_axis: usize,
}

impl TrendData {
    pub fn points(&self) -> usize {
        self.series.iter().map(|series| series.points.len()).sum()
    }
}

/// A group's value along `against`, in its base unit.
pub fn axis_value(group: &Group, against: Against, order: usize, data: &Data) -> Option<f64> {
    let conditions = || {
        let session = group
            .session_id
            .as_ref()
            .and_then(|id| data.groups.sessions.iter().find(|s| &s.id == id))
            .map(|session| session.conditions)
            .unwrap_or_default();
        group.conditions.or(session)
    };
    let ladder = || {
        group
            .ladder_id
            .as_ref()
            .and_then(|id| data.groups.ladders.iter().find(|ladder| &ladder.id == id))
    };
    let recipe = || {
        data.bullets
            .iter()
            .find(|bullet| group.bullet_id.as_ref() == Some(&bullet.id))
            .and_then(|bullet| bullet.recipe.clone())
    };
    let rung = |variable: LadderVariable| {
        ladder()
            .filter(|ladder| ladder.variable == variable)
            .and(group.rung_value)
    };
    match against {
        Against::Order => Some(order as f64),
        Against::Date => Some(group.taken_at as f64 / DAY_MS),
        // A ladder's rung, else what the recipe was when the group was
        // saved, else (for groups saved without it) the recipe now.
        Against::Charge => rung(LadderVariable::Charge)
            .or(group.charge_g)
            .or_else(|| recipe()?.charge_g),
        Against::Seating => rung(LadderVariable::Seating)
            .or(group.cbto_mm)
            .or_else(|| recipe()?.cbto_mm),
        Against::Temperature => {
            let conditions = conditions();
            conditions.powder_temperature_c.or(conditions.temperature_c)
        }
        Against::Pressure => conditions().pressure_hpa,
        Against::BarrelRounds | Against::SinceClean => {
            let rifle = data
                .rifles
                .iter()
                .find(|rifle| group.rifle_id.as_ref() == Some(&rifle.id))?;
            let barrel: Option<&Barrel> = group
                .barrel_id
                .as_ref()
                .and_then(|id| data.groups.barrels.iter().find(|barrel| &barrel.id == id));
            let log: Vec<_> = data
                .groups
                .round_log
                .iter()
                .filter(|entry| entry.at <= group.taken_at)
                .cloned()
                .collect();
            let cleans: Vec<_> = data
                .groups
                .cleans
                .iter()
                .filter(|clean| clean.at <= group.taken_at)
                .cloned()
                .collect();
            let counts = RoundCounts::derive(rifle, barrel, &log, &cleans);
            if against == Against::BarrelRounds {
                counts.barrel.map(|rounds| rounds as f64)
            } else {
                counts.since_clean.map(|rounds| rounds as f64)
            }
        }
    }
}

/// A group's value of `metric`, in its base unit, with its range and box.
fn metric_value(
    group: &Group,
    measured: Option<&Measured>,
    metric: Metric,
) -> Option<(f64, Option<Interval>, Option<BoxPlot>)> {
    let readings = || velocities(&group.marks, &group.extra_velocities_mps);
    match metric {
        Metric::VelocityAverage => {
            let readings = readings();
            let stats = VelocityStats::new(&readings).ok()?;
            Some((stats.average, None, BoxPlot::new(&readings).ok()))
        }
        Metric::VelocitySd => {
            let stats = VelocityStats::new(&readings()).ok()?;
            Some((stats.sd?.value, None, None))
        }
        Metric::ColdBoreOffset => {
            let measured = measured?;
            let distance = group.distance_m?;
            let aim = measured.aim_mm?;
            let shot = measured.shots_mm[*measured.cold_bore.first()?];
            let mm = ((shot.x - aim.x).powi(2) + (shot.y - aim.y).powi(2)).sqrt();
            Some((groups_core::angles::mrad(mm, distance), None, None))
        }
        Metric::MeanRadius | Metric::ExtremeSpread | Metric::Sigma => {
            let stats: &GroupStats = measured?.stats.as_ref()?;
            let distance = group.distance_m?;
            let per = 1.0 / distance;
            let spread = stats.spread?;
            match metric {
                Metric::MeanRadius => Some((
                    stats.mean_radius * per,
                    Some(spread.mean_radius_range.scaled(per)),
                    None,
                )),
                Metric::ExtremeSpread => {
                    Some((spread.extreme_spread.centre_to_centre * per, None, None))
                }
                _ => {
                    let sigma = stats.sigma_mrad()?.estimate();
                    Some((sigma.value, Some(sigma.range), None))
                }
            }
        }
    }
}

/// The index of a load's colour: its place among the bullets by id, so it
/// keeps its colour whatever is filtered.
pub fn colour_of(group: &Group, data: &Data) -> usize {
    let key = |g: &Group| (g.bullet_id.clone(), g.rifle_id.clone(), g.barrel_id.clone());
    let mut keys: Vec<_> = data.groups.groups.iter().map(key).collect();
    keys.sort_unstable();
    keys.dedup();
    keys.iter()
        .position(|known| *known == key(group))
        .unwrap_or(0)
}

/// The bullet with the fitted temperature sensitivity as its velocity
/// change per degree (m/s per °C), within what the calculator accepts.
pub fn bullet_with_sensitivity(bullet: &Bullet, fit: &TemperatureSensitivity) -> Bullet {
    let mut bullet = bullet.clone();
    bullet.velocity_per_degree = fit.per_degree.value.clamp(-5.0, 5.0);
    bullet
}

/// Whether the calculator takes this rifle's load of the bullet from its
/// own velocities at several temperatures, so the bullet's sensitivity
/// isn't used for it.
pub fn load_sets_own_sensitivity(data: &Data, rifle_id: &str, bullet: &Bullet) -> bool {
    data.loads
        .iter()
        .find(|load| load.rifle_id == rifle_id && load.bullet_id == bullet.id)
        .is_some_and(|load| load.velocity_model(bullet).fitted)
}

/// The trend of `metric` against `against` for the groups `filters` let
/// through, one line per load in the order shot.
pub fn trend(data: &Data, metric: Metric, against: Against, filters: &TrendFilters) -> TrendData {
    let mut groups: Vec<&Group> = data
        .groups
        .groups
        .iter()
        .filter(|group| filters.lets(group))
        .collect();
    groups.sort_by(|a, b| a.taken_at.cmp(&b.taken_at).then_with(|| a.id.cmp(&b.id)));
    let mut result = TrendData {
        groups: groups.len(),
        ..TrendData::default()
    };
    // Each line's sessions for the temperature fit: (session key, °C,
    // readings).
    let mut sessions: Vec<Vec<(String, f64, Vec<f64>)>> = Vec::new();
    for (order, group) in groups.iter().enumerate() {
        let measured = measured(group, data);
        let Some((y, range, spread)) = metric_value(group, measured.as_deref(), metric) else {
            result.missing_metric += 1;
            continue;
        };
        let Some(x) = axis_value(group, against, order + 1, data) else {
            result.missing_axis += 1;
            continue;
        };
        let index = match result.series.iter().position(|series| {
            series.rifle_id == group.rifle_id
                && series.barrel_id == group.barrel_id
                && series.bullet_id == group.bullet_id
        }) {
            Some(index) => index,
            None => {
                result.series.push(Series {
                    rifle_id: group.rifle_id.clone(),
                    barrel_id: group.barrel_id.clone(),
                    bullet_id: group.bullet_id.clone(),
                    colour: colour_of(group, data),
                    points: Vec::new(),
                    sensitivity: None,
                });
                sessions.push(Vec::new());
                result.series.len() - 1
            }
        };
        result.series[index].points.push(TrendPoint {
            group_id: group.id.clone(),
            x,
            y,
            range,
            spread,
        });
        if metric == Metric::VelocityAverage && against == Against::Temperature {
            let key = group.session_id.clone().unwrap_or_else(|| group.id.clone());
            let readings = velocities(&group.marks, &group.extra_velocities_mps);
            let line = &mut sessions[index];
            match line
                .iter_mut()
                .find(|(known, temperature, _)| *known == key && *temperature == x)
            {
                Some((_, _, pooled)) => pooled.extend(readings),
                None => line.push((key, x, readings)),
            }
        }
    }
    if metric == Metric::VelocityAverage && against == Against::Temperature {
        for (series, line) in result.series.iter_mut().zip(&sessions) {
            series.sensitivity = Some(temperature_fit(line));
        }
    }
    result
}

/// The temperature sensitivity of sessions given as (key, °C, readings).
pub fn temperature_fit(
    sessions: &[(String, f64, Vec<f64>)],
) -> Result<TemperatureSensitivity, AnalysisError> {
    let sessions: Vec<TemperatureSession> = sessions
        .iter()
        .filter_map(|(_, temperature_c, readings)| {
            let stats = VelocityStats::new(readings).ok()?;
            Some(TemperatureSession {
                temperature_c: *temperature_c,
                mean_mps: stats.average,
                shots: stats.shots,
                sd_mps: stats.sd.map_or(0.0, |sd| sd.value),
            })
        })
        .collect();
    TemperatureSensitivity::new(&sessions)
}

// --- Ladder --------------------------------------------------------------

/// A ladder's groups and their rungs.
#[derive(Clone, Debug)]
pub struct LadderData {
    /// The groups counted, by id, in the order `groups` gives them to the
    /// ladder.
    pub ids: Vec<String>,
    pub left_out: Vec<(String, Ineligible)>,
    /// `None` without groups.
    pub ladder: Option<Result<groups_core::Ladder, AnalysisError>>,
    /// The distance the groups were shot at, when they share one.
    pub distance_m: Option<f64>,
}

pub fn ladder(data: &Data, ladder: &Ladder, key: LadderKey) -> LadderData {
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut left_out = Vec::new();
    let mut distances: Vec<f64> = Vec::new();
    let mut members: Vec<&Group> = data
        .groups
        .groups
        .iter()
        .filter(|group| group.ladder_id.as_deref() == Some(ladder.id.as_str()))
        .collect();
    members.sort_by(|a, b| a.taken_at.cmp(&b.taken_at).then_with(|| a.id.cmp(&b.id)));
    for group in members {
        let Some(value) = group.rung_value else {
            continue;
        };
        if !group_has_aim(group) {
            left_out.push((group.id.clone(), Ineligible::NoAim));
            continue;
        }
        let Some(distance) = group.distance_m else {
            left_out.push((group.id.clone(), Ineligible::NoDistance));
            continue;
        };
        let readings = velocities(&group.marks, &group.extra_velocities_mps);
        match measured(group, data)
            .and_then(|measured| measured.stats)
            .map(|stats| LadderGroup::from_stats(&stats, value, readings))
        {
            Some(Ok(rung)) => {
                ids.push(group.id.clone());
                groups.push(rung);
                distances.push(distance);
            }
            _ => left_out.push((group.id.clone(), Ineligible::NotMeasured)),
        }
    }
    let shared = distances
        .first()
        .copied()
        .filter(|first| distances.iter().all(|d| d == first));
    let options = LadderOptions {
        key,
        ..LadderOptions::default()
    };
    LadderData {
        ladder: (!groups.is_empty()).then(|| groups_core::Ladder::new(&groups, options)),
        ids,
        left_out,
        distance_m: shared,
    }
}

/// A ladder's chosen run made into a load: the handload with the run's
/// value in its recipe, the ladder naming it, and for a new load the
/// rifle's data copied from the ladder's.
#[derive(Clone, Debug, PartialEq)]
pub struct Promotion {
    pub bullet: Bullet,
    pub ladder: Ladder,
    pub load: Option<RifleLoad>,
}

/// The promotion of a ladder's run at `value` (g or mm, as the ladder
/// varies) into `target`, or into a new copy of `source` named with
/// `name_suffix` when there is none.
pub fn promotion(
    source: &Bullet,
    target: Option<&Bullet>,
    ladder: &Ladder,
    value: f64,
    name_suffix: &str,
    data: &Data,
) -> Promotion {
    let (mut bullet, load) = match target {
        Some(target) => (target.clone(), None),
        None => {
            let mut copy = source.clone();
            copy.id = ballistics_store::model::new_id();
            copy.name = format!("{} ({name_suffix})", source.name)
                .chars()
                .take(100)
                .collect();
            copy.updated_at = 0;
            let load = data
                .loads
                .iter()
                .find(|load| load.rifle_id == ladder.rifle_id && load.bullet_id == source.id)
                .map(|load| {
                    let mut new = load.clone();
                    new.id = RifleLoad::new(&ladder.rifle_id, &copy.id).id;
                    new.bullet_id = copy.id.clone();
                    new.updated_at = 0;
                    new
                });
            (copy, load)
        }
    };
    bullet.kind = ballistics_store::BulletKind::Handload;
    let recipe = bullet
        .recipe
        .get_or_insert_with(ballistics_store::Recipe::default);
    match ladder.variable {
        LadderVariable::Charge => recipe.charge_g = Some(value.clamp(0.01, 30.0)),
        LadderVariable::Seating => recipe.cbto_mm = Some(value.clamp(5.0, 200.0)),
    }
    let mut ladder = ladder.clone();
    ladder.promoted_bullet_id = Some(bullet.id.clone());
    Promotion {
        bullet,
        ladder,
        load,
    }
}

/// The middle of a run of rungs: halfway between its first and last.
pub fn run_middle(ladder: &groups_core::Ladder, run: &groups_core::LadderRun) -> f64 {
    let first = ladder.rungs[run.first].value;
    let last = ladder.rungs[run.first + run.rungs - 1].value;
    (first + last) / 2.0
}

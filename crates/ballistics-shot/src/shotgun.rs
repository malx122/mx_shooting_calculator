//! A shotgun shell from the hunter's records, flown by the engine: the
//! shot's pellets and the pattern from a barrel's choke (or from the
//! pattern boards the hunter counted), or the slug.

use std::sync::Arc;

use ballistics_core::pellets::{self, MaxRange, Pellet, Swarm, SwarmSpread};
use ballistics_core::solver::{self, SolveError};
use ballistics_core::{BcBands, DragModel, Load, Scenario, Wind};
use ballistics_store::{
    Choke, ChokeName, Payload, Shell, ShotMaterial, Shotgun, ShotgunLoad, VelocityAt,
};

use crate::Conditions;

/// The engine's material for the shot's.
pub fn material(material: ShotMaterial) -> pellets::Material {
    match material {
        ShotMaterial::Lead => pellets::Material::Lead,
        ShotMaterial::Steel => pellets::Material::Steel,
        ShotMaterial::Bismuth => pellets::Material::Bismuth,
        ShotMaterial::TungstenIron => pellets::Material::TungstenIron,
        ShotMaterial::TungstenMatrix => pellets::Material::TungstenMatrix,
        ShotMaterial::Tss => pellets::Material::Tss,
        ShotMaterial::Zinc => pellets::Material::Zinc,
    }
}

/// How the shell's pellets differ from each other and from shot to shot;
/// `None` for a slug.
pub fn swarm_spread(shell: &Shell) -> Option<SwarmSpread> {
    match shell.payload {
        Payload::Shot {
            material: shot,
            pellet_mm,
            ..
        } => Some(SwarmSpread::of(material(shot), pellet_mm / 1000.0)),
        Payload::Slug { .. } => None,
    }
}

/// The shell's pellets flown one by one, out to the farthest a pattern is
/// worked out for; `None` for a slug.
pub fn swarm(
    shell: &Shell,
    muzzle_mps: f64,
    conditions: &Conditions,
) -> Result<Option<Swarm>, SolveError> {
    let (Some(pellet), Some(spread)) = (pellet(shell), swarm_spread(shell)) else {
        return Ok(None);
    };
    Swarm::new(
        &pellet,
        muzzle_mps,
        &engine_conditions(conditions),
        spread,
        pellets::MAX_PATTERN_RANGE_M,
    )
    .map(Some)
}

/// One pellet of the shell's shot; `None` for a slug.
pub fn pellet(shell: &Shell) -> Option<Pellet> {
    match shell.payload {
        Payload::Shot {
            material: shot,
            pellet_mm,
            ..
        } => Some(Pellet::of(material(shot), pellet_mm / 1000.0)),
        Payload::Slug { .. } => None,
    }
}

/// The pellets in the shell's load; `None` for a slug.
pub fn pellet_count(shell: &Shell) -> Option<usize> {
    let Payload::Shot { load_g, .. } = shell.payload else {
        return None;
    };
    pellet(shell)?.count_in(load_g / 1000.0)
}

/// The engine's conditions for a shot: level (shotgun ranges are short, and
/// the calculator's shot angle is for its rifle shot), without cant.
pub fn engine_conditions(conditions: &Conditions) -> ballistics_core::Conditions {
    ballistics_core::Conditions {
        atmosphere: conditions.atmosphere(),
        powder_temperature_c: conditions.powder_temperature_c(),
        wind: Wind {
            speed: conditions.wind_speed_mps,
            from_degrees: conditions.wind_clock * 30.0,
        },
        inclination_degrees: 0.0,
        coriolis: conditions.coriolis,
        cant_degrees: 0.0,
    }
}

fn engine_velocity_at(at: VelocityAt) -> pellets::VelocityAt {
    match at {
        VelocityAt::Muzzle => pellets::VelocityAt::Muzzle,
        VelocityAt::V1 => pellets::VelocityAt::Distance(1.0),
        VelocityAt::V2_5 => pellets::VelocityAt::Distance(2.5),
        VelocityAt::ThreeFeet => pellets::VelocityAt::Distance(0.9144),
        VelocityAt::Observed => pellets::VelocityAt::AverageOver(18.288),
    }
}

/// The shell's velocity at the muzzle, from the one on its box, in the
/// day's air.
pub fn muzzle_velocity(
    shotgun: &Shotgun,
    shell: &Shell,
    conditions: &Conditions,
) -> Result<f64, SolveError> {
    let calm = pellets::calm(conditions.atmosphere());
    if let Some(pellet) = pellet(shell) {
        return pellets::muzzle_velocity(
            &pellet,
            shell.velocity_mps,
            engine_velocity_at(shell.velocity_at),
            &calm,
        );
    }
    // A slug: find the muzzle velocity that gives the box's, flying it.
    let distance = match engine_velocity_at(shell.velocity_at) {
        pellets::VelocityAt::Muzzle => return Ok(shell.velocity_mps),
        pellets::VelocityAt::Distance(distance) | pellets::VelocityAt::AverageOver(distance) => {
            distance
        }
    };
    let measured = |muzzle: f64| -> Option<f64> {
        let mut scenario = slug_scenario(shotgun, shell, conditions, muzzle)?;
        scenario.shot.wind = Wind::CALM;
        let row = solver::solve(&scenario, distance + 1.0)
            .ok()?
            .at(distance)?;
        Some(match shell.velocity_at {
            VelocityAt::Observed => distance / row.time,
            _ => row.velocity,
        })
    };
    let (mut low, mut high) = (shell.velocity_mps, (shell.velocity_mps * 1.5).min(1500.0));
    if !measured(high).is_some_and(|fastest| fastest >= shell.velocity_mps) {
        return Err(SolveError::InvalidInput(
            "no muzzle velocity gives the slug's velocity there",
        ));
    }
    for _ in 0..50 {
        let middle = (low + high) / 2.0;
        if measured(middle).is_some_and(|got| got >= shell.velocity_mps) {
            high = middle;
        } else {
            low = middle;
        }
    }
    Ok((low + high) / 2.0)
}

/// The sights' height above the bore when the gun has none of its own set:
/// a bead on a rib.
pub const BEAD_HEIGHT_CM: f64 = 1.5;
/// The slug zero when the gun has none set.
pub const DEFAULT_SLUG_ZERO_M: f64 = 50.0;

/// The engine's scenario for a slug fired at `muzzle_mps`, the gun zeroed
/// in the same air; `None` for shot.
pub fn slug_scenario(
    shotgun: &Shotgun,
    shell: &Shell,
    conditions: &Conditions,
    muzzle_mps: f64,
) -> Option<Scenario> {
    let Payload::Slug {
        mass_g,
        diameter_mm,
        bc_g1,
        ..
    } = shell.payload
    else {
        return None;
    };
    // A slug is aimed at the angle of the shot, as a rifle is.
    let shot = ballistics_core::Conditions {
        inclination_degrees: conditions.inclination_degrees,
        ..engine_conditions(conditions)
    };
    Some(Scenario {
        bullet: ballistics_core::Bullet {
            mass_kg: mass_g / 1000.0,
            diameter_m: diameter_mm / 1000.0,
            length_m: None,
            ballistic_coefficient: bc_g1,
            drag_model: DragModel::G1,
            bc_bands: BcBands::NONE,
            drag_curve: None,
        },
        rifle: ballistics_core::Rifle {
            sight_height_m: shotgun.sight_height_cm.unwrap_or(BEAD_HEIGHT_CM) / 100.0,
            zero_range_m: shotgun.slug_zero_m.unwrap_or(DEFAULT_SLUG_ZERO_M),
            zero_height_m: 0.0,
            // A smooth bore: the slug barely spins, and spin drift is off.
            twist_m: 100.0,
            right_hand_twist: true,
        },
        load: Load {
            muzzle_velocity: muzzle_mps,
            reference_temperature_c: shot.powder_temperature_c,
            temperature_sensitivity: 0.0,
        },
        zero_atmosphere: shot.atmosphere,
        zero_powder_temperature_c: shot.powder_temperature_c,
        impact_offset: [0.0, 0.0],
        shot,
        spin_drift: false,
        aerodynamic_jump: false,
    })
}

/// Where a barrel's pattern percentage comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PatternFrom {
    /// The hunter's pattern boards with this shell and barrel (how many).
    Boards(usize),
    /// The choke's nominal pattern; tighter for hard shot.
    Choke { hard_shot: bool },
}

/// A barrel's pattern with a shell: its percentage of pellets in the
/// 30-inch circle at 40 yards, the measure the engine's pattern model
/// grows from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarrelPattern {
    pub percent: f64,
    pub from: PatternFrom,
}

/// The engine's choke for the store's.
pub fn choke(name: ChokeName) -> pellets::Choke {
    match name {
        ChokeName::Cylinder => pellets::Choke::Cylinder,
        ChokeName::Skeet => pellets::Choke::Skeet,
        ChokeName::ImprovedCylinder => pellets::Choke::ImprovedCylinder,
        ChokeName::LightModified => pellets::Choke::LightModified,
        ChokeName::Modified => pellets::Choke::Modified,
        ChokeName::ImprovedModified => pellets::Choke::ImprovedModified,
        ChokeName::LightFull => pellets::Choke::LightFull,
        ChokeName::Full => pellets::Choke::Full,
        ChokeName::ExtraFull => pellets::Choke::ExtraFull,
    }
}

/// A choke's nominal percentage with lead, in a gun of this gauge: a
/// constriction is read on the 12-bore scale, scaled to the bore.
pub fn choke_percent(choke_setting: Choke, bore_mm: f64) -> Result<f64, SolveError> {
    match choke_setting {
        Choke::Name { name } => Ok(choke(name).nominal_percent()),
        Choke::Constriction { mm } => {
            let twelve_bore_mm = mm * ballistics_store::Gauge::Twelve.bore_mm() / bore_mm;
            pellets::percent_from_constriction(twelve_bore_mm.min(3.0) / 1000.0)
        }
    }
}

/// The pattern of `shell` from the gun's barrel `barrel` (counted from 0),
/// from the pattern boards in `load` (the shell in this gun) when there are
/// any; `None` for a slug or a barrel the gun doesn't have.
pub fn barrel_pattern(
    shotgun: &Shotgun,
    barrel: usize,
    shell: &Shell,
    load: Option<&ShotgunLoad>,
) -> Option<BarrelPattern> {
    let Payload::Shot {
        material: shot,
        pellet_mm,
        load_g,
    } = shell.payload
    else {
        return None;
    };
    let setting = shotgun.barrels.get(barrel)?.choke;
    let in_load = Pellet::of(material(shot), pellet_mm / 1000.0).count_in(load_g / 1000.0);
    let boards: Vec<f64> = load
        .map_or(&[][..], |load| &load.patterns[..])
        .iter()
        .filter(|test| test.barrel == barrel && test.pellets_inside > 0)
        .filter_map(|test| {
            let all = test
                .pellets_in_load
                .map(|count| count as usize)
                .or(in_load)?;
            // More inside than the load holds is a miscount (or a load
            // that isn't what the shell says): leave it out.
            if test.pellets_inside as usize > all {
                return None;
            }
            let percent = 100.0 * test.pellets_inside as f64 / all.max(1) as f64;
            pellets::percent_from_test(
                percent.clamp(0.5, 99.5),
                test.circle_cm / 200.0,
                test.distance_m,
            )
            .ok()
        })
        .collect();
    if !boards.is_empty() {
        return Some(BarrelPattern {
            percent: boards.iter().sum::<f64>() / boards.len() as f64,
            from: PatternFrom::Boards(boards.len()),
        });
    }
    let lead = choke_percent(setting, shotgun.gauge().bore_mm()).ok()?;
    let hard_shot = shot.is_hard();
    let percent = if hard_shot {
        pellets::steel_percent(lead).ok()?
    } else {
        lead
    };
    Some(BarrelPattern {
        percent,
        from: PatternFrom::Choke { hard_shot },
    })
}

/// One range of a shot's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotRow {
    pub range_m: f64,
    pub velocity_mps: f64,
    /// One pellet's energy.
    pub pellet_energy_j: f64,
    /// Below the bore's line, cm.
    pub drop_cm: f64,
    /// Sideways in the day's wind, cm (positive right).
    pub drift_cm: f64,
    /// The circle holding 85 % of the pellets, cm across: about what pattern
    /// tables call the spread.
    pub spread_cm: f64,
    /// The share of the load in the Nordic test's 75 cm circle, %.
    pub in_75_cm_percent: f64,
    /// Pellets on each 10 × 10 cm in the pattern's middle.
    pub middle_per_dm2: f64,
    /// The shot string: the shortest length holding 80 % of the pellets, m.
    pub string_80_m: Option<f64>,
    /// The energy of a pellet at the back of the string, arriving after nine
    /// in ten.
    pub back_pellet_energy_j: Option<f64>,
}

/// A shot shell from one barrel, range by range.
#[derive(Clone, Debug)]
pub struct ShotTable {
    pub muzzle_velocity_mps: f64,
    pub pellets: usize,
    pub pellet_mass_g: f64,
    pub pattern: BarrelPattern,
    pub rows: Vec<ShotRow>,
    /// The farthest a pellet carries over level ground in this air.
    pub max_range: Option<MaxRange>,
    /// The pellets flown one by one: the string, and a shot's pellets.
    pub swarm: Arc<Swarm>,
}

/// The table for a shot shell from barrel `barrel`, at `ranges` (m, at
/// most 150); `None` for a slug.
pub fn shot_table(
    shotgun: &Shotgun,
    barrel: usize,
    shell: &Shell,
    load: Option<&ShotgunLoad>,
    conditions: &Conditions,
    ranges: &[f64],
) -> Result<Option<ShotTable>, SolveError> {
    let (Some(pellet), Some(pattern)) =
        (pellet(shell), barrel_pattern(shotgun, barrel, shell, load))
    else {
        return Ok(None);
    };
    let muzzle = muzzle_velocity(shotgun, shell, conditions)?;
    let Some(swarm) = swarm(shell, muzzle, conditions)? else {
        return Ok(None);
    };
    let rows = shot_rows(
        &pellet,
        &pattern,
        shell,
        muzzle,
        conditions,
        ranges,
        Some(&swarm),
    )?;
    let count = pellet_count(shell).unwrap_or(0);
    let max_range =
        pellets::max_range(&pellet, muzzle, &pellets::calm(conditions.atmosphere())).ok();
    Ok(Some(ShotTable {
        muzzle_velocity_mps: muzzle,
        pellets: count,
        pellet_mass_g: pellet.mass_kg() * 1000.0,
        pattern,
        rows,
        max_range,
        swarm: Arc::new(swarm),
    }))
}

/// The rows of a shot shell's table at `ranges` (m, at most 150), from its
/// muzzle velocity (worked out already) and its barrel's pattern; the
/// string from `swarm`, when given.
pub fn shot_rows(
    pellet: &Pellet,
    pattern: &BarrelPattern,
    shell: &Shell,
    muzzle_mps: f64,
    conditions: &Conditions,
    ranges: &[f64],
    swarm: Option<&Swarm>,
) -> Result<Vec<ShotRow>, SolveError> {
    let count = pellet_count(shell).unwrap_or(0);
    let farthest = ranges.iter().copied().fold(0.0, f64::max);
    let path = pellets::fly(pellet, muzzle_mps, &engine_conditions(conditions), farthest)?;
    let mut rows = Vec::new();
    for &range in ranges {
        let Some(sample) = solver::interpolate(&path, range) else {
            break;
        };
        let sigma = pellets::sigma_at(pattern.percent, range)?;
        let string = swarm.and_then(|swarm| swarm.string_at(range));
        rows.push(ShotRow {
            range_m: range,
            velocity_mps: sample.velocity,
            pellet_energy_j: pellet.energy_j(sample.velocity),
            drop_cm: -sample.drop * 100.0,
            drift_cm: sample.windage * 100.0,
            spread_cm: pellets::spread_diameter(pattern.percent, 0.85, range)? * 100.0,
            in_75_cm_percent: pellets::share_inside(
                pattern.percent,
                pellets::NORDIC_PATTERN_RADIUS_M,
                range,
            )? * 100.0,
            middle_per_dm2: if sigma > 0.0 {
                count as f64 * 0.01 / (2.0 * std::f64::consts::PI * sigma * sigma)
            } else {
                f64::INFINITY
            },
            string_80_m: string.map(|string| string.length_80_m),
            back_pellet_energy_j: string.map(|string| pellet.energy_j(string.back_velocity_mps)),
        });
    }
    Ok(rows)
}

/// A slug's rows at `ranges`, from its muzzle velocity (worked out already).
pub fn slug_rows(
    shotgun: &Shotgun,
    shell: &Shell,
    conditions: &Conditions,
    muzzle_mps: f64,
    ranges: &[f64],
) -> Result<Vec<ballistics_core::RangeRow>, SolveError> {
    let Some(scenario) = slug_scenario(shotgun, shell, conditions, muzzle_mps) else {
        return Ok(Vec::new());
    };
    let farthest = ranges.iter().copied().fold(0.0, f64::max);
    let solution = solver::solve(&scenario, farthest + 1.0)?;
    Ok(ranges
        .iter()
        .filter_map(|&range| solution.at(range))
        .collect())
}

/// A slug from the gun, range by range: its path against the sights,
/// velocity and energy; `None` for shot.
pub fn slug_table(
    shotgun: &Shotgun,
    shell: &Shell,
    conditions: &Conditions,
    ranges: &[f64],
) -> Result<Option<(f64, Vec<ballistics_core::RangeRow>)>, SolveError> {
    if pellet(shell).is_some() {
        return Ok(None);
    }
    let muzzle = muzzle_velocity(shotgun, shell, conditions)?;
    let Some(scenario) = slug_scenario(shotgun, shell, conditions, muzzle) else {
        return Ok(None);
    };
    let farthest = ranges.iter().copied().fold(0.0, f64::max);
    let solution = solver::solve(&scenario, farthest + 1.0)?;
    Ok(Some((
        muzzle,
        ranges
            .iter()
            .filter_map(|&range| solution.at(range))
            .collect(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::{PatternTest, ShotgunBarrel};

    fn calm() -> Conditions {
        Conditions {
            temperature_c: 15.0,
            pressure: crate::Pressure::Station(1013.25),
            humidity_percent: 50.0,
            powder_temperature_c: None,
            wind_speed_mps: 0.0,
            wind_clock: 3.0,
            inclination_degrees: 0.0,
            coriolis: None,
            spin_drift: false,
            aerodynamic_jump: false,
            cant_degrees: 0.0,
        }
    }

    fn gun() -> Shotgun {
        let mut gun = Shotgun::new();
        gun.name = "Test".into();
        gun
    }

    fn shot(material: ShotMaterial, pellet_mm: f64) -> Shell {
        let mut shell = Shell::new("12-70");
        shell.name = "Test".into();
        shell.payload = Payload::Shot {
            material,
            pellet_mm,
            load_g: 32.0,
        };
        shell
    }

    #[test]
    fn a_v2_5_on_the_box_is_below_the_muzzle_velocity() {
        let shell = shot(ShotMaterial::Lead, 3.0);
        let muzzle = muzzle_velocity(&gun(), &shell, &calm()).unwrap();
        assert!((420.0..435.0).contains(&muzzle), "{muzzle}");
        let at_muzzle = Shell {
            velocity_at: VelocityAt::Muzzle,
            ..shell
        };
        assert_eq!(muzzle_velocity(&gun(), &at_muzzle, &calm()).unwrap(), 400.0);
    }

    #[test]
    fn a_barrel_patterns_by_its_choke_tighter_with_steel_or_by_the_boards() {
        let gun = gun();
        // Under: quarter choke (55 %); over: half (60 %).
        let lead = shot(ShotMaterial::Lead, 3.0);
        assert_eq!(barrel_pattern(&gun, 0, &lead, None).unwrap().percent, 55.0);
        let steel = shot(ShotMaterial::Steel, 3.5);
        let pattern = barrel_pattern(&gun, 1, &steel, None).unwrap();
        assert_eq!(pattern.percent, 70.0);
        assert_eq!(pattern.from, PatternFrom::Choke { hard_shot: true });
        assert!(barrel_pattern(&gun, 2, &lead, None).is_none());

        // A board: 75 % of the load in the 75 cm circle at 35 m is about a
        // 71–73 % pattern at 40 yards.
        let mut counted = ShotgunLoad::new(&gun.id, &lead.id);
        let all = pellet_count(&lead).unwrap() as u32;
        counted.patterns.push(PatternTest {
            barrel: 1,
            distance_m: 35.0,
            circle_cm: 75.0,
            pellets_inside: all * 3 / 4,
            pellets_in_load: None,
        });
        let board = barrel_pattern(&gun, 1, &lead, Some(&counted)).unwrap();
        assert_eq!(board.from, PatternFrom::Boards(1));
        assert!((70.0..74.0).contains(&board.percent), "{}", board.percent);
        // The other barrel keeps its choke.
        assert_eq!(
            barrel_pattern(&gun, 0, &lead, Some(&counted))
                .unwrap()
                .percent,
            55.0
        );
        // A board not counted yet, or counting more than the load holds, is
        // left out.
        for inside in [0, all + 5] {
            let mut miscounted = ShotgunLoad::new(&gun.id, &lead.id);
            miscounted.patterns.push(PatternTest {
                pellets_inside: inside,
                ..counted.patterns[0]
            });
            let pattern = barrel_pattern(&gun, 1, &lead, Some(&miscounted)).unwrap();
            assert_eq!(
                pattern.from,
                PatternFrom::Choke { hard_shot: false },
                "{inside}"
            );
        }

        // A 20 bore's constriction counts for more than a 12's.
        let mut twenty = gun.clone();
        twenty.caliber = "20-76".to_owned();
        twenty.barrels = vec![ShotgunBarrel {
            name: String::new(),
            choke: Choke::Constriction { mm: 0.42 },
        }];
        let in_twenty = barrel_pattern(&twenty, 0, &lead, None).unwrap().percent;
        assert!((59.0..61.5).contains(&in_twenty), "{in_twenty}");
    }

    #[test]
    fn a_shot_table_goes_out_to_the_ranges_asked_for() {
        let shell = shot(ShotMaterial::Lead, 3.0);
        let table = shot_table(&gun(), 1, &shell, None, &calm(), &[10.0, 20.0, 30.0, 40.0])
            .unwrap()
            .unwrap();
        // Svenska Jägareförbundet: 32 g of US 5 (3.0 mm) lead, 200 pellets.
        assert!((195..210).contains(&table.pellets), "{}", table.pellets);
        assert_eq!(table.rows.len(), 4);
        let at_40 = table.rows[3];
        assert!(at_40.velocity_mps < table.rows[0].velocity_mps);
        assert!(at_40.spread_cm > table.rows[0].spread_cm);
        assert!(at_40.in_75_cm_percent < 80.0 && at_40.in_75_cm_percent > 40.0);
        assert!(at_40.drop_cm > 0.0 && at_40.drift_cm.abs() < 1e-9);
        let farthest = table.max_range.unwrap().distance_m;
        assert!((220.0..260.0).contains(&farthest), "{farthest}");
        assert!(
            slug_table(&gun(), &shell, &calm(), &[50.0])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_slug_flies_as_brenneke_tabled_it() {
        let mut gun = gun();
        gun.barrels.truncate(1);
        let mut slug = Shell::new("12-70");
        slug.name = "Brenneke Classic".into();
        slug.payload = Payload::Slug {
            mass_g: 31.5,
            diameter_mm: 18.5,
            bc_g1: 0.071,
            lead_free: false,
        };
        slug.velocity_mps = 430.0;
        slug.velocity_at = VelocityAt::Muzzle;
        let (muzzle, rows) = slug_table(&gun, &slug, &calm(), &[25.0, 50.0, 100.0])
            .unwrap()
            .unwrap();
        assert_eq!(muzzle, 430.0);
        for (row, tabled) in rows.iter().zip([370.0, 330.0, 275.0]) {
            assert!((row.velocity / tabled - 1.0).abs() < 0.02, "{row:?}");
        }
        // Zeroed at 50 m.
        assert!(rows[1].drop.abs() < 0.005, "{}", rows[1].drop);
        // A V2.5 on the box: the muzzle is a little faster.
        slug.velocity_at = VelocityAt::V2_5;
        let from_v2_5 = muzzle_velocity(&gun, &slug, &calm()).unwrap();
        assert!((431.0..440.0).contains(&from_v2_5), "{from_v2_5}");
        assert!(
            shot_table(&gun, 0, &slug, None, &calm(), &[25.0])
                .unwrap()
                .is_none()
        );
    }
}

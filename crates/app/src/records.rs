//! Changes to a rifle's history (barrels, rounds, cleans), to range
//! sessions and to a load's velocities, as the Rifles and Sessions pages
//! and a group's velocities ask for them. Each is applied to the store at
//! once, and says what was done.

use ballistics_store::model::now_ms;
use ballistics_store::{
    Barrel, Clean, Rifle, RifleLoad, RoundLogEntry, Session, Store, StoreError,
};

use crate::app::Data;
use crate::i18n::{tr, trf};

/// One change to the hunter's records.
pub enum Change {
    /// Save a barrel. A new one is fitted to its rifle at once.
    SaveBarrel(Barrel),
    /// Fit a barrel to its rifle; the barrel fitted before comes off.
    FitBarrel(String),
    /// Take a barrel off for good.
    RetireBarrel(String),
    /// Delete a barrel. Its round log stays: the rounds went through the
    /// action too.
    DeleteBarrel(String),
    /// Copy the barrel's length and twist into its rifle's ballistics.
    CopyBarrelToRifle(String),
    LogRounds(RoundLogEntry),
    DeleteRounds(String),
    MarkCleaned(Clean),
    SaveSession(Session),
    /// Delete a session. Its groups keep its conditions as their own.
    DeleteSession(String),
    /// A velocity measured with a group, for its rifle and bullet.
    UseVelocity(RifleLoad),
}

/// What a change did.
pub struct Applied {
    pub message: String,
    /// The rifle, when its record changed with it.
    pub rifle: Option<Rifle>,
    /// A rifle's load changed.
    pub load: bool,
    /// The session as saved.
    pub session: Option<Session>,
    /// The session deleted.
    pub session_deleted: Option<String>,
}

impl Applied {
    fn said(message: impl Into<String>) -> Applied {
        Applied {
            message: message.into(),
            rifle: None,
            load: false,
            session: None,
            session_deleted: None,
        }
    }
}

fn missing() -> StoreError {
    StoreError::Invalid(tr("That record is no longer on this device").to_owned())
}

fn barrel(data: &Data, id: &str) -> Result<Barrel, StoreError> {
    data.groups
        .barrels
        .iter()
        .find(|barrel| barrel.id == id)
        .cloned()
        .ok_or_else(missing)
}

fn rifle(data: &Data, id: &str) -> Result<Rifle, StoreError> {
    data.rifles
        .iter()
        .find(|rifle| rifle.id == id)
        .cloned()
        .ok_or_else(missing)
}

/// Fits `barrel` to its rifle and saves both.
fn fit(store: &Store, data: &Data, mut barrel: Barrel, at: i64) -> Result<Rifle, StoreError> {
    let mut rifle = rifle(data, &barrel.rifle_id)?;
    barrel.fitted_at = Some(at);
    barrel.retired_at = None;
    store.save_barrel(&mut barrel)?;
    rifle.fitted_barrel_id = Some(barrel.id.clone());
    store.save_rifle(&mut rifle)?;
    Ok(rifle)
}

pub fn apply(store: &Store, data: &Data, change: Change) -> Result<Applied, StoreError> {
    let now = now_ms();
    match change {
        Change::SaveBarrel(mut barrel) => {
            let new = !data.groups.barrels.iter().any(|b| b.id == barrel.id);
            barrel.validate().map_err(StoreError::Invalid)?;
            if new {
                let name = barrel.name.clone();
                let rifle = fit(store, data, barrel, now)?;
                Ok(Applied {
                    message: trf!("Added {} and fitted it", name),
                    rifle: Some(rifle),
                    load: false,
                    ..Applied::said("")
                })
            } else {
                store.save_barrel(&mut barrel)?;
                Ok(Applied::said(trf!("Saved {}", barrel.name)))
            }
        }
        Change::FitBarrel(id) => {
            let barrel = barrel(data, &id)?;
            let name = barrel.name.clone();
            let rifle = fit(store, data, barrel, now)?;
            Ok(Applied {
                message: trf!("{} is fitted", name),
                rifle: Some(rifle),
                load: false,
                ..Applied::said("")
            })
        }
        Change::RetireBarrel(id) => {
            let mut barrel = barrel(data, &id)?;
            barrel.retired_at = Some(barrel.fitted_at.map_or(now, |fitted| fitted.max(now)));
            store.save_barrel(&mut barrel)?;
            let mut changed = None;
            if let Ok(mut rifle) = rifle(data, &barrel.rifle_id)
                && rifle.fitted_barrel_id.as_deref() == Some(id.as_str())
            {
                rifle.fitted_barrel_id = None;
                store.save_rifle(&mut rifle)?;
                changed = Some(rifle);
            }
            Ok(Applied {
                message: trf!("{} is retired", barrel.name),
                rifle: changed,
                load: false,
                ..Applied::said("")
            })
        }
        Change::DeleteBarrel(id) => {
            let barrel = barrel(data, &id)?;
            store.delete_barrel(&id)?;
            let rifle = store.rifle(&barrel.rifle_id)?;
            Ok(Applied {
                message: trf!(
                    "{} deleted; its rounds still count for the action",
                    barrel.name
                ),
                rifle,
                load: false,
                ..Applied::said("")
            })
        }
        Change::CopyBarrelToRifle(id) => {
            let barrel = barrel(data, &id)?;
            let mut rifle = rifle(data, &barrel.rifle_id)?;
            if barrel.length_mm.is_some() {
                rifle.barrel_length_mm = barrel.length_mm;
            }
            if let Some(twist) = barrel.twist_in {
                rifle.twist_in = twist;
                rifle.right_hand_twist = barrel.right_hand_twist;
            }
            store.save_rifle(&mut rifle)?;
            Ok(Applied {
                message: trf!("The rifle's ballistics now use {}", barrel.name),
                rifle: Some(rifle),
                load: false,
                ..Applied::said("")
            })
        }
        Change::LogRounds(mut entry) => {
            entry.validate().map_err(StoreError::Invalid)?;
            store.save_round_log_entry(&mut entry)?;
            Ok(Applied::said(if entry.rounds == 1 {
                tr("Logged 1 round").to_owned()
            } else {
                trf!("Logged {} rounds", entry.rounds)
            }))
        }
        Change::DeleteRounds(id) => {
            store.delete_round_log_entry(&id)?;
            Ok(Applied::said(tr("Round log entry deleted")))
        }
        Change::MarkCleaned(mut clean) => {
            clean.validate().map_err(StoreError::Invalid)?;
            store.save_clean(&mut clean)?;
            Ok(Applied::said(tr("Marked as cleaned")))
        }
        Change::SaveSession(mut session) => {
            session.validate().map_err(StoreError::Invalid)?;
            store.save_session(&mut session)?;
            Ok(Applied {
                session: Some(session),
                ..Applied::said(tr("Saved the session"))
            })
        }
        Change::DeleteSession(id) => {
            let session = data
                .groups
                .sessions
                .iter()
                .find(|session| session.id == id)
                .ok_or_else(missing)?;
            for group in data
                .groups
                .groups
                .iter()
                .filter(|group| group.session_id.as_deref() == Some(id.as_str()))
            {
                let mut group = group.clone();
                group.conditions = group.conditions.or(session.conditions);
                group.session_id = None;
                store.save_group(&mut group)?;
            }
            store.delete_session(&id)?;
            Ok(Applied {
                session_deleted: Some(id),
                ..Applied::said(tr(
                    "Session deleted; its groups keep its weather as their own",
                ))
            })
        }
        Change::UseVelocity(mut load) => {
            load.validate().map_err(StoreError::Invalid)?;
            store.save_rifle_load(&mut load)?;
            Ok(Applied {
                load: true,
                ..Applied::said(tr("Saved as this load's velocity: the calculator uses it"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::{Conditions, Group};

    fn data(store: &Store) -> Data {
        Data {
            rifles: store.rifles().unwrap(),
            loads: store.rifle_loads().unwrap(),
            groups: store.groups_data().unwrap(),
            ..Data::default()
        }
    }

    #[test]
    fn a_new_barrel_is_fitted_and_a_retired_one_comes_off() {
        let store = Store::open_in_memory().unwrap();
        let mut rifle = Rifle::new("Tikka", "308-win", 11.0);
        store.save_rifle(&mut rifle).unwrap();
        let first = Barrel::new(&rifle.id, "Factory");
        apply(&store, &data(&store), Change::SaveBarrel(first.clone())).unwrap();
        assert_eq!(
            store.rifle(&rifle.id).unwrap().unwrap().fitted_barrel_id,
            Some(first.id.clone())
        );
        let second = Barrel::new(&rifle.id, "Proof");
        apply(&store, &data(&store), Change::SaveBarrel(second.clone())).unwrap();
        assert_eq!(
            store.rifle(&rifle.id).unwrap().unwrap().fitted_barrel_id,
            Some(second.id.clone()),
            "the new barrel replaces the old"
        );
        let applied = apply(
            &store,
            &data(&store),
            Change::RetireBarrel(second.id.clone()),
        )
        .unwrap();
        assert_eq!(applied.rifle.unwrap().fitted_barrel_id, None);
        assert!(
            store
                .barrel(&second.id)
                .unwrap()
                .unwrap()
                .retired_at
                .is_some()
        );
        apply(&store, &data(&store), Change::FitBarrel(first.id.clone())).unwrap();
        let mut entry = RoundLogEntry::new(&rifle.id, Some(&first.id), 20, 1_000);
        apply(&store, &data(&store), Change::LogRounds(entry.clone())).unwrap();
        apply(
            &store,
            &data(&store),
            Change::DeleteBarrel(first.id.clone()),
        )
        .unwrap();
        entry.updated_at = store.round_log().unwrap()[0].updated_at;
        assert_eq!(store.round_log().unwrap(), vec![entry], "its rounds stay");
        assert_eq!(
            store.rifle(&rifle.id).unwrap().unwrap().fitted_barrel_id,
            None
        );
    }

    #[test]
    fn a_deleted_sessions_groups_keep_its_weather() {
        let store = Store::open_in_memory().unwrap();
        let mut session = Session::new(1_000);
        session.conditions = Conditions {
            temperature_c: Some(4.0),
            pressure_hpa: Some(990.0),
            ..Conditions::default()
        };
        store.save_session(&mut session).unwrap();
        let mut group = Group::on_grid(200.0, 200.0, 2_000);
        group.session_id = Some(session.id.clone());
        group.conditions.temperature_c = Some(6.0);
        store.save_group(&mut group).unwrap();
        apply(
            &store,
            &data(&store),
            Change::DeleteSession(session.id.clone()),
        )
        .unwrap();
        assert!(store.sessions().unwrap().is_empty());
        let kept = store.group(&group.id).unwrap().unwrap();
        assert_eq!(kept.session_id, None);
        assert_eq!(kept.conditions.temperature_c, Some(6.0), "its own stays");
        assert_eq!(kept.conditions.pressure_hpa, Some(990.0));
    }
}

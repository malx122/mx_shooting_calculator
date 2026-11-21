//! A group's entry in the round log, kept in step with the group: its shot
//! count, rifle, barrel and time, or gone when the group no longer counts.

use ballistics_store::{Group, RoundLogEntry};

/// What the round log needs after a group is saved.
#[derive(Clone, Debug, PartialEq)]
pub enum RoundSync {
    /// Save this entry (new, or the group's entry brought up to date).
    Save(RoundLogEntry),
    /// Delete the group's entry, by id.
    Delete(String),
    /// The log is as it should be.
    Nothing,
}

/// The change that keeps `existing`, the group's entry if it has one, in
/// step with `group`: an entry of all its shots on its rifle and barrel when
/// it counts its rounds and names a rifle this device has (`rifle_known`),
/// none otherwise.
pub fn sync(group: &Group, existing: Option<&RoundLogEntry>, rifle_known: bool) -> RoundSync {
    let rounds = u32::try_from(group.shot_count()).unwrap_or(u32::MAX);
    let wanted = if group.count_rounds && rounds > 0 && rifle_known {
        RoundLogEntry::for_group(group, rounds)
    } else {
        None
    };
    match (wanted, existing) {
        (Some(mut entry), Some(existing)) => {
            entry.id = existing.id.clone();
            entry.note = existing.note.clone();
            entry.updated_at = existing.updated_at;
            if entry == *existing {
                RoundSync::Nothing
            } else {
                RoundSync::Save(entry)
            }
        }
        (Some(entry), None) => RoundSync::Save(entry),
        (None, Some(existing)) => RoundSync::Delete(existing.id.clone()),
        (None, None) => RoundSync::Nothing,
    }
}

/// The group's entry in `log`, if it has one.
pub fn entry_for<'a>(log: &'a [RoundLogEntry], group_id: &str) -> Option<&'a RoundLogEntry> {
    log.iter()
        .find(|entry| entry.group_id.as_deref() == Some(group_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::GridShot;
    use ballistics_store::groups::Marks;

    fn group(shots: usize) -> Group {
        let mut group = Group::on_grid(200.0, 200.0, 1_000);
        group.rifle_id = Some("rifle".into());
        group.barrel_id = Some("barrel".into());
        if let Marks::Grid(grid) = &mut group.marks {
            grid.shots = (0..shots)
                .map(|i| GridShot {
                    x_mm: i as f64,
                    y_mm: 0.0,
                    cold_bore: false,
                    called_flier: false,
                    velocity_mps: None,
                })
                .collect();
        }
        group
    }

    #[test]
    fn a_counted_group_logs_its_shots_and_follows_their_count() {
        let mut group = group(5);
        let RoundSync::Save(entry) = sync(&group, None, true) else {
            panic!("a new entry");
        };
        assert_eq!(entry.rounds, 5);
        assert_eq!(entry.id, RoundLogEntry::id_for_group(&group.id));
        assert_eq!(entry.group_id.as_deref(), Some(group.id.as_str()));
        assert_eq!(entry.barrel_id.as_deref(), Some("barrel"));
        assert_eq!(sync(&group, Some(&entry), true), RoundSync::Nothing);

        let mut noted = entry.clone();
        noted.note = "foulers".into();
        noted.updated_at = 77;
        group = self::group(7);
        let RoundSync::Save(updated) = sync(&group, Some(&noted), true) else {
            panic!("the count follows the shots");
        };
        assert_eq!(updated.rounds, 7);
        assert_eq!(updated.id, noted.id, "the same entry");
        assert_eq!(updated.note, "foulers", "its note is kept");

        group.barrel_id = None;
        let RoundSync::Save(moved) = sync(&group, Some(&updated), true) else {
            panic!("the barrel follows the group");
        };
        assert_eq!(moved.barrel_id, None);
    }

    #[test]
    fn the_entry_goes_when_the_group_stops_counting() {
        let mut group = group(5);
        let RoundSync::Save(entry) = sync(&group, None, true) else {
            panic!("a new entry");
        };
        group.count_rounds = false;
        assert_eq!(
            sync(&group, Some(&entry), true),
            RoundSync::Delete(entry.id.clone())
        );
        assert_eq!(sync(&group, None, true), RoundSync::Nothing);

        let mut empty = self::group(0);
        assert_eq!(
            sync(&empty, Some(&entry), true),
            RoundSync::Delete(entry.id.clone())
        );
        empty = self::group(3);
        empty.rifle_id = None;
        assert_eq!(
            sync(&empty, Some(&entry), true),
            RoundSync::Delete(entry.id.clone()),
            "rounds are fired from a rifle"
        );
        assert_eq!(
            entry_for(std::slice::from_ref(&entry), &group.id),
            Some(&entry)
        );
        assert_eq!(entry_for(&[entry], "another"), None);
    }
}

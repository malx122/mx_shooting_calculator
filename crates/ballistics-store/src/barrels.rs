//! A rifle's history: its barrels, the rounds fired through it, when it was
//! cleaned and when its zero was set again. Round counts are never stored:
//! `RoundCounts` works them out from the log.

use serde::{Deserialize, Serialize};

use crate::groups::Group;
use crate::model::{MAX_ROUNDS, NAME_MAX, Rifle, first_failure, new_id};

/// The namespace of the ids of round log entries made from groups.
const GROUP_ROUNDS: uuid::Uuid = uuid::Uuid::from_u128(0x6d78_7363_726f_756e_6473_8f2a_41c6_d903);

/// The most rounds one entry of the round log may hold.
pub const MAX_ROUNDS_PER_ENTRY: u32 = 10_000;

/// A barrel that is or was on one of the hunter's rifles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "RifleBarrel")]
pub struct Barrel {
    /// The barrel's id, unique among barrels.
    pub id: String,
    /// The rifle (one of the file's `rifles`) the barrel belongs to.
    pub rifle_id: String,
    /// The hunter's name for it, e.g. `Factory` or `Bartlein 26"`.
    pub name: String,
    /// Its length; `null` when not known.
    #[serde(default)]
    pub length_mm: Option<f64>,
    /// Barrel length for one turn of the rifling, in inches; `null` when not
    /// known.
    #[serde(default)]
    pub twist_in: Option<f64>,
    /// Whether the rifling turns to the right, as most do.
    #[serde(default = "yes")]
    pub right_hand_twist: bool,
    /// Who made it.
    #[serde(default)]
    pub maker: String,
    /// When it was fitted to the rifle, in milliseconds since 1970 (UTC), if
    /// known.
    #[serde(default)]
    pub fitted_at: Option<i64>,
    /// When it came off for good; `null` while it is still in use.
    #[serde(default)]
    pub retired_at: Option<i64>,
    /// Rounds fired through it before the hunter began the round log; the
    /// logged rounds add to it.
    #[serde(default)]
    pub rounds_before_logging: u32,
    /// The hunter's notes.
    #[serde(default)]
    pub notes: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

fn yes() -> bool {
    true
}

impl Barrel {
    pub fn new(rifle_id: &str, name: impl Into<String>) -> Self {
        Barrel {
            id: new_id(),
            rifle_id: rifle_id.to_owned(),
            name: name.into(),
            length_mm: None,
            twist_in: None,
            right_hand_twist: true,
            maker: String::new(),
            fitted_at: None,
            retired_at: None,
            rounds_before_logging: 0,
            notes: String::new(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a barrel needs an id"),
            (!self.rifle_id.is_empty(), "a barrel belongs to a rifle"),
            (!self.name.trim().is_empty(), "the barrel needs a name"),
            (
                self.name.chars().count() <= NAME_MAX && self.maker.chars().count() <= NAME_MAX,
                "the name is too long",
            ),
            (
                self.length_mm
                    .is_none_or(|mm| (100.0..=1500.0).contains(&mm)),
                "barrel length must be 100 to 1500 mm",
            ),
            (
                self.twist_in
                    .is_none_or(|inches| (3.0..=40.0).contains(&inches)),
                "twist must be 1 turn in 3 to 40 inches",
            ),
            (
                self.fitted_at.is_none_or(|at| at >= 0) && self.retired_at.is_none_or(|at| at >= 0),
                "the barrel's dates are before 1970",
            ),
            (
                match (self.fitted_at, self.retired_at) {
                    (Some(fitted), Some(retired)) => retired >= fitted,
                    _ => true,
                },
                "the barrel came off before it was fitted",
            ),
            (
                self.rounds_before_logging <= MAX_ROUNDS,
                "the rounds fired before the log must be at most 1 000 000",
            ),
        ];
        first_failure(&checks, &self.name)
    }
}

/// Rounds fired through a rifle at one time, for the round counts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RoundLogEntry {
    /// The entry's id, unique among round log entries.
    pub id: String,
    /// The rifle fired (one of the file's `rifles`).
    pub rifle_id: String,
    /// The barrel they went through (one of the file's `barrels`); `null`
    /// when the hunter keeps no barrels for the rifle or didn't say: then
    /// they count for the action only.
    #[serde(default)]
    pub barrel_id: Option<String>,
    /// How many rounds, 1 to 10 000.
    pub rounds: u32,
    /// When they were fired, in milliseconds since 1970 (UTC).
    pub at: i64,
    /// The hunter's note.
    #[serde(default)]
    pub note: String,
    /// The group (one of the file's `groups`) these rounds were, when the
    /// entry was made by saving one. Such an entry's id is the UUID (version
    /// 5) of the group's id in the namespace
    /// `6d787363-726f-756e-6473-8f2a41c6d903`, so every device makes the same
    /// one.
    #[serde(default)]
    pub group_id: Option<String>,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl RoundLogEntry {
    pub fn new(rifle_id: &str, barrel_id: Option<&str>, rounds: u32, at: i64) -> Self {
        RoundLogEntry {
            id: new_id(),
            rifle_id: rifle_id.to_owned(),
            barrel_id: barrel_id.map(str::to_owned),
            rounds,
            at,
            note: String::new(),
            group_id: None,
            updated_at: 0,
        }
    }

    /// The id of the entry logging a group's rounds: the same on every
    /// device, so a group saved on two is counted once.
    pub fn id_for_group(group_id: &str) -> String {
        uuid::Uuid::new_v5(&GROUP_ROUNDS, group_id.as_bytes()).to_string()
    }

    /// The entry logging `rounds` fired for a group, through its rifle and
    /// barrel when it was shot; `None` when the group names no rifle.
    pub fn for_group(group: &Group, rounds: u32) -> Option<Self> {
        let mut entry = RoundLogEntry::new(
            group.rifle_id.as_deref()?,
            group.barrel_id.as_deref(),
            rounds,
            group.taken_at,
        );
        entry.id = RoundLogEntry::id_for_group(&group.id);
        entry.group_id = Some(group.id.clone());
        Some(entry)
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a round log entry needs an id"),
            (!self.rifle_id.is_empty(), "rounds are fired from a rifle"),
            (
                (1..=MAX_ROUNDS_PER_ENTRY).contains(&self.rounds),
                "an entry holds 1 to 10 000 rounds",
            ),
            (self.at >= 0, "the rounds' time is before 1970"),
        ];
        first_failure(&checks, "")
    }
}

/// A cleaning of a rifle's barrel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Clean {
    /// The clean's id, unique among cleans.
    pub id: String,
    /// The rifle cleaned (one of the file's `rifles`).
    pub rifle_id: String,
    /// The barrel cleaned (one of the file's `barrels`); `null` for a rifle
    /// whose barrels the hunter doesn't keep apart.
    #[serde(default)]
    pub barrel_id: Option<String>,
    /// When, in milliseconds since 1970 (UTC).
    pub at: i64,
    /// The hunter's note, e.g. what was used.
    #[serde(default)]
    pub note: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl Clean {
    pub fn new(rifle_id: &str, barrel_id: Option<&str>, at: i64) -> Self {
        Clean {
            id: new_id(),
            rifle_id: rifle_id.to_owned(),
            barrel_id: barrel_id.map(str::to_owned),
            at,
            note: String::new(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a clean needs an id"),
            (!self.rifle_id.is_empty(), "a clean is of a rifle"),
            (self.at >= 0, "the clean's time is before 1970"),
        ];
        first_failure(&checks, "")
    }
}

/// The moment a rifle's zero was set again (a new scope, a new mount, a
/// new zero): zero checks pool only the groups shot since.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ZeroReset {
    /// The reset's id, unique among zero resets.
    pub id: String,
    /// The rifle (one of the file's `rifles`).
    pub rifle_id: String,
    /// The barrel (one of the file's `barrels`) the zero is for; `null` for
    /// the rifle whatever its barrel.
    #[serde(default)]
    pub barrel_id: Option<String>,
    /// When, in milliseconds since 1970 (UTC).
    pub at: i64,
    /// The hunter's note, e.g. why.
    #[serde(default)]
    pub note: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

impl ZeroReset {
    pub fn new(rifle_id: &str, barrel_id: Option<&str>, at: i64) -> Self {
        ZeroReset {
            id: new_id(),
            rifle_id: rifle_id.to_owned(),
            barrel_id: barrel_id.map(str::to_owned),
            at,
            note: String::new(),
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let checks = [
            (!self.id.trim().is_empty(), "a zero reset needs an id"),
            (!self.rifle_id.is_empty(), "a zero reset is of a rifle"),
            (self.at >= 0, "the zero reset's time is before 1970"),
        ];
        first_failure(&checks, "")
    }
}

/// The round counts of a rifle and one of its barrels, worked out from the
/// round log and the cleans.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoundCounts {
    /// Rounds through the action: the rifle's rounds before logging and
    /// every logged round, whatever the barrel.
    pub action: u64,
    /// Rounds through the barrel: its rounds before logging and the rounds
    /// logged to it; `None` when no barrel was asked for.
    pub barrel: Option<u64>,
    /// Rounds logged to the barrel (or, with no barrel, to the rifle without
    /// one) after its latest clean; `None` when it has no clean logged.
    /// A clean naming no barrel counts only for a rifle without barrel
    /// records: where there are barrels, cleans name the one fitted.
    pub since_clean: Option<u64>,
    /// When that clean was, in milliseconds since 1970 (UTC).
    pub last_clean_at: Option<i64>,
}

impl RoundCounts {
    /// The counts for `rifle` and `barrel`, from the whole log and every
    /// clean. A barrel's rounds and cleans are found by the barrel alone,
    /// so they follow it to another rifle; with `barrel` `None` (a rifle
    /// without barrel records) they are the rifle's entries and cleans that
    /// name no barrel. A clean at the same moment as rounds counts as after
    /// them.
    pub fn derive(
        rifle: &Rifle,
        barrel: Option<&Barrel>,
        log: &[RoundLogEntry],
        cleans: &[Clean],
    ) -> RoundCounts {
        let through = |entry: &RoundLogEntry| match barrel {
            Some(barrel) => entry.barrel_id.as_deref() == Some(barrel.id.as_str()),
            None => entry.rifle_id == rifle.id && entry.barrel_id.is_none(),
        };
        let cleaned = |clean: &Clean| match barrel {
            Some(barrel) => clean.barrel_id.as_deref() == Some(barrel.id.as_str()),
            None => clean.rifle_id == rifle.id && clean.barrel_id.is_none(),
        };
        let rounds = |entries: &mut dyn Iterator<Item = &RoundLogEntry>| {
            entries.map(|entry| u64::from(entry.rounds)).sum::<u64>()
        };
        let last_clean_at = cleans
            .iter()
            .filter(|clean| cleaned(clean))
            .map(|clean| clean.at)
            .max();
        RoundCounts {
            action: u64::from(rifle.rounds_before_logging)
                + rounds(&mut log.iter().filter(|entry| entry.rifle_id == rifle.id)),
            barrel: barrel.map(|barrel| {
                u64::from(barrel.rounds_before_logging)
                    + rounds(&mut log.iter().filter(|entry| through(entry)))
            }),
            since_clean: last_clean_at
                .map(|at| rounds(&mut log.iter().filter(|entry| through(entry) && entry.at > at))),
            last_clean_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_come_from_the_log_and_the_cleans() {
        let mut rifle = Rifle::new("Sako", "308-win", 11.0);
        rifle.rounds_before_logging = 400;
        let mut factory = Barrel::new(&rifle.id, "Factory");
        factory.rounds_before_logging = 350;
        let match_barrel = Barrel::new(&rifle.id, "Match");
        let other = Rifle::new("Other", "308-win", 11.0);
        let log = vec![
            RoundLogEntry::new(&rifle.id, Some(&factory.id), 20, 1_000),
            RoundLogEntry::new(&rifle.id, Some(&factory.id), 5, 3_000),
            RoundLogEntry::new(&rifle.id, Some(&match_barrel.id), 40, 2_000),
            RoundLogEntry::new(&rifle.id, None, 3, 2_500),
            RoundLogEntry::new(&other.id, None, 100, 2_000),
        ];
        let cleans = vec![
            Clean::new(&rifle.id, Some(&factory.id), 500),
            Clean::new(&rifle.id, Some(&factory.id), 1_000),
            Clean::new(&other.id, None, 2_900),
        ];

        let factory_counts = RoundCounts::derive(&rifle, Some(&factory), &log, &cleans);
        assert_eq!(
            factory_counts,
            RoundCounts {
                action: 400 + 20 + 5 + 40 + 3,
                barrel: Some(350 + 25),
                // The rounds at the clean's own moment were before it.
                since_clean: Some(5),
                last_clean_at: Some(1_000),
            }
        );
        let match_counts = RoundCounts::derive(&rifle, Some(&match_barrel), &log, &cleans);
        assert_eq!(match_counts.barrel, Some(40));
        assert_eq!(match_counts.since_clean, None, "never cleaned");
        let without = RoundCounts::derive(&rifle, None, &log, &cleans);
        assert_eq!((without.action, without.barrel), (468, None));
        assert_eq!(without.since_clean, None);

        let cleaned = [Clean::new(&rifle.id, None, 2_400)];
        assert_eq!(
            RoundCounts::derive(&rifle, None, &log, &cleaned).since_clean,
            Some(3)
        );
        assert_eq!(
            RoundCounts::derive(&rifle, None, &[], &[]),
            RoundCounts {
                action: 400,
                ..RoundCounts::default()
            }
        );
    }

    #[test]
    fn a_barrel_moved_to_another_rifle_keeps_its_counts() {
        let first = Rifle::new("Sako", "308-win", 11.0);
        let second = Rifle::new("Tikka", "308-win", 11.0);
        let mut barrel = Barrel::new(&first.id, "Match");
        let log = vec![
            RoundLogEntry::new(&first.id, Some(&barrel.id), 30, 1_000),
            RoundLogEntry::new(&first.id, Some(&barrel.id), 10, 3_000),
            RoundLogEntry::new(&second.id, Some(&barrel.id), 4, 5_000),
        ];
        let cleans = vec![Clean::new(&first.id, Some(&barrel.id), 2_000)];
        barrel.rifle_id = second.id.clone();
        let counts = RoundCounts::derive(&second, Some(&barrel), &log, &cleans);
        assert_eq!(counts.barrel, Some(44));
        assert_eq!(counts.since_clean, Some(14));
        assert_eq!(counts.action, 4, "the action counts its own rounds");
    }

    #[test]
    fn impossible_history_is_refused() {
        let rifle = Rifle::new("Sako", "308-win", 11.0);
        let mut barrel = Barrel::new(&rifle.id, "Factory");
        assert!(barrel.validate().is_ok());
        barrel.fitted_at = Some(2_000);
        barrel.retired_at = Some(1_000);
        assert!(barrel.validate().is_err());
        barrel.retired_at = None;
        barrel.twist_in = Some(1.0);
        assert!(barrel.validate().is_err());

        let mut entry = RoundLogEntry::new(&rifle.id, None, 0, 0);
        assert!(entry.validate().is_err());
        entry.rounds = MAX_ROUNDS_PER_ENTRY;
        assert!(entry.validate().is_ok());
        entry.at = -1;
        assert!(entry.validate().is_err());
        assert!(Clean::new("", None, 0).validate().is_err());
        assert!(ZeroReset::new(&rifle.id, None, -5).validate().is_err());
    }
}

//! Each country's hunting rules for rifle ammunition: the levels its law
//! sets (Sweden's classes 1–4, say), what a load must have to meet each, and
//! which level a species needs (kept on the target, per country). One file
//! per country; the hunter picks theirs in the app.
//!
//! Levels are numbered from 1, the strictest; a load that meets a stricter
//! level may be used wherever a looser one is asked for.

use serde::{Deserialize, Serialize};

mod finland;
pub mod hunting;
mod sweden;

use std::sync::OnceLock;

pub use hunting::HuntingLaw;

/// A country whose rules the app knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Country {
    Sweden,
    Finland,
}

impl Country {
    pub const ALL: [Country; 2] = [Country::Sweden, Country::Finland];

    pub fn name(self) -> &'static str {
        match self {
            Country::Sweden => "Sweden",
            Country::Finland => "Finland",
        }
    }

    /// The two-letter country code: "SE".
    pub fn code(self) -> &'static str {
        match self {
            Country::Sweden => "SE",
            Country::Finland => "FI",
        }
    }

    /// The adjective for texts: "Swedish rules".
    pub fn adjective(self) -> &'static str {
        match self {
            Country::Sweden => "Swedish",
            Country::Finland => "Finnish",
        }
    }

    pub fn rules(self) -> &'static Rules {
        match self {
            Country::Sweden => &sweden::RULES,
            Country::Finland => &finland::RULES,
        }
    }

    /// The country's hunting seasons and times of day.
    pub fn hunting(self) -> &'static HuntingLaw {
        static SWEDEN: OnceLock<HuntingLaw> = OnceLock::new();
        static FINLAND: OnceLock<HuntingLaw> = OnceLock::new();
        let (cell, json) = match self {
            Country::Sweden => (&SWEDEN, include_str!("../../catalog/hunting/sweden.json")),
            Country::Finland => (&FINLAND, include_str!("../../catalog/hunting/finland.json")),
        };
        cell.get_or_init(|| serde_json::from_str(json).expect("the hunting data parses"))
    }

    /// The country a language tag points to: its region ("sv-FI" is
    /// Finland), else its language ("sv" Sweden, "fi" Finland).
    pub fn from_locale(tag: &str) -> Option<Country> {
        let tag = tag.replace('_', "-").to_lowercase();
        let mut parts = tag.split(['-', '.']);
        let language = parts.next().unwrap_or("");
        let region = parts.find(|part| part.len() == 2);
        match (language, region) {
            (_, Some("se")) => Some(Country::Sweden),
            (_, Some("fi")) => Some(Country::Finland),
            (_, Some(_)) => None,
            ("sv", None) => Some(Country::Sweden),
            ("fi", None) => Some(Country::Finland),
            _ => None,
        }
    }
}

/// What the rules look at in a load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub mass_g: f64,
    /// Energy at the muzzle (J).
    pub e0_j: f64,
    /// Energy at 100 m (J).
    pub e100_j: f64,
    /// The bullet is made to expand.
    pub expanding: bool,
    pub lead_free: bool,
}

/// One level of a country's rules.
pub struct Level {
    /// 1 is the strictest.
    pub number: u8,
    /// "Class 1".
    pub name: &'static str,
    /// What a load needs, in a sentence.
    pub needs: &'static str,
}

pub struct Rules {
    pub country: Country,
    /// Strictest first.
    pub levels: &'static [Level],
    /// The law, for the hunter to check: "Naturvårdsverket, NFS 2023:8".
    pub law: &'static str,
    pub source_url: &'static str,
    /// The strictest level a load meets, if any.
    meets: fn(&Load) -> Option<u8>,
}

impl Rules {
    /// The strictest level `load` meets, if any.
    pub fn level_met(&self, load: &Load) -> Option<u8> {
        (self.meets)(load)
    }

    pub fn level(&self, number: u8) -> Option<&'static Level> {
        self.levels.iter().find(|level| level.number == number)
    }

    /// "Class 2", or the bare number for an unknown level.
    pub fn level_name(&self, number: u8) -> String {
        self.level(number)
            .map_or_else(|| format!("level {number}"), |level| level.name.to_owned())
    }

    /// "Class 2 or stricter" (just "Class 1" for the strictest).
    pub fn requirement_text(&self, number: u8) -> String {
        if number == 1 {
            self.level_name(number)
        } else {
            format!("{} or stricter", self.level_name(number))
        }
    }

    /// What the levels mean, for a help text.
    pub fn help(&self) -> String {
        let mut text = format!(
            "The least rifle ammunition {} law allows for this species ({}). A load of a \
             stricter level is allowed too.",
            self.country.adjective(),
            self.law
        );
        for level in self.levels {
            text.push_str(&format!("\n{}: {}", level.name, level.needs));
        }
        text.push_str("\nCheck the current rules.");
        text
    }

    pub fn valid_level(&self, number: u8) -> bool {
        self.level(number).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_from_language_tags() {
        assert_eq!(Country::from_locale("sv-SE"), Some(Country::Sweden));
        assert_eq!(Country::from_locale("sv_SE.UTF-8"), Some(Country::Sweden));
        assert_eq!(Country::from_locale("sv"), Some(Country::Sweden));
        assert_eq!(Country::from_locale("sv-FI"), Some(Country::Finland));
        assert_eq!(Country::from_locale("fi"), Some(Country::Finland));
        assert_eq!(Country::from_locale("en-GB"), None);
        assert_eq!(Country::from_locale("en"), None);
        assert_eq!(Country::from_locale(""), None);
    }

    #[test]
    fn every_country_numbers_its_levels_from_one() {
        for country in Country::ALL {
            let rules = country.rules();
            assert_eq!(rules.country, country);
            for (index, level) in rules.levels.iter().enumerate() {
                assert_eq!(level.number as usize, index + 1, "{country:?}");
            }
            assert!(rules.source_url.starts_with("https://"));
        }
    }
}

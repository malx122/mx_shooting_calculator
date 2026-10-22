//! The app in English, Swedish and Finnish. A text is looked up by its
//! English: `tr("Range")`, or `trf!("Saved {}", name)` for a text with
//! values in it. A text with no translation shows in English, so new code
//! works before anyone translates it. The translations are data:
//! `crates/app/i18n/sv.json` and `fi.json` map the English to the
//! translation. README's Translating section tells how to keep them up to
//! date.
//!
//! The language is kept per thread, like the egui context it is used with:
//! the app sets it once a frame from the hunter's choice.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::LazyLock;

use ballistics_shot::{Allowed, Assessment};
use ballistics_store::{Country, Rules};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "sv")]
    Swedish,
    #[serde(rename = "fi")]
    Finnish,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::English, Language::Swedish, Language::Finnish];

    /// The language's name in itself: "Svenska".
    pub fn own_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Swedish => "Svenska",
            Language::Finnish => "Suomi",
        }
    }

    /// The language a language tag asks for ("sv-FI" is Swedish), English
    /// when the app doesn't have it.
    pub fn from_locale(tag: &str) -> Language {
        let tag = tag.replace('_', "-").to_lowercase();
        match tag.split(['-', '.']).next().unwrap_or("") {
            "sv" => Language::Swedish,
            "fi" => Language::Finnish,
            _ => Language::English,
        }
    }

    /// The device's language: the browser's on the web, the system's on
    /// desktop; English when it isn't one the app has.
    pub fn system() -> Language {
        sys_locale::get_locale()
            .as_deref()
            .map_or(Language::English, Language::from_locale)
    }

    /// The language a setting picks: its own, or the device's for `None`.
    pub fn chosen(setting: Option<Language>) -> Language {
        setting.unwrap_or_else(Language::system)
    }

    fn translations(self) -> Option<&'static HashMap<String, String>> {
        static SWEDISH: LazyLock<HashMap<String, String>> =
            LazyLock::new(|| parse(include_str!("../i18n/sv.json")));
        static FINNISH: LazyLock<HashMap<String, String>> =
            LazyLock::new(|| parse(include_str!("../i18n/fi.json")));
        match self {
            Language::English => None,
            Language::Swedish => Some(&SWEDISH),
            Language::Finnish => Some(&FINNISH),
        }
    }
}

/// A language file's translations; an empty one is still to be done.
fn parse(json: &str) -> HashMap<String, String> {
    let all: HashMap<String, String> =
        serde_json::from_str(json).expect("the language file parses");
    all.into_iter()
        .filter(|(_, translation)| !translation.is_empty())
        .collect()
}

thread_local! {
    static CURRENT: Cell<Language> = const { Cell::new(Language::English) };
}

/// Shows the app's texts in `language` from now on.
pub fn set(language: Language) {
    CURRENT.with(|current| current.set(language));
}

pub fn current() -> Language {
    CURRENT.with(Cell::get)
}

/// A text in the current language. Takes the English as written in the
/// code, so the translation files and their checks can find it.
pub fn tr(english: &'static str) -> &'static str {
    tr_data(english)
}

/// A text from the data rather than the code (a catalog target's name, a
/// target group), in the current language when it has a translation.
pub fn tr_data(english: &str) -> &str {
    current()
        .translations()
        .and_then(|translations| translations.get(english))
        .map_or(english, String::as_str)
}

/// A text with values in it: `{}` takes the next of `positional`, `{0}` a
/// given one, `{name}` one of `named`. A value may be formatted as in
/// `format!`, with a sign and decimals only: `{:+.1}`. `{{` and `}}` are
/// braces. Use it through `trf!`.
pub fn fill(template: &str, positional: &[&dyn Display], named: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let brace = rest.as_bytes()[at];
        rest = &rest[at + 1..];
        if rest.as_bytes().first() == Some(&brace) {
            out.push(brace as char);
            rest = &rest[1..];
            continue;
        }
        let Some(end) = rest.find('}').filter(|_| brace == b'{') else {
            out.push(brace as char);
            continue;
        };
        let placeholder = &rest[..end];
        rest = &rest[end + 1..];
        let (name, spec) = placeholder.split_once(':').unwrap_or((placeholder, ""));
        let value = if name.is_empty() {
            next += 1;
            positional.get(next - 1).copied()
        } else if let Ok(index) = name.parse::<usize>() {
            positional.get(index).copied()
        } else {
            named
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let plus = spec.starts_with('+');
        let decimals = spec
            .trim_start_matches('+')
            .strip_prefix('.')
            .and_then(|digits| digits.parse::<usize>().ok());
        let _ = match (value, plus, decimals) {
            // A placeholder the code doesn't fill (a slip in a
            // translation) shows as it is rather than failing.
            (None, ..) => write!(out, "{{{placeholder}}}"),
            (Some(value), false, None) => write!(out, "{value}"),
            (Some(value), true, None) => write!(out, "{value:+}"),
            (Some(value), false, Some(decimals)) => write!(out, "{value:.decimals$}"),
            (Some(value), true, Some(decimals)) => write!(out, "{value:+.decimals$}"),
        };
    }
    out.push_str(rest);
    out
}

/// `format!` for texts the hunter reads: `trf!("Saved {}", name)` or
/// `trf!("{count} rifles", count = rifles.len())`, in the current language.
/// The English must be a literal; see `fill` for the placeholders.
macro_rules! trf {
    ($english:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($english),
            &[],
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    };
    ($english:literal $(, $value:expr)* $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($english),
            &[$(&$value as &dyn ::std::fmt::Display),*],
            &[],
        )
    };
}
pub(crate) use trf;

/// A target's name in the current language: a catalog target's Swedish or
/// Finnish name, the hunter's own targets as they named them.
pub fn target_name(target: &ballistics_store::Target) -> &str {
    if ballistics_store::Catalog::builtin()
        .target(&target.id)
        .is_some()
    {
        tr_data(&target.name)
    } else {
        &target.name
    }
}

/// A country's name in the current language: "Sverige".
pub fn country_name(country: Country) -> &'static str {
    tr_data(country.name())
}

/// A country for texts that start with it, in the current language:
/// "Swedish" rules, "Sveriges" regler, "Ruotsin" säännöt.
pub fn country_adjective(country: Country) -> &'static str {
    tr_data(country.adjective())
}

/// A rules level's name in the current language: "Klass 2".
pub fn level_name(rules: &Rules, number: u8) -> String {
    match rules.level(number) {
        Some(level) => tr_data(level.name).to_owned(),
        None => trf!("level {}", number),
    }
}

/// "Class 2 or stricter" (just "Class 1" for the strictest), in the
/// current language.
pub fn requirement_text(rules: &Rules, number: u8) -> String {
    if number == 1 {
        level_name(rules, number)
    } else {
        trf!("{} or stricter", level_name(rules, number))
    }
}

/// What a country's levels mean, for a help text, in the current language.
pub fn rules_help(rules: &Rules) -> String {
    let mut text = trf!(
        "The least rifle ammunition {} law allows for this species ({}). A load of a \
         stricter level is allowed too.",
        tr_data(rules.country.adjective()),
        rules.law
    );
    for level in rules.levels {
        text.push_str(&format!(
            "\n{}: {}",
            tr_data(level.name),
            tr_data(level.needs)
        ));
    }
    text.push('\n');
    text.push_str(tr("Check the current rules."));
    text
}

/// The legal verdict in a few words, for a pill, in the current language:
/// `Assessment::short_verdict` ("Class 1: allowed").
pub fn short_verdict(assessment: &Assessment, needed: u8) -> (Allowed, String) {
    let (allowed, _) = assessment.short_verdict(needed);
    let rules = assessment.country.rules();
    let level = assessment
        .level
        .map(|level| level_name(rules, level))
        .unwrap_or_default();
    let text = match allowed {
        Allowed::Yes => trf!("{}: allowed", level),
        Allowed::IfExpanding => trf!("{}: if it expands", level),
        Allowed::No => trf!("Not allowed: needs {}", requirement_text(rules, needed)),
    };
    (allowed, text)
}

/// The legal verdict in a sentence, in the current language:
/// `Assessment::verdict`.
pub fn verdict(assessment: &Assessment, needed: u8, species: &str) -> String {
    let (allowed, _) = assessment.verdict(needed, species);
    let rules = assessment.country.rules();
    let rules_name = trf!("{} rules", country_adjective(assessment.country));
    let wanted = requirement_text(rules, needed).to_lowercase();
    let load = assessment
        .level
        .map_or(tr("meets no level").to_owned(), |level| {
            trf!("is {}", level_name(rules, level).to_lowercase())
        });
    match allowed {
        Allowed::Yes => trf!(
            "{rules}: {species} needs {wanted}; this load {load}: allowed.",
            rules = rules_name,
            species = species,
            wanted = wanted,
            load = load
        ),
        Allowed::IfExpanding => trf!(
            "{rules}: {species} needs {wanted}; this load {load} if the bullet is made to \
             expand. Its construction isn't set in the app: set it on the bullet to be sure.",
            rules = rules_name,
            species = species,
            wanted = wanted,
            load = load
        ),
        Allowed::No => trf!(
            "{rules}: {species} needs {wanted}; this load {load}: not allowed.",
            rules = rules_name,
            species = species,
            wanted = wanted,
            load = load
        ),
    }
}

#[cfg(test)]
mod tests;

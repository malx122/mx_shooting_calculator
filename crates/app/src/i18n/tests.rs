//! The language lookup, and checks that the language files match the
//! code: no stale keys, placeholders kept, and a list of what is still to
//! translate (`cargo test -p mxsc i18n -- --nocapture`; with
//! `MXSC_I18N_UPDATE=1` the missing texts are added to the files, empty,
//! and stale ones taken out).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ballistics_store::{Catalog, Construction, Country, TargetGroup};

use super::*;

/// The language's code, as its file is named: "sv".
fn code(language: Language) -> String {
    serde_json::to_value(language)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .expect("a language serialises as its code")
}

fn language_file(language: Language) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("i18n")
        .join(format!("{}.json", code(language)))
}

fn translated() -> [Language; 2] {
    [Language::Swedish, Language::Finnish]
}

fn read(language: Language) -> BTreeMap<String, String> {
    let json = std::fs::read_to_string(language_file(language)).expect("the language file reads");
    serde_json::from_str(&json).expect("the language file parses")
}

/// Every `tr("…")` and `trf!("…", …)` in the app's code (these tests left
/// out): the texts to translate.
fn wrapped_strings() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut folders = vec![source];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("the source folder reads") {
            let path = entry.expect("a source entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "i18n") {
                    folders.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let code = std::fs::read_to_string(&path).expect("a source file reads");
                found.extend(wrapped_in(&code));
            }
        }
    }
    found
}

/// The texts the code wraps, comments left out.
fn wrapped_in(code: &str) -> Vec<String> {
    let code: String = code
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| [line, "\n"])
        .collect();
    let mut found = Vec::new();
    for call in ["tr(", "trf!("] {
        let mut from = 0;
        while let Some(at) = code[from..].find(call) {
            let start = from + at;
            from = start + call.len();
            let before = code[..start].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if let Some(text) = string_literal(code[from..].trim_start()) {
                found.push(text);
            }
        }
    }
    found
}

/// The value of the Rust string literal `code` starts with, if it does.
fn string_literal(code: &str) -> Option<String> {
    if let Some(raw) = code.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let body = raw[hashes..].strip_prefix('"')?;
        let end = format!("\"{}", "#".repeat(hashes));
        return Some(body[..body.find(&end)?].to_owned());
    }
    let mut chars = code.strip_prefix('"')?.chars().peekable();
    let mut text = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(text),
            '\\' => match chars.next()? {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                'r' => text.push('\r'),
                '0' => text.push('\0'),
                '\n' => {
                    while chars.peek().is_some_and(|c| c.is_whitespace()) {
                        chars.next();
                    }
                }
                'u' => {
                    let hex: String = chars.by_ref().skip(1).take_while(|&c| c != '}').collect();
                    text.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => text.push(other),
            },
            c => text.push(c),
        }
    }
    None
}

/// Texts the app shows from its data through `tr_data`: the catalog's
/// target names, groups and hit zones, bullet constructions, the countries
/// and their rules' levels.
fn data_strings() -> BTreeSet<String> {
    let catalog = Catalog::builtin();
    let mut found: BTreeSet<String> = catalog
        .targets
        .iter()
        .map(|target| target.name.clone())
        .collect();
    found.extend(
        catalog
            .drawings
            .iter()
            .flat_map(|drawing| &drawing.zones)
            .map(|zone| zone.name.clone()),
    );
    found.extend(TargetGroup::ALL.map(|group| group.label().to_owned()));
    found.extend(Construction::ALL.map(|construction| construction.label().to_owned()));
    for country in Country::ALL {
        found.insert(country.name().to_owned());
        found.insert(country.adjective().to_owned());
        for level in country.rules().levels {
            found.insert(level.name.to_owned());
            found.insert(level.needs.to_owned());
        }
    }
    found
}

fn all_strings() -> BTreeSet<String> {
    let mut all = wrapped_strings();
    all.extend(data_strings());
    all
}

#[test]
fn a_text_shows_in_the_chosen_language_or_in_english() {
    set(Language::Swedish);
    assert_eq!(tr("Range"), "Avstånd");
    assert_eq!(trf!("Saved {}", "Tikka"), "Sparade Tikka");
    set(Language::Finnish);
    assert_eq!(tr("Range"), "Etäisyys");
    // No translation: the English.
    assert_eq!(tr("No such text in the app"), "No such text in the app");
    set(Language::English);
    assert_eq!(tr("Range"), "Range");
}

#[test]
fn catalog_targets_have_their_own_language_names() {
    let moose = Catalog::builtin().target("moose").expect("the moose");
    set(Language::Swedish);
    assert_eq!(target_name(moose), "Älg");
    set(Language::Finnish);
    assert_eq!(target_name(moose), "Hirvi");
    let mut own = moose.to_custom();
    own.name = "Moose".to_owned();
    assert_eq!(
        target_name(&own),
        "Moose",
        "the hunter's own keep their name"
    );
    set(Language::English);
    assert_eq!(target_name(moose), "Moose");
}

#[test]
fn languages_follow_the_locale_tag() {
    assert_eq!(Language::from_locale("sv-SE"), Language::Swedish);
    assert_eq!(Language::from_locale("sv_FI.UTF-8"), Language::Swedish);
    assert_eq!(Language::from_locale("fi"), Language::Finnish);
    assert_eq!(Language::from_locale("FI-fi"), Language::Finnish);
    assert_eq!(Language::from_locale("en-GB"), Language::English);
    assert_eq!(Language::from_locale("de-DE"), Language::English);
    assert_eq!(Language::from_locale(""), Language::English);
    assert_eq!(Language::chosen(Some(Language::Finnish)), Language::Finnish);
}

#[test]
fn fill_puts_values_in_their_places() {
    let positional: [&dyn Display; 2] = [&1.25_f64, &"m"];
    assert_eq!(fill("{} {}", &positional, &[]), "1.25 m");
    assert_eq!(fill("{1} {0:.1}", &positional, &[]), "m 1.2");
    assert_eq!(fill("{:+.0}{{}}", &positional, &[]), "+1{}");
    let named: [(&str, &dyn Display); 1] = [("count", &3)];
    assert_eq!(fill("{count} rifles", &[], &named), "3 rifles");
    assert_eq!(fill("{missing} and }", &[], &named), "{missing} and }");
    assert_eq!(trf!("{count} of {all}", count = 2, all = 5), "2 of 5");
    assert_eq!(trf!("{:.1} m", 2.04), "2.0 m");
}

#[test]
fn the_code_scan_reads_string_literals() {
    let code = "ui.label(tr(\"One\"));\n// tr(\"A comment\")\nlet x = attr(\"no\");\n\
                trf!(\n    \"Two {} \\\n     lines \\\"q\\\"\", 1);\ntr(r#\"Raw \"x\"\"#);";
    // Found per call: the tr( ones first, then the trf!( ones.
    assert_eq!(wrapped_in(code), ["One", "Raw \"x\"", "Two {} lines \"q\""]);
}

#[test]
fn every_translated_text_is_still_in_the_app() {
    let all = all_strings();
    for language in translated() {
        let stale: Vec<String> = read(language)
            .into_keys()
            .filter(|key| !all.contains(key))
            .collect();
        assert!(
            stale.is_empty(),
            "{}.json has texts the app no longer shows (MXSC_I18N_UPDATE=1 takes them out): \
             {stale:#?}",
            code(language)
        );
    }
}

/// The `{…}` placeholders in a text: the unnumbered ones in order (a
/// translation can't move them), the others sorted.
fn placeholders(text: &str) -> (Vec<String>, Vec<String>) {
    let text = text.replace("{{", "").replace("}}", "");
    let (in_order, mut named): (Vec<String>, Vec<String>) = text
        .split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}').map(|(inside, _)| inside.to_owned()))
        .partition(|inside| inside.is_empty() || inside.starts_with(':'));
    named.sort();
    (in_order, named)
}

#[test]
fn translations_keep_the_placeholders() {
    for language in translated() {
        for (english, translation) in read(language) {
            if !translation.is_empty() {
                assert_eq!(
                    placeholders(&translation),
                    placeholders(&english),
                    "{}.json: \"{translation}\" for \"{english}\"",
                    code(language)
                );
            }
        }
    }
}

/// Lists the texts still to translate. With `MXSC_I18N_UPDATE=1` it adds
/// them to the language files with an empty translation (shown in English
/// until filled in) and takes out the stale ones.
#[test]
fn texts_still_to_translate() {
    let all = all_strings();
    let update = std::env::var_os("MXSC_I18N_UPDATE").is_some();
    for language in translated() {
        let mut file = read(language);
        let missing: Vec<&String> = all
            .iter()
            .filter(|text| file.get(*text).is_none_or(String::is_empty))
            .collect();
        println!(
            "{}: {} of {} texts not translated",
            code(language),
            missing.len(),
            all.len()
        );
        for text in &missing {
            println!("  {text:?}");
        }
        if update {
            file.retain(|key, _| all.contains(key));
            for text in &all {
                file.entry(text.clone()).or_default();
            }
            let json = serde_json::to_string_pretty(&file).expect("the translations serialise");
            std::fs::write(language_file(language), json + "\n").expect("the language file writes");
        }
    }
}

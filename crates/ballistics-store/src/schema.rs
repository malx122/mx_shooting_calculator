//! The export file's JSON Schema and the field reference in
//! docs/export-format.md, both generated from the types the file is made
//! of, so the documentation follows every change. A test fails when the
//! committed files differ from what the types say; to bring them up to
//! date, run `MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format`.

use serde_json::Value;

use crate::transfer::PersonalData;

/// Where the schema is published.
pub const SCHEMA_ID: &str =
    "https://malx122.github.io/mx_shooting_calculator/download/export-format.schema.json";

/// The JSON Schema (2020-12) of an export file.
pub fn export_schema() -> Value {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    let schema = generator.into_root_schema_for::<PersonalData>();
    let mut value = serde_json::to_value(schema).expect("a schema is JSON");
    if let Value::Object(fields) = &mut value {
        fields.insert("$id".into(), SCHEMA_ID.into());
        fields.insert("title".into(), "MX Shooting Calculator export".into());
    }
    value
}

/// The field reference for docs/export-format.md: a section for the file
/// and for each kind of record, with its fields, their types and meaning.
pub fn field_reference(schema: &Value) -> String {
    let mut out = String::new();
    out.push_str("### The file\n\n");
    out.push_str(&describe(schema));
    fields_table(&mut out, schema);
    if let Some(Value::Object(definitions)) = schema.get("$defs") {
        for (name, definition) in definitions {
            out.push_str(&format!("### {name}\n\n"));
            out.push_str(&describe(definition));
            if definition.get("properties").is_some() {
                fields_table(&mut out, definition);
            }
            if let Some(Value::Array(variants)) = definition.get("oneOf") {
                variants_list(&mut out, variants, definition.get("properties").is_some());
            } else if definition.get("enum").is_some() {
                variants_list(&mut out, std::slice::from_ref(definition), false);
            }
        }
    }
    out
}

/// A description as one paragraph, or nothing.
fn describe(value: &Value) -> String {
    match text(value) {
        Some(text) => format!("{text}\n\n"),
        None => String::new(),
    }
}

/// The description on one line, in the file's words rather than the code's.
fn text(value: &Value) -> Option<String> {
    let description = value.get("description")?.as_str()?;
    let joined = description
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    Some(joined.replace("`None`", "`null`").replace('|', "\\|"))
}

fn fields_table(out: &mut String, object: &Value) {
    let Some(Value::Object(properties)) = object.get("properties") else {
        return;
    };
    let required: Vec<&str> = object
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    out.push_str("| Field | Type | Meaning |\n|---|---|---|\n");
    for (name, property) in properties {
        let mut meaning = text(property).unwrap_or_default();
        if !required.contains(&name.as_str()) {
            if !meaning.is_empty() {
                meaning.push(' ');
            }
            meaning.push_str("May be left out.");
        }
        out.push_str(&format!(
            "| `{name}` | {} | {meaning} |\n",
            type_of(property)
        ));
    }
    out.push('\n');
}

/// The values of an enumeration, or the forms a tagged record can take.
fn variants_list(out: &mut String, variants: &[Value], has_fields: bool) {
    out.push_str(if has_fields {
        "And one of these, by its tag:\n\n"
    } else {
        "One of:\n\n"
    });
    for variant in variants {
        let meaning = text(variant)
            .filter(|_| variant.get("const").is_some() || variant.get("properties").is_some())
            .map(|text| format!(": {text}"))
            .unwrap_or_default();
        if let Some(values) = variant.get("enum").and_then(Value::as_array) {
            for value in values {
                out.push_str(&format!("- `{value}`{meaning}\n"));
            }
        } else if let Some(value) = variant.get("const") {
            out.push_str(&format!("- `{value}`{meaning}\n"));
        } else if let Some(Value::Object(properties)) = variant.get("properties") {
            // A tagged form: the tag's value, then its own fields.
            let mut tag = String::new();
            let mut fields = Vec::new();
            for (name, property) in properties {
                match property.get("const") {
                    Some(value) => tag = format!("`{name}`: `{value}`"),
                    None => fields.push(format!("`{name}` ({})", type_of(property))),
                }
            }
            if let Some(reference) = variant.get("$ref").and_then(Value::as_str) {
                fields.push(format!("the fields of {}", link(reference)));
            }
            let fields = if fields.is_empty() {
                String::new()
            } else {
                format!(", with {}", fields.join(", "))
            };
            out.push_str(&format!("- {tag}{fields}{meaning}\n"));
        }
    }
    out.push('\n');
}

/// A field's type in words, with links to the record types.
fn type_of(property: &Value) -> String {
    if let Some(reference) = property.get("$ref").and_then(Value::as_str) {
        return link(reference);
    }
    if let Some(Value::Array(choices)) = property.get("anyOf") {
        let named: Vec<String> = choices
            .iter()
            .filter(|choice| choice.get("type").and_then(Value::as_str) != Some("null"))
            .map(type_of)
            .collect();
        let nullable = named.len() < choices.len();
        let mut words = named.join(" or ");
        if nullable {
            words.push_str(" or null");
        }
        return words;
    }
    let names: Vec<&str> = match property.get("type") {
        Some(Value::String(name)) => vec![name.as_str()],
        Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let words: Vec<String> = names
        .iter()
        .map(|name| match *name {
            "array" => match (property.get("prefixItems"), property.get("items")) {
                (Some(Value::Array(items)), _) => format!(
                    "array of {} ({})",
                    items.len(),
                    items.iter().map(type_of).collect::<Vec<_>>().join(", ")
                ),
                (_, Some(items)) => match (property.get("minItems"), property.get("maxItems")) {
                    (Some(min), Some(max)) if min == max => {
                        format!("array of {min} × {}", type_of(items))
                    }
                    _ => format!("array of {}", type_of(items)),
                },
                _ => "array".to_owned(),
            },
            "object" => match property.get("properties") {
                Some(Value::Object(keys)) => format!(
                    "object with any of {}",
                    keys.keys()
                        .map(|key| format!("`{key}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                _ => "object".to_owned(),
            },
            other => other.to_owned(),
        })
        .collect();
    words.join(" or ")
}

/// A link to a record type's section.
fn link(reference: &str) -> String {
    let name = reference.rsplit('/').next().unwrap_or(reference);
    format!("[{name}](#{})", name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marks around the generated part of docs/export-format.md.
    const START: &str = "<!-- field reference: generated from the code, see schema.rs -->\n";
    const END: &str = "<!-- end of the field reference -->\n";

    /// What the app writes matches the schema: an export of the demo data,
    /// with every kind of record and the catalog section.
    #[test]
    fn an_export_matches_the_schema() {
        let mut store = crate::Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        let export: Value = serde_json::from_str(&store.export_json().unwrap()).unwrap();
        for kind in ["rifles", "bullets", "rifle_loads", "scopes", "targets"] {
            assert!(
                export[kind]
                    .as_array()
                    .is_some_and(|records| !records.is_empty()),
                "the demo data has {kind}"
            );
        }
        assert!(
            export["catalog"]["calibers"]
                .as_array()
                .is_some_and(|c| !c.is_empty())
        );
        assert!(
            export["catalog"]["scopes"]
                .as_array()
                .is_some_and(|c| !c.is_empty())
        );
        let validator = jsonschema::validator_for(&export_schema()).expect("the schema is valid");
        let problems: Vec<String> = validator
            .iter_errors(&export)
            .map(|error| format!("{} at {}", error, error.instance_path()))
            .collect();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// The committed schema and field reference match the types. With
    /// MXSC_UPDATE_DOCS=1 the test writes them instead.
    #[test]
    fn export_format_docs_are_current() {
        let docs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/");
        let schema_path = format!("{docs}export-format.schema.json");
        let page_path = format!("{docs}export-format.md");
        let schema = export_schema();
        let schema_text = serde_json::to_string_pretty(&schema).unwrap() + "\n";
        let page = std::fs::read_to_string(&page_path).expect("docs/export-format.md exists");
        let (before, rest) = page.split_once(START).expect("the page has the start mark");
        let (_, after) = rest.split_once(END).expect("the page has the end mark");
        let page_text = format!("{before}{START}\n{}{END}{after}", field_reference(&schema));
        if std::env::var_os("MXSC_UPDATE_DOCS").is_some() {
            std::fs::write(&schema_path, &schema_text).unwrap();
            std::fs::write(&page_path, &page_text).unwrap();
            return;
        }
        let stale = "the export format changed: run \
                     MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format \
                     and check the docs";
        assert!(
            std::fs::read_to_string(&schema_path).ok().as_deref() == Some(schema_text.as_str()),
            "docs/export-format.schema.json is out of date; {stale}"
        );
        assert!(
            page == page_text,
            "the field reference in docs/export-format.md is out of date; {stale}"
        );
    }
}

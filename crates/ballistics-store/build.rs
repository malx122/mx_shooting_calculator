//! Lists the catalog's per-maker files (`catalog/bullets/*.json`,
//! `catalog/scopes/*.json`) so they are compiled in without naming each one:
//! adding a maker's file is enough.

use std::{env, fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog");
    let mut code = String::new();
    for (constant, folder) in [
        ("BULLETS", "bullets"),
        ("SCOPES", "scopes"),
        ("CHAMBERSIM_BULLETS", "third-party/chambersim/bullets"),
    ] {
        let dir = root.join(folder);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.expect("a readable catalog folder").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        code += &format!(
            "/// The `catalog/{folder}` files in name order: (file name, contents).\n\
             pub const {}: &[(&str, &str)] = &[\n",
            constant
        );
        for path in files {
            println!("cargo:rerun-if-changed={}", path.display());
            let name = path.file_name().unwrap().to_string_lossy();
            code += &format!(
                "    ({name:?}, include_str!({:?})),\n",
                path.display().to_string()
            );
        }
        code += "];\n";
    }
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("catalog_files.rs");
    fs::write(out, code).unwrap();
}

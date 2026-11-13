//! Lists the most popular calibers for a use, from the catalog's rough
//! popularity grades (see docs/caliber-popularity.md).
//!
//!     cargo run -q -p ballistics-store --example popular -- hunting_europe 20
//!
//! Uses: hunting_nordic, hunting_europe, hunting_north_america, precision,
//! f_class, benchrest, long_range, rimfire, nordic_field. Calibers with the
//! same grade come in order of how many factory loads the catalog has.

use std::collections::HashMap;

use ballistics_store::{Catalog, CatalogKind, Popularity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let usage = format!("usage: popular <{}> [count]", Popularity::USES.join("|"));
    let of = arguments.next().ok_or(usage.clone())?;
    if !Popularity::USES.contains(&of.as_str()) {
        return Err(usage.into());
    }
    let count: usize = arguments.next().map_or(Ok(20), |n| n.parse())?;

    let catalog = Catalog::builtin();
    let mut loads: HashMap<&str, usize> = HashMap::new();
    for bullet in &catalog.bullets {
        if bullet.kind == CatalogKind::Ammunition
            && let Some(caliber) = &bullet.caliber
        {
            *loads.entry(caliber.as_str()).or_default() += 1;
        }
    }
    let mut graded: Vec<_> = catalog
        .calibers
        .iter()
        .filter_map(|caliber| {
            let grade = caliber.popularity?.grade(&of)?;
            Some((
                grade,
                loads.get(caliber.key.as_str()).copied().unwrap_or(0),
                caliber,
            ))
        })
        .collect();
    graded.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    for (rank, (grade, loads, caliber)) in graded.iter().take(count).enumerate() {
        println!(
            "{:>3}. {:<28} grade {grade}  ({loads} factory loads)",
            rank + 1,
            caliber.name
        );
    }
    Ok(())
}

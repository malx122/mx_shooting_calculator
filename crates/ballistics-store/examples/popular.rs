//! Lists the most popular calibers for a use, from the catalog's rough
//! popularity grades (see docs/caliber-popularity.md).
//!
//!     cargo run -q -p ballistics-store --example popular -- hunting_europe 20
//!
//! Uses: hunting_nordic, hunting_europe, hunting_north_america, precision,
//! f_class, benchrest, long_range, rimfire, nordic_field. Calibers with the
//! same grade come in order of how many factory loads the catalog has.

use ballistics_store::{Catalog, Popularity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let usage = format!("usage: popular <{}> [count]", Popularity::USES.join("|"));
    let of = arguments.next().ok_or(usage.clone())?;
    let count: usize = arguments.next().map_or(Ok(20), |n| n.parse())?;
    let catalog = Catalog::builtin();
    let ranked = catalog.popular(&of).ok_or(usage)?;
    for (rank, entry) in ranked.iter().take(count).enumerate() {
        println!(
            "{:>3}. {:<28} grade {}  ({} factory loads)",
            rank + 1,
            entry.caliber.name,
            entry.grade,
            entry.factory_loads
        );
    }
    Ok(())
}

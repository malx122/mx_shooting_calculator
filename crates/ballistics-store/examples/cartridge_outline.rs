//! Prints a caliber's cartridge as JSON: its outline (as the 3D view and the
//! STL use it) and its drawing's measurements, for
//! `tools/cartridge-drawing.py`.
//!
//!     cargo run -q -p ballistics-store --example cartridge_outline -- 308-win > outline.json

use ballistics_store::Catalog;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::args()
        .nth(1)
        .ok_or("usage: cartridge_outline <caliber key>")?;
    let caliber = Catalog::builtin()
        .caliber(&key)
        .ok_or_else(|| format!("no caliber {key}"))?;
    let cartridge = caliber
        .cartridge
        .as_ref()
        .ok_or_else(|| format!("{} has no cartridge", caliber.name))?;
    let outline = cartridge.outline();
    let case = &cartridge.case;
    let shoulder = case.shoulder.as_ref();
    let groove = case.groove.as_ref();
    let bevel = case.head_bevel.as_ref();
    let out = json!({
        "name": caliber.name,
        "start": outline.start,
        "stretches": outline.stretches.iter().map(|(material, points)| json!({
            "material": format!("{material:?}"),
            "points": points,
        })).collect::<Vec<_>>(),
        "letters": {
            "R": case.rim_thickness,
            "R1": case.rim_diameter,
            "f": bevel.map(|b| b.length),
            "beta": bevel.map(|b| b.degrees),
            "E1": groove.map(|g| g.diameter),
            "e": groove.map(|g| g.floor_min),
            "delta": groove.map(|g| g.bevel_degrees),
            "E": case.body_start,
            "P1": case.body_diameter,
            "L1": shoulder.map(|s| s.start),
            "P2": shoulder.map(|s| s.diameter),
            "L2": shoulder.map(|s| s.neck_start),
            "H1": shoulder.map(|s| s.neck_diameter),
            "H2": case.mouth_diameter,
            "L3": case.length,
            "L6": cartridge.overall_length,
            "G1": cartridge.bullet.diameter,
        },
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

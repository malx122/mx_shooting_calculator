//! Shot sizes: the numbers on the box and the diameters they stand for.
//!
//! A number means different sizes in different countries: US 5 is 3.05 mm,
//! English 5 is 2.79 mm, Italian 5 about 2.9 mm. Swedish boxes use US
//! numbers with the millimetres continental makers print for them, such as
//! "US 5 / 3,0 mm"; Finnish shops print the maker's own number and the
//! millimetres. The millimetres are what to go by.

/// American sizes (ANSI/SAAMI Z299.2), lead and steel alike: from 7 to FF
/// each is 0.010 inch larger, with 8½ and 7½ halfway steps below. Then
/// buckshot.
pub const US_SIZES: [(&str, f64); 25] = [
    ("9", 0.002_03),
    ("8½", 0.002_16),
    ("8", 0.002_29),
    ("7½", 0.002_41),
    ("7", 0.002_54),
    ("6", 0.002_79),
    ("5", 0.003_05),
    ("4", 0.003_30),
    ("3", 0.003_56),
    ("2", 0.003_81),
    ("1", 0.004_06),
    ("B", 0.004_32),
    ("BB", 0.004_57),
    ("BBB", 0.004_83),
    ("T", 0.005_08),
    ("TT", 0.005_33),
    ("F", 0.005_59),
    ("FF", 0.005_84),
    ("4 Buck", 0.006_10),
    ("3 Buck", 0.006_35),
    ("2 Buck", 0.006_86),
    ("1 Buck", 0.007_62),
    ("0 Buck", 0.008_13),
    ("00 Buck", 0.008_38),
    ("000 Buck", 0.009_14),
];

/// British sizes (Purdey, 1936).
pub const ENGLISH_SIZES: [(&str, f64); 22] = [
    ("LG", 0.009_14),
    ("SG", 0.008_43),
    ("Special SG", 0.007_57),
    ("SSG", 0.006_83),
    ("AAA", 0.005_16),
    ("AA", 0.004_92),
    ("A", 0.004_57),
    ("BBB", 0.004_32),
    ("BB", 0.004_09),
    ("B", 0.003_91),
    ("1", 0.003_63),
    ("2", 0.003_43),
    ("3", 0.003_25),
    ("4", 0.003_05),
    ("5", 0.002_79),
    ("5½", 0.002_72),
    ("6", 0.002_59),
    ("6½", 0.002_51),
    ("7", 0.002_41),
    ("7½", 0.002_28),
    ("8", 0.002_21),
    ("9", 0.002_03),
];

/// The diameter of a US-numbered size as Swedish boxes and Svenska
/// Jägareförbundet give it: 1 is 4.1 mm (the largest Swedish law allows),
/// then 2 is 3.75 mm and each number 0.25 mm smaller, so 3 is 3.5, 5 is 3.0
/// and 7 is 2.5 mm; 7½ is 2.4 mm. Numbers 1 to 9 only.
pub fn nordic_diameter(number: &str) -> Option<f64> {
    match number {
        "1" => return Some(0.004_1),
        "7½" => return Some(0.002_4),
        _ => {}
    }
    let number: u32 = number.parse().ok()?;
    (1..=9)
        .contains(&number)
        .then_some(0.004_25 - 0.000_25 * number as f64)
}

/// The diameter of a size in a table of sizes, by its label.
pub fn diameter_of(sizes: &[(&str, f64)], label: &str) -> Option<f64> {
    sizes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(label.trim()))
        .map(|&(_, diameter)| diameter)
}

/// The steel pellet to use for a lead one of `lead_diameter_m`: two US
/// numbers, 0.5 mm, larger (Naturvårdsverket's advice). Bismuth needs about
/// one number larger, tungsten one or two smaller.
pub fn steel_for_lead(lead_diameter_m: f64) -> f64 {
    lead_diameter_m + 0.000_5
}

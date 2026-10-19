//! Sweden: Naturvårdsverkets föreskrifter och allmänna råd om jakt
//! (NFS 2023:8), 2 kap 11–14 §§. Four rifle ammunition classes; a species
//! needs one of them (see each target's `ammunition`).

use super::{Country, Level, Load, Rules};

pub static RULES: Rules = Rules {
    country: Country::Sweden,
    levels: &[
        Level {
            number: 1,
            name: "Class 1",
            needs: "an expanding bullet of at least 9 g with 2 700 J at 100 m, or 10 g with \
                    2 000 J, or a lead-free 7.7 g with 2 000 J",
        },
        Level {
            number: 2,
            name: "Class 2",
            needs: "an expanding bullet of at least 3.2 g with 800 J at 100 m",
        },
        Level {
            number: 3,
            name: "Class 3",
            needs: "a bullet of at least 2.5 g with 200 J at 100 m",
        },
        Level {
            number: 4,
            name: "Class 4",
            needs: "150 J at the muzzle",
        },
    ],
    law: "Naturvårdsverket, NFS 2023:8",
    source_url: "https://www.naturvardsverket.se/lagar-och-regler/foreskrifter-och-allmanna-rad/2023/nfs-20238/",
    meets: class,
};

/// The strictest class a load meets.
fn class(load: &Load) -> Option<u8> {
    let Load {
        mass_g,
        e0_j,
        e100_j,
        expanding,
        lead_free,
    } = *load;
    if expanding
        && ((mass_g >= 9.0 && e100_j >= 2700.0)
            || (mass_g >= 10.0 && e100_j >= 2000.0)
            || (lead_free && mass_g >= 7.7 && e100_j >= 2000.0))
    {
        Some(1)
    } else if expanding && mass_g >= 3.2 && e100_j >= 800.0 {
        Some(2)
    } else if mass_g >= 2.5 && e100_j >= 200.0 {
        Some(3)
    } else if e0_j >= 150.0 {
        Some(4)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(mass_g: f64, e100_j: f64, expanding: bool, lead_free: bool) -> Load {
        Load {
            mass_g,
            e0_j: e100_j * 1.3,
            e100_j,
            expanding,
            lead_free,
        }
    }

    #[test]
    fn the_classes_follow_the_regulation() {
        let level = |l: Load| RULES.level_met(&l);
        // .308 Win 11.7 g soft point: class 1.
        assert_eq!(level(load(11.7, 3200.0, true, false)), Some(1));
        // 9 g needs 2 700 J; 10 g needs 2 000 J.
        assert_eq!(level(load(9.0, 2600.0, true, false)), Some(2));
        assert_eq!(level(load(10.0, 2100.0, true, false)), Some(1));
        // Lead-free 7.7 g with 2 000 J.
        assert_eq!(level(load(7.8, 2100.0, true, true)), Some(1));
        assert_eq!(level(load(7.8, 2100.0, true, false)), Some(2));
        // Full metal jacket: never classes 1–2.
        assert_eq!(level(load(11.7, 3200.0, false, false)), Some(3));
        // .17 HMR: class 4.
        assert_eq!(level(load(1.1, 150.0, true, false)), Some(4));
        assert_eq!(level(load(0.5, 50.0, false, false)), None);
    }
}

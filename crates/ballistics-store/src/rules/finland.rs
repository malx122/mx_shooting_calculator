//! Finland: Metsästysasetus (Jaktförordning) 666/1993 § 16 a, as amended by
//! 64/2020. Five levels, the strictest first; the decree numbers its tiers
//! the other way round (§ 16 a 2 mom. points 4 to 1, then the 100 J rule
//! of 1 mom.). Points 3 and 4 also ask for an expanding bullet (3 mom.).
//! Energies are at 100 m except the 100 J and 300 J muzzle rules.

use super::{Country, Level, Load, Rules};

pub static RULES: Rules = Rules {
    country: Country::Finland,
    levels: &[
        Level {
            number: 1,
            name: "Moose class",
            needs: "for moose, wild boar and bear (§ 16 a point 4): an expanding bullet of at \
                    least 9 g with 2 700 J at 100 m, or 10 g with 2 000 J, or a lead-free 7.5 g \
                    with 1 900 J",
        },
        Level {
            number: 2,
            name: "Deer class",
            needs: "for red, fallow, white-tailed and sika deer and forest reindeer (point 3): \
                    an expanding bullet of at least 6 g with 2 000 J at 100 m, or 8 g with \
                    1 700 J, or a lead-free 5.1 g with 1 700 J",
        },
        Level {
            number: 3,
            name: "Roe deer class",
            needs: "for roe deer, mouflon, beaver, seals, wolf, lynx and wolverine (point 2): a \
                    bullet of at least 3.2 g (lead-free 2.9 g) with 800 J at 100 m",
        },
        Level {
            number: 4,
            name: "Fox class",
            needs: "for fox, badger, otter, hares, capercaillie, black grouse and geese \
                    (point 1): a bullet of at least 2.5 g with 200 J at 100 m, or 300 J at the \
                    muzzle",
        },
        Level {
            number: 5,
            name: "Small game class",
            needs: "for all other game (1 mom.): 100 J at the muzzle",
        },
    ],
    law: "Metsästysasetus 666/1993 § 16 a",
    source_url: "https://www.finlex.fi/fi/lainsaadanto/1993/666",
    meets: class,
};

/// The strictest level a load meets.
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
            || (lead_free && mass_g >= 7.5 && e100_j >= 1900.0))
    {
        Some(1)
    } else if expanding
        && ((mass_g >= 6.0 && e100_j >= 2000.0)
            || (mass_g >= 8.0 && e100_j >= 1700.0)
            || (lead_free && mass_g >= 5.1 && e100_j >= 1700.0))
    {
        Some(2)
    } else if (mass_g >= 3.2 || (lead_free && mass_g >= 2.9)) && e100_j >= 800.0 {
        Some(3)
    } else if (mass_g >= 2.5 && e100_j >= 200.0) || e0_j >= 300.0 {
        Some(4)
    } else if e0_j >= 100.0 {
        Some(5)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(mass_g: f64, e0_j: f64, e100_j: f64, expanding: bool, lead_free: bool) -> Load {
        Load {
            mass_g,
            e0_j,
            e100_j,
            expanding,
            lead_free,
        }
    }

    #[test]
    fn the_levels_follow_the_decree() {
        let level = |l: Load| RULES.level_met(&l);
        // .308 Win 11.7 g soft point: moose.
        assert_eq!(level(load(11.7, 3600.0, 3000.0, true, false)), Some(1));
        // Lead-free 7.5 g with 1 900 J: moose; leaded, only deer.
        assert_eq!(level(load(7.5, 2600.0, 1950.0, true, true)), Some(1));
        assert_eq!(level(load(7.5, 2600.0, 1950.0, true, false)), Some(3));
        // 6.5×55 6 g with 2 100 J: deer, not moose.
        assert_eq!(level(load(6.0, 2600.0, 2100.0, true, false)), Some(2));
        // Full metal jacket: never moose or deer, but roe deer needs no
        // expanding bullet in Finland.
        assert_eq!(level(load(11.7, 3600.0, 3000.0, false, false)), Some(3));
        // .222: roe deer.
        assert_eq!(level(load(3.2, 1300.0, 900.0, true, false)), Some(3));
        // .17 HMR: fox through the 300 J muzzle rule.
        assert_eq!(level(load(1.1, 330.0, 150.0, true, false)), Some(4));
        // .22 LR: small game.
        assert_eq!(level(load(2.6, 140.0, 90.0, false, false)), Some(5));
        assert_eq!(level(load(0.5, 60.0, 30.0, false, false)), None);
    }
}

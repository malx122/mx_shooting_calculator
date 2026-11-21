//! What Swedish and Finnish law says about shotguns: which species may be
//! shot with shot or a slug, from which gauges, and with what shot; and the
//! advice the countries' hunting bodies give, each named with its source.
//! The app words it; this keeps the rules as data, by the catalog's target
//! ids, with the paragraphs they come from (researched 2026-11-21, see
//! docs/shotgun-research/lethality.md).

use crate::shotguns::Gauge;

use super::Country;

/// What the law says about shooting a species with shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotVerdict {
    Allowed,
    /// Allowed, but not from a gauge this small.
    GaugeTooSmall,
    /// The country doesn't allow this gauge for hunting at all.
    GaugeNotAllowed,
    /// Not with shot.
    NotAllowed,
}

/// What the law says about shooting a species with a slug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlugVerdict {
    /// Allowed; in Sweden only for a hunter qualified to use a rifle of this
    /// ammunition class.
    Allowed { rifle_class: Option<u8> },
    /// Only from a gun with a single barrel.
    OnlySingleBarrel,
    /// Not from this gauge.
    GaugeNotAllowed,
    /// Not with a slug.
    NotAllowed,
}

/// A country's rules for shotguns.
pub struct ShotgunLaw {
    pub country: Country,
    /// The law, for the hunter to check: "NFS 2023:8, 3 kap.".
    pub law: &'static str,
    pub source_url: &'static str,
    /// The largest shot allowed, mm.
    pub max_pellet_mm: Option<f64>,
    /// Lead shot is banned when hunting waterfowl (ducks, geese and swans).
    pub no_lead_for_waterfowl: bool,
    /// Gauges allowed for game at all.
    gauges: &'static [Gauge],
    /// The gauges allowed only for the species in `small_gauge_species`.
    small_gauges: &'static [Gauge],
    small_gauge_species: &'static [&'static str],
    /// Species that may not be shot with shot.
    no_shot: &'static [&'static str],
    slugs: Slugs,
}

enum Slugs {
    /// Sweden: only these species, from a single barrel of these gauges,
    /// each for a hunter qualified for a rifle class.
    Only {
        species: &'static [(&'static str, u8)],
        gauges: &'static [Gauge],
    },
    /// Finland: every species but these.
    Except(&'static [&'static str]),
}

use Gauge::*;

/// NFS 2023:8, 3 kap. 4, 12–14 and 16 §§.
static SWEDEN: ShotgunLaw = ShotgunLaw {
    country: Country::Sweden,
    law: "Naturvårdsverket, NFS 2023:8, 3 kap.",
    source_url: "https://www.naturvardsverket.se/lagar-och-regler/foreskrifter-och-allmanna-rad/2023/nfs-20238/",
    // 12 §: "4,1 mm (US nr 1) eller mindre".
    max_pellet_mm: Some(4.1),
    no_lead_for_waterfowl: false,
    // 4 §.
    gauges: &[
        Twelve,
        Sixteen,
        Twenty,
        TwentyFour,
        TwentyEight,
        ThirtyTwo,
        Four10,
    ],
    // 14 §.
    small_gauges: &[TwentyFour, TwentyEight, ThirtyTwo, Four10],
    small_gauge_species: &[
        "european-rabbit",
        "polecat",
        "mink",
        "stoat",
        "red-squirrel",
        "pine-marten",
        "weasel",
        "muskrat",
        "goldeneye",
        "tufted-duck",
        "mallard",
        "wigeon",
        "teal",
        "woodcock",
        "willow-ptarmigan",
        "rock-ptarmigan",
        "hazel-grouse",
        "pheasant",
        "grey-partridge",
        "wood-pigeon",
        "common-gull",
        "herring-gull",
        "great-black-backed-gull",
        "goosander",
        "red-breasted-merganser",
        "raven",
        "hooded-crow",
        "rook",
        "jackdaw",
        "magpie",
        "jay",
        "fieldfare",
    ],
    // 13 §: "utom älg, hjortar, visent, bison, myskoxe, mufflonfår, varg,
    // björn, sälar, vildsvin, järv och bäver". Roe deer are not among the
    // "hjortar" there.
    no_shot: &[
        "moose",
        "red-deer",
        "wapiti",
        "fallow-deer",
        "white-tailed-deer",
        "sika-deer",
        "forest-reindeer",
        "mouflon",
        "wolf",
        "brown-bear",
        "grey-seal",
        "ringed-seal",
        "harbour-seal",
        "wild-boar",
        "wolverine",
        "beaver",
    ],
    // 16 §: a single-barrel 12, 16 or 20; wild boar, fallow deer and
    // mouflon for a hunter qualified for class 1 rifles, roe deer for
    // class 2.
    slugs: Slugs::Only {
        species: &[
            ("wild-boar", 1),
            ("fallow-deer", 1),
            ("mouflon", 1),
            ("roe-deer", 2),
        ],
        gauges: &[Twelve, Sixteen, Twenty],
    },
};

/// Metsästysasetus 666/1993, 17–18 §§, and Metsästyslaki 615/1993, 33 §.
static FINLAND: ShotgunLaw = ShotgunLaw {
    country: Country::Finland,
    law: "Metsästysasetus 666/1993, 17–18 §; metsästyslaki 615/1993, 33 §",
    source_url: "https://www.finlex.fi/fi/lainsaadanto/1993/666",
    max_pellet_mm: None,
    no_lead_for_waterfowl: true,
    // 17 §: gauge 10 to 20; smaller, down to 36 (.410), for a few species.
    gauges: &[
        Ten,
        Twelve,
        Sixteen,
        Twenty,
        TwentyFour,
        TwentyEight,
        ThirtyTwo,
        Four10,
    ],
    small_gauges: &[TwentyFour, TwentyEight, ThirtyTwo, Four10],
    small_gauge_species: &[
        "european-rabbit",
        "red-squirrel",
        "stoat",
        "willow-ptarmigan",
        "rock-ptarmigan",
        "hazel-grouse",
        "grey-partridge",
        "woodcock",
        "wood-pigeon",
    ],
    // 18 § 1 mom. Wapiti isn't named (nor game in Finland); it is kept
    // with red deer (saksanhirvi) as the same kind of animal.
    no_shot: &[
        "moose",
        "white-tailed-deer",
        "red-deer",
        "wapiti",
        "forest-reindeer",
        "fallow-deer",
        "sika-deer",
        "brown-bear",
        "wild-boar",
        "grey-seal",
        "ringed-seal",
        "harbour-seal",
        "mouflon",
    ],
    // 18 § 2 mom.: no slugs on moose or bear.
    slugs: Slugs::Except(&["moose", "brown-bear"]),
};

impl Country {
    /// The country's rules for shotguns.
    pub fn shotgun_law(self) -> &'static ShotgunLaw {
        match self {
            Country::Sweden => &SWEDEN,
            Country::Finland => &FINLAND,
        }
    }
}

impl ShotgunLaw {
    /// The gauges allowed for every species with shot: "12, 16, 20".
    pub fn full_gauges(&self) -> Vec<Gauge> {
        self.gauges
            .iter()
            .copied()
            .filter(|gauge| !self.small_gauges.contains(gauge))
            .collect()
    }

    /// Whether `species` (a catalog target id) may be shot with shot from
    /// `gauge`.
    pub fn shot(&self, species: &str, gauge: Gauge) -> ShotVerdict {
        if self.no_shot.contains(&species) {
            ShotVerdict::NotAllowed
        } else if !self.gauges.contains(&gauge) {
            ShotVerdict::GaugeNotAllowed
        } else if self.small_gauges.contains(&gauge) && !self.small_gauge_species.contains(&species)
        {
            ShotVerdict::GaugeTooSmall
        } else {
            ShotVerdict::Allowed
        }
    }

    /// Whether `species` may be shot with a slug from a gun of `gauge` with
    /// `barrels` barrels.
    pub fn slug(&self, species: &str, gauge: Gauge, barrels: usize) -> SlugVerdict {
        match &self.slugs {
            Slugs::Only {
                species: list,
                gauges,
            } => {
                let Some(&(_, class)) = list.iter().find(|(id, _)| *id == species) else {
                    return SlugVerdict::NotAllowed;
                };
                if !gauges.contains(&gauge) {
                    SlugVerdict::GaugeNotAllowed
                } else if barrels != 1 {
                    SlugVerdict::OnlySingleBarrel
                } else {
                    SlugVerdict::Allowed {
                        rifle_class: Some(class),
                    }
                }
            }
            Slugs::Except(list) => {
                if list.contains(&species) {
                    SlugVerdict::NotAllowed
                } else if !self.gauges.contains(&gauge)
                    || (self.small_gauges.contains(&gauge)
                        && !self.small_gauge_species.contains(&species))
                {
                    SlugVerdict::GaugeNotAllowed
                } else {
                    SlugVerdict::Allowed { rifle_class: None }
                }
            }
        }
    }
}

/// The EU's ban on lead shot in and within 100 m of wetlands applies from
/// this day (Regulation (EU) 2021/57)…
pub const EU_WETLAND_LEAD_BAN: &str = "2023-02-15";
/// …and its ban on lead shot for all hunting from this day (Regulation
/// (EU) 2026/2530). Slugs and bullets are not affected.
pub const EU_HUNTING_LEAD_BAN: &str = "2033-12-07";

/// Advice for shooting a species with shot, from a named body.
pub struct ShotAdvice {
    /// Who gives it.
    pub by: &'static str,
    pub source_url: &'static str,
    /// The catalog target ids it is for.
    pub species: &'static [&'static str],
    /// Shoot no farther than this, m.
    pub max_range_m: Option<f64>,
    /// Shot of these sizes, mm, whatever it is made of.
    pub any_mm: Option<(f64, f64)>,
    /// Lead shot of these sizes, mm.
    pub lead_mm: Option<(f64, f64)>,
    /// Steel shot of these sizes, mm.
    pub steel_mm: Option<(f64, f64)>,
}

/// Svenska Jägareförbundet's leaflet "Rätt ammunition": shot sizes per
/// species, as US numbers on its own scale (US 1 = 4.1 mm, then 0.25 mm a
/// number).
const SVJF: &str = "https://qr.jagareforbundet.se/contentassets/7830c70c9d5349f28c583f1dd84fee4c/nytt-korr-ratt-ammunition-a4.pdf";

/// The advice that applies in a country, all of it; the app picks a
/// species' with `advice_for`.
pub fn advice(country: Country) -> &'static [ShotAdvice] {
    // US 1 4.1, 2 3.75, 3 3.5, 4 3.25, 5 3.0, 6 2.75, 7 2.5 mm.
    static SWEDEN: [ShotAdvice; 10] = [
        sv(&["roe-deer"], (3.25, 3.5), (3.75, 4.1)),
        sv(&["red-fox"], (3.25, 3.5), (3.75, 4.1)),
        sv(
            &[
                "greylag-goose",
                "bean-goose",
                "white-fronted-goose",
                "canada-goose",
                "barnacle-goose",
            ],
            (3.25, 3.5),
            (3.75, 4.1),
        ),
        sv(&["brown-hare", "mountain-hare"], (2.75, 3.0), (3.25, 3.5)),
        sv(&["capercaillie"], (2.75, 3.25), (3.25, 3.75)),
        sv(&["mallard"], (2.5, 3.0), (3.0, 3.5)),
        sv(&["teal"], (2.5, 2.75), (3.0, 3.5)),
        sv(
            &[
                "willow-ptarmigan",
                "rock-ptarmigan",
                "black-grouse",
                "pheasant",
            ],
            (2.5, 2.75),
            (3.0, 3.25),
        ),
        sv(&["wood-pigeon"], (2.5, 2.75), (2.5, 3.0)),
        sv(&["hooded-crow"], (2.5, 2.75), (3.0, 3.25)),
    ];
    static FINLAND: [ShotAdvice; 2] = [
        // Riistakeskus, "Pienten hirvieläinten metsästyksen eettiset
        // ohjeet" (2018): roe deer at most 25 m, 3.5–4.0 mm.
        ShotAdvice {
            by: "Suomen riistakeskus",
            source_url: "https://riista.fi/wp-content/uploads/2018/09/eettiset_ohjeet_pienet_hirvielaimet.pdf",
            species: &["roe-deer"],
            max_range_m: Some(25.0),
            any_mm: Some((3.5, 4.0)),
            lead_mm: None,
            steel_mm: None,
        },
        // Metsästäjäliitto, "Aloittelevan metsästäjän ABC" (2020): shotgun
        // ranges at most 35 m.
        ShotAdvice {
            by: "Suomen Metsästäjäliitto",
            source_url: "https://metsastajaliitto.fi/sites/default/files/2020-10/Aloittelevan_metastajan_ABC_2.pdf",
            species: &[],
            max_range_m: Some(35.0),
            any_mm: None,
            lead_mm: None,
            steel_mm: None,
        },
    ];
    const fn sv(
        species: &'static [&'static str],
        lead_mm: (f64, f64),
        steel_mm: (f64, f64),
    ) -> ShotAdvice {
        ShotAdvice {
            by: "Svenska Jägareförbundet",
            source_url: SVJF,
            species,
            max_range_m: None,
            any_mm: None,
            lead_mm: Some(lead_mm),
            steel_mm: Some(steel_mm),
        }
    }
    match country {
        Country::Sweden => &SWEDEN,
        Country::Finland => &FINLAND,
    }
}

/// The advice for `species`: its own, and what is given for every species
/// (an empty species list).
pub fn advice_for(country: Country, species: &str) -> Vec<&'static ShotAdvice> {
    advice(country)
        .iter()
        .filter(|advice| advice.species.is_empty() || advice.species.contains(&species))
        .collect()
}

/// The Swedish hunting test's running target at 20 m with US 7 or 7½ shot
/// (NFS 2015:10, 20 §): a hit is at least this many pellets in a circle of
/// this diameter over the heart and lungs. A test of the hunter's aim, not a
/// standard for a kill.
pub struct HuntingTestFigure {
    pub species: &'static [&'static str],
    pub circle_cm: f64,
    pub pellets: u32,
}

pub const HUNTING_TEST_RANGE_M: f64 = 20.0;
pub const HUNTING_TEST_URL: &str = "https://www.naturvardsverket.se/4ab54f/contentassets/920f50525eae458ca353d4b92a7020d4/handbok-for-provledare-jagarexamen.pdf";

pub static HUNTING_TEST: [HuntingTestFigure; 2] = [
    HuntingTestFigure {
        species: &["brown-hare", "mountain-hare", "red-fox"],
        circle_cm: 9.0,
        pellets: 8,
    },
    HuntingTestFigure {
        species: &["roe-deer"],
        circle_cm: 18.0,
        pellets: 28,
    },
];

/// The hunting test's figure for `species`, if it has one.
pub fn hunting_test_figure(species: &str) -> Option<&'static HuntingTestFigure> {
    HUNTING_TEST
        .iter()
        .find(|figure| figure.species.contains(&species))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;

    #[test]
    fn swedish_shot_and_slugs_follow_nfs_2023_8() {
        let law = Country::Sweden.shotgun_law();
        assert_eq!(law.shot("roe-deer", Twelve), ShotVerdict::Allowed);
        assert_eq!(law.shot("lynx", Twenty), ShotVerdict::Allowed);
        assert_eq!(law.shot("moose", Twelve), ShotVerdict::NotAllowed);
        assert_eq!(law.shot("wild-boar", Twelve), ShotVerdict::NotAllowed);
        assert_eq!(law.shot("red-fox", Four10), ShotVerdict::GaugeTooSmall);
        assert_eq!(law.shot("mallard", Four10), ShotVerdict::Allowed);
        assert_eq!(law.shot("mallard", Ten), ShotVerdict::GaugeNotAllowed);
        assert_eq!(
            law.slug("wild-boar", Twelve, 1),
            SlugVerdict::Allowed {
                rifle_class: Some(1)
            }
        );
        assert_eq!(
            law.slug("roe-deer", Twenty, 1),
            SlugVerdict::Allowed {
                rifle_class: Some(2)
            }
        );
        assert_eq!(
            law.slug("wild-boar", Twelve, 2),
            SlugVerdict::OnlySingleBarrel
        );
        assert_eq!(law.slug("moose", Twelve, 1), SlugVerdict::NotAllowed);
        assert_eq!(law.slug("red-deer", Twelve, 1), SlugVerdict::NotAllowed);
        assert_eq!(
            law.slug("wild-boar", TwentyEight, 1),
            SlugVerdict::GaugeNotAllowed
        );
        assert_eq!(law.max_pellet_mm, Some(4.1));
        assert_eq!(law.full_gauges(), vec![Twelve, Sixteen, Twenty]);
        assert_eq!(
            Country::Finland.shotgun_law().full_gauges(),
            vec![Ten, Twelve, Sixteen, Twenty]
        );
    }

    #[test]
    fn finnish_shot_and_slugs_follow_the_decree() {
        let law = Country::Finland.shotgun_law();
        assert_eq!(law.shot("roe-deer", Twelve), ShotVerdict::Allowed);
        assert_eq!(
            law.shot("white-tailed-deer", Twelve),
            ShotVerdict::NotAllowed
        );
        assert_eq!(law.shot("hazel-grouse", Four10), ShotVerdict::Allowed);
        assert_eq!(law.shot("black-grouse", Four10), ShotVerdict::GaugeTooSmall);
        assert_eq!(law.shot("mallard", Ten), ShotVerdict::Allowed);
        assert_eq!(
            law.slug("white-tailed-deer", Twelve, 2),
            SlugVerdict::Allowed { rifle_class: None }
        );
        assert_eq!(law.slug("moose", Twelve, 1), SlugVerdict::NotAllowed);
        assert_eq!(law.slug("brown-bear", Twelve, 1), SlugVerdict::NotAllowed);
        assert!(law.no_lead_for_waterfowl);
    }

    #[test]
    fn every_species_the_rules_and_advice_name_is_in_the_catalog() {
        let catalog = Catalog::builtin();
        let known = |id: &str| catalog.target(id).is_some();
        for country in Country::ALL {
            let law = country.shotgun_law();
            for id in law
                .no_shot
                .iter()
                .chain(law.small_gauge_species)
                .copied()
                .chain(match &law.slugs {
                    Slugs::Only { species, .. } => species.iter().map(|(id, _)| *id).collect(),
                    Slugs::Except(list) => list.to_vec(),
                })
                .chain(
                    advice(country)
                        .iter()
                        .flat_map(|advice| advice.species.iter().copied()),
                )
            {
                assert!(known(id), "{country:?}: {id}");
            }
            for advice in advice(country) {
                assert!(advice.source_url.starts_with("https://"));
                for (low, high) in [advice.any_mm, advice.lead_mm, advice.steel_mm]
                    .into_iter()
                    .flatten()
                {
                    assert!(low > 1.0 && low <= high && high <= 4.1, "{}", advice.by);
                }
            }
        }
        for figure in &HUNTING_TEST {
            assert!(figure.species.iter().all(|id| known(id)));
        }
        assert_eq!(
            advice_for(Country::Finland, "roe-deer").len(),
            2,
            "Riistakeskus and Metsästäjäliitto"
        );
        assert_eq!(advice_for(Country::Sweden, "teal").len(), 1);
        assert!(advice_for(Country::Sweden, "moose").is_empty());
    }
}

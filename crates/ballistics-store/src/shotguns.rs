//! The hunter's shotguns: each gun with its barrels' chokes, the shells the
//! hunter shoots in it, and the patterns they counted on a pattern board.
//!
//! A shell holds shot (many pellets) or a slug. Its velocity is kept as the
//! box gives it, with where it was measured, since makers measure at the
//! muzzle, at 2.5 m (C.I.P.), at 3 feet (SAAMI) or as an average over the
//! first 20 yards (British "observed velocity").

use serde::{Deserialize, Serialize};

/// A shotgun's bore size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Gauge {
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "12")]
    Twelve,
    #[serde(rename = "16")]
    Sixteen,
    #[serde(rename = "20")]
    Twenty,
    #[serde(rename = "24")]
    TwentyFour,
    #[serde(rename = "28")]
    TwentyEight,
    #[serde(rename = "32")]
    ThirtyTwo,
    /// .410 bore, also called 36 gauge.
    #[serde(rename = ".410")]
    Four10,
}

impl Gauge {
    pub const ALL: [Gauge; 8] = [
        Gauge::Ten,
        Gauge::Twelve,
        Gauge::Sixteen,
        Gauge::Twenty,
        Gauge::TwentyFour,
        Gauge::TwentyEight,
        Gauge::ThirtyTwo,
        Gauge::Four10,
    ];

    /// "12", ".410".
    pub fn label(self) -> &'static str {
        match self {
            Gauge::Ten => "10",
            Gauge::Twelve => "12",
            Gauge::Sixteen => "16",
            Gauge::Twenty => "20",
            Gauge::TwentyFour => "24",
            Gauge::TwentyEight => "28",
            Gauge::ThirtyTwo => "32",
            Gauge::Four10 => ".410",
        }
    }

    /// The bore's nominal diameter, mm.
    pub fn bore_mm(self) -> f64 {
        match self {
            Gauge::Ten => 19.7,
            Gauge::Twelve => 18.5,
            Gauge::Sixteen => 16.8,
            Gauge::Twenty => 15.6,
            Gauge::TwentyFour => 14.7,
            Gauge::TwentyEight => 14.0,
            Gauge::ThirtyTwo => 12.7,
            Gauge::Four10 => 10.4,
        }
    }
}

/// A choke by its name, from open to tight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChokeName {
    /// No constriction (★★★★★).
    Cylinder,
    /// About 0.13 mm (⅛).
    Skeet,
    /// American improved cylinder, about 0.25 mm.
    ImprovedCylinder,
    /// Quarter choke (¼, ★★★★), often called improved cylinder in Europe.
    LightModified,
    /// Half choke (½, ★★★).
    Modified,
    /// Three-quarter choke (¾, ★★).
    ImprovedModified,
    /// Light full (⅞).
    LightFull,
    /// Full choke (1/1, ★).
    Full,
    /// Tighter than full, as turkey chokes are.
    ExtraFull,
}

impl ChokeName {
    pub const ALL: [ChokeName; 9] = [
        ChokeName::Cylinder,
        ChokeName::Skeet,
        ChokeName::ImprovedCylinder,
        ChokeName::LightModified,
        ChokeName::Modified,
        ChokeName::ImprovedModified,
        ChokeName::LightFull,
        ChokeName::Full,
        ChokeName::ExtraFull,
    ];

    /// "Half choke (½, ★★★)".
    pub fn label(self) -> &'static str {
        match self {
            ChokeName::Cylinder => "Cylinder (★★★★★)",
            ChokeName::Skeet => "Skeet (⅛)",
            ChokeName::ImprovedCylinder => "Improved cylinder (US)",
            ChokeName::LightModified => "Quarter choke (¼, ★★★★)",
            ChokeName::Modified => "Half choke (½, ★★★)",
            ChokeName::ImprovedModified => "Three-quarter choke (¾, ★★)",
            ChokeName::LightFull => "Light full (⅞)",
            ChokeName::Full => "Full choke (1/1, ★)",
            ChokeName::ExtraFull => "Extra full",
        }
    }

    /// The choke's other names and markings, beyond those in its label: the
    /// letters, notches and crosses stamped on barrels and tubes, and what
    /// American and European makers call it. From the tables of Beretta,
    /// Spencer (after Gough Thomas) and Hassel; see
    /// docs/shotgun-research/patterns.md.
    pub fn other_names(self) -> &'static str {
        match self {
            ChokeName::Cylinder => "C, CYL, CL, IIIII · true cylinder",
            ChokeName::Skeet => "SK",
            ChokeName::ImprovedCylinder => "IC · British improved cylinder",
            ChokeName::LightModified => {
                "IIII, ++ · improved cylinder (IC) in Europe · light modified (LM) in the US"
            }
            ChokeName::Modified => "III, +− · modified (M)",
            ChokeName::ImprovedModified => "II, ++− · improved modified (IM)",
            ChokeName::LightFull => "LF · light full",
            ChokeName::Full => "I, + · full (F)",
            ChokeName::ExtraFull => "XF · turkey",
        }
    }
}

/// A barrel's choke: by its name, or by its constriction as measured or
/// stamped.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Choke {
    /// By its name.
    Name {
        /// The choke.
        name: ChokeName,
    },
    /// By its constriction: how much narrower the choke is than the bore.
    Constriction {
        /// Constriction, mm.
        mm: f64,
    },
}

/// One of the gun's barrels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Barrel {
    /// What the hunter calls it: "Under", "Over", "Right"; empty for a gun
    /// with one barrel.
    #[serde(default)]
    pub name: String,
    /// Its choke, fixed or the tube in it.
    pub choke: Choke,
}

/// What shot is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShotMaterial {
    /// Lead (hardened with antimony).
    Lead,
    /// Soft iron, called steel.
    Steel,
    /// Bismuth with a little tin.
    Bismuth,
    /// Tungsten, nickel and iron, such as HEVI-Shot.
    TungstenIron,
    /// Tungsten powder in a polymer, such as Tungsten-Matrix.
    TungstenMatrix,
    /// Tungsten super shot (TSS).
    Tss,
    /// Zinc.
    Zinc,
}

impl ShotMaterial {
    pub const ALL: [ShotMaterial; 7] = [
        ShotMaterial::Lead,
        ShotMaterial::Steel,
        ShotMaterial::Bismuth,
        ShotMaterial::TungstenIron,
        ShotMaterial::TungstenMatrix,
        ShotMaterial::Tss,
        ShotMaterial::Zinc,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShotMaterial::Lead => "Lead",
            ShotMaterial::Steel => "Steel",
            ShotMaterial::Bismuth => "Bismuth",
            ShotMaterial::TungstenIron => "Tungsten-iron (HEVI-Shot)",
            ShotMaterial::TungstenMatrix => "Tungsten-Matrix",
            ShotMaterial::Tss => "Tungsten super shot (TSS)",
            ShotMaterial::Zinc => "Zinc",
        }
    }

    pub fn is_lead(self) -> bool {
        self == ShotMaterial::Lead
    }

    /// Hard shot that patterns tighter and wants an open choke: steel and
    /// the tungsten alloys.
    pub fn is_hard(self) -> bool {
        matches!(
            self,
            ShotMaterial::Steel | ShotMaterial::TungstenIron | ShotMaterial::Tss
        )
    }
}

/// Where the velocity on the box was measured.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VelocityAt {
    /// At the muzzle ("V0").
    Muzzle,
    /// 1 m from the muzzle ("V1", as Gamebore prints it).
    V1,
    /// 2.5 m from the muzzle ("V2.5", as C.I.P. makers print it).
    #[default]
    V2_5,
    /// 3 feet from the muzzle, as American makers print it.
    ThreeFeet,
    /// The British "observed velocity": the average over the first 20 yards.
    Observed,
}

impl VelocityAt {
    pub const ALL: [VelocityAt; 5] = [
        VelocityAt::Muzzle,
        VelocityAt::V1,
        VelocityAt::V2_5,
        VelocityAt::ThreeFeet,
        VelocityAt::Observed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VelocityAt::Muzzle => "At the muzzle (V0)",
            VelocityAt::V1 => "At 1 m (V1)",
            VelocityAt::V2_5 => "At 2.5 m (V2.5)",
            VelocityAt::ThreeFeet => "At 3 feet (US boxes)",
            VelocityAt::Observed => "Observed velocity (UK, over 20 yards)",
        }
    }
}

/// What a shell is loaded with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Payload {
    /// Shot: many round pellets.
    Shot {
        /// What the pellets are made of.
        material: ShotMaterial,
        /// One pellet's diameter, mm (the number on the box stands for
        /// different sizes in different countries).
        pellet_mm: f64,
        /// The whole load of shot, g.
        load_g: f64,
    },
    /// A slug: one projectile.
    Slug {
        /// The slug's mass, g.
        mass_g: f64,
        /// Its diameter, mm.
        diameter_mm: f64,
        /// Its ballistic coefficient against the G1 standard (lb/in²), as
        /// makers publish it.
        bc_g1: f64,
        /// It contains no lead.
        #[serde(default)]
        lead_free: bool,
    },
}

/// A pattern the hunter shot and counted: how many pellets landed inside a
/// circle at a distance. The Nordic test is a 75 cm circle at 35 m.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PatternTest {
    /// Which of the gun's barrels shot it, counted from 0 in `barrels`.
    pub barrel: usize,
    /// From the muzzle to the board, m.
    pub distance_m: f64,
    /// The circle's diameter, cm, drawn round the pattern's densest part.
    pub circle_cm: f64,
    /// Pellets counted inside the circle; 0 until counted, and a board
    /// with none is left out.
    pub pellets_inside: u32,
    /// Pellets in the whole load, if counted (or taken from the box);
    /// `null` to work them out from the load and pellet size.
    #[serde(default)]
    pub pellets_in_load: Option<u32>,
}

/// A shotgun shell in the hunter's collection: shot or a slug, for one
/// shotgun caliber, as rifle bullets are for one rifle caliber.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Shell {
    /// The shell's id, unique among shells.
    pub id: String,
    /// Its caliber's key in the app's catalog: "12-70" for a 12/70 shell
    /// (see the export's `catalog`, or docs/shotgun-research/calibers.md).
    /// It fits a gun of its gauge with a chamber as long or longer.
    #[serde(default)]
    pub caliber: String,
    /// Earlier files kept only the gauge; read into `caliber`.
    #[serde(default, rename = "gauge", skip_serializing)]
    pub legacy_gauge: Option<Gauge>,
    /// The catalog shell it was copied from, if any.
    #[serde(default)]
    pub catalog_id: Option<String>,
    /// Its maker: "Gyttorp".
    #[serde(default)]
    pub manufacturer: String,
    /// Its name, as the box gives it: "Röda Papp 32 g US 5".
    pub name: String,
    /// Shot or a slug.
    pub payload: Payload,
    /// The velocity the box gives (or the hunter measured), m/s.
    pub velocity_mps: f64,
    /// Where that velocity is measured.
    #[serde(default)]
    pub velocity_at: VelocityAt,
    /// Notes about it.
    #[serde(default)]
    pub notes: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

/// A shotgun: its caliber and barrels with their chokes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Shotgun {
    /// The gun's id, unique among shotguns.
    pub id: String,
    /// The hunter's name for it.
    pub name: String,
    /// Its caliber's key in the app's catalog: "12-76" for a 12/76.
    #[serde(default)]
    pub caliber: String,
    /// Earlier files kept only the gauge; read into `caliber`.
    #[serde(default, rename = "gauge", skip_serializing)]
    pub legacy_gauge: Option<Gauge>,
    /// Its barrels, each with a choke: one for a single-barrel gun, two for
    /// an over-and-under or side-by-side.
    pub barrels: Vec<Barrel>,
    /// For slugs: the height of the sights (or scope) above the bore, cm;
    /// `null` for a bead.
    #[serde(default)]
    pub sight_height_cm: Option<f64>,
    /// For slugs: the range the sights are set for, m.
    #[serde(default)]
    pub slug_zero_m: Option<f64>,
    /// Notes about it.
    #[serde(default)]
    pub notes: String,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

/// What is particular to a shell in one shotgun: the pattern boards the
/// hunter shot with it, as a rifle load holds the velocities measured in
/// one rifle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShotgunLoad {
    /// The record's id, unique among shotgun loads.
    pub id: String,
    /// The shotgun (`shotguns`).
    pub shotgun_id: String,
    /// The shell (`shells`).
    pub shell_id: String,
    /// Pattern boards counted with this shell in this gun, which replace
    /// its choke's nominal pattern for that barrel.
    #[serde(default)]
    pub patterns: Vec<PatternTest>,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

/// The most barrels a gun and pattern boards a load may have.
pub const MAX_BARRELS: usize = 4;
pub const MAX_PATTERN_TESTS: usize = 50;

impl Shotgun {
    /// A new 12-bore over-and-under to edit.
    pub fn new() -> Shotgun {
        let barrel = |name: &str, choke| Barrel {
            name: name.to_owned(),
            choke: Choke::Name { name: choke },
        };
        Shotgun {
            id: crate::model::new_id(),
            name: String::new(),
            caliber: "12-76".to_owned(),
            legacy_gauge: None,
            barrels: vec![
                barrel("Under", ChokeName::LightModified),
                barrel("Over", ChokeName::Modified),
            ],
            sight_height_cm: None,
            slug_zero_m: None,
            notes: String::new(),
            updated_at: 0,
        }
    }

    /// Its caliber from the catalog.
    pub fn caliber(&self) -> Option<&'static crate::ShotgunCaliber> {
        crate::Catalog::builtin().shotgun_caliber(&self.caliber)
    }

    /// Its gauge; 12 for a caliber the catalog doesn't know.
    pub fn gauge(&self) -> Gauge {
        self.caliber()
            .map_or(Gauge::Twelve, |caliber| caliber.gauge)
    }

    /// Whether a shell can be fired in it.
    pub fn takes(&self, shell: &Shell) -> bool {
        matches!((shell.caliber(), self.caliber()), (Some(shell), Some(gun)) if shell.fits(gun))
    }

    /// A record from before calibers: its gauge's usual caliber.
    pub fn upgrade_legacy(&mut self) {
        if let Some(gauge) = self.legacy_gauge.take()
            && self.caliber.is_empty()
            && let Some(caliber) = crate::Catalog::builtin().usual_shotgun_caliber(gauge)
        {
            self.caliber = caliber.key.clone();
        }
    }

    /// Whether the record can be stored: what the app can shoot.
    pub fn validate(&self) -> Result<(), String> {
        let problem = if self.id.trim().is_empty() {
            Some("a shotgun needs an id")
        } else if self.caliber().is_none() {
            Some("a shotgun needs a caliber from the catalog")
        } else if self.name.trim().is_empty() {
            Some("a shotgun needs a name")
        } else if self.barrels.is_empty() || self.barrels.len() > MAX_BARRELS {
            Some("a shotgun has 1 to 4 barrels")
        } else if !self
            .sight_height_cm
            .is_none_or(|height| (0.0..=20.0).contains(&height))
        {
            Some("the sights must be 0–20 cm above the bore")
        } else if !self
            .slug_zero_m
            .is_none_or(|zero| (5.0..=200.0).contains(&zero))
        {
            Some("the slug zero must be 5–200 m")
        } else if self.barrels.iter().any(|barrel| {
            matches!(barrel.choke, Choke::Constriction { mm } if !(0.0..=3.0).contains(&mm))
        }) {
            Some("a choke's constriction must be 0–3 mm")
        } else {
            None
        };
        problem.map_or(Ok(()), |problem| Err(problem.to_owned()))
    }
}

impl Default for Shotgun {
    fn default() -> Self {
        Shotgun::new()
    }
}

impl Shell {
    /// A new shell to edit: 32 g of 3 mm lead (US 5) at 400 m/s V2.5.
    pub fn new(caliber: &str) -> Shell {
        Shell {
            id: crate::model::new_id(),
            caliber: caliber.to_owned(),
            legacy_gauge: None,
            catalog_id: None,
            manufacturer: String::new(),
            name: String::new(),
            payload: Payload::Shot {
                material: ShotMaterial::Lead,
                pellet_mm: 3.0,
                load_g: 32.0,
            },
            velocity_mps: 400.0,
            velocity_at: VelocityAt::V2_5,
            notes: String::new(),
            updated_at: 0,
        }
    }

    /// Its caliber from the catalog.
    pub fn caliber(&self) -> Option<&'static crate::ShotgunCaliber> {
        crate::Catalog::builtin().shotgun_caliber(&self.caliber)
    }

    /// Its gauge; 12 for a caliber the catalog doesn't know.
    pub fn gauge(&self) -> Gauge {
        self.caliber()
            .map_or(Gauge::Twelve, |caliber| caliber.gauge)
    }

    /// A record from before calibers: its gauge's usual caliber.
    pub fn upgrade_legacy(&mut self) {
        if let Some(gauge) = self.legacy_gauge.take()
            && self.caliber.is_empty()
            && let Some(caliber) = crate::Catalog::builtin().usual_shotgun_caliber(gauge)
        {
            self.caliber = caliber.key.clone();
        }
    }

    /// "Gyttorp Röda Papp 32 g US 5".
    pub fn display_name(&self) -> String {
        if self.manufacturer.trim().is_empty() {
            self.name.clone()
        } else {
            format!("{} {}", self.manufacturer.trim(), self.name)
        }
    }

    /// Whether it contains lead.
    pub fn has_lead(&self) -> bool {
        match self.payload {
            Payload::Shot { material, .. } => material.is_lead(),
            Payload::Slug { lead_free, .. } => !lead_free,
        }
    }

    pub fn is_slug(&self) -> bool {
        matches!(self.payload, Payload::Slug { .. })
    }

    /// Whether the record can be stored: what the app can shoot.
    pub fn validate(&self) -> Result<(), String> {
        let problem = if self.id.trim().is_empty() {
            Some("a shell needs an id")
        } else if self.caliber().is_none() {
            Some("a shell needs a caliber from the catalog")
        } else if self.name.trim().is_empty() {
            Some("a shell needs a name")
        } else if !(50.0..=1000.0).contains(&self.velocity_mps) {
            Some("the velocity must be 50–1000 m/s")
        } else {
            match self.payload {
                Payload::Shot {
                    pellet_mm, load_g, ..
                } => {
                    if !(1.0..=10.0).contains(&pellet_mm) {
                        Some("the pellets must be 1–10 mm")
                    } else if !(1.0..=80.0).contains(&load_g) {
                        Some("the load of shot must be 1–80 g")
                    } else {
                        None
                    }
                }
                Payload::Slug {
                    mass_g,
                    diameter_mm,
                    bc_g1,
                    ..
                } => {
                    if !(3.0..=70.0).contains(&mass_g) {
                        Some("the slug must weigh 3–70 g")
                    } else if !(6.0..=25.0).contains(&diameter_mm) {
                        Some("the slug must be 6–25 mm across")
                    } else if !(0.01..=1.0).contains(&bc_g1) {
                        Some("the slug's G1 BC must be 0.01–1")
                    } else {
                        None
                    }
                }
            }
        };
        problem.map_or(Ok(()), |problem| Err(problem.to_owned()))
    }
}

impl ShotgunLoad {
    pub fn new(shotgun_id: &str, shell_id: &str) -> ShotgunLoad {
        ShotgunLoad {
            id: crate::model::new_id(),
            shotgun_id: shotgun_id.to_owned(),
            shell_id: shell_id.to_owned(),
            patterns: Vec::new(),
            updated_at: 0,
        }
    }

    /// Whether the record can be stored; `barrels` is how many the gun has,
    /// when it is known.
    pub fn validate(&self, barrels: Option<usize>) -> Result<(), String> {
        if self.id.trim().is_empty() || self.shotgun_id.is_empty() || self.shell_id.is_empty() {
            return Err("a shotgun load needs its id, gun and shell".to_owned());
        }
        if self.patterns.len() > MAX_PATTERN_TESTS {
            return Err("too many pattern boards".to_owned());
        }
        for test in &self.patterns {
            let problem = if barrels.is_some_and(|barrels| test.barrel >= barrels)
                || test.barrel >= MAX_BARRELS
            {
                Some("a pattern board names a barrel the gun doesn't have")
            } else if !(5.0..=100.0).contains(&test.distance_m) {
                Some("a pattern board must be shot from 5–100 m")
            } else if !(10.0..=300.0).contains(&test.circle_cm) {
                Some("a pattern board's circle must be 10–300 cm across")
            } else if test
                .pellets_in_load
                .is_some_and(|all| all == 0 || all > 100_000 || test.pellets_inside > all)
            {
                Some("a pattern board can't count more pellets inside than the load has")
            } else {
                None
            };
            if let Some(problem) = problem {
                return Err(problem.to_owned());
            }
        }
        Ok(())
    }
}

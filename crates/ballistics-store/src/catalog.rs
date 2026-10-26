//! The read-only base catalog of calibers and ready-made bullets. It is
//! compiled into the program from the JSON files in `catalog/`, so an app
//! update can replace it without touching the hunter's own database.
//! Bullets and scopes have one file per maker (`catalog/bullets/*.json`,
//! `catalog/scopes/*.json`); the build script lists them.

use std::sync::OnceLock;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::drawings::{Drawing, RENAMED};
use crate::model::{BcModel, Bullet, BulletKind, Construction, DataAttribution, new_id};
use crate::optics::{Reticle, Scope};
use crate::targets::Target;

mod files {
    include!(concat!(env!("OUT_DIR"), "/catalog_files.rs"));
}

/// Attribution for the separately licensed imported collection, also shown offline.
pub const CHAMBERSIM_DATA_NOTICE: &str =
    include_str!("../catalog/third-party/chambersim/ATTRIBUTION.txt");

/// One list from the files of a catalog folder, in file name order.
fn from_files<T: DeserializeOwned>(files: &[(&str, &str)]) -> serde_json::Result<Vec<T>> {
    let mut all = Vec::new();
    for (_, json) in files {
        all.extend(serde_json::from_str::<Vec<T>>(json)?);
    }
    Ok(all)
}

/// Bullets whose diameters differ by less than this fit the same bore.
const DIAMETER_TOLERANCE_MM: f64 = 0.03;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Caliber {
    pub key: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub bullet_diameter_mm: f64,
    /// A common twist for new rifles in this caliber; check your barrel.
    pub typical_twist_in: f64,
}

impl Caliber {
    /// Every word of the query must appear in the name or one alias, ignoring
    /// case and notation, so "6.5 swe", "6,5x55" and "6.5×55" all find 6.5×55 SE.
    pub fn matches(&self, query: &str) -> bool {
        let words: Vec<String> = query.split_whitespace().map(normalise).collect();
        std::iter::once(&self.name)
            .chain(&self.aliases)
            .any(|name| {
                let name = normalise(name);
                words.iter().all(|word| name.contains(word.as_str()))
            })
    }
}

fn normalise(text: &str) -> String {
    text.to_lowercase()
        .replace('×', "x")
        .replace(',', ".")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogKind {
    Ammunition,
    Component,
}

/// Explicitly recorded BC provenance. An absent classification is unknown;
/// it must not be interpreted as a manufacturer measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BcSource {
    Estimated,
    UserSupplied,
    Borrowed,
    Retailer,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CatalogBullet {
    pub id: String,
    pub kind: CatalogKind,
    pub manufacturer: String,
    pub product: String,
    /// Caliber key, for factory ammunition.
    pub caliber: Option<String>,
    /// Bullet diameter, for component bullets (and ammunition if published).
    pub diameter_mm: Option<f64>,
    pub mass_g: f64,
    pub mass_gr: Option<f64>,
    pub length_mm: Option<f64>,
    pub bc_g1: Option<f64>,
    pub bc_g7: Option<f64>,
    #[serde(default)]
    pub bc_source: Option<BcSource>,
    /// License and credit for a separately licensed data collection.
    #[serde(default)]
    pub data_attribution: Option<DataAttribution>,
    pub muzzle_velocity_mps: Option<f64>,
    pub test_barrel_mm: Option<f64>,
    pub construction: Construction,
    pub lead_free: bool,
    pub min_expansion_mps: Option<f64>,
    pub source_url: String,
    pub verified_on: String,
    #[serde(default)]
    pub notes: String,
}

impl CatalogBullet {
    pub fn display_name(&self) -> String {
        let grains = self.mass_gr.unwrap_or(self.mass_g / crate::GRAMS_PER_GRAIN);
        format!(
            "{} {} {:.1} g ({grains:.0} gr)",
            self.manufacturer, self.product, self.mass_g
        )
    }

    /// Whether this can be used in a rifle of the given caliber.
    pub fn fits(&self, caliber: &Caliber) -> bool {
        match self.kind {
            CatalogKind::Ammunition => self.caliber.as_deref() == Some(caliber.key.as_str()),
            CatalogKind::Component => self.diameter_mm.is_some_and(|diameter| {
                (diameter - caliber.bullet_diameter_mm).abs() < DIAMETER_TOLERANCE_MM
            }),
        }
    }

    /// A copy for the hunter's collection, loaded in `caliber`. Missing
    /// velocities become zero, so the hunter must enter one before saving.
    pub fn to_collection(&self, caliber: &Caliber) -> Bullet {
        let mut bullet = Bullet::new_manual(&caliber.key, caliber.bullet_diameter_mm);
        bullet.id = new_id();
        bullet.kind = match self.kind {
            CatalogKind::Ammunition => BulletKind::Factory,
            CatalogKind::Component => BulletKind::Handload,
        };
        bullet.manufacturer = self.manufacturer.clone();
        bullet.name = self.product.clone();
        bullet.diameter_mm = self.diameter_mm.unwrap_or(caliber.bullet_diameter_mm);
        bullet.mass_g = self.mass_g;
        bullet.length_mm = self.length_mm;
        bullet.bc_g1 = self.bc_g1;
        bullet.bc_g7 = self.bc_g7;
        // G7 describes modern boat-tail bullets better when both are given.
        bullet.drag_model = if self.bc_g7.is_some() {
            BcModel::G7
        } else {
            BcModel::G1
        };
        bullet.muzzle_velocity_mps = self.muzzle_velocity_mps.unwrap_or(0.0);
        bullet.test_barrel_mm = self.test_barrel_mm;
        bullet.construction = self.construction;
        bullet.lead_free = self.lead_free;
        bullet.min_expansion_mps = self.min_expansion_mps;
        bullet.catalog_id = Some(self.id.clone());
        bullet.data_attribution = self.data_attribution.clone();
        if self.data_attribution.is_some() {
            // Preserve the source's uncertainty notes independently of later catalog updates.
            bullet.notes = self.notes.clone();
        }
        bullet
    }
}

#[derive(Debug)]
pub struct Catalog {
    pub calibers: Vec<Caliber>,
    pub bullets: Vec<CatalogBullet>,
    pub scopes: Vec<Scope>,
    pub reticles: Vec<Reticle>,
    pub targets: Vec<Target>,
    pub drawings: Vec<Drawing>,
}

impl Catalog {
    /// The catalog shipped with this version of the app.
    pub fn builtin() -> &'static Catalog {
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Catalog::builtin_parts().expect("the built-in catalog is valid JSON (checked by tests)")
        })
    }

    fn builtin_parts() -> serde_json::Result<Catalog> {
        let mut bullets = from_files(files::BULLETS)?;
        bullets.extend(from_files::<CatalogBullet>(files::CHAMBERSIM_BULLETS)?);
        Catalog {
            calibers: serde_json::from_str(include_str!("../catalog/calibers.json"))?,
            bullets,
            scopes: from_files(files::SCOPES)?,
            reticles: serde_json::from_str(include_str!("../catalog/reticles.json"))?,
            targets: Vec::new(),
            drawings: Vec::new(),
        }
        .with_targets(
            include_str!("../catalog/targets.json"),
            include_str!("../catalog/drawings.json"),
        )
    }

    pub fn from_json(calibers: &str, bullets: &str) -> serde_json::Result<Catalog> {
        Ok(Catalog {
            calibers: serde_json::from_str(calibers)?,
            bullets: serde_json::from_str(bullets)?,
            scopes: Vec::new(),
            reticles: Vec::new(),
            targets: Vec::new(),
            drawings: Vec::new(),
        })
    }

    pub fn with_optics(self, scopes: &str, reticles: &str) -> serde_json::Result<Catalog> {
        Ok(Catalog {
            scopes: serde_json::from_str(scopes)?,
            reticles: serde_json::from_str(reticles)?,
            ..self
        })
    }

    pub fn with_targets(self, targets: &str, drawings: &str) -> serde_json::Result<Catalog> {
        let mut targets: Vec<Target> = serde_json::from_str(targets)?;
        targets.iter_mut().for_each(Target::upgrade_legacy);
        Ok(Catalog {
            targets,
            drawings: serde_json::from_str(drawings)?,
            ..self
        })
    }

    /// The drawing with this id (or the one that replaced it), else the
    /// first one, so a target always has something to show.
    pub fn drawing(&self, id: &str) -> &Drawing {
        let find = |id: &str| self.drawings.iter().find(|drawing| drawing.id == id);
        find(id)
            .or_else(|| {
                let (_, new) = RENAMED.iter().find(|(old, _)| *old == id)?;
                find(new)
            })
            .or(self.drawings.first())
            .expect("the built-in catalog has drawings")
    }

    pub fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|target| target.id == id)
    }

    pub fn scope(&self, id: &str) -> Option<&Scope> {
        self.scopes.iter().find(|scope| scope.id == id)
    }

    /// A reticle by its id, or by the id of an entry merged into it.
    pub fn reticle(&self, id: &str) -> Option<&Reticle> {
        self.reticles
            .iter()
            .find(|reticle| reticle.id == id)
            .or_else(|| {
                self.reticles
                    .iter()
                    .find(|reticle| reticle.replaces.iter().any(|old| old == id))
            })
    }

    /// The current id for a reticle id that may have been merged away.
    pub fn reticle_id<'a>(&'a self, id: &'a str) -> &'a str {
        self.reticle(id).map_or(id, |reticle| reticle.id.as_str())
    }

    pub fn caliber(&self, key: &str) -> Option<&Caliber> {
        self.calibers.iter().find(|caliber| caliber.key == key)
    }

    pub fn search_calibers(&self, query: &str) -> Vec<&Caliber> {
        self.calibers
            .iter()
            .filter(|caliber| caliber.matches(query))
            .collect()
    }

    pub fn bullet(&self, id: &str) -> Option<&CatalogBullet> {
        self.bullets.iter().find(|bullet| bullet.id == id)
    }

    /// Ammunition in this caliber and component bullets that fit its bore.
    pub fn bullets_for<'a>(
        &'a self,
        caliber: &'a Caliber,
    ) -> impl Iterator<Item = &'a CatalogBullet> {
        self.bullets
            .iter()
            .filter(move |bullet| bullet.fits(caliber))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn builtin_catalog_is_consistent() {
        let catalog = Catalog::builtin();
        assert!(catalog.calibers.len() >= 40);
        let keys: HashSet<_> = catalog.calibers.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys.len(), catalog.calibers.len(), "duplicate caliber key");
        assert!(
            keys.contains("22-long") && keys.contains("22-lr"),
            ".22 Long and .22 LR must remain separate calibers"
        );
        let ids: HashSet<_> = catalog.bullets.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids.len(), catalog.bullets.len(), "duplicate bullet id");

        for bullet in &catalog.bullets {
            let name = &bullet.id;
            match bullet.kind {
                CatalogKind::Ammunition => {
                    let caliber = bullet.caliber.as_deref().expect(name);
                    assert!(keys.contains(caliber), "{name}: unknown caliber {caliber}");
                    if let Some(velocity) = bullet.muzzle_velocity_mps {
                        assert!(
                            (0.0..1300.0).contains(&velocity),
                            "{name}: velocity {velocity}"
                        );
                    }
                }
                CatalogKind::Component => {
                    assert!(
                        catalog.calibers.iter().any(|caliber| bullet.fits(caliber)),
                        "{name}: no caliber fits this diameter"
                    );
                    assert!(
                        bullet.muzzle_velocity_mps.is_none(),
                        "{name}: component with velocity"
                    );
                }
            }
            assert!(
                bullet.bc_g1.is_some() || bullet.bc_g7.is_some(),
                "{name}: no BC"
            );
            for bc in [bullet.bc_g1, bullet.bc_g7].into_iter().flatten() {
                assert!((0.05..1.2).contains(&bc), "{name}: BC {bc}");
            }
            if let Some(grains) = bullet.mass_gr {
                let grams = grains * crate::GRAMS_PER_GRAIN;
                assert!(
                    (grams - bullet.mass_g).abs() < 0.1,
                    "{name}: {grains} gr vs {} g",
                    bullet.mass_g
                );
            }
            assert!(bullet.source_url.starts_with("https://"), "{name}: source");
            let collected =
                bullet.to_collection(catalog.calibers.iter().find(|c| bullet.fits(c)).unwrap());
            if bullet.kind == CatalogKind::Ammunition {
                if bullet.muzzle_velocity_mps.is_some() {
                    collected
                        .validate()
                        .unwrap_or_else(|problem| panic!("{name}: {problem}"));
                } else {
                    assert_eq!(
                        collected.muzzle_velocity_mps, 0.0,
                        "{name}: missing velocity"
                    );
                    assert!(collected.validate().is_err(), "{name}: missing velocity");
                }
            }
        }
    }

    #[test]
    fn sellier_bellot_profiles_include_both_published_bc_models() {
        let profiles: Vec<_> = Catalog::builtin()
            .bullets
            .iter()
            .filter(|bullet| {
                bullet.manufacturer == "Sellier & Bellot" && bullet.data_attribution.is_none()
            })
            .collect();
        assert_eq!(profiles.len(), 152);
        for profile in profiles {
            assert!(profile.bc_g1.is_some(), "{}: missing G1 BC", profile.id);
            assert!(profile.bc_g7.is_some(), "{}: missing G7 BC", profile.id);
        }
    }

    #[test]
    fn fiocchi_profiles_keep_published_bc_models() {
        let profiles: Vec<_> = Catalog::builtin()
            .bullets
            .iter()
            .filter(|bullet| bullet.manufacturer == "Fiocchi" && bullet.data_attribution.is_none())
            .collect();
        assert_eq!(profiles.len(), 46);
        assert_eq!(
            profiles
                .iter()
                .filter(|bullet| bullet.bc_g7.is_some())
                .count(),
            9
        );
        for profile in &profiles {
            assert!(profile.bc_g1.is_some(), "{}: missing G1 BC", profile.id);
            assert!(
                profile.source_url.starts_with("https://fiocchi.com/"),
                "{}: unexpected source",
                profile.id
            );
        }

        let profile = |id: &str| profiles.iter().find(|bullet| bullet.id == id).unwrap();
        assert_eq!(profile("fiocchi-709891-ammunition").bc_g7, None);
        let caliber = Catalog::builtin().caliber("308-win").unwrap();
        let copied = profile("fiocchi-709891-ammunition").to_collection(caliber);
        assert_eq!(copied.drag_model, BcModel::G1);
        assert_eq!(copied.ballistic_coefficient(), Some(0.355));
        assert_eq!(profile("fiocchi-709977-ammunition").bc_g7, Some(0.381));
        assert_eq!(profile("fiocchi-709976-ammunition").bc_g7, Some(0.314));
        assert!(profile("fiocchi-745582-ammunition").lead_free);
        assert!(profile("fiocchi-709898-ammunition").lead_free);
        assert!(!profile("fiocchi-709913-ammunition").lead_free);
    }

    #[test]
    fn prvi_partizan_profiles_keep_matched_g1_values() {
        let profiles: Vec<_> = Catalog::builtin()
            .bullets
            .iter()
            .filter(|bullet| {
                bullet.manufacturer == "Prvi Partizan" && bullet.data_attribution.is_none()
            })
            .collect();
        assert_eq!(profiles.len(), 55);
        assert_eq!(
            profiles
                .iter()
                .filter(|bullet| bullet.kind == CatalogKind::Ammunition)
                .count(),
            37
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|bullet| bullet.kind == CatalogKind::Component)
                .count(),
            18
        );
        for profile in profiles {
            assert!(profile.bc_g1.is_some(), "{}: missing G1 BC", profile.id);
            assert!(profile.bc_g7.is_none(), "{}: unexpected G7 BC", profile.id);
        }

        let catalog = Catalog::builtin();
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-a-132-standard")
                .unwrap()
                .bc_g1,
            Some(0.215)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-a-203-standard")
                .unwrap()
                .bc_g1,
            Some(0.256)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-a-718-match")
                .unwrap()
                .bc_g1,
            Some(0.400)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-b-540-component")
                .unwrap()
                .bc_g1,
            Some(0.400)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-a-721-match")
                .unwrap()
                .bc_g1,
            Some(0.519)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-b-689-component")
                .unwrap()
                .bc_g1,
            Some(0.535)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-a-594-match")
                .unwrap()
                .bc_g1,
            Some(0.485)
        );
        assert_eq!(
            catalog
                .bullets
                .iter()
                .find(|bullet| bullet.id == "ppu-b-594-component")
                .unwrap()
                .bc_g1,
            Some(0.485)
        );
    }

    #[test]
    fn builtin_optics_are_consistent() {
        let catalog = Catalog::builtin();
        let scope_ids: HashSet<_> = catalog.scopes.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(scope_ids.len(), catalog.scopes.len(), "duplicate scope id");
        let reticle_ids: HashSet<_> = catalog.reticles.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            reticle_ids.len(),
            catalog.reticles.len(),
            "duplicate reticle id"
        );
        for scope in &catalog.scopes {
            let name = &scope.id;
            assert!(
                scope.magnification_min > 0.0 && scope.magnification_min <= scope.magnification_max,
                "{name}"
            );
            assert!(!scope.reticles.is_empty(), "{name}: no reticles");
            for reticle in &scope.reticles {
                // Scopes whose reticle isn't drawn yet offer the plain crosshair.
                assert!(
                    reticle == crate::optics::PLAIN_CROSSHAIR || catalog.reticle(reticle).is_some(),
                    "{name}: unknown reticle {reticle}"
                );
            }
            assert!(
                scope.click_value.is_none_or(|value| value > 0.0),
                "{name}: click value"
            );
            if let Some([wide, narrow]) = scope.fov_m_at_100m {
                assert!(
                    wide >= narrow && narrow > 0.0,
                    "{name}: field of view {wide} / {narrow}"
                );
            }
            if scope.focal_plane == crate::optics::FocalPlane::Sfp {
                let reference = scope.reticle_reference_magnification.expect(name);
                assert!(
                    reference >= scope.magnification_min && reference <= scope.magnification_max,
                    "{name}"
                );
            }
            assert!(scope.source_url.starts_with("https://"), "{name}: source");
        }
        // One entry per drawing: scopes that share a reticle point to the
        // same entry instead of carrying a copy.
        let mut drawings = std::collections::HashMap::new();
        for reticle in &catalog.reticles {
            let drawing = format!("{:?} {:?}", reticle.unit, reticle.elements);
            if let Some(other) = drawings.insert(drawing, &reticle.id) {
                panic!("{} draws the same as {other}: share one entry", reticle.id);
            }
            for old in &reticle.replaces {
                assert!(
                    !reticle_ids.contains(old.as_str()),
                    "{}: replaces {old}, which still exists",
                    reticle.id
                );
            }
        }
        for reticle in &catalog.reticles {
            assert!(
                reticle.problems().is_empty(),
                "{}: {:?}",
                reticle.id,
                reticle.problems()
            );
            assert!(
                reticle.source_url.starts_with("https://"),
                "{}: source",
                reticle.id
            );
        }
    }

    #[test]
    fn reticles_are_found_by_every_name() {
        let catalog = Catalog::builtin();
        let found = |query: &str| -> Vec<&str> {
            catalog
                .reticles
                .iter()
                .filter(|reticle| reticle.matches(query))
                .map(|reticle| reticle.id.as_str())
                .collect()
        };
        assert!(found("deadhold 2a").contains(&"vortex-illuminated-dead-hold-bdc-moa"));
        assert!(
            found("Illuminated Dead-Hold BDC").contains(&"vortex-illuminated-dead-hold-bdc-moa")
        );
        assert!(found("ebr-7c mrad").len() >= 2);
        assert!(found("vortex ebr8").contains(&"vortex-ebr-8-moa"));
        // Every alternative name finds its reticle.
        for reticle in &catalog.reticles {
            for name in reticle.names() {
                assert!(reticle.matches(name), "{}: {name}", reticle.id);
            }
        }
    }

    #[test]
    fn merged_reticle_ids_still_resolve() {
        let catalog = Catalog::builtin();
        let merged = catalog
            .reticle("vortex-dead-hold-bdc-crossfire-hd-3-9x40")
            .expect("an old id finds the shared entry");
        assert_eq!(merged.id, "vortex-dead-hold-bdc-moa");
        assert_eq!(
            catalog.reticle_id("vortex-ar-bdc3-venom-1-6x24"),
            "vortex-ar-bdc3-moa"
        );
        assert_eq!(catalog.reticle_id("unknown"), "unknown");
    }

    #[test]
    fn chambersim_records_keep_their_license_and_unknown_fields() {
        let imported: Vec<CatalogBullet> = from_files(files::CHAMBERSIM_BULLETS).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../catalog/third-party/chambersim/import-manifest.json"
        ))
        .unwrap();
        assert_eq!(
            imported.len(),
            manifest["counts"]["imported"].as_u64().unwrap() as usize
        );
        assert!(!imported.is_empty());
        let catalog = Catalog::builtin();
        for entry in imported {
            assert!(entry.id.starts_with("chambersim-"));
            assert_eq!(entry.kind, CatalogKind::Component);
            assert!(entry.length_mm.is_none());
            assert_eq!(entry.construction, Construction::Other);
            assert!(!entry.lead_free);
            assert!(entry.min_expansion_mps.is_none());
            assert!(entry.verified_on.is_empty());
            let attribution = entry.data_attribution.as_ref().unwrap();
            assert_eq!(attribution.credit, "ChamberSim Reloaded project database");
            assert_eq!(attribution.license, "CC-BY-SA-4.0");
            assert_eq!(
                attribution.source_revision,
                manifest["source_revision"].as_str().unwrap()
            );
            assert!(entry.source_url.contains(&attribution.source_revision));
            assert_eq!(attribution.source_url, entry.source_url);
            assert!(CHAMBERSIM_DATA_NOTICE.contains(&attribution.source_revision));
            let caliber = catalog.calibers.iter().find(|c| entry.fits(c)).unwrap();
            let copied = entry.to_collection(caliber);
            assert_eq!(copied.data_attribution.as_ref(), Some(attribution));
            assert_eq!(copied.notes, entry.notes);
            assert_eq!(copied.muzzle_velocity_mps, 0.0);
        }
    }

    #[test]
    fn each_maker_file_holds_that_maker_only() {
        let slug = |maker: &str| {
            maker
                .to_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        };
        for (name, json) in files::BULLETS.iter().chain(files::CHAMBERSIM_BULLETS) {
            let bullets: Vec<CatalogBullet> = serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("bullets/{name}: {error}"));
            assert!(!bullets.is_empty(), "bullets/{name} is empty");
            for bullet in bullets {
                assert_eq!(
                    format!("{}.json", slug(&bullet.manufacturer)),
                    *name,
                    "{}",
                    bullet.id
                );
            }
        }
        for (name, json) in files::SCOPES {
            let scopes: Vec<Scope> =
                serde_json::from_str(json).unwrap_or_else(|error| panic!("scopes/{name}: {error}"));
            assert!(!scopes.is_empty(), "scopes/{name} is empty");
            for scope in scopes {
                assert_eq!(
                    format!("{}.json", slug(&scope.manufacturer)),
                    *name,
                    "{}",
                    scope.id
                );
            }
        }
    }

    #[test]
    fn every_catalog_bullet_can_be_added_to_a_collection() {
        let catalog = Catalog::builtin();
        for caliber in &catalog.calibers {
            for bullet in catalog.bullets_for(caliber) {
                let collected = bullet.to_collection(caliber);
                // Handloads need the hunter's velocity before saving.
                if collected.muzzle_velocity_mps > 0.0 {
                    collected.validate().unwrap_or_else(|problem| {
                        panic!("{} in {}: {problem}", collected.display_name(), caliber.key)
                    });
                }
            }
        }
    }

    #[test]
    fn caliber_search_ignores_notation() {
        let catalog = Catalog::builtin();
        for query in ["6.5x55", "6,5×55", "6.5 swedish", "6.5×55 SE"] {
            let found: Vec<_> = catalog
                .search_calibers(query)
                .into_iter()
                .map(|c| c.key.as_str())
                .collect();
            assert!(found.contains(&"6-5x55-se"), "{query}: {found:?}");
        }
        for query in [".300 BLK", "7.62x35mm"] {
            let found: Vec<_> = catalog
                .search_calibers(query)
                .into_iter()
                .map(|c| c.key.as_str())
                .collect();
            assert!(found.contains(&"300-aac-blackout"), "{query}: {found:?}");
        }
        assert_eq!(catalog.search_calibers("").len(), catalog.calibers.len());
    }

    #[test]
    fn components_fit_by_diameter_and_ammunition_by_caliber() {
        let catalog = Catalog::from_json(
            include_str!("../catalog/calibers.json"),
            r#"[
              {"id":"a","kind":"ammunition","manufacturer":"M","product":"P","caliber":"308-win","diameter_mm":null,
               "mass_g":11.7,"mass_gr":180,"length_mm":null,"bc_g1":0.4,"bc_g7":null,"muzzle_velocity_mps":790,
               "test_barrel_mm":null,"construction":"bonded","lead_free":false,"min_expansion_mps":null,
               "source_url":"https://example.com","verified_on":"2026-10-19"},
              {"id":"c","kind":"component","manufacturer":"M","product":"P","caliber":null,"diameter_mm":7.82,
               "mass_g":10.69,"mass_gr":165,"length_mm":30.0,"bc_g1":0.45,"bc_g7":0.22,"muzzle_velocity_mps":null,
               "test_barrel_mm":null,"construction":"monolithic","lead_free":true,"min_expansion_mps":550,
               "source_url":"https://example.com","verified_on":"2026-10-19"}
            ]"#,
        )
        .unwrap();
        let caliber = |key| catalog.caliber(key).unwrap();
        let ids = |key| {
            catalog
                .bullets_for(caliber(key))
                .map(|b| b.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("308-win"), ["a", "c"]);
        assert_eq!(ids("30-06-sprg"), ["c"]);
        assert!(ids("6-5x55-se").is_empty());

        let handload = catalog
            .bullet("c")
            .unwrap()
            .to_collection(caliber("30-06-sprg"));
        assert_eq!(handload.kind, BulletKind::Handload);
        assert_eq!(handload.caliber, "30-06-sprg");
        assert_eq!(handload.drag_model, BcModel::G7);
        assert_eq!(handload.catalog_id.as_deref(), Some("c"));
        assert!(
            handload.validate().is_err(),
            "needs the hunter's own velocity"
        );
        let factory = catalog
            .bullet("a")
            .unwrap()
            .to_collection(caliber("308-win"));
        assert_eq!(factory.kind, BulletKind::Factory);
        assert_eq!(factory.muzzle_velocity_mps, 790.0);
        factory.validate().unwrap();
    }
}

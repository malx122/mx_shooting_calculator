//! The import window: reads a backup (this app's JSON or a Strelok Pro
//! `.srl`), shows what importing it would do, and imports on confirmation.
//! Nothing is written until the hunter presses Import.

use std::collections::HashMap;

use ballistics_store::strelok::{CaliberSource, PlanOptions, StrelokPlan};
use ballistics_store::transfer::{PersonalData, read_export};
use ballistics_store::{ImportReport, Store, StoreError};
use eframe::egui::{self, RichText};

use crate::i18n::{tr, trf};
use crate::widgets::caliber_picker;

pub enum Source {
    /// One of this app's own exports.
    Export(Box<PersonalData>),
    Strelok(Box<StrelokImport>),
}

pub struct StrelokImport {
    xml: String,
    options: PlanOptions,
    plan: StrelokPlan,
    caliber_searches: HashMap<String, String>,
}

pub struct ImportPreview {
    file_name: String,
    source: Source,
    report: ImportReport,
}

/// What the hunter decided in the window.
pub enum Decision {
    Undecided,
    Cancel,
    /// Imported; for a Strelok backup, the rifle and bullet to select.
    Imported(ImportReport, Option<(String, Option<String>)>),
}

impl ImportPreview {
    /// Reads a backup file and works out what importing it would do.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(store: &mut Store, path: &std::path::Path) -> Result<ImportPreview, StoreError> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            StoreError::Invalid(trf!("could not read {}: {}", path.display(), error))
        })?;
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into(),
        );
        let modified_ms = std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_millis() as i64);
        ImportPreview::from_text(store, file_name, text, modified_ms)
    }

    /// Works out what importing a backup's contents would do. `modified_ms`
    /// is the file's modification time, when known.
    pub fn from_text(
        store: &mut Store,
        file_name: String,
        text: String,
        modified_ms: Option<i64>,
    ) -> Result<ImportPreview, StoreError> {
        let is_strelok = file_name.to_lowercase().ends_with(".srl") || text.contains("<StrelokPro");
        let source = if is_strelok {
            // Records take the backup's modification time, so a newer backup
            // updates what wasn't edited here since.
            let updated_at = modified_ms.unwrap_or_else(ballistics_store::model::now_ms);
            let options = PlanOptions {
                updated_at,
                ..Default::default()
            };
            let plan = store.plan_strelok(&text, &options)?;
            Source::Strelok(Box::new(StrelokImport {
                xml: text,
                options,
                plan,
                caliber_searches: HashMap::new(),
            }))
        } else {
            Source::Export(Box::new(read_export(&text)?))
        };
        let mut preview = ImportPreview {
            file_name,
            source,
            report: ImportReport::default(),
        };
        preview.refresh(store)?;
        Ok(preview)
    }

    fn data(&self) -> &PersonalData {
        match &self.source {
            Source::Export(data) => data,
            Source::Strelok(strelok) => &strelok.plan.data,
        }
    }

    /// Re-plans (after a caliber change) and re-counts with a dry run.
    fn refresh(&mut self, store: &mut Store) -> Result<(), StoreError> {
        if let Source::Strelok(strelok) = &mut self.source {
            strelok.plan = store.plan_strelok(&strelok.xml, &strelok.options)?;
        }
        self.report = store.import_data(self.data(), true)?;
        Ok(())
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        store: &mut Store,
    ) -> Result<Decision, StoreError> {
        let mut decision = Decision::Undecided;
        let mut replan = false;
        let mut open = true;
        egui::Window::new(trf!("Import {}", self.file_name))
            .id(egui::Id::unique("import"))
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(context, |ui| {
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| match &mut self.source {
                    Source::Export(data) => {
                        ui.label(trf!(
                            "An MX Shooting Calculator backup with {} rifles, {} bullets, {} rifle loads, {} scopes, {} targets and {} logged shots.",
                            data.rifles.len(),
                            data.bullets.len(),
                            data.rifle_loads.len(),
                            data.scopes.len(),
                            data.targets.len(),
                            data.shots.len()
                        ));
                    }
                    Source::Strelok(strelok) => {
                        let StrelokImport { options, plan, caliber_searches, .. } = &mut **strelok;
                        replan = strelok_details(ui, options, plan, caliber_searches);
                    }
                });
                ui.separator();
                let report = self.report;
                ui.label(RichText::new(summary(&report)).strong());
                ui.weak(tr(
                    "Records are recognised on every import, so importing the same backup again adds nothing, \
                     and anything you have changed here since is kept.",
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let changes = report.added + report.updated;
                    let label = if changes == 0 { tr("Nothing to import").to_owned() } else { trf!("Import {changes} change(s)", changes = changes) };
                    if ui.add_enabled(changes > 0, egui::Button::new(label)).clicked() {
                        decision = Decision::Imported(ImportReport::default(), None);
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        decision = Decision::Cancel;
                    }
                });
            });
        if !open {
            return Ok(Decision::Cancel);
        }
        if replan {
            self.refresh(store)?;
        }
        if let Decision::Imported(..) = decision {
            let report = store.import_data(self.data(), false)?;
            let selected = match &self.source {
                Source::Strelok(strelok) => strelok.plan.selected.clone(),
                Source::Export(_) => None,
            };
            return Ok(Decision::Imported(report, selected));
        }
        Ok(decision)
    }
}

/// The Strelok part of the window: rifles with their calibers, warnings.
/// Returns true when the hunter changed a caliber.
fn strelok_details(
    ui: &mut egui::Ui,
    options: &mut PlanOptions,
    plan: &StrelokPlan,
    searches: &mut HashMap<String, String>,
) -> bool {
    let mut changed = false;
    ui.label(trf!(
        "A Strelok Pro backup: {} rifles, {} bullets, {} scopes.",
        plan.data.rifles.len(),
        plan.data.bullets.len(),
        plan.data.scopes.len()
    ));
    ui.weak(tr("Check each rifle's caliber: only bullets of the rifle's caliber can be chosen when calculating."));
    ui.add_space(4.0);
    egui::Grid::new("strelok-rifles")
        .num_columns(4)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.strong(tr("Rifle in Strelok"));
            ui.strong(tr("Caliber"));
            ui.strong("");
            ui.strong(tr("Bullets"));
            ui.end_row();
            for rifle in &plan.rifles {
                ui.label(&rifle.strelok_name);
                let search = searches.entry(rifle.strelok_name.clone()).or_default();
                if let Some(caliber) = caliber_picker(
                    ui,
                    &format!("import-{}", rifle.rifle_id),
                    &rifle.caliber,
                    search,
                ) {
                    options
                        .caliber_overrides
                        .insert(rifle.strelok_name.clone(), caliber.key.clone());
                    changed = true;
                }
                let how = match rifle.caliber_source {
                    CaliberSource::Chosen => tr("chosen by you"),
                    CaliberSource::Existing => tr("kept from your rifle"),
                    CaliberSource::Names => tr("from the names"),
                    CaliberSource::Diameter => tr("from the bullet diameter"),
                    CaliberSource::Guess => tr("a guess: please check"),
                };
                if rifle.caliber_source.is_certain() {
                    ui.weak(how);
                } else {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {how}"));
                }
                ui.label(rifle.bullets.to_string());
                ui.end_row();
            }
        });
    if !plan.warnings.is_empty() {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(trf!("{} note(s)", plan.warnings.len()))
            .default_open(plan.warnings.len() <= 6)
            .show(ui, |ui| {
                for warning in &plan.warnings {
                    ui.label(format!("• {warning}"));
                }
            });
    }
    ui.add_space(4.0);
    ui.weak(tr(
        "Each rifle gets its own scope under My scopes (magnification, focal plane, clicks) with a plain crosshair; \
         pick a catalog reticle there if one matches. Strelok's own reticle numbers can't be used.",
    ));
    changed
}

pub fn summary(report: &ImportReport) -> String {
    let mut parts = vec![
        trf!("{} new", report.added),
        trf!("{} updated", report.updated),
    ];
    if report.unchanged > 0 {
        parts.push(trf!("{} already here", report.unchanged));
    }
    if report.kept_local > 0 {
        parts.push(trf!("{} kept as you changed them", report.kept_local));
    }
    parts.join(", ")
}

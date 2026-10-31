//! Named appearance presets for printed labels. They keep presentation choices
//! only; applying a preset leaves the label's data and physical turret alone.

use std::collections::BTreeMap;

use eframe::egui;
use serde::{Deserialize, Serialize};

use super::fonts::Font;
use super::turret::{Colours, Marker, Numbers, Rgb, TurretOptions};
use crate::i18n::tr;
use crate::widgets::confirm_button;

/// The presentation choices that can be reused independently of label data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelAppearance {
    numbers: Numbers,
    ruler: bool,
    description: bool,
    copies: u32,
    font: Font,
    number_size: f32,
    marker: Marker,
    marker_line_start: f32,
    #[serde(alias = "marker_line_length")]
    marker_line_end: f32,
    background: Rgb,
    ink: Rgb,
    beyond: Rgb,
    description_colours: Colours,
    ruler_colours: Colours,
    bands: Vec<Colours>,
    range_colours: BTreeMap<u32, Rgb>,
}

impl Default for LabelAppearance {
    fn default() -> Self {
        Self::capture(&TurretOptions::default())
    }
}

impl LabelAppearance {
    fn capture(options: &TurretOptions) -> Self {
        Self {
            numbers: options.numbers,
            ruler: options.ruler,
            description: options.description,
            copies: options.copies,
            font: options.font,
            number_size: options.number_size,
            marker: options.marker,
            marker_line_start: options.marker_line_start,
            marker_line_end: options.marker_line_end,
            background: options.background,
            ink: options.ink,
            beyond: options.beyond,
            description_colours: options.description_colours,
            ruler_colours: options.ruler_colours,
            bands: options.bands.clone(),
            range_colours: options.range_colours.clone(),
        }
    }

    fn apply(&self, options: &mut TurretOptions) {
        options.numbers = self.numbers;
        options.ruler = self.ruler;
        options.description = self.description;
        options.copies = self.copies;
        options.font = self.font;
        options.number_size = self.number_size;
        options.marker = self.marker;
        options.marker_line_start = self.marker_line_start;
        options.marker_line_end = self.marker_line_end;
        options.background = self.background;
        options.ink = self.ink;
        options.beyond = self.beyond;
        options.description_colours = self.description_colours;
        options.ruler_colours = self.ruler_colours;
        self.bands.clone_into(&mut options.bands);
        self.range_colours.clone_into(&mut options.range_colours);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LabelDesign {
    name: String,
    appearance: LabelAppearance,
}

/// A rifle's saved designs, kept in the existing device-local card settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabelDesigns {
    designs: Vec<LabelDesign>,
}

impl LabelDesigns {
    fn name_available(&self, name: &str, except: Option<usize>) -> bool {
        let name = name.trim();
        !name.is_empty()
            && name.chars().count() <= 80
            && !name.chars().any(char::is_control)
            && !self.designs.iter().enumerate().any(|(index, design)| {
                Some(index) != except && design.name.to_lowercase() == name.to_lowercase()
            })
    }

    fn save_as(&mut self, name: &str, options: &TurretOptions) -> Option<usize> {
        if !self.name_available(name, None) {
            return None;
        }
        self.designs.push(LabelDesign {
            name: name.trim().into(),
            appearance: LabelAppearance::capture(options),
        });
        Some(self.designs.len() - 1)
    }

    fn rename(&mut self, index: usize, name: &str) -> bool {
        if !self.name_available(name, Some(index)) {
            return false;
        }
        let Some(design) = self.designs.get_mut(index) else {
            return false;
        };
        design.name = name.trim().into();
        true
    }
}

/// Selection and name editing are transient; saved appearances are explicit
/// snapshots, so editing one design cannot silently overwrite another.
#[derive(Default)]
pub struct DesignEditor {
    rifle_id: String,
    selected: Option<usize>,
    name: String,
}

impl DesignEditor {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        rifle_id: &str,
        library: &mut LabelDesigns,
        options: &mut TurretOptions,
    ) {
        if self.rifle_id != rifle_id {
            self.rifle_id = rifle_id.into();
            self.selected = None;
            self.name.clear();
        }
        self.selected = self.selected.filter(|&index| index < library.designs.len());
        ui.push_id(("label-designs", rifle_id), |ui| {
            ui.strong(tr("Label designs"));
            ui.weak(tr(
                "Appearance and print choices, kept on this device for this rifle.",
            ));
            let mut picked = self.selected;
            let mut apply = false;
            egui::ComboBox::from_id_salt("design-picker")
                .width(ui.available_width())
                .truncate()
                .selected_text(self.selected.map_or(tr("Current appearance"), |index| {
                    &library.designs[index].name
                }))
                .show_ui(ui, |ui| {
                    for (index, design) in library.designs.iter().enumerate() {
                        if ui
                            .selectable_label(picked == Some(index), &design.name)
                            .clicked()
                        {
                            picked = Some(index);
                            apply = true;
                            ui.close();
                        }
                    }
                });
            if apply {
                self.selected = picked;
                if let Some(index) = picked {
                    let design = &library.designs[index];
                    design.appearance.apply(options);
                    self.name.clone_from(&design.name);
                }
            }
            if self.selected.is_some_and(|index| {
                library.designs[index].appearance != LabelAppearance::capture(options)
            }) {
                ui.weak(tr(
                    "Appearance changed. Update the design or save it with a new name.",
                ));
            }
            ui.label(tr("Design name"));
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .id_salt("design-name")
                    .char_limit(80)
                    .desired_width(ui.available_width())
                    .hint_text(tr("e.g. High contrast")),
            );
            let new_name = library.name_available(&self.name, None);
            if !self.name.trim().is_empty() && !library.name_available(&self.name, self.selected) {
                ui.weak(tr("Use a unique name of up to 80 characters."));
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(new_name, egui::Button::new(tr("Save as new")))
                    .clicked()
                {
                    self.selected = library.save_as(&self.name, options);
                    self.name = self.name.trim().into();
                }
                if let Some(index) = self.selected {
                    let changed =
                        library.designs[index].appearance != LabelAppearance::capture(options);
                    if ui
                        .add_enabled(changed, egui::Button::new(tr("Update design")))
                        .clicked()
                    {
                        library.designs[index].appearance = LabelAppearance::capture(options);
                    }
                    let renamed = self.name.trim() != library.designs[index].name
                        && library.name_available(&self.name, Some(index));
                    if ui
                        .add_enabled(renamed, egui::Button::new(tr("Rename")))
                        .clicked()
                    {
                        library.rename(index, &self.name);
                        self.name = self.name.trim().into();
                    }
                    if confirm_button(ui, &format!("delete-design-{index}"), tr("Delete")) {
                        library.designs.remove(index);
                        self.selected = None;
                    }
                }
            });
            ui.weak(tr(
                "Choose a design to apply it. To make a copy, change its name and save as new.",
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::CardSettings;

    fn draw(
        context: &egui::Context,
        editor: &mut DesignEditor,
        library: &mut LabelDesigns,
        options: &mut TurretOptions,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                crate::mobile::apply_style(context);
                editor.show(ui, "rifle-one", library, options);
                assert!(
                    ui.min_rect().right() <= 320.5,
                    "design controls overflow the phone: {:?}, {:?}",
                    crate::i18n::current(),
                    ui.min_rect()
                );
            },
        );
        output.textures_delta.clear();
        output
    }

    fn click(
        context: &egui::Context,
        editor: &mut DesignEditor,
        library: &mut LabelDesigns,
        options: &mut TurretOptions,
        label: &str,
    ) {
        let output = draw(context, editor, library, options, Vec::new());
        let point = output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    return Some(text.pos + text.galley.size() * 0.5);
                }
                None
            })
            .unwrap_or_else(|| panic!("missing control: {label}"));
        draw(
            context,
            editor,
            library,
            options,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }

    #[test]
    fn editor_saves_updates_copies_and_confirms_deletion_on_a_phone() {
        crate::i18n::set(crate::i18n::Language::English);
        let context = egui::Context::default();
        let mut editor = DesignEditor::default();
        let mut library = LabelDesigns::default();
        let mut options = TurretOptions::default();
        for _ in 0..2 {
            draw(
                &context,
                &mut editor,
                &mut library,
                &mut options,
                Vec::new(),
            );
        }
        editor.name = "Original".into();
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Save as new",
        );
        assert_eq!(library.designs.len(), 1);
        options.ink = [200, 120, 80];
        assert_ne!(
            library.designs[0].appearance,
            LabelAppearance::capture(&options)
        );
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Update design",
        );
        assert_eq!(
            library.designs[0].appearance,
            LabelAppearance::capture(&options)
        );
        let saved_background = options.background;
        options.background = [5, 10, 15];
        editor.name.clear();
        // Selecting the current preset again also restores its saved appearance.
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Original",
        );
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Original",
        );
        assert_eq!(options.background, saved_background);
        editor.name = "Copy".into();
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Save as new",
        );
        assert_eq!(library.designs.len(), 2);
        editor.name = "Renamed".into();
        click(&context, &mut editor, &mut library, &mut options, "Rename");
        assert_eq!(library.designs[1].name, "Renamed");
        click(&context, &mut editor, &mut library, &mut options, "Delete");
        assert_eq!(library.designs.len(), 2, "deletion needs confirmation");
        click(
            &context,
            &mut editor,
            &mut library,
            &mut options,
            "Really delete?",
        );
        assert_eq!(library.designs.len(), 1);
        assert_eq!(library.designs[0].name, "Original");
        assert_eq!(editor.selected, None);
    }

    #[test]
    fn long_design_names_fit_on_a_phone_in_every_language() {
        for language in crate::i18n::Language::ALL {
            crate::i18n::set(language);
            let context = egui::Context::default();
            let mut library = LabelDesigns::default();
            let mut options = TurretOptions::default();
            let name = "W".repeat(80);
            let index = library.save_as(&name, &options).unwrap();
            let mut editor = DesignEditor {
                rifle_id: "rifle-one".into(),
                selected: Some(index),
                name,
            };
            for _ in 0..2 {
                draw(
                    &context,
                    &mut editor,
                    &mut library,
                    &mut options,
                    Vec::new(),
                );
            }
        }
        crate::i18n::set(crate::i18n::Language::English);
    }

    #[test]
    fn saved_designs_are_independent_and_names_cannot_overwrite_each_other() {
        let mut library = LabelDesigns::default();
        let mut options = TurretOptions::default();
        let first = library.save_as("  High contrast  ", &options).unwrap();
        options.ink = [240, 180, 80];
        let second = library.save_as("Amber", &options).unwrap();
        assert_eq!(library.designs[first].name, "High contrast");
        assert_ne!(
            library.designs[first].appearance,
            library.designs[second].appearance
        );
        assert_eq!(library.save_as("HIGH CONTRAST", &options), None);
        assert!(!library.rename(second, " high contrast "));
        assert!(library.rename(second, " Warm amber "));
        assert_eq!(library.designs[second].name, "Warm amber");
        for name in ["", "  ", "line\nbreak", &"x".repeat(81)] {
            assert_eq!(library.save_as(name, &options), None);
        }
        assert_eq!(library.designs.len(), 2);
        library.designs.remove(first);
        assert_eq!(library.designs[0].name, "Warm amber");
    }

    #[test]
    fn applying_a_design_restores_appearance_and_preserves_label_data() {
        let mut source = TurretOptions {
            font: Font::ALL[1],
            numbers: Numbers::Full,
            marker: Marker::Line,
            marker_line_start: 0.2,
            marker_line_end: 0.6,
            ruler: false,
            description: false,
            copies: 5,
            number_size: 0.8,
            background: [30, 40, 50],
            ink: [210, 220, 230],
            beyond: [110, 120, 130],
            description_colours: Colours {
                ink: Some([10, 20, 30]),
                background: None,
            },
            ruler_colours: Colours {
                ink: None,
                background: Some([40, 50, 60]),
            },
            bands: vec![Colours {
                ink: Some([70, 80, 90]),
                background: None,
            }],
            ..TurretOptions::default()
        };
        source.range_colours.insert(200, [150, 160, 170]);
        let appearance = LabelAppearance::capture(&source);
        let mut current = TurretOptions {
            end_m: 800.0,
            numbers_every: 50.0,
            marks_every: 10.0,
            temperature: super::super::turret::Temperature::Chosen(12.0),
            stop_at_limit: true,
            ..TurretOptions::default()
        };
        let before = serde_json::to_value(&current).unwrap();
        appearance.apply(&mut current);
        assert_eq!(LabelAppearance::capture(&current), appearance);
        let after = serde_json::to_value(&current).unwrap();
        for field in [
            "end_m",
            "numbers_every",
            "marks_every",
            "temperature",
            "stop_at_limit",
        ] {
            assert_eq!(
                after[field], before[field],
                "preset changed label data: {field}"
            );
        }
    }

    #[test]
    fn old_card_settings_load_and_named_designs_persist_separately_for_each_rifle() {
        let settings = CardSettings::default();
        let mut old_json = serde_json::to_value(&settings).unwrap();
        old_json.as_object_mut().unwrap().remove("label_designs");
        let mut settings: CardSettings = serde_json::from_value(old_json).unwrap();
        assert!(settings.label_designs.is_empty());
        let first = settings
            .label_designs
            .entry("rifle-one".into())
            .or_default();
        first.save_as("My design", &settings.turret).unwrap();
        settings.turret.ink = [100, 120, 140];
        let second = settings
            .label_designs
            .entry("rifle-two".into())
            .or_default();
        second.save_as("My design", &settings.turret).unwrap();
        let store = ballistics_store::Store::open_in_memory().unwrap();
        store.set_setting("cards", &settings).unwrap();
        let restored: CardSettings = store.setting("cards").unwrap().unwrap();
        assert_eq!(restored, settings);
        assert_ne!(
            restored.label_designs["rifle-one"].designs[0].appearance,
            restored.label_designs["rifle-two"].designs[0].appearance
        );
    }

    #[test]
    fn saved_design_lengths_load_as_end_positions() {
        let options = TurretOptions {
            marker: Marker::Line,
            marker_line_end: 0.63,
            ..TurretOptions::default()
        };
        let appearance = LabelAppearance::capture(&options);
        let mut value = serde_json::to_value(&appearance).unwrap();
        let fields = value.as_object_mut().unwrap();
        let end = fields.remove("marker_line_end").unwrap();
        fields.remove("marker_line_start");
        fields.insert("marker_line_length".into(), end);
        let restored: LabelAppearance = serde_json::from_value(value).unwrap();
        assert_eq!(restored, appearance);
    }
}

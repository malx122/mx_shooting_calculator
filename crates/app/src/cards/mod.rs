//! Printable cards: the dope card (what to dial or hold at each range) and
//! the stand card (a map of a hunting stand with its ranges and limits).
//! Each card is laid out once as a `Sheet` in millimetres, shown as the
//! preview and saved as a PDF, so the preview is what prints.

pub mod dope;
pub mod fonts;
mod pdf;
pub mod sheet;
pub mod stand;
pub mod turret;

use ballistics_core::Solution;
use ballistics_core::analysis::{self, PointBlank};
use ballistics_core::solver::SolveError;
use ballistics_store::{Bullet, Rifle, RifleLoad};
use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};

use crate::app::{Action, Page};
use crate::mobile;
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs};
use crate::widgets::{confirm_button, grid};
use dope::{DopeOptions, DopeTable, Measure};
use sheet::Sheet;
use stand::{Landmark, NoShoot, Stand, StandOptions, View};
use turret::{Marker, Numbers, Plan, Temperature, Turret, TurretOptions};

/// Paper and card sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardSize {
    /// A bank card, ISO ID-1.
    Card,
    A7,
    A6,
    A5,
    A4,
}

impl CardSize {
    pub const ALL: [CardSize; 5] = [
        CardSize::Card,
        CardSize::A7,
        CardSize::A6,
        CardSize::A5,
        CardSize::A4,
    ];

    /// Width and height upright, mm.
    pub fn portrait(self) -> [f32; 2] {
        match self {
            CardSize::Card => [53.98, 85.6],
            CardSize::A7 => [74.0, 105.0],
            CardSize::A6 => [105.0, 148.0],
            CardSize::A5 => [148.0, 210.0],
            CardSize::A4 => [210.0, 297.0],
        }
    }

    pub fn oriented(self, landscape: bool) -> [f32; 2] {
        let [width, height] = self.portrait();
        if landscape {
            [height, width]
        } else {
            [width, height]
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CardSize::Card => "Card",
            CardSize::A7 => "A7",
            CardSize::A6 => "A6",
            CardSize::A5 => "A5",
            CardSize::A4 => "A4",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            CardSize::Card => "86 × 54 mm, a bank card: a wallet or a stock pouch",
            CardSize::A7 => "74 × 105 mm: a shirt pocket",
            CardSize::A6 => "105 × 148 mm: a postcard, a binocular harness",
            CardSize::A5 => "148 × 210 mm: half a sheet",
            CardSize::A4 => "210 × 297 mm: a whole sheet",
        }
    }
}

/// What a card is for: the rifle and load, the game, and the weather.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    pub rifle: Rifle,
    pub bullet: Bullet,
    pub load: Option<RifleLoad>,
    /// The target's name, if one is chosen.
    pub target: Option<String>,
    /// The vital (or hit) zone across.
    pub vital_cm: Option<f64>,
    /// `None` for no ethical limit (sport targets, no target).
    pub min_energy_j: Option<f64>,
    pub min_velocity_mps: f64,
    pub conditions: ballistics_shot::Conditions,
    /// The scope's whole elevation travel in clicks, when known.
    pub elevation_travel_clicks: Option<f64>,
    /// The weather the load was zeroed in, if known.
    pub zero_weather: Option<ballistics_store::ZeroWeather>,
}

impl Subject {
    /// The flight at an air temperature (the cartridges at the same) in a
    /// full-value wind from the right, level.
    pub fn solve(
        &self,
        temperature_c: f64,
        wind_mps: f64,
        max_range: f64,
    ) -> Result<Solution, SolveError> {
        self.solve_at(temperature_c, None, wind_mps, max_range)
    }

    /// Like `solve`, with the cartridges at their own temperature (`None`:
    /// the air's).
    pub fn solve_at(
        &self,
        temperature_c: f64,
        powder_temperature_c: Option<f64>,
        wind_mps: f64,
        max_range: f64,
    ) -> Result<Solution, SolveError> {
        let conditions = ballistics_shot::Conditions {
            temperature_c,
            powder_temperature_c,
            wind_speed_mps: wind_mps,
            wind_clock: 3.0,
            inclination_degrees: 0.0,
            coriolis: None,
            cant_degrees: 0.0,
            ..self.conditions.clone()
        };
        let scenario =
            ballistics_shot::scenario(&self.rifle, &self.bullet, self.load.as_ref(), &conditions);
        ballistics_core::solver::solve(&scenario, max_range)
    }

    pub fn ethical_limit(&self, solution: &Solution) -> Option<f64> {
        self.min_energy_j
            .and_then(|energy| analysis::ethical_limit(solution, energy, self.min_velocity_mps))
    }

    pub fn dead_on(&self, solution: &Solution) -> Option<PointBlank> {
        self.vital_cm
            .and_then(|cm| analysis::point_blank_with_zero(solution, cm / 100.0))
    }
}

/// Today's date, for the foot of a card.
pub fn today() -> String {
    let days = ballistics_store::model::now_ms().div_euclid(86_400_000);
    // Days since 1970 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Dope,
    Stand,
    Turret,
}

/// The hunter's card choices and stands, kept on this device.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardSettings {
    pub kind: Kind,
    pub dope: DopeOptions,
    pub stand: StandOptions,
    pub stands: Vec<Stand>,
    pub stand_index: usize,
    pub turret: TurretOptions,
    /// Each rifle's elevation turret, by rifle id.
    pub turrets: std::collections::BTreeMap<String, Turret>,
}

/// A compass reading wanted for a field.
#[derive(Clone, Copy, PartialEq)]
enum Heading {
    Facing,
    SectorLeft,
    SectorRight,
    Landmark(usize),
}

/// The laid-out card and what came of it.
struct Preview {
    card: Sheet,
    /// The pages to print.
    pages: Vec<Sheet>,
    text_size: Option<f32>,
    rows: usize,
    columns: usize,
    ethical_m: Option<f64>,
    /// Things worth knowing about how the card came out.
    notes: Vec<String>,
    /// The turret label's marks.
    plan: Option<Plan>,
}

type PreviewKey = (CardSettings, Subject, UnitPrefs);

pub struct CardsPage {
    pub settings: CardSettings,
    preview: Option<(PreviewKey, Result<Preview, String>)>,
    /// Show the card at its printed size instead of fitting it.
    true_size: bool,
    /// The compass reading wanted, once the compass answers.
    heading: Option<Heading>,
    status: Option<(String, bool)>,
}

impl CardsPage {
    pub fn new(settings: CardSettings) -> CardsPage {
        CardsPage {
            settings,
            preview: None,
            true_size: false,
            heading: None,
            status: None,
        }
    }

    pub fn show(&mut self, context: &egui::Context, subject: Option<Subject>) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some(subject) = subject else {
            egui::CentralPanel::default().show(context, |ui| {
                ui.heading("Cards");
                ui.label(
                    "Dope cards and stand cards are made for a rifle and a bullet. Add a rifle \
                     and a bullet for it first.",
                );
                if ui.button("Go to Rifles").clicked() {
                    actions.push(Action::Goto(Page::Rifles));
                }
            });
            return actions;
        };
        if self.settings.stands.is_empty() {
            self.settings.stands.push(Stand::default());
        }
        self.settings.stand_index = self
            .settings
            .stand_index
            .min(self.settings.stands.len() - 1);
        self.take_heading(context);
        let u = units::prefs(context);
        self.refresh(&subject, &u);

        if mobile::is_narrow(context) {
            egui::CentralPanel::default().show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.kind_picker(ui);
                    self.subject_line(ui, &subject, &mut actions);
                    self.fit_meter(ui);
                    self.save_button(ui, &subject);
                    let width = ui.available_width();
                    self.preview(ui, egui::vec2(width, f32::INFINITY));
                    ui.add_space(8.0);
                    self.controls(ui, &subject);
                });
            });
            return actions;
        }
        egui::SidePanel::left("card-controls")
            .resizable(true)
            .default_width(390.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.kind_picker(ui);
                    self.subject_line(ui, &subject, &mut actions);
                    ui.separator();
                    self.controls(ui, &subject);
                });
            });
        egui::CentralPanel::default().show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                self.save_button(ui, &subject);
                ui.separator();
                ui.selectable_value(&mut self.true_size, false, "Fit");
                ui.selectable_value(&mut self.true_size, true, "Printed size")
                    .on_hover_text("About the size it prints, on a typical screen");
            });
            self.fit_meter(ui);
            ui.add_space(4.0);
            let room = ui.available_size();
            egui::ScrollArea::both().show(ui, |ui| {
                self.preview(ui, room);
            });
        });
        actions
    }

    fn kind_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (kind, label, hint) in [
                (
                    Kind::Dope,
                    "Dope card",
                    "What to dial or hold at each range, for the rifle",
                ),
                (
                    Kind::Stand,
                    "Stand card",
                    "A map of a stand: rings with holds, limits, landmarks",
                ),
                (
                    Kind::Turret,
                    "Turret label",
                    "A strip for the elevation turret with the ranges at their clicks",
                ),
            ] {
                let selected = self.settings.kind == kind;
                let text = RichText::new(label).size(16.0);
                if ui
                    .add(
                        egui::Button::new(text)
                            .selected(selected)
                            .min_size(egui::vec2(104.0, 34.0)),
                    )
                    .on_hover_text(hint)
                    .clicked()
                {
                    self.settings.kind = kind;
                }
            }
        });
    }

    fn subject_line(&self, ui: &mut egui::Ui, subject: &Subject, actions: &mut Vec<Action>) {
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!(
                "For {} with {}{}, in today's weather, level and in calm air (Calculate also counts today's wind, angle and cant).",
                subject.rifle.name,
                subject.bullet.display_name(),
                subject
                    .target
                    .as_ref()
                    .map_or(String::new(), |target| format!(
                        ", {}",
                        target.to_lowercase()
                    ))
            ));
            if ui
                .small_button("Change…")
                .on_hover_text("Choose in Calculate")
                .clicked()
            {
                actions.push(Action::Goto(Page::Calculate));
            }
        });
    }

    fn refresh(&mut self, subject: &Subject, u: &UnitPrefs) {
        let key = (self.settings.clone(), subject.clone(), u.clone());
        if self.preview.as_ref().is_some_and(|(old, _)| *old == key) {
            return;
        }
        let preview = match self.settings.kind {
            Kind::Dope => {
                let options = &self.settings.dope;
                DopeTable::compute(subject, options, u).map(|table| {
                    let layout = dope::layout(subject, &table, options, u);
                    Preview {
                        pages: tile(&layout.sheet, options),
                        text_size: Some(layout.text_size),
                        rows: table.ranges.len(),
                        columns: table.columns.len(),
                        ethical_m: table.ethical_m,
                        notes: Vec::new(),
                        plan: None,
                        card: layout.sheet,
                    }
                })
            }
            Kind::Stand => {
                let stand = &self.settings.stands[self.settings.stand_index];
                stand::layout(subject, stand, &self.settings.stand, u).map(|sheet| Preview {
                    pages: vec![sheet.clone()],
                    card: sheet,
                    text_size: None,
                    rows: stand.landmarks.len(),
                    columns: 0,
                    ethical_m: None,
                    notes: Vec::new(),
                    plan: None,
                })
            }
            Kind::Turret => {
                let turret = self.turret_for(subject);
                let options = &self.settings.turret;
                Plan::compute(subject, &turret, options, u).map(|mut plan| {
                    let strip = turret::strip(subject, &turret, options, &mut plan, u);
                    let page = turret::page(subject, &turret, options, &plan, &strip, u);
                    let mut notes = Vec::new();
                    let crowded = plan.marks.iter().filter(|mark| mark.crowded).count();
                    if crowded > 0 {
                        notes.push(format!(
                            "{crowded} marks left off where they would overlap (they are in the \
                             table on the sheet)."
                        ));
                    }
                    if let Some(range) = plan.out_of_travel.first() {
                        notes.push(format!(
                            "From {} the scope runs out of elevation.",
                            u.format(Quantity::Distance, *range)
                        ));
                    }
                    if plan.turns > 1 {
                        notes.push(format!(
                            "{} turns of the turret: a band for each, the first at the bottom.",
                            plan.turns
                        ));
                    }
                    Preview {
                        pages: vec![page],
                        card: strip,
                        text_size: None,
                        rows: plan.marks.len(),
                        columns: 0,
                        ethical_m: plan.ethical_m,
                        notes,
                        plan: Some(plan),
                    }
                })
            }
        };
        self.preview = Some((key, preview));
    }

    fn current(&self) -> Option<&Result<Preview, String>> {
        self.preview.as_ref().map(|(_, preview)| preview)
    }

    /// How readable the dope card's text came out.
    fn fit_meter(&self, ui: &mut egui::Ui) {
        let Some(result) = self.current() else {
            return;
        };
        let preview = match result {
            Ok(preview) => preview,
            Err(error) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("No card: {error}"));
                return;
            }
        };
        for note in &preview.notes {
            ui.weak(note);
        }
        let Some(size) = preview.text_size else {
            return;
        };
        let (colour, verdict) = if size >= 6.5 {
            (theme::MEADOW, "easy to read")
        } else if size >= 5.0 {
            (theme::AMBER, "small, but readable")
        } else {
            (theme::ALARM, "too small to read")
        };
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 5.0, colour);
            ui.label(RichText::new(format!("Text {size:.1} pt: {verdict}")).color(colour));
            ui.weak(format!(
                "· {} ranges, {} columns",
                preview.rows, preview.columns
            ));
        });
        if size < 5.0 {
            ui.weak(
                "Fewer ranges (a longer step), fewer columns or temperatures, or a bigger card \
                 make the text bigger.",
            );
        }
    }

    /// The card, fitted into `room` (or at about its printed size).
    fn preview(&self, ui: &mut egui::Ui, room: egui::Vec2) {
        let Some(Ok(preview)) = self.current() else {
            return;
        };
        let card = &preview.card;
        // A point is a CSS pixel: 96 to the inch, 3.78 to the millimetre.
        let scale = if self.true_size {
            3.78
        } else {
            ((room.x - 8.0) / card.width).min((room.y - 8.0) / card.height)
        };
        let size = egui::vec2(card.width * scale, card.height * scale);
        let (rect, _) = ui.allocate_exact_size(size + egui::vec2(8.0, 8.0), egui::Sense::hover());
        let paper = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), size);
        ui.painter().rect_filled(
            paper.translate(egui::vec2(2.0, 3.0)),
            2.0,
            Color32::from_black_alpha(90),
        );
        card.paint(ui.painter(), paper);
    }

    fn save_button(&mut self, ui: &mut egui::Ui, subject: &Subject) {
        ui.horizontal_wrapped(|ui| {
            let ready = matches!(self.current(), Some(Ok(_)));
            if ui
                .add_enabled(ready, egui::Button::new(RichText::new("Save PDF").strong()))
                .on_hover_text("A PDF to print or keep on the phone")
                .clicked()
                && let Some(Ok(preview)) = self.current()
            {
                let (file, title) = match self.settings.kind {
                    Kind::Dope => (
                        format!("dope-card-{}.pdf", file_part(&subject.rifle.name)),
                        format!("Dope card, {}", subject.rifle.name),
                    ),
                    Kind::Stand => {
                        let stand = &self.settings.stands[self.settings.stand_index];
                        (
                            format!("stand-card-{}.pdf", file_part(&stand.name)),
                            format!("Stand card, {}", stand.name),
                        )
                    }
                    Kind::Turret => (
                        format!("turret-label-{}.pdf", file_part(&subject.rifle.name)),
                        format!("Turret label, {}", subject.rifle.name),
                    ),
                };
                let bytes = pdf::write(&preview.pages, &title);
                self.status = Some(save(ui.ctx(), &file, &bytes));
            }
            if let Some(Ok(preview)) = self.current()
                && self.settings.kind == Kind::Dope
                && preview
                    .pages
                    .first()
                    .is_some_and(|page| page.width != preview.card.width)
            {
                ui.weak(format!(
                    "{} cards on an A4 sheet; print at actual size (100 %)",
                    cards_on_a4(&preview.card)
                ));
            }
            if let Some((text, error)) = &self.status {
                if *error {
                    ui.colored_label(ui.visuals().error_fg_color, text);
                } else {
                    ui.weak(text);
                }
            }
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, subject: &Subject) {
        match self.settings.kind {
            Kind::Dope => self.dope_controls(ui, subject),
            Kind::Stand => self.stand_controls(ui),
            Kind::Turret => self.turret_controls(ui, subject),
        }
    }

    /// The rifle's turret as measured, or a start from its clicks.
    fn turret_for(&self, subject: &Subject) -> Turret {
        let rifle = &subject.rifle;
        self.settings
            .turrets
            .get(&rifle.id)
            .cloned()
            .unwrap_or_else(|| {
                Turret::for_clicks(
                    rifle.click_value,
                    rifle.click_unit == ballistics_store::ClickUnit::Mil,
                )
            })
    }

    fn turret_controls(&mut self, ui: &mut egui::Ui, subject: &Subject) {
        let rifle = &subject.rifle;
        let fallback = self.turret_for(subject);
        let u = units::prefs(ui.ctx());
        let ethical = self
            .current()
            .and_then(|preview| preview.as_ref().ok())
            .and_then(|preview| preview.ethical_m);
        let unit = ballistics_shot::unit_label(rifle.click_unit);
        let plan = self
            .current()
            .and_then(|preview| preview.as_ref().ok())
            .and_then(|preview| preview.plan.clone());
        let marks = plan.as_ref().map_or(Vec::new(), |plan| plan.marks.clone());
        let turns = plan.as_ref().map_or(1, |plan| plan.turns as usize);
        let turret = self
            .settings
            .turrets
            .entry(rifle.id.clone())
            .or_insert(fallback);

        ui.strong("The turret");
        ui.weak(
            "Measure the elevation turret's cap with a caliper where the label goes: over \
             the ridges if it is knurled, as the label sits on them. The height is the band \
             the label will cover. Kept for this rifle.",
        );
        grid(ui, "turret-size", |ui| {
            ui.label("Diameter");
            ui.add(
                egui::DragValue::new(&mut turret.diameter_mm)
                    .range(10.0..=80.0)
                    .speed(0.1)
                    .max_decimals(1)
                    .suffix(" mm"),
            );
            ui.end_row();
            ui.label("Height");
            ui.add(
                egui::DragValue::new(&mut turret.height_mm)
                    .range(5.0..=40.0)
                    .speed(0.1)
                    .max_decimals(1)
                    .suffix(" mm"),
            );
            ui.end_row();
            ui.label("Label");
            ui.horizontal(|ui| {
                const MATERIALS: [(&str, f64, &str); 4] = [
                    ("Paper", 0.1, "Ordinary printer paper, about 0.1 mm"),
                    (
                        "Sticker",
                        0.15,
                        "Printable vinyl or sticker paper with its glue",
                    ),
                    ("Photo paper", 0.25, "Photo paper or thin card"),
                    ("Laminated", 0.3, "Paper laminated before it is wrapped"),
                ];
                let chosen = MATERIALS
                    .iter()
                    .find(|(_, thickness, _)| (turret.thickness_mm - thickness).abs() < 1e-6)
                    .map_or("Your own", |(name, _, _)| name);
                egui::ComboBox::from_id_salt("turret-material")
                    .selected_text(chosen)
                    .show_ui(ui, |ui| {
                        for (name, thickness, hint) in MATERIALS {
                            ui.selectable_value(&mut turret.thickness_mm, thickness, name)
                                .on_hover_text(hint);
                        }
                    });
                ui.add(
                    egui::DragValue::new(&mut turret.thickness_mm)
                        .range(0.0..=1.0)
                        .speed(0.01)
                        .max_decimals(2)
                        .suffix(" mm thick"),
                )
                .on_hover_text(
                    "It wraps round its middle, so the strip is made π × thickness longer. \
                     Tape over it afterwards doesn't count.",
                );
            });
            ui.end_row();
            ui.label("Clicks a turn");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut turret.clicks_per_turn)
                        .range(10..=400)
                        .speed(1.0),
                );
                ui.weak(format!(
                    "= {:.1} {unit} a turn",
                    f64::from(turret.clicks_per_turn) * rifle.click_value
                ));
            });
            ui.end_row();
            ui.label("Dialling up turns");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut turret.clockwise, false, "anticlockwise")
                    .on_hover_text("Seen from above; most scopes");
                ui.selectable_value(&mut turret.clockwise, true, "clockwise");
            });
            ui.end_row();
        });
        ui.weak(format!(
            "The strip: {:.1} × {:.1} mm ({:.1} mm longer for the label's thickness), {} \
             clicks of {} {unit}.",
            turret.circumference_mm(),
            turret.height_mm,
            std::f64::consts::PI * turret.thickness_mm,
            turret.clicks_per_turn,
            rifle.click_value
        ));
        let per_turn = i64::from(turret.clicks_per_turn.max(1));

        let options = &mut self.settings.turret;
        ui.add_space(6.0);
        ui.strong("Ranges");
        grid(ui, "turret-ranges", |ui| {
            ui.label("From");
            ui.weak(if rifle.zero_height_cm.abs() < 0.05 {
                format!(
                    "the zero, {}",
                    u.format(Quantity::Distance, rifle.zero_range_m)
                )
            } else {
                "where the bullet comes down to the line of sight (the turret's 0)".to_owned()
            });
            ui.end_row();
            units::quantity(
                ui,
                "To",
                &mut options.end_m,
                Quantity::Distance,
                100.0..=2000.0,
                5.0,
            );
            ui.label("Numbers every");
            ui.horizontal_wrapped(|ui| {
                for every in [10.0, 25.0, 50.0, 100.0] {
                    ui.selectable_value(
                        &mut options.numbers_every,
                        every,
                        format!("{every:.0} {}", u.label(Quantity::Distance)),
                    );
                }
            });
            ui.end_row();
            ui.label("Marks between");
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut options.marks_every, 0.0, "none");
                for every in [5.0, 10.0, 25.0, 50.0] {
                    let divides = (options.numbers_every / every).fract().abs() < 1e-9;
                    if every < options.numbers_every && divides {
                        ui.selectable_value(&mut options.marks_every, every, format!("{every:.0}"));
                    }
                }
            });
            ui.end_row();
            ui.label("Numbers as");
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut options.numbers, Numbers::Hundreds, "1 2 3")
                    .on_hover_text("In hundreds: 1.1 for 110, 1.25 for 125");
                ui.selectable_value(&mut options.numbers, Numbers::Tens, "10 11 12")
                    .on_hover_text("In tens: 11 for 110");
                ui.selectable_value(&mut options.numbers, Numbers::Full, "100 110 120");
            });
            ui.end_row();
        });
        let divides = |every: f64| (options.numbers_every / every).fract().abs() < 1e-9;
        if options.marks_every >= options.numbers_every || !divides(options.marks_every.max(1.0)) {
            options.marks_every = 0.0;
        }
        ui.horizontal_wrapped(|ui| {
            ui.weak("Quick:");
            if let Some(limit) = ethical
                && ui.small_button("To the limit").clicked()
            {
                options.end_m = (limit / 25.0).floor() * 25.0;
            }
            for end in [300.0, 500.0, 800.0] {
                if ui.small_button(u.format(Quantity::Distance, end)).clicked() {
                    options.end_m = end;
                }
            }
        });
        ui.add_enabled_ui(ethical.is_some() || options.stop_at_limit, |ui| {
            ui.checkbox(&mut options.stop_at_limit, "End at the ethical limit")
                .on_hover_text("Otherwise the ranges beyond it get their own colour");
        });

        ui.add_space(6.0);
        ui.strong("Temperature");
        ui.horizontal_wrapped(|ui| {
            let today = subject.conditions.temperature_c;
            ui.selectable_value(
                &mut options.temperature,
                Temperature::Today,
                format!("Today ({})", u.format(Quantity::Temperature, today)),
            );
            match subject.zero_weather {
                Some(zero) => {
                    ui.selectable_value(
                        &mut options.temperature,
                        Temperature::Zero,
                        format!(
                            "Zeroing ({})",
                            u.format(Quantity::Temperature, zero.temperature_c)
                        ),
                    )
                    .on_hover_text("The weather the rifle (or this bullet) was zeroed in");
                }
                None => {
                    ui.add_enabled(false, egui::Button::new("Zeroing"))
                        .on_disabled_hover_text("This bullet is zeroed in the shot's own weather");
                }
            }
            let chosen = matches!(options.temperature, Temperature::Chosen(_));
            if ui.selectable_label(chosen, "Chosen").clicked() && !chosen {
                options.temperature = Temperature::Chosen(today.round());
            }
            if let Temperature::Chosen(temperature) = &mut options.temperature {
                units::drag(ui, temperature, Quantity::Temperature, -40.0..=50.0, 1.0);
            }
        });
        ui.weak("Cold air and cartridges need more clicks: a label for the season.");

        ui.add_space(6.0);
        ui.strong("Lettering");
        grid(ui, "turret-lettering", |ui| {
            ui.label("Font");
            egui::ComboBox::from_id_salt("turret-font")
                .selected_text(RichText::new(options.font.name()).family(options.font.family()))
                .show_ui(ui, |ui| {
                    for font in fonts::Font::ALL {
                        ui.selectable_value(
                            &mut options.font,
                            font,
                            RichText::new(format!("{}  123", font.name()))
                                .family(font.family())
                                .size(16.0),
                        )
                        .on_hover_text(font.hint());
                    }
                });
            ui.end_row();
            ui.label("Number size");
            ui.add(
                egui::Slider::new(&mut options.number_size, 0.35..=0.9)
                    .custom_formatter(|value, _| format!("{:.0} %", value * 100.0))
                    .custom_parser(|text| {
                        text.trim_end_matches(['%', ' '])
                            .parse::<f64>()
                            .ok()
                            .map(|value| value / 100.0)
                    }),
            )
            .on_hover_text("Of the band's height");
            ui.end_row();
            ui.label("Under the numbers");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut options.marker, Marker::None, "nothing");
                ui.selectable_value(&mut options.marker, Marker::Tick, "a tick");
                ui.selectable_value(&mut options.marker, Marker::Triangle, "a triangle");
            })
            .response
            .on_hover_text("A small mark at the number's exact click");
            ui.end_row();
            ui.label("Show");
            ui.horizontal_wrapped(|ui| {
                toggle(ui, &mut options.ruler, "click ruler");
                toggle(ui, &mut options.description, "description line");
            });
            ui.end_row();
        });

        ui.add_space(6.0);
        ui.strong("Colours");
        ui.horizontal_wrapped(|ui| {
            for (name, background, ink, beyond) in turret::SCHEMES {
                let text = RichText::new(name)
                    .color(Color32::from_rgb(ink[0], ink[1], ink[2]))
                    .background_color(Color32::from_rgb(
                        background[0],
                        background[1],
                        background[2],
                    ));
                if ui.button(text).clicked() {
                    options.background = background;
                    options.ink = ink;
                    options.beyond = beyond;
                    options.bands.clear();
                    options.description_colours = turret::Colours::default();
                    options.ruler_colours = turret::Colours::default();
                }
            }
        });
        grid(ui, "turret-colours", |ui| {
            ui.label("Label");
            ui.horizontal(|ui| {
                egui::color_picker::color_edit_button_srgb(ui, &mut options.background)
                    .on_hover_text("Background");
                egui::color_picker::color_edit_button_srgb(ui, &mut options.ink)
                    .on_hover_text("Marks and text");
                ui.weak("background, marks");
            });
            ui.end_row();
            ui.label("Beyond the limit");
            egui::color_picker::color_edit_button_srgb(ui, &mut options.beyond);
            ui.end_row();
            if options.description {
                ui.label("Description");
                part_colours(
                    ui,
                    &mut options.description_colours,
                    options.background,
                    options.ink,
                );
                ui.end_row();
            }
            for turn in 0..turns {
                if options.bands.len() <= turn {
                    options.bands.resize(turn + 1, turret::Colours::default());
                }
                ui.label(format!("Turn {}", turn + 1));
                let (background, ink) = (options.background, options.ink);
                part_colours(ui, &mut options.bands[turn], background, ink);
                ui.end_row();
            }
            if options.ruler {
                ui.label("Ruler");
                part_colours(
                    ui,
                    &mut options.ruler_colours,
                    options.background,
                    options.ink,
                );
                ui.end_row();
            }
        });
        egui::CollapsingHeader::new("Colours of single ranges")
            .id_salt("turret-range-colours")
            .show(ui, |ui| {
                ui.weak("Give a range a colour of its own, say the ones you shoot most.");
                egui::Grid::new("turret-range-colour-grid")
                    .num_columns(3)
                    .spacing([10.0, 3.0])
                    .show(ui, |ui| {
                        for mark in &marks {
                            let key = turret::range_key(mark.range_m);
                            let turn = (mark.clicks / per_turn) as usize;
                            let mut colour = options.mark_colour(mark, turn);
                            ui.label(u.format(Quantity::Distance, mark.range_m));
                            ui.weak(if mark.crowded {
                                format!("{} clicks, left off", mark.clicks)
                            } else {
                                format!("{} clicks", mark.clicks)
                            });
                            ui.horizontal(|ui| {
                                if egui::color_picker::color_edit_button_srgb(ui, &mut colour)
                                    .changed()
                                {
                                    options.range_colours.insert(key, colour);
                                }
                                if options.range_colours.contains_key(&key)
                                    && ui
                                        .small_button("↺")
                                        .on_hover_text("Back to its band's")
                                        .clicked()
                                {
                                    options.range_colours.remove(&key);
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Copies on the sheet");
            ui.add(egui::DragValue::new(&mut options.copies).range(1..=8));
        });
    }

    fn dope_controls(&mut self, ui: &mut egui::Ui, subject: &Subject) {
        let ethical = self
            .current()
            .and_then(|preview| preview.as_ref().ok())
            .and_then(|preview| preview.ethical_m);
        let today = subject.conditions.temperature_c;
        let angle = ballistics_shot::unit_label(subject.rifle.click_unit);
        let u = units::prefs(ui.ctx());
        let options = &mut self.settings.dope;

        ui.strong("Size");
        size_tiles(ui, &mut options.size, options.landscape, &CardSize::ALL);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut options.landscape, false, "Portrait");
            ui.selectable_value(&mut options.landscape, true, "Landscape");
            if options.size != CardSize::A4 {
                ui.checkbox(&mut options.on_a4, "Several on an A4 sheet")
                    .on_hover_text("With cut lines, for an ordinary printer");
            }
        });

        ui.add_space(6.0);
        ui.strong("Ranges");
        grid(ui, "card-ranges", |ui| {
            units::quantity(
                ui,
                "From",
                &mut options.start_m,
                Quantity::Distance,
                0.0..=2900.0,
                5.0,
            );
            units::quantity(
                ui,
                "To",
                &mut options.end_m,
                Quantity::Distance,
                10.0..=3000.0,
                5.0,
            );
            units::quantity(
                ui,
                "Every",
                &mut options.step_m,
                Quantity::Distance,
                5.0..=500.0,
                1.0,
            );
        });
        if options.end_m <= options.start_m {
            options.end_m = options.start_m + options.step_m;
        }
        ui.horizontal_wrapped(|ui| {
            ui.weak("Quick:");
            if let Some(limit) = ethical
                && ui
                    .small_button("To the limit")
                    .on_hover_text("From 50 m to the ethical limit, every 25 m")
                    .clicked()
            {
                options.start_m = 50.0;
                options.end_m = (limit / 25.0).floor() * 25.0;
                options.step_m = 25.0;
            }
            for (label, hint, start, end, step) in [
                ("Driven hunt", "25 to 150 m every 25 m", 25.0, 150.0, 25.0),
                ("Stand", "50 to 300 m every 50 m", 50.0, 300.0, 50.0),
                ("Long", "100 to 600 m every 50 m", 100.0, 600.0, 50.0),
            ] {
                if ui.small_button(label).on_hover_text(hint).clicked() {
                    options.start_m = start;
                    options.end_m = end;
                    options.step_m = step;
                }
            }
        });
        ui.add_enabled_ui(ethical.is_some() || options.stop_at_limit, |ui| {
            ui.checkbox(
                &mut options.stop_at_limit,
                match ethical {
                    Some(limit) => format!(
                        "End at the ethical limit ({})",
                        u.format(Quantity::Distance, limit)
                    ),
                    None => "End at the ethical limit".to_owned(),
                },
            )
            .on_hover_text("Otherwise the ranges beyond it are greyed below a red line");
        });

        ui.add_space(6.0);
        ui.strong("Temperatures");
        ui.weak("An elevation column for each; the cartridges at the same temperature.");
        let mut temperatures = options.temperatures(today);
        if chips(
            ui,
            "card-temperatures",
            &mut temperatures,
            Quantity::Temperature,
            -40.0..=50.0,
            1.0,
            |last| last - 10.0,
        ) {
            options.temperatures = temperatures;
        }
        ui.horizontal_wrapped(|ui| {
            ui.weak("Quick:");
            if ui
                .small_button(format!(
                    "Today ({})",
                    u.format(Quantity::Temperature, today)
                ))
                .clicked()
            {
                options.temperatures.clear();
            }
            if ui
                .small_button("Autumn")
                .on_hover_text("−5, +5 and +15 °C")
                .clicked()
            {
                options.temperatures = vec![-5.0, 5.0, 15.0];
            }
            if ui
                .small_button("Winter")
                .on_hover_text("−20, −10 and 0 °C")
                .clicked()
            {
                options.temperatures = vec![-20.0, -10.0, 0.0];
            }
            if ui
                .small_button("All year")
                .on_hover_text("−20, 0 and +20 °C")
                .clicked()
            {
                options.temperatures = vec![-20.0, 0.0, 20.0];
            }
        });

        ui.add_space(6.0);
        ui.strong("Elevation as");
        ui.horizontal_wrapped(|ui| {
            toggle(ui, &mut options.clicks, "clicks");
            toggle(ui, &mut options.angle, angle);
            toggle(
                ui,
                &mut options.drop,
                &format!("{} (dead-on hold)", u.label(Quantity::ShortLength)),
            );
        });
        if !(options.clicks || options.angle || options.drop) {
            options.clicks = true;
        }

        ui.add_space(6.0);
        ui.strong("Wind");
        ui.weak("Full value, from 3 or 9 o'clock: a column for each speed.");
        chips(
            ui,
            "card-winds",
            &mut options.winds,
            Quantity::WindSpeed,
            0.5..=30.0,
            0.5,
            |last| last + 2.0,
        );
        ui.horizontal_wrapped(|ui| {
            ui.weak("Quick:");
            if ui.small_button("None").clicked() {
                options.winds.clear();
            }
            if ui
                .small_button("Per 1 m/s")
                .on_hover_text("Multiply by the wind")
                .clicked()
            {
                options.winds = vec![1.0];
            }
            if ui.small_button("2, 4, 6 m/s").clicked() {
                options.winds = vec![2.0, 4.0, 6.0];
            }
        });
        if !options.winds.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("as");
                ui.selectable_value(&mut options.wind_as, Measure::Clicks, "clicks");
                ui.selectable_value(&mut options.wind_as, Measure::Angle, angle);
                ui.selectable_value(
                    &mut options.wind_as,
                    Measure::Length,
                    u.label(Quantity::ShortLength),
                );
            });
        }

        ui.add_space(6.0);
        ui.strong("Also");
        ui.horizontal_wrapped(|ui| {
            toggle(ui, &mut options.velocity, "speed");
            toggle(ui, &mut options.energy, "energy");
            toggle(ui, &mut options.time, "time of flight");
        });
        ui.checkbox(&mut options.legend, "Explain the marks at the foot");
        ui.horizontal(|ui| {
            ui.label("Note");
            ui.add(
                egui::TextEdit::singleline(&mut options.note)
                    .hint_text("e.g. Moose hunt 2026, tower 4")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    fn stand_controls(&mut self, ui: &mut egui::Ui) {
        let settings = &mut self.settings;
        let mut chosen = settings.stand_index;
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("card-stand")
                .selected_text(&settings.stands[chosen].name)
                .show_ui(ui, |ui| {
                    for (index, stand) in settings.stands.iter().enumerate() {
                        ui.selectable_value(&mut chosen, index, &stand.name);
                    }
                });
            if ui.button("New stand").clicked() {
                settings.stands.push(Stand {
                    name: format!("Stand {}", settings.stands.len() + 1),
                    ..Stand::default()
                });
                chosen = settings.stands.len() - 1;
            }
            if settings.stands.len() > 1 && confirm_button(ui, "delete-stand", "Delete") {
                settings.stands.remove(chosen);
                chosen = chosen.saturating_sub(1);
            }
        });
        settings.stand_index = chosen.min(settings.stands.len() - 1);
        ui.weak("Stands are kept on this device.");
        let stand = &mut settings.stands[settings.stand_index];
        let compass = crate::sensors::has_compass();
        let mut wanted = None;

        grid(ui, "stand-main", |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut stand.name);
            ui.end_row();
            ui.label("Faces");
            ui.horizontal(|ui| {
                bearing(ui, &mut stand.facing_degrees);
                if compass && compass_button(ui) {
                    wanted = Some(Heading::Facing);
                }
            });
            ui.end_row();
            ui.label("Map");
            ui.horizontal(|ui| {
                for view in [View::Quarter, View::Half, View::Full] {
                    ui.selectable_value(&mut stand.view, view, view.name());
                }
            });
            ui.end_row();
            ui.label("Note");
            ui.add(
                egui::TextEdit::singleline(&mut stand.note)
                    .hint_text("e.g. Shoot only towards the ridge")
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();
        });

        ui.add_space(6.0);
        let mut has_sector = stand.sector.is_some();
        if ui
            .checkbox(&mut has_sector, "Shooting sector")
            .on_hover_text("The edges the hunt leader gave; outside is greyed")
            .changed()
        {
            stand.sector = has_sector.then(|| {
                [stand.facing_degrees - 45.0, stand.facing_degrees + 45.0]
                    .map(|degrees: f64| degrees.rem_euclid(360.0))
            });
        }
        if let Some([left, right]) = &mut stand.sector {
            grid(ui, "stand-sector", |ui| {
                ui.label("Left edge");
                ui.horizontal(|ui| {
                    bearing(ui, left);
                    if compass && compass_button(ui) {
                        wanted = Some(Heading::SectorLeft);
                    }
                });
                ui.end_row();
                ui.label("Right edge");
                ui.horizontal(|ui| {
                    bearing(ui, right);
                    if compass && compass_button(ui) {
                        wanted = Some(Heading::SectorRight);
                    }
                });
                ui.end_row();
            });
        }

        ui.add_space(6.0);
        ui.strong("Landmarks");
        ui.weak("What you see from the stand, with its range (a rangefinder) and direction.");
        let mut remove = None;
        egui::Grid::new("stand-landmarks")
            .num_columns(4)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                for (index, landmark) in stand.landmarks.iter_mut().enumerate() {
                    ui.add_sized(
                        [110.0, 20.0],
                        egui::TextEdit::singleline(&mut landmark.name).hint_text("e.g. Big rock"),
                    );
                    units::drag(
                        ui,
                        &mut landmark.range_m,
                        Quantity::Distance,
                        5.0..=1500.0,
                        1.0,
                    );
                    ui.horizontal(|ui| {
                        bearing(ui, &mut landmark.bearing_degrees);
                        if compass && compass_button(ui) {
                            wanted = Some(Heading::Landmark(index));
                        }
                    });
                    if ui.small_button("✖").on_hover_text("Remove").clicked() {
                        remove = Some(index);
                    }
                    ui.end_row();
                }
            });
        if let Some(index) = remove {
            stand.landmarks.remove(index);
        }
        if ui.button("+ Landmark").clicked() {
            let previous = stand.landmarks.last();
            stand.landmarks.push(Landmark {
                name: String::new(),
                range_m: previous.map_or(100.0, |landmark| landmark.range_m + 50.0),
                bearing_degrees: previous.map_or(stand.facing_degrees, |landmark| {
                    (landmark.bearing_degrees + 20.0).rem_euclid(360.0)
                }),
            });
        }

        ui.add_space(6.0);
        ui.strong("Never shoot towards");
        ui.weak("Roads, houses, the next stand: clockwise from one direction to the other.");
        let mut remove = None;
        egui::Grid::new("stand-no-shoot")
            .num_columns(4)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                for (index, no) in stand.no_shoot.iter_mut().enumerate() {
                    ui.add_sized(
                        [110.0, 20.0],
                        egui::TextEdit::singleline(&mut no.name).hint_text("e.g. Road"),
                    );
                    bearing(ui, &mut no.from_degrees);
                    bearing(ui, &mut no.to_degrees);
                    if ui.small_button("✖").on_hover_text("Remove").clicked() {
                        remove = Some(index);
                    }
                    ui.end_row();
                }
            });
        if let Some(index) = remove {
            stand.no_shoot.remove(index);
        }
        if ui.button("+ Direction").clicked() {
            stand.no_shoot.push(NoShoot {
                name: String::new(),
                from_degrees: (stand.facing_degrees + 60.0).rem_euclid(360.0),
                to_degrees: (stand.facing_degrees + 80.0).rem_euclid(360.0),
            });
        }

        ui.add_space(6.0);
        let u = units::prefs(ui.ctx());
        let options = &mut settings.stand;
        ui.strong("Rings");
        ui.horizontal_wrapped(|ui| {
            ui.label("Every");
            ui.selectable_value(&mut options.ring_step_m, None, "auto");
            for step in [25.0, 50.0, 100.0] {
                ui.selectable_value(
                    &mut options.ring_step_m,
                    Some(step),
                    u.format(Quantity::Distance, step),
                );
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut fixed = options.max_range_m.is_some();
            if ui.checkbox(&mut fixed, "Out to").changed() {
                options.max_range_m = fixed.then_some(300.0);
            }
            match &mut options.max_range_m {
                Some(range) => {
                    units::drag(ui, range, Quantity::Distance, 50.0..=2000.0, 5.0);
                }
                None => {
                    ui.weak("past the landmarks and the limit");
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Hold as");
            ui.selectable_value(&mut options.hold_as, Measure::Clicks, "clicks");
            ui.selectable_value(&mut options.hold_as, Measure::Angle, "mil or MOA");
            ui.selectable_value(
                &mut options.hold_as,
                Measure::Length,
                u.label(Quantity::ShortLength),
            );
        });
        ui.add_space(6.0);
        ui.strong("Size");
        size_tiles(
            ui,
            &mut options.size,
            options.landscape,
            &[CardSize::A6, CardSize::A5, CardSize::A4],
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut options.landscape, false, "Portrait");
            ui.selectable_value(&mut options.landscape, true, "Landscape");
        });
        if options.size == CardSize::Card || options.size == CardSize::A7 {
            options.size = CardSize::A6;
        }

        if let Some(wanted) = wanted {
            crate::sensors::start_compass(ui.ctx().clone());
            self.heading = Some(wanted);
        }
        if self.heading.is_some() {
            ui.weak("🧭 Hold the phone flat, its top edge towards the place…");
            ui.ctx().request_repaint();
        }
    }

    /// Fills the field waiting for the compass, once it answers.
    fn take_heading(&mut self, context: &egui::Context) {
        let Some(wanted) = self.heading else {
            return;
        };
        let Some(heading) = crate::sensors::heading() else {
            context.request_repaint();
            return;
        };
        let heading = heading.round().rem_euclid(360.0);
        let index = self
            .settings
            .stand_index
            .min(self.settings.stands.len().saturating_sub(1));
        if let Some(stand) = self.settings.stands.get_mut(index) {
            match wanted {
                Heading::Facing => stand.facing_degrees = heading,
                Heading::SectorLeft => {
                    if let Some(sector) = &mut stand.sector {
                        sector[0] = heading;
                    }
                }
                Heading::SectorRight => {
                    if let Some(sector) = &mut stand.sector {
                        sector[1] = heading;
                    }
                }
                Heading::Landmark(landmark) => {
                    if let Some(landmark) = stand.landmarks.get_mut(landmark) {
                        landmark.bearing_degrees = heading;
                    }
                }
            }
        }
        self.heading = None;
    }
}

/// The card's sizes as little sheets drawn to scale, to pick one.
fn size_tiles(ui: &mut egui::Ui, size: &mut CardSize, landscape: bool, sizes: &[CardSize]) {
    ui.horizontal_wrapped(|ui| {
        for &option in sizes {
            let [width, height] = option.oriented(landscape);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(58.0, 64.0), egui::Sense::click());
            // Square root scaling keeps the bank card visible beside A4.
            let scale = |mm: f32| (mm / 297.0).sqrt() * 40.0;
            let paper = egui::Rect::from_center_size(
                rect.center() - egui::vec2(0.0, 7.0),
                egui::vec2(scale(width), scale(height)),
            );
            let selected = *size == option;
            let painter = ui.painter();
            if response.hovered() {
                painter.rect_filled(rect, 4.0, theme::FOREST_700);
            }
            painter.rect_filled(paper, 1.5, theme::BONE);
            for line in 1..4 {
                let y = paper.top() + paper.height() * line as f32 / 4.5;
                painter.line_segment(
                    [
                        egui::pos2(paper.left() + 3.0, y),
                        egui::pos2(paper.right() - 3.0, y),
                    ],
                    egui::Stroke::new(1.0_f32, theme::BONE_MUTED),
                );
            }
            if selected {
                painter.rect_stroke(
                    paper.expand(2.0),
                    2.0,
                    egui::Stroke::new(2.0_f32, theme::BLAZE),
                    egui::StrokeKind::Outside,
                );
            }
            painter.text(
                egui::pos2(rect.center().x, rect.bottom() - 6.0),
                egui::Align2::CENTER_CENTER,
                option.name(),
                egui::FontId::proportional(12.0),
                if selected { theme::BLAZE } else { theme::BONE },
            );
            if response.on_hover_text(option.hint()).clicked() {
                *size = option;
            }
        }
    });
}

/// A row of values to add to and remove from. Returns whether they changed.
#[allow(clippy::too_many_arguments)]
fn chips(
    ui: &mut egui::Ui,
    id: &str,
    values: &mut Vec<f64>,
    quantity: Quantity,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
    next: impl Fn(f64) -> f64,
) -> bool {
    let before = values.clone();
    let mut remove = None;
    ui.push_id(id, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (index, value) in values.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(theme::FOREST_700)
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            units::drag(ui, value, quantity, range.clone(), speed);
                            if ui.small_button("✖").on_hover_text("Remove").clicked() {
                                remove = Some(index);
                            }
                        });
                    });
            }
            if values.len() < 6 && ui.button("+").on_hover_text("Add one").clicked() {
                let value = values
                    .last()
                    .map_or(range.start().max(0.0), |last| next(*last));
                values.push(value.clamp(*range.start(), *range.end()));
            }
        });
    });
    if let Some(index) = remove {
        values.remove(index);
    }
    *values != before
}

/// A part's own background and marks, or the label's; ↺ goes back.
fn part_colours(
    ui: &mut egui::Ui,
    colours: &mut turret::Colours,
    background: turret::Rgb,
    ink: turret::Rgb,
) {
    ui.horizontal(|ui| {
        for (value, fallback, hint) in [
            (&mut colours.background, background, "Background"),
            (&mut colours.ink, ink, "Marks and text"),
        ] {
            let mut colour = value.unwrap_or(fallback);
            if egui::color_picker::color_edit_button_srgb(ui, &mut colour)
                .on_hover_text(hint)
                .changed()
            {
                *value = Some(colour);
            }
        }
        if (colours.background.is_some() || colours.ink.is_some())
            && ui
                .small_button("↺")
                .on_hover_text("The label's colours")
                .clicked()
        {
            *colours = turret::Colours::default();
        }
    });
}

fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) {
    if ui.selectable_label(*value, label).clicked() {
        *value = !*value;
    }
}

/// A compass direction in degrees, with its compass point.
fn bearing(ui: &mut egui::Ui, degrees: &mut f64) {
    let mut shown = degrees.rem_euclid(360.0);
    if ui
        .add(
            egui::DragValue::new(&mut shown)
                .range(0.0..=359.0)
                .speed(1.0)
                .max_decimals(0)
                .suffix("°"),
        )
        .changed()
    {
        *degrees = shown;
    }
    ui.weak(crate::weather::compass_point(*degrees));
}

fn compass_button(ui: &mut egui::Ui) -> bool {
    ui.small_button("🧭")
        .on_hover_text("Point the phone's top edge that way")
        .clicked()
}

/// How many cards of this size go on an A4 sheet.
fn cards_on_a4(card: &Sheet) -> usize {
    let (columns, rows) = a4_grid(card);
    columns * rows
}

const A4: [f32; 2] = [210.0, 297.0];
const A4_MARGIN: f32 = 10.0;
const GAP: f32 = 4.0;

fn a4_grid(card: &Sheet) -> (usize, usize) {
    let fit = |room: f32, size: f32| {
        (((room - 2.0 * A4_MARGIN + GAP) / (size + GAP)).floor() as usize).max(1)
    };
    (fit(A4[0], card.width), fit(A4[1], card.height))
}

/// The pages to print: the card alone, or as many as fit on A4.
fn tile(card: &Sheet, options: &DopeOptions) -> Vec<Sheet> {
    if !options.on_a4 || options.size == CardSize::A4 {
        return vec![card.clone()];
    }
    let (columns, rows) = a4_grid(card);
    let mut page = Sheet::new(A4[0], A4[1]);
    let used = [
        columns as f32 * card.width + (columns - 1) as f32 * GAP,
        rows as f32 * card.height + (rows - 1) as f32 * GAP,
    ];
    let left = (A4[0] - used[0]) / 2.0;
    let top = (A4[1] - used[1]) / 2.0;
    for row in 0..rows {
        for column in 0..columns {
            page.place(
                card,
                [
                    left + column as f32 * (card.width + GAP),
                    top + row as f32 * (card.height + GAP),
                ],
            );
        }
    }
    page.text(
        [A4[0] / 2.0, A4[1] - 4.0],
        "Print at actual size (100 %). Cut along the thin grey frames.",
        7.0,
        false,
        sheet::Align::Center,
        sheet::Ink::Grey,
    );
    vec![page]
}

/// A file name part from a name: letters, digits and dashes.
fn file_part(name: &str) -> String {
    let part: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let part = part
        .split('-')
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if part.is_empty() { "card".into() } else { part }
}

/// Saves the PDF: a save dialog and the system's viewer on the desktop, a
/// download in the browser. Returns the outcome to show.
fn save(context: &egui::Context, file: &str, bytes: &[u8]) -> (String, bool) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(file)
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return ("Not saved".into(), false);
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                crate::http::open_in_browser(context, &path.to_string_lossy());
                (format!("Saved {}", path.display()), false)
            }
            Err(error) => (format!("Could not save {}: {error}", path.display()), true),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = context;
        match crate::web::download_bytes(file, bytes, "application/pdf") {
            Ok(()) => (format!("Saved {file} in your downloads"), false),
            Err(error) => (error, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_tidy() {
        assert_eq!(file_part("Browning X-Bolt .308"), "browning-x-bolt-308");
        assert_eq!(file_part("Tornet vid ån"), "tornet-vid-ån");
        assert_eq!(file_part("…"), "card");
    }

    #[test]
    fn bank_cards_fill_an_a4_sheet() {
        let card = Sheet::new(85.6, 53.98);
        assert_eq!(a4_grid(&card), (2, 4));
        let options = DopeOptions::default();
        let pages = tile(&card, &options);
        assert_eq!((pages[0].width, pages[0].height), (210.0, 297.0));
    }

    /// The demo's first rifle with a bullet for it, shooting at a moose.
    fn demo_subject() -> Subject {
        let mut store = ballistics_store::Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        let rifle = store.rifles().unwrap().remove(0);
        let bullet = store
            .bullets()
            .unwrap()
            .into_iter()
            .find(|bullet| bullet.caliber == rifle.caliber)
            .unwrap();
        Subject {
            rifle,
            bullet,
            load: None,
            target: Some("Moose".into()),
            vital_cm: Some(30.0),
            min_energy_j: Some(2000.0),
            min_velocity_mps: 550.0,
            conditions: ballistics_shot::Conditions::default(),
            elevation_travel_clicks: None,
            zero_weather: None,
        }
    }

    /// Both cards as PDFs; with MXSC_CARDS_OUT set, written to that folder
    /// to look at.
    #[test]
    fn cards_make_pdfs() {
        let subject = demo_subject();
        let u = UnitPrefs::metric();
        let mut options = DopeOptions {
            temperatures: vec![-10.0, 10.0],
            winds: vec![2.0, 4.0],
            end_m: 400.0,
            ..DopeOptions::default()
        };
        let table = DopeTable::compute(&subject, &options, &u).unwrap();
        assert_eq!(table.ranges.len(), 8);
        assert!(table.ethical_m.is_some_and(|limit| limit > 150.0));
        let card = dope::layout(&subject, &table, &options, &u);
        assert!(card.text_size > 4.0, "{}", card.text_size);
        let sheet_pdf = pdf::write(&tile(&card.sheet, &options), "Dope card");
        options.on_a4 = false;
        options.size = CardSize::A6;
        options.landscape = false;
        let big = dope::layout(&subject, &table, &options, &u);
        assert!(big.text_size > card.text_size);
        let a6_pdf = pdf::write(&tile(&big.sheet, &options), "Dope card");

        let stand = Stand {
            name: "Tower by the bog".into(),
            facing_degrees: 200.0,
            sector: Some([160.0, 250.0]),
            landmarks: vec![
                Landmark {
                    name: "Big rock".into(),
                    range_m: 120.0,
                    bearing_degrees: 185.0,
                },
                Landmark {
                    name: "Ditch".into(),
                    range_m: 240.0,
                    bearing_degrees: 225.0,
                },
            ],
            no_shoot: vec![NoShoot {
                name: "Road".into(),
                from_degrees: 255.0,
                to_degrees: 280.0,
            }],
            ..Stand::default()
        };
        let mut stand_pdfs = Vec::new();
        for view in [View::Quarter, View::Half, View::Full] {
            let stand = Stand {
                view,
                ..stand.clone()
            };
            let sheet = stand::layout(&subject, &stand, &StandOptions::default(), &u).unwrap();
            stand_pdfs.push(pdf::write(&[sheet], "Stand card"));
        }
        let turret = Turret::default();
        let options = TurretOptions::default();
        let mut plan = Plan::compute(&subject, &turret, &options, &u).unwrap();
        // The zero range sits at click 0 and the clicks grow with range.
        assert_eq!(plan.marks[0].clicks, 0);
        assert_eq!(plan.marks[0].text, "1");
        assert!(
            plan.marks
                .windows(2)
                .all(|pair| pair[0].clicks <= pair[1].clicks)
        );
        assert!(plan.marks.iter().any(|mark| mark.beyond));
        let strip = turret::strip(&subject, &turret, &options, &mut plan, &u);
        assert!((strip.width - turret.circumference_mm()).abs() < 1e-3);
        let turret_pdf = pdf::write(
            &[turret::page(&subject, &turret, &options, &plan, &strip, &u)],
            "Turret label",
        );
        let (_, background, ink, beyond) = turret::SCHEMES[1];
        let mut dark = TurretOptions {
            background,
            ink,
            beyond,
            numbers: Numbers::Tens,
            numbers_every: 50.0,
            marks_every: 10.0,
            end_m: 900.0,
            font: fonts::Font::BarlowCondensedBold,
            marker: Marker::Triangle,
            temperature: Temperature::Chosen(-15.0),
            ..options
        };
        dark.bands = vec![
            turret::Colours::default(),
            turret::Colours {
                background: Some([255, 212, 0]),
                ink: Some([20, 20, 20]),
            },
        ];
        dark.range_colours.insert(300, [80, 200, 255]);
        let mut dark_plan = Plan::compute(&subject, &turret, &dark, &u).unwrap();
        assert!(dark_plan.turns >= 2);
        let dark_strip = turret::strip(&subject, &turret, &dark, &mut dark_plan, &u);
        let dark_pdf = pdf::write(
            &[turret::page(
                &subject,
                &turret,
                &dark,
                &dark_plan,
                &dark_strip,
                &u,
            )],
            "Turret label",
        );
        if let Ok(folder) = std::env::var("MXSC_CARDS_OUT") {
            let folder = std::path::Path::new(&folder);
            std::fs::write(folder.join("turret.pdf"), &turret_pdf).unwrap();
            std::fs::write(folder.join("turret-dark.pdf"), &dark_pdf).unwrap();
            std::fs::write(folder.join("dope-a4.pdf"), &sheet_pdf).unwrap();
            std::fs::write(folder.join("dope-a6.pdf"), &a6_pdf).unwrap();
            for (name, bytes) in ["quarter", "half", "full"].iter().zip(&stand_pdfs) {
                std::fs::write(folder.join(format!("stand-{name}.pdf")), bytes).unwrap();
            }
        }
    }

    #[test]
    fn today_is_a_date() {
        let date = today();
        assert_eq!(date.len(), 10);
        assert!(date.starts_with("20"));
    }
}

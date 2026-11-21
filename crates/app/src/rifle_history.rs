//! A rifle's history on its page: the round counts (action, barrel, since
//! clean), logging rounds and cleans, the barrels, the recent round log and
//! the precision measured from the groups, per barrel and per load.

use ballistics_store::model::now_ms;
use ballistics_store::{Barrel, Clean, Group, Rifle, RoundCounts, RoundLogEntry};
use eframe::egui::{self, RichText};
use groups_core::{Composite, CompositeMode, ShotGroup};

use crate::app::{Action, Data};
use crate::i18n::{tr, trf};
use crate::mobile;
use crate::number_field::NumberField;
use crate::records::Change;
use crate::theme;
use crate::units::{self, Quantity, UnitPrefs, optional_quantity};
use crate::widgets::{confirm_button, grid, text};

/// "Clean after" when the hunter hasn't set it for the rifle.
pub const CLEAN_AFTER_ROUNDS: u32 = 200;
/// How many round log entries the page lists.
const RECENT_ENTRIES: usize = 25;
const DAY_MS: i64 = 86_400_000;

/// What the history part of the rifle page has open.
#[derive(Default)]
pub struct HistoryState {
    /// The rifle the state is for; another rifle starts afresh.
    rifle_id: Option<String>,
    /// The Log rounds sheet, when open.
    log: Option<LogDraft>,
    /// A barrel being added or edited.
    barrel: Option<Barrel>,
    /// The barrel whose deletion is being confirmed.
    deleting: Option<String>,
    /// Precision worked out from the groups, kept until they change.
    precision: Option<(PrecisionKey, Vec<Found>)>,
}

struct LogDraft {
    rounds: u32,
    /// `None`: through the action only.
    barrel_id: Option<String>,
    note: String,
}

/// A barrel's or a bullet's id with its precision, if any was measured.
type Found = (String, Option<Precision>);

/// What the precision was worked out from.
#[derive(Clone, PartialEq)]
struct PrecisionKey {
    rifle_id: String,
    groups: usize,
    changed: i64,
    photos: usize,
}

/// σ pooled about the groups' own centres, mrad, and from how many groups.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Precision {
    pub sigma_mrad: f64,
    pub groups: usize,
}

/// One of the round count tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub title: &'static str,
    pub value: String,
    pub detail: String,
    /// A clean is due: shown in amber.
    pub due: bool,
}

/// The rifle's fitted barrel, if it has its record.
pub fn fitted<'a>(rifle: &Rifle, data: &'a Data) -> Option<&'a Barrel> {
    let id = rifle.fitted_barrel_id.as_deref()?;
    data.groups.barrels.iter().find(|barrel| barrel.id == id)
}

fn barrels_of<'a>(rifle: &Rifle, data: &'a Data) -> Vec<&'a Barrel> {
    data.groups
        .barrels
        .iter()
        .filter(|barrel| barrel.rifle_id == rifle.id)
        .collect()
}

/// When to clean the rifle: the hunter's choice, else every 200 rounds.
pub fn clean_after(rifle: &Rifle, data: &Data) -> u32 {
    data.clean_after
        .get(&rifle.id)
        .copied()
        .unwrap_or(CLEAN_AFTER_ROUNDS)
}

fn day(ms: i64, data: &Data) -> String {
    let today = crate::groups::local_ms(now_ms(), data.country).div_euclid(DAY_MS);
    crate::groups::library::day_name(
        crate::groups::local_ms(ms, data.country).div_euclid(DAY_MS),
        today,
    )
}

fn count(rounds: u64) -> String {
    let digits = rounds.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('\u{202f}');
        }
        grouped.push(digit);
    }
    grouped
}

/// The tiles for Action, Barrel and Since clean. `barrel` is the fitted
/// barrel's name, when it has one; `last_clean` how the last clean reads.
pub fn tiles(
    counts: &RoundCounts,
    barrel: Option<&str>,
    clean_after: u32,
    last_clean: Option<String>,
) -> [Tile; 3] {
    let since = counts.since_clean;
    [
        Tile {
            title: tr("Action"),
            value: count(counts.action),
            detail: tr("every round fired").to_owned(),
            due: false,
        },
        Tile {
            title: tr("Barrel"),
            value: counts.barrel.map_or_else(|| "—".to_owned(), count),
            detail: barrel.map_or_else(|| tr("no barrel fitted").to_owned(), str::to_owned),
            due: false,
        },
        Tile {
            title: tr("Since clean"),
            value: since.map_or_else(|| "—".to_owned(), count),
            detail: match (since, last_clean) {
                (Some(rounds), _) if rounds >= u64::from(clean_after) => {
                    trf!("time to clean (every {})", clean_after)
                }
                (Some(_), Some(when)) => trf!("cleaned {}", when),
                _ => tr("no clean logged").to_owned(),
            },
            due: since.is_some_and(|rounds| rounds >= u64::from(clean_after)),
        },
    ]
}

/// The precision of `groups` pooled about their own centres, as angles;
/// groups without a distance, a scale or two counted shots are left out.
pub fn pooled<'a>(groups: impl Iterator<Item = &'a Group>, data: &Data) -> Option<Precision> {
    let shot_groups: Vec<ShotGroup> = groups
        .filter(|group| group.ladder_id.is_none())
        .filter_map(|group| {
            let distance_m = group.distance_m?;
            let measured = crate::groups::measure_group(group, data).ok()?;
            let shots: Vec<_> = measured
                .counted
                .iter()
                .map(|&index| measured.shots_mm[index])
                .collect();
            (shots.len() >= 2).then_some(ShotGroup {
                shots,
                aim: None,
                distance_m,
            })
        })
        .collect();
    if shot_groups.is_empty() {
        return None;
    }
    let composite = Composite::with_mode(&shot_groups, CompositeMode::AboutOwnCentres).ok()?;
    Some(Precision {
        sigma_mrad: composite.sigma.value,
        groups: composite.groups - composite.left_out.len(),
    })
}

fn precision_text(u: &UnitPrefs, precision: Option<Precision>) -> String {
    match precision {
        Some(precision) => trf!(
            "σ {} · {} groups",
            u.format(Quantity::GroupAngle, precision.sigma_mrad),
            precision.groups
        ),
        None => tr("no measured groups yet").to_owned(),
    }
}

fn length_and_twist(u: &UnitPrefs, length_mm: Option<f64>, twist_in: Option<f64>) -> String {
    let short = |quantity: Quantity, value: f64| {
        let number = format!("{:.2}", u.show(quantity, value));
        let number = number.trim_end_matches('0').trim_end_matches('.');
        format!("{number} {}", u.label(quantity))
    };
    let mut parts = Vec::new();
    if let Some(mm) = length_mm {
        parts.push(short(Quantity::BarrelLength, mm));
    }
    if let Some(twist) = twist_in {
        parts.push(format!("1:{}", short(Quantity::Twist, twist)));
    }
    parts.join(" · ")
}

/// Whether the barrel's length or twist differ from the rifle's
/// ballistics.
pub fn differs_from_rifle(barrel: &Barrel, rifle: &Rifle) -> bool {
    let length = barrel.length_mm.is_some_and(|mm| {
        rifle
            .barrel_length_mm
            .is_none_or(|own| (own - mm).abs() > 0.5)
    });
    let twist = barrel.twist_in.is_some_and(|twist| {
        (twist - rifle.twist_in).abs() > 0.005 || barrel.right_hand_twist != rifle.right_hand_twist
    });
    length || twist
}

impl HistoryState {
    fn for_rifle(&mut self, rifle: &Rifle) {
        if self.rifle_id.as_deref() != Some(rifle.id.as_str()) {
            *self = HistoryState {
                rifle_id: Some(rifle.id.clone()),
                ..HistoryState::default()
            };
        }
    }

    /// σ for each barrel (by id) and each load (by bullet id), worked out
    /// again only when the groups change.
    fn precision(&mut self, rifle: &Rifle, data: &Data) -> &[(String, Option<Precision>)] {
        let key = PrecisionKey {
            rifle_id: rifle.id.clone(),
            groups: data.groups.groups.len(),
            changed: data
                .groups
                .groups
                .iter()
                .map(|group| group.updated_at)
                .max()
                .unwrap_or_default(),
            photos: data.groups.photos.len(),
        };
        if self.precision.as_ref().is_none_or(|(kept, _)| *kept != key) {
            let rifle_groups: Vec<&Group> = data
                .groups
                .groups
                .iter()
                .filter(|group| group.rifle_id.as_deref() == Some(rifle.id.as_str()))
                .collect();
            let mut found = Vec::new();
            for barrel in barrels_of(rifle, data) {
                let groups = rifle_groups
                    .iter()
                    .copied()
                    .filter(|group| group.barrel_id.as_deref() == Some(barrel.id.as_str()));
                found.push((barrel.id.clone(), pooled(groups, data)));
            }
            for load in data.loads.iter().filter(|load| load.rifle_id == rifle.id) {
                let groups = rifle_groups
                    .iter()
                    .copied()
                    .filter(|group| group.bullet_id.as_deref() == Some(load.bullet_id.as_str()));
                found.push((load.bullet_id.clone(), pooled(groups, data)));
            }
            for bullet_id in rifle_groups
                .iter()
                .filter_map(|group| group.bullet_id.clone())
            {
                if !found.iter().any(|(id, _)| *id == bullet_id) {
                    let groups = rifle_groups
                        .iter()
                        .copied()
                        .filter(|group| group.bullet_id.as_deref() == Some(bullet_id.as_str()));
                    let precision = pooled(groups, data);
                    found.push((bullet_id, precision));
                }
            }
            self.precision = Some((key, found));
        }
        self.precision.as_ref().map_or(&[], |(_, found)| found)
    }
}

/// The round counts, barrels, precision and recent rounds of a saved
/// rifle.
pub fn section(
    ui: &mut egui::Ui,
    rifle: &Rifle,
    data: &Data,
    state: &mut HistoryState,
    actions: &mut Vec<Action>,
) {
    state.for_rifle(rifle);
    let u = units::prefs(ui.ctx());
    let fitted = fitted(rifle, data);
    let counts = RoundCounts::derive(rifle, fitted, &data.groups.round_log, &data.groups.cleans);
    let every = clean_after(rifle, data);
    let last_clean = counts.last_clean_at.map(|at| day(at, data));
    let tiles = tiles(&counts, fitted.map(|b| b.name.as_str()), every, last_clean);
    ui.heading(tr("Round counts"));
    tile_row(ui, &tiles);
    ui.horizontal_wrapped(|ui| {
        if ui.button(tr("＋ Log rounds")).clicked() {
            state.log = Some(LogDraft {
                rounds: 5,
                barrel_id: fitted.map(|barrel| barrel.id.clone()),
                note: String::new(),
            });
        }
        if confirm_button(ui, "rifle-cleaned", tr("Cleaned")) {
            let mut clean = Clean::new(&rifle.id, fitted.map(|b| b.id.as_str()), now_ms());
            clean.note = fitted.map(|b| b.name.clone()).unwrap_or_default();
            actions.push(Action::Record(Box::new(Change::MarkCleaned(clean))));
        }
        ui.label(tr("Clean after"));
        let mut after = every;
        let field = ui.add(
            NumberField::new(&mut after)
                .range(10..=5000)
                .speed(5)
                .suffix(format!(" {}", tr("rounds"))),
        );
        if field.changed() && after != every {
            actions.push(Action::SetCleanAfter(rifle.id.clone(), after));
        }
        field.on_hover_text(tr("Kept on this device"));
    });
    if !barrels_of(rifle, data).is_empty() && fitted.is_none() {
        ui.weak(tr(
            "No barrel is fitted: fit one below so its rounds and cleans count.",
        ));
    }
    log_sheet(ui.ctx(), rifle, data, state, actions);

    ui.add_space(8.0);
    barrels(ui, rifle, data, state, &u, actions);
    ui.add_space(8.0);
    loads_precision(ui, rifle, data, state, &u);
    ui.add_space(8.0);
    recent(ui, rifle, data, actions);
}

/// The width left on screen: a row wider than the page would be cut off.
fn room(ui: &egui::Ui) -> f32 {
    let visible = ui.clip_rect().right() - ui.cursor().left();
    ui.available_width().min(visible).max(120.0)
}

fn tile_row(ui: &mut egui::Ui, tiles: &[Tile; 3]) {
    let palette = theme::palette(ui.ctx());
    let gap = ui.spacing().item_spacing.x;
    // The frame's margin and stroke come on top of the width set inside it.
    let width = ((room(ui) - 2.0 * gap) / 3.0 - 4.0).clamp(80.0, 200.0);
    ui.horizontal(|ui| {
        for tile in tiles {
            let (fill, edge) = if tile.due {
                (
                    palette.warning_tint,
                    egui::Stroke::new(1.5_f32, palette.warning),
                )
            } else {
                (palette.raised, palette.edge)
            };
            egui::Frame::new()
                .fill(fill)
                .stroke(edge)
                .corner_radius(10)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(tile.title).small().color(palette.text_muted));
                        let colour = if tile.due {
                            palette.warning
                        } else {
                            palette.text
                        };
                        ui.label(RichText::new(&tile.value).size(22.0).strong().color(colour));
                        ui.add(
                            egui::Label::new(
                                RichText::new(&tile.detail)
                                    .small()
                                    .color(palette.text_muted),
                            )
                            .truncate(),
                        );
                    });
                });
        }
    });
}

fn log_sheet(
    context: &egui::Context,
    rifle: &Rifle,
    data: &Data,
    state: &mut HistoryState,
    actions: &mut Vec<Action>,
) {
    let Some(draft) = &mut state.log else {
        return;
    };
    let barrels: Vec<&Barrel> = barrels_of(rifle, data)
        .into_iter()
        .filter(|barrel| barrel.retired_at.is_none())
        .collect();
    let mut log = false;
    let shown = mobile::sheet(context, "log-rounds", tr("Log rounds"), |ui| {
        grid(ui, "log-rounds-grid", |ui| {
            ui.label(tr("Rounds"));
            ui.add(
                NumberField::new(&mut draft.rounds)
                    .range(1..=10_000)
                    .speed(1),
            );
            ui.end_row();
            ui.label(tr("Through"));
            ui.vertical(|ui| {
                for barrel in &barrels {
                    ui.radio_value(&mut draft.barrel_id, Some(barrel.id.clone()), &barrel.name);
                }
                ui.radio_value(&mut draft.barrel_id, None, tr("Action only"));
            });
            ui.end_row();
            text(ui, tr("Note"), &mut draft.note);
        });
        if draft.barrel_id.is_none() && !barrels.is_empty() {
            ui.weak(tr(
                "Action only: the rounds count for the action, not for a barrel.",
            ));
        }
        ui.add_space(6.0);
        log = ui
            .add(egui::Button::new(RichText::new(tr("Log")).strong()))
            .clicked();
    });
    if log {
        let mut entry = RoundLogEntry::new(
            &rifle.id,
            draft.barrel_id.as_deref(),
            draft.rounds,
            now_ms(),
        );
        entry.note = draft.note.trim().to_owned();
        actions.push(Action::Record(Box::new(Change::LogRounds(entry))));
        state.log = None;
    } else if shown.closed {
        state.log = None;
    }
}

fn barrels(
    ui: &mut egui::Ui,
    rifle: &Rifle,
    data: &Data,
    state: &mut HistoryState,
    u: &UnitPrefs,
    actions: &mut Vec<Action>,
) {
    ui.heading(tr("Barrels"));
    let all = barrels_of(rifle, data);
    let precision = state.precision(rifle, data).to_vec();
    let precision_of = |id: &str| {
        precision
            .iter()
            .find(|(key, _)| key == id)
            .and_then(|(_, found)| *found)
    };
    if all.is_empty() {
        ui.weak(tr(
            "No barrels yet. Add the one on the rifle to count its rounds apart from the \
             action's.",
        ));
    }
    for barrel in &all {
        let is_fitted = rifle.fitted_barrel_id.as_deref() == Some(barrel.id.as_str());
        let counts = RoundCounts::derive(rifle, Some(barrel), &data.groups.round_log, &[]);
        let mut first = vec![barrel.name.clone()];
        let spec = length_and_twist(u, barrel.length_mm, barrel.twist_in);
        if !spec.is_empty() {
            first.push(spec);
        }
        if is_fitted {
            first.push(tr("fitted").to_owned());
        } else if barrel.retired_at.is_some() {
            first.push(tr("retired").to_owned());
        }
        let mut second = Vec::new();
        if is_fitted && let Some(at) = barrel.fitted_at {
            second.push(trf!("since {}", day(at, data)));
        }
        let rounds = counts.barrel.unwrap_or_default();
        second.push(if rounds == 1 {
            tr("1 round").to_owned()
        } else {
            trf!("{} rounds", count(rounds))
        });
        second.push(precision_text(u, precision_of(&barrel.id)));
        let palette = theme::palette(ui.ctx());
        egui::Frame::new()
            .fill(palette.raised)
            .stroke(palette.edge)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(room(ui));
                ui.label(RichText::new(first.join(" · ")).strong());
                ui.weak(second.join(" · "));
                ui.horizontal_wrapped(|ui| {
                    if !is_fitted && ui.button(tr("Fit")).clicked() {
                        actions.push(Action::Record(Box::new(Change::FitBarrel(
                            barrel.id.clone(),
                        ))));
                    }
                    if ui.button(tr("Edit")).clicked() {
                        state.barrel = Some((*barrel).clone());
                    }
                    if barrel.retired_at.is_none()
                        && confirm_button(ui, &format!("retire-{}", barrel.id), tr("Retire"))
                    {
                        actions.push(Action::Record(Box::new(Change::RetireBarrel(
                            barrel.id.clone(),
                        ))));
                    }
                    if state.deleting.as_deref() != Some(barrel.id.as_str())
                        && ui.button(tr("Delete")).clicked()
                    {
                        state.deleting = Some(barrel.id.clone());
                    }
                });
                if state.deleting.as_deref() == Some(barrel.id.as_str()) {
                    let cleans = data
                        .groups
                        .cleans
                        .iter()
                        .filter(|clean| clean.barrel_id.as_deref() == Some(barrel.id.as_str()))
                        .count();
                    let resets = data
                        .groups
                        .zero_resets
                        .iter()
                        .filter(|reset| reset.barrel_id.as_deref() == Some(barrel.id.as_str()))
                        .count();
                    ui.add(
                        egui::Label::new(
                            RichText::new(trf!(
                                "Delete {} for good? Its cleans ({}) and zero resets ({}) go \
                                 with it, and there is no undo. Its rounds stay in the round \
                                 log and still count for the action. To keep its history, \
                                 retire it instead.",
                                barrel.name,
                                cleans,
                                resets
                            ))
                            .color(palette.warning),
                        )
                        .wrap(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let delete = egui::Button::new(
                            RichText::new(tr("Delete the barrel"))
                                .color(ui.visuals().error_fg_color),
                        );
                        if ui.add(delete).clicked() {
                            actions.push(Action::Record(Box::new(Change::DeleteBarrel(
                                barrel.id.clone(),
                            ))));
                            state.deleting = None;
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            state.deleting = None;
                        }
                    });
                }
                if is_fitted && differs_from_rifle(barrel, rifle) {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(
                            palette.warning,
                            trf!(
                                "The rifle's ballistics say {}.",
                                length_and_twist(u, rifle.barrel_length_mm, Some(rifle.twist_in))
                            ),
                        );
                        if ui.button(tr("Use this barrel's")).clicked() {
                            actions.push(Action::Record(Box::new(Change::CopyBarrelToRifle(
                                barrel.id.clone(),
                            ))));
                        }
                    });
                }
            });
    }
    if state.barrel.is_none() && ui.button(tr("＋ Add a barrel")).clicked() {
        let mut barrel = Barrel::new(&rifle.id, "");
        barrel.length_mm = rifle.barrel_length_mm;
        barrel.twist_in = Some(rifle.twist_in);
        barrel.right_hand_twist = rifle.right_hand_twist;
        state.barrel = Some(barrel);
    }
    barrel_editor(ui, data, state, actions);
}

fn barrel_editor(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut HistoryState,
    actions: &mut Vec<Action>,
) {
    let Some(barrel) = &mut state.barrel else {
        return;
    };
    let new = !data.groups.barrels.iter().any(|b| b.id == barrel.id);
    let palette = theme::palette(ui.ctx());
    let mut close = false;
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, palette.accent))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(if new {
                    tr("New barrel")
                } else {
                    tr("Edit barrel")
                })
                .strong(),
            );
            grid(ui, "barrel-editor", |ui| {
                text(ui, tr("Name"), &mut barrel.name);
                text(ui, tr("Maker"), &mut barrel.maker);
                optional_quantity(
                    ui,
                    tr("Barrel length"),
                    &mut barrel.length_mm,
                    560.0,
                    Quantity::BarrelLength,
                    100.0..=1500.0,
                    1.0,
                );
                optional_quantity(
                    ui,
                    tr("Twist, 1 turn in"),
                    &mut barrel.twist_in,
                    10.0,
                    Quantity::Twist,
                    3.0..=40.0,
                    0.05,
                );
                ui.label(tr("Twist direction"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut barrel.right_hand_twist, true, tr("Right"));
                    ui.selectable_value(&mut barrel.right_hand_twist, false, tr("Left"));
                });
                ui.end_row();
                ui.label(tr("Rounds before logging"));
                ui.add(
                    NumberField::new(&mut barrel.rounds_before_logging)
                        .range(0..=1_000_000)
                        .speed(10),
                )
                .on_hover_text(tr("Rounds fired through it before you began the log"));
                ui.end_row();
            });
            let problem = barrel.validate().err();
            ui.horizontal(|ui| {
                let label = if new { tr("Add and fit") } else { tr("Save") };
                if ui
                    .add_enabled(
                        problem.is_none(),
                        egui::Button::new(RichText::new(label).strong()),
                    )
                    .clicked()
                {
                    let mut barrel = barrel.clone();
                    barrel.name = barrel.name.trim().to_owned();
                    actions.push(Action::Record(Box::new(Change::SaveBarrel(barrel))));
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
            if let Some(problem) = problem.filter(|_| !barrel.name.trim().is_empty()) {
                ui.colored_label(ui.visuals().warn_fg_color, problem);
            }
        });
    if close {
        state.barrel = None;
    }
}

fn loads_precision(
    ui: &mut egui::Ui,
    rifle: &Rifle,
    data: &Data,
    state: &mut HistoryState,
    u: &UnitPrefs,
) {
    let precision = state.precision(rifle, data).to_vec();
    let barrel_ids: Vec<&str> = barrels_of(rifle, data)
        .iter()
        .map(|barrel| barrel.id.as_str())
        .collect();
    let loads: Vec<_> = precision
        .iter()
        .filter(|(id, _)| !barrel_ids.contains(&id.as_str()))
        .filter_map(|(id, found)| {
            let bullet = data.bullets.iter().find(|bullet| &bullet.id == id)?;
            Some((bullet, *found))
        })
        .collect();
    if loads.is_empty() {
        return;
    }
    ui.heading(tr("Measured precision"));
    ui.weak(tr(
        "σ of the groups pooled about their own centres, from the groups shot with each.",
    ));
    for (bullet, found) in loads {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(bullet.display_name()).strong());
            ui.weak(precision_text(u, found));
        });
    }
}

fn recent(ui: &mut egui::Ui, rifle: &Rifle, data: &Data, actions: &mut Vec<Action>) {
    let entries: Vec<&RoundLogEntry> = data
        .groups
        .round_log
        .iter()
        .filter(|entry| entry.rifle_id == rifle.id)
        .take(RECENT_ENTRIES)
        .collect();
    ui.heading(tr("Recent rounds"));
    if entries.is_empty() {
        ui.weak(tr("Nothing logged yet. Saving a group logs its shots."));
        return;
    }
    for entry in entries {
        let through = match entry.barrel_id.as_deref() {
            Some(id) => data
                .groups
                .barrels
                .iter()
                .find(|barrel| barrel.id == id)
                .map_or_else(|| tr("a removed barrel").to_owned(), |b| b.name.clone()),
            None => tr("action only").to_owned(),
        };
        let mut line = trf!("{} · {} on {}", day(entry.at, data), entry.rounds, through);
        if entry.group_id.is_some() {
            line.push_str(" · ");
            line.push_str(tr("group"));
        }
        if !entry.note.is_empty() {
            line.push_str(" · ");
            line.push_str(&entry.note);
        }
        ui.horizontal(|ui| {
            let width = room(ui) - 90.0;
            ui.allocate_ui_with_layout(
                egui::vec2(width.max(60.0), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(width.max(60.0));
                    ui.add(egui::Label::new(line).truncate());
                },
            );
            if confirm_button(ui, &format!("rounds-{}", entry.id), "✖") {
                actions.push(Action::Record(Box::new(Change::DeleteRounds(
                    entry.id.clone(),
                ))));
            }
        });
    }
}

/// The demo X-Bolt's barrels, rounds and last clean, added once.
pub fn load_demo(store: &ballistics_store::Store) -> Result<(), ballistics_store::StoreError> {
    const X_BOLT: &str = "9f017006-3ab4-50b9-b751-ea5070d9b3b7";
    const FACTORY: &str = "6d787363-6261-7272-656c-64656d6f0001";
    const PROOF: &str = "6d787363-6261-7272-656c-64656d6f0002";
    let Some(mut rifle) = store.rifle(X_BOLT)? else {
        return Ok(());
    };
    if store.barrel(FACTORY)?.is_some() {
        return Ok(());
    }
    let now = now_ms();
    let days = |count: i64| (now - count * DAY_MS).max(0);
    let mut factory = Barrel::new(X_BOLT, "Factory");
    factory.id = FACTORY.to_owned();
    factory.length_mm = Some(560.0);
    factory.twist_in = Some(10.0);
    factory.fitted_at = Some(days(900));
    factory.retired_at = Some(days(120));
    factory.rounds_before_logging = 1_150;
    store.save_barrel(&mut factory)?;
    let mut proof = Barrel::new(X_BOLT, "Proof 24\"");
    proof.id = PROOF.to_owned();
    proof.maker = "Proof Research".to_owned();
    proof.length_mm = Some(610.0);
    proof.twist_in = Some(10.0);
    proof.fitted_at = Some(days(120));
    store.save_barrel(&mut proof)?;
    rifle.fitted_barrel_id = Some(PROOF.to_owned());
    rifle.rounds_before_logging = 1_180;
    store.save_rifle(&mut rifle)?;
    for (rounds, ago, note) in [
        (60, 110, "break-in"),
        (40, 60, "practice"),
        (120, 30, "match"),
        (28, 10, "zero check"),
        (24, 2, "ladder"),
    ] {
        let mut entry = RoundLogEntry::new(X_BOLT, Some(PROOF), rounds, days(ago));
        entry.note = note.to_owned();
        store.save_round_log_entry(&mut entry)?;
    }
    let mut clean = Clean::new(X_BOLT, Some(PROOF), days(61));
    clean.note = "copper solvent".to_owned();
    store.save_clean(&mut clean)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_clean_turns_amber_at_the_rifles_limit() {
        let counts = RoundCounts {
            action: 2_412,
            barrel: Some(1_180),
            since_clean: Some(212),
            last_clean_at: Some(0),
        };
        let [action, barrel, since] = tiles(&counts, Some("Proof"), 200, Some("12 Nov".into()));
        assert_eq!(action.value, "2\u{202f}412");
        assert_eq!(barrel.value, "1\u{202f}180");
        assert_eq!(barrel.detail, "Proof");
        assert!(since.due && !action.due && !barrel.due);
        let [_, _, since] = tiles(&counts, Some("Proof"), 250, Some("12 Nov".into()));
        assert!(!since.due);
        assert!(since.detail.contains("12 Nov"));

        let never = RoundCounts {
            action: 40,
            ..RoundCounts::default()
        };
        let [_, barrel, since] = tiles(&never, None, 200, None);
        assert_eq!((barrel.value.as_str(), since.value.as_str()), ("—", "—"));
        assert!(!since.due);
    }

    #[test]
    fn the_demo_action_has_fired_at_least_what_each_barrel_has() {
        let mut store = ballistics_store::Store::open_in_memory().unwrap();
        store.load_demo().unwrap();
        load_demo(&store).unwrap();
        let log = store.round_log().unwrap();
        let cleans = store.cleans().unwrap();
        let mut checked = 0;
        for rifle in store.rifles().unwrap() {
            let barrels = store.barrels_for_rifle(&rifle.id).unwrap();
            for barrel in &barrels {
                let counts = RoundCounts::derive(&rifle, Some(barrel), &log, &cleans);
                assert!(
                    counts.action >= counts.barrel.unwrap(),
                    "{} fired {} through {}",
                    rifle.name,
                    counts.action,
                    barrel.name
                );
                checked += 1;
            }
            let total: u64 = barrels
                .iter()
                .map(|barrel| {
                    RoundCounts::derive(&rifle, Some(barrel), &log, &cleans)
                        .barrel
                        .unwrap()
                })
                .sum();
            let action = RoundCounts::derive(&rifle, None, &log, &cleans).action;
            assert!(action >= total, "{}: {action} < {total}", rifle.name);
        }
        assert!(checked >= 2, "the demo has barrels");
    }

    #[test]
    fn a_barrel_differs_when_its_length_or_twist_do() {
        let mut rifle = Rifle::new("Tikka", "308-win", 11.0);
        rifle.barrel_length_mm = Some(610.0);
        let mut barrel = Barrel::new(&rifle.id, "Proof");
        assert!(!differs_from_rifle(&barrel, &rifle), "nothing known");
        barrel.length_mm = Some(610.0);
        barrel.twist_in = Some(11.0);
        assert!(!differs_from_rifle(&barrel, &rifle));
        barrel.twist_in = Some(8.0);
        assert!(differs_from_rifle(&barrel, &rifle));
        barrel.twist_in = Some(11.0);
        barrel.right_hand_twist = false;
        assert!(differs_from_rifle(&barrel, &rifle));
    }
}

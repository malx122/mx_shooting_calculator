//! What a group is of: the rifle, its barrel, the bullet, the distance, the
//! session, tags and notes, and whether its rounds count on the barrel.
//! Shown in a sheet on a phone and in the right pane on a desktop.

use ballistics_store::model::now_ms;
use ballistics_store::{Bullet, Group, Ladder, LadderVariable, Rifle, Session};
use eframe::egui;

use super::library::{day_name, session_line};
use crate::app::Data;
use crate::i18n::{tr, trf};
use crate::units::{self, Quantity};

const DAY_MS: i64 = 86_400_000;
const GRAINS_PER_GRAM: f64 = 15.432_358;
/// The most sessions the form offers to pick from.
const SESSIONS_OFFERED: usize = 12;
/// The distance a first group is at.
pub const DEFAULT_DISTANCE_M: f64 = 100.0;

/// The session a group belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionChoice {
    None,
    Existing(String),
    /// A new session at a place, `days_ago` days before today.
    New {
        place: String,
        days_ago: i64,
    },
}

/// The group's details as edited.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub rifle_id: Option<String>,
    pub bullet_id: Option<String>,
    pub distance_m: f64,
    pub session: SessionChoice,
    /// The tags, separated by commas.
    pub tags: String,
    pub notes: String,
    pub count_rounds: bool,
    /// The ladder the group is a rung of.
    pub ladder_id: Option<String>,
    /// The rung's value: the charge in g, or the CBTO in mm.
    pub rung_value: f64,
    /// What was dialled away from the zero for it, mrad, [right, up].
    pub dialled_mrad: [f64; 2],
    pub include_in_zero: bool,
}

impl Form {
    /// A new group's details: those of the newest group (the last used),
    /// in today's session if there is one.
    pub fn for_new(data: &Data, count_rounds: bool) -> Form {
        let last = data.groups.groups.iter().max_by_key(|group| group.taken_at);
        let rifle_id = last
            .and_then(|group| group.rifle_id.clone())
            .filter(|id| data.rifles.iter().any(|rifle| &rifle.id == id))
            .or_else(|| data.rifles.first().map(|rifle| rifle.id.clone()));
        let bullet_id = last
            .filter(|group| group.rifle_id == rifle_id)
            .and_then(|group| group.bullet_id.clone())
            .or_else(|| {
                let rifle = data
                    .rifles
                    .iter()
                    .find(|r| Some(&r.id) == rifle_id.as_ref())?;
                bullets_for(rifle, data)
                    .first()
                    .map(|bullet| bullet.id.clone())
            });
        let today = super::local_ms(now_ms(), data.country).div_euclid(DAY_MS);
        let session = data
            .groups
            .sessions
            .iter()
            .filter(|session| {
                super::local_ms(session.started_at, data.country).div_euclid(DAY_MS) == today
            })
            .max_by_key(|session| session.started_at)
            .map_or(SessionChoice::None, |session| {
                SessionChoice::Existing(session.id.clone())
            });
        let distance_m = last
            .and_then(|group| group.distance_m)
            .unwrap_or(DEFAULT_DISTANCE_M);
        let mut form = Form {
            rifle_id,
            bullet_id,
            distance_m,
            session,
            tags: String::new(),
            notes: String::new(),
            count_rounds,
            ladder_id: None,
            rung_value: 0.0,
            dialled_mrad: [0.0, 0.0],
            include_in_zero: true,
        };
        form.include_in_zero = form.at_zero_range(data);
        form
    }

    /// Whether the group's distance is its rifle's zero range, as zero
    /// checks take it; true without a rifle.
    pub fn at_zero_range(&self, data: &Data) -> bool {
        self.rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id))
            .is_none_or(|rifle| {
                super::analyse::pool::at_zero_range(self.distance_m, rifle.zero_range_m)
            })
    }

    /// A saved group's details.
    pub fn of(group: &Group) -> Form {
        Form {
            rifle_id: group.rifle_id.clone(),
            bullet_id: group.bullet_id.clone(),
            distance_m: group.distance_m.unwrap_or(DEFAULT_DISTANCE_M),
            session: group
                .session_id
                .clone()
                .map_or(SessionChoice::None, SessionChoice::Existing),
            tags: group.tags.join(", "),
            notes: group.notes.clone(),
            count_rounds: group.count_rounds,
            ladder_id: group.ladder_id.clone(),
            rung_value: group.rung_value.unwrap_or(0.0),
            dialled_mrad: group.dialled_offset_mrad,
            include_in_zero: group.include_in_zero,
        }
    }

    /// The form as it would be saved: a ladder's rung counts in no zero
    /// check, and without a ladder there is no rung.
    fn saved(&self) -> Form {
        let mut form = self.clone();
        if form.ladder_id.is_some() {
            form.include_in_zero = false;
        } else {
            form.rung_value = 0.0;
        }
        form
    }

    /// Another group shot like `group`, as on the same photo.
    pub fn like(group: &Group) -> Form {
        Form {
            notes: String::new(),
            ..Form::of(group)
        }
    }

    /// Whether the details differ from the group's.
    pub fn changed(&self, group: &Group) -> bool {
        self.saved() != Form::of(group).saved()
    }

    /// The bullet's diameter, for bullet-sized holes and edge-to-edge sizes.
    pub fn calibre_mm(&self, data: &Data) -> Option<f64> {
        calibre_of(self.bullet_id.as_deref(), data)
    }

    /// Puts the details on `group`; returns the new session to save with it,
    /// if one was asked for.
    pub fn apply(&self, group: &mut Group, data: &Data) -> Option<Session> {
        let rifle = self
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id));
        if group.rifle_id != self.rifle_id || group.barrel_id.is_none() {
            group.barrel_id = rifle.and_then(|rifle| rifle.fitted_barrel_id.clone());
        }
        if rifle.is_none() {
            group.barrel_id = None;
        }
        let bullet_changed = group.bullet_id != self.bullet_id;
        group.rifle_id = self.rifle_id.clone();
        group.bullet_id = self.bullet_id.clone();
        group.distance_m = Some(self.distance_m.clamp(1.0, 5000.0));
        group.tags = parse_tags(&self.tags);
        group.notes = self.notes.trim_end().to_owned();
        group.count_rounds = self.count_rounds;
        let ladder = self
            .ladder_id
            .as_ref()
            .filter(|id| data.groups.ladders.iter().any(|ladder| &ladder.id == *id));
        group.ladder_id = ladder.cloned();
        group.rung_value = ladder.map(|_| self.rung_value);
        if ladder.is_some() {
            (group.charge_g, group.cbto_mm) = (None, None);
        } else if bullet_changed || (group.charge_g.is_none() && group.cbto_mm.is_none()) {
            (group.charge_g, group.cbto_mm) = recipe_now(self.bullet_id.as_deref(), data);
        }
        group.include_in_zero = self.include_in_zero && ladder.is_none();
        group.dialled_offset_mrad = self.dialled_mrad.map(|mrad| {
            if mrad.is_finite() {
                mrad.clamp(-100.0, 100.0)
            } else {
                0.0
            }
        });
        match &self.session {
            SessionChoice::None => {
                group.session_id = None;
                None
            }
            SessionChoice::Existing(id) => {
                group.session_id = Some(id.clone());
                None
            }
            SessionChoice::New { place, days_ago } => {
                let mut session = Session::new((now_ms() - days_ago * DAY_MS).max(0));
                session.place = place.trim().chars().take(100).collect();
                group.session_id = Some(session.id.clone());
                Some(session)
            }
        }
    }
}

/// The handload's charge (g) and CBTO (mm) as its recipe gives them now,
/// kept with a group so a later change to the recipe doesn't move it.
pub fn recipe_now(bullet_id: Option<&str>, data: &Data) -> (Option<f64>, Option<f64>) {
    let recipe = data
        .bullets
        .iter()
        .find(|bullet| Some(bullet.id.as_str()) == bullet_id)
        .and_then(|bullet| bullet.recipe.as_ref());
    let within = |value: Option<f64>, range: std::ops::RangeInclusive<f64>| {
        value.filter(|value| range.contains(value))
    };
    recipe.map_or((None, None), |recipe| {
        (
            within(recipe.charge_g, 0.01..=30.0),
            within(recipe.cbto_mm, 5.0..=200.0),
        )
    })
}

/// The bullet's diameter, mm.
pub fn calibre_of(bullet_id: Option<&str>, data: &Data) -> Option<f64> {
    let bullet = data
        .bullets
        .iter()
        .find(|b| Some(b.id.as_str()) == bullet_id)?;
    (bullet.diameter_mm > 0.0).then_some(bullet.diameter_mm)
}

/// The bullets a rifle shoots: those with a load set up for it first, then
/// the others of its calibre.
pub fn bullets_for<'a>(rifle: &Rifle, data: &'a Data) -> Vec<&'a Bullet> {
    let loaded = |bullet: &Bullet| {
        data.loads
            .iter()
            .any(|load| load.rifle_id == rifle.id && load.bullet_id == bullet.id)
    };
    let mut bullets: Vec<&Bullet> = data
        .bullets
        .iter()
        .filter(|bullet| bullet.caliber == rifle.caliber || loaded(bullet))
        .collect();
    bullets.sort_by_key(|bullet| !loaded(bullet));
    bullets
}

/// Tags from text separated by commas: trimmed, each once, at most 50 of
/// at most 60 characters.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in text.split(',') {
        let tag: String = tag
            .trim()
            .chars()
            .take(ballistics_store::groups::TAG_MAX)
            .collect();
        let tag = tag.trim().to_owned();
        if !tag.is_empty() && !tags.iter().any(|kept| kept.eq_ignore_ascii_case(&tag)) {
            tags.push(tag);
        }
    }
    tags.truncate(ballistics_store::groups::MAX_TAGS);
    tags
}

/// The form's fields. `shots` is how many rounds the group holds.
pub fn show(ui: &mut egui::Ui, form: &mut Form, data: &Data, shots: usize) {
    let u = units::prefs(ui.ctx());
    let placed = (form.rifle_id.clone(), form.distance_m);
    crate::widgets::grid(ui, "group-form", |ui| {
        ui.label(tr("Rifle"));
        let rifle_name = match &form.rifle_id {
            None => tr("None").to_owned(),
            Some(id) => data
                .rifles
                .iter()
                .find(|rifle| &rifle.id == id)
                .map_or_else(
                    || tr("A rifle not on this device").to_owned(),
                    |rifle| rifle.name.clone(),
                ),
        };
        let before = form.rifle_id.clone();
        egui::ComboBox::from_id_salt("group-rifle")
            .selected_text(rifle_name)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.rifle_id, None, tr("None"));
                for rifle in &data.rifles {
                    ui.selectable_value(&mut form.rifle_id, Some(rifle.id.clone()), &rifle.name);
                }
            });
        ui.end_row();
        let rifle = form
            .rifle_id
            .as_ref()
            .and_then(|id| data.rifles.iter().find(|rifle| &rifle.id == id));
        if form.rifle_id != before {
            form.bullet_id =
                rifle.and_then(|rifle| bullets_for(rifle, data).first().map(|b| b.id.clone()));
        }
        if let Some(rifle) = rifle {
            ui.label(tr("Barrel"));
            let barrel = rifle
                .fitted_barrel_id
                .as_ref()
                .and_then(|id| data.groups.barrels.iter().find(|barrel| &barrel.id == id));
            match barrel {
                Some(barrel) => ui.label(&barrel.name),
                None => ui.add(
                    egui::Label::new(
                        egui::RichText::new(tr("none fitted: the rounds count on the rifle"))
                            .weak(),
                    )
                    .wrap(),
                ),
            };
            ui.end_row();
        }

        ui.label(tr("Bullet"));
        let bullet_name = form
            .bullet_id
            .as_ref()
            .and_then(|id| data.bullets.iter().find(|bullet| &bullet.id == id))
            .map_or_else(|| tr("None").to_owned(), Bullet::display_name);
        let offered: Vec<&Bullet> = match rifle {
            Some(rifle) => bullets_for(rifle, data),
            None => data.bullets.iter().collect(),
        };
        egui::ComboBox::from_id_salt("group-bullet")
            .selected_text(bullet_name)
            .width(ui.available_width().clamp(120.0, 260.0))
            .truncate()
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.bullet_id, None, tr("None"));
                for bullet in offered {
                    ui.selectable_value(
                        &mut form.bullet_id,
                        Some(bullet.id.clone()),
                        bullet.display_name(),
                    );
                }
            });
        ui.end_row();

        units::quantity(
            ui,
            tr("Distance"),
            &mut form.distance_m,
            Quantity::Distance,
            1.0..=5000.0,
            1.0,
        );

        ladder_rows(ui, form, data);
        ui.label(tr("Dialled for this group")).on_hover_text(tr(
            "What you dialled away from your zero for it, up and right. Zero checks take \
                 it off the group's impact.",
        ));
        ui.horizontal(|ui| {
            ui.weak("↑");
            units::drag(
                ui,
                &mut form.dialled_mrad[1],
                Quantity::GroupAngle,
                -100.0..=100.0,
                0.01,
            );
            ui.weak("→");
            units::drag(
                ui,
                &mut form.dialled_mrad[0],
                Quantity::GroupAngle,
                -100.0..=100.0,
                0.01,
            );
        });
        ui.end_row();

        ui.label(tr("Session"));
        session_picker(ui, form, data, &u);
        ui.end_row();
        if let SessionChoice::New { place, days_ago } = &mut form.session {
            ui.label(tr("Place"));
            ui.add(
                egui::TextEdit::singleline(place)
                    .hint_text(tr("Range or place"))
                    .desired_width(200.0),
            );
            ui.end_row();
            ui.label(tr("Date"));
            ui.horizontal(|ui| {
                if ui
                    .small_button("‹")
                    .on_hover_text(tr("A day earlier"))
                    .clicked()
                {
                    *days_ago += 1;
                }
                let today = super::local_ms(now_ms(), data.country).div_euclid(DAY_MS);
                ui.label(day_name(today - *days_ago, today));
                if ui
                    .add_enabled(*days_ago > 0, egui::Button::new("›").small())
                    .on_hover_text(tr("A day later"))
                    .clicked()
                {
                    *days_ago -= 1;
                }
            });
            ui.end_row();
        }

        ui.label(tr("Tags"));
        ui.add(
            egui::TextEdit::singleline(&mut form.tags)
                .hint_text(tr("practice, prone"))
                .desired_width(200.0),
        );
        ui.end_row();
    });
    ui.label(tr("Notes"));
    ui.add(
        egui::TextEdit::multiline(&mut form.notes)
            .hint_text(tr("Wind, light, how it felt…"))
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    // "Include in zero check" follows the zero range when the rifle or the
    // distance changes; the hunter can still change it.
    if (form.rifle_id.clone(), form.distance_m) != placed {
        form.include_in_zero = form.at_zero_range(data);
    }
    if form.ladder_id.is_none() {
        ui.checkbox(&mut form.include_in_zero, tr("Include in zero check"))
            .on_hover_text(tr(
                "Zero checks pool the groups shot since the last zero reset. Leave out a group \
                 shot on purpose somewhere else.",
            ));
    }
    let rifle_known = form
        .rifle_id
        .as_ref()
        .is_some_and(|id| data.rifles.iter().any(|rifle| &rifle.id == id));
    if rifle_known {
        let label = if shots == 1 {
            tr("Count this round on the barrel").to_owned()
        } else {
            trf!("Count these {} rounds on the barrel", shots)
        };
        ui.checkbox(&mut form.count_rounds, label);
    }
}

/// The rung value a ladder's next group starts at: its newest group's, or
/// its bullet's recipe.
pub fn rung_start(ladder: &Ladder, data: &Data) -> f64 {
    let newest = data
        .groups
        .groups
        .iter()
        .filter(|group| group.ladder_id.as_deref() == Some(ladder.id.as_str()))
        .max_by_key(|group| group.taken_at)
        .and_then(|group| group.rung_value);
    let recipe = data
        .bullets
        .iter()
        .find(|bullet| bullet.id == ladder.bullet_id)
        .and_then(|bullet| bullet.recipe.as_ref());
    match ladder.variable {
        LadderVariable::Charge => newest.or_else(|| recipe?.charge_g).unwrap_or(2.5),
        LadderVariable::Seating => newest.or_else(|| recipe?.cbto_mm).unwrap_or(50.0),
    }
}

/// "Part of a ladder", and the rung's charge or seating.
fn ladder_rows(ui: &mut egui::Ui, form: &mut Form, data: &Data) {
    let ladders: Vec<&Ladder> = data
        .groups
        .ladders
        .iter()
        .filter(|ladder| {
            form.rifle_id
                .as_ref()
                .is_none_or(|id| &ladder.rifle_id == id)
        })
        .collect();
    if ladders.is_empty() && form.ladder_id.is_none() {
        return;
    }
    ui.label(tr("Part of a ladder"));
    let chosen = form
        .ladder_id
        .as_ref()
        .and_then(|id| data.groups.ladders.iter().find(|ladder| &ladder.id == id));
    let name = |ladder: &Ladder| super::analyse::ladder_name(ladder, data);
    let before = form.ladder_id.clone();
    egui::ComboBox::from_id_salt("group-ladder")
        .selected_text(chosen.map_or_else(|| tr("No").to_owned(), name))
        .width(ui.available_width().clamp(120.0, 260.0))
        .truncate()
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut form.ladder_id, None, tr("No"));
            for ladder in &ladders {
                ui.selectable_value(&mut form.ladder_id, Some(ladder.id.clone()), name(ladder));
            }
        });
    ui.end_row();
    let chosen = form
        .ladder_id
        .as_ref()
        .and_then(|id| data.groups.ladders.iter().find(|ladder| &ladder.id == id));
    if form.ladder_id != before
        && let Some(ladder) = chosen
    {
        form.rung_value = rung_start(ladder, data);
        form.rifle_id = Some(ladder.rifle_id.clone());
        form.bullet_id = Some(ladder.bullet_id.clone());
    }
    if let Some(ladder) = chosen {
        match ladder.variable {
            LadderVariable::Charge => units::quantity(
                ui,
                tr("Charge"),
                &mut form.rung_value,
                Quantity::PowderCharge,
                0.01..=30.0,
                // 0.1 gr a step.
                0.1 / GRAINS_PER_GRAM,
            ),
            LadderVariable::Seating => units::quantity(
                ui,
                tr("Seating (CBTO)"),
                &mut form.rung_value,
                Quantity::BulletLength,
                5.0..=200.0,
                0.01,
            ),
        }
    }
}

fn session_picker(ui: &mut egui::Ui, form: &mut Form, data: &Data, u: &units::UnitPrefs) {
    let today = super::local_ms(now_ms(), data.country).div_euclid(DAY_MS);
    let local = |ms: i64| super::local_ms(ms, data.country);
    let name = |session: &Session| {
        let line = session_line(session, today, local, u);
        if line.is_empty() {
            tr("Session").to_owned()
        } else {
            line
        }
    };
    let chosen = match &form.session {
        SessionChoice::None => tr("None").to_owned(),
        SessionChoice::New { .. } => tr("New session").to_owned(),
        SessionChoice::Existing(id) => data
            .groups
            .sessions
            .iter()
            .find(|session| &session.id == id)
            .map_or_else(|| tr("A session not on this device").to_owned(), name),
    };
    egui::ComboBox::from_id_salt("group-session")
        .selected_text(chosen)
        .width(ui.available_width().clamp(120.0, 260.0))
        .truncate()
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut form.session, SessionChoice::None, tr("None"));
            let new = matches!(form.session, SessionChoice::New { .. });
            if ui.selectable_label(new, tr("+ New session")).clicked() && !new {
                form.session = SessionChoice::New {
                    place: String::new(),
                    days_ago: 0,
                };
            }
            for session in data.groups.sessions.iter().take(SESSIONS_OFFERED) {
                ui.selectable_value(
                    &mut form.session,
                    SessionChoice::Existing(session.id.clone()),
                    name(session),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_split_trimmed_and_kept_once() {
        assert_eq!(
            parse_tags(" practice, prone ,, Practice,  bench rest "),
            ["practice", "prone", "bench rest"]
        );
        assert!(parse_tags(" , ").is_empty());
        let long = "x".repeat(80);
        assert_eq!(parse_tags(&long)[0].chars().count(), 60);
        let many: Vec<String> = (0..70).map(|i| format!("t{i}")).collect();
        assert_eq!(parse_tags(&many.join(",")).len(), 50);
    }

    #[test]
    fn a_saved_groups_details_are_unchanged_until_edited() {
        let mut group = Group::on_grid(200.0, 200.0, 1);
        group.tags = vec!["practice".into(), "prone".into()];
        group.distance_m = Some(300.0);
        let mut form = Form::of(&group);
        assert!(!form.changed(&group));
        assert_eq!(form.tags, "practice, prone");
        form.notes = "gusty".into();
        assert!(form.changed(&group));
        let data = Data::default();
        let session = form.apply(&mut group, &data);
        assert!(session.is_none());
        assert_eq!(group.notes, "gusty");
        assert!(!form.changed(&group));

        form.session = SessionChoice::New {
            place: " Boden ".into(),
            days_ago: 1,
        };
        let session = form.apply(&mut group, &data).expect("a new session");
        assert_eq!(session.place, "Boden");
        assert_eq!(group.session_id.as_deref(), Some(session.id.as_str()));
        assert!(session.validate().is_ok());
    }

    #[test]
    fn a_ladder_rung_counts_in_no_zero_check_and_dialling_is_kept() {
        let mut data = Data::default();
        let ladder = Ladder::new("rifle", "load", LadderVariable::Charge);
        data.groups.ladders.push(ladder.clone());
        let mut group = Group::on_grid(200.0, 200.0, 1);
        let mut form = Form::of(&group);
        assert!(form.include_in_zero);
        form.dialled_mrad = [0.3, 2.5];
        form.apply(&mut group, &data);
        assert_eq!(group.dialled_offset_mrad, [0.3, 2.5]);
        assert!(group.include_in_zero && group.ladder_id.is_none() && group.rung_value.is_none());

        form.ladder_id = Some(ladder.id.clone());
        form.rung_value = 2.916;
        assert!(form.changed(&group));
        form.apply(&mut group, &data);
        assert_eq!(group.ladder_id.as_deref(), Some(ladder.id.as_str()));
        assert_eq!(group.rung_value, Some(2.916), "grams, as stored");
        assert!(
            !group.include_in_zero,
            "a ladder's rung stays out of zero checks"
        );
        assert!(!form.changed(&group));

        form.ladder_id = None;
        form.apply(&mut group, &data);
        assert!(group.rung_value.is_none());
        assert!(
            !form.changed(&group),
            "the rung's value goes with the ladder"
        );
    }
}

//! The Garmin watch (`watch/`): every rifle and load on the MX Shooting
//! Calculator app on a Garmin watch, kept up to date while this app is
//! open. The watch pairs once, with a code it shows; after that the
//! profiles go through the sync server encrypted for that watch only. See
//! docs/watch.md.

mod profile;
mod seal;

use std::sync::{Arc, Mutex};

use ballistics_store::Store;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cards::Subject;
use crate::http::{self, Method, Request};
use crate::i18n::{tr, trf};
use crate::units::{Quantity, UnitPrefs};

pub use profile::WatchProfile;

/// This device's link to the watch: a device-local setting, never synced
/// or exported (the keys in it are this device's own).
const SETTING: &str = "watch";
const CODE_DIGITS: usize = 8;
/// The most profiles the watch takes.
pub const MAX_PROFILES: usize = 16;
const DEFAULT_FARTHEST_M: f64 = 600.0;
const SHORTEST_FARTHEST_M: f64 = 100.0;
const MOST_FARTHEST_M: f64 = 1500.0;
/// How often to look for changes to send, s…
const CHECK_EVERY_S: f64 = 2.0;
/// …and how long they must stand before they go, s.
const SETTLE_S: f64 = 4.0;
/// Asking the server whether the watch has said yes to the pairing, s…
const ASK_PAIRING_S: f64 = 5.0;
/// …and, once paired, what it has (while the panel is open), s.
const ASK_PAIRED_S: f64 = 20.0;
/// After a failed update, the same profiles are tried again after this
/// long, s, doubling each time…
const RETRY_FIRST_S: f64 = 60.0;
/// …up to this.
const RETRY_MOST_S: f64 = 1800.0;
/// A forget the server hasn't heard yet is tried again this often, s.
const FORGET_EVERY_S: f64 = 60.0;
/// The most a watch keeps of all its profiles together, bytes.
const MAX_BUNDLE: usize = 96 * 1024;
/// Versions fit the watch's signed 32-bit numbers.
const MAX_VERSION: u32 = i32::MAX as u32;

fn default_farthest() -> f64 {
    DEFAULT_FARTHEST_M
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    link: Option<Link>,
    /// Links forgotten here that the server hasn't heard of yet.
    #[serde(default)]
    forgets: Vec<Forget>,
    /// The farthest range in the profiles, m.
    #[serde(default = "default_farthest")]
    farthest_m: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            link: None,
            forgets: Vec::new(),
            farthest_m: DEFAULT_FARTHEST_M,
        }
    }
}

/// The watch this device is paired with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Link {
    server: String,
    /// The watch's public key, base64 (docs/watch.md, "Server API").
    watch_key: String,
    /// This app's private key for the watch, hex.
    secret: String,
    /// Lets this app replace the profiles on the server, hex; the server
    /// keeps only its hash.
    writer: String,
    /// The version last sent.
    version: u32,
    /// What it was made from (`fingerprint`).
    sent: String,
    /// The profiles' names, as sent.
    names: Vec<String>,
    /// Rifles and loads left out of what was sent: why.
    #[serde(default)]
    skipped: Vec<String>,
    /// The watch has said yes to the pairing.
    paired: bool,
}

/// A link to delete on the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Forget {
    server: String,
    writer: String,
}

/// What the watch has told the server it has.
#[derive(Clone, Debug, Default, PartialEq)]
struct WatchWord {
    /// The version it keeps.
    version: Option<u32>,
    /// It couldn't read the last one sent.
    unreadable: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Status {
    #[default]
    Idle,
    Working(String),
    Failed(String),
}

/// What a job hands back for the panel's own thread to keep.
enum Event {
    /// The profiles went to the code: keep the link, not yet said yes to.
    Linked(Link),
    /// The watch said yes.
    Paired,
    /// Changes went.
    Sent {
        version: u32,
        sent: String,
        names: Vec<String>,
        skipped: Vec<String>,
    },
    /// Changes didn't go (what they were made from).
    NotSent(String),
    /// The server has deleted a forgotten link (its writer).
    Forgotten(String),
    /// The link is no more (declined, expired, forgotten): why.
    Unlinked(String),
}

#[derive(Default)]
struct Shared {
    /// Bumped when a link is made or forgotten: older jobs' results are
    /// dropped.
    generation: u32,
    busy: bool,
    status: Status,
    events: Vec<(u32, Event)>,
    watch: WatchWord,
}

pub struct WatchSync {
    settings: Settings,
    code: String,
    shared: Arc<Mutex<Shared>>,
    /// The inputs last looked at and since when they have stood (s).
    seen: Option<(String, f64)>,
    next_check: f64,
    next_ask: f64,
    /// When the panel was last drawn (s).
    panel_at: f64,
    /// Profiles that didn't go: what they were made from, when to try
    /// again, and how long the wait was.
    failed: Option<(String, f64, f64)>,
    next_forget: f64,
}

/// The code's digits, if it is a whole code (spaces and dashes allowed).
fn parse_code(text: &str) -> Option<String> {
    let digits: String = text
        .chars()
        .filter(|character| !matches!(character, ' ' | '-'))
        .collect();
    (digits.len() == CODE_DIGITS && digits.chars().all(|character| character.is_ascii_digit()))
        .then_some(digits)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

fn now_s() -> u32 {
    (ballistics_store::model::now_ms() / 1000).clamp(1, i64::from(MAX_VERSION)) as u32
}

/// What a profile's name will be.
fn name_of(subject: &Subject) -> String {
    format!("{} · {}", subject.rifle.name, subject.bullet.display_name())
}

/// Everything the profiles are made from: when it changes, they go again.
/// Only what the flight and the watch use, so a changed note or card design
/// doesn't send them all again.
fn fingerprint(subjects: &[Subject], u: &UnitPrefs, farthest_m: f64) -> String {
    let mut hash = Sha256::new();
    hash.update([profile::FORMAT]);
    hash.update(farthest_m.to_le_bytes());
    for quantity in [
        Quantity::Distance,
        Quantity::WindSpeed,
        Quantity::ShortLength,
        Quantity::Temperature,
        Quantity::Pressure,
    ] {
        hash.update(u.label(quantity));
        hash.update([0]);
    }
    for subject in subjects.iter().take(MAX_PROFILES) {
        let subject = profile::on_grid(subject);
        let scenario = ballistics_shot::scenario(
            &subject.rifle,
            &subject.bullet,
            subject.load.as_ref(),
            &subject.conditions,
        );
        hash.update(format!(
            "{:?}|{}|{}|{}|{:?}|{}|{:?}|{:?}\n",
            scenario,
            subject.rifle.id,
            subject.bullet.id,
            name_of(&subject),
            subject.rifle.click_unit,
            subject.rifle.click_value,
            subject.min_energy_j,
            subject.min_velocity_mps,
        ));
    }
    hex(&hash.finalize())
}

/// The profiles, and the rifles and loads left out.
struct Built {
    profiles: Vec<WatchProfile>,
    skipped: Vec<String>,
}

/// The profiles: one per rifle and load, a shorter one for a bullet that
/// falls short of `farthest_m`, none (and why) for one that can't be worked
/// out at all. Each takes a moment: between them the browser gets to draw.
async fn build(subjects: &[Subject], u: &UnitPrefs, farthest_m: f64) -> Result<Built, String> {
    if subjects.is_empty() {
        return Err(tr("Add a rifle and a bullet for it first.").to_owned());
    }
    let mut built = Built {
        profiles: Vec::new(),
        skipped: Vec::new(),
    };
    for subject in subjects.iter().take(MAX_PROFILES) {
        http::sleep(0).await;
        let mut farthest = farthest_m;
        loop {
            match WatchProfile::compute(subject, u, farthest) {
                Ok(profile) => {
                    built.profiles.push(profile);
                    break;
                }
                Err(_) if farthest > SHORTEST_FARTHEST_M => {
                    farthest = (farthest / 2.0).max(SHORTEST_FARTHEST_M);
                }
                Err(error) => {
                    built.skipped.push(format!("{}: {error}", name_of(subject)));
                    break;
                }
            }
        }
    }
    if built.profiles.is_empty() {
        return Err(built.skipped.join("\n"));
    }
    let bytes: usize = built
        .profiles
        .iter()
        .map(|profile| profile.encode(0, 0, 1).len())
        .sum();
    if bytes > MAX_BUNDLE {
        return Err(tr(
            "That is more than the watch can keep: choose a shorter range, or remove rifles \
             or loads you don't use.",
        )
        .to_owned());
    }
    Ok(built)
}

/// The profiles sealed for the watch, as the server takes them.
fn seal_all(
    profiles: &[WatchProfile],
    secret: &p256::SecretKey,
    watch: &p256::PublicKey,
    version: u32,
) -> Result<Vec<String>, String> {
    let count = profiles.len() as u8;
    profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let sealed = seal::seal(secret, watch, &profile.encode(version, index as u8, count));
            if sealed.len() > seal::MAX_BOX {
                return Err(
                    tr("The profiles are too long for the watch: choose a shorter range.")
                        .to_owned(),
                );
            }
            Ok(BASE64.encode(sealed))
        })
        .collect()
}

impl WatchSync {
    pub fn load(store: &Store) -> Self {
        WatchSync {
            settings: store.setting(SETTING).ok().flatten().unwrap_or_default(),
            code: String::new(),
            shared: Arc::default(),
            seen: None,
            next_check: 0.0,
            next_ask: 0.0,
            panel_at: f64::NEG_INFINITY,
            failed: None,
            next_forget: 0.0,
        }
    }

    /// For the menu: paired or not.
    pub fn short_status(&self) -> &'static str {
        match &self.settings.link {
            Some(link) if link.paired => tr("Paired"),
            Some(_) => tr("Pairing"),
            None => tr("Not paired"),
        }
    }

    fn save(&self, store: &Store) {
        let _ = store.set_setting(SETTING, &self.settings);
    }

    fn with<T>(&self, then: impl FnOnce(&mut Shared) -> T) -> Option<T> {
        self.shared.lock().ok().map(|mut shared| then(&mut shared))
    }

    fn busy(&self) -> bool {
        self.with(|shared| shared.busy).unwrap_or(true)
    }

    /// Keeps what the jobs found, asks after the pairing, and sends the
    /// profiles again when what they are made from has changed.
    pub fn poll(
        &mut self,
        store: &Store,
        context: &egui::Context,
        subjects: impl FnOnce() -> Vec<Subject>,
        u: &UnitPrefs,
    ) {
        let events = self
            .with(|shared| {
                let generation = shared.generation;
                std::mem::take(&mut shared.events)
                    .into_iter()
                    .filter(|(made_in, _)| *made_in == generation)
                    .map(|(_, event)| event)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Linked(link) => {
                    self.settings.link = Some(link);
                    self.next_ask = 0.0;
                }
                Event::Paired => {
                    if let Some(link) = &mut self.settings.link {
                        link.paired = true;
                    }
                }
                Event::Sent {
                    version,
                    sent,
                    names,
                    skipped,
                } => {
                    if let Some(link) = &mut self.settings.link {
                        link.version = version;
                        link.sent = sent;
                        link.names = names;
                        link.skipped = skipped;
                    }
                    self.failed = None;
                    self.next_ask = 0.0;
                }
                Event::NotSent(print) => {
                    let now = context.input(|input| input.time);
                    let wait = match &self.failed {
                        Some((failed, _, wait)) if *failed == print => {
                            (wait * 2.0).min(RETRY_MOST_S)
                        }
                        _ => RETRY_FIRST_S,
                    };
                    self.failed = Some((print, now + wait, wait));
                }
                Event::Forgotten(writer) => {
                    self.settings
                        .forgets
                        .retain(|forget| forget.writer != writer);
                }
                Event::Unlinked(why) => {
                    self.settings.link = None;
                    self.with(|shared| {
                        shared.generation += 1;
                        shared.status = Status::Failed(why);
                        shared.watch = WatchWord::default();
                    });
                }
            }
            self.save(store);
        }
        let now = context.input(|input| input.time);
        if self.busy() {
            return;
        }
        if !self.settings.forgets.is_empty() {
            if now >= self.next_forget {
                self.next_forget = now + FORGET_EVERY_S;
                self.spawn(context, forget(self.settings.forgets.clone()));
                return;
            }
            // Tried again even if nothing else draws a frame.
            context.request_repaint_after_secs((self.next_forget - now) as f32 + 0.05);
        }
        let Some(link) = self.settings.link.clone() else {
            return;
        };
        if !link.paired {
            if now >= self.next_ask {
                self.next_ask = now + ASK_PAIRING_S;
                self.spawn(context, ask(link));
            }
            context.request_repaint_after_secs(ASK_PAIRING_S as f32);
            return;
        }
        if now - self.panel_at < 1.0 && now >= self.next_ask {
            self.next_ask = now + ASK_PAIRED_S;
            self.spawn(context, ask(link));
            return;
        }
        if now < self.next_check {
            // Changes are settling: come back for them even if nothing else
            // draws a frame.
            if self.seen.is_some() {
                context.request_repaint_after_secs((self.next_check - now) as f32 + 0.05);
            }
            return;
        }
        self.next_check = now + CHECK_EVERY_S;
        let subjects = subjects();
        let print = fingerprint(&subjects, u, self.settings.farthest_m);
        if print == link.sent {
            self.seen = None;
            return;
        }
        // The same profiles failed a moment ago: wait before trying again.
        if let Some((failed, retry_at, _)) = &self.failed
            && *failed == print
            && now < *retry_at
        {
            context.request_repaint_after_secs((retry_at - now) as f32 + 0.05);
            return;
        }
        match &self.seen {
            Some((seen, since)) if *seen == print && now - since >= SETTLE_S => {
                self.seen = None;
                let job = update(link, subjects, u.clone(), self.settings.farthest_m, print);
                self.spawn(context, job);
            }
            Some((seen, _)) if *seen == print => {
                context.request_repaint_after_secs(CHECK_EVERY_S as f32);
            }
            _ => {
                self.seen = Some((print, now));
                context.request_repaint_after_secs(CHECK_EVERY_S as f32);
            }
        }
    }

    /// Runs a job, one at a time; its events count only while the link it
    /// was started for stands.
    fn spawn(
        &self,
        context: &egui::Context,
        job: impl FnOnce(Reporter) -> std::pin::Pin<Box<dyn JobFuture>>,
    ) {
        let Some(generation) = self.with(|shared| {
            shared.busy = true;
            shared.generation
        }) else {
            return;
        };
        let reporter = Reporter {
            shared: self.shared.clone(),
            context: context.clone(),
            generation,
        };
        let future = job(reporter.clone());
        http::spawn(async move {
            future.await;
            reporter.with(|shared| shared.busy = false);
        });
    }

    pub fn panel(&mut self, ui: &mut egui::Ui, store: &Store, subjects: &[Subject], u: &UnitPrefs) {
        self.panel_at = ui.ctx().input(|input| input.time);
        let (status, watch, busy) = self
            .with(|shared| (shared.status.clone(), shared.watch.clone(), shared.busy))
            .unwrap_or_default();
        if self.settings.link.is_some() {
            // What the watch has, asked about while the panel is open.
            ui.ctx().request_repaint_after_secs(ASK_PAIRED_S as f32);
        }
        match self.settings.link.clone() {
            None => self.unpaired(ui, store, subjects, u, &status, busy),
            Some(link) => self.paired(ui, store, subjects, u, &link, &status, &watch, busy),
        }
        ui.add_space(6.0);
        ui.weak(tr(
            "The profiles are encrypted on this device for that watch. The sync server \
             only passes them on and keeps nothing it can read.",
        ));
    }

    fn farthest(&mut self, ui: &mut egui::Ui, store: &Store, u: &UnitPrefs) {
        let distance = u.unit(Quantity::Distance);
        ui.horizontal(|ui| {
            ui.label(tr("Out to"));
            let mut shown = distance.show(self.settings.farthest_m).round();
            if ui
                .add(
                    egui::DragValue::new(&mut shown)
                        .range(
                            distance.show(SHORTEST_FARTHEST_M).round()
                                ..=distance.show(MOST_FARTHEST_M).round(),
                        )
                        .speed(10.0)
                        .suffix(format!(" {}", distance.label)),
                )
                .changed()
            {
                self.settings.farthest_m = distance
                    .base(shown)
                    .clamp(SHORTEST_FARTHEST_M, MOST_FARTHEST_M);
                self.save(store);
            }
        });
    }

    fn names(ui: &mut egui::Ui, names: impl Iterator<Item = String>) {
        for name in names {
            ui.label(format!("• {name}"));
        }
    }

    fn unpaired(
        &mut self,
        ui: &mut egui::Ui,
        store: &Store,
        subjects: &[Subject],
        u: &UnitPrefs,
        status: &Status,
        busy: bool,
    ) {
        ui.label(tr(
            "On the watch, open MX Shooting and choose Pair with phone. Type the code it \
             shows here.",
        ));
        ui.add_space(6.0);
        if subjects.is_empty() {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                tr("Add a rifle and a bullet for it first."),
            );
            return;
        }
        ui.label(RichText::new(tr("Every rifle and load goes to the watch:")).strong());
        Self::names(ui, subjects.iter().take(MAX_PROFILES).map(name_of));
        if subjects.len() > MAX_PROFILES {
            ui.weak(trf!("Only the first {} fit on the watch.", MAX_PROFILES));
        }
        ui.weak(tr(
            "On the watch you set the temperature; the watch reads the air pressure itself.",
        ));
        ui.add_space(6.0);
        self.farthest(ui, store, u);
        ui.horizontal(|ui| {
            ui.label(tr("Code"));
            ui.add(
                egui::TextEdit::singleline(&mut self.code)
                    .hint_text("1234 5678")
                    .desired_width(120.0),
            );
        });
        let code = parse_code(&self.code);
        if ui
            .add_enabled(
                code.is_some() && !busy,
                egui::Button::new(RichText::new(tr("Pair")).strong()),
            )
            .clicked()
            && let Some(code) = code
        {
            let job = pair(
                crate::sync::DEFAULT_SERVER.to_owned(),
                code,
                subjects.to_vec(),
                u.clone(),
                self.settings.farthest_m,
            );
            self.spawn(ui.ctx(), job);
        }
        match status {
            Status::Idle => {}
            Status::Working(what) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(what);
                });
            }
            Status::Failed(why) => {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paired(
        &mut self,
        ui: &mut egui::Ui,
        store: &Store,
        subjects: &[Subject],
        u: &UnitPrefs,
        link: &Link,
        status: &Status,
        watch: &WatchWord,
        busy: bool,
    ) {
        if !link.paired {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr("Sent. Waiting for the watch to say yes…"));
            });
            ui.weak(tr(
                "The watch lists the rifles and asks whether to use them. Check that they \
                 are yours.",
            ));
        } else {
            ui.label(RichText::new(tr("Paired with a watch.")).strong());
            let waiting = fingerprint(subjects, u, self.settings.farthest_m) != link.sent;
            if busy && waiting {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr("Sending the changes…"));
                });
            } else if waiting {
                ui.label(tr("Changes go to the watch in a moment."));
            } else if watch.version == Some(link.version) {
                ui.label(tr("The watch has the latest."));
            } else {
                ui.label(tr(
                    "Sent. The watch picks it up the next time MX Shooting opens on it.",
                ));
            }
            if watch.unreadable && watch.version != Some(link.version) {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tr("The watch couldn't read the last update."),
                );
            }
            if let Status::Failed(why) = status {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
        }
        ui.add_space(6.0);
        ui.label(tr("On the watch:"));
        Self::names(ui, link.names.iter().cloned());
        if !link.skipped.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, tr("Left out:"));
            for why in &link.skipped {
                ui.colored_label(ui.visuals().warn_fg_color, format!("• {why}"));
            }
        }
        if subjects.len() > MAX_PROFILES {
            ui.weak(trf!("Only the first {} fit on the watch.", MAX_PROFILES));
        }
        ui.add_space(6.0);
        self.farthest(ui, store, u);
        ui.add_space(6.0);
        let forget = if link.paired {
            ui.button(tr("Forget this watch")).on_hover_text(tr(
                "Stops sending to it and deletes its profiles from the sync server. The \
                 watch keeps what it has.",
            ))
        } else {
            ui.button(tr("Cancel pairing"))
        };
        if forget.clicked() {
            // Deleted on the server as soon as it hears (tried again until
            // it does).
            self.settings.forgets.push(Forget {
                server: link.server.clone(),
                writer: link.writer.clone(),
            });
            self.settings.link = None;
            self.next_forget = 0.0;
            self.save(store);
            self.with(|shared| {
                shared.generation += 1;
                shared.status = Status::Idle;
                shared.watch = WatchWord::default();
            });
        }
    }
}

/// A job's line back to the panel.
#[derive(Clone)]
struct Reporter {
    shared: Arc<Mutex<Shared>>,
    context: egui::Context,
    generation: u32,
}

impl Reporter {
    fn with(&self, then: impl FnOnce(&mut Shared)) {
        if let Ok(mut shared) = self.shared.lock() {
            then(&mut shared);
        }
        self.context.request_repaint();
    }

    /// Whether the link the job was started for still stands.
    fn current(&self) -> bool {
        self.shared
            .lock()
            .is_ok_and(|shared| shared.generation == self.generation)
    }

    fn event(&self, event: Event) {
        let generation = self.generation;
        self.with(|shared| shared.events.push((generation, event)));
    }

    fn status(&self, status: Status) {
        if self.current() {
            self.with(|shared| shared.status = status);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
trait JobFuture: Future<Output = ()> + Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<F: Future<Output = ()> + Send> JobFuture for F {}
#[cfg(target_arch = "wasm32")]
trait JobFuture: Future<Output = ()> {}
#[cfg(target_arch = "wasm32")]
impl<F: Future<Output = ()>> JobFuture for F {}

type Job = Box<dyn FnOnce(Reporter) -> std::pin::Pin<Box<dyn JobFuture>>>;

fn url(server: &str, path: &str) -> String {
    format!("{}/v1/watch/{path}", server.trim_end_matches('/'))
}

/// A request with the link's writer secret.
fn authorised(
    method: Method,
    server: &str,
    path: &str,
    writer: &str,
    body: Option<serde_json::Value>,
) -> Request {
    let mut headers = vec![("Authorization", format!("Bearer {writer}"))];
    if body.is_some() {
        headers.push(("Content-Type", "application/json".into()));
    }
    Request {
        method,
        url: url(server, path),
        headers,
        body: body.map(|body| body.to_string().into_bytes()),
    }
}

/// Pairs with the watch waiting on `code`: fetches its key, makes this
/// app's key and writer secret, and sends the profiles under the code.
fn pair(
    server: String,
    code: String,
    subjects: Vec<Subject>,
    u: UnitPrefs,
    farthest_m: f64,
) -> Job {
    Box::new(move |reporter| {
        Box::pin(async move {
            reporter.status(Status::Working(tr("Pairing…").into()));
            match pairing(&server, &code, &subjects, &u, farthest_m).await {
                Ok(link) => {
                    reporter.status(Status::Idle);
                    reporter.event(Event::Linked(link));
                }
                Err(why) => reporter.status(Status::Failed(why)),
            }
        })
    })
}

async fn pairing(
    server: &str,
    code: &str,
    subjects: &[Subject],
    u: &UnitPrefs,
    farthest_m: f64,
) -> Result<Link, String> {
    let built = build(subjects, u, farthest_m).await?;
    let profiles = &built.profiles;
    let address = url(server, &format!("pairings/{code}"));
    let response = http::fetch(Request {
        method: Method::Get,
        url: address.clone(),
        headers: Vec::new(),
        body: None,
    })
    .await?;
    if response.status != 200 {
        return Err(pairing_failure(response.status));
    }
    #[derive(Deserialize)]
    struct Waiting {
        key: String,
    }
    let watch_key = serde_json::from_slice::<Waiting>(&response.body)
        .ok()
        .map(|waiting| waiting.key)
        .ok_or_else(|| tr("The watch's answer can't be read.").to_owned())?;
    let watch = BASE64
        .decode(&watch_key)
        .ok()
        .and_then(|key| seal::public_key(&key))
        .ok_or_else(|| tr("The watch's answer can't be read.").to_owned())?;
    let secret = seal::new_secret();
    let writer = hex(&seal::random32());
    let version = now_s();
    let items = seal_all(profiles, &secret, &watch, version)?;
    let body = serde_json::json!({
        "key": BASE64.encode(seal::wire_key(&secret.public_key())),
        "writer": writer,
        "version": version,
        "items": items,
    });
    // Sent again, the same, if the answer is lost on the way: the server
    // gives the same answer to the same sender.
    let mut tries = 0;
    let response = loop {
        tries += 1;
        let response = http::fetch(Request {
            method: Method::Put,
            url: address.clone(),
            headers: vec![("Content-Type", "application/json".into())],
            body: Some(body.to_string().into_bytes()),
        })
        .await;
        match response {
            Ok(response) if response.status < 500 || tries >= 3 => break response,
            Err(why) if tries >= 3 => return Err(why),
            _ => http::sleep(2000).await,
        }
    };
    if response.status != 201 {
        return Err(pairing_failure(response.status));
    }
    Ok(Link {
        server: server.to_owned(),
        watch_key,
        secret: hex(&secret.to_bytes()),
        writer,
        version,
        sent: fingerprint(subjects, u, farthest_m),
        names: profiles
            .iter()
            .map(|profile| profile.name.clone())
            .collect(),
        skipped: built.skipped,
        paired: false,
    })
}

/// Sends the profiles again, as a newer version.
fn update(link: Link, subjects: Vec<Subject>, u: UnitPrefs, farthest_m: f64, print: String) -> Job {
    Box::new(move |reporter| {
        Box::pin(async move {
            match updating(&link, &subjects, &u, farthest_m).await {
                Ok((version, built)) => {
                    reporter.status(Status::Idle);
                    reporter.event(Event::Sent {
                        version,
                        sent: print,
                        names: built
                            .profiles
                            .iter()
                            .map(|profile| profile.name.clone())
                            .collect(),
                        skipped: built.skipped,
                    });
                }
                Err(Gone) => reporter.event(Event::Unlinked(gone_text().into())),
                Err(Failed(why)) => {
                    reporter.status(Status::Failed(why));
                    reporter.event(Event::NotSent(print));
                }
            }
        })
    })
}

enum UpdateError {
    /// The server doesn't know the link (any more).
    Gone,
    Failed(String),
}
use UpdateError::{Failed, Gone};

fn gone_text() -> &'static str {
    tr(
        "The watch isn't paired with this device any more: it was forgotten on the watch, \
         or not used for over a year. Pair again.",
    )
}

async fn updating(
    link: &Link,
    subjects: &[Subject],
    u: &UnitPrefs,
    farthest_m: f64,
) -> Result<(u32, Built), UpdateError> {
    let built = build(subjects, u, farthest_m).await.map_err(Failed)?;
    let unreadable =
        || Failed(tr("The link to the watch is broken. Forget it and pair again.").into());
    let watch = BASE64
        .decode(&link.watch_key)
        .ok()
        .and_then(|key| seal::public_key(&key))
        .ok_or_else(unreadable)?;
    let secret = from_hex(&link.secret)
        .and_then(|bytes| p256::SecretKey::from_slice(&bytes).ok())
        .ok_or_else(unreadable)?;
    let version = now_s().max(link.version.saturating_add(1)).min(MAX_VERSION);
    let items = seal_all(&built.profiles, &secret, &watch, version).map_err(Failed)?;
    let response = http::fetch(authorised(
        Method::Put,
        &link.server,
        "link",
        &link.writer,
        Some(serde_json::json!({ "version": version, "items": items })),
    ))
    .await
    .map_err(Failed)?;
    match response.status {
        200 => Ok((version, built)),
        404 => Err(Gone),
        status => Err(Failed(server_failure(status))),
    }
}

/// Deletes forgotten links on the server; each one it has heard of (or
/// never knew) is done with.
fn forget(forgets: Vec<Forget>) -> Job {
    Box::new(move |reporter| {
        Box::pin(async move {
            for forget in forgets {
                let request =
                    authorised(Method::Delete, &forget.server, "link", &forget.writer, None);
                if let Ok(response) = http::fetch(request).await
                    && response.status == 200
                {
                    reporter.event(Event::Forgotten(forget.writer));
                }
            }
        })
    })
}

/// What the server knows of the link: whether the watch has said yes,
/// and what it has.
#[derive(Deserialize)]
struct LinkStatus {
    done: Option<String>,
    watch_version: Option<u32>,
    watch_said: Option<String>,
}

fn ask(link: Link) -> Job {
    Box::new(move |reporter| {
        Box::pin(async move {
            let Ok(response) = http::fetch(authorised(
                Method::Get,
                &link.server,
                "link/status",
                &link.writer,
                None,
            ))
            .await
            else {
                return;
            };
            match response.status {
                200 => {}
                404 => {
                    reporter.event(Event::Unlinked(if link.paired {
                        gone_text().into()
                    } else {
                        tr(
                            "The code ran out without word from the watch. Check what it shows; \
                             if it isn't paired, get a new code and pair again.",
                        )
                        .into()
                    }));
                    return;
                }
                _ => return,
            }
            let Ok(status) = serde_json::from_slice::<LinkStatus>(&response.body) else {
                return;
            };
            match status.done.as_deref() {
                None => {}
                Some("used") => {
                    if !link.paired {
                        reporter.event(Event::Paired);
                    }
                }
                Some(outcome) => {
                    reporter.event(Event::Unlinked(outcome_text(outcome).into()));
                    return;
                }
            }
            if reporter.current() {
                reporter.with(|shared| {
                    shared.watch = WatchWord {
                        version: status.watch_version,
                        unreadable: status.watch_said.as_deref() == Some("unreadable"),
                    };
                });
            }
        })
    })
}

/// How a pairing the watch didn't take ended.
fn outcome_text(outcome: &str) -> &'static str {
    match outcome {
        "declined" => tr("The watch said no to the rifles."),
        "unreadable" => {
            tr("The watch couldn't read the rifles. Get a new code on the watch and pair again.")
        }
        _ => tr(
            "The watch stopped waiting before it got the rifles. Get a new code on the watch \
             and pair again.",
        ),
    }
}

fn pairing_failure(status: u16) -> String {
    match status {
        404 => tr(
            "No watch is waiting with this code. A code lasts ten minutes: get a new one on \
             the watch.",
        )
        .into(),
        409 => tr(
            "Something has already been sent to this code. Check what the watch shows, and \
             get a new code if it isn't yours.",
        )
        .into(),
        410 => {
            tr("The watch has stopped waiting with this code. Get a new one on the watch.").into()
        }
        status => server_failure(status),
    }
}

fn server_failure(status: u16) -> String {
    match status {
        429 => tr("The sync server is busy. Try again in a minute.").into(),
        507 => tr("The sync server is full. Try again later.").into(),
        status => trf!("The sync server answered {}.", status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::tests::demo_subject;

    /// The watch app's test vector (`watch/source/TestVector.mc`): fixed
    /// keys, IV and profile, what the app sends, and the holds the watch
    /// must show. Rewrite it with `MXSC_UPDATE_DOCS=1 cargo test -p mxsc
    /// watch`.
    #[test]
    fn watch_test_vector() {
        let watch = p256::SecretKey::from_bytes(&[0x11; 32].into()).unwrap();
        let app = p256::SecretKey::from_bytes(&[0x22; 32].into()).unwrap();
        let iv = [0x33; 16];
        let profile = WatchProfile::compute(&demo_subject(), &UnitPrefs::metric(), 400.0).unwrap();
        let version = 1_793_491_200;
        let plain = profile.encode(version, 0, 1);
        let sealed = seal::seal_with(&app, iv, &watch.public_key(), &plain);
        assert_eq!(
            seal::open(&watch, &app.public_key(), &sealed).as_deref(),
            Some(plain.as_slice())
        );
        let hold = |range: f64, wind: f64, clock: f64, t: f64, p: f64| {
            let hold = profile.hold(range, wind, clock, t, p).unwrap();
            format!(
                "[{range:.1}f, {wind:.1}f, {clock:.1}f, {t:.1}f, {p:.1}f, {:.4}f, {:.4}f]",
                hold.elevation_mrad, hold.windage_mrad
            )
        };
        let limit = |t: f64, p: f64| profile.ethical_limit(t, p).unwrap();
        let text = format!(
            "// Made by the app's tests (crates/app/src/watch/mod.rs, watch_test_vector):\n\
             // don't edit. Fixed keys, IV and profile: what the app sends, and what the\n\
             // watch must show: holds (range m, wind m/s, clock, °C, hPa, up mrad, right\n\
             // mrad) and ethical limits (°C, hPa, m).\n\
             import Toybox.Lang;\n\n\
             (:test)\n\
             module TestVector {{\n\
             \x20   // The watch's private key, big-endian.\n\
             \x20   const WATCH_SECRET = \"{}\";\n\
             \x20   const WATCH_KEY = \"{}\";\n\
             \x20   const APP_KEY = \"{}\";\n\
             \x20   const BOX = \"{}\";\n\
             \x20   const PROFILE = \"{}\";\n\
             \x20   const VERSION = {version};\n\
             \x20   const ID = \"{}\";\n\
             \x20   const NAME = \"{}\";\n\
             \x20   const HOLDS = [\n\
             \x20       {},\n\
             \x20       {},\n\
             \x20       {},\n\
             \x20       {},\n\
             \x20   ];\n\
             \x20   const LIMITS = [\n\
             \x20       [13.0f, 1050.0f, {:.2}f],\n\
             \x20       [0.0f, 990.0f, {:.2}f],\n\
             \x20   ];\n\
             }}\n",
            hex(&watch.to_bytes()),
            BASE64.encode(seal::wire_key(&watch.public_key())),
            BASE64.encode(seal::wire_key(&app.public_key())),
            BASE64.encode(&sealed),
            hex(&plain),
            hex(&profile.id),
            profile.name.replace('"', "\\\""),
            hold(100.0, 0.0, 12.0, 13.0, 1050.0),
            hold(347.0, 7.0, 2.0, -17.0, 1003.0),
            hold(255.0, 4.5, 9.0, 24.0, 931.0),
            hold(40.0, 3.0, 3.0, -40.0, 1100.0),
            limit(13.0, 1050.0),
            limit(0.0, 990.0),
        );
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../watch/source/TestVector.mc"
        );
        if std::env::var_os("MXSC_UPDATE_DOCS").is_some() {
            std::fs::write(path, &text).unwrap();
        }
        let written = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            written == text,
            "the watch's test vector is out of date: MXSC_UPDATE_DOCS=1 cargo test -p mxsc watch"
        );
    }

    #[test]
    fn every_profile_fits_in_a_box() {
        let subject = demo_subject();
        let u = UnitPrefs::metric();
        let profile = WatchProfile::compute(&subject, &u, MOST_FARTHEST_M).unwrap();
        let secret = seal::new_secret();
        let watch = seal::new_secret().public_key();
        let sealed = seal::seal(&secret, &watch, &profile.encode(1, 0, 1));
        assert!(sealed.len() <= seal::MAX_BOX, "{}", sealed.len());
    }

    #[test]
    fn fingerprints_ignore_the_weather_but_not_the_rifle() {
        let subject = demo_subject();
        let u = UnitPrefs::metric();
        let print = fingerprint(std::slice::from_ref(&subject), &u, 600.0);
        let mut windy = subject.clone();
        windy.conditions.wind_speed_mps = 8.0;
        windy.conditions.temperature_c = -12.0;
        assert_eq!(fingerprint(&[windy], &u, 600.0), print);
        let mut rezeroed = subject.clone();
        rezeroed.rifle.zero_range_m += 50.0;
        assert_ne!(fingerprint(&[rezeroed], &u, 600.0), print);
        assert_ne!(
            fingerprint(std::slice::from_ref(&subject), &u, 500.0),
            print
        );
        assert_ne!(
            fingerprint(&[subject], &UnitPrefs::imperial(), 600.0),
            print
        );
    }

    /// Pairs with a watch waiting on a sync server, then sends an update:
    /// `MXSC_WATCH_SERVER=http://127.0.0.1:8787 MXSC_WATCH_CODE=… cargo test
    /// -p mxsc -- --ignored pairs_with_a_waiting_watch`, writing the first
    /// profile's bytes (version 0) to `MXSC_WATCH_EXPECT` to compare.
    #[test]
    #[ignore = "needs a sync server and a waiting watch"]
    #[cfg(not(target_arch = "wasm32"))]
    fn pairs_with_a_waiting_watch() {
        let server = std::env::var("MXSC_WATCH_SERVER").unwrap();
        let code = parse_code(&std::env::var("MXSC_WATCH_CODE").unwrap()).unwrap();
        let subjects = vec![demo_subject()];
        let u = UnitPrefs::metric();
        if let Some(path) = std::env::var_os("MXSC_WATCH_EXPECT") {
            let profile = WatchProfile::compute(&subjects[0], &u, 600.0).unwrap();
            std::fs::write(path, profile.encode(0, 0, 1)).unwrap();
        }
        crate::http::spawn_blocking_for_tests(async move {
            let link = pairing(&server, &code, &subjects, &u, 600.0).await.unwrap();
            assert!(
                pairing(&server, &code, &subjects, &u, 600.0).await.is_err(),
                "a code works once"
            );
            // The watch says yes within half a minute.
            let ask = |link: Link| async move {
                let response = http::fetch(authorised(
                    Method::Get,
                    &link.server,
                    "link/status",
                    &link.writer,
                    None,
                ))
                .await
                .unwrap();
                assert_eq!(response.status, 200);
                serde_json::from_slice::<LinkStatus>(&response.body).unwrap()
            };
            let mut said = None;
            for _ in 0..30 {
                said = ask(link.clone()).await.done;
                if said.is_some() {
                    break;
                }
                crate::http::sleep(1000).await;
            }
            assert_eq!(said.as_deref(), Some("used"));
            // An update, and the watch picks it up.
            let mut longer = subjects.clone();
            longer[0].rifle.name.push_str(" (updated)");
            let Ok((version, _)) = updating(&link, &longer, &u, 300.0).await else {
                panic!("the update didn't go");
            };
            for _ in 0..30 {
                if ask(link.clone()).await.watch_version == Some(version) {
                    return;
                }
                crate::http::sleep(1000).await;
            }
            panic!("the watch never picked up the update");
        });
    }

    #[test]
    fn codes_are_eight_digits() {
        assert_eq!(parse_code("1234 5678").as_deref(), Some("12345678"));
        assert_eq!(parse_code("1234-5678").as_deref(), Some("12345678"));
        assert_eq!(parse_code("1234567"), None);
        assert_eq!(parse_code("1234 567x"), None);
        assert_eq!(parse_code("123456789"), None);
    }

    #[test]
    fn hex_goes_both_ways() {
        let bytes = seal::random32();
        assert_eq!(from_hex(&hex(&bytes)).as_deref(), Some(bytes.as_slice()));
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }
}

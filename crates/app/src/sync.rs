//! Sync with the hunter's other devices. Every device keeps the same
//! rifles, bullets, loads, scopes, targets and shot log, through one of:
//!
//! - **a sync key** and the app's sync server (see `server/` and
//!   `ballistics_store::sync`): no account; the copy is encrypted;
//! - **Google Drive**: the hunter's own Drive, in the app's hidden folder
//!   (see `google.rs`); sign in with the same account on each device.
//!
//! A round: read the shared copy, merge it here, write the merged copy back
//! if the shared one lacks anything. Rounds run on start, a moment after a
//! change, when the app comes back to the front and every few minutes.
//! Everything is saved on the device first, so the app works offline and
//! catches up later.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use ballistics_store::{ImportReport, Store, SyncKey};
use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};

use crate::google;
use crate::http::{self, Method, Request};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::widgets::confirm_button;

/// The device-local setting with the key or Google sign-in (never exported).
const SETTING: &str = "sync";
/// The sync server, unless the build names another (`MXSC_SYNC_URL`).
pub const DEFAULT_SERVER: &str = match option_env!("MXSC_SYNC_URL") {
    Some(url) => url,
    None => "https://mxsc-sync.malx122.workers.dev",
};
/// Where the web app lives, for pairing links.
const APP_URL: &str = "https://malx122.github.io/mx_shooting_calculator/";
/// Wait this long after a change, so a burst of edits goes in one round.
const AFTER_CHANGE_S: f64 = 2.0;
/// Sync this often while the app is open.
const EVERY_S: f64 = 300.0;
/// After a failure, try again this much later.
const RETRY_S: f64 = 60.0;
/// Give up a round after this many collisions with other devices.
const MAX_CONFLICTS: u32 = 5;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "via", rename_all = "snake_case")]
enum Settings {
    Key {
        key: String,
        server: String,
    },
    // Earlier versions kept the account's e-mail address here too; it is
    // no longer asked for, and an old one is ignored.
    Google {
        refresh_token: String,
        server: String,
    },
}

impl Settings {
    fn server(&self) -> &str {
        match self {
            Settings::Key { server, .. } | Settings::Google { server, .. } => {
                server.trim_end_matches('/')
            }
        }
    }
}

/// Where the shared copy is.
#[derive(Clone)]
enum Target {
    Key(SyncKey),
    Google(google::Shared),
}

/// The shared copy and its version (the sync server) or file id (Drive),
/// or `None` before any device wrote one.
type Copy = Option<(Vec<u8>, Option<String>)>;

enum Reply {
    Read(Result<Copy, Failure>),
    Wrote(Result<(), Failure>),
    Deleted(Result<(), Failure>),
    /// A Google sign-in finished (for the attempt numbered).
    SignedIn(u32, Result<google::Account, String>),
}

enum Failure {
    /// Another device wrote in between.
    Conflict,
    /// The shared copy was deleted.
    Gone,
    /// Google wants the hunter to sign in again, with why.
    SignIn(String),
    Other(String),
}

impl From<google::Trouble> for Failure {
    fn from(trouble: google::Trouble) -> Failure {
        match trouble {
            google::Trouble::SignIn(why) => Failure::SignIn(why),
            google::Trouble::Other(error) => Failure::Other(error),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Reading,
    Writing,
    Deleting,
}

/// A Google sign-in going on in the browser.
struct SigningIn {
    attempt: u32,
    url: String,
    /// Typed codes go here (on the web).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pick_up: google::PickUp,
    /// The pick-up code as being typed, from the sign-in page.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    typed: String,
}

/// What the app should do after a frame of sync.
pub enum Event {
    /// Data came in from other devices: reload.
    Merged(ImportReport),
    /// Sync was turned off, with why.
    Stopped(String),
}

pub struct Sync {
    settings: Option<Settings>,
    target: Option<Target>,
    phase: Phase,
    sender: Sender<Reply>,
    receiver: Receiver<Reply>,
    /// When the last unsynced change was made.
    changed_at: Option<f64>,
    /// When the next scheduled round is due.
    due_at: f64,
    last_ok: Option<f64>,
    error: Option<String>,
    conflicts: u32,
    focused: bool,
    /// Just signed in to Google: a copy deleted earlier may be replaced.
    fresh_start: bool,
    /// Google wants the hunter to sign in again; no rounds until then.
    needs_sign_in: bool,
    signing_in: Option<SigningIn>,
    sign_in_attempts: u32,
    /// What the hunter typed to join.
    join_text: String,
    join_error: Option<String>,
    /// A key from a pairing link, waiting for a yes.
    pub offered: Option<SyncKey>,
    /// The desktop window is open.
    pub window_open: bool,
}

impl Sync {
    pub fn new(store: &Store) -> Sync {
        let settings: Option<Settings> = store.setting(SETTING).ok().flatten();
        let target = settings.as_ref().and_then(|settings| match settings {
            Settings::Key { key, .. } => SyncKey::parse(key).map(Target::Key),
            Settings::Google { refresh_token, .. } => Some(Target::Google(Arc::new(Mutex::new(
                google::Account::new(refresh_token.clone()),
            )))),
        });
        let (sender, receiver) = channel();
        Sync {
            settings: target.as_ref().and(settings),
            target,
            phase: Phase::Idle,
            sender,
            receiver,
            changed_at: None,
            due_at: 0.0,
            last_ok: None,
            error: None,
            conflicts: 0,
            focused: true,
            fresh_start: false,
            needs_sign_in: false,
            signing_in: None,
            sign_in_attempts: 0,
            join_text: String::new(),
            join_error: None,
            offered: None,
            window_open: false,
        }
    }

    pub fn is_on(&self) -> bool {
        self.target.is_some()
    }

    /// The sync key, when syncing with one.
    fn key(&self) -> Option<&SyncKey> {
        match &self.target {
            Some(Target::Key(key)) => Some(key),
            _ => None,
        }
    }

    /// The hunter changed something: sync it soon.
    pub fn changed(&mut self, now: f64) {
        if self.is_on() {
            self.changed_at = Some(now);
        }
    }

    /// Runs the sync: collects answers, starts rounds that are due and
    /// schedules the next frame. Call every frame.
    pub fn poll(&mut self, store: &mut Store, context: &egui::Context) -> Option<Event> {
        let now = context.input(|input| input.time);
        let mut event = None;
        while let Ok(reply) = self.receiver.try_recv() {
            if let Some(happened) = self.on_reply(reply, store, context, now) {
                event = Some(happened);
            }
        }
        if !self.is_on() || self.needs_sign_in {
            return event;
        }
        // Coming back to the front is a good moment to catch up.
        let focused = context.input(|input| input.focused);
        if focused && !self.focused {
            self.due_at = self.due_at.min(now);
        }
        self.focused = focused;
        if self.phase == Phase::Idle {
            let change_due = self.changed_at.map(|at| at + AFTER_CHANGE_S);
            let next = change_due.map_or(self.due_at, |due| due.min(self.due_at));
            if now >= next {
                self.changed_at = None;
                self.due_at = now + EVERY_S;
                self.read(context);
            } else {
                context.request_repaint_after(std::time::Duration::from_secs_f64(next - now));
            }
        }
        event
    }

    fn server(&self) -> String {
        self.settings
            .as_ref()
            .map_or(DEFAULT_SERVER, Settings::server)
            .to_owned()
    }

    fn copy_url(&self, key: &SyncKey) -> String {
        format!("{}/v1/copies/{}", self.server(), key.blob_id())
    }

    /// Sends a job's answer to the UI.
    fn reply_with(&self, context: &egui::Context) -> impl Fn(Reply) + Send + 'static {
        let sender = self.sender.clone();
        let context = context.clone();
        move |reply| {
            let _ = sender.send(reply);
            context.request_repaint();
        }
    }

    fn read(&mut self, context: &egui::Context) {
        let Some(target) = self.target.clone() else {
            return;
        };
        self.phase = Phase::Reading;
        let reply = self.reply_with(context);
        match target {
            Target::Key(key) => {
                let request = Request {
                    method: Method::Get,
                    url: self.copy_url(&key),
                    headers: Vec::new(),
                    body: None,
                };
                http::send(request, context.clone(), move |result| {
                    reply(Reply::Read(match result {
                        Ok(response) if response.status == 200 => {
                            Ok(Some((response.body, response.etag)))
                        }
                        Ok(response) if response.status == 404 => Ok(None),
                        Ok(response) => Err(failure(response.status)),
                        Err(error) => Err(Failure::Other(error)),
                    }));
                });
            }
            Target::Google(account) => {
                let server = self.server();
                http::spawn(async move {
                    let result = google::read(&server, &account).await;
                    reply(Reply::Read(
                        result
                            .map(|copy| copy.map(|(bytes, id)| (bytes, Some(id))))
                            .map_err(Failure::from),
                    ));
                });
            }
        }
    }

    fn write(&mut self, upload: Vec<u8>, version: Option<String>, context: &egui::Context) {
        let Some(target) = self.target.clone() else {
            return;
        };
        self.phase = Phase::Writing;
        let reply = self.reply_with(context);
        match target {
            Target::Key(key) => {
                let condition = match version {
                    Some(etag) => ("If-Match", etag),
                    None => ("If-None-Match", "*".to_owned()),
                };
                let request = Request {
                    method: Method::Put,
                    url: self.copy_url(&key),
                    headers: vec![
                        condition,
                        ("Content-Type", "application/octet-stream".to_owned()),
                    ],
                    body: Some(upload),
                };
                http::send(request, context.clone(), move |result| {
                    reply(Reply::Wrote(match result {
                        Ok(response) if (200..300).contains(&response.status) => Ok(()),
                        Ok(response) => Err(failure(response.status)),
                        Err(error) => Err(Failure::Other(error)),
                    }));
                });
            }
            Target::Google(account) => {
                let server = self.server();
                http::spawn(async move {
                    let result = google::write(&server, &account, version, upload).await;
                    reply(Reply::Wrote(result.map_err(Failure::from)));
                });
            }
        }
    }

    fn on_reply(
        &mut self,
        reply: Reply,
        store: &mut Store,
        context: &egui::Context,
        now: f64,
    ) -> Option<Event> {
        if let Reply::SignedIn(attempt, result) = reply {
            if self
                .signing_in
                .as_ref()
                .is_some_and(|s| s.attempt == attempt)
            {
                self.signing_in = None;
                match result {
                    Ok(account) => {
                        if let Err(error) = self.turn_on_google(store, account) {
                            self.join_error = Some(error);
                        }
                    }
                    Err(error) => self.join_error = Some(error),
                }
            }
            return None;
        }
        self.target.as_ref()?;
        match reply {
            Reply::SignedIn(..) => None,
            Reply::Read(Ok(copy)) => {
                let (mut bytes, version) = match copy {
                    Some((bytes, version)) => (Some(bytes), version),
                    None => (None, None),
                };
                if bytes.as_deref() == Some(google::DELETED) {
                    if !self.fresh_start {
                        return self.stopped_elsewhere(store);
                    }
                    // Signed in again after deleting: start the copy afresh.
                    bytes = None;
                }
                match store.sync_round(self.key(), bytes.as_deref()) {
                    Ok(round) => {
                        match round.upload {
                            Some(upload) => self.write(upload, version, context),
                            None => self.succeeded(now),
                        }
                        round
                            .report
                            .changed()
                            .then_some(Event::Merged(round.report))
                    }
                    Err(error) => {
                        self.failed(now, error.to_string());
                        None
                    }
                }
            }
            Reply::Wrote(Ok(())) => {
                self.succeeded(now);
                None
            }
            Reply::Read(Err(Failure::Conflict)) | Reply::Wrote(Err(Failure::Conflict)) => {
                // Another device wrote first: read its copy and merge again.
                self.conflicts += 1;
                if self.conflicts < MAX_CONFLICTS {
                    self.read(context);
                } else {
                    self.failed(now, tr("other devices keep writing; will try again").into());
                }
                None
            }
            Reply::Read(Err(Failure::Gone)) | Reply::Wrote(Err(Failure::Gone)) => {
                self.stopped_elsewhere(store)
            }
            Reply::Read(Err(Failure::SignIn(why))) | Reply::Wrote(Err(Failure::SignIn(why))) => {
                self.phase = Phase::Idle;
                self.needs_sign_in = true;
                self.error = Some(why);
                None
            }
            Reply::Read(Err(Failure::Other(error))) | Reply::Wrote(Err(Failure::Other(error))) => {
                self.failed(now, error);
                None
            }
            Reply::Deleted(result) => {
                self.phase = Phase::Idle;
                match result {
                    Ok(()) | Err(Failure::Gone) => {
                        self.turn_off(store);
                        Some(Event::Stopped(
                            tr(
                                "The synced copy is deleted and sync is off. Your data on this \
                                device is kept.",
                            )
                            .into(),
                        ))
                    }
                    Err(_) => {
                        self.error = Some(tr("could not delete the synced copy; try again").into());
                        None
                    }
                }
            }
        }
    }

    fn stopped_elsewhere(&mut self, store: &Store) -> Option<Event> {
        self.turn_off(store);
        Some(Event::Stopped(
            tr(
                "The synced copy was deleted on another device, so sync is off here. Your data \
                on this device is kept.",
            )
            .into(),
        ))
    }

    fn succeeded(&mut self, now: f64) {
        self.phase = Phase::Idle;
        self.last_ok = Some(now);
        self.error = None;
        self.conflicts = 0;
        self.fresh_start = false;
    }

    fn failed(&mut self, now: f64, error: String) {
        self.phase = Phase::Idle;
        self.error = Some(error);
        self.conflicts = 0;
        self.due_at = now + RETRY_S;
    }

    fn save(&mut self, store: &Store, settings: Settings, target: Target) -> Result<(), String> {
        store
            .set_setting(SETTING, &Some(settings.clone()))
            .map_err(|error| error.to_string())?;
        self.settings = Some(settings);
        self.target = Some(target);
        self.error = None;
        self.join_error = None;
        self.needs_sign_in = false;
        self.last_ok = None;
        self.due_at = 0.0;
        Ok(())
    }

    /// Starts syncing with `key`: a new one, or another device's.
    fn turn_on(&mut self, store: &Store, key: SyncKey) -> Result<(), String> {
        let settings = Settings::Key {
            key: key.to_string(),
            server: DEFAULT_SERVER.to_owned(),
        };
        self.fresh_start = false;
        self.save(store, settings, Target::Key(key))
    }

    fn turn_on_google(&mut self, store: &Store, account: google::Account) -> Result<(), String> {
        let settings = Settings::Google {
            refresh_token: account.refresh_token.clone(),
            server: self.server(),
        };
        self.fresh_start = true;
        self.save(
            store,
            settings,
            Target::Google(Arc::new(Mutex::new(account))),
        )
    }

    fn turn_off(&mut self, store: &Store) {
        let _ = store.set_setting::<Option<Settings>>(SETTING, &None);
        self.settings = None;
        self.target = None;
        self.phase = Phase::Idle;
        self.error = None;
        self.last_ok = None;
        self.needs_sign_in = false;
        self.fresh_start = false;
    }

    /// Opens Google's sign-in in the browser and waits for it to finish.
    fn sign_in(&mut self, context: &egui::Context) {
        self.sign_in_attempts += 1;
        let attempt = self.sign_in_attempts;
        let session = uuid_hex();
        let server = self.server();
        let pick_up = google::PickUp::default();
        // The pick-up code comes back to this app only: to a port the
        // desktop app listens on, or as a message to the web app's window.
        #[cfg(not(target_arch = "wasm32"))]
        let reply_to = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                let port = listener
                    .local_addr()
                    .map(|address| address.port())
                    .unwrap_or(0);
                let (session, pick_up) = (session.clone(), std::sync::Arc::clone(&pick_up));
                std::thread::spawn(move || google::listen(listener, session, pick_up));
                port.to_string()
            }
            Err(error) => {
                self.join_error = Some(error.to_string());
                return;
            }
        };
        #[cfg(target_arch = "wasm32")]
        let reply_to = match crate::web::listen_for_google(&session, &pick_up) {
            Some(reply_to) => reply_to,
            None => {
                self.join_error =
                    Some(tr("Google sign-in works only in the app's own web address").into());
                return;
            }
        };
        let url = google::sign_in_url(&server, &session, &reply_to);
        http::open_in_browser(context, &url);
        self.signing_in = Some(SigningIn {
            attempt,
            url,
            pick_up: std::sync::Arc::clone(&pick_up),
            typed: String::new(),
        });
        self.join_error = None;
        let reply = self.reply_with(context);
        http::spawn(async move {
            let result = google::finish_sign_in(server, pick_up).await;
            reply(Reply::SignedIn(attempt, result));
        });
    }

    fn delete_shared(&mut self, context: &egui::Context) {
        let Some(target) = self.target.clone() else {
            return;
        };
        self.phase = Phase::Deleting;
        let reply = self.reply_with(context);
        match target {
            Target::Key(key) => {
                let request = Request {
                    method: Method::Delete,
                    url: self.copy_url(&key),
                    headers: Vec::new(),
                    body: None,
                };
                http::send(request, context.clone(), move |result| {
                    reply(Reply::Deleted(match result {
                        Ok(response) if (200..300).contains(&response.status) => Ok(()),
                        Ok(response) => Err(failure(response.status)),
                        Err(error) => Err(Failure::Other(error)),
                    }));
                });
            }
            Target::Google(account) => {
                let server = self.server();
                http::spawn(async move {
                    // Leave a mark, so the other devices stop too.
                    let result =
                        google::write(&server, &account, None, google::DELETED.to_vec()).await;
                    reply(Reply::Deleted(result.map_err(Failure::from)));
                });
            }
        }
    }

    /// One line on how sync is doing.
    pub fn status(&self, now: f64) -> String {
        if self.signing_in.is_some() {
            return tr("Signing in to Google…").into();
        }
        if !self.is_on() {
            return tr("Off").into();
        }
        match (self.phase, &self.error, self.last_ok) {
            (Phase::Reading | Phase::Writing, _, _) => tr("Syncing…").into(),
            (Phase::Deleting, _, _) => tr("Deleting the synced copy…").into(),
            (_, Some(error), _) => trf!("Not synced: {error}", error = error),
            (_, None, Some(at)) => match ((now - at) / 60.0).floor() as i64 {
                0 => tr("Synced just now").into(),
                1 => tr("Synced a minute ago").into(),
                minutes => trf!("Synced {minutes} minutes ago", minutes = minutes),
            },
            (_, None, None) => tr("Waiting to sync").into(),
        }
    }

    /// A word for the header, next to the dot.
    pub fn short_status(&self) -> &'static str {
        if self.signing_in.is_some() || matches!(self.phase, Phase::Reading | Phase::Writing) {
            tr("syncing")
        } else if !self.is_on() {
            tr("sync off")
        } else if self.error.is_some() {
            tr("not synced")
        } else {
            tr("synced")
        }
    }

    /// The colour of the header's sync dot.
    pub fn colour(&self) -> egui::Color32 {
        if !self.is_on() {
            theme::BONE_MUTED
        } else if self.error.is_some() {
            theme::BLAZE
        } else {
            theme::MEADOW
        }
    }

    /// The sync settings: start (with Google or a key), join, or how this
    /// device syncs.
    pub fn panel(&mut self, ui: &mut egui::Ui, store: &Store) {
        let now = ui.ctx().input(|input| input.time);
        let context = ui.ctx().clone();
        if let Some(signing_in) = &mut self.signing_in {
            ui.label(tr(
                "Finish signing in to Google in your browser; this updates by itself.",
            ));
            let mut cancel = false;
            ui.horizontal(|ui| {
                ui.hyperlink_to(tr("Open the sign-in page again"), &signing_in.url);
                cancel = ui.button(tr("Cancel")).clicked();
            });
            // An app on a phone's home screen may not hear back from the
            // sign-in page, which then shows the code to type here.
            #[cfg(target_arch = "wasm32")]
            {
                ui.add_space(6.0);
                ui.weak(tr("If it doesn't, enter the code the sign-in page shows:"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut signing_in.typed)
                            .hint_text("XXXXX-XXXXX")
                            .desired_width(130.0),
                    );
                    let code = google::pick_up_code(&signing_in.typed);
                    if ui
                        .add_enabled(code.is_some(), egui::Button::new(tr("Use code")))
                        .clicked()
                        && let Ok(mut slot) = signing_in.pick_up.lock()
                    {
                        *slot = code;
                    }
                });
            }
            if cancel {
                self.signing_in = None;
            }
            return;
        }
        match self.target.clone() {
            None => self.off_panel(ui, store, &context),
            Some(Target::Key(key)) => {
                self.status_line(ui, now);
                ui.label(tr(
                    "To sync another device, scan this with its camera, or enter the key there:",
                ));
                let link = format!("{APP_URL}#sync={}", key.compact());
                qr_code(ui, &link, 168.0);
                ui.label(RichText::new(key.to_string()).monospace().strong());
                ui.horizontal(|ui| {
                    if ui.button(tr("Copy key")).clicked() {
                        ui.ctx().copy_text(key.to_string());
                    }
                    if ui.button(tr("Copy link")).clicked() {
                        ui.ctx().copy_text(link.clone());
                    }
                });
                ui.add_space(4.0);
                ui.weak(tr(
                    "Keep the key to yourself: anyone with it can read and change your synced \
                     data. If you lose every device and the key, the synced copy can't be \
                     recovered, so export a backup now and then.",
                ));
                ui.add_space(8.0);
                self.buttons(ui, store, &context, now);
            }
            Some(Target::Google(_)) => {
                self.status_line(ui, now);
                ui.label(tr("Syncing through Google Drive."));
                ui.weak(tr(
                    "Your data is kept in a hidden folder in your own Google Drive that only \
                     this app can reach. Use the same Google account on every device: each \
                     account keeps a copy of its own.",
                ));
                if self.needs_sign_in
                    && ui
                        .button(RichText::new(tr("Sign in again")).strong())
                        .clicked()
                {
                    self.sign_in(&context);
                }
                ui.add_space(8.0);
                self.buttons(ui, store, &context, now);
            }
        }
    }

    fn status_line(&self, ui: &mut egui::Ui, now: f64) {
        let status = self.status(now);
        if self.error.is_some() {
            ui.colored_label(ui.visuals().error_fg_color, status);
        } else {
            ui.label(RichText::new(status).color(theme::MEADOW));
        }
        ui.add_space(8.0);
    }

    fn buttons(&mut self, ui: &mut egui::Ui, store: &Store, context: &egui::Context, now: f64) {
        ui.horizontal_wrapped(|ui| {
            let busy = self.phase != Phase::Idle;
            if ui
                .add_enabled(
                    !busy && !self.needs_sign_in,
                    egui::Button::new(tr("Sync now")),
                )
                .clicked()
            {
                self.due_at = now;
                context.request_repaint();
            }
            if ui
                .button(tr("Stop syncing here"))
                .on_hover_text(tr(
                    "This device keeps its data; the other devices go on syncing",
                ))
                .clicked()
            {
                self.turn_off(store);
            }
            if !busy
                && !self.needs_sign_in
                && confirm_button(ui, "delete-synced-copy", tr("Delete the synced copy"))
            {
                self.delete_shared(context);
            }
        });
    }

    fn off_panel(&mut self, ui: &mut egui::Ui, store: &Store, context: &egui::Context) {
        ui.label(tr(
            "Keep your rifles, bullets, scopes, targets and shot log the same on your phone \
             and computer.",
        ));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("With Google Drive")).strong());
        ui.label(tr(
            "Kept in a hidden folder in your own Drive. Sign in on each device, with the \
             same Google account on all of them.",
        ));
        if ui.button(tr("Sign in with Google")).clicked() {
            self.sign_in(context);
        }
        ui.add_space(10.0);
        ui.label(RichText::new(tr("With a sync key, no account")).strong());
        ui.label(tr(
            "Encrypted on this device before it is sent; the sync server can't read it. \
             Pair other devices with a QR code.",
        ));
        if ui
            .button(tr("Start syncing with a key"))
            .on_hover_text(tr("Makes a new sync key for your devices"))
            .clicked()
            && let Err(error) = self.turn_on(store, SyncKey::generate())
        {
            self.join_error = Some(error);
        }
        ui.add_space(6.0);
        ui.label(tr("Already syncing with a key on another device? Scan its code, open its link, or type its key:"));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.join_text)
                    .hint_text("XXXX-XXXX-…")
                    .desired_width(240.0),
            );
            if ui.button(tr("Join")).clicked() {
                match SyncKey::parse(&self.join_text) {
                    Some(key) => {
                        self.join_text.clear();
                        self.join_error = self.turn_on(store, key).err();
                    }
                    None => self.join_error = Some(tr("that is not a sync key").into()),
                }
            }
        });
        if let Some(error) = &self.join_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    /// Asks before joining the sync a pairing link offered.
    pub fn offer_window(&mut self, context: &egui::Context, store: &Store) {
        let Some(key) = self.offered.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(tr("Sync this device?"))
            .id(egui::Id::new("sync-offer"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut open)
            .show(context, |ui| {
                ui.set_max_width(360.0);
                if self.key() == Some(&key) {
                    ui.label(tr("This device already syncs with that key."));
                    if ui.button(tr("OK")).clicked() {
                        self.offered = None;
                    }
                    return;
                }
                ui.label(tr(
                    "The link holds a sync key. This device's rifles, bullets, scopes, targets \
                     and shot log will be merged with the other devices' and kept the same \
                     from now on.",
                ));
                if self.is_on() {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        tr("This device syncs another way now; it will switch."),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.button(RichText::new(tr("Sync")).strong()).clicked() {
                        self.join_error = self.turn_on(store, key.clone()).err();
                        self.offered = None;
                    }
                    if ui.button(tr("Not now")).clicked() {
                        self.offered = None;
                    }
                });
            });
        if !open {
            self.offered = None;
        }
    }
}

/// 32 random hex digits.
fn uuid_hex() -> String {
    ballistics_store::model::new_id().replace('-', "")
}

fn failure(status: u16) -> Failure {
    match status {
        412 => Failure::Conflict,
        410 => Failure::Gone,
        413 => Failure::Other(tr("your data is too large for the sync server").into()),
        429 => Failure::Other(tr("the sync server is busy; will try again").into()),
        507 => Failure::Other(
            tr("the sync server is full and takes no new synced copies just now").into(),
        ),
        status => Failure::Other(trf!("the sync server answered {status}", status = status)),
    }
}

/// A QR code for `text`, `size` points square, on white with a margin.
fn qr_code(ui: &mut egui::Ui, text: &str, size: f32) {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        return;
    };
    let modules = code.width();
    let colours = code.to_colors();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::WHITE);
    // A quiet zone of four modules around the code.
    let module = size / (modules + 8) as f32;
    let origin = rect.min + egui::vec2(4.0 * module, 4.0 * module);
    for (index, colour) in colours.iter().enumerate() {
        if *colour == qrcode::Color::Dark {
            let (x, y) = ((index % modules) as f32, (index / modules) as f32);
            painter.rect_filled(
                egui::Rect::from_min_size(
                    origin + egui::vec2(x * module, y * module),
                    egui::vec2(module + 0.3, module + 0.3),
                ),
                0.0,
                egui::Color32::BLACK,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_google_sign_in_saved_with_an_email_address_still_loads() {
        let saved = r#"{"via":"google","refresh_token":"token","email":"a@example.com","server":"https://example.com"}"#;
        let settings: Settings = serde_json::from_str(saved).unwrap();
        assert!(
            matches!(settings, Settings::Google { refresh_token, .. } if refresh_token == "token")
        );
        let written = serde_json::to_string(&Settings::Google {
            refresh_token: "token".into(),
            server: "https://example.com".into(),
        })
        .unwrap();
        assert!(!written.contains("email"));
    }
}

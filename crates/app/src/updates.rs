//! Updates for the desktop app, when the hunter turns them on (they are off
//! until then). The app fetches the latest release's update manifest from
//! GitHub and checks its signature against the key built into the app. On
//! the hunter's word it downloads the new version for this system, checks
//! it against the manifest, puts it in place of this one and starts it.
//!
//! The release workflow (`.github/workflows/desktop.yml`) writes the
//! manifest, `update.json`, and signs it with `tools/sign-update`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, RichText};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::i18n::{tr, trf};

const REPOSITORY: &str = "https://github.com/malx122/mx_shooting_calculator";
/// The minisign key the release workflow signs update manifests with; its
/// secret half is a GitHub secret of the repository.
const PUBLIC_KEY: &str = "RWQOT8bt+OOxmARrudjcC3zVXPNbhl40rijMDwBcgWpjuX8Xoj5R2j/a";
/// Where releases come from: GitHub, or, to test the updater,
/// `MXSC_UPDATE_FROM` (a server laid out the same way; the signature is
/// still checked).
fn source() -> String {
    std::env::var("MXSC_UPDATE_FROM").unwrap_or_else(|_| REPOSITORY.to_owned())
}

/// The setting that turns the check at start on.
pub const SETTING: &str = "check_for_updates";

/// This system's name in the manifest; none where no build is made.
const PLATFORM: Option<&str> = if cfg!(target_os = "macos") {
    Some("macos")
} else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    Some("windows-x64")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x64")
} else {
    None
};

/// What a release offers: its version, and the update file for each system.
#[derive(Clone, Debug, Deserialize)]
struct Manifest {
    version: String,
    files: HashMap<String, UpdateFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateFile {
    name: String,
    /// Hex.
    sha256: String,
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    file: UpdateFile,
}

#[derive(Clone, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(String),
    /// The new version is in place; `start` starts it.
    Installed {
        version: String,
        start: PathBuf,
    },
    /// `installing`: it failed while updating, not while checking.
    Failed {
        message: String,
        installing: bool,
    },
}

#[derive(Default)]
pub struct Updater {
    state: Arc<Mutex<State>>,
    /// The hunter put off the update offered this session.
    later: bool,
}

impl Updater {
    pub fn state(&self) -> State {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    fn busy(&self) -> bool {
        matches!(self.state(), State::Checking | State::Downloading(_))
    }

    /// Asks GitHub for the latest version, in the background.
    pub fn check(&mut self, context: &egui::Context) {
        if self.busy() {
            return;
        }
        self.run(context, State::Checking, || {
            latest().unwrap_or_else(|message| State::Failed {
                message,
                installing: false,
            })
        });
    }

    /// Downloads the version offered and puts it in place, in the
    /// background.
    fn install(&mut self, context: &egui::Context) {
        let State::Available(release) = self.state() else {
            return;
        };
        let version = release.version.clone();
        self.run(
            context,
            State::Downloading(version.clone()),
            move || match download_and_install(&release) {
                Ok(start) => State::Installed { version, start },
                Err(message) => State::Failed {
                    message,
                    installing: true,
                },
            },
        );
    }

    fn run(
        &mut self,
        context: &egui::Context,
        meanwhile: State,
        job: impl FnOnce() -> State + Send + 'static,
    ) {
        if let Ok(mut state) = self.state.lock() {
            *state = meanwhile;
        }
        let state = Arc::clone(&self.state);
        let context = context.clone();
        // The messages in the hunter's language: it is kept per thread.
        let language = crate::i18n::current();
        std::thread::spawn(move || {
            crate::i18n::set(language);
            let done = job();
            if let Ok(mut state) = state.lock() {
                *state = done;
            }
            context.request_repaint();
        });
    }

    /// Starts the new version and closes this one.
    fn restart(&self, context: &egui::Context) -> Result<(), String> {
        let State::Installed { start, .. } = self.state() else {
            return Ok(());
        };
        #[cfg(target_os = "macos")]
        let started = std::process::Command::new("/usr/bin/open")
            .arg("-n")
            .arg(&start)
            .spawn();
        #[cfg(not(target_os = "macos"))]
        let started = std::process::Command::new(&start).spawn();
        started.map_err(|error| trf!("Couldn't start the new version: {}", error))?;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
        Ok(())
    }

    /// The state and what can be done about it, for the Updates sheet.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self.state() {
            State::Idle => {}
            State::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr("Checking…"));
                });
            }
            State::UpToDate => {
                ui.label(tr("You have the latest version."));
            }
            State::Failed { message, .. } => {
                ui.colored_label(ui.visuals().error_fg_color, message);
            }
            _ => self.offer(ui),
        }
    }

    /// A line at the bottom of the window while an update is offered,
    /// downloading or ready to start.
    pub fn notice(&mut self, context: &egui::Context) {
        let wanted = match self.state() {
            State::Available(_) => !self.later,
            State::Downloading(_) | State::Installed { .. } => true,
            State::Failed { installing, .. } => installing,
            _ => false,
        };
        if !wanted {
            return;
        }
        egui::TopBottomPanel::bottom("update").show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                if let State::Failed { message, .. } = self.state() {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                    if ui.small_button("✖").clicked()
                        && let Ok(mut state) = self.state.lock()
                    {
                        *state = State::Idle;
                    }
                } else {
                    self.offer(ui);
                }
            });
        });
    }

    /// The update offered, being downloaded, or ready, with its buttons.
    fn offer(&mut self, ui: &mut egui::Ui) {
        match self.state() {
            State::Available(release) => {
                ui.label(RichText::new(trf!("Version {} is out.", release.version)).strong());
                if ui.button(tr("Update now")).clicked() {
                    self.install(ui.ctx());
                }
                if !self.later && ui.button(tr("Later")).clicked() {
                    self.later = true;
                }
            }
            State::Downloading(version) => {
                ui.spinner();
                ui.label(trf!("Downloading version {}…", version));
            }
            State::Installed { version, .. } => {
                ui.label(trf!("Version {} is installed.", version));
                if ui.button(tr("Restart now")).clicked()
                    && let Err(message) = self.restart(ui.ctx())
                    && let Ok(mut state) = self.state.lock()
                {
                    *state = State::Failed {
                        message,
                        installing: true,
                    };
                }
            }
            _ => {}
        }
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

fn get(url: &str, limit: u64, timeout: Duration) -> Result<Vec<u8>, String> {
    agent(timeout)
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().with_config().limit(limit).read_to_vec())
        .map_err(|error| match error {
            // A release made before updates, or without the signing key.
            ureq::Error::StatusCode(404) => {
                tr("The latest release has no update for the app.").to_owned()
            }
            error => trf!("Couldn't reach GitHub: {}", error),
        })
}

/// The latest release, if it is newer than this version.
fn latest() -> Result<State, String> {
    let platform = PLATFORM.ok_or_else(|| tr("No updates are made for this system.").to_owned())?;
    let timeout = Duration::from_secs(30);
    let manifest = get(
        &format!("{}/releases/latest/download/update.json", source()),
        64 * 1024,
        timeout,
    )?;
    let signature = get(
        &format!("{}/releases/latest/download/update.json.minisig", source()),
        4096,
        timeout,
    )?;
    verify(&manifest, &signature)?;
    let manifest: Manifest = serde_json::from_slice(&manifest)
        .map_err(|error| trf!("The update's description is unreadable: {}", error))?;
    if !newer(&manifest.version, crate::VERSION) {
        return Ok(State::UpToDate);
    }
    let file = manifest.files.get(platform).cloned().ok_or_else(|| {
        trf!(
            "Version {} has no update for this system.",
            manifest.version
        )
    })?;
    Ok(State::Available(Release {
        version: manifest.version,
        file,
    }))
}

/// Whether `data` was signed with the release key.
fn verify(data: &[u8], signature: &[u8]) -> Result<(), String> {
    let refused = || tr("The update isn't signed by the app's maker: not installed.").to_owned();
    let key = minisign_verify::PublicKey::from_base64(PUBLIC_KEY).map_err(|_| refused())?;
    let signature = std::str::from_utf8(signature)
        .ok()
        .and_then(|text| minisign_verify::Signature::decode(text).ok())
        .ok_or_else(refused)?;
    key.verify(data, &signature, false).map_err(|_| refused())
}

/// Whether version `candidate` comes after `current` (as 0.8.10 after 0.8.9).
fn newer(candidate: &str, current: &str) -> bool {
    let parts = |version: &str| -> Option<Vec<u64>> {
        version.split('.').map(|part| part.parse().ok()).collect()
    };
    match (parts(candidate), parts(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn download_and_install(release: &Release) -> Result<PathBuf, String> {
    let url = format!(
        "{}/releases/download/v{}/{}",
        source(),
        release.version,
        release.file.name
    );
    let bytes = get(&url, 512 * 1024 * 1024, Duration::from_secs(15 * 60))?;
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if !digest.eq_ignore_ascii_case(&release.file.sha256) {
        return Err(tr("The download doesn't match the signed update: not installed.").to_owned());
    }
    put_in_place(&bytes)
}

fn write_error(error: std::io::Error) -> String {
    trf!("Couldn't replace the app: {}", error)
}

/// Unpacks the new app bundle (a .tar.gz) next to this one and swaps them;
/// returns the bundle, to open.
#[cfg(target_os = "macos")]
fn put_in_place(archive: &[u8]) -> Result<PathBuf, String> {
    use std::fs;
    let program = std::env::current_exe().map_err(write_error)?;
    let bundle = program
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| tr("Only the app from the download page can update itself.").to_owned())?
        .to_path_buf();
    let shown = bundle.to_string_lossy();
    if shown.contains("/AppTranslocation/") || shown.starts_with("/Volumes/") {
        return Err(tr("Move the app to Applications first, then update.").to_owned());
    }
    let folder = bundle
        .parent()
        .ok_or_else(|| write_error(std::io::ErrorKind::NotFound.into()))?;
    let staging = folder.join(format!(".mxsc-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(write_error)?;
    let result = (|| {
        let packed = staging.join("update.tar.gz");
        fs::write(&packed, archive).map_err(write_error)?;
        let unpacked = std::process::Command::new("/usr/bin/tar")
            .arg("-xzf")
            .arg(&packed)
            .arg("-C")
            .arg(&staging)
            .status()
            .map_err(write_error)?;
        let new = staging.join(format!("{}.app", crate::APP_NAME));
        if !unpacked.success() || !new.is_dir() {
            return Err(tr("The update couldn't be unpacked.").to_owned());
        }
        let old = staging.join("old.app");
        fs::rename(&bundle, &old).map_err(write_error)?;
        if let Err(error) = fs::rename(&new, &bundle) {
            let _ = fs::rename(&old, &bundle);
            return Err(write_error(error));
        }
        Ok(bundle.clone())
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Writes the new program beside this one and swaps them; returns it, to
/// start.
#[cfg(not(target_os = "macos"))]
fn put_in_place(program: &[u8]) -> Result<PathBuf, String> {
    use std::fs;
    use std::io::Write;
    // Before the swap: on Linux, afterwards it names the replaced file.
    let current = std::env::current_exe().map_err(write_error)?;
    let folder = current
        .parent()
        .ok_or_else(|| write_error(std::io::ErrorKind::NotFound.into()))?;
    // In the program's own folder, which only its owner can write to, and
    // made new: never a file or link someone else put there.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let new = folder.join(format!(".mxsc-update-{}-{stamp}", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o755);
    }
    let written = options
        .open(&new)
        .and_then(|mut file| file.write_all(program).and_then(|()| file.sync_all()))
        .map_err(write_error);
    let replaced = written.and_then(|()| self_replace::self_replace(&new).map_err(write_error));
    let _ = fs::remove_file(&new);
    replaced.map(|()| current)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `test`, signed with the release key by `tools/sign-update`.
    const SIGNED: &str = "untrusted comment: signature from rsign secret key
RUQOT8bt+OOxmJIhrGjiXnEYAqAUXd6qHpyTylL+fhXVgO8Hiwg3x44v7FNL7hghbXrcqxuC87gi1ozPWUnR97IZSbA+B2pPRA4=
trusted comment: MX Shooting Calculator update: test.txt
/IXsPnq3iMRQfF0OnKLs6z2ms070Qp/Wpswj20qoyieVLmj9MhWIm/QeAdI5pvpcCWlCMiyNMwr1+jd/n/RUAQ==
";

    #[test]
    fn only_what_the_release_key_signed_is_accepted() {
        assert!(verify(b"test", SIGNED.as_bytes()).is_ok());
        assert!(verify(b"tesT", SIGNED.as_bytes()).is_err());
        assert!(verify(b"test", b"not a signature").is_err());
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.8.2", "0.8.1"));
        assert!(newer("0.8.10", "0.8.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("0.8.1", "0.8.1"));
        assert!(!newer("0.8.0", "0.8.1"));
        assert!(!newer("0.9.0-beta", "0.8.1"));
    }

    /// The whole update from a local server set up by hand, run from a
    /// copy of the test program inside an app bundle (macOS) or anywhere
    /// (elsewhere): MXSC_UPDATE_FROM names the server.
    #[test]
    #[ignore]
    fn updates_from_a_server() {
        let State::Available(release) = latest().unwrap() else {
            panic!("no update offered");
        };
        let start = download_and_install(&release).unwrap();
        println!("installed {} at {}", release.version, start.display());
    }

    #[test]
    fn the_manifest_reads() {
        let manifest: Manifest = serde_json::from_str(
            r#"{"version":"0.8.2","files":{"macos":{"name":"a.tar.gz","sha256":"00"}}}"#,
        )
        .unwrap();
        assert_eq!(manifest.version, "0.8.2");
        assert_eq!(manifest.files["macos"].name, "a.tar.gz");
    }
}

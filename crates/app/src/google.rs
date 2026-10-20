//! Syncing through the hunter's own Google Drive: signing in (through the
//! sync server, which holds the Google client secret), keeping an access
//! token fresh, and reading and writing the app's file in Drive's hidden
//! app-data folder, where only this app can see it.

use std::sync::{Arc, Mutex};

use ballistics_store::model::now_ms;
use serde::Deserialize;

use crate::http::{self, Method, Request};

const DRIVE: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const FILE_NAME: &str = "mxsc-sync.json";
/// What the file holds once the synced copy is deleted, so the other
/// devices stop instead of starting it again.
pub const DELETED: &[u8] = b"MXS-DELETED";
/// How long a sign-in may take in the browser.
const SIGN_IN_S: u32 = 600;

/// A signed-in Google account. The app doesn't ask which: only for its
/// own folder in the account's Drive.
pub struct Account {
    pub refresh_token: String,
    access_token: Option<String>,
    expires_at_ms: i64,
}

impl Account {
    pub fn new(refresh_token: String) -> Account {
        Account {
            refresh_token,
            access_token: None,
            expires_at_ms: 0,
        }
    }
}

pub type Shared = Arc<Mutex<Account>>;

/// The permission the app needs: its own hidden folder in Drive.
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
/// What to tell the hunter when Google's sign-in left Drive out.
pub const NO_DRIVE_ACCESS: &str = "Google gave the app no access to your Drive. Sign in again \
     and tick the box that lets the app keep its data in your Google Drive.";

/// Why a Drive request failed.
pub enum Trouble {
    /// Google no longer accepts the sign-in, or it lacks Drive: the hunter
    /// must sign in again (with why).
    SignIn(String),
    /// The synced copy was deleted.
    Deleted,
    Other(String),
}

#[derive(Deserialize)]
struct Tokens {
    refresh_token: Option<String>,
    access_token: String,
    expires_in: Option<i64>,
    /// The permissions granted, separated by spaces.
    scope: Option<String>,
}

#[derive(Deserialize)]
struct FileList {
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    id: String,
}

/// The page that starts a sign-in, for the session `session`.
pub fn sign_in_url(server: &str, session: &str) -> String {
    format!("{server}/v1/google/start?session={session}")
}

/// Waits for the hunter to finish signing in in the browser, then picks up
/// the account from the sync server.
pub async fn finish_sign_in(server: String, session: String) -> Result<Account, String> {
    let started = now_ms();
    while now_ms() - started < i64::from(SIGN_IN_S) * 1000 {
        http::sleep(2000).await;
        let response = http::fetch(Request {
            method: Method::Post,
            url: format!("{server}/v1/google/session/{session}"),
            headers: Vec::new(),
            body: None,
        })
        .await;
        match response {
            Ok(response) if response.status == 200 => {
                let tokens: Tokens = serde_json::from_slice(&response.body)
                    .map_err(|_| "the sign-in answer was garbled".to_owned())?;
                if tokens
                    .scope
                    .as_deref()
                    .is_some_and(|scope| !scope.split(' ').any(|s| s == DRIVE_SCOPE))
                {
                    return Err(NO_DRIVE_ACCESS.into());
                }
                let refresh_token = tokens
                    .refresh_token
                    .ok_or_else(|| "Google sent no lasting sign-in".to_owned())?;
                let mut account = Account::new(refresh_token);
                account.access_token = Some(tokens.access_token);
                account.expires_at_ms = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
                return Ok(account);
            }
            Ok(response) if response.status == 404 => {}
            Ok(response) => return Err(format!("the sync server answered {}", response.status)),
            // No connection for a moment: keep waiting.
            Err(_) => {}
        }
    }
    Err("signing in took too long".into())
}

/// The app's file: its bytes and its Drive id, or `None` before any device
/// wrote one.
pub async fn read(server: &str, account: &Shared) -> Result<Option<(Vec<u8>, String)>, Trouble> {
    let Some(id) = find(server, account).await? else {
        return Ok(None);
    };
    let response = drive(
        server,
        account,
        Method::Get,
        format!("{DRIVE}/{id}?alt=media"),
        None,
        None,
    )
    .await?;
    if response.body == DELETED {
        return Err(Trouble::Deleted);
    }
    Ok(Some((response.body, id)))
}

/// Writes the app's file (`id` from `read`, or `None` to make it).
pub async fn write(
    server: &str,
    account: &Shared,
    id: Option<String>,
    bytes: Vec<u8>,
) -> Result<(), Trouble> {
    let id = match id {
        Some(id) => id,
        // Another device may have made it meanwhile.
        None => match find(server, account).await? {
            Some(id) => id,
            None => create(server, account).await?,
        },
    };
    drive(
        server,
        account,
        Method::Patch,
        format!("{UPLOAD}/{id}?uploadType=media"),
        Some("application/json"),
        Some(bytes),
    )
    .await?;
    Ok(())
}

/// The oldest file of the app's: if two devices made one at the same time,
/// both settle on the same.
async fn find(server: &str, account: &Shared) -> Result<Option<String>, Trouble> {
    let url = format!(
        "{DRIVE}?spaces=appDataFolder&q=name%3D%27{FILE_NAME}%27%20and%20trashed%3Dfalse\
         &orderBy=createdTime&fields=files(id)&pageSize=10"
    );
    let response = drive(server, account, Method::Get, url, None, None).await?;
    let list: FileList = serde_json::from_slice(&response.body)
        .map_err(|_| Trouble::Other("Google Drive's answer was garbled".into()))?;
    Ok(list.files.into_iter().next().map(|file| file.id))
}

async fn create(server: &str, account: &Shared) -> Result<String, Trouble> {
    let metadata = serde_json::json!({
        "name": FILE_NAME,
        "parents": ["appDataFolder"],
        "mimeType": "application/json",
    });
    let response = drive(
        server,
        account,
        Method::Post,
        format!("{DRIVE}?fields=id"),
        Some("application/json"),
        Some(metadata.to_string().into_bytes()),
    )
    .await?;
    let file: File = serde_json::from_slice(&response.body)
        .map_err(|_| Trouble::Other("Google Drive's answer was garbled".into()))?;
    Ok(file.id)
}

/// A Drive request with a fresh access token; on 401 the token is renewed
/// once and the request tried again.
async fn drive(
    server: &str,
    account: &Shared,
    method: Method,
    url: String,
    content_type: Option<&str>,
    body: Option<Vec<u8>>,
) -> Result<http::Response, Trouble> {
    for attempt in 0..2 {
        let token = access_token(server, account, attempt > 0).await?;
        let mut headers = vec![("Authorization", format!("Bearer {token}"))];
        if let Some(content_type) = content_type {
            headers.push(("Content-Type", content_type.to_owned()));
        }
        let response = http::fetch(Request {
            method,
            url: url.clone(),
            headers,
            body: body.clone(),
        })
        .await
        .map_err(Trouble::Other)?;
        match response.status {
            200..=299 => return Ok(response),
            401 if attempt == 0 => continue,
            401 => return Err(Trouble::SignIn("sign in to Google again".into())),
            403 => return Err(refused(&response.body)),
            status => return Err(Trouble::Other(format!("Google Drive answered {status}"))),
        }
    }
    Err(Trouble::SignIn("sign in to Google again".into()))
}

/// Why Drive refused (403), from its answer.
fn refused(body: &[u8]) -> Trouble {
    #[derive(Deserialize)]
    struct Answer {
        error: Problem,
    }
    #[derive(Deserialize)]
    struct Problem {
        message: Option<String>,
        #[serde(default)]
        errors: Vec<Detail>,
    }
    #[derive(Deserialize)]
    struct Detail {
        reason: Option<String>,
    }
    let Ok(answer) = serde_json::from_slice::<Answer>(body) else {
        return Trouble::Other("Google Drive refused".into());
    };
    let reason = answer
        .error
        .errors
        .first()
        .and_then(|detail| detail.reason.clone())
        .unwrap_or_default();
    let message = answer.error.message.unwrap_or_default();
    match reason.as_str() {
        "insufficientPermissions" | "insufficientScopes" => Trouble::SignIn(NO_DRIVE_ACCESS.into()),
        _ if message.contains("scope") => Trouble::SignIn(NO_DRIVE_ACCESS.into()),
        "accessNotConfigured" => {
            Trouble::Other("the Google Drive API isn't enabled for the app".into())
        }
        _ => Trouble::Other(format!("Google Drive refused: {message}")),
    }
}

/// The access token, renewed through the sync server when it is about to
/// run out (or `renew` says so).
async fn access_token(server: &str, account: &Shared, renew: bool) -> Result<String, Trouble> {
    let (refresh_token, current) = {
        let account = account.lock().expect("not poisoned");
        let fresh = account.expires_at_ms > now_ms() + 60_000;
        (
            account.refresh_token.clone(),
            account.access_token.clone().filter(|_| fresh && !renew),
        )
    };
    if let Some(token) = current {
        return Ok(token);
    }
    let response = http::fetch(Request {
        method: Method::Post,
        url: format!("{server}/v1/google/refresh"),
        headers: vec![("Content-Type", "application/json".into())],
        body: Some(
            serde_json::json!({ "refresh_token": refresh_token })
                .to_string()
                .into_bytes(),
        ),
    })
    .await
    .map_err(Trouble::Other)?;
    match response.status {
        200 => {
            let tokens: Tokens = serde_json::from_slice(&response.body)
                .map_err(|_| Trouble::Other("the sync server's answer was garbled".into()))?;
            let mut account = account.lock().expect("not poisoned");
            account.access_token = Some(tokens.access_token.clone());
            account.expires_at_ms = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
            Ok(tokens.access_token)
        }
        401 | 400 => Err(Trouble::SignIn("sign in to Google again".into())),
        status => Err(Trouble::Other(format!("the sync server answered {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With a made-up sign-in, Drive and the sync server must answer "sign
    /// in again", not fail on the requests themselves:
    /// `cargo test -p mxsc -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn a_dead_sign_in_asks_to_sign_in_again() {
        let account: Shared = Arc::new(Mutex::new(Account::new("made-up".into())));
        let server = crate::sync::DEFAULT_SERVER.to_owned();
        let mut outcome = None;
        http::spawn_blocking_for_tests(async {
            outcome = Some(read(&server, &account).await);
        });
        match outcome.unwrap() {
            Err(Trouble::SignIn(_)) => {}
            Err(Trouble::Other(error)) => panic!("{error}"),
            Err(Trouble::Deleted) => panic!("deleted"),
            Ok(_) => panic!("read with a made-up sign-in"),
        }
    }
}

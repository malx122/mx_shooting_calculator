//! Syncing through the hunter's own Google Drive: signing in (through the
//! sync server, which holds the Google client secret), keeping an access
//! token fresh, and reading and writing the app's file in Drive's hidden
//! app-data folder, where only this app can see it.

use std::sync::{Arc, Mutex};

use ballistics_store::model::now_ms;
use serde::Deserialize;

use crate::http::{self, Method, Request};
use crate::i18n::{tr, trf};

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
fn no_drive_access() -> &'static str {
    tr(
        "Google gave the app no access to your Drive. Sign in again and tick the box that lets \
        the app keep its data in your Google Drive.",
    )
}

/// Why a Drive request failed.
pub enum Trouble {
    /// Google no longer accepts the sign-in, or it lacks Drive: the hunter
    /// must sign in again (with why).
    SignIn(String),
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

/// The page that starts a sign-in, for the session `session`; `reply`
/// says how the sync server hands the pick-up code back to this app (see
/// the server's google.js): the port the desktop app listens on, or `w`
/// and the web app's origin's number.
pub fn sign_in_url(server: &str, session: &str, reply: &str) -> String {
    format!("{server}/v1/google/start?session={session}&reply={reply}")
}

/// The code the sync server keeps a finished sign-in under, once it reaches
/// the app: through the desktop's port, a message to the web app's window,
/// or typed in from the sign-in page.
pub type PickUp = Arc<Mutex<Option<String>>>;

/// A pick-up code as typed, in its own letters (Crockford's base 32: no I,
/// L, O or U), or `None` if it isn't one.
pub fn pick_up_code(typed: &str) -> Option<String> {
    let code: String = typed
        .chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect();
    (code.len() == 10
        && code
            .chars()
            .all(|c| c.is_ascii_digit() || (c.is_ascii_uppercase() && !"ILOU".contains(c))))
    .then_some(code)
}

/// Waits for the pick-up code of the hunter's sign-in in the browser, then
/// picks up the account from the sync server.
pub async fn finish_sign_in(server: String, pick_up: PickUp) -> Result<Account, String> {
    let started = now_ms();
    while now_ms() - started < i64::from(SIGN_IN_S) * 1000 {
        http::sleep(500).await;
        let Some(code) = pick_up.lock().ok().and_then(|code| code.clone()) else {
            continue;
        };
        let response = http::fetch(Request {
            method: Method::Post,
            url: format!("{server}/v1/google/session/{code}"),
            headers: Vec::new(),
            body: None,
        })
        .await;
        match response {
            Ok(response) if response.status == 200 => {
                let tokens: Tokens = serde_json::from_slice(&response.body)
                    .map_err(|_| tr("the sign-in answer was garbled").to_owned())?;
                if tokens
                    .scope
                    .as_deref()
                    .is_some_and(|scope| !scope.split(' ').any(|s| s == DRIVE_SCOPE))
                {
                    return Err(no_drive_access().into());
                }
                let refresh_token = tokens
                    .refresh_token
                    .ok_or_else(|| tr("Google sent no lasting sign-in").to_owned())?;
                let mut account = Account::new(refresh_token);
                account.access_token = Some(tokens.access_token);
                account.expires_at_ms = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
                return Ok(account);
            }
            Ok(response) if response.status == 404 => {
                return Err(tr("that sign-in has expired or was already used").into());
            }
            Ok(response) => {
                return Err(trf!(
                    "the sync server answered {status}",
                    status = response.status
                ));
            }
            Err(error) => return Err(error),
        }
    }
    Err(tr("signing in took too long").into())
}

/// Waits on `listener` (on 127.0.0.1) for the browser to come back from the
/// sync server with the pick-up code of `session`, and puts it in
/// `pick_up`. Requests for other sessions are turned away.
#[cfg(not(target_arch = "wasm32"))]
pub fn listen(listener: std::net::TcpListener, session: String, pick_up: PickUp) {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(SIGN_IN_S.into()) {
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(_) => return,
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        // The request line and headers, which may come in pieces.
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while request.len() < 8192 && !request.windows(4).any(|end| end == b"\r\n\r\n") {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => request.extend_from_slice(&chunk[..read]),
            }
        }
        let request = String::from_utf8_lossy(&request);
        let code = request
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("GET /google?"))
            .and_then(|line| line.split(' ').next())
            .and_then(|query| {
                let field = |name: &str| {
                    query
                        .split('&')
                        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                };
                (field("session") == Some(session.as_str()))
                    .then(|| field("pickup").and_then(pick_up_code))
                    .flatten()
            });
        let (status, title, text) = match &code {
            Some(_) => (
                "200 OK",
                tr("Signed in"),
                tr(
                    "Go back to MX Shooting Calculator; it starts syncing in a moment. You can \
                    close this window.",
                ),
            ),
            None => (
                "404 Not Found",
                tr("Sign-in failed"),
                tr("This isn't the sign-in the app is waiting for. Try again from the app."),
            ),
        };
        let page = format!(
            "<!doctype html><meta charset=\"utf-8\"><title>{title}</title>\
             <body style=\"font-family:system-ui,sans-serif;background:#16261d;color:#ece6d6;\
             text-align:center;padding:3rem\"><h1 style=\"color:#e0682b\">{title}</h1>\
             <p>{text}</p>"
        );
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        if let Some(code) = code {
            if let Ok(mut slot) = pick_up.lock() {
                *slot = Some(code);
            }
            return;
        }
    }
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
    // The deletion marker comes back as it is: sync stops on it, unless
    // this device was just signed in again to start afresh.
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
        .map_err(|_| Trouble::Other(tr("Google Drive's answer was garbled").into()))?;
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
        .map_err(|_| Trouble::Other(tr("Google Drive's answer was garbled").into()))?;
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
            401 => return Err(Trouble::SignIn(tr("sign in to Google again").into())),
            403 => return Err(refused(&response.body)),
            status => {
                return Err(Trouble::Other(trf!(
                    "Google Drive answered {status}",
                    status = status
                )));
            }
        }
    }
    Err(Trouble::SignIn(tr("sign in to Google again").into()))
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
        return Trouble::Other(tr("Google Drive refused").into());
    };
    let reason = answer
        .error
        .errors
        .first()
        .and_then(|detail| detail.reason.clone())
        .unwrap_or_default();
    let message = answer.error.message.unwrap_or_default();
    match reason.as_str() {
        "insufficientPermissions" | "insufficientScopes" => {
            Trouble::SignIn(no_drive_access().into())
        }
        _ if message.contains("scope") => Trouble::SignIn(no_drive_access().into()),
        "accessNotConfigured" => {
            Trouble::Other(tr("the Google Drive API isn't enabled for the app").into())
        }
        _ => Trouble::Other(trf!("Google Drive refused: {message}", message = message)),
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
                .map_err(|_| Trouble::Other(tr("the sync server's answer was garbled").into()))?;
            let mut account = account.lock().expect("not poisoned");
            account.access_token = Some(tokens.access_token.clone());
            account.expires_at_ms = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
            Ok(tokens.access_token)
        }
        401 | 400 => Err(Trouble::SignIn(tr("sign in to Google again").into())),
        status => Err(Trouble::Other(trf!(
            "the sync server answered {status}",
            status = status
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With a made-up sign-in, Drive and the sync server must answer "sign
    /// in again", not fail on the requests themselves:
    /// `cargo test -p mxsc -- --ignored`.
    #[test]
    fn pick_up_codes_read_as_typed() {
        assert_eq!(pick_up_code("abcde-fgh12").as_deref(), Some("ABCDEFGH12"));
        assert_eq!(pick_up_code("0O1IL 23456").as_deref(), Some("0011123456"));
        assert_eq!(pick_up_code("ABCDEFGH1"), None);
        assert_eq!(pick_up_code("ABCDEFGHU1"), None);
        assert_eq!(pick_up_code("ABCDE/GH12"), None);
    }

    /// The desktop's listener takes the code of its own session only.
    #[test]
    fn the_listener_takes_its_own_sessions_code() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let pick_up: PickUp = Arc::default();
        let waiting = {
            let pick_up = Arc::clone(&pick_up);
            std::thread::spawn(move || listen(listener, "s1".into(), pick_up))
        };
        let get = |path: &str| {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).unwrap();
            answer
        };
        assert!(get("/google?session=s2&pickup=ABCDEFGH12").starts_with("HTTP/1.1 404"));
        assert!(get("/google?session=s1&pickup=bad").starts_with("HTTP/1.1 404"));
        assert_eq!(*pick_up.lock().unwrap(), None);
        assert!(get("/google?session=s1&pickup=ABCDEFGH12").starts_with("HTTP/1.1 200"));
        waiting.join().unwrap();
        assert_eq!(pick_up.lock().unwrap().as_deref(), Some("ABCDEFGH12"));
    }

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
            Ok(_) => panic!("read with a made-up sign-in"),
        }
    }
}

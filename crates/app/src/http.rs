//! Plain HTTP requests in the background: a thread on the desktop, the
//! browser's fetch on the web. Either a single request with a callback
//! (`send`), or a longer job written as async code (`spawn` with `fetch`
//! and `sleep`). Answers arrive off the UI's frame; the UI is asked to
//! repaint to pick them up.

use std::future::Future;

use eframe::egui;

pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Put,
    Post,
    Patch,
    Delete,
}

impl Method {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Put => "PUT",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub body: Vec<u8>,
}

/// Sends the request; `done` gets the response, or why there was none
/// (no connection, for instance).
pub fn send(
    request: Request,
    context: egui::Context,
    done: impl FnOnce(Result<Response, String>) + Send + 'static,
) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        done(blocking(request));
        context.request_repaint();
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        done(browser_fetch(request).await);
        context.request_repaint();
    });
}

/// What a background job must be: sendable to a thread on the desktop;
/// anything in the browser, which has one thread.
#[cfg(not(target_arch = "wasm32"))]
pub trait Job: Future<Output = ()> + Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<F: Future<Output = ()> + Send + 'static> Job for F {}
#[cfg(target_arch = "wasm32")]
pub trait Job: Future<Output = ()> + 'static {}
#[cfg(target_arch = "wasm32")]
impl<F: Future<Output = ()> + 'static> Job for F {}

/// Runs a job of several requests in the background.
pub fn spawn(job: impl Job) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || block_on(job));
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(job);
}

/// A request inside a job.
pub async fn fetch(request: Request) -> Result<Response, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return blocking(request);
    #[cfg(target_arch = "wasm32")]
    return browser_fetch(request).await;
}

/// Waits inside a job.
pub async fn sleep(milliseconds: u32) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::sleep(std::time::Duration::from_millis(milliseconds.into()));
    #[cfg(target_arch = "wasm32")]
    {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    &resolve,
                    milliseconds as i32,
                );
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }
}

/// Opens a page in the browser (a new tab on the web).
pub fn open_in_browser(context: &egui::Context, url: &str) {
    #[cfg(target_arch = "wasm32")]
    context.open_url(egui::OpenUrl::new_tab(url));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = context;
        #[cfg(target_os = "macos")]
        let opened = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "windows")]
        let opened = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let opened = std::process::Command::new("xdg-open").arg(url).spawn();
        let _ = opened;
    }
}

/// Runs a job to the end on this thread, for tests.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub fn spawn_blocking_for_tests(job: impl Future<Output = ()>) {
    block_on(job);
}

/// Runs a job to the end on this thread. The desktop's requests and
/// sleeps block instead of waiting, so a job is ready the first time it is
/// polled; the loop is only a safeguard.
#[cfg(not(target_arch = "wasm32"))]
fn block_on(job: impl Future<Output = ()>) {
    use std::task::{Context, Poll, Waker};
    let mut job = std::pin::pin!(job);
    let mut context = Context::from_waker(Waker::noop());
    while job.as_mut().poll(&mut context) == Poll::Pending {
        std::thread::yield_now();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn blocking(request: Request) -> Result<Response, String> {
    use std::time::Duration;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut response = match request.method {
        Method::Get => {
            let mut call = agent.get(&request.url);
            for (name, value) in &request.headers {
                call = call.header(*name, value);
            }
            call.call()
        }
        Method::Delete => {
            let mut call = agent.delete(&request.url);
            for (name, value) in &request.headers {
                call = call.header(*name, value);
            }
            call.call()
        }
        Method::Put | Method::Post | Method::Patch => {
            let mut call = match request.method {
                Method::Post => agent.post(&request.url),
                Method::Patch => agent.patch(&request.url),
                _ => agent.put(&request.url),
            };
            for (name, value) in &request.headers {
                call = call.header(*name, value);
            }
            call.send(request.body.as_deref().unwrap_or_default())
        }
    }
    .map_err(|error| error.to_string())?;
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|error| error.to_string())?;
    Ok(Response {
        status: response.status().as_u16(),
        etag,
        body,
    })
}

#[cfg(target_arch = "wasm32")]
async fn browser_fetch(request: Request) -> Result<Response, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let fail = |what: &str| format!("no connection ({what})");
    let init = web_sys::RequestInit::new();
    init.set_method(request.method.as_str());
    let headers = web_sys::Headers::new().map_err(|_| fail("headers"))?;
    for (name, value) in &request.headers {
        headers.set(name, value).map_err(|_| fail("headers"))?;
    }
    init.set_headers(&headers);
    if let Some(body) = &request.body {
        init.set_body(&js_sys::Uint8Array::from(body.as_slice()));
    }
    let js_request = web_sys::Request::new_with_str_and_init(&request.url, &init)
        .map_err(|_| fail("request"))?;
    let window = web_sys::window().ok_or_else(|| fail("window"))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&js_request))
        .await
        .map_err(|_| fail("fetch"))?
        .dyn_into()
        .map_err(|_| fail("response"))?;
    let etag = response.headers().get("ETag").ok().flatten();
    let buffer = JsFuture::from(response.array_buffer().map_err(|_| fail("body"))?)
        .await
        .map_err(|_| fail("body"))?;
    Ok(Response {
        status: response.status(),
        etag,
        body: js_sys::Uint8Array::new(&buffer).to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Talks to the real sync server: `cargo test -p mxsc -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn the_sync_server_answers() {
        let id: String = (0..64)
            .map(|i| if i % 2 == 0 { 'a' } else { '0' })
            .collect();
        let url = format!("{}/v1/copies/{id}", crate::sync::DEFAULT_SERVER);
        let missing = blocking(Request {
            method: Method::Get,
            url: url.clone(),
            headers: Vec::new(),
            body: None,
        })
        .unwrap();
        assert!(matches!(missing.status, 404 | 410), "{}", missing.status);
        let stale = blocking(Request {
            method: Method::Put,
            url,
            headers: vec![("If-Match", "\"999999\"".into())],
            body: Some(b"x".to_vec()),
        })
        .unwrap();
        assert!(matches!(stale.status, 412 | 410), "{}", stale.status);
    }
}

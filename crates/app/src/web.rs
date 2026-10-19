//! Browser-only pieces: start-up in a canvas, saving a file as a download,
//! and picking a file to import. The desktop build uses native dialogs.

use std::cell::RefCell;
use std::rc::Rc;

use ballistics_store::Store;
use eframe::egui;
use wasm_bindgen::JsCast;

/// The database's name in the browser's storage.
const DATABASE: &str = "mxsc.sqlite";

/// A file the hunter picked, read in the background.
pub struct PickedFile {
    pub name: String,
    pub text: String,
    /// The file's modification time (ms since the epoch).
    pub modified_ms: i64,
}

/// Where a picked file lands for the app to collect on its next frame.
pub type PickedSlot = Rc<RefCell<Option<PickedFile>>>;

/// Offers `text` as a file download named `file_name`.
pub fn download(file_name: &str, text: &str) -> Result<(), String> {
    let fail = |what: &str| format!("could not save the file ({what})");
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)
        .map_err(|_| fail("blob"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| fail("url"))?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| fail("no document"))?;
    let link: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|_| fail("link"))?
        .dyn_into()
        .map_err(|_| fail("link"))?;
    link.set_href(&url);
    link.set_download(file_name);
    link.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Opens the browser's file picker; the chosen file is read in the
/// background and put in `slot`.
pub fn pick_backup(slot: PickedSlot, context: egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(handle) = rfd::AsyncFileDialog::new()
            .add_filter("Backups", &["json", "srl"])
            .pick_file()
            .await
        else {
            return;
        };
        let bytes = handle.read().await;
        *slot.borrow_mut() = Some(PickedFile {
            name: handle.file_name(),
            text: String::from_utf8_lossy(&bytes).into_owned(),
            modified_ms: handle.inner().last_modified() as i64,
        });
        context.request_repaint();
    });
}

/// A sync key from a pairing link (`…#sync=KEY`), taken off the address so
/// a reload or a bookmark doesn't offer it again.
pub fn take_sync_link() -> Option<ballistics_store::SyncKey> {
    let window = web_sys::window()?;
    let location = window.location();
    let hash = location.hash().ok()?;
    if !hash.contains("sync=") {
        return None;
    }
    let clean = format!(
        "{}{}",
        location.pathname().unwrap_or_default(),
        location.search().unwrap_or_default()
    );
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&clean));
    }
    ballistics_store::SyncKey::parse(&hash)
}

/// Opens the database kept in this browser's IndexedDB. SQLite works on
/// a copy in memory that is written back to IndexedDB in the background
/// after each change.
async fn open_store() -> Result<Store, String> {
    use sqlite_wasm_vfs::relaxed_idb::{RelaxedIdbCfg, install};
    install::<sqlite_wasm_rs::WasmOsCallback>(&RelaxedIdbCfg::default(), true)
        .await
        .map_err(|error| {
            format!("this browser does not allow the app to store data ({error:?})")
        })?;
    Store::open(DATABASE).map_err(|error| format!("could not open the database: {error}"))
}

/// Opens the database, then starts the app in the page's
/// `<canvas id="mxsc">`.
pub fn start(make_app: fn(Result<Store, String>) -> eframe::AppCreator<'static>) {
    console_error_panic_hook::set_once();
    keep_storage();
    wasm_bindgen_futures::spawn_local(async move {
        let make_app = make_app(open_store().await);
        let document = web_sys::window()
            .and_then(|window| window.document())
            .expect("a web page");
        let canvas = document
            .get_element_by_id("mxsc")
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("the page has <canvas id=\"mxsc\">");
        let started = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), make_app)
            .await;
        // Replace the loading text with the outcome.
        if let Some(loading) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => loading.remove(),
                Err(error) => {
                    loading.set_text_content(Some(&format!("The app could not start: {error:?}")))
                }
            }
        }
    });
}

thread_local! {
    /// The phone's latest front-to-back tilt (degrees), once measuring.
    static TILT: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
    static MEASURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this browser can report the phone's tilt.
pub fn tilt_supported() -> bool {
    web_sys::window().is_some_and(|window| {
        js_sys::Reflect::has(&window, &"DeviceOrientationEvent".into()).unwrap_or(false)
            && js_sys::Reflect::has(&window, &"ontouchstart".into()).unwrap_or(false)
    })
}

/// The phone's tilt: 0 lying flat, positive with the top edge raised.
pub fn tilt() -> Option<f64> {
    TILT.get()
}

/// Starts reading the tilt sensor. iPhones ask the hunter's permission
/// first; the answer comes later.
pub fn start_tilt(context: egui::Context) {
    if MEASURING.get() {
        return;
    }
    MEASURING.set(true);
    let Some(window) = web_sys::window() else {
        return;
    };
    let listen = {
        let window = window.clone();
        move || {
            let closure = wasm_bindgen::closure::Closure::<
                dyn FnMut(web_sys::DeviceOrientationEvent),
            >::new(move |event: web_sys::DeviceOrientationEvent| {
                TILT.set(event.beta());
                context.request_repaint();
            });
            let _ = window.add_event_listener_with_callback(
                "deviceorientation",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
        }
    };
    let request = js_sys::Reflect::get(&window, &"DeviceOrientationEvent".into())
        .ok()
        .and_then(|class| {
            let request = js_sys::Reflect::get(&class, &"requestPermission".into()).ok()?;
            let request: js_sys::Function = request.dyn_into().ok()?;
            request
                .call0(&class)
                .ok()?
                .dyn_into::<js_sys::Promise>()
                .ok()
        });
    match request {
        Some(permission) => wasm_bindgen_futures::spawn_local(async move {
            let answer = wasm_bindgen_futures::JsFuture::from(permission).await;
            if answer.ok().and_then(|answer| answer.as_string()).as_deref() == Some("granted") {
                listen();
            } else {
                MEASURING.set(false);
            }
        }),
        None => listen(),
    }
}

/// Calls `object.name()` in JavaScript, if it exists.
fn call(
    object: &wasm_bindgen::JsValue,
    name: &str,
    argument: Option<&wasm_bindgen::JsValue>,
) -> Option<wasm_bindgen::JsValue> {
    let function: js_sys::Function = js_sys::Reflect::get(object, &name.into())
        .ok()?
        .dyn_into()
        .ok()?;
    match argument {
        Some(argument) => function.call1(object, argument).ok(),
        None => function.call0(object).ok(),
    }
}

/// Asks the browser to keep the app's data instead of clearing it when
/// space runs low or the site isn't visited for a while.
pub fn keep_storage() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(storage) = js_sys::Reflect::get(&window.navigator(), &"storage".into()) {
        let _ = call(&storage, "persist", None);
    }
}

/// How the app can be put on the home screen.
#[derive(Clone, Copy, PartialEq)]
pub enum Install {
    /// Running as an installed app already.
    Installed,
    /// The browser offers an install prompt (Chrome, Android).
    Prompt,
    /// Only by hand, from the browser's menu (Safari on iPhone).
    ByHand,
}

pub fn install_state() -> Install {
    let Some(window) = web_sys::window() else {
        return Install::ByHand;
    };
    let standalone = call(
        &window,
        "matchMedia",
        Some(&"(display-mode: standalone)".into()),
    )
    .and_then(|query| js_sys::Reflect::get(&query, &"matches".into()).ok())
    .and_then(|matches| matches.as_bool())
    .unwrap_or(false)
        || js_sys::Reflect::get(&window.navigator(), &"standalone".into())
            .ok()
            .and_then(|standalone| standalone.as_bool())
            .unwrap_or(false);
    if standalone {
        Install::Installed
    } else if js_sys::Reflect::get(&window, &"mxscInstallPrompt".into())
        .is_ok_and(|prompt| prompt.is_object())
    {
        Install::Prompt
    } else {
        Install::ByHand
    }
}

/// Shows the browser's install prompt kept by the page.
pub fn prompt_install() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(prompt) = js_sys::Reflect::get(&window, &"mxscInstallPrompt".into()) {
        let _ = call(&prompt, "prompt", None);
        let _ = js_sys::Reflect::set(
            &window,
            &"mxscInstallPrompt".into(),
            &wasm_bindgen::JsValue::UNDEFINED,
        );
    }
}

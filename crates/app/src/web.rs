//! Browser-only pieces: start-up in a canvas, saving a file as a download,
//! and picking a file to import. The desktop build uses native dialogs.

use std::cell::RefCell;
use std::rc::Rc;

use ballistics_store::Store;
use eframe::egui;
use wasm_bindgen::JsCast;

use crate::i18n::{tr, trf};

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
    download_bytes(file_name, text.as_bytes(), "application/json")
}

/// Saves bytes as a file in the browser's downloads.
pub fn download_bytes(file_name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let fail = |what: &str| trf!("could not save the file ({what})", what = what);
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
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
            .add_filter(tr("Backups"), &["json", "srl"])
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
            trf!(
                "this browser does not allow the app to store data ({})",
                format!("{error:?}")
            )
        })?;
    Store::open(DATABASE).map_err(|error| trf!("could not open the database: {}", error))
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
                Err(error) => loading.set_text_content(Some(&trf!(
                    "The app could not start: {}",
                    format!("{error:?}")
                ))),
            }
        }
    });
}

thread_local! {
    /// The phone's latest front-to-back tilt (degrees), once measuring.
    static TILT: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
    /// Where the phone's top edge points (degrees from north), once known.
    static HEADING: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
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
            let repaint = context.clone();
            let closure = wasm_bindgen::closure::Closure::<
                dyn FnMut(web_sys::DeviceOrientationEvent),
            >::new(move |event: web_sys::DeviceOrientationEvent| {
                TILT.set(event.beta());
                // iPhones give the compass heading here.
                if let Some(heading) = js_sys::Reflect::get(&event, &"webkitCompassHeading".into())
                    .ok()
                    .and_then(|heading| heading.as_f64())
                {
                    HEADING.set(Some(heading));
                }
                repaint.request_repaint();
            });
            let _ = window.add_event_listener_with_callback(
                "deviceorientation",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
            // Android gives it as an absolute orientation: alpha counts
            // anticlockwise from north.
            let absolute = wasm_bindgen::closure::Closure::<
                dyn FnMut(web_sys::DeviceOrientationEvent),
            >::new(move |event: web_sys::DeviceOrientationEvent| {
                if let Some(alpha) = event.alpha() {
                    HEADING.set(Some((360.0 - alpha).rem_euclid(360.0)));
                    context.request_repaint();
                }
            });
            let _ = window.add_event_listener_with_callback(
                "deviceorientationabsolute",
                absolute.as_ref().unchecked_ref(),
            );
            absolute.forget();
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

/// Where the phone's top edge points, degrees from north, once known.
pub fn heading() -> Option<f64> {
    HEADING.get()
}

fn geolocation() -> Option<wasm_bindgen::JsValue> {
    let navigator = web_sys::window()?.navigator();
    js_sys::Reflect::get(&navigator, &"geolocation".into())
        .ok()
        .filter(|geolocation| !geolocation.is_undefined())
}

/// Whether the browser can tell where the device is.
pub fn can_locate() -> bool {
    geolocation().is_some()
}

/// A position from the browser: latitude, longitude (degrees) and how far
/// off it may be (m), when the browser says.
pub type Fix = (f64, f64, Option<f64>);

/// Reads a `GeolocationPosition`.
fn read_fix(position: &wasm_bindgen::JsValue) -> Option<Fix> {
    let coordinate = |name: &str| {
        js_sys::Reflect::get(position, &"coords".into())
            .and_then(|coords| js_sys::Reflect::get(&coords, &name.into()))
            .ok()
            .and_then(|value| value.as_f64())
    };
    Some((
        coordinate("latitude")?,
        coordinate("longitude")?,
        coordinate("accuracy"),
    ))
}

/// What a `GeolocationPositionError` means, for the hunter.
fn position_error(error: &wasm_bindgen::JsValue) -> String {
    let code = js_sys::Reflect::get(error, &"code".into())
        .ok()
        .and_then(|code| code.as_f64());
    match code {
        Some(1.0) => tr("the browser wasn't allowed to tell where you are").into(),
        Some(3.0) => tr("finding your position took too long").into(),
        _ => tr("your position couldn't be found").into(),
    }
}

/// The options for a position: `precise` asks for the GPS's best and a
/// fresh answer; otherwise one from the last ten minutes will do.
fn position_options(precise: bool) -> js_sys::Object {
    let options = js_sys::Object::new();
    let set = |name: &str, value: wasm_bindgen::JsValue| {
        let _ = js_sys::Reflect::set(&options, &name.into(), &value);
    };
    if precise {
        set("enableHighAccuracy", true.into());
        set("timeout", 30000.into());
        set("maximumAge", 0.into());
    } else {
        set("timeout", 15000.into());
        set("maximumAge", 600000.into());
    }
    options
}

/// Asks the browser where the device is (it asks the hunter first); `done`
/// gets latitude and longitude, or why not.
pub fn locate(done: impl Fn(Result<(f64, f64), String>) + 'static) {
    position(false, move |fix| {
        done(fix.map(|(latitude, longitude, _)| (latitude, longitude)))
    });
}

/// Asks the browser once where the device is, as exactly as it can with
/// `precise` (it asks the hunter first); `done` gets the answer.
pub fn position(precise: bool, done: impl Fn(Result<Fix, String>) + 'static) {
    let Some(geolocation) = geolocation() else {
        return done(Err(tr("this browser can't tell where you are").into()));
    };
    let done = std::rc::Rc::new(done);
    let found = {
        let done = done.clone();
        wasm_bindgen::closure::Closure::once_into_js(move |position: wasm_bindgen::JsValue| {
            done(read_fix(&position).ok_or_else(|| tr("the browser gave no position").to_owned()));
        })
    };
    let failed = {
        let done = done.clone();
        wasm_bindgen::closure::Closure::once_into_js(move |error: wasm_bindgen::JsValue| {
            done(Err(position_error(&error)));
        })
    };
    let started = js_sys::Reflect::get(&geolocation, &"getCurrentPosition".into())
        .ok()
        .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
        .map(|function| function.call3(&geolocation, &found, &failed, &position_options(precise)));
    if !matches!(started, Some(Ok(_))) {
        done(Err(tr("this browser can't tell where you are").into()));
    }
}

/// A running `watchPosition`: its id, and the callbacks it calls.
struct Watch {
    id: wasm_bindgen::JsValue,
    _found: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>,
    _failed: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>,
}

thread_local! {
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
    /// The latest answer while following the device's position.
    static FOLLOWED: RefCell<Option<Result<Fix, String>>> = const { RefCell::new(None) };
}

/// Starts following the device's position as it moves (the browser asks
/// the hunter first). Answers come in `followed`.
pub fn follow(context: egui::Context) {
    if WATCH.with_borrow(Option::is_some) {
        return;
    }
    let Some(geolocation) = geolocation() else {
        FOLLOWED.set(Some(
            Err(tr("this browser can't tell where you are").into()),
        ));
        return;
    };
    let repaint = context.clone();
    let found = wasm_bindgen::closure::Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(
        move |position: wasm_bindgen::JsValue| {
            if let Some(fix) = read_fix(&position) {
                FOLLOWED.set(Some(Ok(fix)));
                repaint.request_repaint();
            }
        },
    );
    let failed = wasm_bindgen::closure::Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(
        move |error: wasm_bindgen::JsValue| {
            // A position found before is still the best there is.
            if !matches!(FOLLOWED.with_borrow(Clone::clone), Some(Ok(_))) {
                FOLLOWED.set(Some(Err(position_error(&error))));
            }
            context.request_repaint();
        },
    );
    let id = js_sys::Reflect::get(&geolocation, &"watchPosition".into())
        .ok()
        .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
        .and_then(|function| {
            function
                .call3(
                    &geolocation,
                    found.as_ref(),
                    failed.as_ref(),
                    &position_options(true),
                )
                .ok()
        });
    match id {
        Some(id) => WATCH.set(Some(Watch {
            id,
            _found: found,
            _failed: failed,
        })),
        None => FOLLOWED.set(Some(
            Err(tr("this browser can't tell where you are").into()),
        )),
    }
}

/// The latest position while following, or why there is none.
pub fn followed() -> Option<Result<Fix, String>> {
    FOLLOWED.with_borrow(Clone::clone)
}

/// Stops following the device's position.
pub fn stop_following() {
    if let Some(watch) = WATCH.take()
        && let Some(geolocation) = geolocation()
    {
        let _ = call(&geolocation, "clearWatch", Some(&watch.id));
    }
    FOLLOWED.set(None);
}

/// How many minutes the device's clock is ahead of UTC at a moment (an hour
/// in Swedish winter, two in summer).
pub fn utc_offset_minutes(unix_ms: i64) -> f64 {
    // JavaScript counts the other way: UTC minus local.
    -js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(unix_ms as f64)).get_timezone_offset()
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

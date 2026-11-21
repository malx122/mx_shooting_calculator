//! Browser-only pieces: start-up in a canvas, saving a file as a download,
//! and picking a file to import. The desktop build uses native dialogs.

use std::cell::{Cell, RefCell};
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

thread_local! {
    /// The photo picker open now; its listeners live until the next one
    /// opens, as a listener can't be dropped while it runs.
    static PICKER: RefCell<Option<Picker>> = const { RefCell::new(None) };
}

/// A photo picker on the page: the hidden file input, the page's own button
/// when one more tap is needed, and their listeners.
struct Picker {
    input: web_sys::HtmlInputElement,
    overlay: Option<web_sys::Element>,
    /// Listeners on the window, taken off when the picker is done.
    on_window: Vec<(&'static str, js_sys::Function)>,
    listeners: Vec<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>>,
}

impl Picker {
    /// Takes the picker off the page.
    fn close(&mut self) {
        self.input.remove();
        if let Some(overlay) = self.overlay.take() {
            overlay.remove();
        }
        if let Some(window) = web_sys::window() {
            for (event, function) in self.on_window.drain(..) {
                let _ = window.remove_event_listener_with_callback(event, &function);
            }
        }
    }

    fn listen(
        &mut self,
        target: &web_sys::EventTarget,
        event: &'static str,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) {
        let listener = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(handler);
        let function: js_sys::Function = listener
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        let _ = target.add_event_listener_with_callback(event, &function);
        if target.dyn_ref::<web_sys::Window>().is_some() {
            self.on_window.push((event, function));
        }
        self.listeners.push(listener);
    }
}

fn close_picker() {
    PICKER.with(|picker| {
        if let Some(picker) = picker.borrow_mut().as_mut() {
            picker.close();
        }
    });
}

/// Opens the browser's picker for a photo: any image, and with `camera` the
/// phone's camera first (`capture="environment"`). Neither asks for a
/// permission. The photo is read in the background and put in `slot`.
/// Browsers open a picker only straight after a tap; when the tap has
/// passed, or the picker didn't open, a button on the page asks for one
/// more.
pub fn pick_photo(
    slot: Rc<RefCell<Option<crate::files::File>>>,
    context: egui::Context,
    camera: bool,
) {
    if let Some(mut previous) = PICKER.with(|picker| picker.borrow_mut().take()) {
        previous.close();
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let Ok(input) = document
        .create_element("input")
        .map(|element| element.unchecked_into::<web_sys::HtmlInputElement>())
    else {
        return;
    };
    input.set_type("file");
    input.set_accept("image/*");
    if camera {
        let _ = input.set_attribute("capture", "environment");
    }
    let _ = input.set_attribute(
        "style",
        "position:fixed;left:-1000px;top:0;width:1px;height:1px;opacity:0",
    );
    let _ = body.append_child(&input);
    let mut picker = Picker {
        input: input.clone(),
        overlay: None,
        on_window: Vec::new(),
        listeners: Vec::new(),
    };
    let picked = input.clone();
    let repaint = context.clone();
    picker.listen(&input, "change", move |_| {
        let file = picked.files().and_then(|files| files.get(0));
        close_picker();
        let Some(file) = file else {
            return;
        };
        let slot = slot.clone();
        let repaint = repaint.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(buffer) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            *slot.borrow_mut() = Some((file.name(), bytes));
            repaint.request_repaint();
        });
    });
    picker.listen(&input, "cancel", |_| close_picker());
    let opened = Rc::new(Cell::new(false));
    let blurred = opened.clone();
    picker.listen(&window, "blur", move |_| blurred.set(true));
    PICKER.with(|cell| *cell.borrow_mut() = Some(picker));

    let activation = js_sys::Reflect::get(&window.navigator(), &"userActivation".into())
        .ok()
        .filter(|activation| !activation.is_undefined())
        .and_then(|activation| js_sys::Reflect::get(&activation, &"isActive".into()).ok())
        .and_then(|active| active.as_bool());
    match activation {
        Some(false) => show_picker_button(&context, camera),
        Some(true) => input.click(),
        None => {
            // Without the activation API: a picker that opened takes the
            // focus from the window; one that didn't leaves it.
            input.click();
            let check = wasm_bindgen::closure::Closure::once_into_js(move || {
                let waiting = PICKER.with(|cell| {
                    cell.borrow()
                        .as_ref()
                        .is_some_and(|picker| picker.input.is_connected())
                });
                if waiting && !opened.get() {
                    show_picker_button(&context, camera);
                }
            });
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(check.unchecked_ref(), 700);
        }
    }
}

/// The page's own button for the picker, for the tap the browser asks for.
/// Cancel, Escape and Back close it.
fn show_picker_button(context: &egui::Context, camera: bool) {
    let palette = crate::theme::palette(context);
    let (Some(window), Some(document)) = (
        web_sys::window(),
        web_sys::window().and_then(|window| window.document()),
    ) else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    PICKER.with(|cell| {
        let mut cell = cell.borrow_mut();
        let Some(picker) = cell.as_mut() else {
            return;
        };
        if picker.overlay.is_some() {
            return;
        }
        let element = |tag: &str, style: String, text: Option<&str>| {
            let element = document.create_element(tag).ok()?;
            let _ = element.set_attribute("style", &style);
            element.set_text_content(text);
            Some(element)
        };
        let font = "font:15px/1.4 system-ui,sans-serif;letter-spacing:0.02em";
        let button = |background: String, colour: String| {
            format!(
                "{font};font-weight:600;padding:12px 20px;border-radius:12px;border:0;\
                 background:{background};color:{colour}"
            )
        };
        let (Some(overlay), Some(card), Some(note), Some(open), Some(cancel)) = (
            element(
                "div",
                "position:fixed;inset:0;z-index:1000;display:flex;align-items:center;\
                 justify-content:center;background:rgba(0,0,0,0.6)"
                    .to_owned(),
                None,
            ),
            element(
                "div",
                format!(
                    "{font};max-width:300px;margin:16px;padding:20px;border-radius:14px;\
                     text-align:center;background:{};color:{}",
                    palette.background.to_hex(),
                    palette.text.to_hex()
                ),
                None,
            ),
            element(
                "p",
                "margin:0 0 16px".to_owned(),
                Some(if camera {
                    tr("The browser asks for one more tap before it opens the camera.")
                } else {
                    tr("The browser asks for one more tap before it opens your photos.")
                }),
            ),
            element(
                "button",
                button(palette.accent.to_hex(), palette.on_accent.to_hex()),
                Some(if camera {
                    tr("Take photo")
                } else {
                    tr("Choose photo")
                }),
            ),
            element(
                "button",
                format!(
                    "{};margin-left:10px",
                    button(palette.raised.to_hex(), palette.text.to_hex())
                ),
                Some(tr("Cancel")),
            ),
        ) else {
            return;
        };
        let _ = card.append_child(&note);
        let _ = card.append_child(&open);
        let _ = card.append_child(&cancel);
        let _ = overlay.append_child(&card);
        let _ = body.append_child(&overlay);
        picker.listen(&open, "click", |_| {
            PICKER.with(|cell| {
                if let Some(picker) = cell.borrow_mut().as_mut() {
                    if let Some(overlay) = picker.overlay.take() {
                        overlay.remove();
                    }
                    picker.input.click();
                }
            });
        });
        picker.listen(&cancel, "click", |_| close_picker());
        picker.listen(&window, "keydown", |event| {
            let key = js_sys::Reflect::get(&event, &"key".into())
                .ok()
                .and_then(|key| key.as_string());
            if key.as_deref() == Some("Escape") {
                close_picker();
            }
        });
        picker.listen(&window, "popstate", |_| close_picker());
        picker.overlay = Some(overlay);
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

/// Where the web app may run, as the sync server knows them (`RETURNS` in
/// its google.js): the reply to ask for is `w` and the number.
const ORIGINS: [&str; 3] = [
    "https://malx122.github.io",
    "http://127.0.0.1:8080",
    "http://localhost:8080",
];

/// Listens for the pick-up code of `session`, which google.html (where the
/// sync server sends the sign-in window back to, on this app's own site)
/// hands on, and puts it in `pick_up`. Returns the reply to ask the server
/// for, or `None` when the app runs somewhere the server won't send it.
pub fn listen_for_google(session: &str, pick_up: &crate::google::PickUp) -> Option<String> {
    let window = web_sys::window()?;
    let origin = window.location().origin().ok()?;
    let index = ORIGINS.iter().position(|known| *known == origin)?;
    let channel = web_sys::BroadcastChannel::new("mxsc-google").ok()?;
    let (session, pick_up) = (session.to_owned(), std::sync::Arc::clone(pick_up));
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
        move |event: web_sys::MessageEvent| {
            let text = |name: &str| {
                js_sys::Reflect::get(&event.data(), &name.into())
                    .ok()
                    .and_then(|value| value.as_string())
            };
            if text("session").as_deref() != Some(session.as_str()) {
                return;
            }
            if let Some(code) = text("pickup")
                .as_deref()
                .and_then(crate::google::pick_up_code)
                && let Ok(mut slot) = pick_up.lock()
            {
                *slot = Some(code);
            }
        },
    );
    channel.set_onmessage(Some(closure.as_ref().unchecked_ref()));
    closure.forget();
    // Kept open for the page's life: a later sign-in opens its own.
    std::mem::forget(channel);
    Some(format!("w{index}"))
}

/// Opens the database kept in this browser's IndexedDB. SQLite works on
/// a copy in memory that is written back to IndexedDB in the background
/// after each change.
async fn open_store() -> Result<Store, String> {
    use crate::relaxed_idb::{RelaxedIdbCfg, install};
    if !hold_database_lock().await {
        return Err(tr(
            "The app is already open in another tab or window. Use that one, or close it and \
             reload this page.",
        )
        .into());
    }
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

/// Takes the database's lock for this page's life (Web Locks), so a second
/// tab of the app can't work on the database at the same time: each keeps
/// its own copy in memory and writes it back, which can corrupt it. False
/// if another tab holds it; true in browsers without Web Locks.
async fn hold_database_lock() -> bool {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return true;
    };
    let locks = js_sys::Reflect::get(&window.navigator(), &"locks".into()).unwrap_or_default();
    let Some(request) = js_sys::Reflect::get(&locks, &"request".into())
        .ok()
        .and_then(|request| request.dyn_into::<js_sys::Function>().ok())
    else {
        return true;
    };
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"ifAvailable".into(), &JsValue::TRUE);
    let granted = js_sys::Promise::new(&mut |resolve, _| {
        // Called with the lock, or null if another tab has it. The lock is
        // held until the promise it returns settles: never.
        let answer = resolve.clone();
        let held = wasm_bindgen::closure::Closure::<dyn FnMut(JsValue) -> JsValue>::new(
            move |lock: JsValue| {
                let _ = answer.call1(&JsValue::NULL, &JsValue::from_bool(!lock.is_null()));
                js_sys::Promise::new(&mut |_, _| {}).into()
            },
        );
        if request
            .call3(&locks, &"mxsc-database".into(), &options, held.as_ref())
            .is_err()
        {
            let _ = resolve.call1(&JsValue::NULL, &JsValue::TRUE);
        }
        held.forget();
    });
    wasm_bindgen_futures::JsFuture::from(granted)
        .await
        .ok()
        .and_then(|granted| granted.as_bool())
        .unwrap_or(true)
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

/// The theme choice in the page's own storage, read by index.html for
/// the loading screen.
const THEME_KEY: &str = "mxsc-theme";

/// Tells the page around the app its theme: the browser's bar takes the
/// header's colour, and the next loading screen the chosen theme.
pub fn show_theme(choice: crate::theme::ThemeChoice, palette: &crate::theme::Palette) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let (Ok(Some(storage)), Ok(serde_json::Value::String(name))) =
        (window.local_storage(), serde_json::to_value(choice))
    {
        let _ = storage.set_item(THEME_KEY, &name);
    }
    let Some(document) = window.document() else {
        return;
    };
    let [r, g, b, _] = palette.chrome.to_array();
    if let Ok(Some(meta)) = document.query_selector("meta[name=theme-color]") {
        let _ = meta.set_attribute("content", &format!("#{r:02X}{g:02X}{b:02X}"));
    }
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("class", if palette.dark { "" } else { "sun" });
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

thread_local! {
    /// Phone views the browser's history has entries for.
    static PUSHED: Cell<usize> = const { Cell::new(0) };
    /// Steps taken with the browser's Back, not yet handled by the app.
    static BACKS: Cell<usize> = const { Cell::new(0) };
    /// Some editor has unsaved changes: leaving the page asks first.
    static UNSAVED: Cell<bool> = const { Cell::new(false) };
    static NAVIGATING: Cell<bool> = const { Cell::new(false) };
}

/// The key of the view depth in the history entries' state.
const DEPTH_KEY: &str = "mxscDepth";

fn depth_state(depth: usize) -> wasm_bindgen::JsValue {
    let state = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&state, &DEPTH_KEY.into(), &(depth as f64).into());
    state.into()
}

/// Steps back taken with the browser's (or the phone's) Back since the last
/// frame: the app closes that many views.
pub fn take_backs() -> usize {
    BACKS.replace(0)
}

/// Keeps the browser's history in step with the views open over a page on
/// a phone, `depth`, so Back closes them instead of leaving the app; and
/// has the browser ask before the page is left or reloaded with `unsaved`
/// changes.
pub fn navigation(context: &egui::Context, depth: usize, unsaved: bool) {
    UNSAVED.set(unsaved);
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(history) = window.history() else {
        return;
    };
    if !NAVIGATING.replace(true) {
        let reloaded_at = history
            .state()
            .ok()
            .and_then(|state| js_sys::Reflect::get(&state, &DEPTH_KEY.into()).ok())
            .and_then(|depth| depth.as_f64())
            .map_or(0, |depth| depth as usize);
        if reloaded_at > 0 {
            // Reloaded inside views that are gone now: back to the app's
            // own entry, so the next Back doesn't seem to do nothing.
            let _ = history.go_with_delta(-(reloaded_at as i32));
        } else {
            let _ = history.replace_state(&depth_state(0), "");
        }
        let repaint = context.clone();
        let popstate = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            let depth = web_sys::window()
                .and_then(|window| window.history().ok())
                .and_then(|history| history.state().ok())
                .and_then(|state| js_sys::Reflect::get(&state, &DEPTH_KEY.into()).ok())
                .and_then(|depth| depth.as_f64())
                .map_or(0, |depth| depth as usize);
            let pushed = PUSHED.replace(depth);
            // Forward needs nothing: the views can't be opened again, so the
            // next frame takes the entries off again.
            if depth < pushed {
                BACKS.set(BACKS.get() + pushed - depth);
            }
            repaint.request_repaint();
        });
        let _ =
            window.add_event_listener_with_callback("popstate", popstate.as_ref().unchecked_ref());
        popstate.forget();
        let before_unload = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            |event: web_sys::Event| {
                if UNSAVED.get() {
                    event.prevent_default();
                    // Older browsers ask only when this is set too.
                    let _ = js_sys::Reflect::set(&event, &"returnValue".into(), &"".into());
                }
            },
        );
        let _ = window.add_event_listener_with_callback(
            "beforeunload",
            before_unload.as_ref().unchecked_ref(),
        );
        before_unload.forget();
    }
    let pushed = PUSHED.get();
    if depth > pushed {
        for level in pushed + 1..=depth {
            let _ = history.push_state(&depth_state(level), "");
        }
        PUSHED.set(depth);
    } else if depth < pushed {
        // Closed in the app: take their entries off the history too.
        let _ = history.go_with_delta(depth as i32 - pushed as i32);
        PUSHED.set(depth);
    }
}

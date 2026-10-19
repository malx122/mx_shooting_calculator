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

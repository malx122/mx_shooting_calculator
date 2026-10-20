//! Picking a file to read, on the desktop (a dialog that answers at once)
//! and in the browser (a picker that answers later). The page keeps a
//! `Picked` and takes the file from it when it has come.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;

/// A file that was picked: its name and contents.
pub type File = (String, Vec<u8>);

/// Where a picked file lands.
#[derive(Clone, Default)]
pub struct Picked(Rc<RefCell<Option<File>>>);

impl Picked {
    /// The file, once it has been picked and read.
    pub fn take(&self) -> Option<File> {
        self.0.borrow_mut().take()
    }

    /// Opens the picker for files with these extensions.
    pub fn pick(&self, context: &egui::Context, kind: &str, extensions: &[&str]) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = context;
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(kind, extensions)
                .pick_file()
                && let Ok(bytes) = std::fs::read(&path)
            {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                *self.0.borrow_mut() = Some((name, bytes));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let slot = self.0.clone();
            let context = context.clone();
            let dialog = rfd::AsyncFileDialog::new().add_filter(kind, extensions);
            wasm_bindgen_futures::spawn_local(async move {
                let Some(handle) = dialog.pick_file().await else {
                    return;
                };
                let bytes = handle.read().await;
                *slot.borrow_mut() = Some((handle.file_name(), bytes));
                context.request_repaint();
            });
        }
    }
}

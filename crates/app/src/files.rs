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

    /// A file that came another way, such as dropped on the window.
    pub fn put(&self, file: File) {
        *self.0.borrow_mut() = Some(file);
    }

    /// Opens the picker for a photo. In the browser it takes any image, and
    /// with `camera` asks for the phone's camera; neither asks for a
    /// permission. On a computer `camera` makes no difference.
    pub fn pick_photo(&self, context: &egui::Context, camera: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = camera;
            self.pick(
                context,
                crate::i18n::tr("Photos"),
                &["jpg", "jpeg", "png", "webp", "heic", "heif"],
            );
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::pick_photo(self.0.clone(), context.clone(), camera);
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

/// A file the app made, to save on the device.
pub struct FileOut {
    pub name: String,
    pub bytes: Vec<u8>,
    /// Its type, for the browser's download.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub mime: &'static str,
}

/// Saves a file: through a save dialog on a computer, as a download in the
/// browser. Answers what was done, or `None` when the hunter cancelled.
pub fn save(file: &FileOut) -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let extension = file
            .name
            .rsplit_once('.')
            .map_or("", |(_, extension)| extension);
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&file.name)
            .add_filter(extension.to_uppercase(), &[extension])
            .save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, &file.bytes)
            .map(|()| Some(crate::i18n::trf!("Saved {}", path.display())))
            .map_err(|error| crate::i18n::trf!("could not write {}: {}", path.display(), error))
    }
    #[cfg(target_arch = "wasm32")]
    crate::web::download_bytes(&file.name, &file.bytes, file.mime).map(|()| {
        Some(crate::i18n::trf!(
            "Saved as {} in your downloads",
            file.name
        ))
    })
}

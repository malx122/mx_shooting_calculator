//! Bringing a photo in: from the camera or the photo library on a phone,
//! from a dialog or dropped on the window on a computer. The photo is
//! turned upright, reduced unless the originals are kept, and stripped of
//! its metadata (its location with it) before anything keeps it.

use eframe::egui;
use groups_vision::{ImportError, ImportOptions, ImportedPhoto, Thumbnail};

use crate::files::{File, Picked};
use crate::i18n::{tr, trf};

/// Extensions taken as photos when dropped on the window.
const PHOTO_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "heic", "heif"];

/// A photo picked or dropped, on its way in.
#[derive(Default)]
pub struct PhotoImport {
    picked: Picked,
    /// The photo's bytes and the frames "Preparing photo…" has been shown.
    preparing: Option<(Vec<u8>, u8)>,
}

impl PhotoImport {
    /// Opens the picker; `camera` asks for the phone's camera.
    pub fn choose(&self, context: &egui::Context, camera: bool) {
        self.picked.pick_photo(context, camera);
    }

    /// Where a photo picked or dropped lands.
    #[cfg(target_arch = "wasm32")]
    pub fn slot(&self) -> Picked {
        self.picked.clone()
    }

    /// A file dropped on the window, if it is a photo.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn dropped(&self, file: File) -> bool {
        let photo = is_photo_name(&file.0);
        if photo {
            self.picked.put(file);
        }
        photo
    }

    /// Whether a photo is being prepared: "Preparing photo…" is shown.
    pub fn preparing(&self) -> bool {
        self.preparing.is_some()
    }

    /// Takes a picked photo in, with its upright pixels at most `long_edge`
    /// on the long side to show. The work starts only once "Preparing
    /// photo…" has been drawn, as it can take seconds on a phone.
    pub fn poll(
        &mut self,
        context: &egui::Context,
        keep_originals: bool,
        long_edge: u32,
    ) -> Option<Result<(ImportedPhoto, Thumbnail), String>> {
        match &mut self.preparing {
            None => {
                let (_, bytes) = self.picked.take()?;
                self.preparing = Some((bytes, 0));
                context.request_repaint();
                None
            }
            Some((_, shown)) if *shown < 1 => {
                *shown += 1;
                context.request_repaint();
                None
            }
            Some(_) => {
                let (bytes, _) = self.preparing.take()?;
                let options = ImportOptions {
                    max_long_edge_px: if keep_originals {
                        None
                    } else {
                        Some(groups_vision::DEFAULT_MAX_LONG_EDGE_PX)
                    },
                    ..ImportOptions::default()
                };
                Some(
                    groups_vision::import_photo_with_pixels(&bytes, options, long_edge)
                        .map_err(|error| message(&error)),
                )
            }
        }
    }
}

/// Whether a file's name says it is a photo.
pub fn is_photo_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        PHOTO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// Why a photo can't be used, for the shooter.
pub fn message(error: &ImportError) -> String {
    match error {
        ImportError::Heic => tr(
            "This is a HEIC photo, which the app can't read yet. Export it as JPEG, or choose \
             it from the phone's browser, which hands it over as JPEG.",
        )
        .to_owned(),
        ImportError::TooLarge => tr(
            "The photo is too large to open here. Take it at a lower resolution, or make it \
             smaller first.",
        )
        .to_owned(),
        ImportError::Unsupported => {
            tr("This isn't a photo the app reads: use JPEG, PNG or WebP.").to_owned()
        }
        ImportError::Damaged(reason) => trf!("The photo can't be read: {}", reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_are_known_by_their_names_and_errors_say_what_to_do() {
        assert!(is_photo_name("IMG_0042.HEIC"));
        assert!(is_photo_name("target.jpeg"));
        assert!(!is_photo_name("backup.json"));
        assert!(!is_photo_name("jpg"));
        assert!(message(&ImportError::Heic).contains("JPEG"));
        assert!(message(&ImportError::TooLarge).contains("too large"));
    }

    #[test]
    fn a_heic_photo_is_refused_after_preparing_is_shown() {
        let context = egui::Context::default();
        let mut import = PhotoImport::default();
        let mut heic = vec![0, 0, 0, 24];
        heic.extend_from_slice(b"ftypheic\0\0\0\0mif1heic");
        assert!(import.dropped(("IMG_1.HEIC".into(), heic)));
        assert!(import.poll(&context, false, 2048).is_none(), "taken in");
        assert!(import.preparing());
        assert!(
            import.poll(&context, false, 2048).is_none(),
            "shown for a frame first"
        );
        let refused = import.poll(&context, false, 2048).expect("worked on");
        assert_eq!(refused.err(), Some(message(&ImportError::Heic)));
        assert!(!import.preparing());
    }
}

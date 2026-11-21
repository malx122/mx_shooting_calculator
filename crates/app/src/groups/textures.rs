//! Photos and thumbnails as textures, read from the photo store as it
//! answers. One full photo is kept, only while it is marked or viewed;
//! thumbnails as they are shown.

use std::collections::HashMap;

use ballistics_store::{Pending, Photo, PhotoStore};
use eframe::egui;

use super::marking::PhotoImage;

/// The most thumbnails kept at once.
const THUMBNAILS_KEPT: usize = 48;

enum Load {
    Waiting(Pending<Option<Vec<u8>>>),
    Ready(PhotoImage),
    /// The store has no such photo: it was taken on another device.
    Missing,
    Failed(String),
}

/// How a photo's texture is coming along.
pub enum State<'a> {
    Loading,
    Ready(&'a PhotoImage),
    Missing,
    Failed(&'a str),
}

#[derive(Default)]
pub struct Textures {
    full: Option<(String, Load)>,
    thumbnails: HashMap<String, Load>,
}

/// The longest side a photo is shown at: the device's limit, at most 4096
/// pixels, and 2048 on a phone, where memory is short.
pub fn display_long_edge(context: &egui::Context) -> u32 {
    let most = if crate::mobile::is_narrow(context) {
        2048
    } else {
        4096
    };
    (context.input(|input| input.max_texture_side) as u32).clamp(512, most)
}

/// A texture of pixels already decoded; `size_px` is the photo's own.
pub fn from_pixels(
    context: &egui::Context,
    id: &str,
    picture: &groups_vision::Thumbnail,
    size_px: [u32; 2],
) -> PhotoImage {
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [picture.width_px as usize, picture.height_px as usize],
        &picture.rgba,
    );
    PhotoImage {
        id: id.to_owned(),
        texture: context.load_texture(format!("photo-{id}"), image, egui::TextureOptions::LINEAR),
        size_px,
    }
}

/// Decodes a photo into a texture at most `long_edge` pixels on its long
/// side; `size_px` stays the photo's own.
pub fn decode(
    context: &egui::Context,
    id: &str,
    bytes: &[u8],
    size_px: [u32; 2],
    long_edge: u32,
) -> Result<PhotoImage, String> {
    let picture = groups_vision::thumbnail(bytes, long_edge).map_err(|error| error.to_string())?;
    Ok(from_pixels(context, id, &picture, size_px))
}

fn poll(load: &mut Load, context: &egui::Context, id: &str, size_px: [u32; 2], long_edge: u32) {
    if let Load::Waiting(pending) = load
        && let Some(answer) = pending.take()
    {
        *load = match answer {
            Ok(Some(bytes)) => match decode(context, id, &bytes, size_px, long_edge) {
                Ok(image) => Load::Ready(image),
                Err(error) => Load::Failed(error),
            },
            Ok(None) => Load::Missing,
            Err(error) => Load::Failed(error.to_string()),
        };
    }
}

fn state(load: &Load) -> State<'_> {
    match load {
        Load::Waiting(_) => State::Loading,
        Load::Ready(image) => State::Ready(image),
        Load::Missing => State::Missing,
        Load::Failed(error) => State::Failed(error),
    }
}

impl Textures {
    /// The photo, at the size the device can show, asked of the store the
    /// first time.
    pub fn photo(
        &mut self,
        context: &egui::Context,
        store: &dyn PhotoStore,
        photo: &Photo,
    ) -> State<'_> {
        if self.full.as_ref().is_none_or(|(id, _)| *id != photo.id) {
            self.full = Some((photo.id.clone(), Load::Waiting(store.get(&photo.id))));
        }
        let long_edge = display_long_edge(context);
        let (id, load) = self.full.as_mut().expect("just set");
        poll(
            load,
            context,
            id,
            [photo.width_px, photo.height_px],
            long_edge,
        );
        if matches!(load, Load::Waiting(_)) {
            context.request_repaint_after(std::time::Duration::from_millis(50));
        }
        state(load)
    }

    /// A photo just imported, ready to mark.
    pub fn keep_photo(&mut self, image: PhotoImage) {
        self.full = Some((image.id.clone(), Load::Ready(image)));
    }

    /// Lets the full photo go.
    pub fn release_photo(&mut self) {
        self.full = None;
    }

    /// The photo's thumbnail.
    pub fn thumbnail(
        &mut self,
        context: &egui::Context,
        store: &dyn PhotoStore,
        photo: &Photo,
    ) -> State<'_> {
        if !self.thumbnails.contains_key(&photo.id) {
            if self.thumbnails.len() >= THUMBNAILS_KEPT {
                self.thumbnails.clear();
            }
            self.thumbnails.insert(
                photo.id.clone(),
                Load::Waiting(store.get_thumbnail(&photo.id)),
            );
        }
        let load = self.thumbnails.get_mut(&photo.id).expect("just put in");
        poll(
            load,
            context,
            &photo.id,
            [photo.width_px, photo.height_px],
            groups_vision::THUMBNAIL_LONG_EDGE_PX,
        );
        if matches!(load, Load::Waiting(_)) {
            context.request_repaint_after(std::time::Duration::from_millis(50));
        }
        state(load)
    }
}

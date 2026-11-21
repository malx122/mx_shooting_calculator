//! The backup with photos: a `.zip` of the backup file and the photos the
//! groups are marked on. Writing one reads the photos one at a time into
//! the zip; reading one puts its photos back in the photo store, one a
//! frame.

use std::collections::VecDeque;

use ballistics_store::bundle::BundleWriter;
use ballistics_store::{Pending, PhotoStore};

use crate::files::FileOut;
use crate::i18n::{tr, trf};

/// The backup with photos' file name.
pub const FILE_NAME: &str = "mxsc-backup-with-photos.zip";

/// A backup with photos being written: each photo is read from the photo
/// store and added to the zip before the next is asked for.
pub struct PhotoExport {
    writer: Option<BundleWriter>,
    /// The photos still to read, next first.
    ids: VecDeque<String>,
    reading: Option<(String, Pending<Option<Vec<u8>>>)>,
    /// Photos this device doesn't have (they are on another one).
    missing: usize,
}

impl PhotoExport {
    /// Starts the zip with the backup `json`; the photos named `ids` follow.
    pub fn start(json: &str, ids: Vec<String>) -> Result<PhotoExport, String> {
        Ok(PhotoExport {
            writer: Some(BundleWriter::new(json).map_err(|error| error.to_string())?),
            ids: ids.into(),
            reading: None,
            missing: 0,
        })
    }

    /// Adds the photos as the store answers; the zip to save, with what to
    /// say, once the last is in.
    pub fn poll(&mut self, photos: &dyn PhotoStore) -> Option<Result<(FileOut, String), String>> {
        let writer = self.writer.as_mut()?;
        loop {
            if let Some((id, pending)) = &self.reading {
                match pending.take()? {
                    Ok(Some(bytes)) => {
                        if let Err(error) = writer.add_photo(id, &bytes) {
                            self.writer = None;
                            return Some(Err(error.to_string()));
                        }
                    }
                    Ok(None) | Err(_) => self.missing += 1,
                }
                self.reading = None;
            }
            let Some(id) = self.ids.pop_front() else {
                break;
            };
            let pending = photos.get(&id);
            self.reading = Some((id, pending));
        }
        let writer = self.writer.take()?;
        let kept = writer.photos();
        let said = match self.missing {
            0 => trf!("{} photos in the backup", kept),
            missing => trf!(
                "{} photos in the backup; {} are on another device and not in it",
                kept,
                missing
            ),
        };
        Some(
            writer
                .finish()
                .map(|bytes| {
                    let file = FileOut {
                        name: FILE_NAME.to_owned(),
                        bytes,
                        mime: "application/zip",
                    };
                    (file, said)
                })
                .map_err(|error| error.to_string()),
        )
    }
}

/// Photos from a backup waiting to be put back in the photo store.
#[derive(Default)]
pub struct PhotoRestore(VecDeque<(String, Vec<u8>)>);

impl PhotoRestore {
    pub fn add(&mut self, photos: Vec<(String, Vec<u8>)>) {
        self.0.extend(photos);
    }

    /// Keeps the next photo with a new thumbnail: the store's answers to
    /// wait for, or `None` when none is left.
    pub fn next(
        &mut self,
        photos: &dyn PhotoStore,
    ) -> Option<(Pending<String>, Option<Pending<()>>)> {
        let (id, bytes) = self.0.pop_front()?;
        let thumbnail = groups_vision::thumbnail_jpeg(&bytes)
            .ok()
            .map(|small| photos.put_thumbnail(&id, small));
        Some((photos.put(bytes), thumbnail))
    }
}

/// The note after restoring photos.
pub fn restored_text(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => tr("1 photo restored").to_owned(),
        count => trf!("{} photos restored", count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::bundle::Bundle;
    use ballistics_store::{Group, MemoryPhotos, Photo, Store, photo_id};

    /// A small JPEG as the photo store keeps them.
    fn jpeg() -> Vec<u8> {
        let mut pixmap = tiny_skia::Pixmap::new(32, 24).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(200, 120, 40, 255));
        let png = pixmap.encode_png().unwrap();
        groups_vision::import_photo(&png, groups_vision::ImportOptions::default())
            .unwrap()
            .jpeg
    }

    #[test]
    fn a_backup_with_photos_brings_the_photos_and_records_back() {
        let store = Store::open_in_memory().unwrap();
        let photos = MemoryPhotos::default();
        let bytes = jpeg();
        let id = photos.put(bytes.clone()).take().unwrap().unwrap();
        assert_eq!(id, photo_id(&bytes));
        let mut photo = Photo::new(&id, 32, 24);
        store.save_photo(&mut photo).unwrap();
        let mut group = Group::on_photo(&id, 1_000);
        store.save_group(&mut group).unwrap();

        let json = store.export_json().unwrap();
        let mut export = PhotoExport::start(&json, vec![id.clone()]).unwrap();
        let (file, said) = export.poll(&photos).unwrap().unwrap();
        assert!(said.contains('1'));
        assert_eq!(file.name, FILE_NAME);

        let bundle = Bundle::read(&file.bytes).unwrap();
        assert_eq!(bundle.photos, vec![(id.clone(), bytes.clone())]);
        let mut other = Store::open_in_memory().unwrap();
        let report = other.import_json(&bundle.json).unwrap();
        assert!(report.changed());
        assert_eq!(other.groups().unwrap().len(), 1);
        assert_eq!(other.photos().unwrap().len(), 1);
        let restored = MemoryPhotos::default();
        let mut restore = PhotoRestore::default();
        restore.add(bundle.photos);
        let (kept, thumbnail) = restore.next(&restored).unwrap();
        assert_eq!(kept.take().unwrap().unwrap(), id);
        thumbnail.unwrap().take().unwrap().unwrap();
        assert!(restore.next(&restored).is_none());
        assert_eq!(restored.get(&id).take().unwrap().unwrap(), Some(bytes));
        assert!(
            restored
                .get_thumbnail(&id)
                .take()
                .unwrap()
                .unwrap()
                .is_some()
        );
    }
}

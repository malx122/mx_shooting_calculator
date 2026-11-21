//! Target photos. The database keeps what is known about each photo
//! (`Photo`); the bytes are kept apart, by a `PhotoStore`, because in the
//! browser the database lives in memory and is written back whole after
//! every change. A photo is known by the SHA-256 of its bytes, so the same
//! photo is kept once however often it is imported.
//!
//! Photo records are never remembered as deleted: a record goes when no
//! group is marked on it, on each device by itself, so a deletion on one
//! device can't take the photo from a group made on another.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::groups::Calibration;
use crate::model::first_failure;

/// A photo of a target that groups are marked on. Only its description is
/// in the file; the photo itself is kept on the device beside the data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Photo {
    /// The lowercase hexadecimal SHA-256 of the kept photo's bytes, which
    /// also names it where it is kept.
    pub id: String,
    /// Its width as kept: upright, after any reduction.
    pub width_px: u32,
    /// Its height as kept.
    pub height_px: u32,
    /// The type of the kept bytes, `image/jpeg`.
    #[serde(default = "jpeg")]
    pub mime: String,
    /// When the camera says it was taken, in milliseconds since 1970 (UTC);
    /// `null` when it doesn't say.
    #[serde(default)]
    pub taken_at: Option<i64>,
    /// How its pixels map to millimetres on the target, for every group
    /// marked on it; `null` until the scale is set.
    #[serde(default)]
    pub calibration: Option<Calibration>,
    /// When the record was last changed, in milliseconds since 1970 (UTC).
    #[serde(default)]
    pub updated_at: i64,
}

fn jpeg() -> String {
    "image/jpeg".to_owned()
}

impl Photo {
    /// The record of a kept JPEG.
    pub fn new(id: impl Into<String>, width_px: u32, height_px: u32) -> Self {
        Photo {
            id: id.into(),
            width_px,
            height_px,
            mime: jpeg(),
            taken_at: None,
            calibration: None,
            updated_at: 0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let calibration = self.calibration.as_ref().and_then(Calibration::problem);
        let checks = [
            (
                is_photo_id(&self.id),
                "a photo's id is the SHA-256 of its bytes",
            ),
            (
                (1..=100_000).contains(&self.width_px) && (1..=100_000).contains(&self.height_px),
                "a photo must be 1 to 100 000 pixels each way",
            ),
            (
                self.mime.starts_with("image/") && self.mime.len() <= 64,
                "a photo's type must be an image type",
            ),
            (
                self.taken_at.is_none_or(|at| at >= 0),
                "the photo's time is before 1970",
            ),
            (calibration.is_none(), calibration.unwrap_or_default()),
        ];
        first_failure(&checks, "")
    }
}

/// A photo's id: the lowercase hexadecimal SHA-256 of its bytes.
pub fn photo_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Whether `text` is a photo id: 64 lowercase hexadecimal digits, and so
/// also safe as a file name.
pub fn is_photo_id(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Why a photo couldn't be kept, read or forgotten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhotoError(pub String);

impl fmt::Display for PhotoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PhotoError {}

/// The answer to a request to a `PhotoStore`, which may come on a later
/// frame: ask `take` each frame until it gives it. The store asks the UI
/// to repaint when it comes.
pub struct Pending<T>(Arc<Mutex<Slot<T>>>);

enum Slot<T> {
    Waiting,
    Ready(Result<T, PhotoError>),
    Taken,
}

/// Gives a `Pending` its answer.
pub struct Answer<T>(Arc<Mutex<Slot<T>>>);

impl<T> Pending<T> {
    /// A request still being worked on, and what answers it.
    pub fn waiting() -> (Pending<T>, Answer<T>) {
        let slot = Arc::new(Mutex::new(Slot::Waiting));
        (Pending(slot.clone()), Answer(slot))
    }

    /// A request answered at once.
    pub fn ready(result: Result<T, PhotoError>) -> Pending<T> {
        Pending(Arc::new(Mutex::new(Slot::Ready(result))))
    }

    /// The answer, the first time it is asked for after it came; `None`
    /// before and after.
    pub fn take(&self) -> Option<Result<T, PhotoError>> {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *slot, Slot::Taken) {
            Slot::Ready(result) => Some(result),
            other => {
                *slot = other;
                None
            }
        }
    }

    /// Whether the answer has come and not been taken.
    pub fn is_ready(&self) -> bool {
        matches!(
            *self.0.lock().unwrap_or_else(PoisonError::into_inner),
            Slot::Ready(_)
        )
    }
}

impl<T> Answer<T> {
    pub fn send(self, result: Result<T, PhotoError>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Slot::Ready(result);
    }
}

/// Where the bytes of photos, and a thumbnail of each, are kept by id (see
/// `photo_id`). Requests return at once; their answers come through a
/// `Pending`, at once or on a later frame.
pub trait PhotoStore {
    /// Keeps the bytes and answers with their id. Bytes already kept are
    /// kept once.
    fn put(&self, bytes: Vec<u8>) -> Pending<String>;
    /// The bytes kept under `id`; `None` when there are none.
    fn get(&self, id: &str) -> Pending<Option<Vec<u8>>>;
    /// Keeps a thumbnail (a small JPEG) for the photo `id`.
    fn put_thumbnail(&self, id: &str, jpeg: Vec<u8>) -> Pending<()>;
    /// The photo's thumbnail; `None` when there is none.
    fn get_thumbnail(&self, id: &str) -> Pending<Option<Vec<u8>>>;
    /// Forgets the photo kept under `id` and its thumbnail, if there are
    /// any.
    fn delete(&self, id: &str) -> Pending<()>;
    /// The ids of every photo kept, sorted.
    fn list(&self) -> Pending<Vec<String>>;
    /// Forgets every photo and thumbnail whose id isn't in `keep`, and
    /// answers with the ids of the photos forgotten.
    fn retain(&self, keep: BTreeSet<String>) -> Pending<Vec<String>>;
}

/// The refusal for a request naming something that isn't a photo id.
pub fn not_an_id(id: &str) -> PhotoError {
    PhotoError(format!("{id:?} is not a photo id"))
}

/// Photos kept in memory, answering at once: for tests, and for a device
/// that can't keep files.
#[derive(Default)]
pub struct MemoryPhotos(Mutex<Kept>);

#[derive(Default)]
struct Kept {
    photos: BTreeMap<String, Vec<u8>>,
    thumbnails: BTreeMap<String, Vec<u8>>,
}

impl MemoryPhotos {
    fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn named<T>(id: &str, work: impl FnOnce() -> T) -> Pending<T> {
        Pending::ready(if is_photo_id(id) {
            Ok(work())
        } else {
            Err(not_an_id(id))
        })
    }
}

impl PhotoStore for MemoryPhotos {
    fn put(&self, bytes: Vec<u8>) -> Pending<String> {
        let id = photo_id(&bytes);
        self.kept().photos.entry(id.clone()).or_insert(bytes);
        Pending::ready(Ok(id))
    }

    fn get(&self, id: &str) -> Pending<Option<Vec<u8>>> {
        MemoryPhotos::named(id, || self.kept().photos.get(id).cloned())
    }

    fn put_thumbnail(&self, id: &str, jpeg: Vec<u8>) -> Pending<()> {
        MemoryPhotos::named(id, || {
            self.kept().thumbnails.insert(id.to_owned(), jpeg);
        })
    }

    fn get_thumbnail(&self, id: &str) -> Pending<Option<Vec<u8>>> {
        MemoryPhotos::named(id, || self.kept().thumbnails.get(id).cloned())
    }

    fn delete(&self, id: &str) -> Pending<()> {
        MemoryPhotos::named(id, || {
            let mut kept = self.kept();
            kept.photos.remove(id);
            kept.thumbnails.remove(id);
        })
    }

    fn list(&self) -> Pending<Vec<String>> {
        Pending::ready(Ok(self.kept().photos.keys().cloned().collect()))
    }

    fn retain(&self, keep: BTreeSet<String>) -> Pending<Vec<String>> {
        let mut kept = self.kept();
        let forgotten: Vec<String> = kept
            .photos
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        kept.photos.retain(|id, _| keep.contains(id));
        kept.thumbnails.retain(|id, _| keep.contains(id));
        Pending::ready(Ok(forgotten))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer<T>(pending: Pending<T>) -> Result<T, PhotoError> {
        pending.take().expect("answered at once")
    }

    #[test]
    fn ids_are_the_sha256_of_the_bytes() {
        assert_eq!(
            photo_id(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(is_photo_id(&photo_id(b"")));
        assert!(!is_photo_id(&photo_id(b"").to_uppercase()));
        assert!(!is_photo_id("../../etc/passwd"));
        assert!(!is_photo_id(&"a".repeat(63)));
    }

    #[test]
    fn a_pending_answer_is_taken_once() {
        let (pending, answer) = Pending::<u8>::waiting();
        assert!(!pending.is_ready());
        assert_eq!(pending.take(), None);
        answer.send(Ok(7));
        assert!(pending.is_ready());
        assert_eq!(pending.take(), Some(Ok(7)));
        assert_eq!(pending.take(), None);
    }

    #[test]
    fn memory_photos_keep_the_same_bytes_once_with_their_thumbnails() {
        let photos = MemoryPhotos::default();
        let id = answer(photos.put(b"one".to_vec())).unwrap();
        assert_eq!(answer(photos.put(b"one".to_vec())).unwrap(), id);
        answer(photos.put_thumbnail(&id, b"small one".to_vec())).unwrap();
        let other = answer(photos.put(b"two".to_vec())).unwrap();
        let mut both = vec![id.clone(), other.clone()];
        both.sort();
        assert_eq!(answer(photos.list()).unwrap(), both);
        assert_eq!(answer(photos.get(&id)).unwrap(), Some(b"one".to_vec()));
        assert_eq!(
            answer(photos.get_thumbnail(&id)).unwrap(),
            Some(b"small one".to_vec())
        );
        answer(photos.delete(&id)).unwrap();
        assert_eq!(answer(photos.get(&id)).unwrap(), None);
        assert_eq!(answer(photos.get_thumbnail(&id)).unwrap(), None);
        assert_eq!(answer(photos.list()).unwrap(), vec![other.clone()]);
        assert!(answer(photos.get("nope")).is_err());

        let third = answer(photos.put(b"three".to_vec())).unwrap();
        answer(photos.put_thumbnail(&third, b"small three".to_vec())).unwrap();
        let forgotten = answer(photos.retain(BTreeSet::from([other.clone()]))).unwrap();
        assert_eq!(forgotten, vec![third.clone()]);
        assert_eq!(answer(photos.list()).unwrap(), vec![other]);
        assert_eq!(answer(photos.get_thumbnail(&third)).unwrap(), None);
    }

    #[test]
    fn photo_records_need_a_real_id_size_and_scale() {
        let mut photo = Photo::new(photo_id(b"jpeg"), 4000, 3000);
        photo.taken_at = Some(0);
        assert!(photo.validate().is_ok());
        photo.id = "IMG_0001".into();
        assert!(photo.validate().is_err());
        photo.id = photo_id(b"jpeg");
        photo.width_px = 0;
        assert!(photo.validate().is_err());
        photo.width_px = 4000;
        photo.calibration = Some(Calibration::FourCorner {
            corners_px: [[0.0, 0.0], [10.0, 0.0], [10.0, 0.0], [0.0, 10.0]],
            width_mm: 100.0,
            height_mm: 100.0,
        });
        assert!(photo.validate().is_err(), "two corners are the same");
        photo.calibration = Some(Calibration::FourCorner {
            corners_px: [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            width_mm: 100.0,
            height_mm: 100.0,
        });
        assert!(photo.validate().is_ok());
    }
}

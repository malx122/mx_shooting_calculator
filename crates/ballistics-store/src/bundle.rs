//! A backup with photos: one `.zip` holding the backup file
//! (`mxsc-backup.json`, as **Export backup** writes it) and the photos the
//! groups are marked on (`photos/<id>.jpg`, each named by its id, the
//! SHA-256 of its bytes). Reading one checks the sizes before anything is
//! unpacked, and every photo against its name.

use std::collections::{BTreeSet, HashSet};
use std::io::{Cursor, Read, Write};

use crate::StoreError;
use crate::photos::{is_photo_id, photo_id};
use crate::transfer::read_export;

/// The backup file's name inside the zip.
pub const BACKUP_NAME: &str = "mxsc-backup.json";
/// The folder the photos are in, inside the zip.
pub const PHOTOS_FOLDER: &str = "photos/";
const MIB: u64 = 1024 * 1024;

/// How much a backup with photos may unpack to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Every file in the zip together, unpacked.
    pub total_bytes: u64,
    /// The backup file, unpacked.
    pub backup_bytes: u64,
    /// One photo.
    pub photo_bytes: u64,
    /// Files in the zip.
    pub entries: usize,
    /// The most a file larger than a mebibyte may shrink in the zip.
    pub ratio: u64,
}

impl Default for Limits {
    /// Less in the browser, where a phone's tab has little memory.
    fn default() -> Self {
        Limits {
            total_bytes: if cfg!(target_arch = "wasm32") {
                512 * MIB
            } else {
                2048 * MIB
            },
            backup_bytes: 256 * MIB,
            photo_bytes: 64 * MIB,
            entries: 20_000,
            ratio: 100,
        }
    }
}

/// What a backup with photos holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundle {
    /// The backup file, as `Store::export_json` writes it.
    pub json: String,
    /// The photos the backup's photo records name, as `(id, JPEG bytes)`,
    /// by id.
    pub photos: Vec<(String, Vec<u8>)>,
}

/// Whether `bytes` start like a zip file.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

fn invalid(what: impl std::fmt::Display) -> StoreError {
    StoreError::Invalid(format!("the backup with photos can't be read: {what}"))
}

/// Writes a backup with photos one photo at a time, so only the zip so far
/// and the photo being added are held.
pub struct BundleWriter {
    zip: zip::ZipWriter<Cursor<Vec<u8>>>,
    photos: usize,
}

impl BundleWriter {
    /// A zip holding the backup file `json`, deflated.
    pub fn new(json: &str) -> Result<BundleWriter, StoreError> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(json.len() as u64 > u64::from(u32::MAX));
        zip.start_file(BACKUP_NAME, deflated).map_err(invalid)?;
        zip.write_all(json.as_bytes()).map_err(invalid)?;
        Ok(BundleWriter { zip, photos: 0 })
    }

    /// Adds a photo, stored as it is (JPEG doesn't shrink further).
    pub fn add_photo(&mut self, id: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        self.zip
            .start_file(format!("{PHOTOS_FOLDER}{id}.jpg"), stored)
            .map_err(invalid)?;
        self.zip.write_all(bytes).map_err(invalid)?;
        self.photos += 1;
        Ok(())
    }

    /// How many photos were added.
    pub fn photos(&self) -> usize {
        self.photos
    }

    /// The zip file.
    pub fn finish(self) -> Result<Vec<u8>, StoreError> {
        Ok(self.zip.finish().map_err(invalid)?.into_inner())
    }
}

/// Unpacks an entry, refusing one that unpacks to more than it says.
fn unpack(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    index: usize,
    size: u64,
    name: &str,
) -> Result<Vec<u8>, StoreError> {
    let entry = zip.by_index(index).map_err(invalid)?;
    let mut contents = Vec::with_capacity(size.min(MIB) as usize);
    entry
        .take(size + 1)
        .read_to_end(&mut contents)
        .map_err(invalid)?;
    if contents.len() as u64 != size {
        return Err(invalid(format!("{name} isn't the size it says")));
    }
    Ok(contents)
}

/// How many files the zip's end-of-directory record says it holds; `None`
/// when it defers to a zip64 record.
fn declared_entries(bytes: &[u8]) -> Option<usize> {
    const END: &[u8] = b"PK\x05\x06";
    let from = bytes.len().saturating_sub(22 + usize::from(u16::MAX));
    let at = from + bytes[from..].windows(4).rposition(|w| w == END)?;
    let count = u16::from_le_bytes([*bytes.get(at + 10)?, *bytes.get(at + 11)?]);
    (count != u16::MAX).then_some(usize::from(count))
}

impl Bundle {
    /// The zip file.
    pub fn write(&self) -> Result<Vec<u8>, StoreError> {
        let mut writer = BundleWriter::new(&self.json)?;
        for (id, bytes) in &self.photos {
            writer.add_photo(id, bytes)?;
        }
        writer.finish()
    }

    /// Reads a backup with photos within the default limits.
    pub fn read(bytes: &[u8]) -> Result<Bundle, StoreError> {
        Bundle::read_with(bytes, &Limits::default())
    }

    /// Reads a backup with photos. Before anything is unpacked it refuses a
    /// zip with too many files, two files of one name, files that unpack to
    /// more than `limits` allow or shrank suspiciously much, and one
    /// without the backup file. Then only the photos the backup's records
    /// name are unpacked, each refused unless it is a JPEG matching its
    /// name. Other files are ignored.
    pub fn read_with(bytes: &[u8], limits: &Limits) -> Result<Bundle, StoreError> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(invalid)?;
        if zip.len() > limits.entries {
            return Err(invalid("it holds too many files"));
        }
        // The archive keeps one file of each name: more in the directory
        // means two of one name.
        if declared_entries(bytes).is_some_and(|declared| declared != zip.len()) {
            return Err(invalid("it holds two files of one name"));
        }
        let mut names = HashSet::new();
        let mut total = 0_u64;
        let mut backup = None;
        let mut photos = Vec::new();
        for index in 0..zip.len() {
            let entry = zip.by_index_raw(index).map_err(invalid)?;
            let name = entry.name().map_err(invalid)?.into_owned();
            if !names.insert(name.clone()) {
                return Err(invalid(format!("it holds {name} twice")));
            }
            if !entry.is_file() {
                continue;
            }
            let (size, packed) = (entry.size(), entry.compressed_size());
            total = total.saturating_add(size);
            if total > limits.total_bytes {
                return Err(invalid("it unpacks to more than this device can hold"));
            }
            if size > MIB && size / packed.max(1) > limits.ratio {
                return Err(invalid(format!("{name} is packed suspiciously small")));
            }
            if name == BACKUP_NAME {
                if size > limits.backup_bytes {
                    return Err(invalid(format!("{name} is too large")));
                }
                backup = Some((index, size));
            } else if let Some(id) = name
                .strip_prefix(PHOTOS_FOLDER)
                .and_then(|file| file.strip_suffix(".jpg"))
                .filter(|id| is_photo_id(id))
            {
                if size > limits.photo_bytes {
                    return Err(invalid(format!("{name} is too large")));
                }
                photos.push((index, size, id.to_owned(), name));
            }
        }
        let (index, size) = backup.ok_or_else(|| invalid(format!("it has no {BACKUP_NAME}")))?;
        let json = String::from_utf8(unpack(&mut zip, index, size, BACKUP_NAME)?)
            .map_err(|_| invalid(format!("{BACKUP_NAME} isn't text")))?;
        let named: BTreeSet<String> = read_export(&json)?
            .photos
            .into_iter()
            .map(|photo| photo.id)
            .collect();
        let mut bundle = Bundle {
            json,
            photos: Vec::new(),
        };
        for (index, size, id, name) in photos {
            if !named.contains(&id) {
                continue;
            }
            let contents = unpack(&mut zip, index, size, &name)?;
            if !contents.starts_with(&[0xff, 0xd8, 0xff]) || photo_id(&contents) != id {
                return Err(invalid(format!("the photo {name} is damaged")));
            }
            bundle.photos.push((id, contents));
        }
        bundle.photos.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Group, Photo, Store};

    fn photo(seed: u8) -> (String, Vec<u8>) {
        let bytes = vec![0xff, 0xd8, 0xff, 0xe0, seed, seed, 0xff, 0xd9];
        (photo_id(&bytes), bytes)
    }

    /// A backup whose groups are marked on these photos.
    fn backup_naming(photos: &[&(String, Vec<u8>)]) -> String {
        let store = Store::open_in_memory().unwrap();
        for (id, _) in photos {
            store
                .save_photo(&mut Photo::new(id.as_str(), 40, 30))
                .unwrap();
            store.save_group(&mut Group::on_photo(id, 1_000)).unwrap();
        }
        store.export_json().unwrap()
    }

    fn zip_of(files: &[(&str, &[u8], zip::CompressionMethod)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes, method) in files {
            let options = zip::write::SimpleFileOptions::default().compression_method(*method);
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn a_backup_with_photos_reads_back_as_written() {
        let (first, second) = (photo(1), photo(2));
        let bundle = Bundle {
            json: backup_naming(&[&first, &second]),
            photos: vec![first, second],
        };
        let zip = bundle.write().unwrap();
        assert!(is_zip(&zip));
        let mut expected = bundle.clone();
        expected.photos.sort();
        assert_eq!(Bundle::read(&zip).unwrap(), expected);
    }

    #[test]
    fn only_the_photos_the_backup_names_are_unpacked() {
        let (named, stray) = (photo(1), photo(2));
        let bundle = Bundle {
            json: backup_naming(&[&named]),
            photos: vec![named.clone(), stray],
        };
        let read = Bundle::read(&bundle.write().unwrap()).unwrap();
        assert_eq!(read.photos, vec![named]);
    }

    #[test]
    fn a_damaged_photo_or_a_missing_backup_is_refused() {
        let (id, _) = photo(1);
        let json = backup_naming(&[&photo(1)]);
        let bundle = Bundle {
            json: json.clone(),
            photos: vec![(id.clone(), vec![0xff, 0xd8, 0xff, 9])],
        };
        let error = Bundle::read(&bundle.write().unwrap()).unwrap_err();
        assert!(error.to_string().contains("damaged"), "{error}");

        let not_jpeg = b"not a jpeg".to_vec();
        let named = format!("{PHOTOS_FOLDER}{}.jpg", photo_id(&not_jpeg));
        let json = backup_naming(&[&(photo_id(&not_jpeg), not_jpeg.clone())]);
        let zip = zip_of(&[
            (
                BACKUP_NAME,
                json.as_bytes(),
                zip::CompressionMethod::Deflated,
            ),
            (&named, &not_jpeg, zip::CompressionMethod::Stored),
        ]);
        assert!(Bundle::read(&zip).is_err(), "a photo must be a JPEG");

        let zip = zip_of(&[("notes.txt", b"hello", zip::CompressionMethod::Stored)]);
        assert!(Bundle::read(&zip).is_err());
        assert!(Bundle::read(b"not a zip").is_err());
    }

    #[test]
    fn a_zip_that_unpacks_to_too_much_is_refused_before_unpacking() {
        let first = photo(1);
        let json = backup_naming(&[&first]);
        let zip = Bundle {
            json: json.clone(),
            photos: vec![first],
        }
        .write()
        .unwrap();
        let tight = Limits {
            total_bytes: json.len() as u64 + 4,
            ..Limits::default()
        };
        let error = Bundle::read_with(&zip, &tight).unwrap_err();
        assert!(error.to_string().contains("more than"), "{error}");

        let zeros = vec![0_u8; 4 * MIB as usize];
        let bomb = zip_of(&[
            (
                BACKUP_NAME,
                json.as_bytes(),
                zip::CompressionMethod::Deflated,
            ),
            ("filler.bin", &zeros, zip::CompressionMethod::Deflated),
        ]);
        assert!(bomb.len() < 64 * 1024, "zeros pack small");
        let error = Bundle::read(&bomb).unwrap_err();
        assert!(error.to_string().contains("suspiciously"), "{error}");
    }

    #[test]
    fn a_zip_with_two_files_of_one_name_is_refused() {
        let json = backup_naming(&[]);
        let zip = zip_of(&[
            (BACKUP_NAME, json.as_bytes(), zip::CompressionMethod::Stored),
            (
                "mxsc-backup.jsoX",
                json.as_bytes(),
                zip::CompressionMethod::Stored,
            ),
        ]);
        // Both names are as long, so renaming the second keeps the zip whole.
        let renamed = replace_all(&zip, b"mxsc-backup.jsoX", BACKUP_NAME.as_bytes());
        assert!(Bundle::read(&zip).is_ok());
        assert!(Bundle::read(&renamed).is_err());
    }

    fn replace_all(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
        let mut out = bytes.to_vec();
        let mut at = 0;
        while let Some(found) = out[at..].windows(from.len()).position(|w| w == from) {
            let start = at + found;
            out[start..start + from.len()].copy_from_slice(to);
            at = start + from.len();
        }
        out
    }
}

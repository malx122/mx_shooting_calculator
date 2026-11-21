//! Where target photos and their thumbnails are kept (see
//! `ballistics_store::photos`): files in a folder beside the database on a
//! computer, named after it so `--db` and `MXSC_DB` move them too; a
//! separate IndexedDB database in the browser, apart from the one SQLite
//! lives in.
#![allow(dead_code, reason = "the Groups screens use it")]

use ballistics_store::{PhotoStore, Store};
use eframe::egui;

/// Forgets photos no record names (left by an import that stopped half-way,
/// or by records gone since). Nothing is forgotten when the records can't
/// be read.
pub fn forget_unused(store: &Store, context: &egui::Context) {
    let Ok(in_use) = store.photo_ids_in_use() else {
        return;
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = context;
        if let Some(database) = store.file_path() {
            let _ = desktop::FilePhotos::beside(&database).retain(in_use);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = web::BrowserPhotos::new(context.clone()).retain(in_use);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod desktop {
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use ballistics_store::photos::{is_photo_id, not_an_id};
    use ballistics_store::{Pending, PhotoError, PhotoStore, photo_id};

    const PHOTO: &str = ".jpg";
    const THUMBNAIL: &str = ".thumb.jpg";
    const PARTIAL: &str = ".part";

    /// Photos as files `<id>.jpg`, with thumbnails `<id>.thumb.jpg`, in a
    /// folder. Answers come at once.
    pub struct FilePhotos {
        folder: PathBuf,
    }

    impl FilePhotos {
        /// The photos of the database file at `database`: in a folder beside
        /// it named after it (`mxsc-photos` for `mxsc.sqlite`), so two
        /// databases in one folder never share photos.
        pub fn beside(database: &Path) -> FilePhotos {
            let mut name: OsString = database
                .file_stem()
                .map_or_else(|| "database".into(), OsString::from);
            name.push("-photos");
            FilePhotos::in_folder(database.with_file_name(name))
        }

        /// The photos in `folder`, with what a write that stopped half-way
        /// left removed.
        pub fn in_folder(folder: PathBuf) -> FilePhotos {
            if let Ok(entries) = std::fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with('.') && name.ends_with(PARTIAL) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            FilePhotos { folder }
        }

        fn photo(&self, id: &str) -> PathBuf {
            self.folder.join(format!("{id}{PHOTO}"))
        }

        fn thumbnail(&self, id: &str) -> PathBuf {
            self.folder.join(format!("{id}{THUMBNAIL}"))
        }

        /// Writes `bytes` beside `file` and moves them there, so the file
        /// always holds all of them.
        fn write(&self, file: &Path, bytes: &[u8]) -> Result<(), PhotoError> {
            static WRITES: AtomicU64 = AtomicU64::new(0);
            std::fs::create_dir_all(&self.folder).map_err(failed)?;
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let partial = self.folder.join(format!(
                ".{name}-{}-{}{PARTIAL}",
                std::process::id(),
                WRITES.fetch_add(1, Ordering::Relaxed)
            ));
            let written = std::fs::File::create(&partial)
                .and_then(|mut out| {
                    out.write_all(bytes)?;
                    out.sync_all()
                })
                .and_then(|()| std::fs::rename(&partial, file))
                .and_then(|()| sync_folder(&self.folder));
            if let Err(error) = written {
                let _ = std::fs::remove_file(&partial);
                return Err(failed(error));
            }
            Ok(())
        }

        fn keep(&self, bytes: &[u8]) -> Result<String, PhotoError> {
            let id = photo_id(bytes);
            let file = self.photo(&id);
            if !file.is_file() {
                self.write(&file, bytes)?;
            }
            Ok(id)
        }

        fn read(file: &Path) -> Result<Option<Vec<u8>>, PhotoError> {
            match std::fs::read(file) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(failed(error)),
            }
        }

        fn remove(file: &Path) -> Result<(), PhotoError> {
            match std::fs::remove_file(file) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(failed(error)),
                _ => Ok(()),
            }
        }

        /// The ids of the photos and of the thumbnails in the folder.
        fn kept(&self) -> Result<(Vec<String>, Vec<String>), PhotoError> {
            let entries = match std::fs::read_dir(&self.folder) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok((Vec::new(), Vec::new()));
                }
                Err(error) => return Err(failed(error)),
            };
            let (mut photos, mut thumbnails) = (Vec::new(), Vec::new());
            for entry in entries {
                let name = entry.map_err(failed)?.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if let Some(id) = name.strip_suffix(THUMBNAIL).filter(|id| is_photo_id(id)) {
                    thumbnails.push(id.to_owned());
                } else if let Some(id) = name.strip_suffix(PHOTO).filter(|id| is_photo_id(id)) {
                    photos.push(id.to_owned());
                }
            }
            photos.sort();
            Ok((photos, thumbnails))
        }

        fn forget_all_but(&self, keep: &BTreeSet<String>) -> Result<Vec<String>, PhotoError> {
            let (photos, thumbnails) = self.kept()?;
            for id in thumbnails.iter().filter(|id| !keep.contains(*id)) {
                FilePhotos::remove(&self.thumbnail(id))?;
            }
            let forgotten: Vec<String> =
                photos.into_iter().filter(|id| !keep.contains(id)).collect();
            for id in &forgotten {
                FilePhotos::remove(&self.photo(id))?;
            }
            Ok(forgotten)
        }
    }

    #[cfg(unix)]
    fn sync_folder(folder: &Path) -> std::io::Result<()> {
        std::fs::File::open(folder)?.sync_all()
    }

    /// Windows can't open a folder as a file; NTFS keeps its own journal of
    /// renames.
    #[cfg(not(unix))]
    fn sync_folder(_: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn failed(error: std::io::Error) -> PhotoError {
        PhotoError(format!("could not keep the photos: {error}"))
    }

    /// `work` for a photo id, or the refusal of something that isn't one.
    fn named<T>(id: &str, work: impl FnOnce() -> Result<T, PhotoError>) -> Pending<T> {
        Pending::ready(if is_photo_id(id) {
            work()
        } else {
            Err(not_an_id(id))
        })
    }

    impl PhotoStore for FilePhotos {
        fn put(&self, bytes: Vec<u8>) -> Pending<String> {
            Pending::ready(self.keep(&bytes))
        }

        fn get(&self, id: &str) -> Pending<Option<Vec<u8>>> {
            named(id, || FilePhotos::read(&self.photo(id)))
        }

        fn put_thumbnail(&self, id: &str, jpeg: Vec<u8>) -> Pending<()> {
            named(id, || self.write(&self.thumbnail(id), &jpeg))
        }

        fn get_thumbnail(&self, id: &str) -> Pending<Option<Vec<u8>>> {
            named(id, || FilePhotos::read(&self.thumbnail(id)))
        }

        fn delete(&self, id: &str) -> Pending<()> {
            named(id, || {
                FilePhotos::remove(&self.thumbnail(id))?;
                FilePhotos::remove(&self.photo(id))
            })
        }

        fn list(&self) -> Pending<Vec<String>> {
            Pending::ready(self.kept().map(|(photos, _)| photos))
        }

        fn retain(&self, keep: BTreeSet<String>) -> Pending<Vec<String>> {
            Pending::ready(self.forget_all_but(&keep))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn answer<T>(pending: Pending<T>) -> Result<T, PhotoError> {
            pending.take().expect("a computer answers at once")
        }

        fn scratch_folder() -> PathBuf {
            std::env::temp_dir().join(format!(
                "mxsc-photos-test-{}",
                ballistics_store::model::new_id()
            ))
        }

        #[test]
        fn photos_are_kept_once_beside_their_own_database() {
            let folder = scratch_folder();
            let photos = FilePhotos::beside(&folder.join("mxsc.sqlite"));
            assert_eq!(answer(photos.list()).unwrap(), Vec::<String>::new());

            let id = answer(photos.put(b"first photo".to_vec())).unwrap();
            assert_eq!(id, photo_id(b"first photo"));
            let file = folder.join("mxsc-photos").join(format!("{id}.jpg"));
            assert_eq!(std::fs::read(&file).unwrap(), b"first photo");
            assert_eq!(answer(photos.put(b"first photo".to_vec())).unwrap(), id);
            answer(photos.put_thumbnail(&id, b"small".to_vec())).unwrap();
            let other = answer(photos.put(b"second photo".to_vec())).unwrap();
            let mut both = vec![id.clone(), other.clone()];
            both.sort();
            assert_eq!(
                answer(photos.list()).unwrap(),
                both,
                "thumbnails aren't photos"
            );
            assert_eq!(
                std::fs::read_dir(folder.join("mxsc-photos"))
                    .unwrap()
                    .count(),
                3,
                "no partial files are left"
            );
            assert_eq!(
                answer(photos.get_thumbnail(&id)).unwrap(),
                Some(b"small".to_vec())
            );

            // Another database in the same folder has photos of its own.
            let dev = FilePhotos::beside(&folder.join("dev.sqlite"));
            assert_eq!(answer(dev.list()).unwrap(), Vec::<String>::new());

            assert_eq!(
                answer(photos.get(&id)).unwrap(),
                Some(b"first photo".to_vec())
            );
            answer(photos.delete(&id)).unwrap();
            answer(photos.delete(&id)).unwrap();
            assert_eq!(answer(photos.get(&id)).unwrap(), None);
            assert_eq!(answer(photos.get_thumbnail(&id)).unwrap(), None);
            assert_eq!(answer(photos.list()).unwrap(), vec![other]);
            std::fs::remove_dir_all(folder).unwrap();
        }

        #[test]
        fn only_photo_ids_are_files_it_reads_or_lists_and_leftovers_go() {
            let folder = scratch_folder();
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("notes.jpg"), b"not a photo").unwrap();
            let partial = folder.join(format!(".{}.jpg-1-0.part", "a".repeat(64)));
            std::fs::write(&partial, b"half").unwrap();
            let photos = FilePhotos::in_folder(folder.clone());
            assert!(!partial.exists(), "a half-written photo is removed");
            assert_eq!(answer(photos.list()).unwrap(), Vec::<String>::new());
            assert!(answer(photos.get("../mxsc")).is_err());
            assert!(answer(photos.delete("notes")).is_err());
            assert!(answer(photos.put_thumbnail("notes", Vec::new())).is_err());
            assert!(folder.join("notes.jpg").exists());
            std::fs::remove_dir_all(folder).unwrap();
        }

        #[test]
        fn photos_no_record_names_are_forgotten() {
            let folder = scratch_folder();
            let photos = FilePhotos::in_folder(folder.clone());
            let used = answer(photos.put(b"used".to_vec())).unwrap();
            let unused = answer(photos.put(b"unused".to_vec())).unwrap();
            answer(photos.put_thumbnail(&used, b"u".to_vec())).unwrap();
            answer(photos.put_thumbnail(&unused, b"n".to_vec())).unwrap();
            let orphan = photo_id(b"thumbnail only");
            answer(photos.put_thumbnail(&orphan, b"o".to_vec())).unwrap();
            let forgotten = answer(photos.retain(BTreeSet::from([used.clone()]))).unwrap();
            assert_eq!(forgotten, vec![unused.clone()]);
            assert_eq!(answer(photos.list()).unwrap(), vec![used.clone()]);
            assert_eq!(
                answer(photos.get_thumbnail(&used)).unwrap(),
                Some(b"u".to_vec())
            );
            assert_eq!(answer(photos.get_thumbnail(&unused)).unwrap(), None);
            assert_eq!(answer(photos.get_thumbnail(&orphan)).unwrap(), None);
            std::fs::remove_dir_all(folder).unwrap();
        }

        #[test]
        fn photos_are_reconciled_with_the_records_at_start() {
            let folder = scratch_folder();
            std::fs::create_dir_all(&folder).unwrap();
            let database = folder.join("mxsc.sqlite");
            let store = ballistics_store::Store::open(&database).unwrap();
            let photos = FilePhotos::beside(&database);
            let kept = answer(photos.put(b"kept".to_vec())).unwrap();
            let marked = answer(photos.put(b"marked".to_vec())).unwrap();
            let left = answer(photos.put(b"left over".to_vec())).unwrap();
            store
                .save_photo(&mut ballistics_store::Photo::new(kept.clone(), 10, 10))
                .unwrap();
            store
                .save_group(&mut ballistics_store::Group::on_photo(&marked, 0))
                .unwrap();
            super::super::forget_unused(&store, &eframe::egui::Context::default());
            let mut expected = vec![kept, marked];
            expected.sort();
            assert_eq!(answer(photos.list()).unwrap(), expected);
            assert!(!expected.contains(&left));
            drop(store);
            std::fs::remove_dir_all(folder).unwrap();
        }

        #[test]
        fn a_folder_that_cant_be_made_is_an_error_not_a_panic() {
            let folder = scratch_folder();
            std::fs::create_dir_all(folder.parent().unwrap()).unwrap();
            std::fs::write(&folder, b"a file where the folder should be").unwrap();
            let photos = FilePhotos::in_folder(folder.clone());
            assert!(answer(photos.put(b"photo".to_vec())).is_err());
            std::fs::remove_file(folder).unwrap();
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::rc::Rc;

    use ballistics_store::photos::{is_photo_id, not_an_id};
    use ballistics_store::{Pending, PhotoError, PhotoStore, photo_id};
    use eframe::egui;
    use indexed_db_futures::database::Database;
    use indexed_db_futures::prelude::*;
    use indexed_db_futures::transaction::TransactionMode;
    use wasm_bindgen::{JsCast, JsValue};

    /// The IndexedDB database the photos are kept in: not the one SQLite's
    /// storage uses (`relaxed_idb`).
    const DATABASE: &str = "mxsc-photos";
    /// Its object stores: photos and their thumbnails, as `Uint8Array`s
    /// keyed by the photo's id.
    const PHOTOS: &str = "photos";
    const THUMBNAILS: &str = "thumbnails";

    /// Photos in the browser's IndexedDB. Answers come on a later frame,
    /// with a repaint.
    pub struct BrowserPhotos {
        database: Rc<tokio::sync::OnceCell<Database>>,
        context: egui::Context,
    }

    impl BrowserPhotos {
        pub fn new(context: egui::Context) -> BrowserPhotos {
            BrowserPhotos {
                database: Rc::default(),
                context,
            }
        }

        /// Runs `work` on the open database in the background.
        fn run<T, Work, Done>(&self, work: Work) -> Pending<T>
        where
            T: 'static,
            Work: FnOnce(Database) -> Done + 'static,
            Done: Future<Output = Result<T, PhotoError>> + 'static,
        {
            let (pending, answer) = Pending::waiting();
            let database = self.database.clone();
            let context = self.context.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = match database.get_or_try_init(open).await {
                    Ok(database) => work(database.clone()).await,
                    Err(error) => Err(error),
                };
                answer.send(result);
                context.request_repaint();
            });
            pending
        }

        /// The bytes kept under `id` in `store`.
        fn read(&self, store: &'static str, id: &str) -> Pending<Option<Vec<u8>>> {
            if !is_photo_id(id) {
                return Pending::ready(Err(not_an_id(id)));
            }
            let id = id.to_owned();
            self.run(move |database| async move {
                let transaction = database.transaction(store).build().map_err(failed)?;
                let values = transaction.object_store(store).map_err(failed)?;
                let value: Option<JsValue> = values.get(id).await.map_err(failed)?;
                value
                    .map(|value| {
                        value
                            .dyn_into::<js_sys::Uint8Array>()
                            .map(|bytes| bytes.to_vec())
                            .map_err(|_| PhotoError("a kept photo isn't bytes".into()))
                    })
                    .transpose()
            })
        }

        /// Keeps `bytes` under `id` in `store`, answering with `done(id)`.
        fn write<T: 'static>(
            &self,
            store: &'static str,
            id: String,
            bytes: Vec<u8>,
            done: impl FnOnce(String) -> T + 'static,
        ) -> Pending<T> {
            self.run(move |database| async move {
                let transaction = database
                    .transaction(store)
                    .with_mode(TransactionMode::Readwrite)
                    .build()
                    .map_err(failed)?;
                let values = transaction.object_store(store).map_err(failed)?;
                let value = JsValue::from(js_sys::Uint8Array::from(bytes.as_slice()));
                values
                    .put(value)
                    .with_key(id.clone())
                    .build()
                    .map_err(failed)?;
                transaction.commit().await.map_err(failed)?;
                Ok(done(id))
            })
        }
    }

    async fn open() -> Result<Database, PhotoError> {
        Database::open(DATABASE)
            .with_version(1u8)
            .with_on_upgrade_needed(|_, database| {
                database.create_object_store(PHOTOS).build()?;
                database.create_object_store(THUMBNAILS).build()?;
                Ok(())
            })
            .await
            .map_err(|error| PhotoError(format!("this browser can't keep photos ({error})")))
    }

    fn failed(error: impl std::fmt::Display) -> PhotoError {
        PhotoError(format!("could not keep the photos: {error}"))
    }

    impl PhotoStore for BrowserPhotos {
        fn put(&self, bytes: Vec<u8>) -> Pending<String> {
            let id = photo_id(&bytes);
            self.write(PHOTOS, id, bytes, |id| id)
        }

        fn get(&self, id: &str) -> Pending<Option<Vec<u8>>> {
            self.read(PHOTOS, id)
        }

        fn put_thumbnail(&self, id: &str, jpeg: Vec<u8>) -> Pending<()> {
            if !is_photo_id(id) {
                return Pending::ready(Err(not_an_id(id)));
            }
            self.write(THUMBNAILS, id.to_owned(), jpeg, |_| ())
        }

        fn get_thumbnail(&self, id: &str) -> Pending<Option<Vec<u8>>> {
            self.read(THUMBNAILS, id)
        }

        fn delete(&self, id: &str) -> Pending<()> {
            if !is_photo_id(id) {
                return Pending::ready(Err(not_an_id(id)));
            }
            let id = id.to_owned();
            self.run(move |database| async move {
                let transaction = database
                    .transaction([PHOTOS, THUMBNAILS])
                    .with_mode(TransactionMode::Readwrite)
                    .build()
                    .map_err(failed)?;
                for store in [PHOTOS, THUMBNAILS] {
                    let values = transaction.object_store(store).map_err(failed)?;
                    values.delete(id.clone()).build().map_err(failed)?;
                }
                transaction.commit().await.map_err(failed)?;
                Ok(())
            })
        }

        fn list(&self) -> Pending<Vec<String>> {
            self.run(move |database| async move {
                let transaction = database.transaction(PHOTOS).build().map_err(failed)?;
                let photos = transaction.object_store(PHOTOS).map_err(failed)?;
                let mut ids = Vec::new();
                for id in photos.get_all_keys::<String>().await.map_err(failed)? {
                    let id = id.map_err(failed)?;
                    if is_photo_id(&id) {
                        ids.push(id);
                    }
                }
                Ok(ids)
            })
        }

        fn retain(&self, keep: BTreeSet<String>) -> Pending<Vec<String>> {
            self.run(move |database| async move {
                let transaction = database
                    .transaction([PHOTOS, THUMBNAILS])
                    .with_mode(TransactionMode::Readwrite)
                    .build()
                    .map_err(failed)?;
                let mut forgotten = Vec::new();
                for store in [PHOTOS, THUMBNAILS] {
                    let values = transaction.object_store(store).map_err(failed)?;
                    for id in values.get_all_keys::<String>().await.map_err(failed)? {
                        let id = id.map_err(failed)?;
                        if !keep.contains(&id) {
                            values.delete(id.clone()).build().map_err(failed)?;
                            if store == PHOTOS {
                                forgotten.push(id);
                            }
                        }
                    }
                }
                transaction.commit().await.map_err(failed)?;
                Ok(forgotten)
            })
        }
    }
}

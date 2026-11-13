// MIT License
//
// Copyright (c) 2024 Spxg
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! The web build's SQLite storage: the `relaxed-idb` VFS, which keeps the
//! database in the browser's IndexedDB.
//!
//! **Where it comes from.** This is `src/relaxed_idb.rs` from the
//! `sqlite-wasm-vfs` crate, version 0.2.0
//! (<https://github.com/Spxg/sqlite-wasm-rs>, MIT licence, notice above).
//! `sqlite-wasm-vfs` 0.3 dropped this VFS, and rusqlite 0.42 needs
//! `sqlite-wasm-rs` 0.6, whose VFS helpers (`rsqlite-vfs` 0.2, re-exported as
//! `sqlite_wasm_rs::vfs`) the 0.2 file can't build against. So the file was
//! copied here and ported.
//!
//! **What changed** (the port to `rsqlite-vfs` 0.2, modelled on
//! `sqlite-wasm-vfs` 0.3's `sahpool.rs`):
//! - SQLite now gets one handle per open ([`VfsStore::open_file`] /
//!   [`VfsStore::close_file`]) instead of looking files up by name on every
//!   call: the in-memory files are shared `Rc<RefCell<…>>`s, and the handle
//!   holds one. `add_file`/`contains_file`/`with_file*` became `open_file`,
//!   `access`, `full_pathname` and `delete_file`; delete-on-close moved from
//!   the library into `close_file`.
//! - `VfsFile` takes `u64` offsets and sizes, `read` returns the number of
//!   bytes read (instead of "was the read complete") and stops at the end of
//!   the file, and `flush` became `sync`, `lock`, `unlock` and
//!   `check_reserved_lock` (still no-ops in effect, as before: one tab owns
//!   the database, see `web.rs`). The sector size stays 512, as the old
//!   library reported.
//! - `OsCallback` is an object now (`&self`), kept in the VFS's data; errors
//!   use `VfsErrorCode` and are kept for `xGetLastError` through
//!   `record_error`/`last_error`.
//! - Registration returns a `VfsRegistration`; the VFS stays registered for
//!   the page's life, as before. An installed VFS is found again through a
//!   per-thread list rather than by casting whatever SQLite has under that
//!   name.
//! - Removed: `import_db`, `import_db_unchecked` and `export_db`, which the
//!   app doesn't use and whose helpers moved to a different API in 0.2.
//!
//! **Kept exactly as 0.2.0 stores it**, so a database saved by the app
//! before this port opens unchanged after it: the IndexedDB database's name
//! (the VFS name, `relaxed-idb` by default) and version (1), its one object
//! store `blocks` with key path `["path", "offset"]`, and each record
//! `{ path: <file name>, offset: <byte offset>, data: Uint8Array }` holding
//! one page of the file; files are rebuilt from those pages at start-up
//! (preloading all by default), and changed pages are written back (and pages
//! past the end deleted) when SQLite syncs. Don't change any of this, nor the
//! defaults in [`RelaxedIdbCfg::default`], without carrying existing
//! browsers' data across.
//!
//! The upstream notes: inspired by wa-sqlite's
//! [`IDBMirrorVFS`](https://github.com/rhashimoto/wa-sqlite/blob/master/src/examples/IDBMirrorVFS.js),
//! this VFS is used in a synchronous context. The database is preloaded into
//! memory before `xOpen`, and then all operations are synchronous. When
//! SQLite syncs, the changed blocks are written to IndexedDB asynchronously
//! in an IndexedDB transaction. Unlike IDBMirrorVFS it only supports
//! `PRAGMA synchronous=off`. Using it from several pages at once can corrupt
//! the database (the app holds a Web Lock for that).

// The management API is kept whole from upstream; the app uses `install`.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, hash_map};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use indexed_db_futures::database::Database;
use indexed_db_futures::prelude::*;
use indexed_db_futures::transaction::TransactionMode;
use js_sys::{Number, Object, Reflect, Uint8Array};
use sqlite_wasm_rs::vfs::{
    AccessMode, FileKind, LockLevel, MemChunksFile, OpenAccess, OpenOptions, OpenRequest,
    OpenedFile, OsCallback, RegisterVfsError, SQLiteIoMethods, SQLiteVfs, SQLiteVfsFile,
    SectorSize, SyncOptions, VfsAppData, VfsError, VfsErrorCode, VfsFile, VfsResult, VfsStore,
    ffi::{
        SQLITE_ERROR, SQLITE_FCNTL_COMMIT_PHASETWO, SQLITE_FCNTL_PRAGMA, SQLITE_FCNTL_SYNC,
        SQLITE_NOTFOUND, SQLITE_OK, sqlite3_file,
    },
    register_vfs, registered_vfs,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use wasm_bindgen::{JsCast, JsValue};

type Result<T> = std::result::Result<T, RelaxedIdbError>;

/// The sector size the 0.1 helpers reported for every file (0.2's default
/// is 4096).
const SECTOR_SIZE: u32 = 512;

struct IdbCommit {
    op: IdbCommitOp,
    notify: Option<tokio::sync::oneshot::Sender<Result<()>>>,
}

enum IdbCommitOp {
    Sync(String),
    Delete(String),
    Clear,
}

/// A file in memory: a main database, mirrored to IndexedDB page by page,
/// or anything else (journals, temporary files), kept in memory only.
enum IdbFile {
    Main(IdbPageFile),
    Temp(MemChunksFile),
}

impl IdbFile {
    fn new(kind: Option<FileKind>) -> Self {
        if kind == Some(FileKind::MainDb) {
            Self::Main(IdbPageFile::default())
        } else {
            Self::Temp(MemChunksFile::default())
        }
    }

    fn read(&mut self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        match self {
            IdbFile::Main(file) => file.read(buf, offset),
            IdbFile::Temp(file) => file.read(buf, offset),
        }
    }

    fn write(&mut self, buf: &[u8], offset: u64) -> VfsResult<()> {
        match self {
            IdbFile::Main(file) => file.write(buf, offset),
            IdbFile::Temp(file) => file.write(buf, offset),
        }
    }

    fn truncate(&mut self, size: u64) -> VfsResult<()> {
        match self {
            IdbFile::Main(file) => file.truncate(size),
            IdbFile::Temp(file) => file.truncate(size),
        }
    }

    fn size(&self) -> VfsResult<u64> {
        match self {
            IdbFile::Main(file) => Ok(file.file_size as u64),
            IdbFile::Temp(file) => file.size(),
        }
    }
}

#[derive(Default)]
struct IdbPageFile {
    file_size: usize,
    block_size: usize,
    blocks: HashMap<usize, Uint8Array>,
    tx_blocks: HashSet<usize>,
    sync_notified: bool,
}

fn offset_to_usize(offset: u64, code: VfsErrorCode) -> VfsResult<usize> {
    usize::try_from(offset)
        .map_err(|_| VfsError::new(code, "offset exceeds the address space".into()))
}

impl IdbPageFile {
    fn read(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let page_size = self.block_size;
        let Ok(offset) = usize::try_from(offset) else {
            return Ok(0);
        };
        if page_size == 0 || offset >= self.file_size {
            return Ok(0);
        }

        let wanted = buf.len().min(self.file_size - offset);
        let mut read = 0;
        while read < wanted {
            let file_offset = offset + read;
            let page_offset = file_offset % page_size;
            let page_addr = file_offset - page_offset;

            let Some(page) = self.blocks.get(&page_addr) else {
                break;
            };

            if page.length() as usize != page_size {
                return Err(VfsError::new(
                    VfsErrorCode::IoRead,
                    "a page of the wrong size".into(),
                ));
            }
            let length = (page_size - page_offset).min(wanted - read);
            page.subarray(page_offset as u32, (page_offset + length) as u32)
                .copy_to(&mut buf[read..read + length]);
            read += length;
        }

        Ok(read)
    }

    /// SQLite writes a main database a whole page at a time, at page
    /// offsets; each page is one block (and one IndexedDB record).
    fn write(&mut self, buf: &[u8], offset: u64) -> VfsResult<()> {
        let offset = offset_to_usize(offset, VfsErrorCode::IoWrite)?;
        let page_size = buf.len();
        if page_size == 0 {
            return Ok(());
        }

        for fill in (self.file_size..offset).step_by(page_size) {
            self.blocks
                .insert(fill, Uint8Array::new_with_length(page_size as u32));
            self.tx_blocks.insert(fill);
        }

        if let Some(buffer) = self.blocks.get_mut(&offset)
            && buffer.length() as usize == page_size
        {
            buffer.copy_from(buf);
        } else {
            self.blocks.insert(offset, Uint8Array::new_from_slice(buf));
        }

        self.tx_blocks.insert(offset);
        self.block_size = page_size;
        self.file_size = self.file_size.max(offset + page_size);
        Ok(())
    }

    fn truncate(&mut self, size: u64) -> VfsResult<()> {
        self.file_size = offset_to_usize(size, VfsErrorCode::IoTruncate)?;
        Ok(())
    }
}

/// What SQLite holds for an open file: the shared file, and this open's own
/// access mode and lock level.
struct IdbFileHandle {
    file: Rc<RefCell<IdbFile>>,
    read_only: bool,
    lock_level: LockLevel,
}

impl IdbFileHandle {
    fn check_writable(&self) -> VfsResult<()> {
        if self.read_only {
            return Err(VfsError::new(
                VfsErrorCode::ReadOnly,
                "file is read-only".into(),
            ));
        }
        Ok(())
    }
}

impl VfsFile for IdbFileHandle {
    fn sector_size(&self) -> SectorSize {
        SectorSize::new(SECTOR_SIZE).expect("a valid sector size")
    }

    fn read(&mut self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.file.borrow_mut().read(buf, offset)
    }

    fn write(&mut self, buf: &[u8], offset: u64) -> VfsResult<()> {
        self.check_writable()?;
        self.file.borrow_mut().write(buf, offset)
    }

    fn truncate(&mut self, size: u64) -> VfsResult<()> {
        self.check_writable()?;
        self.file.borrow_mut().truncate(size)
    }

    /// Nothing to do here: IndexedDB is written when SQLite sends
    /// `SQLITE_FCNTL_SYNC` or `SQLITE_FCNTL_COMMIT_PHASETWO` (see
    /// `xFileControl`), as before.
    fn sync(&mut self, _options: SyncOptions) -> VfsResult<()> {
        Ok(())
    }

    fn size(&self) -> VfsResult<u64> {
        self.file.borrow().size()
    }

    // One connection per database (the app's Web Lock keeps other tabs
    // out); this only tracks the level, as sahpool does.
    fn lock(&mut self, level: LockLevel) -> VfsResult<()> {
        self.lock_level = self.lock_level.max(level);
        Ok(())
    }

    fn unlock(&mut self, level: LockLevel) -> VfsResult<()> {
        self.lock_level = self.lock_level.min(level);
        Ok(())
    }

    fn check_reserved_lock(&self) -> VfsResult<bool> {
        Ok(self.lock_level >= LockLevel::Reserved)
    }
}

fn key_range(file: &str, start: usize) -> std::ops::RangeInclusive<[JsValue; 2]> {
    [JsValue::from(file), JsValue::from(start)]
        ..=[
            JsValue::from(file),
            JsValue::from(Number::POSITIVE_INFINITY),
        ]
}

async fn clear_impl(indexed_db: &Database) -> Result<()> {
    let transaction = indexed_db
        .transaction("blocks")
        .with_mode(TransactionMode::Readwrite)
        .build()?;
    let blocks = transaction.object_store("blocks")?;
    blocks.clear()?;
    transaction.commit().await?;
    Ok(())
}

async fn preload_db_impl(
    indexed_db: &Database,
    preload: &Preload,
) -> Result<HashMap<String, Rc<RefCell<IdbFile>>>> {
    if matches!(preload, &Preload::None) {
        return Ok(HashMap::new());
    }

    let transaction = indexed_db
        .transaction("blocks")
        .with_mode(TransactionMode::Readonly)
        .build()?;
    let blocks = transaction.object_store("blocks")?;

    let mut name2file = HashMap::new();
    let mut insert_fn = |block: JsValue| -> Result<()> {
        let (path, offset, data) = get_block(block)?;
        match name2file.entry(path) {
            hash_map::Entry::Occupied(mut occupied_entry) => {
                let IdbFile::Main(db) = occupied_entry.get_mut() else {
                    unreachable!();
                };
                if data.length() as usize != db.block_size {
                    return Err(RelaxedIdbError::Generic(format!(
                        "a stored page of {} bytes in a database of {}-byte pages",
                        data.length(),
                        db.block_size
                    )));
                }
                db.file_size += db.block_size;
                db.blocks.insert(offset, data);
            }
            hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(IdbFile::Main(IdbPageFile {
                    file_size: data.length() as _,
                    block_size: data.length() as _,
                    blocks: HashMap::from([(offset, data)]),
                    tx_blocks: HashSet::new(),
                    sync_notified: false,
                }));
            }
        }
        Ok(())
    };

    match preload {
        Preload::All => {
            for block in blocks.get_all::<JsValue>().await? {
                insert_fn(block?)?;
            }
        }
        Preload::Paths(items) => {
            for file in items {
                for block in blocks
                    .get_all::<JsValue>()
                    .with_query(key_range(file, 0))
                    .await?
                {
                    insert_fn(block?)?;
                }
            }
        }
        Preload::None => unreachable!(),
    }

    Ok(name2file
        .into_iter()
        .map(|(name, file)| (name, Rc::new(RefCell::new(file))))
        .collect())
}

struct RelaxedIdb {
    idb: Database,
    name2file: RefCell<HashMap<String, Rc<RefCell<IdbFile>>>>,
    tx: UnboundedSender<IdbCommit>,
    os: Box<dyn OsCallback>,
    last_error: RefCell<Option<VfsError>>,
}

impl RelaxedIdb {
    async fn new(
        options: &RelaxedIdbCfg,
        tx: UnboundedSender<IdbCommit>,
        os: Box<dyn OsCallback>,
    ) -> Result<Self> {
        let indexed_db = Database::open(&options.vfs_name)
            .with_version(1u8)
            .with_on_upgrade_needed(|_, db| {
                db.create_object_store("blocks")
                    .with_key_path(["path", "offset"].into())
                    .build()?;
                Ok(())
            })
            .await?;

        if options.clear_on_init {
            clear_impl(&indexed_db).await?;
        }

        let name2file = preload_db_impl(&indexed_db, &options.preload).await?;
        Ok(RelaxedIdb {
            idb: indexed_db,
            name2file: RefCell::new(name2file),
            tx,
            os,
            last_error: RefCell::new(None),
        })
    }

    fn send_task(&self, op: IdbCommitOp) -> Result<()> {
        if self.tx.send(IdbCommit { op, notify: None }).is_err() {
            return Err(RelaxedIdbError::Generic(
                "failed to send commit task".into(),
            ));
        }
        Ok(())
    }

    fn send_task_with_notify(&self, op: IdbCommitOp) -> Result<WaitCommit> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let commit = IdbCommit {
            op,
            notify: Some(tx),
        };
        if self.tx.send(commit).is_err() {
            return Err(RelaxedIdbError::Generic(
                "failed to send commit task".into(),
            ));
        }
        Ok(WaitCommit(rx))
    }

    async fn preload_db(&self, files: Vec<String>) -> Result<()> {
        let preload = {
            let name2file = self.name2file.borrow();
            files
                .into_iter()
                .filter(|x| !name2file.contains_key(x))
                .collect::<Vec<_>>()
        };
        let preload = preload_db_impl(&self.idb, &Preload::Paths(preload)).await?;
        self.name2file.borrow_mut().extend(preload);
        Ok(())
    }

    fn delete_db(&self, name: &str) -> Result<WaitCommit> {
        self.name2file.borrow_mut().remove(name);
        self.send_task_with_notify(IdbCommitOp::Delete(name.into()))
    }

    fn clear_all(&self) -> Result<WaitCommit> {
        std::mem::take(&mut *self.name2file.borrow_mut());
        self.send_task_with_notify(IdbCommitOp::Clear)
    }

    fn exists(&self, file: &str) -> bool {
        self.name2file.borrow().contains_key(file)
    }

    async fn delete_db_impl(&self, file: &str) -> Result<()> {
        let transaction = self
            .idb
            .transaction("blocks")
            .with_mode(TransactionMode::Readwrite)
            .build()?;

        let store = transaction.object_store("blocks")?;

        store.delete(key_range(file, 0)).build()?;
        transaction.commit().await?;

        Ok(())
    }

    async fn sync_db_impl(&self, file: &str) -> Result<()> {
        let Some(idb_file) = self.name2file.borrow().get(file).cloned() else {
            return Ok(());
        };

        // Queue the changes with the file borrowed, then let it go before
        // waiting for IndexedDB.
        let transaction = {
            let mut idb_file = idb_file.borrow_mut();
            let IdbFile::Main(idb_blocks) = &mut *idb_file else {
                return Ok(());
            };

            idb_blocks.sync_notified = false;

            let file_size = idb_blocks.file_size;
            let mut truncated_offset = idb_blocks.file_size;
            while idb_blocks.blocks.remove(&truncated_offset).is_some() {
                truncated_offset += idb_blocks.block_size;
            }

            let tx_blocks = std::mem::take(&mut idb_blocks.tx_blocks);
            if tx_blocks.is_empty() && file_size == truncated_offset {
                // no need to put or delete
                return Ok(());
            }

            let queued = (|| -> Result<_> {
                let path = JsValue::from(file);
                let transaction = self
                    .idb
                    .transaction("blocks")
                    .with_mode(TransactionMode::Readwrite)
                    .build()?;
                let store = transaction.object_store("blocks")?;
                for offset in &tx_blocks {
                    if let Some(buffer) = idb_blocks.blocks.get(offset) {
                        store.put(&set_block(&path, *offset, buffer)).build()?;
                    }
                }
                store.delete(key_range(file, file_size)).build()?;
                Ok(transaction)
            })();
            match queued {
                Ok(transaction) => (transaction, tx_blocks),
                Err(error) => {
                    idb_blocks.tx_blocks.extend(tx_blocks);
                    return Err(error);
                }
            }
        };
        let (transaction, sent) = transaction;

        // A failed write (the quota full, the browser aborting it) keeps its
        // pages for the next sync: sending only later changes would leave a
        // mix of old and new pages in IndexedDB.
        if let Err(error) = transaction.commit().await {
            if let IdbFile::Main(idb_blocks) = &mut *idb_file.borrow_mut() {
                idb_blocks.tx_blocks.extend(sent);
            }
            web_sys::console::error_1(
                &format!("could not save the database to IndexedDB: {error}").into(),
            );
            return Err(error.into());
        }

        Ok(())
    }

    async fn commit_loop(&self, mut rx: UnboundedReceiver<IdbCommit>) {
        while let Some(commit) = rx.recv().await {
            let IdbCommit { op, notify } = commit;
            let ret = match op {
                IdbCommitOp::Sync(file) => self.sync_db_impl(&file).await,
                IdbCommitOp::Delete(file) => self.delete_db_impl(&file).await,
                IdbCommitOp::Clear => clear_impl(&self.idb).await,
            };
            if let Some(notify) = notify {
                // An unsuccessful send would be one where the corresponding receiver
                // has already been deallocated.
                let _ = notify.send(ret);
            }
        }
    }
}

/// A stored page, or an error for a record that isn't one (damaged data
/// gives an error to show, rather than a panic that stops the app).
fn get_block(value: JsValue) -> Result<(String, usize, Uint8Array)> {
    let field = |name: &str| Reflect::get(&value, &JsValue::from(name)).ok();
    let malformed = || RelaxedIdbError::Generic("a stored page is malformed".into());
    let path = field("path")
        .and_then(|v| v.as_string())
        .ok_or_else(malformed)?;
    let offset = field("offset")
        .and_then(|v| v.as_f64())
        .ok_or_else(malformed)?;
    let data = field("data")
        .filter(|data| data.is_instance_of::<Uint8Array>())
        .ok_or_else(malformed)?;
    if offset < 0.0 || offset.fract() != 0.0 {
        return Err(malformed());
    }
    Ok((path, offset as usize, Uint8Array::from(data)))
}

fn set_block(path: &JsValue, offset: usize, data: &Uint8Array) -> JsValue {
    let block = Object::new();
    Reflect::set(&block, &JsValue::from("path"), path).unwrap();
    Reflect::set(&block, &JsValue::from("offset"), &JsValue::from(offset)).unwrap();
    Reflect::set(&block, &JsValue::from("data"), &JsValue::from(data)).unwrap();
    block.into()
}

type RelaxedIdbAppData = Rc<RelaxedIdb>;

struct RelaxedIdbStore;

impl VfsStore for RelaxedIdbStore {
    type File = IdbFileHandle;
    type AppData = RelaxedIdbAppData;

    fn record_error(pool: &Self::AppData, error: VfsError) {
        pool.last_error.replace(Some(error));
    }

    fn last_error(pool: &Self::AppData) -> Option<VfsError> {
        pool.last_error.borrow().clone()
    }

    fn open_file(
        pool: &Self::AppData,
        request: OpenRequest<'_>,
    ) -> VfsResult<OpenedFile<Self::File>> {
        let options = request.options;
        let file = match request.filename {
            // Anonymous temporary files live in memory, outside the namespace.
            None => Rc::new(RefCell::new(IdbFile::Temp(MemChunksFile::default()))),
            Some(filename) => {
                let name = filename.path();
                let mut name2file = pool.name2file.borrow_mut();
                match name2file.get(name) {
                    Some(_) if options.exclusive() => {
                        return Err(VfsError::new(
                            VfsErrorCode::CantOpen,
                            format!("file already exists: {name}").into(),
                        ));
                    }
                    Some(file) => file.clone(),
                    None if options.create() => {
                        let file = Rc::new(RefCell::new(IdbFile::new(options.kind())));
                        name2file.insert(name.into(), file.clone());
                        file
                    }
                    None => {
                        return Err(VfsError::new(
                            VfsErrorCode::CantOpen,
                            format!("file not found: {name}").into(),
                        ));
                    }
                }
            }
        };

        Ok(OpenedFile {
            file: IdbFileHandle {
                file,
                read_only: options.access() == OpenAccess::ReadOnly,
                lock_level: LockLevel::None,
            },
            access: options.access(),
        })
    }

    fn close_file(
        pool: &Self::AppData,
        name: Option<&str>,
        file: Self::File,
        options: OpenOptions,
    ) -> VfsResult<()> {
        let Some(name) = name else {
            return Ok(());
        };
        if !options.delete_on_close() {
            return Ok(());
        }
        // Don't delete a file created since under the same name.
        let current = pool.name2file.borrow().get(name).cloned();
        match current {
            Some(current) if Rc::ptr_eq(&current, &file.file) => {
                Self::delete_file(pool, name, false)
            }
            _ => Ok(()),
        }
    }

    fn access(pool: &Self::AppData, name: &str, _mode: AccessMode) -> VfsResult<bool> {
        Ok(pool.name2file.borrow().contains_key(name))
    }

    fn full_pathname(_pool: &Self::AppData, name: &str) -> VfsResult<String> {
        Ok(name.into())
    }

    fn delete_file(pool: &Self::AppData, name: &str, _sync_dir: bool) -> VfsResult<()> {
        let idb_file = match pool.name2file.borrow_mut().remove(name) {
            Some(file) => file,
            None => {
                return Err(VfsError::new(
                    VfsErrorCode::IoDelete,
                    format!("{name} not found").into(),
                ));
            }
        };
        // temp db never put into indexed db, no need to delete
        if matches!(&*idb_file.borrow(), IdbFile::Main(_))
            && pool.send_task(IdbCommitOp::Delete(name.into())).is_err()
        {
            return Err(VfsError::new(
                VfsErrorCode::IoDelete,
                format!("failed to send delete task, file: {name}").into(),
            ));
        }
        Ok(())
    }
}

/// Records `message` for `xGetLastError` and returns SQLite's error code.
fn fail(pool: &RelaxedIdbAppData, code: VfsErrorCode, message: String) -> c_int {
    let error = VfsError::new(code, message.into());
    let raw = error.raw_code();
    RelaxedIdbStore::record_error(pool, error);
    raw
}

struct RelaxedIdbIoMethods;

impl SQLiteIoMethods for RelaxedIdbIoMethods {
    type Store = RelaxedIdbStore;

    const VERSION: c_int = 1;

    unsafe extern "C" fn xFileControl(
        file: *mut sqlite3_file,
        op: c_int,
        arg: *mut c_void,
    ) -> c_int {
        // SAFETY: SQLite passes a file opened by this VFS's `xOpen`, whose
        // handle is an `IdbFileHandle` and whose VFS data is the pool.
        let vfs_file = unsafe { &*SQLiteVfsFile::from_file(file) };
        let pool = unsafe { VfsAppData::<RelaxedIdbAppData>::get(vfs_file.vfs()) };
        let Some(name) = (unsafe { vfs_file.name() }) else {
            return SQLITE_NOTFOUND;
        };
        let handle = unsafe { vfs_file.handle::<IdbFileHandle>() };

        let mut file = handle.file.borrow_mut();
        let IdbFile::Main(file) = &mut *file else {
            return SQLITE_NOTFOUND;
        };

        match op {
            SQLITE_FCNTL_PRAGMA => {
                // SAFETY: for SQLITE_FCNTL_PRAGMA, SQLite passes an array of
                // three strings: error message slot, name and value.
                let (name, value) = unsafe {
                    let arg = arg as *mut *mut c_char;
                    (*arg.add(1), *arg.add(2))
                };

                if name.is_null() {
                    return SQLITE_ERROR;
                }
                if value.is_null() {
                    return SQLITE_NOTFOUND;
                }

                let Ok(key) = unsafe { CStr::from_ptr(name) }.to_str() else {
                    return SQLITE_ERROR;
                };
                let Ok(value) = unsafe { CStr::from_ptr(value) }.to_str() else {
                    return SQLITE_ERROR;
                };

                if key.eq_ignore_ascii_case("page_size") {
                    let Ok(page_size) = value.parse::<usize>() else {
                        return SQLITE_ERROR;
                    };
                    if page_size == file.block_size {
                        return SQLITE_OK;
                    } else if file.block_size == 0 {
                        file.block_size = page_size;
                    } else {
                        return fail(
                            pool,
                            VfsErrorCode::Error,
                            "page_size cannot be changed".into(),
                        );
                    }
                } else if key.eq_ignore_ascii_case("synchronous")
                    && !value.eq_ignore_ascii_case("off")
                {
                    return fail(
                        pool,
                        VfsErrorCode::Error,
                        "relaxed-idb vfs only supports synchronous=off".into(),
                    );
                };
            }
            SQLITE_FCNTL_SYNC | SQLITE_FCNTL_COMMIT_PHASETWO if !file.sync_notified => {
                if pool.send_task(IdbCommitOp::Sync(name.into())).is_err() {
                    return fail(
                        pool,
                        VfsErrorCode::Error,
                        format!("failed to send sync task, file: {name}"),
                    );
                }
                file.sync_notified = true;
            }
            _ => (),
        }

        SQLITE_NOTFOUND
    }
}

struct RelaxedIdbVfs;

impl SQLiteVfs<RelaxedIdbIoMethods> for RelaxedIdbVfs {
    type Os = dyn OsCallback;

    fn os(pool: &RelaxedIdbAppData) -> &Self::Os {
        &*pool.os
    }

    const VERSION: c_int = 1;
}

/// A future that resolves when a pending IndexedDB commit operation is complete.
pub struct WaitCommit(tokio::sync::oneshot::Receiver<Result<()>>);

impl Future for WaitCommit {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(ret) => Poll::Ready(ret.unwrap_or_else(|_| {
                Err(RelaxedIdbError::Generic(
                    "Waiting for notify failure".into(),
                ))
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RelaxedIdbError {
    #[error(transparent)]
    Vfs(#[from] RegisterVfsError),
    #[error(transparent)]
    OpenDb(#[from] indexed_db_futures::error::OpenDbError),
    #[error(transparent)]
    IndexedDb(#[from] indexed_db_futures::error::Error),
    #[error("Generic error: {0}")]
    Generic(String),
}

/// Select which dbs to preload into memory.
pub enum Preload {
    /// Preload all databases
    All,
    /// Specify the path to load the database
    Paths(Vec<String>),
    /// Not preloaded, can be manually loaded later via `RelaxedIdbUtil`
    None,
}

/// Build `RelaxedIdbCfg`
pub struct RelaxedIdbCfgBuilder(RelaxedIdbCfg);

impl RelaxedIdbCfgBuilder {
    pub fn new() -> Self {
        Self(RelaxedIdbCfg::default())
    }

    /// The SQLite VFS name under which this pool's VFS is registered.
    pub fn vfs_name(mut self, name: &str) -> Self {
        self.0.vfs_name = name.into();
        self
    }

    /// Delete all files on initialization.
    pub fn clear_on_init(mut self, set: bool) -> Self {
        self.0.clear_on_init = set;
        self
    }

    /// Select which dbs to preload into memory.
    pub fn preload(mut self, preload: Preload) -> Self {
        self.0.preload = preload;
        self
    }

    /// Build `RelaxedIdbCfg`.
    pub fn build(self) -> RelaxedIdbCfg {
        self.0
    }
}

impl Default for RelaxedIdbCfgBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// `RelaxedIdb` options
pub struct RelaxedIdbCfg {
    /// The SQLite VFS name under which this pool's VFS is registered, and
    /// the name of the IndexedDB database it keeps the files in.
    pub vfs_name: String,
    /// Delete all files on initialization.
    pub clear_on_init: bool,
    /// Select which dbs to preload into memory.
    pub preload: Preload,
}

/// The defaults the app's data was saved with: don't change them.
impl Default for RelaxedIdbCfg {
    fn default() -> Self {
        Self {
            vfs_name: "relaxed-idb".into(),
            clear_on_init: false,
            preload: Preload::All,
        }
    }
}

/// RelaxedIdbVfs management tool.
pub struct RelaxedIdbUtil {
    pool: RelaxedIdbAppData,
}

impl RelaxedIdbUtil {
    /// Preload the db.
    ///
    /// Because indexed db reading data is an asynchronous operation,
    /// the db must be preloaded into memory before opening the sqlite db.
    pub async fn preload_db(&self, preload: Vec<String>) -> Result<()> {
        self.pool.preload_db(preload).await
    }

    /// Delete the specified database, make sure that the database is closed.
    pub fn delete_db(&self, filename: &str) -> Result<WaitCommit> {
        self.pool.delete_db(filename)
    }

    /// Delete all database, make sure that all database is closed.
    pub fn clear_all(&self) -> Result<WaitCommit> {
        self.pool.clear_all()
    }

    /// Does the database exists.
    pub fn exists(&self, filename: &str) -> bool {
        self.pool.exists(filename)
    }

    /// List all files.
    pub fn list(&self) -> Vec<String> {
        self.pool.name2file.borrow().keys().cloned().collect()
    }

    /// Number of files.
    pub fn count(&self) -> usize {
        self.pool.name2file.borrow().len()
    }
}

thread_local! {
    /// The VFSs this module installed, by name.
    static INSTALLED: RefCell<HashMap<String, RelaxedIdbAppData>> = RefCell::new(HashMap::new());
}

/// Register `relaxed-idb` vfs and return a management tool which can be used
/// to perform basic administration of the file pool.
///
/// If the vfs corresponding to `options.vfs_name` has been registered,
/// only return a management tool without register.
pub async fn install<C: OsCallback + Default + 'static>(
    options: &RelaxedIdbCfg,
    default_vfs: bool,
) -> Result<RelaxedIdbUtil> {
    static REGISTER_GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = REGISTER_GUARD.lock().await;

    if let Some(pool) =
        INSTALLED.with(|installed| installed.borrow().get(&options.vfs_name).cloned())
    {
        return Ok(RelaxedIdbUtil { pool });
    }
    // SAFETY: single-threaded SQLite (wasm), and the guard above serializes
    // this module's registrations.
    if unsafe { registered_vfs(&options.vfs_name)? }.is_some() {
        return Err(RegisterVfsError::NameConflict(options.vfs_name.clone()).into());
    }

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let pool = Rc::new(RelaxedIdb::new(options, tx, Box::new(C::default())).await?);
    // SAFETY: the VFS, its I/O methods and store agree on the file handle
    // and app-data types; no other code registers under this name between
    // the check inside `register_vfs` and the registration (no await).
    let registration = unsafe {
        register_vfs::<RelaxedIdbIoMethods, RelaxedIdbVfs>(
            &options.vfs_name,
            pool.clone(),
            default_vfs,
        )?
    };
    // Registered for the page's life.
    let _ = registration.into_raw();

    INSTALLED.with(|installed| {
        installed
            .borrow_mut()
            .insert(options.vfs_name.clone(), pool.clone())
    });
    let commit_pool = pool.clone();
    wasm_bindgen_futures::spawn_local(async move { commit_pool.commit_loop(rx).await });

    Ok(RelaxedIdbUtil { pool })
}

#[cfg(test)]
mod tests {
    //! Run in a browser with `wasm-bindgen-test` (not run by the Tests
    //! workflow). The first is upstream's store test (rsqlite-vfs 0.1's
    //! `test_vfs_store`, whole pages as SQLite writes them) on the 0.2 API;
    //! the second checks the IndexedDB records keep their layout.

    use super::{
        IdbCommitOp, RelaxedIdb, RelaxedIdbCfgBuilder, RelaxedIdbStore, get_block, key_range,
    };
    use indexed_db_futures::prelude::*;
    use indexed_db_futures::transaction::TransactionMode;
    use sqlite_wasm_rs::WasmOsCallback;
    use sqlite_wasm_rs::vfs::{
        AccessMode, FileKind, OpenAccess, OpenOptions, OpenRequest, VfsFile, VfsStore,
    };
    use std::rc::Rc;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn relaxed_idb_vfs_store() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = Rc::new(
            RelaxedIdb::new(
                &RelaxedIdbCfgBuilder::new()
                    .vfs_name("test_relaxed_idb_suite")
                    .clear_on_init(true)
                    .build(),
                tx,
                Box::new(WasmOsCallback),
            )
            .await
            .unwrap(),
        );
        wasm_bindgen_futures::spawn_local(async move { while rx.recv().await.is_some() {} });

        for (name, kind) in [
            ("___test_vfs_store#1___", FileKind::MainDb),
            ("___test_vfs_store#2___", FileKind::TempDb),
        ] {
            let exists = |pool| RelaxedIdbStore::access(pool, name, AccessMode::Exists).unwrap();
            assert!(!exists(&pool));
            let options = OpenOptions::new(OpenAccess::ReadWrite, kind).with_create();
            let mut file = RelaxedIdbStore::open_file(&pool, OpenRequest::named(name, options))
                .unwrap()
                .file;
            assert!(exists(&pool));

            let base_offset = 1024 * 1024;
            let mut write_buffer = vec![42; 64 * 1024];
            let mut read_buffer = vec![42; base_offset + write_buffer.len()];
            let hw = b"hello world!";
            write_buffer[..hw.len()].copy_from_slice(hw);

            file.write(&write_buffer, 0).unwrap();
            assert_eq!(file.size().unwrap(), write_buffer.len() as u64);
            read_buffer.fill(99);
            let read = file.read(&mut read_buffer, 0).unwrap();
            assert_eq!(read, write_buffer.len());
            assert_eq!(&read_buffer[..read], &write_buffer[..]);

            file.write(&write_buffer, base_offset as u64).unwrap();
            assert_eq!(
                file.size().unwrap(),
                (base_offset + write_buffer.len()) as u64
            );
            let read = file.read(&mut read_buffer, 0).unwrap();
            assert_eq!(read, read_buffer.len());
            assert_eq!(&read_buffer[..write_buffer.len()], &write_buffer[..]);
            assert!(
                read_buffer[write_buffer.len()..base_offset]
                    .iter()
                    .all(|&x| x == 0)
            );
            assert_eq!(&read_buffer[base_offset..], &write_buffer[..]);
            // At the end of the file, nothing more.
            let mut tail = [7; 16];
            assert_eq!(
                file.read(&mut tail, (base_offset + write_buffer.len()) as u64)
                    .unwrap(),
                0
            );

            RelaxedIdbStore::close_file(&pool, Some(name), file, options).unwrap();
            RelaxedIdbStore::delete_file(&pool, name, false).unwrap();
            assert!(!exists(&pool));
        }
    }

    /// A main database's pages land in IndexedDB as relaxed_idb 0.2 put
    /// them, and a fresh start reads them back.
    #[wasm_bindgen_test]
    async fn relaxed_idb_layout() {
        let config = || {
            RelaxedIdbCfgBuilder::new()
                .vfs_name("test_relaxed_idb_layout")
                .build()
        };
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = Rc::new(
            RelaxedIdb::new(
                &RelaxedIdbCfgBuilder::new()
                    .vfs_name("test_relaxed_idb_layout")
                    .clear_on_init(true)
                    .build(),
                tx,
                Box::new(WasmOsCallback),
            )
            .await
            .unwrap(),
        );
        let commit_pool = pool.clone();
        wasm_bindgen_futures::spawn_local(async move { commit_pool.commit_loop(rx).await });

        let name = "layout.db";
        let page = 4096;
        let options = OpenOptions::new(OpenAccess::ReadWrite, FileKind::MainDb).with_create();
        let mut file = RelaxedIdbStore::open_file(&pool, OpenRequest::named(name, options))
            .unwrap()
            .file;
        let pages: Vec<Vec<u8>> = (0..3u8).map(|n| vec![n + 1; page]).collect();
        for (index, bytes) in pages.iter().enumerate() {
            file.write(bytes, (index * page) as u64).unwrap();
        }
        RelaxedIdbStore::close_file(&pool, Some(name), file, options).unwrap();
        pool.send_task_with_notify(IdbCommitOp::Sync(name.into()))
            .unwrap()
            .await
            .unwrap();

        let transaction = pool
            .idb
            .transaction("blocks")
            .with_mode(TransactionMode::Readonly)
            .build()
            .unwrap();
        let store = transaction.object_store("blocks").unwrap();
        let records: Vec<JsValue> = store
            .get_all::<JsValue>()
            .with_query(key_range(name, 0))
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(records.len(), pages.len());
        for (index, record) in records.into_iter().enumerate() {
            let (path, offset, data) = get_block(record).unwrap();
            assert_eq!(path, name);
            assert_eq!(offset, index * page);
            assert_eq!(data.to_vec(), pages[index]);
        }
        drop(transaction);

        // As at the next start: preloaded from IndexedDB.
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let reloaded = Rc::new(
            RelaxedIdb::new(&config(), tx, Box::new(WasmOsCallback))
                .await
                .unwrap(),
        );
        let options = OpenOptions::new(OpenAccess::ReadWrite, FileKind::MainDb);
        let mut file = RelaxedIdbStore::open_file(&reloaded, OpenRequest::named(name, options))
            .unwrap()
            .file;
        assert_eq!(file.size().unwrap(), (pages.len() * page) as u64);
        let mut bytes = vec![0; pages.len() * page];
        assert_eq!(file.read(&mut bytes, 0).unwrap(), bytes.len());
        assert_eq!(bytes, pages.concat());
    }
}

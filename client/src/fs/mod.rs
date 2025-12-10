//! This module is the root of the FUSE filesystem implementation.
//!
//! It defines the main `RemoteFS` state struct, which holds all filesystem
//! caches (attributes, paths, open files) and the asynchronous Tokio runtime.
//!
//! The `impl Filesystem` block acts as the primary dispatcher, receiving
//! calls from the FUSE kernel and forwarding them to the appropriate
//! sub-modules (`attr`, `read`, `write`, etc.) for processing.
use std::sync::{Arc, Mutex};
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, ReplyWrite, Request, ReplyEmpty
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};
use crate::config::Config;
use crate::fs::cache::AttributeCache;

// --- Module Declarations ---
// These files contain the logic for handling FUSE operations.
pub mod cache;
pub mod prelude;
mod attr;
mod read;
mod write;
mod create;
mod delete;
mod rename;

/// Default Time-To-Live (TTL) for FUSE kernel attribute/entry caches.
pub const TTL: Duration = Duration::from_secs(5);
/// Static, hardcoded attributes for the root directory (inode 1).
pub const ROOT_DIR_ATTR: FileAttr = FileAttr {
    ino: 1, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH, kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 501, gid: 20,
    rdev: 0, flags: 0, blksize: 5120,
};

/// Holds the in-memory cache for a file opened with write access.
///
/// This is the core of the "cache-on-write" strategy. `write` calls
/// store their data blocks in the `buffer` HashMap, indexed by offset.
/// The `release` function later assembles these blocks for upload.
pub struct OpenWriteFile {
    /// The server-relative path of the file (e.g., "dir/file.txt").
    pub(crate) path: String,
    /// In-memory cache of written data blocks, keyed by their file offset.
    pub(crate) buffer: HashMap<i64, Vec<u8>>,
}

/// The main state struct for the remote filesystem.
///
/// An instance of this struct is created when the filesystem is mounted.
/// It holds all persistent state required to serve FUSE requests, including
/// the asynchronous runtime, API client, and various caches.
pub struct RemoteFS {
    /// The `reqwest` client for making HTTP requests to the remote server.
    pub(crate) client: reqwest::Client,
    /// The Tokio `Runtime` used to execute asynchronous API calls (`block_on`).
    pub(crate) runtime: tokio::runtime::Runtime,
    /// Maps an Inode number (u64) to its full path string (e.g., 1 -> "").
    pub(crate) inode_to_path: HashMap<u64, String>,
    /// Maps a full path string to its Inode number (e.g., "" -> 1).
    pub(crate) path_to_inode: HashMap<String, u64>,
    /// Caches the `FileType` (File or Dir) for a known Inode.
    pub(crate) inode_to_type: HashMap<u64, FileType>,
    /// A simple counter to generate new, unique Inode numbers.
    pub(crate) next_inode: u64,
    /// The attribute cache (LRU or TTL) for `getattr` calls.
    pub(crate) attribute_cache: AttributeCache,
    /// The loaded filesystem configuration.
    pub(crate) config: Config,
    /// The in-memory cache for files opened with write access.
    /// Keyed by File Handle (`fh`).
    pub(crate) open_files: HashMap<u64, OpenWriteFile>,
    /// A simple counter to generate new, unique File Handle (fh) numbers.
    pub(crate) next_fh: u64,
}

impl RemoteFS {
    /// Creates a new instance of the `RemoteFS`.
    ///
    /// This initializes the Tokio runtime, the `reqwest` client, all caches,
    /// and populates the maps with the root directory (inode 1).
    pub fn new(config: Config) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let mut fs = Self {
            client: reqwest::Client::new(),
            runtime,
            inode_to_path: HashMap::new(),
            path_to_inode: HashMap::new(),
            inode_to_type: HashMap::new(),
            next_inode: 2, // 1 is root
            attribute_cache: AttributeCache::new(&config),
            config,
            open_files: HashMap::new(),
            next_fh: 1,
        };

        // Initialize root directory
        fs.inode_to_path.insert(1, "".to_string());
        fs.path_to_inode.insert("".to_string(), 1);
        fs.inode_to_type.insert(1, FileType::Directory);
        let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
        fs.attribute_cache.put(1, ROOT_DIR_ATTR, ttl);
        fs
    }
}

#[derive(Clone)]
pub struct FsWrapper(pub Arc<Mutex<RemoteFS>>);
/// Main FUSE trait implementation.
///
/// This block acts as a simple "dispatcher" or "router". All FUSE kernel
/// calls land here, and are immediately forwarded to the appropriate
/// function in one of the sub-modules (e.g., `attr::getattr`).
impl Filesystem for FsWrapper {
    // --- Attribute Operations (attr.rs) ---

    /// Delegates `getattr` to `attr::getattr`.
    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        let mut fs = self.0.lock().unwrap();
        attr::getattr(&mut fs, req, ino, reply);
    }

    /// Delegates `setattr` to `attr::setattr`.
    fn setattr(&mut self, req: &Request<'_>, ino: u64, mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, atime: Option<fuser::TimeOrNow>, mtime: Option<fuser::TimeOrNow>, ctime: Option<std::time::SystemTime>, fh: Option<u64>, crtime: Option<std::time::SystemTime>, chgtime: Option<std::time::SystemTime>, bkuptime: Option<std::time::SystemTime>, flags: Option<u32>, reply: ReplyAttr) {
        let mut fs = self.0.lock().unwrap();
        attr::setattr(&mut fs, req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
    }

    // --- Read Operations (read.rs) ---

    /// Delegates `lookup` to `read::lookup`.
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let mut fs = self.0.lock().unwrap();
        read::lookup(&mut fs, req, parent, name, reply);
    }

    /// Delegates `readdir` to `read::readdir`.
    fn readdir(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, reply: ReplyDirectory) {
        let mut fs = self.0.lock().unwrap();
        read::readdir(&mut fs, req, ino, fh, offset, reply);
    }

    /// Delegates `read` to `read::read`.
    fn read(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        let mut fs = self.0.lock().unwrap();
        read::read(&mut fs, req, ino, fh, offset, size, flags, lock_owner, reply);
    }

    /// Delegates `open` to `read::open`.
    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let mut fs = self.0.lock().unwrap();
        read::open(&mut fs, req, ino, flags, reply);
    }

    // --- Write Operations (write.rs) ---

    /// Delegates `write` to `write::write`.
    fn write(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyWrite) {
        let mut fs = self.0.lock().unwrap();
        write::write(&mut fs, req, ino, fh, offset, data, write_flags, flags, lock_owner, reply);
    }

    /// Delegates `release` to `write::release`.
    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        let mut fs = self.0.lock().unwrap();
        write::release(&mut fs, _req, _ino, _fh, _flags, _lock_owner, _flush, reply);
    }

    /// Delegates `flush` to `write::flush`.
    fn flush(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        let mut fs = self.0.lock().unwrap();
        write::flush(&mut fs, _req, _ino, _fh, _lock_owner, reply);
    }

    // --- Create Operations (create.rs) ---

    /// Delegates `create` to `create::create`.
    fn create(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
        let mut fs = self.0.lock().unwrap();
        create::create(&mut fs, req, parent, name, mode, umask, flags, reply);
    }

    /// Delegates `mkdir` to `create::mkdir`.
    fn mkdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
        let mut fs = self.0.lock().unwrap();
        create::mkdir(&mut fs, req, parent, name, mode, umask, reply);
    }

    // --- Delete Operations (delete.rs) ---

    /// Delegates `unlink` to `delete::unlink`.
    fn unlink(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let mut fs = self.0.lock().unwrap();
        delete::unlink(&mut fs, req, parent, name, reply);
    }

    /// Delegates `rmdir` to `delete::rmdir`.
    fn rmdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let mut fs = self.0.lock().unwrap();
        delete::rmdir(&mut fs, req, parent, name, reply);
    }

    // --- Rename Operations (rename.rs) ---

    /// Delegates `rename` to `rename::rename`.
    fn rename(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, flags: u32, reply: ReplyEmpty) {
        let mut fs = self.0.lock().unwrap();
        rename::rename(&mut fs, req, parent, name, newparent, newname, flags, reply);
    }
}
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, ReplyWrite, Request, ReplyEmpty
};

// Dichiara i moduli del filesystem
pub mod cache;
mod read_ops;
mod write_ops;
mod attr_ops;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::config::Config;
use crate::fs::cache::AttributeCache;

pub(crate) const TTL: Duration = Duration::from_secs(5); // Questo è il TTL per il kernel, non la nostra cache
pub(crate) const ROOT_DIR_ATTR: FileAttr = FileAttr {
    ino: 1, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH, kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 501, gid: 20,
    rdev: 0, flags: 0, blksize: 5120,
};

pub struct RemoteFS {
    pub(crate) client: reqwest::Client,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) inode_to_path: HashMap<u64, String>,
    pub(crate) path_to_inode: HashMap<String, u64>,
    pub(crate) inode_to_type: HashMap<u64, FileType>,
    pub(crate) next_inode: u64,
    // Campi per la nuova cache e configurazione
    pub(crate) attribute_cache: AttributeCache,
    pub(crate) config: Config,
}

impl RemoteFS {
    pub fn new(config: Config) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let mut fs = Self {
            client: reqwest::Client::new(),
            runtime,
            inode_to_path: HashMap::new(),
            path_to_inode: HashMap::new(),
            inode_to_type: HashMap::new(),
            next_inode: 2,
            attribute_cache: AttributeCache::new(&config),
            config,
        };

        // Inizializza la root directory
        fs.inode_to_path.insert(1, "".to_string());
        fs.path_to_inode.insert("".to_string(), 1);
        fs.inode_to_type.insert(1, FileType::Directory);
        // Inserisce anche gli attributi della root nella cache
        let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
        fs.attribute_cache.put(1, ROOT_DIR_ATTR, ttl);

        fs
    }
}

// L'implementazione del trait Filesystem rimane un semplice dispatcher
impl Filesystem for RemoteFS {
    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        attr_ops::getattr(self, req, ino, reply);
    }
    fn setattr(&mut self, req: &Request<'_>, ino: u64, mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, atime: Option<fuser::TimeOrNow>, mtime: Option<fuser::TimeOrNow>, ctime: Option<std::time::SystemTime>, fh: Option<u64>, crtime: Option<std::time::SystemTime>, chgtime: Option<std::time::SystemTime>, bkuptime: Option<std::time::SystemTime>, flags: Option<u32>, reply: ReplyAttr) {
        attr_ops::setattr(self, req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
    }
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        read_ops::lookup(self, req, parent, name, reply);
    }
    fn readdir(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, reply: ReplyDirectory) {
        read_ops::readdir(self, req, ino, fh, offset, reply);
    }
    fn read(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        read_ops::read(self, req, ino, fh, offset, size, flags, lock_owner, reply);
    }
    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        read_ops::open(self, req, ino, flags, reply);
    }
    fn write(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyWrite) {
        write_ops::write(self, req, ino, fh, offset, data, write_flags, flags, lock_owner, reply);
    }
    fn create(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
        write_ops::create(self, req, parent, name, mode, umask, flags, reply);
    }
    fn mkdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
        write_ops::mkdir(self, req, parent, name, mode, umask, reply);
    }
    fn unlink(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        write_ops::unlink(self, req, parent, name, reply);
    }
    fn rmdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        write_ops::rmdir(self, req, parent, name, reply);
    }
    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        write_ops::release(self, _req, _ino, _fh, _flags, _lock_owner, _flush, reply);
    }
    fn flush(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        write_ops::flush(self, _req, _ino, _fh, _lock_owner, reply);
    }
    fn rename(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, flags: u32, reply: ReplyEmpty) {
        write_ops::rename(self, req, parent, name, newparent, newname, flags, reply);
    }
}
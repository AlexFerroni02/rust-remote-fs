use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, ReplyWrite, Request, ReplyEmpty
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};
use crate::config::Config;
use crate::fs::cache::AttributeCache;
pub mod cache;
pub mod prelude;
mod attr;
mod read;
mod write;
mod create;
mod delete;
mod rename;

pub const TTL: Duration = Duration::from_secs(5);
pub const ROOT_DIR_ATTR: FileAttr = FileAttr {
    ino: 1, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH, kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 501, gid: 20,
    rdev: 0, flags: 0, blksize: 5120,
};

pub struct OpenWriteFile {
    pub(crate) path: String,
    pub(crate) buffer: HashMap<i64, Vec<u8>>,
}

pub struct RemoteFS {
    pub(crate) client: reqwest::Client,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) inode_to_path: HashMap<u64, String>,
    pub(crate) path_to_inode: HashMap<String, u64>,
    pub(crate) inode_to_type: HashMap<u64, FileType>,
    pub(crate) next_inode: u64,
    pub(crate) attribute_cache: AttributeCache,
    pub(crate) config: Config,
    pub(crate) open_files: HashMap<u64, OpenWriteFile>,
    pub(crate) next_fh: u64,
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
            open_files: HashMap::new(),
            next_fh: 1,
        };

        fs.inode_to_path.insert(1, "".to_string());
        fs.path_to_inode.insert("".to_string(), 1);
        fs.inode_to_type.insert(1, FileType::Directory);
        let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
        fs.attribute_cache.put(1, ROOT_DIR_ATTR, ttl);
        fs
    }
}

// L'implementazione del trait Filesystem ora smista ai nuovi moduli
impl Filesystem for RemoteFS {
    // --- ATTR ---
    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        attr::getattr(self, req, ino, reply);
    }
    fn setattr(&mut self, req: &Request<'_>, ino: u64, mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, atime: Option<fuser::TimeOrNow>, mtime: Option<fuser::TimeOrNow>, ctime: Option<std::time::SystemTime>, fh: Option<u64>, crtime: Option<std::time::SystemTime>, chgtime: Option<std::time::SystemTime>, bkuptime: Option<std::time::SystemTime>, flags: Option<u32>, reply: ReplyAttr) {
        attr::setattr(self, req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
    }

    // --- READ ---
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        read::lookup(self, req, parent, name, reply);
    }
    fn readdir(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, reply: ReplyDirectory) {
        read::readdir(self, req, ino, fh, offset, reply);
    }
    fn read(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        read::read(self, req, ino, fh, offset, size, flags, lock_owner, reply);
    }
    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        read::open(self, req, ino, flags, reply);
    }

    // --- WRITE ---
    fn write(&mut self, req: &Request<'_>, ino: u64, fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyWrite) {
        write::write(self, req, ino, fh, offset, data, write_flags, flags, lock_owner, reply);
    }
    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        write::release(self, _req, _ino, _fh, _flags, _lock_owner, _flush, reply);
    }
    fn flush(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        write::flush(self, _req, _ino, _fh, _lock_owner, reply);
    }

    // --- CREATE ---
    fn create(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
        create::create(self, req, parent, name, mode, umask, flags, reply);
    }
    fn mkdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
        create::mkdir(self, req, parent, name, mode, umask, reply);
    }

    // --- DELETE ---
    fn unlink(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        delete::unlink(self, req, parent, name, reply);
    }
    fn rmdir(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        delete::rmdir(self, req, parent, name, reply);
    }

    // --- RENAME ---
    fn rename(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, flags: u32, reply: ReplyEmpty) {
        rename::rename(self, req, parent, name, newparent, newname, flags, reply);
    }
}
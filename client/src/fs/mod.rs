use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, ReplyWrite, Request, ReplyEmpty
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

mod read_ops;
mod write_ops;
mod attr_ops;

pub(crate) const TTL: Duration = Duration::from_secs(1);
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
    pub(crate) inode_to_attr: HashMap<u64, FileAttr>,
    pub(crate) next_inode: u64,
}

impl RemoteFS {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let mut inode_to_path = HashMap::new();
        let mut path_to_inode = HashMap::new();
        let mut inode_to_type = HashMap::new();
        let inode_to_attr = HashMap::new();

        inode_to_path.insert(1, "".to_string());
        path_to_inode.insert("".to_string(), 1);
        inode_to_type.insert(1, FileType::Directory);

        Self {
            client: reqwest::Client::new(),
            runtime, inode_to_path, path_to_inode,
            inode_to_type, inode_to_attr, next_inode: 2,
        }
    }
}

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

    fn release(&mut self, req: &Request<'_>, ino: u64, fh: u64, flags: i32, lock_owner: Option<u64>, flush: bool, reply: ReplyEmpty) {
        write_ops::release(self, req, ino, fh, flags, lock_owner, flush, reply);
    }
    fn flush(&mut self, req: &Request<'_>, ino: u64, fh: u64, lock_owner: u64, reply: ReplyEmpty) {
        write_ops::flush(self, req, ino, fh, lock_owner, reply);
    }
}
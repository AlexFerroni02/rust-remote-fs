use fuser::{FileAttr, ReplyAttr, Request};
use libc::ENOENT;
use super::{RemoteFS, ROOT_DIR_ATTR, TTL};

pub fn getattr(fs: &mut RemoteFS, _req: &Request, ino: u64, reply: ReplyAttr) {
    println!("<- GETATTR: Request for inode {}", ino);

    if ino == 1 {
        reply.attr(&TTL, &ROOT_DIR_ATTR);
        return;
    }

    match fs.inode_to_attr.get(&ino) {
        Some(attrs) => {
            println!("   ✔ GETATTR: Found attributes for inode {} in cache", ino);
            reply.attr(&TTL, attrs);
        }
        None => {
            println!("   ❌ GETATTR: Did NOT find attributes for inode {} in cache!", ino);
            reply.error(ENOENT);
        }
    }
}

pub fn setattr(_fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, _atime: Option<fuser::TimeOrNow>, _mtime: Option<fuser::TimeOrNow>, _ctime: Option<std::time::SystemTime>, _fh: Option<u64>, _crtime: Option<std::time::SystemTime>, _chgtime: Option<std::time::SystemTime>, _bkuptime: Option<std::time::SystemTime>, _flags: Option<u32>, reply: ReplyAttr) {
    let attrs = _fs.inode_to_attr.get(&ino).cloned().unwrap_or(ROOT_DIR_ATTR);
    println!("SETATTR called for inode {}, new size: {:?}", ino, size);
    reply.attr(&TTL, &attrs);
}
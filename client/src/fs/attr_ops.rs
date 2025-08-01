// src/fs/attr_ops.rs

use fuser::{FileAttr, FileType, ReplyAttr, Request};
use libc::ENOENT;
use std::time::UNIX_EPOCH;

// `super` si riferisce al modulo genitore (`mod.rs`)
use super::{RemoteFS, ROOT_DIR_ATTR, TTL};

pub fn getattr(fs: &mut RemoteFS, _req: &Request, ino: u64, reply: ReplyAttr) {
    println!("📋 GETATTR: ino={}", ino);
    if ino == 1 {
        reply.attr(&TTL, &ROOT_DIR_ATTR);
        return;
    }

    if let Some(path) = fs.inode_to_path.get(&ino) {
        let kind = fs.inode_to_type.get(&ino).copied().unwrap_or(FileType::RegularFile);
        println!("📋 GETATTR: path='{}', kind={:?}", path, kind);
        let attrs = FileAttr {
            ino,
            size: 1024,
            blocks: 1,
            kind,
            perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
            nlink: 1, uid: 501, gid: 20, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
        };
        reply.attr(&TTL, &attrs);
    } else {
        reply.error(ENOENT);
    }
}

pub fn setattr(_fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, _atime: Option<fuser::TimeOrNow>, _mtime: Option<fuser::TimeOrNow>, _ctime: Option<std::time::SystemTime>, _fh: Option<u64>, _crtime: Option<std::time::SystemTime>, _chgtime: Option<std::time::SystemTime>, _bkuptime: Option<std::time::SystemTime>, _flags: Option<u32>, reply: ReplyAttr) {
    println!("SETATTR called for inode {}, new size: {:?}", ino, size);
    let dummy_attrs = FileAttr {
        ino, size: size.unwrap_or(1024), blocks: 1, atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH, ctime: UNIX_EPOCH, crtime: UNIX_EPOCH,
        kind: FileType::RegularFile, perm: 0o644, nlink: 1,
        uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };
    reply.attr(&TTL, &dummy_attrs);
}
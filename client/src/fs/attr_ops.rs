use fuser::{FileAttr, ReplyAttr, Request, TimeOrNow};
use libc::ENOENT;
use super::{RemoteFS, ROOT_DIR_ATTR, TTL};
use std::time::SystemTime;

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

pub fn setattr(
    fs: &mut RemoteFS,
    _req: &Request<'_>,
    ino: u64,
    mode: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
    size: Option<u64>,
    atime: Option<TimeOrNow>,
    mtime: Option<TimeOrNow>,
    _ctime: Option<SystemTime>,
    _fh: Option<u64>,
    _crtime: Option<SystemTime>,
    _chgtime: Option<SystemTime>,
    _bkuptime: Option<SystemTime>,
    _flags: Option<u32>,
    reply: ReplyAttr,
) {
    println!("-> SETATTR: Request for inode {}", ino);

    let attrs = match fs.inode_to_attr.get_mut(&ino) {
        Some(a) => a,
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    if let Some(m) = mode {
        attrs.perm = m as u16;
    }
    if let Some(u) = uid {
        attrs.uid = u;
    }
    if let Some(g) = gid {
        attrs.gid = g;
    }
    if let Some(s) = size {
        attrs.size = s;
    }
    let now = SystemTime::now();
    if let Some(a) = atime {
        attrs.atime = match a {
            TimeOrNow::SpecificTime(t) => t,
            TimeOrNow::Now => now,
        }
    }
    if let Some(m) = mtime {
        attrs.mtime = match m {
            TimeOrNow::SpecificTime(t) => t,
            TimeOrNow::Now => now,
        }
    }
    attrs.ctime = now;

    reply.attr(&TTL, attrs);
    println!("   ✔ SETATTR: Replied with updated attrs for inode {}", ino);
}
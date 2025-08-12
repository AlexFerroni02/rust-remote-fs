use fuser::{FileAttr, ReplyAttr, Request, TimeOrNow,FileType};
use libc::{EIO, ENOENT};
use super::{RemoteFS, ROOT_DIR_ATTR, TTL};
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};
use crate::api_client::get_files_from_server;
pub fn getattr(fs: &mut RemoteFS, _req: &Request, ino: u64, reply: ReplyAttr) {
    println!("<- GETATTR: Request for inode {}", ino);

    if ino == 1 {
        reply.attr(&TTL, &ROOT_DIR_ATTR);
        return;
    }

    // parte nuova
    // 1) Cache hit
    if let Some(attr) = fs.inode_to_attr.get(&ino) {
        reply.attr(&TTL, attr);
        return;
    }
    // 2) Cache miss: ricostruisci interrogando il server (list del parent)
    let Some(path) = fs.inode_to_path.get(&ino).cloned() else {
        reply.error(ENOENT);
        return;
    };
    let (parent_path, name) = match path.rsplit_once('/') {
        Some((p, n)) => (p.to_string(), n.to_string()),
        None => (String::new(), path.clone()),
    };

    let entries = match fs.runtime.block_on(async {
        get_files_from_server(&fs.client, &parent_path).await
    }) {
        Ok(v) => v,
        Err(_) => {
            reply.error(EIO);
            return;
        }
    };

    if let Some(entry) = entries.into_iter().find(|e| e.name == name) {
        let kind = if entry.kind.eq_ignore_ascii_case("dir") || entry.kind.eq_ignore_ascii_case("directory") {
            FileType::Directory
        } else {
            FileType::RegularFile
        };
        let perm = u16::from_str_radix(&entry.perm, 8)
            .unwrap_or_else(|_| if kind == FileType::Directory { 0o755 } else { 0o644 });

        let attrs = FileAttr {
            ino,
            size: entry.size,
            blocks: (entry.size + 511) / 512,
            atime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            mtime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            ctime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            crtime: UNIX_EPOCH,
            kind,
            perm,
            nlink: if kind == FileType::Directory { 2 } else { 1 },
            uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
        };

        fs.inode_to_attr.insert(ino, attrs.clone());
        reply.attr(&TTL, &attrs);
    } else {
        reply.error(ENOENT);
    }
    /* 
    match fs.inode_to_attr.get(&ino) {
        Some(attrs) => {
            println!("   ✔ GETATTR: Found attributes for inode {} in cache", ino);
            reply.attr(&TTL, attrs);
        }
        None => {
            println!("   ❌ GETATTR: Did NOT find attributes for inode {} in cache!", ino);
            reply.error(ENOENT);
        }
    }*/
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
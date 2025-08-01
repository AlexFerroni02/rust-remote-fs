use fuser::{FileAttr, FileType, ReplyCreate, ReplyWrite, ReplyEntry, Request, ReplyAttr, ReplyEmpty};
use libc::{ENOENT, EIO};
use std::ffi::OsStr;
use std::time::UNIX_EPOCH;
use crate::api_client::put_file_content_to_server;
use super::{RemoteFS, TTL};

pub fn write(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, _offset: i64, data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
    if let Some(file_path) = fs.inode_to_path.get(&ino) {
        let content = String::from_utf8_lossy(data).to_string();
        let res = fs.runtime.block_on(async {
            put_file_content_to_server(&fs.client, file_path, &content).await
        });
        match res {
            Ok(_) => reply.written(data.len() as u32),
            Err(_) => reply.error(EIO),
        }
    } else {
        reply.error(ENOENT);
    }
}

pub fn create(fs: &mut RemoteFS, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, _umask: u32, _flags: i32, reply: ReplyCreate) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };
    let filename = name.to_str().unwrap();
    let full_path = if parent_path.is_empty() {
        filename.to_string()
    } else {
        format!("{}/{}", parent_path, filename)
    };

    if fs.runtime.block_on(put_file_content_to_server(&fs.client, &full_path, "")).is_err() {
        reply.error(EIO);
        return;
    }

    let inode = fs.next_inode;
    fs.next_inode += 1;
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path, inode);
    fs.inode_to_type.insert(inode, FileType::RegularFile);

    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, kind: FileType::RegularFile,
        perm: mode as u16, nlink: 1, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };
    reply.created(&TTL, &attrs, 0, inode, 0);
}

pub fn mkdir(fs: &mut RemoteFS, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, _umask: u32, reply: ReplyEntry) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };
    let dirname = name.to_str().unwrap();
    let full_path = if parent_path.is_empty() {
        dirname.to_string()
    } else {
        format!("{}/{}", parent_path, dirname)
    };

    if fs.runtime.block_on(async {
        let url = format!("http://localhost:8080/mkdir/{}", full_path);
        fs.client.post(&url).send().await
    }).is_err() {
        reply.error(EIO);
        return;
    }

    let inode = fs.next_inode;
    fs.next_inode += 1;
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path, inode);
    fs.inode_to_type.insert(inode, FileType::Directory);

    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, kind: FileType::Directory,
        perm: mode as u16, nlink: 2, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };
    reply.entry(&TTL, &attrs, 0);
}

pub fn unlink(fs: &mut RemoteFS, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };
    let filename = name.to_str().unwrap();
    let full_path = if parent_path.is_empty() {
        filename.to_string()
    } else {
        format!("{}/{}", parent_path, filename)
    };

    if fs.runtime.block_on(async {
        let url = format!("http://localhost:8080/files/{}", full_path);
        fs.client.delete(&url).send().await
    }).is_err() {
        reply.error(EIO);
        return;
    }

    if let Some(&inode) = fs.path_to_inode.get(&full_path) {
        fs.inode_to_path.remove(&inode);
        fs.inode_to_type.remove(&inode);
    }
    fs.path_to_inode.remove(&full_path);
    reply.ok();
}

pub fn rmdir(fs: &mut RemoteFS, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
    unlink(fs, req, parent, name, reply);
}
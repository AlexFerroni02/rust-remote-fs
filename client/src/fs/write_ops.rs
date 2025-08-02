use fuser::{FileAttr, FileType, ReplyCreate, ReplyWrite, ReplyEntry, Request, ReplyAttr, ReplyEmpty};
use libc::{ENOENT, EIO};
use std::ffi::OsStr;
use std::time::UNIX_EPOCH;
use crate::api_client::{put_file_content_to_server, get_file_content_from_server};
use super::{RemoteFS, TTL};

pub fn write(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
    let file_path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let old_content_result = fs.runtime.block_on(async {
        get_file_content_from_server(&fs.client, &file_path).await
    });

    let old_content = match old_content_result {
        Ok(c) => c,
        Err(_) if offset == 0 => "".to_string(),
        Err(_) => {
            reply.error(EIO);
            return;
        }
    };

    let old_bytes = old_content.as_bytes();
    let offset = offset as usize;

    let final_capacity = std::cmp::max(offset + data.len(), old_bytes.len());
    let mut new_content = Vec::with_capacity(final_capacity);

    let prefix_len = std::cmp::min(offset, old_bytes.len());
    new_content.extend_from_slice(&old_bytes[..prefix_len]);

    if new_content.len() < offset {
        new_content.resize(offset, 0);
    }

    new_content.extend_from_slice(data);


    let end_of_write = offset + data.len();
    if offset > 0 && old_bytes.len() > end_of_write {
        new_content.extend_from_slice(&old_bytes[end_of_write..]);
    }

    match String::from_utf8(new_content) {
        Ok(content_str) => {
            let res = fs.runtime.block_on(async {
                put_file_content_to_server(&fs.client, &file_path, &content_str).await
            });

            match res {
                Ok(_) => reply.written(data.len() as u32),
                Err(_) => reply.error(EIO),
            }
        },
        Err(_) => reply.error(EIO),
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

    fs.inode_to_attr.insert(inode, attrs.clone());

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
pub fn release(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
    reply.ok();
}

pub fn flush(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
    reply.ok();
}
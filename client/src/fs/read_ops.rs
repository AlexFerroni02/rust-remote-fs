use fuser::{FileAttr, FileType, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyOpen, Request};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::UNIX_EPOCH;
use crate::api_client::{get_file_content_from_server, get_files_from_server};

use super::{RemoteFS, ROOT_DIR_ATTR, TTL};

pub fn lookup(fs: &mut RemoteFS, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let name_str = name.to_str().unwrap_or("");
    println!("🔍 LOOKUP: parent={}, name='{}'", parent, name_str);
    let full_path = if parent_path.is_empty() {
        name_str.to_string()
    } else {
        format!("{}/{}", parent_path, name_str)
    };

    if let Some(&inode) = fs.path_to_inode.get(&full_path) {
        let kind = fs.inode_to_type.get(&inode).copied().unwrap_or(FileType::RegularFile);
        let attrs = FileAttr {
            ino: inode, size: 1024, blocks: 1, kind,
            perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
            nlink: 1, uid: 501, gid: 20, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
        };
        reply.entry(&TTL, &attrs, 0);
        return;
    }

    let file_list = fs.runtime.block_on(async {
        get_files_from_server(&fs.client, &parent_path).await
    });

    if let Ok(files) = file_list {
        if let Some(found_file) = files.iter().find(|f| f.trim_end_matches('/') == name_str) {
            let inode = fs.next_inode;
            fs.next_inode += 1;

            let is_dir = found_file.ends_with('/');
            let kind = if is_dir { FileType::Directory } else { FileType::RegularFile };
            fs.inode_to_path.insert(inode, full_path.clone());
            fs.path_to_inode.insert(full_path, inode);
            fs.inode_to_type.insert(inode, kind);

            let attrs = FileAttr {
                ino: inode, size: 1024, blocks: 1, kind,
                perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
                nlink: 1, uid: 501, gid: 20, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
            };
            reply.entry(&TTL, &attrs, 0);
        } else {
            reply.error(ENOENT);
        }
    } else {
        reply.error(ENOENT);
    }
}

pub fn readdir(fs: &mut RemoteFS, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
    let dir_path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let file_list = fs.runtime.block_on(async {
        get_files_from_server(&fs.client, &dir_path).await
    });

    let mut entries = vec![
        (ino, FileType::Directory, ".".to_string()),
        (1, FileType::Directory, "..".to_string()),
    ];

    if let Ok(files) = file_list {
        for file_name in files {
            let is_dir = file_name.ends_with('/');
            let clean_name = file_name.trim_end_matches('/').to_string();
            let full_path = if dir_path.is_empty() {
                clean_name.clone()
            } else {
                format!("{}/{}", dir_path, clean_name)
            };
            let inode = *fs.path_to_inode.entry(full_path.clone()).or_insert_with(|| {
                let new_ino = fs.next_inode;
                fs.next_inode += 1;
                fs.inode_to_path.insert(new_ino, full_path);
                new_ino
            });
            let kind = if is_dir { FileType::Directory } else { FileType::RegularFile };
            fs.inode_to_type.insert(inode, kind);
            entries.push((inode, kind, clean_name));
        }
    }

    for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
        if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
            break;
        }
    }
    reply.ok();
}

pub fn read(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
    if let Some(file_path) = fs.inode_to_path.get(&ino) {
        let content = fs.runtime.block_on(async {
            get_file_content_from_server(&fs.client, file_path).await
        }).unwrap_or_default();
        let start = offset as usize;
        let end = std::cmp::min(start + size as usize, content.len());
        reply.data(&content.as_bytes()[start..end]);
    } else {
        reply.error(ENOENT);
    }
}

pub fn open(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
    reply.opened(ino, 0);
}
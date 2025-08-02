// src/fs/read_ops.rs

use crate::api_client::{get_files_from_server,get_file_content_from_server};
use fuser::{FileAttr, FileType, ReplyDirectory, ReplyEntry, Request, ReplyData, ReplyOpen};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};
use super::{RemoteFS, TTL};

pub fn lookup(fs: &mut RemoteFS, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };


    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &parent_path)) {
        Ok(list) => list,
        Err(_) => { reply.error(ENOENT); return; }
    };

    let name_str = name.to_str().unwrap();
    if let Some(entry) = entry_list.iter().find(|e| e.name == name_str) {
        let full_path = if parent_path.is_empty() { name_str.to_string() } else { format!("{}/{}", parent_path, name_str) };

        let inode = *fs.path_to_inode.entry(full_path.clone()).or_insert_with_key(|_key| {
            let new_ino = fs.next_inode;
            fs.next_inode += 1;
            fs.inode_to_path.insert(new_ino, full_path);
            new_ino
        });

        let kind = if entry.kind == "directory" { FileType::Directory } else { FileType::RegularFile };
        let perm = u16::from_str_radix(&entry.perm, 8).unwrap_or(if kind == FileType::Directory { 0o755 } else { 0o644 });
        let attrs = FileAttr {
            ino: inode, size: entry.size, blocks: (entry.size + 511) / 512,
            atime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            mtime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            ctime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            crtime: UNIX_EPOCH, kind, perm,
            nlink: if kind == FileType::Directory { 2 } else { 1 },
            uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
        };
        fs.inode_to_attr.insert(inode, attrs.clone());
        fs.inode_to_type.insert(inode, kind);
        reply.entry(&TTL, &attrs, 0);
    } else {
        reply.error(ENOENT);
    }
}

pub fn readdir(fs: &mut RemoteFS, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
    let dir_path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };

    let mut entries_to_add: Vec<(u64, FileType, String)> = vec![];
    if offset == 0 {
        entries_to_add.push((ino, FileType::Directory, ".".to_string()));
    }
    if offset <= 1 {
        entries_to_add.push((1, FileType::Directory, "..".to_string()));
    }

    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &dir_path)) {
        Ok(list) => list,
        Err(_) => { reply.ok(); return; }
    };

    for entry in entry_list {
        let full_path = if dir_path.is_empty() { entry.name.clone() } else { format!("{}/{}", dir_path, &entry.name) };
        let inode = *fs.path_to_inode.entry(full_path.clone()).or_insert_with_key(|_key| {
            let new_ino = fs.next_inode;
            fs.next_inode += 1;
            fs.inode_to_path.insert(new_ino, full_path);
            new_ino
        });

        let kind = if entry.kind == "directory" { FileType::Directory } else { FileType::RegularFile };
        let perm = u16::from_str_radix(&entry.perm, 8).unwrap_or(if kind == FileType::Directory { 0o755 } else { 0o644 });
        let attrs = FileAttr {
            ino, size: entry.size, blocks: (entry.size + 511) / 512,
            atime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            mtime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            ctime: UNIX_EPOCH + Duration::from_secs(entry.mtime as u64),
            crtime: UNIX_EPOCH, kind, perm,
            nlink: if kind == FileType::Directory { 2 } else { 1 },
            uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
        };
        fs.inode_to_attr.insert(inode, attrs);
        fs.inode_to_type.insert(inode, kind);
        entries_to_add.push((inode, kind, entry.name));
    }

    for (i, (ino_to_add, kind_to_add, name_to_add)) in entries_to_add.into_iter().enumerate().skip(offset as usize) {
        if reply.add(ino_to_add, (i + 1) as i64, kind_to_add, &name_to_add) {
            break;
        }
    }

    reply.ok();
}

pub fn read(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
    if let Some(file_path) = fs.inode_to_path.get(&ino) {

        let content_result = fs.runtime.block_on(async {
            get_file_content_from_server(&fs.client, file_path).await
        });

        match content_result {
            Ok(content) => {
                let content_bytes = content.as_bytes();
                let start = offset as usize;
                if start >= content_bytes.len() {
                    reply.data(&[]);
                    return;
                }
                let end = std::cmp::min(start + size as usize, content_bytes.len());
                reply.data(&content_bytes[start..end]);
            },
            Err(_) => {
                reply.error(ENOENT);
            }
        }
    } else {
        reply.error(ENOENT);
    }
}

pub fn open(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
    reply.opened(ino, 0);
}
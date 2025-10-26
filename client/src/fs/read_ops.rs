use crate::api_client::{get_files_from_server,get_file_content_from_server};
use fuser::{FileAttr, FileType, ReplyDirectory, ReplyEntry, Request, ReplyData, ReplyOpen};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};
use super::{RemoteFS, TTL, ROOT_DIR_ATTR};

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

        // --- MODIFICA ---
        // Chiama la nuova funzione helper che restituisce direttamente gli attributi.
        if let Some(attr) = super::attr_ops::fetch_and_cache_attributes(fs, inode) {
            reply.entry(&TTL, &attr, 0);
        } else {
            reply.error(ENOENT);
        }
        // --- FINE MODIFICA ---
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
        // Trova il parent inode per ".."
        let parent_ino = if ino == 1 { 1 } else {
            let parent_p = dir_path.rsplit_once('/').map_or("", |(p, _)| p);
            *fs.path_to_inode.get(parent_p).unwrap_or(&1)
        };
        entries_to_add.push((parent_ino, FileType::Directory, "..".to_string()));
    }

    // --- MODIFICA ---
    // Non facciamo il fetch qui se non necessario.
    // `readdir` deve solo fornire nomi, inode e tipi.
    if offset < 2 { // Solo se dobbiamo leggere le entry reali
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

            let kind = if entry.kind.eq_ignore_ascii_case("dir") || entry.kind.eq_ignore_ascii_case("directory") { FileType::Directory } else { FileType::RegularFile };
            fs.inode_to_type.insert(inode, kind);
            entries_to_add.push((inode, kind, entry.name));
        }
    }
    // --- FINE MODIFICA ---

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

pub fn open(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
    reply.opened(0, 0);
}
use crate::api_client::{get_files_from_server,get_file_content_from_server};
use fuser::{FileType, ReplyDirectory, ReplyEntry, Request, ReplyData, ReplyOpen};
use libc::ENOENT;
use std::ffi::OsStr;
use super::{RemoteFS, TTL};

// --- AGGIUNTE NECESSARIE ---
use libc; // Per i flag O_WRONLY, O_RDWR
use super::OpenWriteFile; // Per la struct della cache
use std::collections::HashMap; // Per creare la cache
// --- FINE AGGIUNTE ---

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
    if let Some(_entry) = entry_list.iter().find(|e| e.name == name_str) {
        let full_path = if parent_path.is_empty() { name_str.to_string() } else { format!("{}/{}", parent_path, name_str) };

        let inode = *fs.path_to_inode.entry(full_path.clone()).or_insert_with_key(|_key| {
            let new_ino = fs.next_inode;
            fs.next_inode += 1;
            fs.inode_to_path.insert(new_ino, full_path);
            new_ino
        });

        // NOTA: 'fetch_and_cache_attributes' deve esistere in attr_ops.rs
        if let Some(attr) = crate::fs::attr_ops::fetch_and_cache_attributes(fs, inode) {
            reply.entry(&TTL, &attr, 0);
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
        None => { reply.error(ENOENT); return; }
    };

    let mut entries_to_add: Vec<(u64, FileType, String)> = vec![];
    if offset == 0 {
        entries_to_add.push((ino, FileType::Directory, ".".to_string()));
        let parent_ino = if ino == 1 { 1 } else {
            let parent_p = dir_path.rsplit_once('/').map_or("", |(p, _)| p);
            *fs.path_to_inode.get(parent_p).unwrap_or(&1)
        };
        entries_to_add.push((parent_ino, FileType::Directory, "..".to_string()));
    }

    if offset < 2 {
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
                let content_bytes = &content;
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

/// OPEN (MODIFICATA)
/// Gestisce l'apertura di file, preparando la cache se si apre in scrittura.
pub fn open(
    fs: &mut RemoteFS,
    _req: &Request<'_>,
    ino: u64,
    flags: i32,
    reply: ReplyOpen,
) {
    // Controlla se le flag di apertura includono l'accesso in scrittura
    // (O_WRONLY = 1, O_RDWR = 2)
    let write_access = (flags & libc::O_WRONLY != 0) || (flags & libc::O_RDWR != 0);

    if write_access {
        // --- PERCORSO DI SCRITTURA ---
        let relative_path = match fs.inode_to_path.get(&ino) {
            Some(p) => p.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        // Genera un nuovo file handle
        let fh = fs.next_fh;
        fs.next_fh += 1;

        // Crea la cache per questo handle
        let open_file = OpenWriteFile {
            path: relative_path,
            buffer: HashMap::new(), // Il buffer inizia sempre vuoto
        };

        fs.open_files.insert(fh, open_file);

        // Rispondi con il nuovo file handle
        reply.opened(fh, 0);

    } else {
        // --- PERCORSO DI SOLA LETTURA ---
        reply.opened(0, 0);
    }
}
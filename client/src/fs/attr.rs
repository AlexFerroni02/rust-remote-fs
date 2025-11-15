use super::prelude::*;
use serde_json::json;

pub fn fetch_and_cache_attributes(fs: &mut RemoteFS, ino: u64) -> Option<FileAttr> {
    if ino == 1 {
        return Some(ROOT_DIR_ATTR);
    }

    // 1. Controlla la cache
    if let Some(attr) = fs.attribute_cache.get(&ino) {
        return Some(attr);
    }
    
    // 2. Se non è in cache (CACHE MISS), contatta il server
    let path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => return None,
    };

    let (parent_path, file_name) = match path.rsplit_once('/') {
        Some((p, f)) => (p.to_string(), f.to_string()),
        None => ("".to_string(), path.clone()),
    };

    let entries = match fs.runtime.block_on(get_files_from_server(&fs.client, &parent_path)) {
        Ok(list) => list,
        Err(_) => return None,
    };

    if let Some(entry) = entries.into_iter().find(|e| e.name == file_name) {
        let kind = if entry.kind.eq_ignore_ascii_case("dir") || entry.kind.eq_ignore_ascii_case("directory") { FileType::Directory } else { FileType::RegularFile };
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

        // 3. Metti in cache i nuovi attributi
        let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
        fs.attribute_cache.put(ino, attrs.clone(), ttl);
        
        Some(attrs)
    } else {
        None
    }
}

/// La funzione `getattr` ora è un semplice wrapper attorno alla funzione helper.
pub fn getattr(fs: &mut RemoteFS, _req: &Request, ino: u64, reply: ReplyAttr) {
    match fetch_and_cache_attributes(fs, ino) {
        Some(attr) => reply.attr(&TTL, &attr),
        None => reply.error(ENOENT),
    }
}

pub fn setattr(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>, _ctime: Option<SystemTime>, _fh: Option<u64>, _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, _flags: Option<u32>, reply: ReplyAttr) {
    
    let path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };

    //  CHMOD ---
    if let Some(new_mode) = mode {
        let perm_str = format!("{:o}", new_mode & 0o777);
        let url = format!("http://localhost:8080/files/{}", path);
        let payload = json!({ "perm": perm_str });

        let res = fs.runtime.block_on(async {
            fs.client.patch(&url).json(&payload).send().await
        });

        if res.is_err() {
            reply.error(EIO);
            return;
        }
    }
    

    if let Some(new_size) = size {
        let old_content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &path)) {
            Ok(c) => c,
            Err(_) => "".into()
        };
        let mut bytes = old_content.to_vec();
        bytes.resize(new_size as usize, 0);

        if let Ok(new_content_str) = String::from_utf8(bytes) {
            if fs.runtime.block_on(put_file_content_to_server(&fs.client, &path, new_content_str.into())).is_err() {
                reply.error(EIO);
                return;
            }
        } else {
            reply.error(EIO);
            return;
        }
    }

    // Questa parte finale è comune a entrambe le modifiche
    println!("[CACHE] INVALIDATE: Rimuovo attributi per l'inode {} a causa di setattr.", ino);
    fs.attribute_cache.remove(&ino);

    match fetch_and_cache_attributes(fs, ino) {
        Some(attr) => reply.attr(&TTL, &attr),
        None => reply.error(ENOENT),
    }
}
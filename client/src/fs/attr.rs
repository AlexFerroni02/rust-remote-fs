use super::prelude::*;
use serde_json::json;

/// Fetches attributes for an Inode, using the cache if available.
///
/// This is the central function for attribute management. It implements a
/// "cache-miss" strategy:
/// 1. Check if the Inode is the ROOT (1). If so, return static root attributes.
/// 2. Check if the attributes are in the `attribute_cache`. If so, return them.
/// 3. On a cache miss, fetch the parent directory's listing from the server.
/// 4. Find the matching entry in the list to build the `FileAttr`.
/// 5. Store the new attributes in the cache before returning them.
///
/// # Arguments
/// * `fs` - A mutable reference to the `RemoteFS` state.
/// * `ino` - The Inode number to look up.
///
/// # Returns
/// * `Some(FileAttr)` if the Inode is found (in cache or on the server).
/// * `None` if the Inode's path cannot be found or the file does not exist on the server.
pub fn fetch_and_cache_attributes(fs: &mut RemoteFS, ino: u64) -> Option<FileAttr> {
    if ino == 1 {
        return Some(ROOT_DIR_ATTR);
    }

    // 1. Check cache
    if let Some(attr) = fs.attribute_cache.get(&ino) {
        return Some(attr);
    }

    // 2. Cache miss, contact server
    let path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => return None,
    };

    // We must list the parent to get metadata for the requested file
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
            uid: 501, // Faked UID
            gid: 20,  // Faked GID
            rdev: 0, flags: 0, blksize: 5120,
        };

        // 3. Store new attributes in cache
        let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
        fs.attribute_cache.put(ino, attrs.clone(), ttl);

        Some(attrs)
    } else {
        None
    }
}

/// FUSE `getattr` implementation.
///
/// This function is a simple wrapper around `fetch_and_cache_attributes`.
/// It replies with the found attributes or an `ENOENT` error.
pub fn getattr(fs: &mut RemoteFS, _req: &Request, ino: u64, reply: ReplyAttr) {
    match fetch_and_cache_attributes(fs, ino) {
        Some(attr) => reply.attr(&TTL, &attr),
        None => reply.error(ENOENT),
    }
}

/// FUSE `setattr` implementation.
///
/// This function handles requests to change file attributes.
/// Currently supported operations:
/// - **`chmod` (mode):** Sends a `PATCH` request to the server with the new permission string.
/// - **`truncate` (size):** Performs a "Read-Modify-Write" operation. It fetches the
///   entire file, resizes it locally, and `PUT`s the entire new file back.
///
/// Unsupported operations (e.g., changing UID, GID, timestamps) are ignored.
///
/// After any successful operation, the attribute cache for the Inode is invalidated.
pub fn setattr(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>, _ctime: Option<SystemTime>, _fh: Option<u64>, _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, _flags: Option<u32>, reply: ReplyAttr) {

    let path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };

    // --- Handle `chmod` (mode change) ---
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

    // --- Handle `truncate` (size change) ---
    // This is a "Read-Modify-Write" operation.
    if let Some(new_size) = size {
        let old_content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &path)) {
            Ok(c) => c,
            Err(_) => "".into() // File might be new or empty
        };
        let mut bytes = old_content.to_vec();
        bytes.resize(new_size as usize, 0); // Truncate or extend with zeros

        // This is a potential bug: assumes file content is valid UTF-8.
        // `bytes` should be PUT directly.
        if let Ok(new_content_str) = String::from_utf8(bytes) {
            if fs.runtime.block_on(put_file_content_to_server(&fs.client, &path, new_content_str.into())).is_err() {
                reply.error(EIO);
                return;
            }
        } else {
            // This will fail for non-UTF8 files (e.g., images)
            reply.error(EIO);
            return;
        }
    }

    // After changes, invalidate cache and fetch new attributes
    println!("[CACHE] INVALIDATE: Removing attributes for Inode {} due to setattr.", ino);
    fs.attribute_cache.remove(&ino);

    match fetch_and_cache_attributes(fs, ino) {
        Some(attr) => reply.attr(&TTL, &attr),
        None => reply.error(ENOENT),
    }
}
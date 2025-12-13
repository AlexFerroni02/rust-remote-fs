use super::prelude::*;

/// Handles the FUSE `lookup` operation.
///
/// This is called by the kernel to find a file or directory by name within a
/// parent directory.
///
/// 1. It fetches the parent directory's contents from the remote server.
/// 2. It searches the list for an entry matching `name`.
/// 3. If found, it gets or creates a new inode for that entry, storing the
///    path-to-inode and inode-to-path mappings.
/// 4. It then calls `fetch_and_cache_attributes` to get the full metadata
///    (either from the cache or a fresh server call) and replies with it.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `parent` - The inode of the directory to search within.
/// * `name` - The name of the entry to look up.
/// * `reply` - The reply object to send the entry's attributes back.
pub fn lookup(fs: &mut RemoteFS, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
    let parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };

    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &parent_path, &fs.config.server_url)) {
        Ok(list) => list,
        Err(_) => { reply.error(ENOENT); return; }
    };

    let name_str = name.to_str().unwrap();
    if let Some(_entry) = entry_list.iter().find(|e| e.name == name_str) {
        let full_path = if parent_path.is_empty() { name_str.to_string() } else { format!("{}/{}", parent_path, name_str) };

        // Get or create a new inode for this path
        let inode = *fs.path_to_inode.entry(full_path.clone()).or_insert_with_key(|_key| {
            let new_ino = fs.next_inode;
            fs.next_inode += 1;
            fs.inode_to_path.insert(new_ino, full_path);
            new_ino
        });

        // Get attributes (from cache or server) and reply
        if let Some(attr) = crate::fs::attr::fetch_and_cache_attributes(fs, inode) {
            reply.entry(&TTL, &attr, 0);
        } else {
            reply.error(ENOENT);
        }
    } else {
        reply.error(ENOENT);
    }
}

/// Handles the FUSE `readdir` operation (e.g., `ls`).
///
/// This function lists the contents of a directory.
///
/// 1. It always adds the special `.` (current) and `..` (parent) entries
///    for `offset == 0`.
/// 2. It fetches the directory's contents from the remote server.
/// 3. It iterates the list, creating inodes for any new entries, and adds
///    each entry to the reply buffer.
/// 4. It respects the `offset` to handle large directories that require
///    multiple `readdir` calls.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `ino` - The inode of the directory to read.
/// * `offset` - The entry offset to start from.
/// * `reply` - The reply buffer to fill with directory entries.
pub fn readdir(fs: &mut RemoteFS, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
    let dir_path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => { reply.error(ENOENT); return; }
    };

    let mut entries_to_add: Vec<(u64, FileType, String)> = vec![];
    if offset == 0 {
        // Add '.' entry
        entries_to_add.push((ino, FileType::Directory, ".".to_string()));

        // Add '..' entry
        let parent_ino = if ino == 1 { 1 } else {
            let parent_p = dir_path.rsplit_once('/').map_or("", |(p, _)| p);
            *fs.path_to_inode.get(parent_p).unwrap_or(&1)
        };
        entries_to_add.push((parent_ino, FileType::Directory, "..".to_string()));
    }

    // Add server entries (only if we haven't finished with '.' and '..')
    if offset < 2 {
        let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &dir_path,  &fs.config.server_url)) {
            Ok(list) => list,
            Err(_) => { reply.ok(); return; } // Empty dir is fine
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

    // Add entries to the reply buffer, respecting the offset
    for (i, (ino_to_add, kind_to_add, name_to_add)) in entries_to_add.into_iter().enumerate().skip(offset as usize) {
        if reply.add(ino_to_add, (i + 1) as i64, kind_to_add, &name_to_add) {
            // Buffer is full
            break;
        }
    }
    reply.ok();
}

/// Handles the FUSE `read` operation.
///
/// This function fetches the *entire* file content from the server upon every
/// read request, and then replies with the specific byte range (`offset` to
/// `offset + size`) requested by the kernel.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `ino` - The inode of the file to read.
/// * `offset` - The byte offset in the file to start reading from.
/// * `size` - The maximum number of bytes to read.
/// * `reply` - The reply object to send the data bytes back.
pub fn read(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
    if let Some(file_path) = fs.inode_to_path.get(&ino) {

        // Fetch the entire file content
        let content_result = fs.runtime.block_on(async {
            get_file_content_from_server(&fs.client, file_path,  &fs.config.server_url).await
        });

        match content_result {
            Ok(content) => {
                // Slice the content based on the request
                let content_bytes = &content;
                let start = offset as usize;
                if start >= content_bytes.len() {
                    reply.data(&[]); // Offset is beyond the end of the file
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

/// Handles the FUSE `open` operation.
///
/// This function is critical for the write-caching strategy.
///
/// - If a file is opened for **reading only**, it replies with a dummy
///   file handle (`fh = 0`).
/// - If a file is opened for **writing** (with `O_WRONLY` or `O_RDWR`), it
///   generates a new, unique file handle (`fh`), creates an empty in-memory
///   write buffer (`OpenWriteFile`), and stores it in the `fs.open_files` map.
///   This `fh` is then used by subsequent `write` and `release` calls.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `ino` - The inode of the file being opened.
/// * `flags` - The open flags (e.g., `O_RDONLY`, `O_WRONLY`, `O_RDWR`).
/// * `reply` - The reply object to send the new file handle back.
pub fn open(
    fs: &mut RemoteFS,
    _req: &Request<'_>,
    ino: u64,
    flags: i32,
    reply: ReplyOpen,
) {
    // Check if the open flags include write access
    // (O_WRONLY = 1, O_RDWR = 2)
    let write_access = (flags & libc::O_WRONLY != 0) || (flags & libc::O_RDWR != 0);

    if write_access {
        // --- WRITE PATH ---
        let relative_path = match fs.inode_to_path.get(&ino) {
            Some(p) => p.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        // Generate a new, unique file handle
        let fh = fs.next_fh;
        fs.next_fh += 1;

        // Create a new, empty write cache for this handle
        let open_file = OpenWriteFile {
            path: relative_path,
            buffer: HashMap::new(), // Buffer always starts empty
        };

        fs.open_files.insert(fh, open_file);

        // Reply with the new file handle
        reply.opened(fh, 0);

    } else {
        // --- READ-ONLY PATH ---
        // No special handle needed for reading.
        reply.opened(0, 0);
    }
}
use super::prelude::*;

/// Handles the FUSE `create` operation (e.g., `touch file.txt` or `> file.txt`).
///
/// This function performs two main tasks:
/// 1. It immediately contacts the server via `PUT` to create an empty file.
/// 2. It sets up the in-memory write cache (`OpenWriteFile`) for this new file.
///
/// A new file handle (`fh`) is generated and associated with the in-memory cache.
/// This `fh` is returned to the kernel, which will use it for subsequent `write` calls.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `req` - The FUSE request (used to get UID/GID for the new attributes).
/// * `parent` - The inode of the parent directory.
/// * `name` - The name of the file to create.
/// * `reply` - The reply object to send the `fh` and attributes back to the kernel.
pub fn create(
    fs: &mut RemoteFS,
    req: &Request<'_>,
    parent: u64,
    name: &OsStr,
    mode: u32,
    _umask: u32,
    _flags: i32,
    reply: ReplyCreate,
) {
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

    // 1. Create the empty file on the server immediately
    if fs.runtime.block_on(put_file_content_to_server(&fs.client, &full_path, "".into(),  &fs.config.server_url)).is_err() {
        reply.error(EIO);
        return;
    }

    // 2. Generate new identifiers
    let inode = fs.next_inode;
    fs.next_inode += 1;
    let fh = fs.next_fh; // This is the handle for the write cache
    fs.next_fh += 1;

    // 3. Update internal maps
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path.clone(), inode);
    fs.inode_to_type.insert(inode, FileType::RegularFile);

    // 4. Create and store the in-memory write cache (buffer)
    let open_file = OpenWriteFile {
        path: full_path,
        buffer: HashMap::new(),
    };
    fs.open_files.insert(fh, open_file);

    // 5. Create and cache stub attributes
    let ts = SystemTime::now();
    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: ts, mtime: ts,
        ctime: ts, crtime: ts, kind: FileType::RegularFile,
        perm: mode as u16, nlink: 1, uid: req.uid(), gid: req.gid(), rdev: 0, flags: 0, blksize: 5120,
    };

    let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
    
    // CACHE IMMEDIATA: Salviamo il nuovo file
    fs.attribute_cache.put(inode, attrs.clone(), ttl);

    // INVALIDAZIONE PADRE: La cartella contenitore è cambiata
    fs.attribute_cache.remove(&parent);

    // 6. Reply to the kernel with the new file handle (fh)
    reply.created(&TTL, &attrs, 0, fh, 0);
}

/// Handles the FUSE `mkdir` operation (e.g., `mkdir my_dir`).
///
/// This function contacts the server's `/mkdir` endpoint via a `POST` request.
/// It then generates a new inode for the directory, updates the internal path mappings,
/// and caches a set of locally-generated attributes.
///
/// This operation does *not* use the `OpenWriteFile` cache, which is only for file I/O.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `parent` - The inode of the parent directory.
/// * `name` - The name of the directory to create.
/// * `reply` - The reply object to send the new entry's attributes back.
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

    // Contact the server to create the directory
    if fs.runtime.block_on(create_directory(&fs.client, &full_path, &fs.config.server_url)).is_err() {
        reply.error(EIO);
        return;
    }

    // Generate new inode and update maps
    let inode = fs.next_inode;
    fs.next_inode += 1;
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path, inode);
    fs.inode_to_type.insert(inode, FileType::Directory);

    // Create and cache stub attributes
    let ts = SystemTime::now();
    let attrs = FileAttr {
        ino: inode, 
        size: 4096, // CORRETTO: Dimensione standard directory Linux
        blocks: 8,  // 4096 / 512 = 8 blocchi
        atime: ts, mtime: ts,
        ctime: ts, crtime: ts, kind: FileType::Directory,
        perm: mode as u16, nlink: 2, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };

    let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
    
    // CACHE IMMEDIATA: Salviamo la nuova cartella con i dati corretti
    fs.attribute_cache.put(inode, attrs.clone(), ttl);

    // INVALIDAZIONE PADRE: La cartella contenitore è cambiata
    fs.attribute_cache.remove(&parent);

    // Reply with the new entry
    reply.entry(&TTL, &attrs, 0);
}
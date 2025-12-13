use super::prelude::*;

/// Handles the FUSE `rmdir` operation (e.g., `rmdir my_dir`).
///
/// This function does not delete the directory itself. It first performs a
/// check to ensure the directory is empty.
///
/// 1. It lists the directory's contents from the server.
/// 2. If the list is not empty, it replies with `ENOTEMPTY`.
/// 3. If the list is empty, it forwards the request to `unlink`, which
///    performs the actual deletion via the server's `DELETE` endpoint.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `req` - The FUSE request (unused here, passed to `unlink`).
/// * `parent` - The inode of the parent directory.
/// * `name` - The name of the directory to remove.
/// * `reply` - The reply object to send success or an error code.
pub fn rmdir(fs: &mut RemoteFS, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
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

    // Check if the directory is empty first
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &full_path,  &fs.config.server_url)) {
        Ok(list) => list,
        Err(_) => {
            reply.error(EIO);
            return;
        }
    };

    if !entry_list.is_empty() {
        reply.error(ENOTEMPTY);
        return;
    }

    // If empty, call `unlink` to do the actual deletion
    unlink(fs, req, parent, name, reply);
}

/// Handles the FUSE `unlink` operation (e.g., `rm file.txt`).
///
/// This function deletes both files and directories.
/// - If the target is a file, it sends a `DELETE /files/{path}` request.
/// - If the target is a directory, it delegates to `recursive_delete` to
///   remove all contents first.
///
/// After a successful deletion, it removes the inode and path from all
/// internal maps and invalidates the attribute cache.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `parent` - The inode of the parent directory.
/// * `name` - The name of the file or directory to remove.
/// * `reply` - The reply object to send success or an error code.
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

    let inode = match fs.path_to_inode.get(&full_path) {
        Some(&ino) => ino,
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let is_dir = fs.inode_to_type.get(&inode).copied() == Some(FileType::Directory);

    if is_dir {
        // Handle recursive deletion for directories
        if let Err(err) = recursive_delete(fs, &full_path) {
            reply.error(err);
            return;
        }
    } else {
        // Handle single file deletion
        if fs.runtime.block_on(delete_resource(&fs.client, &full_path, &fs.config.server_url)).is_err() {
            reply.error(EIO);
            return;
        }
    }

    // On success, clean up all internal state
    fs.attribute_cache.remove(&inode);
    fs.path_to_inode.remove(&full_path);
    fs.inode_to_path.remove(&inode);
    fs.inode_to_type.remove(&inode);

    reply.ok();
}

/// A private helper function to recursively delete a directory's contents.
///
/// This is called by `unlink` when it receives a request to delete a directory.
/// It lists all entries, deletes files, recurses into subdirectories, and
/// *after* all children are deleted, it deletes the (now empty) directory itself.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `path` - The relative path of the directory to delete.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(libc::c_int)` with an error code (e.g., `EIO`) on failure.
pub fn recursive_delete(fs: &mut RemoteFS, path: &str) -> Result<(), libc::c_int> {
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, path,  &fs.config.server_url)) {
        Ok(list) => list,
        Err(_) => return Err(libc::EIO),
    };

    // Delete all children first
    for entry in entry_list {
        let full_path = format!("{}/{}", path, entry.name);
        if entry.kind == "directory" {
            recursive_delete(fs, &full_path)?;
        } else {
            if fs.runtime.block_on(delete_resource(&fs.client, &full_path, &fs.config.server_url)).is_err() {
                return Err(libc::EIO);
            }
        }
    }

    // After children are gone, delete the directory itself
    if fs.runtime.block_on(delete_resource(&fs.client, path, &fs.config.server_url)).is_err() {
        return Err(libc::EIO);
    }

    Ok(())
}
use super::prelude::*;

/// A private helper function to recursively move a directory's contents.
///
/// This is a client-side implementation of `mv` that works by recursively
/// copying all contents to the new location and then deleting the old
/// location, using only the existing server endpoints.
///
/// This operation is NOT ATOMIC and can be slow for large directories.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `old_path` - The relative path of the source directory (e.g., "dir1").
/// * `new_path` - The relative path of the destination (e.g., "dir2").
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(libc::c_int)` with an error code (e.g., `EIO`) on failure.
fn recursive_move_client_side(
    fs: &mut RemoteFS,
    old_path: &str,
    new_path: &str,
) -> Result<(), libc::c_int> {

    // 1. Create the new destination directory
    let mkdir_url = format!("http://localhost:8080/mkdir/{}", new_path);
    if fs.runtime.block_on(fs.client.post(&mkdir_url).send()).is_err() {
        // This might fail if the dir already exists, but for a rename,
        // it should be a new path. We treat this as a critical error.
        return Err(EIO);
    }

    // 2. List the contents of the old directory
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, old_path)) {
        Ok(list) => list,
        Err(_) => return Err(EIO),
    };

    // 3. Move all children recursively
    for entry in entry_list {
        let old_child_path = format!("{}/{}", old_path, entry.name);
        let new_child_path = format!("{}/{}", new_path, entry.name);

        if entry.kind == "directory" {
            // Recursive call for subdirectories
            recursive_move_client_side(fs, &old_child_path, &new_child_path)?;
        } else {
            // "Copy + Delete" logic for files
            let content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &old_child_path)) {
                Ok(c) => c,
                Err(_) => return Err(ENOENT),
            };
            if fs.runtime.block_on(put_file_content_to_server(&fs.client, &new_child_path, content)).is_err() {
                return Err(EIO);
            }
            // Delete the old file after successful copy
            let delete_url = format!("http://localhost:8080/files/{}", old_child_path);
            if fs.runtime.block_on(fs.client.delete(&delete_url).send()).is_err() {
                return Err(EIO);
            }
        }
    }

    // 4. Delete the now-empty old directory
    let delete_url = format!("http://localhost:8080/files/{}", old_path);
    if fs.runtime.block_on(fs.client.delete(&delete_url).send()).is_err() {
        return Err(EIO);
    }

    Ok(())
}


/// Handles the FUSE `rename` operation (e.g., `mv old.txt dir/new.txt`).
///
/// This function implements the move logic entirely on the client side,
/// using only the existing server API endpoints.
///
/// # File Logic
/// 1. Fetches (`GET`) the content of the source file.
/// 2. Uploads (`PUT`) that content to the destination path.
/// 3. Deletes (`DELETE`) the source file.
///
/// # Directory Logic
/// 1. Delegates to the `recursive_move_client_side` helper function.
/// 2. This helper recursively creates the new directory structure,
///    moves all child files (using the file logic), and then
///    deletes the original directory structure.
///
/// # Warning
/// This operation is **NOT ATOMIC** and may be slow for large directories.
///
/// # Arguments
/// * `fs` - The mutable `RemoteFS` state.
/// * `parent` - The inode of the source directory.
/// * `name` - The name of the source file/directory.
/// * `newparent` - The inode of the destination directory.
/// * `newname` - The new name for the file/directory.
/// * `reply` - The reply object to send success or an error code.
pub fn rename(fs: &mut RemoteFS, _req: &Request<'_>, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, _flags: u32, reply: ReplyEmpty) {
    let old_parent_path = match fs.inode_to_path.get(&parent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };
    let new_parent_path = match fs.inode_to_path.get(&newparent) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let old_name = name.to_str().unwrap();
    let new_name = newname.to_str().unwrap();

    let old_full_path = if old_parent_path.is_empty() {
        old_name.to_string()
    } else {
        format!("{}/{}", old_parent_path, old_name)
    };

    let new_full_path = if new_parent_path.is_empty() {
        new_name.to_string()
    } else {
        format!("{}/{}", new_parent_path, new_name)
    };

    let inode = match fs.path_to_inode.get(&old_full_path) {
        Some(&ino) => ino,
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let is_dir = fs.inode_to_type.get(&inode).copied() == Some(FileType::Directory);

    // --- LOGIC DISPATCH ---
    if is_dir {
        // Use the new recursive helper function for directories
        match recursive_move_client_side(fs, &old_full_path, &new_full_path) {
            Ok(_) => { /* Success, continue to cache update */ },
            Err(e) => {
                reply.error(e); // Return the specific error (e.g., EIO)
                return;
            }
        }
    } else {
        // Use the original "Copy + Delete" logic for files
        let content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &old_full_path)) {
            Ok(c) => c,
            Err(_) => { reply.error(ENOENT); return; }
        };
        if fs.runtime.block_on(put_file_content_to_server(&fs.client, &new_full_path, content)).is_err() {
            reply.error(EIO);
            return;
        }
        // Delete the old file
        if fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        }).is_err() {
            reply.error(EIO);
            return;
        }
    }
    // --- END LOGIC DISPATCH ---

    // Update internal caches (this logic is correct)
    if let Some(&inode) = fs.path_to_inode.get(&old_full_path) {
        fs.attribute_cache.remove(&inode);
        fs.path_to_inode.remove(&old_full_path);
        fs.path_to_inode.insert(new_full_path.clone(), inode);
        fs.inode_to_path.insert(inode, new_full_path);
    }
    // Invalidate parent directory caches
    if let Some(&inode_parent) = fs.path_to_inode.get(&old_parent_path) {
        fs.attribute_cache.remove(&inode_parent);
    }
    if let Some(&inode_newparent) = fs.path_to_inode.get(&new_parent_path) {
        fs.attribute_cache.remove(&inode_newparent);
    }

    reply.ok();
}
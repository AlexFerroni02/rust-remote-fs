use super::prelude::*;

/// Handles the FUSE `rename` operation (e.g., `mv old.txt dir/new.txt`).
///
/// This function implements the move operation by performing a "Copy + Delete"
/// strategy on the remote server for files.
///
/// # File Logic
/// 1. Fetches the *entire* content of the source file (`old_full_path`).
/// 2. Writes that content to the *new* file path (`new_full_path`).
/// 3. Deletes the *old* file from the server.
///
/// # Directory Logic (Simplified)
/// The logic for renaming directories is currently simplified. It only sends a
/// `DELETE` request for the *source* directory and does not create the
/// destination directory or move its contents.
///
/// # Internal State
/// After a successful server operation, this function updates the internal
/// `path_to_inode` and `inode_to_path` maps to reflect the new path,
/// reusing the existing inode. It also invalidates the attribute cache
/// for the moved item and its parent directories.
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

    if is_dir {
        // Simplified logic for directories
        let result = fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        });
        if result.is_err() {
            reply.error(EIO);
            return;
        }
    } else {
        // "Copy + Delete" logic for files
        let content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &old_full_path)) {
            Ok(c) => c,
            Err(_) => { reply.error(ENOENT); return; }
        };
        if fs.runtime.block_on(put_file_content_to_server(&fs.client, &new_full_path, content)).is_err() {
            reply.error(EIO);
            return;
        }

        // Delete the old file (this was the line with the port 88 bug)
        if fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        }).is_err() {
            reply.error(EIO);
            return;
        }
    }

    // Update internal caches to reflect the move
    if let Some(&inode) = fs.path_to_inode.get(&old_full_path) {
        fs.attribute_cache.remove(&inode);
        fs.path_to_inode.remove(&old_full_path);
        fs.path_to_inode.insert(new_full_path.clone(), inode);
        fs.inode_to_path.insert(inode, new_full_path);
    }
    // Invalidate parent directory caches
    if is_dir {
        if let Some(&inode_parent) = fs.path_to_inode.get(&old_parent_path) {
            fs.attribute_cache.remove(&inode_parent);
        }
        if let Some(&inode_newparent) = fs.path_to_inode.get(&new_parent_path) {
            fs.attribute_cache.remove(&inode_newparent);
        }
    }

    reply.ok();
}
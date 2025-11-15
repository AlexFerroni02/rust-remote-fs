use super::prelude::*;
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

    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &full_path)) {
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

    unlink(fs, req, parent, name, reply);
}

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
        if let Err(err) = recursive_delete(fs, &full_path) {
            reply.error(err);
            return;
        }
    } else {
        let url = format!("http://localhost:8080/files/{}", full_path);
        if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
            reply.error(EIO);
            return;
        }
    }

    fs.attribute_cache.remove(&inode);
    fs.path_to_inode.remove(&full_path);
    fs.inode_to_path.remove(&inode);
    fs.inode_to_type.remove(&inode);

    reply.ok();
}

pub fn recursive_delete(fs: &mut RemoteFS, path: &str) -> Result<(), libc::c_int> {
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, path)) {
        Ok(list) => list,
        Err(_) => return Err(libc::EIO),
    };

    for entry in entry_list {
        let full_path = format!("{}/{}", path, entry.name);
        if entry.kind == "directory" {
            recursive_delete(fs, &full_path)?;
        } else {
            let url = format!("http://localhost:8080/files/{}", full_path);
            if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
                return Err(libc::EIO);
            }
        }
    }

    let url = format!("http://localhost:8080/files/{}", path);
    if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
        return Err(libc::EIO);
    }

    Ok(())
}
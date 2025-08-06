use fuser::{FileAttr, FileType, ReplyCreate, ReplyWrite, ReplyEntry, Request, ReplyAttr, ReplyEmpty};
use libc::{ENOENT, EIO,ENOTEMPTY};
use std::ffi::OsStr;
use std::time::UNIX_EPOCH;
use crate::api_client::{put_file_content_to_server, get_file_content_from_server, get_files_from_server};
use super::{RemoteFS, TTL};

pub fn write(fs: &mut RemoteFS, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
    let file_path = match fs.inode_to_path.get(&ino) {
        Some(p) => p.clone(),
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let old_content_result = fs.runtime.block_on(async {
        get_file_content_from_server(&fs.client, &file_path).await
    });

    let old_content = match old_content_result {
        Ok(c) => c,
        Err(_) if offset == 0 => "".to_string(),
        Err(_) => {
            reply.error(EIO);
            return;
        }
    };

    let old_bytes = old_content.as_bytes();
    let offset = offset as usize;

    let final_capacity = std::cmp::max(offset + data.len(), old_bytes.len());
    let mut new_content = Vec::with_capacity(final_capacity);

    let prefix_len = std::cmp::min(offset, old_bytes.len());
    new_content.extend_from_slice(&old_bytes[..prefix_len]);

    if new_content.len() < offset {
        new_content.resize(offset, 0);
    }

    new_content.extend_from_slice(data);


    let end_of_write = offset + data.len();
    if offset > 0 && old_bytes.len() > end_of_write {
        new_content.extend_from_slice(&old_bytes[end_of_write..]);
    }

    match String::from_utf8(new_content) {
        Ok(content_str) => {
            let res = fs.runtime.block_on(async {
                put_file_content_to_server(&fs.client, &file_path, &content_str).await
            });

            match res {
                Ok(_) => reply.written(data.len() as u32),
                Err(_) => reply.error(EIO),
            }
        },
        Err(_) => reply.error(EIO),
    }
}

pub fn create(fs: &mut RemoteFS, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, _umask: u32, _flags: i32, reply: ReplyCreate) {
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

    if fs.runtime.block_on(put_file_content_to_server(&fs.client, &full_path, "")).is_err() {
        reply.error(EIO);
        return;
    }

    let inode = fs.next_inode;
    fs.next_inode += 1;
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path, inode);
    fs.inode_to_type.insert(inode, FileType::RegularFile);

    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, kind: FileType::RegularFile,
        perm: mode as u16, nlink: 1, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };

    fs.inode_to_attr.insert(inode, attrs.clone());

    reply.created(&TTL, &attrs, 0, inode, 0);
}

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

    if fs.runtime.block_on(async {
        let url = format!("http://localhost:8080/mkdir/{}", full_path);
        fs.client.post(&url).send().await
    }).is_err() {
        reply.error(EIO);
        return;
    }

    let inode = fs.next_inode;
    fs.next_inode += 1;
    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path, inode);
    fs.inode_to_type.insert(inode, FileType::Directory);

    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH, crtime: UNIX_EPOCH, kind: FileType::Directory,
        perm: mode as u16, nlink: 2, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };
    reply.entry(&TTL, &attrs, 0);
}

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

    // Check if the directory is empty
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, &full_path)) {
        Ok(list) => list,
        Err(_) => {
            reply.error(EIO);
            return;
        }
    };

    if !entry_list.is_empty() {
        reply.error(ENOTEMPTY); // Return error if the directory is not empty
        return;
    }

    // Proceed with unlinking the directory
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

    // Check if the path exists and determine its type
    let inode = match fs.path_to_inode.get(&full_path) {
        Some(&ino) => ino,
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let is_dir = fs.inode_to_type.get(&inode).copied() == Some(FileType::Directory);

    if is_dir {
        // Perform recursive deletion for directories
        if let Err(err) = recursive_delete(fs, &full_path) {
            reply.error(err);
            return;
        }
    } else {
        // Delete file directly
        let url = format!("http://localhost:8080/files/{}", full_path);
        if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
            reply.error(EIO);
            return;
        }
    }
    // Remove inode mappings
    fs.path_to_inode.remove(&full_path);
    fs.inode_to_path.remove(&inode);
    fs.inode_to_type.remove(&inode);

    reply.ok();
}
pub fn release(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
    reply.ok();
}

pub fn flush(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
    reply.ok();
}
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

    // Trova l'inode associato al vecchio path per determinare il tipo (file/directory)
    let inode = match fs.path_to_inode.get(&old_full_path) {
        Some(&ino) => ino,
        None => {
            reply.error(ENOENT);
            return;
        }
    };

    let is_dir = fs.inode_to_type.get(&inode).copied() == Some(FileType::Directory);

    if is_dir {
        let result = fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        });
        if result.is_err() {
            reply.error(EIO);
            return;
        }

    } else {
        let content_result = fs.runtime.block_on(async {
            get_file_content_from_server(&fs.client, &old_full_path).await
        });

        let content = match content_result {
            Ok(c) => c,
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };
        let write_result = fs.runtime.block_on(async {
            put_file_content_to_server(&fs.client, &new_full_path, &content).await
        });

        if write_result.is_err() {
            reply.error(EIO);
            return;
        }

        // 3. Cancella il file originale.
        let delete_result = fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        });

        if delete_result.is_err() {
            reply.error(EIO);
            return;
        }
    }

    if let Some(&inode) = fs.path_to_inode.get(&old_full_path) {
        fs.path_to_inode.remove(&old_full_path);
        fs.path_to_inode.insert(new_full_path.clone(), inode);
        fs.inode_to_path.insert(inode, new_full_path);
    }

    // Per le directory, dobbiamo anche invalidare la cache
    if is_dir {
        if let Some(&inode_parent) = fs.path_to_inode.get(&old_parent_path) {
            // rimuovi la entry dalla cache
            fs.inode_to_path.remove(&inode_parent);
            fs.inode_to_type.remove(&inode_parent);
            fs.path_to_inode.remove(&old_parent_path);
        }
        if let Some(&inode_newparent) = fs.path_to_inode.get(&new_parent_path) {
            // rimuovi la entry dalla cache
            fs.inode_to_path.remove(&inode_newparent);
            fs.inode_to_type.remove(&inode_newparent);
            fs.path_to_inode.remove(&new_parent_path);
        }
    }

    reply.ok();
}
pub fn recursive_delete(fs: &mut RemoteFS, path: &str) -> Result<(), libc::c_int> {
    // Get the list of entries in the directory
    let entry_list = match fs.runtime.block_on(get_files_from_server(&fs.client, path)) {
        Ok(list) => list,
        Err(_) => return Err(libc::EIO),
    };

    for entry in entry_list {
        let full_path = format!("{}/{}", path, entry.name);
        if entry.kind == "directory" {
            // Recursively delete subdirectory
            recursive_delete(fs, &full_path)?;
        } else {
            // Delete file
            let url = format!("http://localhost:8080/files/{}", full_path);
            if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
                return Err(libc::EIO);
            }
        }
    }

    // Delete the directory itself
    let url = format!("http://localhost:8080/files/{}", path);
    if fs.runtime.block_on(fs.client.delete(&url).send()).is_err() {
        return Err(libc::EIO);
    }

    Ok(())
}
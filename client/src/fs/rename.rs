use super::prelude::*;
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
        // Logica per le directory (semplificata)
        let result = fs.runtime.block_on(async {
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        });
        if result.is_err() {
            reply.error(EIO);
            return;
        }
    } else {
        // Logica per i file (Copia + Cancella)
        let content = match fs.runtime.block_on(get_file_content_from_server(&fs.client, &old_full_path)) {
            Ok(c) => c,
            Err(_) => { reply.error(ENOENT); return; }
        };
        if fs.runtime.block_on(put_file_content_to_server(&fs.client, &new_full_path, content)).is_err() {
            reply.error(EIO);
            return;
        }

        // --- QUESTA ERA LA RIGA SBAGLIATA ---
        if fs.runtime.block_on(async {
            // Corretto: :8080
            let url = format!("http://localhost:8080/files/{}", old_full_path);
            fs.client.delete(&url).send().await
        }).is_err() {
            reply.error(EIO);
            return;
        }
    }

    // Aggiorna le cache interne
    if let Some(&inode) = fs.path_to_inode.get(&old_full_path) {
        fs.attribute_cache.remove(&inode);
        fs.path_to_inode.remove(&old_full_path);
        fs.path_to_inode.insert(new_full_path.clone(), inode);
        fs.inode_to_path.insert(inode, new_full_path);
    }
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
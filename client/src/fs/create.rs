use super::prelude::*;
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

    if fs.runtime.block_on(put_file_content_to_server(&fs.client, &full_path, "".into())).is_err() {
        reply.error(EIO);
        return;
    }

    let inode = fs.next_inode;
    fs.next_inode += 1;
    let fh = fs.next_fh;
    fs.next_fh += 1;

    fs.inode_to_path.insert(inode, full_path.clone());
    fs.path_to_inode.insert(full_path.clone(), inode);
    fs.inode_to_type.insert(inode, FileType::RegularFile);

    let open_file = OpenWriteFile {
        path: full_path,
        buffer: HashMap::new(),
    };
    fs.open_files.insert(fh, open_file);

    let attrs = FileAttr {
        ino: inode, size: 0, blocks: 0, atime: SystemTime::now(), mtime: SystemTime::now(),
        ctime: SystemTime::now(), crtime: SystemTime::now(), kind: FileType::RegularFile,
        perm: mode as u16, nlink: 1, uid: req.uid(), gid: req.gid(), rdev: 0, flags: 0, blksize: 5120,
    };

    let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
    fs.attribute_cache.put(inode, attrs.clone(), ttl);

    reply.created(&TTL, &attrs, 0, fh, 0);
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
        ino: inode, size: 0, blocks: 0, atime: SystemTime::now(), mtime: SystemTime::now(),
        ctime: SystemTime::now(), crtime: SystemTime::now(), kind: FileType::Directory,
        perm: mode as u16, nlink: 2, uid: 501, gid: 20, rdev: 0, flags: 0, blksize: 5120,
    };

    let ttl = Duration::from_secs(fs.config.cache_ttl_seconds);
    fs.attribute_cache.put(inode, attrs.clone(), ttl);

    reply.entry(&TTL, &attrs, 0);
}

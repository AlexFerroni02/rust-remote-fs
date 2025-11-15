use super::prelude::*;

pub fn write(
    fs: &mut RemoteFS,
    _req: &Request<'_>,
    _ino: u64,
    fh: u64,
    offset: i64,
    data: &[u8],
    _write_flags: u32,
    _flags: i32,
    _lock_owner: Option<u64>,
    reply: ReplyWrite,
) {
    if let Some(open_file) = fs.open_files.get_mut(&fh) {
        open_file.buffer.insert(offset, data.to_vec());
        reply.written(data.len() as u32);
    } else {
        reply.error(EBADF);
    }
}


pub fn release(
    fs: &mut RemoteFS,
    _req: &Request<'_>,
    ino: u64,
    fh: u64,
    _flags: i32,
    _lock_owner: Option<u64>,
    _flush: bool,
    reply: ReplyEmpty,
) {
    if let Some(open_file) = fs.open_files.remove(&fh) {

        if open_file.buffer.is_empty() {
            reply.ok();
            return;
        }

        // 1. Scarica il contenuto attuale
        let old_content_result = fs.runtime.block_on(
            api_client::get_file_content_from_server(&fs.client, &open_file.path)
        );

        let mut new_data_vec = match old_content_result {
            Ok(bytes) => bytes.to_vec(),
            Err(_) => Vec::new(),
        };

        // 2. Applica le modifiche dalla cache
        for (offset, data) in open_file.buffer {
            let start = offset as usize;
            let end = start + data.len();
            if end > new_data_vec.len() {
                new_data_vec.resize(end, 0);
            }
            new_data_vec[start..end].copy_from_slice(&data);
        }

        // 3. Esegui UN SOLO UPLOAD
        let put_result = fs.runtime.block_on(
            api_client::put_file_content_to_server(
                &fs.client,
                &open_file.path,
                Bytes::from(new_data_vec)
            )
        );

        match put_result {
            Ok(_) => {
                fs.attribute_cache.remove(&ino);
                reply.ok();
            }
            Err(e) => {
                eprintln!("[FUSE CLIENT] Errore critico during PUT in release: {:?}", e);
                reply.error(EIO);
            }
        }
    } else {
        reply.ok();
    }
}

pub fn flush(_fs: &mut RemoteFS, _req: &Request<'_>, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
    reply.ok();
}

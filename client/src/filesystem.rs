use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, ReplyWrite, Request,
};
use libc::ENOENT;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

// Importa le funzioni dal nostro modulo api_client
// La keyword `crate` si riferisce alla radice del nostro progetto
use crate::api_client::{
    get_file_content_from_server, get_files_from_server, put_file_content_to_server,
};

// --- Costanti ---
const TTL: Duration = Duration::from_secs(1);
const ROOT_DIR_ATTR: FileAttr = FileAttr {
    ino: 1, size: 0, blocks: 0, atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH, kind: FileType::Directory, perm: 0o755, nlink: 2, uid: 501, gid: 20,
    rdev: 0, flags: 0, blksize: 5120,
};

// --- Struct Principale ---
// Per ora, non contiene la mappa.
pub struct RemoteFS {
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    inode_to_path: HashMap<u64, String>,
    path_to_inode: HashMap<String, u64>,
    inode_to_type: HashMap<u64, FileType>,
    next_inode: u64,
    
}

impl RemoteFS {
    // Costruttore pubblico per creare l'istanza in main.rs
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let mut inode_to_path = HashMap::new();
        let mut path_to_inode = HashMap::new();
        let mut inode_to_type = HashMap::new();
        // Initialize the root
        inode_to_path.insert(1, "".to_string());
        path_to_inode.insert("".to_string(), 1);
        inode_to_type.insert(1, FileType::Directory);
        Self {
            client: reqwest::Client::new(),
            runtime,
            inode_to_path,
            path_to_inode,
            inode_to_type,
            next_inode: 2, 
        }
    }
}


// --- Implementazione del Trait Filesystem ---
impl Filesystem for RemoteFS {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("📋 GETATTR: ino={}", ino);
        if ino == 1 {
            reply.attr(&TTL, &ROOT_DIR_ATTR);
            return;
        }
        
        // Cerca il path nella mappa
        if let Some(path) = self.inode_to_path.get(&ino) {
            // Per ora, restituisci attributi fittizi
            // In futuro, potresti chiedere al server gli attributi reali
            let kind = self.inode_to_type.get(&ino).copied().unwrap_or(FileType::RegularFile);
            println!("📋 GETATTR: path='{}', kind={:?}", path, kind);
            let attrs = FileAttr {
                ino,
                size: 1024,
                blocks: 1,
                kind,
                perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
                nlink: 1,
                uid: 501,
                gid: 20,
                atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
            };
            reply.attr(&TTL, &attrs);
        } else {
            reply.error(ENOENT);
        }
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
            // 1. Ricava il path della directory padre
        let parent_path = match self.inode_to_path.get(&parent) {
            Some(p) => p.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let name_str = name.to_str().unwrap_or("");
        println!("🔍 LOOKUP: parent={}, name='{}'", parent, name_str);
        // 2. Costruisci il path completo
        let full_path = if parent_path.is_empty() {
            name_str.to_string()
        } else {
            format!("{}/{}", parent_path, name_str)
        };

        // 3. Se già in mappa, restituisci subito
        if let Some(&inode) = self.path_to_inode.get(&full_path) {
            println!("Client: Found cached inode {} for path '{}'", inode, full_path);
            let kind = self.inode_to_type.get(&inode).copied().unwrap_or(FileType::RegularFile);
            
            let attrs = FileAttr {
                ino: inode,
                size: 1024,
                blocks: 1,
                kind,
                perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
                nlink: 1,
                uid: 501,
                gid: 20,
                atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
            };
            reply.entry(&TTL, &attrs, 0);
            return;
        }

        // 4. Chiedi al server la lista della directory padre
        println!("Client: Querying server for parent path '{}'", parent_path);
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client, &parent_path).await
        });

        if let Ok(files) = file_list {
            // 5. Cerca il file/directory richiesto
            if let Some(found_file) = files.iter().find(|f| {
                f.trim_end_matches('/') == name_str
            }) {
                // 6. Assegna un nuovo inode
                let inode = self.next_inode;
                self.next_inode += 1;
                
                let is_dir = found_file.ends_with('/');
                let kind = if is_dir { FileType::Directory } else { FileType::RegularFile };
                println!("📁 LOOKUP: Found '{}', is_dir={}, kind={:?}", found_file, is_dir, kind);
                // 7. Aggiorna le mappe
                self.inode_to_path.insert(inode, full_path.clone());
                self.path_to_inode.insert(full_path.clone(), inode);
                self.inode_to_type.insert(inode, kind);

                
                
                let attrs = FileAttr {
                    ino: inode,
                    size: 1024,
                    blocks: 1,
                    kind,
                    perm: if kind == FileType::Directory { 0o755 } else { 0o644 },
                    nlink: 1,
                    uid: 501,
                    gid: 20,
                    atime: UNIX_EPOCH, mtime: UNIX_EPOCH, ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH, rdev: 0, flags: 0, blksize: 5120,
                };
                
                println!("Client: Created new inode {} for path '{}'", inode, full_path);
                reply.entry(&TTL, &attrs, 0);
            } else {
                println!("Client: File '{}' not found in parent '{}'", name_str, parent_path);
                reply.error(ENOENT);
            }
        } else {
            println!("Client: Failed to get file list for parent '{}'", parent_path);
            reply.error(ENOENT);
        }
    }
    
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
            // 1. Ricava il path dalla mappa
        let dir_path = match self.inode_to_path.get(&ino) {
            Some(p) => p.clone(),
            None => {
                println!("Client: Unknown inode {} in readdir", ino);
                reply.error(ENOENT);
                return;
            }
        };

        println!("READDIRE Client: Reading directory '{}'", dir_path);

        // 2. Chiedi al server la lista dei file
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client, &dir_path).await
        });

        let mut entries = vec![
            (ino, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()), // Parent sempre root per ora
        ];

        if let Ok(files) = file_list {
            for file_name in files {
                let is_dir = file_name.ends_with('/');
                let clean_name = file_name.trim_end_matches('/').to_string();
                
                // 3. Costruisci il path completo
                let full_path = if dir_path.is_empty() {
                    clean_name.clone()
                } else {
                    format!("{}/{}", dir_path, clean_name)
                };

                // 4. Trova o crea inode
                let inode = if let Some(&existing_ino) = self.path_to_inode.get(&full_path) {
                    existing_ino
                } else {
                    let new_ino = self.next_inode;
                    self.next_inode += 1;
                    self.inode_to_path.insert(new_ino, full_path.clone());
                    self.path_to_inode.insert(full_path, new_ino);
                    new_ino
                };

                let kind = if is_dir { FileType::Directory } else { FileType::RegularFile };
                self.inode_to_type.insert(inode, kind);
                entries.push((inode, kind, clean_name));
            }
        } else {
            println!("Client: Failed to get file list for '{}'", dir_path);
        }

        // 5. Restituisci le entries
        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
                break;
            }
        }
        reply.ok();
    }

    fn read(&mut self,_req: &Request<'_>,ino: u64,fh: u64,offset: i64,size: u32,flags: i32,lock_owner: Option<u64>,reply: fuser::ReplyData,) {
        let dir_path= "";
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client,dir_path ).await
        });

        let filename = if let Ok(files) = file_list {
            files.get((ino - 2) as usize).cloned()
        } else {
            None
        };

        if let Some(name) = filename {
            let content = self.runtime.block_on(async {
                get_file_content_from_server(&self.client, &name).await
            }).unwrap_or_default();

            let start = offset as usize;
            let end = std::cmp::min(start + size as usize, content.len());
            reply.data(&content.as_bytes()[start..end]);
        } else {
            reply.error(ENOENT);
        }
    }
    
    fn open(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
        // Per questo filesystem semplice, se non è la root directory (ino > 1),
        // consideriamo il file "apribile".
        // Usiamo l'inode stesso come file handle (fh), è una pratica comune.
        // Il secondo parametro sono le flags di apertura, 0 va bene per ora.
        reply.opened(ino, 0);
    }
    
    fn create(&mut self,_req: &Request<'_>,parent: u64,name: &OsStr,mode: u32,umask: u32,flags: i32,reply: fuser::ReplyCreate,) {
        let filename = name.to_str().unwrap().to_string();
        println!("Client: Received CREATE request for {}", filename);

        // 1. Chiediamo al server di creare un file vuoto.
        //    La nostra funzione `put` sul server già crea il file se non esiste.
        let create_res = self.runtime.block_on(async {
            put_file_content_to_server(&self.client, &filename, "").await
        });

        if create_res.is_err() {
            reply.error(ENOENT); // Errore durante la creazione sul server
            return;
        }

        // 2. Ora che il file esiste sul server, recuperiamo la lista aggiornata
        //    per trovare il suo nuovo inode e gli attributi.
        let dir_path= "";
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client,dir_path ).await
        });

        if let Ok(files) = file_list {
            if let Some((i, _)) = files.iter().enumerate().find(|(_, s)| *s == &filename) {
                let inode = i as u64 + 2;
                let attrs = FileAttr {
                    ino: inode,
                    size: 0, // Il file è nuovo e vuoto
                    blocks: 0,
                    atime: UNIX_EPOCH,
                    mtime: UNIX_EPOCH,
                    ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH,
                    kind: FileType::RegularFile,
                    perm: mode as u16, // Usiamo i permessi richiesti dalla chiamata `create`
                    nlink: 1,
                    uid: 501, // Dovresti usare l'uid dell'utente che esegue il comando
                    gid: 20,  // e il suo gid
                    rdev: 0,
                    flags: 0,
                    blksize: 5120,
                };
                // 3. Rispondiamo con successo, fornendo gli attributi e il file handle.
                println!("Client: File {} created successfully with inode {}", filename, inode);
                reply.created(&TTL, &attrs, 0, inode, 0);
            } else {
                // Non dovrebbe succedere se il PUT ha funzionato
                reply.error(ENOENT);
            }
        } else {
            reply.error(ENOENT);
        }
    }
    
    fn setattr(&mut self,_req: &Request<'_>, ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        println!("SETATTR called for inode {}, new size: {:?}", ino, size);

        // In una implementazione completa, dovresti:
        // 1. Trovare il nome del file dall'inode.
        // 2. Chiamare un nuovo endpoint sul server per cambiare gli attributi (es. troncare il file).
        // 3. Il server dovrebbe rispondere con i nuovi attributi aggiornati.
        // 4. Usare quegli attributi nella risposta qui sotto.

        // Per ora, facciamo finta che abbia funzionato e rispondiamo con attributi fittizi.
        // Questo è sufficiente a sbloccare l'operazione di scrittura.
        let dummy_attrs = FileAttr {
            ino,
            size: size.unwrap_or(1024), // Usa la nuova dimensione se fornita
            blocks: 1,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: 501,
            gid: 20,
            rdev: 0,
            flags: 0,
            blksize: 5120,
        };
        reply.attr(&TTL, &dummy_attrs);
    }

    fn write(&mut self,_req: &Request<'_>,ino: u64,fh: u64,offset: i64,data: &[u8],write_flags: u32,flags: i32,lock_owner: Option<u64>,reply: fuser::ReplyWrite,) {
        // Ricava il nome del file dall'inode
        let dir_path= "";
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client,dir_path ).await
        });

        let filename = if let Ok(files) = file_list {
            files.get((ino - 2) as usize).cloned()
        } else {
            None
        };

        if let Some(name) = filename {
            // Per semplicità, sovrascriviamo tutto il file (offset ignorato)
            let content = String::from_utf8_lossy(data).to_string();
            let res = self.runtime.block_on(async {
                put_file_content_to_server(&self.client, &name, &content).await
            });

            match res {
                Ok(_) => reply.written(data.len() as u32),
                Err(_) => reply.error(ENOENT),
            }
        } else {
            reply.error(ENOENT);
        }
    }
    
}
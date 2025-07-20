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
}

impl RemoteFS {
    // Costruttore pubblico per creare l'istanza in main.rs
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        Self {
            client: reqwest::Client::new(),
            runtime,
        }
    }
}

// --- Implementazione del Trait Filesystem ---
impl Filesystem for RemoteFS {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            reply.attr(&TTL, &ROOT_DIR_ATTR);
        } else {
            // Per ora, non conosciamo altri inode, quindi restituiamo un errore.
            // `lookup` e `readdir` si occuperanno di fornire gli attributi per gli altri file.
            reply.error(ENOENT);
        }
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        // Logica semplificata che funziona solo per la root (parent == 1)
        if parent != 1 {
            reply.error(ENOENT);
            return;
        }

        let filename_to_find = name.to_str().unwrap_or("");
        
        // Chiede sempre e solo la lista di file della root
        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client, "").await
        });

        if let Ok(files) = file_list {
            if let Some((i, file_name)) = files.iter().enumerate().find(|(_, s)| s.as_str() == filename_to_find) {
                
                // Logica fragile di generazione inode: da migliorare con la mappa
                let ino = i as u64 + 2; 
                let kind = if file_name.ends_with('/') { FileType::Directory } else { FileType::RegularFile };

                let attrs = FileAttr {
                    ino,
                    size: 1024, // Fittizio
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
            } else {
                reply.error(ENOENT);
            }
        } else {
            reply.error(ENOENT);
        }
    }
    
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        // Ignora l'inode e chiede sempre la root: questo è il punto da migliorare.
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let file_list = self.runtime.block_on(async {
            get_files_from_server(&self.client, "").await
        });

        let mut entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        if let Ok(files) = file_list {
            for (i, mut file_name) in files.into_iter().enumerate() {
                let inode = i as u64 + 2; // Logica fragile: da migliorare
                let kind = if file_name.ends_with('/') {
                    file_name.pop();
                    FileType::Directory
                } else {
                    FileType::RegularFile
                };
                entries.push((inode, kind, file_name));
            }
        } else if let Err(e) = file_list {
            eprintln!("Error fetching files from server: {}", e);
        }

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
    // Incolla qui le tue funzioni `read`, `open`, `create`, `setattr`, `write`...
    // Assicurati che usino una logica simile a quella qui sopra, ovvero:
    // 1. Chiamare `get_files_from_server` per avere la lista.
    // 2. Usare la logica `files.get((ino - 2) as usize)` per "indovinare" il nome del file.
    // Questo è un approccio fragile ma è quello che avevi nel tuo codice originale e
    // ti permette di avere una base funzionante prima di introdurre la mappa.
}
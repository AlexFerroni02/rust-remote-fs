//! This is the main entry point for the FUSE client.
//!
//! This binary is responsible for:
//! 1. Loading the configuration from `config.toml`.
//! 2. Parsing the mountpoint from command-line arguments.
//! 3. Creating an instance of the `RemoteFS` filesystem.
//! 4. Mounting the filesystem at the specified mountpoint.

// Make the API client public so the `fs` module can access it.
pub mod api_client;
mod config;
mod fs;

use fs::{RemoteFS, FsWrapper};
use fuser::MountOption;
use std::env;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use futures_util::StreamExt;
use clap::Parser;
use crate::config::CacheStrategy;
// NOTA: Non usiamo #[tokio::main] qui perché FUSE deve girare su un thread sincrono,
// mentre block_on verrebbe chiamato all'interno di un contesto async, causando il panico.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Il punto di mount per il filesystem.
    mountpoint: String,

    /// Sovrascrive la strategia di cache (ttl, lru, none).
    #[arg(long, value_enum)]
    cache_strategy: Option<CacheStrategy>,

    /// Sovrascrive il TTL della cache in secondi (usato con --cache-strategy=ttl).
    #[arg(long)]
    cache_ttl_seconds: Option<u64>,

    /// Sovrascrive la capacità della cache LRU (usato con --cache-strategy=lru).
    #[arg(long)]
    cache_lru_capacity: Option<usize>,
}
fn main() {
    // 1. Leggi gli argomenti da riga di comando
    let cli = Cli::parse();

    // 2. Carica la configurazione di base dal file config.toml
    let mut config = config::load_config();
    println!("Configurazione da file: {:?}", config);

    // 3. Sovrascrivi i valori con gli argomenti della CLI, se forniti
    if let Some(strategy) = cli.cache_strategy {
        config.cache_strategy = strategy;
        println!("INFO: Strategia cache sovrascritta da CLI: {:?}", strategy);
    }
    if let Some(ttl) = cli.cache_ttl_seconds {
        config.cache_ttl_seconds = ttl;
        println!("INFO: TTL cache sovrascritto da CLI: {}s", ttl);
    }
    if let Some(capacity) = cli.cache_lru_capacity {
        config.cache_lru_capacity = capacity;
        println!("INFO: Capacità LRU sovrascritta da CLI: {}", capacity);
    }
    
    println!("Configurazione finale: {:?}", config);

    // 4. Prendi il mountpoint dalla CLI
    let mountpoint = std::ffi::OsString::from(cli.mountpoint);

    // 5. Crea l'istanza di RemoteFS con la configurazione finale
    let fs_inner = RemoteFS::new(config.clone());
    let fs_wrapper = FsWrapper(Arc::new(Mutex::new(fs_inner)));

    // 6. Avvia il watcher in un thread separato
    let fs_clone_for_watcher = fs_wrapper.0.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            connect_and_watch(fs_clone_for_watcher).await;
        });
    });

    // 7. Monta il filesystem (bloccante)
    let filesystem = fs_wrapper;
    let options = vec![
        MountOption::AutoUnmount,
        MountOption::FSName("remoteFS".to_string()),
    ];
    println!("Mounting filesystem at {:?}", mountpoint);
    if let Err(e) = fuser::mount2(filesystem, &mountpoint, &options) {
        eprintln!("Failed to mount filesystem: {}", e);
    }
}

async fn connect_and_watch(fs_arc: Arc<Mutex<RemoteFS>>) {
    let url_str = "ws://localhost:8080/ws";
    let url = Url::parse(url_str).expect("URL WebSocket non valido");
    
    // Recuperiamo il nostro ID client per filtrare i messaggi
    let my_client_id = {
        let fs = fs_arc.lock().unwrap();
        fs.client_id.clone()
    };
    println!("[WATCHER_CLIENT] Il mio Client ID è: {}", my_client_id);

    println!("[WATCHER_CLIENT] Avvio loop di connessione verso {}", url_str);

    loop {
        match connect_async(url.clone()).await {
            Ok((ws_stream, _)) => {
                println!("[WATCHER_CLIENT] Connesso al watcher del server.");
                let (_, mut read) = ws_stream.split();

                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            println!("[WATCHER_CLIENT] Ricevuta notifica: {}", text);
                            
                            // --- LOGICA ECHO SUPPRESSION ---
                            let (clean_text, sender_id) = if let Some((msg, id)) = text.rsplit_once("|BY:") {
                                (msg, Some(id))
                            } else {
                                (text.as_str(), None)
                            };

                            if let Some(id) = sender_id {
                                if id == my_client_id {
                                    println!("[WATCHER_CLIENT] Ignoro notifica (Echo Suppression): modifica fatta da me.");
                                    continue;
                                }
                            }
                            // -------------------------------

                            if let Some(path_str) = clean_text.strip_prefix("CHANGE:") {
                                let mut fs = fs_arc.lock().unwrap();
                                
                                // 1. INVALIDIAMO IL FILE STESSO (Se esiste in cache)
                                // Questo era il pezzo mancante!
                                if let Some(&ino) = fs.path_to_inode.get(path_str) {
                                    println!("[WATCHER_CLIENT] Invalido cache per FILE: {} (inode {})", path_str, ino);
                                    fs.attribute_cache.remove(&ino);
                                }

                                // 2. INVALIDIAMO LA CARTELLA PADRE
                                // Serve per aggiornare la lista dei file e l'mtime della cartella
                                let parent_path = std::path::Path::new(path_str)
                                    .parent()
                                    .map_or("".to_string(), |p| p.to_string_lossy().to_string());
                                
                                if let Some(&parent_ino) = fs.path_to_inode.get(&parent_path) {
                                    println!("[WATCHER_CLIENT] Invalido cache per PARENT: {} (inode {})", parent_path, parent_ino);
                                    fs.attribute_cache.remove(&parent_ino);
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            println!("[WATCHER_CLIENT] Il server ha chiuso la connessione.");
                            break;
                        }
                        Err(e) => {
                            eprintln!("[WATCHER_CLIENT] Errore nella lettura del messaggio: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                println!("[WATCHER_CLIENT] Disconnesso. Riconnessione...");
            }
            Err(e) => {
                println!("[WATCHER_CLIENT] Connessione fallita: {}. Riprovo tra 5 secondi...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}
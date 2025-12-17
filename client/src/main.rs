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
use std::sync::{Arc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use futures_util::StreamExt;
use clap::Parser;
use crate::config::CacheStrategy;
use daemonize::Daemonize; 
use std::fs::File;

// NOTA: Non usiamo #[tokio::main] qui perché FUSE deve girare su un thread sincrono.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Il punto di mount per il filesystem.
    mountpoint: String,

    /// Esegui il processo come demone in background.
    #[arg(long)]
    daemon: bool,

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
    let should_daemonize = cli.daemon || config.daemon;
    // Deve essere eseguita PRIMA di spawnare qualsiasi thread (watcher) o creare connessioni.
    if should_daemonize {
        let stdout = File::create("/tmp/fuse_client.out").unwrap();
        let stderr = File::create("/tmp/fuse_client.err").unwrap();

        let daemonize = Daemonize::new()
            .pid_file("/tmp/fuse_client.pid") // Crea file PID per gestire il processo
            .chown_pid_file(true)
            .working_directory("/") // Buona norma per i demoni
            .stdout(stdout)  // Redireziona stdout su file
            .stderr(stderr); // Redireziona stderr su file

        match daemonize.start() {
            Ok(_) => println!("Success, daemonized"),
            Err(e) => {
                eprintln!("Error, {}", e);
                std::process::exit(1);
            }
        }
    }
    // --------------------------------

    // 4. Prendi il mountpoint dalla CLI
    let mountpoint = std::ffi::OsString::from(cli.mountpoint);

    // 5. Crea l'istanza di RemoteFS con la configurazione finale
    let fs_inner = RemoteFS::new(config.clone());
    let fs_wrapper = FsWrapper(Arc::new(Mutex::new(fs_inner)));

    // 6. Avvia il watcher in un thread separato
    // (IMPORTANTE: Questo thread viene creato DOPO il daemonize, quindi sopravvive nel processo figlio)
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
        MountOption::RW, 
        // MountOption::Debug, // Utile, ma ricorda che l'output va su file se sei in daemon mode
    ];
    
    println!("Mounting filesystem at {:?}", mountpoint);
    if let Err(e) = fuser::mount2(filesystem, &mountpoint, &options) {
        eprintln!("Failed to mount filesystem: {}", e);
    }
}

async fn connect_and_watch(fs_arc: Arc<Mutex<RemoteFS>>) {
    // Recuperiamo URL e ID Client proteggendo l'accesso con il lock
    let (url_str, my_client_id) = {
        let fs = fs_arc.lock().unwrap();
        // Costruiamo l'URL WS basandoci sulla config HTTP (es. http://... -> ws://...)
        let base = fs.config.server_url.replace("https://", "wss://").replace("http://", "ws://");
        (format!("{}/ws", base), fs.client_id.clone())
    };

    let url = Url::parse(&url_str).expect("URL WebSocket non valido");
    
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
                            // --- LOGICA ECHO SUPPRESSION ---
                            let (clean_text, sender_id) = if let Some((msg, id)) = text.rsplit_once("|BY:") {
                                (msg, Some(id))
                            } else {
                                (text.as_str(), None)
                            };

                            if let Some(id) = sender_id {
                                if id == my_client_id {
                                    // Ignora le notifiche generate da noi stessi
                                    continue;
                                }
                            }
                            // -------------------------------

                            if let Some(path_str) = clean_text.strip_prefix("CHANGE:") {
                                println!("[WATCHER_CLIENT] Notifica rilevante per: {}", path_str);
                                let mut fs = fs_arc.lock().unwrap();
                                
                                // 1. INVALIDIAMO IL FILE STESSO (Se esiste in cache)
                                if let Some(&ino) = fs.path_to_inode.get(path_str) {
                                    println!("[WATCHER_CLIENT] -> Invalido cache FILE (inode {})", ino);
                                    fs.attribute_cache.remove(&ino);
                                }

                                // 2. INVALIDIAMO LA CARTELLA PADRE
                                let parent_path = std::path::Path::new(path_str)
                                    .parent()
                                    .map_or("".to_string(), |p| p.to_string_lossy().to_string());
                                
                                if let Some(&parent_ino) = fs.path_to_inode.get(&parent_path) {
                                    println!("[WATCHER_CLIENT] -> Invalido cache PARENT (inode {})", parent_ino);
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
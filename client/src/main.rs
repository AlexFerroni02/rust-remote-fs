mod api_client;
mod config;
mod fs;

use fs::RemoteFS;
use fuser::MountOption;
use std::env;

fn main() {
    // 1. Carica la configurazione dal file config.toml
    let config = config::load_config();
    println!("Configurazione caricata: {:?}", config);

    // 2. Parsing degli argomenti per il mountpoint
    let mountpoint = match env::args_os().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Usage: {} <MOUNTPOINT>", env::args().next().unwrap());
            return;
        }
    };

    // 3. Creazione dell'istanza del filesystem con la configurazione
    let filesystem = RemoteFS::new(config.clone());

    // 4. Mount del filesystem usando i timeout dalla configurazione
    let options = vec![
        MountOption::AutoUnmount,
        MountOption::FSName("remoteFS".to_string()),
     ];
    println!("Mounting filesystem at {:?}", mountpoint);
    if let Err(e) = fuser::mount2(filesystem, &mountpoint, &options) {
        eprintln!("Failed to mount filesystem: {}", e);
    }
}
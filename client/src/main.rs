
// Dichiara agli altri file che sono moduli di questo crate
mod api_client;
mod fs;

use fs::RemoteFS; // Importa la struct principale
use std::env;
use fuser::MountOption;

fn main() {
    // 1. Parsing degli argomenti
    let mountpoint = match env::args_os().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Usage: {} <MOUNTPOINT>", env::args().next().unwrap());
            return;
        }
    };

    // 2. Creazione dell'istanza del filesystem
    //    Il costruttore `new` si occupa di tutta l'inizializzazione.
    let filesystem = RemoteFS::new();

    // 3. Mount del filesystem
    let options = vec![MountOption::AutoUnmount, MountOption::FSName("remoteFS".to_string())];
    println!("Mounting filesystem at {:?}", mountpoint);
    if let Err(e) = fuser::mount2(filesystem, &mountpoint, &options) {
        eprintln!("Failed to mount filesystem: {}", e);
    }
}
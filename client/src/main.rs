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

use fs::RemoteFS;
use fuser::MountOption;
use std::env;

fn main() {
    // 1. Load configuration from `config.toml`
    let config = config::load_config();
    println!("Configuration loaded: {:?}", config);

    // 2. Parse the mountpoint from command-line arguments
    let mountpoint = match env::args_os().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Usage: {} <MOUNTPOINT>", env::args().next().unwrap());
            return;
        }
    };

    // 3. Create an instance of the filesystem, passing in the configuration
    let filesystem = RemoteFS::new(config.clone());

    // 4. Mount the filesystem
    // We use `AutoUnmount` to automatically unmount when this process exits.
    let options = vec![
        MountOption::AutoUnmount,
        MountOption::FSName("remoteFS".to_string()),
    ];
    println!("Mounting filesystem at {:?}", mountpoint);
    if let Err(e) = fuser::mount2(filesystem, &mountpoint, &options) {
        eprintln!("Failed to mount filesystem: {}", e);
    }
}
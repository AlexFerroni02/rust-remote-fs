use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Defines the available strategies for the internal attribute cache.
///
/// This is read from `config.toml` and controls the behavior of `AttributeCache`.
#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CacheStrategy {
    /// Time-to-Live: Entries expire after a set duration.
    Ttl,
    /// Least Recently Used: The cache evicts the oldest entries when full.
    Lru,
    /// No Caching: Caching is disabled. `getattr` will always fetch from the server.
    None,
}

/// Holds all filesystem configuration, loaded from `config.toml`.
///
/// This struct defines the behavior of both the internal application cache
/// (what `AttributeCache` does) and the timeouts reported to the FUSE kernel.
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// The URL of the remote filesystem server.
    pub server_url: String,
    /// The strategy to use for the internal attribute cache.
    pub cache_strategy: CacheStrategy,
    /// Time-to-live in seconds for entries in the `Ttl` cache.
    pub cache_ttl_seconds: u64,
    /// The maximum number of entries for the `Lru` cache.
    pub cache_lru_capacity: usize,
    /// The attribute timeout (in seconds) reported to the FUSE kernel.
    /// This is the `TTL` value used in `reply.attr()`.
    pub kernel_attr_timeout_seconds: u64,
    /// The entry timeout (in seconds) reported to the FUSE kernel.
    /// This is the `TTL` value used in `reply.entry()`.
    pub kernel_entry_timeout_seconds: u64,
}

/// Provides a sane default configuration.
///
/// This is used as a fallback if `config.toml` is missing, unreadable,
/// or contains parsing errors.
impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8080".to_string(),
            cache_strategy: CacheStrategy::Ttl,
            cache_ttl_seconds: 60,
            cache_lru_capacity: 1000,
            kernel_attr_timeout_seconds: 1, // Keep kernel cache low for consistency
            kernel_entry_timeout_seconds: 1, // Keep kernel cache low for consistency
        }
    }
}

/// Loads the filesystem configuration from `config.toml` in the current directory.
///
/// If `config.toml` is not found, cannot be read, or fails to parse,
/// this function will print an error message to `stderr` and
/// return `Config::default()`.
pub fn load_config() -> Config {
    let path = Path::new("config.toml");
    if !path.exists() {
        println!("WARNING: 'config.toml' not found. Using default configuration.");
        return Config::default();
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: Failed to read 'config.toml': {}. Using default.", e);
            return Config::default();
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("ERROR: Failed to parse 'config.toml': {}. Using default.", e);
            Config::default()
        }
    }
}
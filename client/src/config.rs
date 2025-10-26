use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CacheStrategy {
    Ttl,
    Lru,
    None,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub cache_strategy: CacheStrategy,
    pub cache_ttl_seconds: u64,
    pub cache_lru_capacity: usize,
    pub kernel_attr_timeout_seconds: u64,
    pub kernel_entry_timeout_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_strategy: CacheStrategy::Ttl,
            cache_ttl_seconds: 60,
            cache_lru_capacity: 1000,
            kernel_attr_timeout_seconds: 1,
            kernel_entry_timeout_seconds: 1,
        }
    }
}

pub fn load_config() -> Config {
    let path = Path::new("config.toml");
    if !path.exists() {
        println!("ATTENZIONE: 'config.toml' non trovato. Uso configurazione di default.");
        return Config::default();
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERRORE: Impossibile leggere 'config.toml': {}. Uso default.", e);
            return Config::default();
        }
    };

    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("ERRORE: Parsing di 'config.toml' fallito: {}. Uso default.", e);
            Config::default()
        }
    }
}
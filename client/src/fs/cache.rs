use fuser::FileAttr;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use crate::config::{Config, CacheStrategy};

// Wrapper per una entry della cache con timestamp di scadenza (per TTL)
// Non serve che sia `pub` perché è un dettaglio implementativo del modulo
#[derive(Debug)]
struct TtlEntry {
    attr: FileAttr,
    expiry: Instant,
}

// Rendi `pub` l'enum perché verrà usato in `RemoteFS`
#[derive(Debug)]
pub enum AttributeCache {
    Ttl(HashMap<u64, TtlEntry>),
    Lru(LruCache<u64, FileAttr>),
    None,
}

// L'implementazione rimane la stessa, ma ora è associata a questo modulo
impl AttributeCache {
    pub fn new(config: &Config) -> Self {
        match config.cache_strategy {
            CacheStrategy::Ttl => AttributeCache::Ttl(HashMap::new()),
            CacheStrategy::Lru => {
                let capacity = NonZeroUsize::new(config.cache_lru_capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
                AttributeCache::Lru(LruCache::new(capacity))
            }
            CacheStrategy::None => AttributeCache::None,
        }
    }

    pub fn get(&mut self, ino: &u64) -> Option<FileAttr> {
        match self {
            AttributeCache::Ttl(cache) => {
                if let Some(entry) = cache.get(ino) {
                    if entry.expiry > Instant::now() {
                 
                        println!("[CACHE] HIT (TTL): Trovati attributi per l'inode {}", ino);
                        return Some(entry.attr.clone());
                    } else {
                        
                        println!("[CACHE] MISS (TTL Scaduto): Rimuovo attributi per l'inode {}", ino);
                        cache.remove(ino);
                    }
                }
            }
            AttributeCache::Lru(cache) => {
                if let Some(attr) = cache.get(ino) {
                    // --- AGGIUNGI QUESTO PRINTLN ---
                    println!("[CACHE] HIT (LRU): Trovati attributi per l'inode {}", ino);
                    return Some(attr.clone());
                }
            }
            AttributeCache::None => {}
        }
        // --- AGGIUNGI QUESTO PRINTLN ---
        println!("[CACHE] MISS: Nessun attributo trovato per l'inode {}", ino);
        None
    }

    pub fn put(&mut self, ino: u64, attr: FileAttr, ttl_duration: Duration) {
        println!("[CACHE] PUT: Inserisco attributi per l'inode {}", ino);
        match self {
            AttributeCache::Ttl(cache) => {
                let entry = TtlEntry {
                    attr,
                    expiry: Instant::now() + ttl_duration,
                };
                cache.insert(ino, entry);
            }
            AttributeCache::Lru(cache) => {
                cache.put(ino, attr);
            }
            AttributeCache::None => {}
        }
    }

    pub fn remove(&mut self, ino: &u64) {
        match self {
            AttributeCache::Ttl(cache) => {
                cache.remove(ino);
            }
            AttributeCache::Lru(cache) => {
                cache.pop(ino);
            }
            AttributeCache::None => {}
        }
    }
}
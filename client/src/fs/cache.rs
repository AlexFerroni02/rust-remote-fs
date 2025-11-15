use fuser::FileAttr;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use crate::config::{Config, CacheStrategy};

/// Holds a cached `FileAttr` and its expiration timestamp.
///
/// This is used exclusively by the `AttributeCache::Ttl` variant.
#[derive(Debug)]
pub(crate) struct TtlEntry {
    attr: FileAttr,
    expiry: Instant,
}

/// Represents the different caching strategies available for file attributes.
///
/// This enum allows `RemoteFS` to be configured with different caching
/// behaviors (TTL, LRU, or no caching at all).
#[derive(Debug)]
pub enum AttributeCache {
    /// A Time-to-Live cache. Entries expire after a set `Duration`.
    Ttl(HashMap<u64, TtlEntry>),
    /// A Least-Recently-Used cache with a fixed capacity.
    Lru(LruCache<u64, FileAttr>),
    /// Caching is disabled. All `get` calls will result in a miss.
    None,
}

impl AttributeCache {
    /// Creates a new `AttributeCache` based on the provided configuration.
    ///
    /// # Arguments
    /// * `config` - The filesystem's `Config` struct, which specifies the
    ///   desired `CacheStrategy` and (if applicable) LRU capacity.
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

    /// Attempts to retrieve a `FileAttr` from the cache.
    ///
    /// This method respects the rules of the active cache strategy:
    /// - `Ttl`: Returns the attributes only if they exist and have not expired.
    ///   Expired entries are automatically removed upon check.
    /// - `Lru`: Returns the attributes and marks the entry as recently used.
    /// - `None`: Always returns `None`.
    ///
    /// # Arguments
    /// * `ino` - The Inode number to look up.
    ///
    /// # Returns
    /// * `Some(FileAttr)` if a valid, non-expired entry is found.
    /// * `None` on a cache miss or if the entry is expired.
    pub fn get(&mut self, ino: &u64) -> Option<FileAttr> {
        match self {
            AttributeCache::Ttl(cache) => {
                if let Some(entry) = cache.get(ino) {
                    if entry.expiry > Instant::now() {

                        println!("[CACHE] HIT (TTL): Found attributes for inode {}", ino);
                        return Some(entry.attr.clone());
                    } else {

                        println!("[CACHE] MISS (Expired TTL): Removing attributes for inode {}", ino);
                        cache.remove(ino);
                    }
                }
            }
            AttributeCache::Lru(cache) => {
                if let Some(attr) = cache.get(ino) {
                    println!("[CACHE] HIT (LRU): Found attributes for inode {}", ino);
                    return Some(attr.clone());
                }
            }
            AttributeCache::None => {}
        }
        println!("[CACHE] MISS: No attributes found for inode {}", ino);
        None
    }

    /// Inserts or updates a `FileAttr` in the cache.
    ///
    /// # Arguments
    /// * `ino` - The Inode number to cache.
    /// * `attr` - The `FileAttr` to store.
    /// * `ttl_duration` - The `Duration` this entry should remain valid (only used by the `Ttl` strategy).
    pub fn put(&mut self, ino: u64, attr: FileAttr, ttl_duration: Duration) {
        println!("[CACHE] PUT: Inserting attributes for inode {}", ino);
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

    /// Manually removes (invalidates) an Inode from the cache.
    ///
    /// This is typically called after an operation that modifies the file
    /// (e.g., `write`, `setattr`, `unlink`).
    ///
    /// # Arguments
    /// * `ino` - The Inode number to remove.
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
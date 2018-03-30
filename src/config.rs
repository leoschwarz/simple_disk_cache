pub use encoding::DataEncoding;

/// General configuration of the cache functionality.
#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// Maximum size of the cache in bytes.
    pub max_bytes: u64,

    /// Encoding format of the data files.
    pub encoding: DataEncoding,

    /// Strategy of the cache used.
    pub strategy: CacheStrategy,

    /// Number of subdirectories per level. (There are two levels.)
    pub subdirs_per_level: u32,
}

#[derive(Clone, Debug)]
pub enum CacheStrategy {
    /// Least recently used.
    ///
    /// Delete the value that was least recently used when needed.
    /// This is a good trade off keeping active values around and
    /// deleting old ones to make room for new ones.
    LRU,
}

impl Default for CacheStrategy {
    fn default() -> Self {
        CacheStrategy::LRU
    }
}

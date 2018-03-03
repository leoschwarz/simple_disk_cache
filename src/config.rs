/// General configuration of the cache functionality.
#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// Maximum size of the cache in bytes.
    pub max_bytes: u64,

    /// Encoding format of the data files.
    pub encoding: DataEncoding,
}

#[derive(Clone, Debug)]
pub enum DataEncoding {
    Json,
}

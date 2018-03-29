extern crate addressable_queue;
extern crate bincode;
#[macro_use]
extern crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use addressable_queue::fifo::Queue;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::{self, File};
use std::hash::Hash;
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;

pub mod config;
use self::config::CacheConfig;

#[derive(Serialize, Deserialize)]
struct Metadata<K>
where
    K: Clone + Eq + Hash,
{
    current_size: u64,
    counter: u64,
    entries: Queue<K, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    id: u64,
}

pub struct SimpleCache<K, V>
where
    K: Clone + Eq + Hash + Serialize + DeserializeOwned,
{
    config: CacheConfig,
    metadata: Metadata<K>,
    metadata_file: PathBuf,
    data_dir: PathBuf,
    _phantom: PhantomData<V>,
}

impl<K, V> SimpleCache<K, V>
where
    K: DeserializeOwned + Serialize + Clone + Eq + Hash,
    V: DeserializeOwned + Serialize,
{
    pub fn initialize<P: Into<PathBuf>>(
        data_dir: P,
        config: CacheConfig,
    ) -> Result<Self, CacheError> {
        let data_dir = data_dir.into();
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir).map_err(|e| CacheError::CreateDir(e))?;
        }

        let metadata_file = data_dir.join(config.encoding.filename("cache_data"));
        let metadata = if metadata_file.exists() {
            let file = File::open(&metadata_file).map_err(|e| CacheError::ReadMetadata(e))?;
            config
                .encoding
                .deserialize(file)
                .map_err(|e| CacheError::DeserializeMetadata(e))?
        } else {
            Metadata {
                current_size: 0,
                entries: Queue::new(),
                counter: 0,
            }
        };

        Ok(SimpleCache {
            config,
            metadata,
            metadata_file,
            data_dir,
            _phantom: PhantomData,
        })
    }

    /// Try getting a value from the cache.
    ///
    /// Unless there is an error, this will either return `Ok(Some(value))` if a value was found,
    /// or `Ok(None)` if no value for the key exists in the cache.
    pub fn get(&mut self, key: &K) -> Result<Option<V>, CacheError> {
        if let Some(item) = self.metadata.entries.remove_key(key) {
            // Read the value from the disk.
            let file_path = self.data_file_path(item.id)?;
            let file = File::open(file_path).map_err(|e| CacheError::ReadCacheFile(e))?;
            let value = self.config
                .encoding
                .deserialize(file)
                .map_err(|e| CacheError::DeserializeValue(e))?;

            // Insert the item again at the end of the queue.
            self.metadata.entries.insert(key.clone(), item);
            self.write_metadata()?;
            Ok(Some(value))
        } else {
            // The cache does not store a relevant entry.
            Ok(None)
        }
    }

    /// Insert a value into the cache.
    ///
    /// If the key already exists, the previous value will be overwritten.
    pub fn put(&mut self, key: &K, value: &V) -> Result<(), CacheError> {
        let entry_id = if let Some(entry) = self.metadata.entries.remove_key(key) {
            // Reuse the same file.
            // Note that later it will be added again to data.entries.
            entry.id
        } else {
            // Create a new entry.
            let entry_id = self.metadata.counter;
            self.metadata.counter += 1;
            entry_id
        };

        // Write the file.
        let file_path = self.data_file_path(entry_id)?;
        let mut file = File::create(&file_path).map_err(|e| CacheError::CreateFile(e, file_path))?;
        let bytes = self.config
            .encoding
            .serialize(&mut file, value)
            .map_err(|e| CacheError::SerializeValue(e))? as u64;

        // Put the entry into the data struct.
        self.metadata.entries.insert(
            key.clone(),
            CacheEntry {
                size: bytes,
                id: entry_id,
            },
        );
        self.metadata.current_size += bytes;

        // Cleanup entries if needed.
        self.cleanup()?;

        // Write modified metadata.
        self.write_metadata()?;

        Ok(())
    }

    fn write_metadata(&self) -> Result<(), CacheError> {
        let mut file = File::create(&self.metadata_file)
            .map_err(|e| CacheError::CreateFile(e, self.metadata_file.clone()))?;

        self.config
            .encoding
            .serialize(&mut file, &self.metadata)
            .map_err(|e| CacheError::SerializeMetadata(e))?;
        Ok(())
    }

    fn data_file_path(&self, entry_id: u64) -> Result<PathBuf, CacheError> {
        // Determine file subdirectory.
        let s = self.config.subdirs_per_level as u64;
        let subdir_1 = entry_id % s;
        let subdir_2 = (entry_id / s) % s;

        // Assert the directory exists.
        let dir = self.data_dir.join(format!("{}/{}", subdir_1, subdir_2));
        fs::create_dir_all(&dir).map_err(|e| CacheError::CreateDir(e))?;

        // Determine file path.
        let path = Ok(dir.join(format!(
            "data_{}.{}",
            entry_id,
            self.config.encoding.extension()
        )));
        path
    }

    /// Deletes as many cache entries as needed until the maximum storage is
    /// free again.
    fn cleanup(&mut self) -> Result<(), CacheError> {
        while self.metadata.current_size > self.config.max_bytes {
            let (_, entry) = self.metadata.entries.remove_head().unwrap();
            self.metadata.current_size -= entry.size;
            let path = self.data_file_path(entry.id)?;
            fs::remove_file(&path).map_err(|e| CacheError::RemoveFile(e, path))?;
        }
        Ok(())
    }
}

/// Various errors that can occur when operating a cache.
#[derive(Debug, Fail)]
pub enum CacheError {
    #[fail(display = "Reading metadata file failed: {:?}", _0)]
    ReadMetadata(io::Error),

    #[fail(display = "Deserializing cache metadata failed: {:?}", _0)]
    DeserializeMetadata(config::DeserializeError),

    #[fail(display = "Serializing cache metadata failed: {:?}", _0)]
    SerializeMetadata(config::SerializeError),

    #[fail(display = "Reading cache data file failed: {:?}", _0)]
    ReadCacheFile(io::Error),

    #[fail(display = "Deserializing cache value failed: {:?}", _0)]
    DeserializeValue(config::DeserializeError),

    #[fail(display = "Serializing cache value failed: {:?}", _0)]
    SerializeValue(config::SerializeError),

    #[fail(display = "Creating directory failed: {:?}", _0)]
    CreateDir(io::Error),

    #[fail(display = "Creating file failed: {:?}, filename = '{:?}'", _0, _1)]
    CreateFile(io::Error, PathBuf),

    #[fail(display = "Writing file failed: {:?}, filename = '{:?}'", _0, _1)]
    WriteFile(io::Error, PathBuf),

    #[fail(display = "Deleting file failed: {:?}, filename = '{:?}'", _0, _1)]
    RemoveFile(io::Error, PathBuf),
}

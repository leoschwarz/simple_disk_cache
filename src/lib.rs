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
use std::io::{self, Write};
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
    data: Metadata<K>,
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

        let data_file = data_dir.join("cache_data.json");
        let data = if data_file.exists() {
            let file = File::open(data_file).map_err(|e| CacheError::ReadMetadata(e))?;
            serde_json::from_reader(file).map_err(|e| CacheError::DeserializeMetada(e))?
        } else {
            Metadata {
                current_size: 0,
                entries: Queue::new(),
                counter: 0,
            }
        };

        Ok(SimpleCache {
            config,
            data,
            data_dir,
            _phantom: PhantomData,
        })
    }

    /// Try getting a value from the cache.
    ///
    /// Unless there is an error, this will either return `Ok(Some(value))` if a value was found,
    /// or `Ok(None)` if no value for the key exists in the cache.
    pub fn get(&mut self, key: &K) -> Result<Option<V>, CacheError> {
        if let Some(item) = self.data.entries.remove_key(key) {
            // Read the value from the disk.
            let file_name = self.data_file_path(item.id)?;
            let file = File::open(file_name).map_err(|e| CacheError::ReadCacheFile(e))?;
            let value = self.config
                .encoding
                .deserialize(file)
                .map_err(|e| CacheError::DeserializeValue(e))?;

            // Insert the item again at the end of the queue.
            self.data.entries.insert(key.clone(), item);
            self.write_metadata()?;
            Ok(Some(value))
        } else {
            // The cache does not store a relevant entry.
            Ok(None)
        }
    }

    /// Insert a value into the cache.
    pub fn put(&mut self, key: &K, value: &V) -> Result<(), CacheError> {
        let entry_id = self.data.counter;
        self.data.counter += 1;

        // Write the file.
        let mut file =
            File::create(self.data_file_path(entry_id)?).map_err(|e| CacheError::CreateFile(e))?;
        let bytes = self.config
            .encoding
            .serialize(&mut file, value)
            .map_err(|e| CacheError::SerializeValue(e))? as u64;

        // Put the entry into the data struct.
        self.data.entries.insert(
            key.clone(),
            CacheEntry {
                size: bytes,
                id: entry_id,
            },
        );
        self.data.current_size += bytes;

        // Cleanup entries if needed.
        self.cleanup()?;

        // Write modified metadata.
        self.write_metadata()?;

        Ok(())
    }

    fn write_metadata(&self) -> Result<(), CacheError> {
        let data_file = self.data_dir.join("cache_data.json");
        let mut file = File::create(data_file).map_err(|e| CacheError::CreateFile(e))?;

        let data = serde_json::to_vec(&self.data).map_err(|e| CacheError::SerializeMetadata(e))?;
        file.write(&data).map_err(|e| CacheError::WriteFile(e))?;
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
        let path = Ok(self.data_dir.join(dir.join(format!(
            "data_{}.{}",
            entry_id,
            self.config.encoding.extension()
        ))));
        println!("path: {:?}", path);
        path
    }

    /// Deletes as many cache entries as needed until the maximum storage is
    /// free again.
    fn cleanup(&mut self) -> Result<(), CacheError> {
        while self.data.current_size > self.config.max_bytes {
            let (_, entry) = self.data.entries.remove_head().unwrap();
            self.data.current_size -= entry.size;
            let path = self.data_file_path(entry.id)?;
            fs::remove_file(path).map_err(|e| CacheError::RemoveFile(e))?;
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
    DeserializeMetada(serde_json::Error),

    #[fail(display = "Serializing cache metadata failed: {:?}", _0)]
    SerializeMetadata(serde_json::Error),

    #[fail(display = "Reading cache data file failed: {:?}", _0)]
    ReadCacheFile(io::Error),

    #[fail(display = "Deserializing cache value failed: {:?}", _0)]
    DeserializeValue(config::DeserializeError),

    #[fail(display = "Serializing cache value failed: {:?}", _0)]
    SerializeValue(config::SerializeError),

    #[fail(display = "Creating directory failed: {:?}", _0)]
    CreateDir(io::Error),

    #[fail(display = "Creating file failed: {:?}", _0)]
    CreateFile(io::Error),

    #[fail(display = "Writing file failed: {:?}", _0)]
    WriteFile(io::Error),

    #[fail(display = "Deleting file failed: {:?}", _0)]
    RemoveFile(io::Error),
}

use bincode;
use serde_json;
use std::io::{self, Read, Write};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// General configuration of the cache functionality.
#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// Maximum size of the cache in bytes.
    pub max_bytes: u64,

    /// Encoding format of the data files.
    pub encoding: DataEncoding,

    /// Strategy of the cache used.
    pub strategy: CacheStrategy,
}

#[derive(Clone, Debug)]
pub enum DataEncoding {
    Bincode,
    Json,
}

impl DataEncoding {
    pub(crate) fn extension(&self) -> &'static str {
        match *self {
            DataEncoding::Bincode => "bincode",
            DataEncoding::Json => "json",
        }
    }

    pub(crate) fn serialize<T: Serialize, W: Write>(
        &self,
        writer: &mut W,
        value: &T,
    ) -> Result<usize, SerializeError> {
        // TODO: This could probably be improved by a lot if we could actually write
        // directly into the Write instead of first into a vec. Both the bincode and serde_json
        // crate offer such method, but neither reports the bytes written. It should be evaluated
        // if a simple wrapper struct for Write which counts the bytes written is more performant
        // than serializing to a Vec<u8> first.

        let bytes = match *self {
            DataEncoding::Bincode => {
                bincode::serialize(value).map_err(|e| SerializeError::Bincode(e))?
            }
            DataEncoding::Json => serde_json::to_vec(value).map_err(|e| SerializeError::Json(e))?,
        };
        let len = bytes.len();
        writer
            .write(&bytes[..])
            .map_err(|e| SerializeError::WriteError(e))?;
        Ok(len)
    }

    pub(crate) fn deserialize<T: DeserializeOwned, R: Read>(
        &self,
        reader: R,
    ) -> Result<T, DeserializeError> {
        match *self {
            DataEncoding::Bincode => {
                bincode::deserialize_from(reader).map_err(|e| DeserializeError::Bincode(e))
            }
            DataEncoding::Json => {
                serde_json::from_reader(reader).map_err(|e| DeserializeError::Json(e))
            }
        }
    }
}

#[derive(Debug, Fail)]
pub enum SerializeError {
    #[fail(display = "Failed serializing to bincode: {:?}", _0)]
    Bincode(bincode::Error),

    #[fail(display = "Failed serializing to json: {:?}", _0)]
    Json(serde_json::Error),

    #[fail(display = "Writing to file failed: {:?}", _0)]
    WriteError(io::Error),
}

#[derive(Debug, Fail)]
pub enum DeserializeError {
    #[fail(display = "Failed deserializing bincode: {:?}", _0)]
    Bincode(bincode::Error),

    #[fail(display = "Failed deserializing json: {:?}", _0)]
    Json(serde_json::Error),
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

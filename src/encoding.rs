use bincode;
use serde_json;
use std::io::{self, Read, Write};
use serde::Serialize;
use serde::de::DeserializeOwned;

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

    pub(crate) fn filename(&self, basename: &str) -> String {
        format!("{}.{}", basename, self.extension())
    }

    pub(crate) fn serialize<T: Serialize, W: Write>(
        &self,
        writer: &mut W,
        value: &T,
    ) -> Result<usize, SerializeError> {
        let mut write_counter = WriteCounter::new(writer);
        match *self {
            DataEncoding::Bincode => bincode::serialize_into(&mut write_counter, value)
                .map_err(|e| SerializeError::Bincode(e))?,
            DataEncoding::Json => serde_json::to_writer(&mut write_counter, value)
                .map_err(|e| SerializeError::Json(e))?,
        };
        Ok(write_counter.counter)
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

/// Write impl which provides a counter for the number of bytes written,
/// even if the functions writing to it don't provide such information.
struct WriteCounter<W> {
    writer: W,
    counter: usize,
}

impl<W> WriteCounter<W> {
    fn new(writer: W) -> Self {
        WriteCounter { writer, counter: 0 }
    }
}

impl<W: Write> Write for WriteCounter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = self.writer.write(buf)?;
        self.counter += len;
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
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

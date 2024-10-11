use alloc::vec::Vec;

use super::{Error, Protocol, Result};

#[derive(Debug, Default, Clone)]
pub struct PostcardProtocol;

impl Protocol for PostcardProtocol {
    fn serialize<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>> {
        postcard::to_allocvec(data).map_err(|e| Error::Serialization(alloc::format!("{:?}", e)))
    }

    fn deserialize<T: for<'de> serde::Deserialize<'de>>(&self, bytes: &[u8]) -> Result<T> {
        postcard::from_bytes(bytes).map_err(|e| Error::Deserialization(alloc::format!("{:?}", e)))
    }

    fn name(&self) -> &'static str {
        "postcard"
    }
}

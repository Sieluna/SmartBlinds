use alloc::vec::Vec;

use super::{Error, Protocol, Result};

#[derive(Debug, Default, Clone)]
pub struct JsonProtocol;

impl Protocol for JsonProtocol {
    fn serialize<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>> {
        serde_json::to_vec(data).map_err(|e| Error::Serialization(alloc::format!("{}", e)))
    }

    fn deserialize<T: for<'de> serde::Deserialize<'de>>(&self, bytes: &[u8]) -> Result<T> {
        serde_json::from_slice(bytes).map_err(|e| Error::Deserialization(alloc::format!("{}", e)))
    }

    fn name(&self) -> &'static str {
        "json"
    }
}

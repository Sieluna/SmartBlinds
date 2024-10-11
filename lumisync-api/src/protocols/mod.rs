pub mod json;
pub mod postcard;

#[derive(Debug)]
pub enum Error {
    /// Serialization error
    Serialization(alloc::string::String),
    /// Deserialization error
    Deserialization(alloc::string::String),
    /// Transport error
    Transport(alloc::string::String),
    /// Unknown error
    Unknown,
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait Protocol: Send + Sync {
    /// Serialize data into bytes
    fn serialize<T: serde::Serialize>(&self, data: &T) -> Result<alloc::vec::Vec<u8>>;

    /// Deserialize bytes into data
    fn deserialize<T: for<'de> serde::Deserialize<'de>>(&self, bytes: &[u8]) -> Result<T>;

    /// Get protocol name
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub enum SerializationProtocol {
    Postcard(postcard::PostcardProtocol),
    Json(json::JsonProtocol),
}

impl Protocol for SerializationProtocol {
    fn serialize<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>> {
        match self {
            SerializationProtocol::Json(protocol) => protocol.serialize(data),
            SerializationProtocol::Postcard(protocol) => protocol.serialize(data),
        }
    }

    fn deserialize<T: for<'de> serde::Deserialize<'de>>(&self, bytes: &[u8]) -> Result<T> {
        match self {
            SerializationProtocol::Json(protocol) => protocol.deserialize(bytes),
            SerializationProtocol::Postcard(protocol) => protocol.deserialize(bytes),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            SerializationProtocol::Json(protocol) => protocol.name(),
            SerializationProtocol::Postcard(protocol) => protocol.name(),
        }
    }
}

impl Default for SerializationProtocol {
    fn default() -> Self {
        Self::Postcard(postcard::PostcardProtocol::default())
    }
}

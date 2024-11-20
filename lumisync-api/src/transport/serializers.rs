use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::error::TransportError;
use super::protocol::Protocol;

pub trait Serializer {
    fn serialize<T: Serialize>(data: &T) -> Result<Vec<u8>, TransportError>;
    fn deserialize<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, TransportError>;
}

pub struct PostcardSerializer;

impl Serializer for PostcardSerializer {
    fn serialize<T: Serialize>(data: &T) -> Result<Vec<u8>, TransportError> {
        postcard::to_allocvec(data)
            .map_err(|e| TransportError::Serialization(alloc::format!("{:?}", e)))
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, TransportError> {
        postcard::from_bytes(data)
            .map_err(|e| TransportError::Deserialization(alloc::format!("{:?}", e)))
    }
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    fn serialize<T: Serialize>(data: &T) -> Result<Vec<u8>, TransportError> {
        serde_json::to_vec(data).map_err(|e| TransportError::Serialization(alloc::format!("{}", e)))
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, TransportError> {
        serde_json::from_slice(data)
            .map_err(|e| TransportError::Deserialization(alloc::format!("{}", e)))
    }
}

pub fn serialize<T: Serialize>(protocol: Protocol, data: &T) -> Result<Vec<u8>, TransportError> {
    match protocol {
        Protocol::Postcard => PostcardSerializer::serialize(data),
        Protocol::Json => JsonSerializer::serialize(data),
    }
}

pub fn deserialize<T: for<'de> Deserialize<'de>>(
    protocol: Protocol,
    data: &[u8],
) -> Result<T, TransportError> {
    match protocol {
        Protocol::Postcard => PostcardSerializer::deserialize(data),
        Protocol::Json => JsonSerializer::deserialize(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: u32,
        name: alloc::string::String,
        values: Vec<i32>,
    }

    impl TestData {
        fn new(id: u32, name: &str, values: Vec<i32>) -> Self {
            Self {
                id,
                name: name.into(),
                values,
            }
        }
    }

    #[test]
    fn test_postcard_roundtrip() {
        let data = TestData::new(42, "test", vec![1, 2, 3]);

        let serialized = PostcardSerializer::serialize(&data).unwrap();
        let deserialized: TestData = PostcardSerializer::deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_json_roundtrip() {
        let data = TestData::new(42, "test", vec![1, 2, 3]);

        let serialized = JsonSerializer::serialize(&data).unwrap();
        let deserialized: TestData = JsonSerializer::deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_protocol_dispatch() {
        let data = TestData::new(1, "dispatch_test", vec![]);

        let postcard_data = serialize(Protocol::Postcard, &data).unwrap();
        let postcard_result: TestData = deserialize(Protocol::Postcard, &postcard_data).unwrap();
        assert_eq!(data, postcard_result);

        let json_data = serialize(Protocol::Json, &data).unwrap();
        let json_result: TestData = deserialize(Protocol::Json, &json_data).unwrap();
        assert_eq!(data, json_result);
    }

    #[test]
    fn test_serialization_efficiency() {
        let data = TestData::new(12345, "size_test", vec![1, 2, 3, 4, 5]);

        let postcard_size = serialize(Protocol::Postcard, &data).unwrap().len();
        let json_size = serialize(Protocol::Json, &data).unwrap().len();

        // Postcard should be more compact than JSON
        assert!(postcard_size < json_size);
    }
}

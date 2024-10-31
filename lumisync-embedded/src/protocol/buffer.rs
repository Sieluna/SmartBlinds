use alloc::vec::Vec;

use lumisync_api::Message;
use lumisync_api::protocols::{Protocol, SerializationProtocol};

use crate::{Error, Result};

pub struct MessageBuffer {
    buffer: Vec<u8>,
    capacity: usize,
}

impl MessageBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Add data to buffer
    pub fn push_data(&mut self, data: &[u8]) -> Result<()> {
        if self.buffer.len() + data.len() > self.capacity {
            return Err(Error::InvalidState);
        }

        self.buffer.extend_from_slice(data);
        Ok(())
    }

    /// Try to parse message
    pub fn try_parse_message(
        &mut self,
        protocol: &SerializationProtocol,
    ) -> Result<Option<Message>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        // Try to deserialize the entire buffer
        match protocol.deserialize::<Message>(&self.buffer) {
            Ok(message) => {
                self.buffer.clear();
                Ok(Some(message))
            }
            Err(_) => {
                // If parsing fails, data might be incomplete, wait for more data
                Ok(None)
            }
        }
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get buffer usage
    pub fn usage(&self) -> (usize, usize) {
        (self.buffer.len(), self.capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_buffer() {
        let mut buffer = MessageBuffer::new(1024);

        let data = b"test data";
        buffer.push_data(data).unwrap();

        let (used, capacity) = buffer.usage();
        assert_eq!(used, data.len());
        assert_eq!(capacity, 1024);

        buffer.clear();
        let (used, _) = buffer.usage();
        assert_eq!(used, 0);
    }
}

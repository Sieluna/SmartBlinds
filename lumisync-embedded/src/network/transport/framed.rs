use alloc::vec::Vec;

use crate::{Error, Result};

use super::RawTransport;

pub struct FramedTransport<T> {
    transport: T,
    receive_state: ReceiveState,
}

enum ReceiveState {
    WaitingForLength,
    WaitingForData {
        expected_len: usize,
        buffer: Vec<u8>,
    },
}

impl<T: RawTransport> FramedTransport<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            receive_state: ReceiveState::WaitingForLength,
        }
    }

    /// Get reference to the inner transport
    pub fn inner(&self) -> &T {
        &self.transport
    }

    /// Get mutable reference to the inner transport
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl<T: RawTransport<Error = Error>> RawTransport for FramedTransport<T> {
    type Error = Error;

    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        let len = data.len() as u32;
        let len_bytes = len.to_be_bytes();
        self.transport.send_bytes(&len_bytes).await?;
        self.transport.send_bytes(data).await?;
        Ok(())
    }

    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>> {
        match &mut self.receive_state {
            ReceiveState::WaitingForLength => {
                let mut len_bytes = [0u8; 4];
                if let Some(4) = self.transport.receive_bytes(&mut len_bytes).await? {
                    let expected_len = u32::from_be_bytes(len_bytes) as usize;
                    self.receive_state = ReceiveState::WaitingForData {
                        expected_len,
                        buffer: Vec::with_capacity(expected_len),
                    };
                }
                Ok(None)
            }
            ReceiveState::WaitingForData {
                expected_len,
                buffer: msg_buffer,
            } => {
                let remaining = *expected_len - msg_buffer.len();
                let mut temp_buffer = alloc::vec![0u8; remaining];

                if let Some(n) = self.transport.receive_bytes(&mut temp_buffer).await? {
                    msg_buffer.extend_from_slice(&temp_buffer[..n]);

                    if msg_buffer.len() >= *expected_len {
                        let len = msg_buffer.len().min(buffer.len());
                        buffer[..len].copy_from_slice(&msg_buffer[..len]);
                        self.receive_state = ReceiveState::WaitingForLength;
                        return Ok(Some(len));
                    }
                }
                Ok(None)
            }
        }
    }
}

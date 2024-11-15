use lumisync_api::{Message, Protocol, SerializationProtocol};

use crate::Error;
use crate::message::MessageTransport;

use super::RawTransport;

pub struct ProtocolWrapper<T> {
    transport: T,
    protocol: SerializationProtocol,
}

impl<T> ProtocolWrapper<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            protocol: SerializationProtocol::default(),
        }
    }

    pub fn with_protocol(transport: T, protocol: SerializationProtocol) -> Self {
        Self {
            transport,
            protocol,
        }
    }

    /// Get mutable reference to the inner transport layer
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Get reference to the inner transport layer
    pub fn inner(&self) -> &T {
        &self.transport
    }
}

impl<T> MessageTransport for ProtocolWrapper<T>
where
    T: RawTransport,
    T::Error: core::fmt::Debug,
{
    type Error = Error;

    async fn send_message(&mut self, message: &Message) -> Result<(), Self::Error> {
        let bytes = self
            .protocol
            .serialize(message)
            .map_err(|_| Error::SerializationError)?;

        self.transport
            .send_bytes(&bytes)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Option<Message>, Self::Error> {
        let mut buffer = [0u8; 512]; // Maximum BLE MTU buffer

        if let Some(len) = self
            .transport
            .receive_bytes(&mut buffer)
            .await
            .map_err(|_| Error::NetworkError)?
        {
            let message = self
                .protocol
                .deserialize(&buffer[..len])
                .map_err(|_| Error::SerializationError)?;
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use lumisync_api::{
        ActuatorCommand, EdgeCommand, MessageHeader, MessagePayload, NodeId, Priority,
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;

    struct MockRawTransport {
        send_data: Vec<Vec<u8>>,
        receive_data: Vec<Vec<u8>>,
    }

    impl MockRawTransport {
        fn new() -> Self {
            Self {
                send_data: Vec::new(),
                receive_data: Vec::new(),
            }
        }

        fn add_receive_data(&mut self, data: Vec<u8>) {
            self.receive_data.push(data);
        }
    }

    impl RawTransport for MockRawTransport {
        type Error = ();

        async fn send_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.send_data.push(data.to_vec());
            Ok(())
        }

        async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Self::Error> {
            if let Some(data) = self.receive_data.pop() {
                let len = data.len().min(buffer.len());
                buffer[..len].copy_from_slice(&data[..len]);
                Ok(Some(len))
            } else {
                Ok(None)
            }
        }
    }

    #[tokio::test]
    async fn test_protocol_wrapper_send_receive() {
        let raw_transport = MockRawTransport::new();
        let mut wrapper = ProtocolWrapper::new(raw_transport);

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id: 42,
                sequence: 1,
                command: ActuatorCommand::SetWindowPosition(75),
            }),
        };

        // Test sending
        wrapper.send_message(&message).await.unwrap();
        assert_eq!(wrapper.inner().send_data.len(), 1);

        // Prepare receive data
        let sent_data = wrapper.inner().send_data[0].clone();
        wrapper.inner_mut().add_receive_data(sent_data);

        // Test receiving
        let received = wrapper.receive_message().await.unwrap();
        assert!(received.is_some());
        let received_message = received.unwrap();
        assert_eq!(received_message.header.id, message.header.id);
    }
}

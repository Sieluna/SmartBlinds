#[cfg(feature = "ble")]
pub mod ble;
pub mod framed;
pub mod protocol;
#[cfg(feature = "tcp")]
pub mod tcp;
#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "ble")]
pub use ble::{BleCentralTransport, BlePeripheralTransport};
pub use framed::FramedTransport;
pub use protocol::ProtocolWrapper;
#[cfg(feature = "tcp")]
pub use tcp::TcpTransport;
#[cfg(feature = "udp")]
pub use udp::{UdpClientTransport, UdpServerTransport, UdpTransport};

#[cfg(feature = "tcp")]
pub type FramedTcpTransport = FramedTransport<TcpTransport>;

#[allow(async_fn_in_trait)]
pub trait RawTransport {
    type Error;

    /// Send raw byte data
    async fn send_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Receive raw byte data
    async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Self::Error>;
}

#[cfg(test)]
pub mod tests {
    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    use lumisync_api::{
        ActuatorCommand, EdgeCommand, Message, MessageHeader, MessagePayload, NodeId, Priority,
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::{Error, MessageTransport};

    use super::*;

    pub struct MockTransport {
        pub sent_data: Vec<Vec<u8>>,
        pub receive_queue: VecDeque<Vec<u8>>,
        pub fragment_size: Option<usize>,
    }

    impl MockTransport {
        pub fn new() -> Self {
            Self {
                sent_data: Vec::new(),
                receive_queue: VecDeque::new(),
                fragment_size: None,
            }
        }

        pub fn add_receive(&mut self, data: Vec<u8>) {
            self.receive_queue.push_back(data);
        }

        pub fn enable_fragmentation(&mut self, size: usize) {
            self.fragment_size = Some(size);
        }
    }

    impl RawTransport for MockTransport {
        type Error = Error;

        async fn send_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.sent_data.push(data.to_vec());
            Ok(())
        }

        async fn receive_bytes(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Self::Error> {
            if let Some(data) = self.receive_queue.front_mut() {
                if data.is_empty() {
                    self.receive_queue.pop_front();
                    return Ok(None);
                }

                let max = self.fragment_size.unwrap_or(buffer.len());
                let n = max.min(buffer.len()).min(data.len());
                buffer[..n].copy_from_slice(&data[..n]);
                data.drain(..n);
                if data.is_empty() {
                    self.receive_queue.pop_front();
                }
                Ok(Some(n))
            } else {
                Ok(None)
            }
        }
    }

    fn test_message() -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Device([1, 2, 3, 4, 5, 6]),
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id: 1,
                sequence: 1,
                command: ActuatorCommand::SetWindowPosition(42),
            }),
        }
    }

    #[tokio::test]
    async fn test_tcp_framed_protocol_stack() {
        let mock = MockTransport::new();
        let framed = FramedTransport::new(mock);
        let mut protocol = ProtocolWrapper::new(framed);
        protocol.send_message(&test_message()).await.unwrap();

        let sent = &protocol.inner().inner().sent_data;
        assert_eq!(sent.len(), 2); // [length prefix, payload]
        assert_eq!(
            u32::from_be_bytes([sent[0][0], sent[0][1], sent[0][2], sent[0][3]]) as usize,
            sent[1].len()
        );
    }

    #[tokio::test]
    async fn test_udp_protocol_stack() {
        let mock = MockTransport::new();
        let mut protocol = ProtocolWrapper::new(mock);
        protocol.send_message(&test_message()).await.unwrap();

        let sent = &protocol.inner().sent_data;
        assert_eq!(sent.len(), 1); // No framing
    }

    #[tokio::test]
    async fn test_receive_with_fragmentation() {
        let mut mock = MockTransport::new();
        mock.enable_fragmentation(3);
        mock.add_receive(b"abcdef".to_vec());

        let mut buf = [0u8; 4];
        let mut out = Vec::new();
        while let Some(n) = mock.receive_bytes(&mut buf).await.unwrap() {
            out.extend_from_slice(&buf[..n]);
        }

        assert_eq!(out, b"abcdef");
    }
}

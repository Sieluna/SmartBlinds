pub mod crc;
pub mod error;
pub mod frame;
pub mod io;
pub mod protocol;
pub mod serializers;

pub use crc::{Crc32, crc32};
pub use error::TransportError;
pub use frame::{FrameFlags, FrameHeader};
pub use io::{AsyncMessageTransport, SyncMessageTransport};
pub use protocol::Protocol;
pub use serializers::{JsonSerializer, PostcardSerializer, Serializer, deserialize, serialize};

/// Default buffer size for transport operations
pub const DEFAULT_BUFFER_SIZE: usize = 4096;

/// Maximum message size limit (16MB)
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

#[cfg(test)]
mod async_tests {
    use alloc::vec::Vec;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestMessage {
        id: u32,
        name: alloc::string::String,
        data: Vec<u8>,
    }

    impl TestMessage {
        fn new(id: u32, name: &str, data: Vec<u8>) -> Self {
            Self {
                id,
                name: name.into(),
                data,
            }
        }
    }

    /// Mock IO for async testing
    #[derive(Debug)]
    struct AsyncMockIo {
        read_data: Vec<u8>,
        write_data: Vec<u8>,
        read_pos: usize,
        read_chunk_size: Option<usize>,
        fail_after_bytes: Option<usize>,
        bytes_written: usize,
    }

    impl AsyncMockIo {
        fn new() -> Self {
            Self {
                read_data: Vec::new(),
                write_data: Vec::new(),
                read_pos: 0,
                read_chunk_size: None,
                fail_after_bytes: None,
                bytes_written: 0,
            }
        }

        fn with_data(data: Vec<u8>) -> Self {
            Self {
                read_data: data,
                write_data: Vec::new(),
                read_pos: 0,
                read_chunk_size: None,
                fail_after_bytes: None,
                bytes_written: 0,
            }
        }

        fn with_chunk_size(mut self, chunk_size: usize) -> Self {
            self.read_chunk_size = Some(chunk_size);
            self
        }

        fn written_data(&self) -> &[u8] {
            &self.write_data
        }

        fn reset_read(&mut self) {
            self.read_pos = 0;
            self.read_data = self.write_data.clone();
            self.write_data.clear();
            self.bytes_written = 0;
        }
    }

    impl embedded_io_async::ErrorType for AsyncMockIo {
        type Error = embedded_io_async::ErrorKind;
    }

    impl embedded_io_async::Read for AsyncMockIo {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let available = self.read_data.len() - self.read_pos;
            if available == 0 {
                return Ok(0);
            }

            let max_read = if let Some(chunk_size) = self.read_chunk_size {
                buf.len().min(chunk_size).min(available)
            } else {
                buf.len().min(available)
            };

            buf[..max_read]
                .copy_from_slice(&self.read_data[self.read_pos..self.read_pos + max_read]);
            self.read_pos += max_read;
            Ok(max_read)
        }
    }

    impl embedded_io_async::Write for AsyncMockIo {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            if let Some(fail_after) = self.fail_after_bytes {
                if self.bytes_written + buf.len() > fail_after {
                    return Err(embedded_io_async::ErrorKind::Other);
                }
            }

            self.write_data.extend_from_slice(buf);
            self.bytes_written += buf.len();
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_async_sticky_packet_scenario() {
        // Test multiple messages in single read
        let messages = vec![
            TestMessage::new(1, "first", vec![1, 2, 3]),
            TestMessage::new(2, "second", vec![4, 5, 6]),
            TestMessage::new(3, "third", vec![7, 8, 9]),
        ];

        // Serialize all messages separately and concatenate
        let mut all_data = Vec::new();
        for msg in &messages {
            let temp_io = AsyncMockIo::new();
            let mut temp_transport = AsyncMessageTransport::new(temp_io);
            temp_transport
                .send_message(msg, Some(Protocol::Postcard), None)
                .await
                .unwrap();
            all_data.extend_from_slice(temp_transport.inner().written_data());
        }

        // Test reading all messages from concatenated data
        let mock_io = AsyncMockIo::with_data(all_data);
        let mut transport = AsyncMessageTransport::new(mock_io);

        for expected in messages {
            let (received, _, _): (TestMessage, _, _) = transport.receive_message().await.unwrap();
            assert_eq!(received, expected);
        }
    }

    #[tokio::test]
    async fn test_async_fragmented_packet_scenario() {
        // Test message fragmentation
        let large_data = vec![42u8; 10000];
        let message = TestMessage::new(1, "fragmented", large_data.clone());

        // Serialize message normally
        let mut temp_transport = AsyncMessageTransport::new(AsyncMockIo::new());
        temp_transport
            .send_message(&message, Some(Protocol::Postcard), None)
            .await
            .unwrap();
        let full_data = temp_transport.inner().written_data().to_vec();

        // Read with small chunks to simulate fragmentation
        let mock_io = AsyncMockIo::with_data(full_data).with_chunk_size(64);
        let mut transport = AsyncMessageTransport::new(mock_io);

        let (received, _, _): (TestMessage, _, _) = transport.receive_message().await.unwrap();
        assert_eq!(received.data, large_data);
    }

    #[tokio::test]
    async fn test_async_protocol_mixing() {
        // Test mixed protocols with stream IDs
        let test_cases = vec![
            (
                TestMessage::new(1, "postcard", vec![1]),
                Protocol::Postcard,
                Some(100),
            ),
            (
                TestMessage::new(2, "json", vec![2, 3]),
                Protocol::Json,
                Some(200),
            ),
            (
                TestMessage::new(3, "no_stream", vec![4, 5, 6]),
                Protocol::Postcard,
                None,
            ),
            (
                TestMessage::new(4, "json_stream", vec![7]),
                Protocol::Json,
                Some(300),
            ),
        ];

        let mut all_data = Vec::new();
        for (msg, protocol, stream_id) in &test_cases {
            let temp_io = AsyncMockIo::new();
            let mut temp_transport = AsyncMessageTransport::new(temp_io).with_crc(true);
            temp_transport
                .send_message(msg, Some(*protocol), *stream_id)
                .await
                .unwrap();
            all_data.extend_from_slice(temp_transport.inner().written_data());
        }

        let mock_io = AsyncMockIo::with_data(all_data);
        let mut transport = AsyncMessageTransport::new(mock_io).with_crc(true);

        for (expected_msg, expected_protocol, expected_stream_id) in test_cases {
            let (received, protocol, stream_id): (TestMessage, _, _) =
                transport.receive_message().await.unwrap();
            assert_eq!(received, expected_msg);
            assert_eq!(protocol, expected_protocol);
            assert_eq!(stream_id, expected_stream_id);
        }
    }

    #[tokio::test]
    async fn test_async_crc_corruption() {
        let message = TestMessage::new(1, "crc_test", vec![1, 2, 3, 4, 5]);
        let mut temp_transport = AsyncMessageTransport::new(AsyncMockIo::new());
        temp_transport = temp_transport.with_crc(true);
        temp_transport
            .send_message(&message, Some(Protocol::Postcard), None)
            .await
            .unwrap();

        // Test with correct CRC
        let mut data = temp_transport.inner().written_data().to_vec();
        let mock_io = AsyncMockIo::with_data(data.clone());
        let mut transport = AsyncMessageTransport::new(mock_io).with_crc(true);
        let (received, _, _): (TestMessage, _, _) = transport.receive_message().await.unwrap();
        assert_eq!(received, message);

        // Test with corrupted CRC
        if data.len() >= 4 {
            let len = data.len();
            data[len - 1] ^= 0xFF; // Corrupt last byte (CRC)
            let mock_io_corrupted = AsyncMockIo::with_data(data);
            let mut transport_corrupted =
                AsyncMessageTransport::new(mock_io_corrupted).with_crc(true);
            let result: Result<(TestMessage, _, _), _> =
                transport_corrupted.receive_message().await;
            assert!(matches!(result, Err(TransportError::CrcMismatch)));
        }
    }

    #[tokio::test]
    async fn test_async_boundary_conditions() {
        let mut transport = AsyncMessageTransport::new(AsyncMockIo::new());

        // Empty message
        let empty_msg = TestMessage::new(0, "", vec![]);
        transport
            .send_message(&empty_msg, Some(Protocol::Json), None)
            .await
            .unwrap();
        transport.inner_mut().reset_read();
        let (received, _, _): (TestMessage, _, _) = transport.receive_message().await.unwrap();
        assert_eq!(received, empty_msg);

        // Maximum stream ID
        let stream_msg = TestMessage::new(1, "max_stream", vec![1]);
        transport
            .send_message(&stream_msg, Some(Protocol::Postcard), Some(u16::MAX))
            .await
            .unwrap();
        transport.inner_mut().reset_read();
        let (received, _, stream_id): (TestMessage, _, _) =
            transport.receive_message().await.unwrap();
        assert_eq!(received, stream_msg);
        assert_eq!(stream_id, Some(u16::MAX));
    }

    #[tokio::test]
    async fn test_async_performance() {
        use std::time::Instant;

        let iterations = 1000;
        let start = Instant::now();
        let mut transport = AsyncMessageTransport::new(AsyncMockIo::new());

        for i in 0..iterations {
            let test_msg = TestMessage::new(i, "perf", vec![0u8; 100]);
            transport
                .send_message(&test_msg, Some(Protocol::Postcard), None)
                .await
                .unwrap();
        }

        let duration = start.elapsed();
        assert!(
            duration.as_millis() < 1000,
            "Async performance too slow: {:?}",
            duration
        );
    }
}

#[cfg(test)]
mod sync_tests {
    use alloc::vec::Vec;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestMessage {
        id: u32,
        name: alloc::string::String,
        data: Vec<u8>,
    }

    impl TestMessage {
        fn new(id: u32, name: &str, data: Vec<u8>) -> Self {
            Self {
                id,
                name: name.into(),
                data,
            }
        }
    }

    /// Mock IO for sync testing
    #[derive(Debug)]
    struct SyncMockIo {
        read_data: Vec<u8>,
        write_data: Vec<u8>,
        read_pos: usize,
        read_chunk_size: Option<usize>,
        fail_after_bytes: Option<usize>,
        bytes_written: usize,
    }

    impl SyncMockIo {
        fn new() -> Self {
            Self {
                read_data: Vec::new(),
                write_data: Vec::new(),
                read_pos: 0,
                read_chunk_size: None,
                fail_after_bytes: None,
                bytes_written: 0,
            }
        }

        fn with_data(data: Vec<u8>) -> Self {
            Self {
                read_data: data,
                write_data: Vec::new(),
                read_pos: 0,
                read_chunk_size: None,
                fail_after_bytes: None,
                bytes_written: 0,
            }
        }

        fn with_chunk_size(mut self, chunk_size: usize) -> Self {
            self.read_chunk_size = Some(chunk_size);
            self
        }

        fn written_data(&self) -> &[u8] {
            &self.write_data
        }

        fn reset_read(&mut self) {
            self.read_pos = 0;
            self.read_data = self.write_data.clone();
            self.write_data.clear();
            self.bytes_written = 0;
        }
    }

    impl embedded_io::ErrorType for SyncMockIo {
        type Error = embedded_io::ErrorKind;
    }

    impl embedded_io::Read for SyncMockIo {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let available = self.read_data.len() - self.read_pos;
            if available == 0 {
                return Ok(0);
            }

            let max_read = if let Some(chunk_size) = self.read_chunk_size {
                buf.len().min(chunk_size).min(available)
            } else {
                buf.len().min(available)
            };

            buf[..max_read]
                .copy_from_slice(&self.read_data[self.read_pos..self.read_pos + max_read]);
            self.read_pos += max_read;
            Ok(max_read)
        }
    }

    impl embedded_io::Write for SyncMockIo {
        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            if let Some(fail_after) = self.fail_after_bytes {
                if self.bytes_written + buf.len() > fail_after {
                    return Err(embedded_io::ErrorKind::Other);
                }
            }

            self.write_data.extend_from_slice(buf);
            self.bytes_written += buf.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn test_sync_sticky_packet_scenario() {
        let messages = (1..=5)
            .map(|i| TestMessage::new(i, &format!("msg{}", i), vec![i as u8; i as usize]))
            .collect::<Vec<_>>();

        // Serialize all messages into one buffer
        let mut all_data = Vec::new();
        for msg in &messages {
            let temp_io = SyncMockIo::new();
            let mut temp_transport = SyncMessageTransport::new(temp_io);
            temp_transport
                .send_message(msg, Some(Protocol::Json), None)
                .unwrap();
            all_data.extend_from_slice(temp_transport.inner().written_data());
        }

        let mock_io = SyncMockIo::with_data(all_data);
        let mut transport = SyncMessageTransport::new(mock_io);

        // Read all messages back
        for expected in messages {
            let (received, protocol, _): (TestMessage, _, _) = transport.receive_message().unwrap();
            assert_eq!(received, expected);
            assert_eq!(protocol, Protocol::Json);
        }
    }

    #[test]
    fn test_sync_fragmented_packet_scenario() {
        let large_data = vec![0x42u8; 5000];
        let message = TestMessage::new(1, "fragmented", large_data.clone());

        // Serialize normally
        let mut temp_transport = SyncMessageTransport::new(SyncMockIo::new());
        temp_transport
            .send_message(&message, Some(Protocol::Postcard), None)
            .unwrap();
        let full_data = temp_transport.inner().written_data().to_vec();

        // Read with fragmentation
        let mock_io = SyncMockIo::with_data(full_data).with_chunk_size(32);
        let mut transport = SyncMessageTransport::new(mock_io);

        let (received, _, _): (TestMessage, _, _) = transport.receive_message().unwrap();
        assert_eq!(received.data, large_data);
    }

    #[test]
    fn test_sync_crc_comprehensive() {
        let test_patterns = vec![
            vec![0u8; 1],                  // Minimal
            vec![0xFFu8; 1000],            // Repeated
            (0..255).collect::<Vec<u8>>(), // Sequential
            [0x55, 0xAA].repeat(500),  // Alternating
        ];

        for (i, test_data) in test_patterns.into_iter().enumerate() {
            let message = TestMessage::new(i as u32, &format!("crc_test_{}", i), test_data.clone());

            let mut temp_transport = SyncMessageTransport::new(SyncMockIo::new()).with_crc(true);
            temp_transport
                .send_message(&message, Some(Protocol::Postcard), None)
                .unwrap();

            // Test correct CRC
            let mut correct_data = temp_transport.inner().written_data().to_vec();
            let mock_io = SyncMockIo::with_data(correct_data.clone());
            let mut transport = SyncMessageTransport::new(mock_io).with_crc(true);
            let (received, _, _): (TestMessage, _, _) = transport.receive_message().unwrap();
            assert_eq!(received.data, test_data);

            // Test corrupted CRC
            if !correct_data.is_empty() {
                let last_idx = correct_data.len() - 1;
                correct_data[last_idx] ^= 0xFF;
                let mock_io_corrupted = SyncMockIo::with_data(correct_data);
                let mut transport_corrupted =
                    SyncMessageTransport::new(mock_io_corrupted).with_crc(true);
                let result: Result<(TestMessage, _, _), _> = transport_corrupted.receive_message();
                assert!(matches!(result, Err(TransportError::CrcMismatch)));
            }
        }
    }

    #[test]
    fn test_sync_boundary_conditions() {
        let mut transport = SyncMessageTransport::new(SyncMockIo::new());

        // Empty message
        let empty_msg = TestMessage::new(0, "", vec![]);
        transport
            .send_message(&empty_msg, Some(Protocol::Json), None)
            .unwrap();
        transport.inner_mut().reset_read();
        let (received, _, _): (TestMessage, _, _) = transport.receive_message().unwrap();
        assert_eq!(received, empty_msg);

        // Large message (but within limits)
        let large_data = vec![0u8; 100_000];
        let large_msg = TestMessage::new(1, "large", large_data.clone());
        transport
            .send_message(&large_msg, Some(Protocol::Postcard), None)
            .unwrap();
        transport.inner_mut().reset_read();
        let (received, _, _): (TestMessage, _, _) = transport.receive_message().unwrap();
        assert_eq!(received.data.len(), large_data.len());
    }

    #[test]
    fn test_sync_frame_header_edge_cases() {
        use crate::transport::frame::FrameHeader;

        // Minimum valid header
        let header = FrameHeader::new(Protocol::Postcard, None, 0, false);
        let encoded = header.encode();
        let (decoded, size) = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.protocol, Protocol::Postcard);
        assert_eq!(decoded.stream_id, None);
        assert_eq!(size, encoded.len());

        // Maximum values
        let header = FrameHeader::new(Protocol::Json, Some(u16::MAX), u32::MAX, true);
        let encoded = header.encode();
        let (decoded, _) = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_length, u32::MAX);
        assert_eq!(decoded.stream_id, Some(u16::MAX));
        assert!(decoded.flags.has_crc());

        // Invalid protocol
        let mut invalid_data = encoded;
        invalid_data[0] = 99; // Invalid protocol
        assert!(FrameHeader::decode(&invalid_data).is_err());
    }

    #[test]
    fn test_sync_transport_configuration() {
        let mock_io = SyncMockIo::new();
        let transport = SyncMessageTransport::new(mock_io)
            .with_default_protocol(Protocol::Json)
            .with_crc(false);

        assert_eq!(transport.default_protocol(), Protocol::Json);
        assert!(!transport.is_crc_enabled());
    }

    #[test]
    fn test_sync_performance() {
        use std::time::Instant;

        let iterations = 500;
        let start = Instant::now();
        let mut transport = SyncMessageTransport::new(SyncMockIo::new());

        for i in 0..iterations {
            let test_msg = TestMessage::new(i, "perf", vec![0u8; 50]);
            transport
                .send_message(&test_msg, Some(Protocol::Postcard), None)
                .unwrap();
        }

        let duration = start.elapsed();
        assert!(
            duration.as_millis() < 500,
            "Sync performance too slow: {:?}",
            duration
        );
    }
}

use core::fmt::Debug;

use alloc::vec::Vec;

use embedded_io::{ErrorType as SyncErrorType, Read as SyncRead, Write as SyncWrite};
use embedded_io_async::{ErrorType as AsyncErrorType, Read as AsyncRead, Write as AsyncWrite};
use serde::{Deserialize, Serialize};

use super::crc::crc32;
use super::error::TransportError;
use super::frame::FrameHeader;
use super::protocol::Protocol;
use super::serializers;
use super::{DEFAULT_BUFFER_SIZE, MAX_MESSAGE_SIZE};

#[derive(Debug)]
pub struct AsyncMessageTransport<IO> {
    io: IO,
    rx_buffer: Vec<u8>,
    default_protocol: Protocol,
    enable_crc: bool,
}

#[derive(Debug)]
pub struct SyncMessageTransport<IO> {
    io: IO,
    rx_buffer: Vec<u8>,
    default_protocol: Protocol,
    enable_crc: bool,
}

macro_rules! impl_transport_common {
    ($transport:ident) => {
        impl<IO> $transport<IO> {
            pub fn new(io: IO) -> Self {
                Self {
                    io,
                    rx_buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
                    default_protocol: Protocol::default(),
                    enable_crc: true,
                }
            }

            pub fn with_default_protocol(mut self, protocol: Protocol) -> Self {
                self.default_protocol = protocol;
                self
            }

            pub fn with_crc(mut self, enable: bool) -> Self {
                self.enable_crc = enable;
                self
            }

            pub fn inner(&self) -> &IO {
                &self.io
            }

            pub fn inner_mut(&mut self) -> &mut IO {
                &mut self.io
            }

            pub fn into_inner(self) -> IO {
                self.io
            }

            pub fn default_protocol(&self) -> Protocol {
                self.default_protocol
            }

            pub fn is_crc_enabled(&self) -> bool {
                self.enable_crc
            }

            pub fn clear_rx_buffer(&mut self) {
                self.rx_buffer.clear();
            }

            pub fn rx_buffer_len(&self) -> usize {
                self.rx_buffer.len()
            }
        }
    };
}

impl_transport_common!(AsyncMessageTransport);
impl_transport_common!(SyncMessageTransport);

impl<IO> AsyncMessageTransport<IO>
where
    IO: AsyncRead + AsyncWrite + AsyncErrorType,
    IO::Error: Debug,
{
    pub async fn send_message<T: Serialize>(
        &mut self,
        message: &T,
        protocol: Option<Protocol>,
        stream_id: Option<u16>,
    ) -> Result<(), TransportError> {
        let protocol = protocol.unwrap_or(self.default_protocol);
        let payload = serializers::serialize(protocol, message)?;

        if payload.len() > MAX_MESSAGE_SIZE {
            return Err(TransportError::MessageTooLarge(payload.len()));
        }

        let header = FrameHeader::new(protocol, stream_id, payload.len() as u32, self.enable_crc);
        let header_bytes = header.encode();

        let mut frame = Vec::with_capacity(header_bytes.len() + payload.len() + 4);
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload);

        if self.enable_crc {
            let crc = crc32(&payload);
            frame.extend_from_slice(&crc.to_be_bytes());
        }

        self.io
            .write_all(&frame)
            .await
            .map_err(|e| TransportError::Io(alloc::format!("{:?}", e)))?;

        Ok(())
    }

    pub async fn receive_message<T: for<'de> Deserialize<'de>>(
        &mut self,
    ) -> Result<(T, Protocol, Option<u16>), TransportError> {
        self.ensure_buffer_has_async(FrameHeader::MIN_SIZE).await?;
        let (header, header_size) = FrameHeader::decode(&self.rx_buffer)?;

        let total_frame_size = header.total_frame_size();
        self.ensure_buffer_has_async(total_frame_size).await?;

        let payload_start = header_size;
        let payload_end = payload_start + header.payload_length as usize;
        let payload = &self.rx_buffer[payload_start..payload_end];

        if header.flags.has_crc() {
            let expected_crc = u32::from_be_bytes([
                self.rx_buffer[payload_end],
                self.rx_buffer[payload_end + 1],
                self.rx_buffer[payload_end + 2],
                self.rx_buffer[payload_end + 3],
            ]);
            let actual_crc = crc32(payload);
            if expected_crc != actual_crc {
                return Err(TransportError::CrcMismatch);
            }
        }

        let message = serializers::deserialize(header.protocol, payload)?;
        self.rx_buffer.drain(..total_frame_size);

        Ok((message, header.protocol, header.stream_id))
    }

    async fn ensure_buffer_has_async(&mut self, required: usize) -> Result<(), TransportError> {
        while self.rx_buffer.len() < required {
            let mut temp_buf = [0u8; 1024];
            let n = self
                .io
                .read(&mut temp_buf)
                .await
                .map_err(|e| TransportError::Io(alloc::format!("{:?}", e)))?;
            if n == 0 {
                return Err(TransportError::Io("Unexpected EOF".into()));
            }
            self.rx_buffer.extend_from_slice(&temp_buf[..n]);
        }
        Ok(())
    }
}

impl<IO> SyncMessageTransport<IO>
where
    IO: SyncRead + SyncWrite + SyncErrorType,
    IO::Error: Debug,
{
    pub fn send_message<T: Serialize>(
        &mut self,
        message: &T,
        protocol: Option<Protocol>,
        stream_id: Option<u16>,
    ) -> Result<(), TransportError> {
        let protocol = protocol.unwrap_or(self.default_protocol);
        let payload = serializers::serialize(protocol, message)?;

        if payload.len() > MAX_MESSAGE_SIZE {
            return Err(TransportError::MessageTooLarge(payload.len()));
        }

        let header = FrameHeader::new(protocol, stream_id, payload.len() as u32, self.enable_crc);
        let header_bytes = header.encode();

        let mut frame = Vec::with_capacity(header_bytes.len() + payload.len() + 4);
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload);

        if self.enable_crc {
            let crc = crc32(&payload);
            frame.extend_from_slice(&crc.to_be_bytes());
        }

        self.io
            .write_all(&frame)
            .map_err(|e| TransportError::Io(alloc::format!("{:?}", e)))?;

        Ok(())
    }

    pub fn receive_message<T: for<'de> Deserialize<'de>>(
        &mut self,
    ) -> Result<(T, Protocol, Option<u16>), TransportError> {
        self.ensure_buffer_has_sync(FrameHeader::MIN_SIZE)?;
        let (header, header_size) = FrameHeader::decode(&self.rx_buffer)?;

        let total_frame_size = header.total_frame_size();
        self.ensure_buffer_has_sync(total_frame_size)?;

        let payload_start = header_size;
        let payload_end = payload_start + header.payload_length as usize;
        let payload = &self.rx_buffer[payload_start..payload_end];

        if header.flags.has_crc() {
            let expected_crc = u32::from_be_bytes([
                self.rx_buffer[payload_end],
                self.rx_buffer[payload_end + 1],
                self.rx_buffer[payload_end + 2],
                self.rx_buffer[payload_end + 3],
            ]);
            let actual_crc = crc32(payload);
            if expected_crc != actual_crc {
                return Err(TransportError::CrcMismatch);
            }
        }

        let message = serializers::deserialize(header.protocol, payload)?;
        self.rx_buffer.drain(..total_frame_size);

        Ok((message, header.protocol, header.stream_id))
    }

    fn ensure_buffer_has_sync(&mut self, required: usize) -> Result<(), TransportError> {
        while self.rx_buffer.len() < required {
            let mut temp_buf = [0u8; 1024];
            let n = self
                .io
                .read(&mut temp_buf)
                .map_err(|e| TransportError::Io(alloc::format!("{:?}", e)))?;
            if n == 0 {
                return Err(TransportError::Io("Unexpected EOF".into()));
            }
            self.rx_buffer.extend_from_slice(&temp_buf[..n]);
        }
        Ok(())
    }
}

use alloc::vec::Vec;

use super::error::TransportError;
use super::protocol::Protocol;

#[derive(Debug, Clone, Copy)]
pub struct FrameFlags(u8);

impl FrameFlags {
    const CRC: u8 = 0b0000_0001;
    const STREAM: u8 = 0b0000_0010;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_crc(mut self) -> Self {
        self.0 |= Self::CRC;
        self
    }

    pub fn with_stream(mut self) -> Self {
        self.0 |= Self::STREAM;
        self
    }

    pub fn has_crc(&self) -> bool {
        (self.0 & Self::CRC) != 0
    }

    pub fn has_stream(&self) -> bool {
        (self.0 & Self::STREAM) != 0
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }

    pub fn from_u8(value: u8) -> Self {
        Self(value)
    }
}

impl Default for FrameFlags {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub protocol: Protocol,
    pub flags: FrameFlags,
    pub stream_id: Option<u16>,
    pub payload_length: u32,
}

impl FrameHeader {
    /// Minimum header size: protocol(1) + flags(1) + length(4)
    pub const MIN_SIZE: usize = 6;

    pub fn new(
        protocol: Protocol,
        stream_id: Option<u16>,
        payload_length: u32,
        enable_crc: bool,
    ) -> Self {
        let mut flags = FrameFlags::new();
        if stream_id.is_some() {
            flags = flags.with_stream();
        }
        if enable_crc {
            flags = flags.with_crc();
        }

        Self {
            protocol,
            flags,
            stream_id,
            payload_length,
        }
    }

    /// Encodes header to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.size());

        buffer.push(self.protocol as u8);
        buffer.push(self.flags.as_u8());

        if let Some(stream_id) = self.stream_id {
            buffer.extend_from_slice(&stream_id.to_be_bytes());
        }

        buffer.extend_from_slice(&self.payload_length.to_be_bytes());

        buffer
    }

    /// Decodes header from bytes, returns (header, bytes_consumed)
    pub fn decode(data: &[u8]) -> Result<(Self, usize), TransportError> {
        if data.len() < Self::MIN_SIZE {
            return Err(TransportError::Protocol(
                "Insufficient data for frame header".into(),
            ));
        }

        let protocol = Protocol::from_u8(data[0])?;
        let flags = FrameFlags::from_u8(data[1]);
        let mut offset = 2;

        let stream_id = if flags.has_stream() {
            if data.len() < offset + 2 {
                return Err(TransportError::Protocol(
                    "Insufficient data for stream ID".into(),
                ));
            }
            let id = u16::from_be_bytes([data[offset], data[offset + 1]]);
            offset += 2;
            Some(id)
        } else {
            None
        };

        if data.len() < offset + 4 {
            return Err(TransportError::Protocol(
                "Insufficient data for payload length".into(),
            ));
        }

        let payload_length = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        Ok((
            Self {
                protocol,
                flags,
                stream_id,
                payload_length,
            },
            offset,
        ))
    }

    /// Returns header size in bytes
    pub fn size(&self) -> usize {
        let mut size = Self::MIN_SIZE;
        if self.stream_id.is_some() {
            size += 2;
        }
        size
    }

    /// Returns total frame size including payload and CRC
    pub fn total_frame_size(&self) -> usize {
        self.size() + self.payload_length as usize + if self.flags.has_crc() { 4 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_flags() {
        let flags = FrameFlags::new();
        assert!(!flags.has_crc());
        assert!(!flags.has_stream());

        let flags = flags.with_crc().with_stream();
        assert!(flags.has_crc());
        assert!(flags.has_stream());

        let roundtrip = FrameFlags::from_u8(flags.as_u8());
        assert_eq!(roundtrip.has_crc(), flags.has_crc());
        assert_eq!(roundtrip.has_stream(), flags.has_stream());
    }

    #[test]
    fn test_frame_header_encoding() {
        let header = FrameHeader::new(Protocol::Json, Some(42), 1024, true);
        let encoded = header.encode();
        let (decoded, size) = FrameHeader::decode(&encoded).unwrap();

        assert_eq!(decoded.protocol, header.protocol);
        assert_eq!(decoded.stream_id, header.stream_id);
        assert_eq!(decoded.payload_length, header.payload_length);
        assert_eq!(decoded.flags.has_crc(), header.flags.has_crc());
        assert_eq!(size, encoded.len());
    }

    #[test]
    fn test_frame_header_no_stream() {
        let header = FrameHeader::new(Protocol::Postcard, None, 512, false);
        let encoded = header.encode();
        let (decoded, size) = FrameHeader::decode(&encoded).unwrap();

        assert_eq!(decoded.protocol, Protocol::Postcard);
        assert_eq!(decoded.stream_id, None);
        assert_eq!(decoded.payload_length, 512);
        assert!(!decoded.flags.has_crc());
        assert!(!decoded.flags.has_stream());
        assert_eq!(size, FrameHeader::MIN_SIZE);
    }

    #[test]
    fn test_frame_size_calculation() {
        let header1 = FrameHeader::new(Protocol::Postcard, None, 100, false);
        assert_eq!(header1.size(), FrameHeader::MIN_SIZE);
        assert_eq!(header1.total_frame_size(), FrameHeader::MIN_SIZE + 100);

        let header2 = FrameHeader::new(Protocol::Json, Some(42), 100, false);
        assert_eq!(header2.size(), FrameHeader::MIN_SIZE + 2);
        assert_eq!(header2.total_frame_size(), FrameHeader::MIN_SIZE + 2 + 100);

        let header3 = FrameHeader::new(Protocol::Postcard, None, 100, true);
        assert_eq!(header3.total_frame_size(), FrameHeader::MIN_SIZE + 100 + 4);
    }

    #[test]
    fn test_frame_header_decode_errors() {
        let short_data = [1, 2];
        assert!(FrameHeader::decode(&short_data).is_err());

        let mut data = vec![1, FrameFlags::new().with_stream().as_u8()];
        data.extend_from_slice(&[0, 0, 0, 100]);
        assert!(FrameHeader::decode(&data).is_err());

        let invalid_proto = [99, 0, 0, 0, 0, 100];
        assert!(FrameHeader::decode(&invalid_proto).is_err());
    }
}

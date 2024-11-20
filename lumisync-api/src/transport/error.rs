use core::fmt;

use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// I/O operation failed
    Io(String),
    /// Message serialization failed
    Serialization(String),
    /// Message deserialization failed
    Deserialization(String),
    /// Buffer capacity exceeded
    BufferFull,
    /// Unknown protocol identifier
    UnknownProtocol(u8),
    /// Message exceeds size limit
    MessageTooLarge(usize),
    /// Protocol format violation
    Protocol(String),
    /// CRC checksum validation failed
    CrcMismatch,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Serialization(e) => write!(f, "Serialization error: {}", e),
            Self::Deserialization(e) => write!(f, "Deserialization error: {}", e),
            Self::BufferFull => write!(f, "Buffer full"),
            Self::UnknownProtocol(p) => write!(f, "Unknown protocol: {}", p),
            Self::MessageTooLarge(size) => write!(f, "Message too large: {} bytes", size),
            Self::Protocol(e) => write!(f, "Protocol error: {}", e),
            Self::CrcMismatch => write!(f, "CRC checksum mismatch"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}

pub type Result<T> = core::result::Result<T, TransportError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(
            TransportError::UnknownProtocol(99).to_string(),
            "Unknown protocol: 99"
        );
        assert_eq!(
            TransportError::MessageTooLarge(1000000).to_string(),
            "Message too large: 1000000 bytes"
        );
        assert_eq!(
            TransportError::CrcMismatch.to_string(),
            "CRC checksum mismatch"
        );
    }
}

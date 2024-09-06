use core::fmt;

#[derive(Debug)]
pub enum Error {
    DeviceNotFound,
    InvalidCommand,
    InvalidState,
    NetworkError,
    SerializationError,
    TimeoutError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DeviceNotFound => write!(f, "Device not found"),
            Error::InvalidCommand => write!(f, "Invalid command"),
            Error::InvalidState => write!(f, "Invalid state"),
            Error::NetworkError => write!(f, "Network error"),
            Error::SerializationError => write!(f, "Serialization error"),
            Error::TimeoutError => write!(f, "Timeout error"),
        }
    }
}

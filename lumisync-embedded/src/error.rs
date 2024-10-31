use core::fmt;

#[derive(Debug, Clone)]
pub enum Error {
    DeviceNotFound,
    InvalidCommand,
    InvalidState,
    NetworkError,
    SerializationError,
    TimeoutError,
    NotConnected,
    SensorReadingOutOfRange,
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
            Error::NotConnected => write!(f, "Not connected"),
            Error::SensorReadingOutOfRange => write!(f, "Sensor reading out of valid range"),
        }
    }
}

impl embedded_hal_nb::serial::Error for Error {
    fn kind(&self) -> embedded_hal_nb::serial::ErrorKind {
        match self {
            Error::DeviceNotFound => embedded_hal_nb::serial::ErrorKind::Other,
            Error::InvalidCommand => embedded_hal_nb::serial::ErrorKind::Other,
            Error::InvalidState => embedded_hal_nb::serial::ErrorKind::Other,
            Error::NetworkError => embedded_hal_nb::serial::ErrorKind::Other,
            Error::SerializationError => embedded_hal_nb::serial::ErrorKind::Other,
            Error::TimeoutError => embedded_hal_nb::serial::ErrorKind::Other,
            Error::NotConnected => embedded_hal_nb::serial::ErrorKind::Other,
            Error::SensorReadingOutOfRange => embedded_hal_nb::serial::ErrorKind::Other,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

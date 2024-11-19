use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// Network-related errors
    Network {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    /// Device communication errors
    Device {
        message: String,
        kind: DeviceErrorKind,
    },
    /// Data serialization/deserialization errors
    Serialization { message: String },
    /// Connection timeout or failure
    Connection {
        message: String,
        endpoint: Option<String>,
    },
    /// Resource not found
    NotFound { resource: String },
}

#[derive(Debug, Clone)]
pub enum DeviceErrorKind {
    Stepper,
    Wifi,
    Generic,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn network<S: Into<String>>(message: S) -> Self {
        Self::Network {
            message: message.into(),
            source: None,
        }
    }

    pub fn network_with_source<S: Into<String>>(
        message: S,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::Network {
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn device<S: Into<String>>(message: S, kind: DeviceErrorKind) -> Self {
        Self::Device {
            message: message.into(),
            kind,
        }
    }

    pub fn stepper<S: Into<String>>(message: S) -> Self {
        Self::Device {
            message: message.into(),
            kind: DeviceErrorKind::Stepper,
        }
    }

    pub fn wifi<S: Into<String>>(message: S) -> Self {
        Self::Device {
            message: message.into(),
            kind: DeviceErrorKind::Wifi,
        }
    }

    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    pub fn connection<S: Into<String>>(message: S) -> Self {
        Self::Connection {
            message: message.into(),
            endpoint: None,
        }
    }

    pub fn connection_to<S: Into<String>, E: Into<String>>(message: S, endpoint: E) -> Self {
        Self::Connection {
            message: message.into(),
            endpoint: Some(endpoint.into()),
        }
    }

    pub fn not_found<S: Into<String>>(resource: S) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Network { message, .. } => write!(f, "Network error: {}", message),
            Error::Device { message, kind } => write!(f, "{:?} device error: {}", kind, message),
            Error::Serialization { message } => write!(f, "Serialization error: {}", message),
            Error::Connection { message, endpoint } => {
                if let Some(ep) = endpoint {
                    write!(f, "Connection error to {}: {}", ep, message)
                } else {
                    write!(f, "Connection error: {}", message)
                }
            }
            Error::NotFound { resource } => write!(f, "Resource not found: {}", resource),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Network {
                source: Some(source),
                ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::serialization(err.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::network_with_source("HTTP request failed", Box::new(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::connection(err.to_string())
    }
}

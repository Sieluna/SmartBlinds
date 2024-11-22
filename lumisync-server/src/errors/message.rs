use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Service already started")]
    AlreadyStarted,

    #[error("Invalid edge source")]
    InvalidEdgeSource,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Message routing failed")]
    RoutingFailed,

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Invalid message format")]
    InvalidMessageFormat,

    #[error("Message timeout")]
    MessageTimeout,
}

impl MessageError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            MessageError::Serialization(_) => StatusCode::BAD_REQUEST,
            MessageError::AlreadyStarted => StatusCode::CONFLICT,
            MessageError::InvalidEdgeSource => StatusCode::BAD_REQUEST,
            MessageError::ChannelClosed => StatusCode::INTERNAL_SERVER_ERROR,
            MessageError::RoutingFailed => StatusCode::INTERNAL_SERVER_ERROR,
            MessageError::TransportError(_) => StatusCode::BAD_GATEWAY,
            MessageError::ProtocolError(_) => StatusCode::BAD_REQUEST,
            MessageError::InvalidMessageFormat => StatusCode::BAD_REQUEST,
            MessageError::MessageTimeout => StatusCode::REQUEST_TIMEOUT,
        }
    }
}

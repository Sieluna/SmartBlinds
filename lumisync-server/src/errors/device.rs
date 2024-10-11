use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Device not found")]
    DeviceNotFound,

    #[error("Device name already exists")]
    DeviceNameExists,

    #[error("Invalid device type")]
    InvalidDeviceType,

    #[error("Invalid device status")]
    InvalidDeviceStatus,

    #[error("Invalid request parameters")]
    InvalidRequest,

    #[error("Insufficient permission")]
    InsufficientPermission,

    #[error("Invalid device setting")]
    InvalidDeviceSetting,

    #[error("Invalid device record")]
    InvalidDeviceRecord,
}

impl DeviceError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            DeviceError::DeviceNotFound => StatusCode::NOT_FOUND,
            DeviceError::DeviceNameExists => StatusCode::CONFLICT,
            DeviceError::InvalidDeviceType => StatusCode::BAD_REQUEST,
            DeviceError::InvalidDeviceStatus => StatusCode::BAD_REQUEST,
            DeviceError::InvalidRequest => StatusCode::BAD_REQUEST,
            DeviceError::InsufficientPermission => StatusCode::FORBIDDEN,
            DeviceError::InvalidDeviceSetting => StatusCode::BAD_REQUEST,
            DeviceError::InvalidDeviceRecord => StatusCode::BAD_REQUEST,
        }
    }
}

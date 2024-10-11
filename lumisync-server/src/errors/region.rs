use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum RegionError {
    #[error("Region not found")]
    RegionNotFound,

    #[error("Region name already exists")]
    RegionNameExists,

    #[error("Invalid request parameters")]
    InvalidRequest,

    #[error("Insufficient permission")]
    InsufficientPermission,

    #[error("Invalid environment data")]
    InvalidEnvironmentData,

    #[error("Invalid region setting")]
    InvalidRegionSetting,

    #[error("Region device limit exceeded")]
    RegionDeviceLimitExceeded,
}

impl RegionError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            RegionError::RegionNotFound => StatusCode::NOT_FOUND,
            RegionError::RegionNameExists => StatusCode::CONFLICT,
            RegionError::InvalidRequest => StatusCode::BAD_REQUEST,
            RegionError::InsufficientPermission => StatusCode::FORBIDDEN,
            RegionError::InvalidEnvironmentData => StatusCode::BAD_REQUEST,
            RegionError::InvalidRegionSetting => StatusCode::BAD_REQUEST,
            RegionError::RegionDeviceLimitExceeded => StatusCode::BAD_REQUEST,
        }
    }
}

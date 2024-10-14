use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum SettingError {
    #[error("Setting not found")]
    SettingNotFound,

    #[error("Invalid time range")]
    InvalidTimeRange,

    #[error("Invalid device setting format")]
    InvalidDeviceSettingFormat,

    #[error("Invalid light range")]
    InvalidLightRange,

    #[error("Invalid temperature range")]
    InvalidTemperatureRange,

    #[error("Invalid humidity range")]
    InvalidHumidityRange,
}

impl SettingError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            SettingError::SettingNotFound => StatusCode::NOT_FOUND,
            SettingError::InvalidTimeRange => StatusCode::BAD_REQUEST,
            SettingError::InvalidDeviceSettingFormat => StatusCode::BAD_REQUEST,
            SettingError::InvalidLightRange => StatusCode::BAD_REQUEST,
            SettingError::InvalidTemperatureRange => StatusCode::BAD_REQUEST,
            SettingError::InvalidHumidityRange => StatusCode::BAD_REQUEST,
        }
    }
}

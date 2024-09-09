use esp_idf_svc::sys::EspError;

#[derive(thiserror::Error, Debug)]
pub enum SmartBlindsError {
    #[error("WiFi connection failed: {0}")]
    WifiConnection(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("System error: {0}")]
    System(String),
}

impl From<EspError> for SmartBlindsError {
    fn from(err: EspError) -> Self {
        SmartBlindsError::System(format!("ESP error: {}", err))
    }
}

pub type Result<T> = core::result::Result<T, SmartBlindsError>;

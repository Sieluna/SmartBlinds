use serde::{Deserialize, Serialize};

/// Error Code Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Invalid request parameters or format
    InvalidRequest,
    /// Target device is not connected
    DeviceOffline,
    /// Requesting entity lacks permission
    PermissionDenied,
    /// Operation exceeds system limits
    OverLimit,
    /// System internal processing error
    InternalError,
    /// Physical hardware failure
    HardwareFailure,
    /// Network communication error
    NetworkError,
    /// Device battery critically low
    BatteryLow,
    /// Operation timed out
    Timeout,
}

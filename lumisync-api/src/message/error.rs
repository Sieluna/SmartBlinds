use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid input parameters.
    InvalidRequest,
    /// Device not connected.
    DeviceOffline,
    /// Operation not allowed.
    PermissionDenied,
    /// Resource limit exceeded.
    OverLimit,
    /// Internal processing error.
    InternalError,
    /// Device hardware error.
    HardwareFailure,
    /// Communication error.
    NetworkError,
    /// Critical battery level.
    BatteryLow,
    /// Operation time exceeded.
    Timeout,
}

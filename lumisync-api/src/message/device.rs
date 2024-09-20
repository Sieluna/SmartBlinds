use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::error::ErrorCode;
use super::{Priority, WindowData};

/// Device Message Frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFrame {
    /// Message metadata
    pub header: DeviceHeader,
    /// Message content
    pub payload: DevicePayload,
}

/// Device Message Header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHeader {
    /// Message unique identifier
    pub id: Uuid,
    /// Message timestamp
    pub timestamp: OffsetDateTime,
    /// Message priority
    pub priority: Priority,
}

/// Device Payload Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevicePayload {
    /// Command to control a device
    Command(DeviceCommand),
    /// Status report from a device
    Status(DeviceStatus),
    /// Error report from a device
    Error(DeviceError),
}

/// Device Command Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceCommand {
    /// Set device position
    SetWindow {
        /// Target window ID
        device_id: i32,
        /// Window data
        #[serde(flatten)]
        data: WindowData
    },
    /// Calibrate device
    Calibrate,
    /// Emergency stop
    EmergencyStop,
}

/// Device Status Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// Device identifier
    pub device_id: i32,
    /// Window data
    #[serde(flatten)]
    pub data: WindowData,
    /// Current position percentage
    pub position: u8,
    /// Battery level percentage
    pub battery: u8,
    /// Last update timestamp
    pub updated_at: OffsetDateTime,
}

/// Device Error Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceError {
    /// Device identifier
    pub device_id: i32,
    /// Error code
    pub code: ErrorCode,
    /// Error message
    pub message: String,
    /// Error timestamp
    pub timestamp: OffsetDateTime,
}

use alloc::string::String;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::error::ErrorCode;
use super::{Priority, SensorData, WindowData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFrame {
    /// Message metadata.
    pub header: DeviceHeader,
    /// Message content.
    pub payload: DevicePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHeader {
    /// Unique message identifier.
    pub id: Uuid,
    /// Message creation time.
    pub timestamp: OffsetDateTime,
    /// Message priority level.
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevicePayload {
    /// Control command for device.
    Command(DeviceCommand),
    /// Device status report.
    Status(DeviceStatus),
    /// Device error report.
    Error(DeviceError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceCommand {
    /// Set window position.
    SetWindow {
        /// Target device identifier.
        device_id: i32,
        /// Window position data.
        #[serde(flatten)]
        data: WindowData,
    },
    /// Start device calibration.
    Calibrate,
    /// Stop all operations.
    EmergencyStop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceType {
    /// Smart window device.
    Window,
    /// Environmental sensor.
    Sensor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceValue {
    /// Window position data.
    Window {
        /// Window identifier.
        window_id: i32,
        /// Window state data.
        data: WindowData,
    },
    /// Sensor reading data.
    Sensor {
        /// Sensor identifier.
        sensor_id: i32,
        /// Sensor readings.
        data: SensorData,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// Current device data.
    pub data: DeviceValue,
    /// Position percentage.
    pub position: u8,
    /// Battery level percentage.
    pub battery: u8,
    /// Last update time.
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceError {
    /// Device identifier.
    pub device_id: i32,
    /// Device category.
    pub device_type: DeviceType,
    /// Error type.
    pub code: ErrorCode,
    /// Error description.
    pub message: String,
    /// Error occurrence time.
    pub timestamp: OffsetDateTime,
}

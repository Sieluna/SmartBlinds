use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use serde_json;
use time::OffsetDateTime;

use super::{Id, SensorData, WindowData};

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeviceType {
    /// Smart window device
    Window,
    /// Environmental sensor
    Sensor,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeviceRequest {
    /// Parent region identifier
    pub region_id: Id,
    /// Device name
    pub name: String,
    /// Device category
    pub device_type: DeviceType,
    /// Device location data
    pub location: serde_json::Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    /// New device name
    pub name: Option<String>,
    /// New device category
    pub device_type: Option<DeviceType>,
    /// New location data
    pub location: Option<serde_json::Value>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecordResponse {
    /// Record identifier
    pub id: Id,
    /// Device identifier
    pub device_id: Id,
    /// Record data
    pub data: serde_json::Value,
    /// Record time
    pub time: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettingResponse {
    /// Setting identifier
    pub id: Id,
    /// Device identifier
    pub device_id: Id,
    /// Setting data
    pub setting: serde_json::Value,
    /// Start time
    pub start: OffsetDateTime,
    /// End time
    pub end: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoResponse {
    /// Device identifier
    pub id: Id,
    /// Parent region identifier
    pub region_id: Id,
    /// Device name
    pub name: String,
    /// Device category
    pub device_type: DeviceType,
    /// Device location data
    pub location: serde_json::Value,
    /// Current status data
    pub status: serde_json::Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    /// Basic device data
    #[serde(flatten)]
    pub info: DeviceInfoResponse,
    /// Device settings list
    pub settings: Vec<DeviceSettingResponse>,
    /// Device records list
    pub records: Vec<DeviceRecordResponse>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceValue {
    /// Window position data
    Window {
        /// Window identifier
        window_id: Id,
        /// Window state data
        data: WindowData,
    },
    /// Sensor reading data
    Sensor {
        /// Sensor identifier
        sensor_id: Id,
        /// Sensor readings
        data: SensorData,
    },
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// Current device data
    pub data: DeviceValue,
    /// Position percentage
    pub position: u8,
    /// Battery level percentage
    pub battery: u8,
    /// Last update time
    pub updated_at: OffsetDateTime,
}

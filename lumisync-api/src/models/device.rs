use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use super::settings::{SensorSettingData, SettingResponse, WindowSettingData};
use super::{Id, SensorData, WindowData};

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    /// Environmental sensor
    #[default]
    Sensor,
    /// Smart window device
    Window,
}

impl From<String> for DeviceType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "window" => DeviceType::Window,
            _ => DeviceType::Sensor,
        }
    }
}

impl core::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            DeviceType::Sensor => write!(f, "sensor"),
            DeviceType::Window => write!(f, "window"),
        }
    }
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
    pub location: Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    /// New device name
    pub name: Option<String>,
    /// New device category
    pub device_type: Option<DeviceType>,
    /// New location data
    pub location: Option<Value>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecordResponse {
    /// Record identifier
    pub id: Id,
    /// Device identifier
    pub device_id: Id,
    /// Record data
    pub data: Value,
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
    pub setting: Value,
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
    pub location: Value,
    /// Current status data
    pub status: Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    /// Basic device data
    #[serde(flatten)]
    pub info: DeviceInfoResponse,
    /// Device settings list
    pub settings: Vec<DeviceSettingUnion>,
    /// Device records list
    pub records: Vec<DeviceRecordResponse>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeviceSettingUnion {
    /// Window device settings
    Window(SettingResponse<WindowSettingData>),
    /// Sensor device settings
    Sensor(SettingResponse<SensorSettingData>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceValue {
    /// Window position data
    Window {
        #[serde(flatten)]
        data: WindowData,
    },
    /// Sensor reading data
    Sensor {
        #[serde(flatten)]
        data: SensorData,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// Current device data
    pub data: DeviceValue,
    /// Battery level percentage
    pub battery: u8,
    /// RSSI signal strength
    pub rssi: i8,
    /// Last update time
    pub updated_at: OffsetDateTime,
}

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeviceRequest {
    /// Parent region identifier.
    pub region_id: i32,
    /// Device name.
    pub name: String,
    /// Device category.
    pub device_type: i32,
    /// Device location data.
    pub location: serde_json::Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    /// New device name.
    pub name: Option<String>,
    /// New device category.
    pub device_type: Option<i32>,
    /// New location data.
    pub location: Option<serde_json::Value>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecordResponse {
    /// Record identifier.
    pub id: i32,
    /// Device identifier.
    pub device_id: i32,
    /// Record data.
    pub data: serde_json::Value,
    /// Record time.
    pub time: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettingResponse {
    /// Setting identifier.
    pub id: i32,
    /// Device identifier.
    pub device_id: i32,
    /// Setting data.
    pub setting: serde_json::Value,
    /// Start time.
    pub start: OffsetDateTime,
    /// End time.
    pub end: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoResponse {
    /// Device identifier.
    pub id: i32,
    /// Parent region identifier.
    pub region_id: i32,
    /// Device name.
    pub name: String,
    /// Device category.
    pub device_type: i32,
    /// Device location data.
    pub location: serde_json::Value,
    /// Current status data.
    pub status: serde_json::Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    /// Basic device data.
    #[serde(flatten)]
    pub info: DeviceInfoResponse,
    /// Device settings list.
    pub settings: Vec<DeviceSettingResponse>,
    /// Device records list.
    pub records: Vec<DeviceRecordResponse>,
}

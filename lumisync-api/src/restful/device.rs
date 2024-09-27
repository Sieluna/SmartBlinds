use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeviceRequest {
    pub region_id: i32,
    pub name: String,
    pub device_type: i32,
    pub location: Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub device_type: Option<i32>,
    pub location: Option<Value>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecordResponse {
    pub id: i32,
    pub device_id: i32,
    pub data: Value,
    pub time: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettingResponse {
    pub id: i32,
    pub device_id: i32,
    pub setting: Value,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoResponse {
    pub id: i32,
    pub region_id: i32,
    pub name: String,
    pub device_type: i32,
    pub location: Value,
    pub status: Value,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    #[serde(flatten)]
    pub info: DeviceInfoResponse,
    pub settings: Vec<DeviceSettingResponse>,
    pub records: Vec<DeviceRecordResponse>,
}

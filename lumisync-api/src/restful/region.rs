use serde::{Deserialize, Serialize};

use super::device::DeviceInfoResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRegionRequest {
    pub name: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRegionSettingRequest {
    pub name: Option<String>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfoResponse {
    pub id: i32,
    pub group_id: i32,
    pub name: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionResponse {
    #[serde(flatten)]
    pub info: RegionInfoResponse,
    pub light: i32,
    pub temperature: f32,
    pub devices: Vec<DeviceInfoResponse>,
}

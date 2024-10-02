use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::device::DeviceInfoResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionRole {
    #[default]
    Visitor,
    Owner,
}

impl From<String> for RegionRole {
    fn from(value: String) -> Self {
        match value.as_str() {
            "owner" => RegionRole::Owner,
            _ => RegionRole::Visitor,
        }
    }
}

impl fmt::Display for RegionRole {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RegionRole::Visitor => write!(f, "visitor"),
            RegionRole::Owner => write!(f, "owner"),
        }
    }
}

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
    pub humidity: f32,
    pub users: HashMap<i32, RegionRole>,
    pub devices: Vec<DeviceInfoResponse>,
}

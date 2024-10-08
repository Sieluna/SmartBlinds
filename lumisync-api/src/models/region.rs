use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::Id;
use super::device::DeviceInfoResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionRole {
    /// Read-only access
    #[default]
    Visitor,
    /// Full control access
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

impl core::fmt::Display for RegionRole {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            RegionRole::Visitor => write!(f, "visitor"),
            RegionRole::Owner => write!(f, "owner"),
        }
    }
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRegionRequest {
    /// Region name
    pub name: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRegionSettingRequest {
    /// New region name
    pub name: Option<String>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfoResponse {
    /// Region identifier
    pub id: Id,
    /// Parent group identifier
    pub group_id: Id,
    /// Region name
    pub name: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionResponse {
    /// Basic region data
    #[serde(flatten)]
    pub info: RegionInfoResponse,
    /// Current light level
    pub light: i32,
    /// Current temperature
    pub temperature: f32,
    /// Current humidity
    pub humidity: f32,
    /// User access list
    pub users: BTreeMap<Id, RegionRole>,
    /// Associated devices
    pub devices: Vec<DeviceInfoResponse>,
}

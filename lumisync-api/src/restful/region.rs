use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRegionRequest {
    /// Users who allowed to manage this region.
    pub users: Vec<i32>,
    /// Name of region.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRegionRequest {
    /// Update name of region.
    pub name: Option<String>,
    /// Update light intensity of region.
    pub light: Option<i32>,
    /// Update temperature of region.
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionResponse {
    /// The id of region.
    pub id: i32,
    /// The group this region belongs to.
    pub group_id: i32,
    /// Name of region.
    pub name: String,
    /// Light intensity of region.
    pub light: i32,
    /// Temperature of region.
    pub temperature: f32,
}

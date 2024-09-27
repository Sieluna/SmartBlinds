use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::region::RegionInfoResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub users: Vec<i32>,
    pub name: String,
    pub description: Option<String>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub created_at: OffsetDateTime,
    pub regions: Vec<RegionInfoResponse>,
}

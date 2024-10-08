use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Id;
use super::region::RegionInfoResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    /// Member user identifiers
    pub users: Vec<Id>,
    /// Group name
    pub name: String,
    /// Group description
    pub description: Option<String>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResponse {
    /// Group identifier
    pub id: Id,
    /// Group name
    pub name: String,
    /// Group description
    pub description: Option<String>,
    /// Creation time
    pub created_at: OffsetDateTime,
    /// Associated regions
    pub regions: Vec<RegionInfoResponse>,
}

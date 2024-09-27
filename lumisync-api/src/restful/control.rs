use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub message: String,
}

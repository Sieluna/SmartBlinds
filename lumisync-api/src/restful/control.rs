use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse<'a> {
    /// Operation result message.
    pub message: &'a str,
}

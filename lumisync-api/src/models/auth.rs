use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::Id;
use super::group::GroupResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Standard user access
    #[default]
    User,
    /// Administrative access
    Admin,
}

impl From<String> for UserRole {
    fn from(value: String) -> Self {
        match value.as_str() {
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        }
    }
}

impl core::fmt::Display for UserRole {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// User email address
    pub email: String,
    /// User password
    pub password: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// User email address
    pub email: String,
    /// User password
    pub password: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoResponse {
    /// User identifier
    pub id: Id,
    /// User email address
    pub email: String,
    /// User permission level
    pub role: UserRole,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    /// User identifier
    pub id: Id,
    /// Primary group identifier
    pub group_id: Id,
    /// User email address
    pub email: String,
    /// User permission level
    pub role: UserRole,
    /// Associated groups
    pub groups: Vec<GroupResponse>,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Authentication token
    pub token: String,
    /// Token type
    pub token_type: String,
    /// Expiration time (seconds)
    pub expires_in: u64,
}

use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

use super::group::GroupResponse;

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    #[default]
    User,
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

impl Display for UserRole {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: i32,
    pub group_id: i32,
    pub email: String,
    pub role: UserRole,
    pub groups: Vec<GroupResponse>,
}

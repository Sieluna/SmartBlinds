mod auth;
mod control;
mod device;
mod group;
mod region;

pub use auth::*;
pub use control::*;
pub use device::*;
pub use group::*;
pub use region::*;

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Member,
    Moderator,
    Admin,
    Owner,
}

impl From<String> for Role {
    fn from(value: String) -> Self {
        match value.as_str() {
            "owner" => Role::Owner,
            "admin" => Role::Admin,
            "moderator" => Role::Moderator,
            _ => Role::Member,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Role::Owner => write!(f, "owner"),
            Role::Admin => write!(f, "admin"),
            Role::Moderator => write!(f, "moderator"),
            Role::Member => write!(f, "member"),
        }
    }
}

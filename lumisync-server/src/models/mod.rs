mod event;
mod group;
mod region;
mod region_setting;
mod sensor;
mod sensor_record;
mod user;
mod user_group;
mod user_region;
mod window;
mod window_record;
mod window_setting;

pub use event::{Event, EventTable};
pub use group::{Group, GroupTable};
pub use region::{Region, RegionTable};
pub use region_setting::{RegionSetting, RegionSettingTable};
pub use sensor::{Sensor, SensorTable};
pub use sensor_record::{SensorRecord, SensorRecordTable};
pub use user::{User, UserTable};
pub use user_group::{UserGroup, UserGroupTable};
pub use user_region::{UserRegion, UserRegionTable};
pub use window::{Window, WindowTable};
pub use window_record::{WindowRecord, WindowRecordTable};
pub use window_setting::{WindowSetting, WindowSettingTable};

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

pub trait Table {
    /// The name of the table
    fn name(&self) -> &'static str;

    /// The SQL statement to create the table
    fn create(&self) -> String;

    /// The SQL statement to dispose the table
    fn dispose(&self) -> String;

    /// The dependencies of the table
    fn dependencies(&self) -> Vec<&'static str>;
}

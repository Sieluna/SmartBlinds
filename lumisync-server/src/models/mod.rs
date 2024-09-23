mod event;
mod group;
mod region;
mod region_setting;
mod sensor;
mod sensor_data;
mod setting;
mod user;
mod user_region;
mod window;
mod window_setting;

pub use event::{Event, EventTable};
pub use group::{Group, GroupTable};
pub use region::{Region, RegionTable};
pub use region_setting::{RegionSetting, RegionSettingTable};
pub use sensor::{Sensor, SensorTable};
pub use sensor_data::{SensorData, SensorDataTable};
pub use setting::{Setting, SettingTable};
pub use user::{Role, User, UserTable};
pub use user_region::{UserRegion, UserRegionTable};
pub use window::{Window, WindowTable};
pub use window_setting::{WindowSetting, WindowSettingTable};

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

mod device;
mod device_record;
mod device_setting;
mod event;
mod group;
mod region;
mod region_setting;
mod user;
mod user_group;
mod user_region;

pub use device::{Device, DeviceTable};
pub use device_record::{DeviceRecord, DeviceRecordTable};
pub use device_setting::{DeviceSetting, DeviceSettingTable};
pub use event::{Event, EventTable};
pub use group::{Group, GroupTable};
pub use region::{Region, RegionTable};
pub use region_setting::{RegionSetting, RegionSettingTable};
pub use user::{User, UserTable};
pub use user_group::{UserGroup, UserGroupTable};
pub use user_region::{UserRegion, UserRegionTable};

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

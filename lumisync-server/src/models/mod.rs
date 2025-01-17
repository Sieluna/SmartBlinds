pub mod group;
pub mod region;
pub mod region_setting;
pub mod sensor;
pub mod sensor_data;
pub mod setting;
pub mod user;
pub mod user_region;
pub mod window;
pub mod window_setting;

pub trait Table {
    fn name(&self) -> &'static str;
    fn create(&self) -> String;
    fn dispose(&self) -> String;
    fn dependencies(&self) -> Vec<&'static str>;
}
